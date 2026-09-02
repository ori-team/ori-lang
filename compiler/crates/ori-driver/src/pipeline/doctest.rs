//! Doctest extraction and execution harness (DX-DOCTEST-1).
//!
//! Extracts markdown code snippets (```ori ... ``` or ```orl ... ```) embedded
//! within `.oridoc` documentation files and `///` doc comments in `.orl` sources.
//! Executes them in-process via Cranelift JIT to verify documentation code samples.

use crate::pipeline::{frontend::run_check_source, native::run_jit, native::TestResult};
use ori_diagnostics::SourceCache;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct DocTestCase {
    pub file: String,
    pub line: usize,
    pub code: String,
}

/// Extract all doctest code blocks from a file or directory.
pub fn extract_doctests(path: &Path) -> Result<Vec<DocTestCase>, String> {
    if !path.exists() {
        return Err(format!("doctest path does not exist: {}", path.display()));
    }
    let mut cases = Vec::new();
    if path.is_dir() {
        collect_doctests_dir(path, &mut cases)?;
    } else if path.is_file() {
        collect_doctests_file(path, &mut cases)?;
    }
    Ok(cases)
}

fn collect_doctests_dir(dir: &Path, cases: &mut Vec<DocTestCase>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory {}: {}", dir.display(), e))?
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                format!(
                    "failed to read directory entry in {}: {}",
                    dir.display(),
                    error
                )
            })
        })
        .collect::<Result<_, _>>()?;
    entries.sort();

    for p in entries {
        let metadata = fs::symlink_metadata(&p).map_err(|error| {
            format!("failed to inspect doctest path {}: {}", p.display(), error)
        })?;
        if metadata.is_dir() {
            collect_doctests_dir(&p, cases)?;
        } else if metadata.is_file() {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext == "oridoc" || ext == "orl" || ext == "md" {
                collect_doctests_file(&p, cases)?;
            }
        }
    }
    Ok(())
}

fn collect_doctests_file(file: &Path, cases: &mut Vec<DocTestCase>) -> Result<(), String> {
    let content = fs::read_to_string(file)
        .map_err(|e| format!("failed to read file {}: {}", file.display(), e))?;
    let file_name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut in_block = false;
    let mut block_start_line = 0;
    let mut block_lines = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let stripped = trimmed.trim_start_matches("///").trim();

        if !in_block {
            if stripped.starts_with("```ori") || stripped.starts_with("```orl") {
                in_block = true;
                block_start_line = idx + 1;
                block_lines.clear();
            }
        } else if stripped.starts_with("```") {
            in_block = false;
            let code = block_lines.join("\n");
            if !code.trim().is_empty() {
                cases.push(DocTestCase {
                    file: file_name.clone(),
                    line: block_start_line,
                    code,
                });
            }
            block_lines.clear();
        } else {
            let code_line = if trimmed.starts_with("///") {
                trimmed
                    .strip_prefix("///")
                    .unwrap_or(trimmed)
                    .strip_prefix(' ')
                    .unwrap_or(trimmed)
            } else {
                line
            };
            block_lines.push(code_line.to_string());
        }
    }

    Ok(())
}

/// Execute extracted doctests using the in-process JIT compiler.
pub fn run_doctests(cases: &[DocTestCase], filter: Option<&str>) -> (Vec<TestResult>, SourceCache) {
    let mut cache = SourceCache::default();
    let mut results = Vec::new();

    for case in cases {
        let name = format!("{} (line {})", case.file, case.line);
        if let Some(pat) = filter {
            if !name.contains(pat) {
                continue;
            }
        }

        let full_src = if !case.code.contains("module ") {
            format!(
                "module doctest_{}\n\nimport ori.io = io\n\npublic main() -> void\n{}\nend\n",
                case.line, case.code
            )
        } else {
            case.code.clone()
        };

        let temp_path = format!("<doctest:{}:{}>", case.file, case.line);
        let check_out = match run_check_source(Path::new(&temp_path), full_src.clone()) {
            Ok(check_out) => check_out,
            Err(error) => {
                results.push(failed_doctest(
                    name,
                    format!("Type-check pipeline failed: {error}"),
                ));
                continue;
            }
        };
        cache.append(check_out.cache);
        if check_out.has_errors {
            results.push(failed_doctest(
                name,
                format!(
                    "Type-check failed with {} diagnostic(s)",
                    check_out.diagnostics.len()
                ),
            ));
            continue;
        }

        // Run the doctest via a unique temporary file and the JIT. A process
        // local counter also keeps concurrent `run_doctests` calls separate.
        let temp_id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ori_doctest_{}_{}", std::process::id(), temp_id));
        if let Err(error) = fs::create_dir(&dir) {
            results.push(failed_doctest(
                name,
                format!("Could not create doctest temporary directory: {error}"),
            ));
            continue;
        }
        let file_path = dir.join("main.orl");
        let result = match fs::write(&file_path, &full_src) {
            Ok(()) => match run_jit(&file_path) {
                Ok(run_out) => {
                    let passed = !run_out.has_errors && run_out.exit_code == 0;
                    TestResult {
                        name,
                        passed,
                        skipped: false,
                        stdout: String::new(),
                        stderr: if passed {
                            String::new()
                        } else {
                            format!("doctest exited with status {}", run_out.exit_code)
                        },
                        status: Some(run_out.exit_code),
                    }
                }
                Err(error) => failed_doctest(name, format!("JIT execution failed: {error}")),
            },
            Err(error) => failed_doctest(
                name,
                format!("Could not write doctest temporary source: {error}"),
            ),
        };
        if let Err(error) = fs::remove_dir_all(&dir) {
            let cleanup_message = format!("Could not clean doctest temporary directory: {error}");
            if result.passed {
                results.push(failed_doctest(result.name, cleanup_message));
                continue;
            }
            let mut result = result;
            if result.stderr.is_empty() {
                result.stderr = cleanup_message;
            } else {
                result.stderr.push_str("; ");
                result.stderr.push_str(&cleanup_message);
            }
            results.push(result);
            continue;
        }
        results.push(result);
    }

    (results, cache)
}

fn failed_doctest(name: String, stderr: String) -> TestResult {
    TestResult {
        name,
        passed: false,
        skipped: false,
        stdout: String::new(),
        stderr,
        status: Some(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn invalid_doctest_reports_check_failure() {
        let cases = [DocTestCase {
            file: "invalid.oridoc".to_string(),
            line: 3,
            code: "const broken: int = true".to_string(),
        }];

        let (results, cache) = run_doctests(&cases, None);

        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
        assert!(results[0].stderr.contains("Type-check failed"));
        assert_eq!(results[0].status, Some(1));
        assert!(!cache.all_files().is_empty());
        assert!(cache
            .all_files()
            .iter()
            .any(|file| file.path.to_string_lossy().contains("<doctest:")));
    }

    #[test]
    fn missing_doctest_path_is_reported() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ori_missing_doctest_{unique}"));

        let error = extract_doctests(&path).expect_err("missing path must fail closed");

        assert!(error.contains("does not exist"));
    }
}
