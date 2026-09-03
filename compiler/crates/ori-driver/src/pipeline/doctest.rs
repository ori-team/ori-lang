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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctestDirective {
    Success,
    Output(String),
    CompileFail(Option<String>),
    ShouldPanic,
}

pub fn parse_doctest_directives(code: &str) -> DoctestDirective {
    let mut expected_output_lines: Option<Vec<String>> = None;
    let mut in_multiline_output = false;

    for line in code.lines() {
        let trimmed = line.trim();
        if in_multiline_output {
            if let Some(rest) = trimmed.strip_prefix("--") {
                let rest_trimmed = rest.strip_prefix(' ').unwrap_or(rest);
                if let Some(ref mut lines) = expected_output_lines {
                    lines.push(rest_trimmed.to_string());
                }
                continue;
            } else {
                in_multiline_output = false;
            }
        }

        if let Some(rest) = trimmed.strip_prefix("--") {
            let rest_trimmed = rest.trim();
            if let Some(fail_arg) = rest_trimmed.strip_prefix("compile_fail") {
                let code = fail_arg
                    .strip_prefix(':')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                return DoctestDirective::CompileFail(code);
            }
            if rest_trimmed == "should_panic" || rest_trimmed == "panics" {
                return DoctestDirective::ShouldPanic;
            }
            if let Some(out_arg) = rest_trimmed.strip_prefix("output:") {
                let text = out_arg.trim();
                if text.is_empty() {
                    in_multiline_output = true;
                    expected_output_lines = Some(Vec::new());
                } else {
                    return DoctestDirective::Output(text.to_string());
                }
            }
        }
    }

    if let Some(lines) = expected_output_lines {
        return DoctestDirective::Output(lines.join("\n"));
    }

    DoctestDirective::Success
}

fn format_source_labelled_diagnostics(
    diagnostics: &[ori_diagnostics::Diagnostic],
    file: &str,
    base_line: usize,
) -> String {
    let mut out = String::new();
    for (i, diag) in diagnostics.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let severity = match diag.severity {
            ori_diagnostics::Severity::Error => "error",
            ori_diagnostics::Severity::Warning => "warning",
        };
        out.push_str(&format!("{severity} [{}]: {}", diag.code, diag.message));
        for label in &diag.labels {
            out.push_str(&format!("\n  --> {}:{}: {}", file, base_line, label.message));
        }
        if let Some(why) = &diag.why {
            out.push_str(&format!("\n  why: {why}"));
        }
        if let Some(action) = &diag.action {
            out.push_str(&format!("\n  action: {action}"));
        }
    }
    out
}

#[cfg(unix)]
struct StdoutCapture {
    saved_stdout: i32,
    pipe_read: i32,
}

#[cfg(unix)]
impl StdoutCapture {
    fn start() -> Option<Self> {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        unsafe {
            let mut fds = [0i32; 2];
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return None;
            }
            let saved = libc::dup(1);
            if saved < 0 {
                libc::close(fds[0]);
                libc::close(fds[1]);
                return None;
            }
            if libc::dup2(fds[1], 1) < 0 {
                libc::close(saved);
                libc::close(fds[0]);
                libc::close(fds[1]);
                return None;
            }
            libc::close(fds[1]);
            Some(Self {
                saved_stdout: saved,
                pipe_read: fds[0],
            })
        }
    }

    fn finish(self) -> String {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        unsafe {
            libc::dup2(self.saved_stdout, 1);
            libc::close(self.saved_stdout);

            let mut output = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = libc::read(self.pipe_read, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
                if n <= 0 {
                    break;
                }
                output.extend_from_slice(&buf[..n as usize]);
            }
            libc::close(self.pipe_read);
            String::from_utf8_lossy(&output).into_owned()
        }
    }
}

#[cfg(not(unix))]
struct StdoutCapture;

#[cfg(not(unix))]
impl StdoutCapture {
    fn start() -> Option<Self> {
        None
    }
    fn finish(self) -> String {
        String::new()
    }
}

fn wrap_doctest_source(code: &str, line: usize) -> String {
    if code.contains("module ") {
        return code.to_string();
    }
    if code.contains("main()") {
        return format!("module doctest_{line}\n\nimport ori.io = io\n\n{code}\n");
    }

    let mut imports = Vec::new();
    let mut body = Vec::new();

    for line_str in code.lines() {
        let trimmed = line_str.trim();
        if trimmed.starts_with("import ") {
            imports.push(line_str);
        } else {
            body.push(line_str);
        }
    }

    let has_io_import = imports.iter().any(|i| i.contains("ori.io"));
    let io_import_str = if has_io_import {
        ""
    } else {
        "import ori.io = io\n"
    };

    let imports_str = if imports.is_empty() {
        String::new()
    } else {
        format!("{}\n", imports.join("\n"))
    };

    format!(
        "module doctest_{line}\n\n{io_import_str}{imports_str}\npublic main() -> void\n{}\nend\n",
        body.join("\n")
    )
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

        let directive = parse_doctest_directives(&case.code);
        let full_src = wrap_doctest_source(&case.code, case.line);

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
            let rendered = format_source_labelled_diagnostics(&check_out.diagnostics, &case.file, case.line);
            match &directive {
                DoctestDirective::CompileFail(expected_code) => {
                    if let Some(code) = expected_code {
                        let matches = check_out
                            .diagnostics
                            .iter()
                            .any(|d| d.code.contains(code) || d.message.contains(code));
                        if matches {
                            results.push(TestResult {
                                name,
                                passed: true,
                                skipped: false,
                                stdout: String::new(),
                                stderr: String::new(),
                                status: Some(0),
                            });
                        } else {
                            results.push(failed_doctest(
                                name,
                                format!("Expected compile failure with `{code}`, but got:\n{rendered}"),
                            ));
                        }
                    } else {
                        results.push(TestResult {
                            name,
                            passed: true,
                            skipped: false,
                            stdout: String::new(),
                            stderr: String::new(),
                            status: Some(0),
                        });
                    }
                }
                _ => {
                    results.push(failed_doctest(
                        name,
                        format!(
                            "Type-check failed with {} diagnostic(s):\n{}",
                            check_out.diagnostics.len(),
                            rendered
                        ),
                    ));
                }
            }
            continue;
        }

        if let DoctestDirective::CompileFail(expected_code) = &directive {
            let code_str = expected_code
                .as_deref()
                .map(|c| format!(" with `{c}`"))
                .unwrap_or_default();
            results.push(failed_doctest(
                name,
                format!("Expected compile failure{code_str}, but type-checking succeeded"),
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

        let capture = StdoutCapture::start();
        let result = match fs::write(&file_path, &full_src) {
            Ok(()) => match run_jit(&file_path) {
                Ok(run_out) => {
                    let captured_stdout = capture.map(|c| c.finish()).unwrap_or_default();
                    let passed_normally = !run_out.has_errors && run_out.exit_code == 0;

                    match &directive {
                        DoctestDirective::ShouldPanic => {
                            let failed = run_out.exit_code != 0 || run_out.has_errors;
                            if failed {
                                TestResult {
                                    name,
                                    passed: true,
                                    skipped: false,
                                    stdout: captured_stdout,
                                    stderr: String::new(),
                                    status: Some(0),
                                }
                            } else {
                                failed_doctest(
                                    name,
                                    "Doctest expected to panic/fail at runtime, but exited with status 0".to_string(),
                                )
                            }
                        }
                        DoctestDirective::Output(expected) => {
                            if !passed_normally {
                                failed_doctest(name, format!("doctest exited with status {}", run_out.exit_code))
                            } else {
                                let actual_trim = captured_stdout.trim();
                                let expected_trim = expected.trim();
                                if actual_trim == expected_trim {
                                    TestResult {
                                        name,
                                        passed: true,
                                        skipped: false,
                                        stdout: captured_stdout,
                                        stderr: String::new(),
                                        status: Some(0),
                                    }
                                } else {
                                    failed_doctest(
                                        name,
                                        format!(
                                            "Doctest output mismatch:\n=== Expected ===\n{}\n=== Actual ===\n{}",
                                            expected_trim, actual_trim
                                        ),
                                    )
                                }
                            }
                        }
                        _ => {
                            TestResult {
                                name,
                                passed: passed_normally,
                                skipped: false,
                                stdout: captured_stdout,
                                stderr: if passed_normally {
                                    String::new()
                                } else {
                                    format!("doctest exited with status {}", run_out.exit_code)
                                },
                                status: Some(run_out.exit_code),
                            }
                        }
                    }
                }
                Err(error) => {
                    let _ = capture.map(|c| c.finish());
                    failed_doctest(name, format!("JIT execution failed: {error}"))
                }
            },
            Err(error) => {
                let _ = capture.map(|c| c.finish());
                failed_doctest(
                    name,
                    format!("Could not write doctest temporary source: {error}"),
                )
            }
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

    #[test]
    fn directive_parsing_recognizes_all_variants() {
        assert_eq!(
            parse_doctest_directives("const x = 1"),
            DoctestDirective::Success
        );
        assert_eq!(
            parse_doctest_directives("const x = 1\n-- compile_fail"),
            DoctestDirective::CompileFail(None)
        );
        assert_eq!(
            parse_doctest_directives("const x = 1\n-- compile_fail: type.type_mismatch"),
            DoctestDirective::CompileFail(Some("type.type_mismatch".to_string()))
        );
        assert_eq!(
            parse_doctest_directives("panic(\"fail\")\n-- should_panic"),
            DoctestDirective::ShouldPanic
        );
        assert_eq!(
            parse_doctest_directives("io.print(\"hi\")\n-- output: hi"),
            DoctestDirective::Output("hi".to_string())
        );
        assert_eq!(
            parse_doctest_directives("io.print(\"1\")\nio.print(\"2\")\n-- output:\n-- 1\n-- 2"),
            DoctestDirective::Output("1\n2".to_string())
        );
    }

    #[test]
    fn compile_fail_doctest_passes_when_error_matches() {
        let cases = [DocTestCase {
            file: "bad_type.oridoc".to_string(),
            line: 5,
            code: "const broken: int = \"hello\"\n-- compile_fail: type.type_mismatch".to_string(),
        }];

        let (results, _) = run_doctests(&cases, None);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "compile_fail doctest should pass: {:?}", results[0]);
    }

    #[test]
    fn compile_fail_doctest_fails_when_compilation_succeeds() {
        let cases = [DocTestCase {
            file: "valid.oridoc".to_string(),
            line: 5,
            code: "const valid: int = 42\n-- compile_fail".to_string(),
        }];

        let (results, _) = run_doctests(&cases, None);
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
        assert!(results[0].stderr.contains("Expected compile failure"));
    }

    #[test]
    fn source_labelled_diagnostics_rendered_on_check_failure() {
        let cases = [DocTestCase {
            file: "broken.oridoc".to_string(),
            line: 12,
            code: "const broken: int = true".to_string(),
        }];

        let (results, _) = run_doctests(&cases, None);
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
        assert!(results[0].stderr.contains("error [type.type_mismatch]"));
        assert!(results[0].stderr.contains("--> broken.oridoc:12"));
    }
}
