//! Doctest extraction and execution harness (DX-DOCTEST-1).
//!
//! Extracts markdown code snippets (```ori ... ``` or ```orl ... ```) embedded
//! within `.oridoc` documentation files and `///` doc comments in `.orl` sources.
//! Executes them in-process via Cranelift JIT to verify documentation code samples.

use std::fs;
use std::path::Path;
use ori_diagnostics::SourceCache;
use crate::pipeline::{native::TestResult, frontend::run_check_source, native::run_jit};

#[derive(Debug, Clone)]
pub struct DocTestCase {
    pub file: String,
    pub line: usize,
    pub code: String,
}

/// Extract all doctest code blocks from a file or directory.
pub fn extract_doctests(path: &Path) -> Result<Vec<DocTestCase>, String> {
    let mut cases = Vec::new();
    if path.is_dir() {
        collect_doctests_dir(path, &mut cases)?;
    } else if path.is_file() {
        collect_doctests_file(path, &mut cases)?;
    }
    Ok(cases)
}

fn collect_doctests_dir(dir: &Path, cases: &mut Vec<DocTestCase>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("failed to read directory {}: {}", dir.display(), e))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let _ = collect_doctests_dir(&p, cases);
        } else if p.is_file() {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext == "oridoc" || ext == "orl" || ext == "md" {
                let _ = collect_doctests_file(&p, cases);
            }
        }
    }
    Ok(())
}

fn collect_doctests_file(file: &Path, cases: &mut Vec<DocTestCase>) -> Result<(), String> {
    let content = fs::read_to_string(file).map_err(|e| format!("failed to read file {}: {}", file.display(), e))?;
    let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

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
                trimmed.strip_prefix("///").unwrap_or(trimmed).strip_prefix(' ').unwrap_or(trimmed)
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
    let cache = SourceCache::default();
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
                case.line,
                case.code
            )
        } else {
            case.code.clone()
        };

        let temp_path = format!("<doctest:{}:{}>", case.file, case.line);
        let check_res = run_check_source(Path::new(&temp_path), full_src.clone());
        if let Ok(check_out) = check_res {
            if check_out.has_errors {
                results.push(TestResult {
                    name,
                    passed: false,
                    skipped: false,
                    stdout: String::new(),
                    stderr: format!("Type-check failed with {} diagnostic(s)", check_out.diagnostics.len()),
                    status: Some(1),
                });
                continue;
            }
        }

        // Run the doctest via temporary file or JIT
        let dir = std::env::temp_dir().join(format!("ori_doctest_{}_{}", std::process::id(), case.line));
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("main.orl");
        if fs::write(&file_path, &full_src).is_ok() {
            match run_jit(&file_path) {
                Ok(run_out) => {
                    let passed = !run_out.has_errors && run_out.exit_code == 0;
                    results.push(TestResult {
                        name,
                        passed,
                        skipped: false,
                        stdout: String::new(),
                        stderr: if passed { String::new() } else { format!("exit code: {}", run_out.exit_code) },
                        status: Some(run_out.exit_code),
                    });
                }
                Err(e) => {
                    results.push(TestResult {
                        name,
                        passed: false,
                        skipped: false,
                        stdout: String::new(),
                        stderr: e,
                        status: Some(1),
                    });
                }
            }
            let _ = fs::remove_dir_all(&dir);
        }
    }

    (results, cache)
}
