//! Integration tests for the JIT execution path (Rust removal Phase 3).
//!
//! These tests spawn `ori run` as a subprocess. Explicit JIT tests set
//! `ORI_USE_JIT=1`; the default-path test relies on a cargo-built cdylib.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_jit_test_{}_{}_{}",
            std::process::id(),
            id,
            name,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, source).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Locate the `ori` driver binary built alongside this test crate.
fn ori_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ori"))
}

fn run_jit(main_orl: &std::path::Path) -> std::process::Output {
    Command::new(ori_exe())
        .arg("run")
        .arg(main_orl)
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn `ori run` subprocess")
}

#[test]
fn jit_run_hello_world() {
    let dir = TestDir::new("jit_hello_world");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    io.print("hello from JIT")
end
"#,
    );

    let output = run_jit(&dir.path("main.orl"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ori run (JIT) failed: status={:?} stderr={stderr}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello from JIT"),
        "expected `hello from JIT` in stdout, got: {stdout}"
    );
}

#[test]
fn jit_run_uses_jit_by_default_when_cdylib_available() {
    let dir = TestDir::new("jit_default");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    io.print("jit default path")
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn `ori run` subprocess");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ori run (JIT default) failed: status={:?} stderr={stderr}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("jit default path"),
        "expected JIT default output, got: {stdout}"
    );
}

#[test]
fn jit_run_computes_arithmetic() {
    let dir = TestDir::new("jit_arithmetic");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const a: int = 21 * 2
    io.print(f"answer={a}")
end
"#,
    );

    let output = run_jit(&dir.path("main.orl"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ori run (JIT) failed: status={:?} stderr={stderr}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("answer=42"),
        "expected `answer=42` in stdout, got: {stdout}"
    );
}

/// Guarded cases must fall through to the next arm when the guard is false
/// (regression: guards were dropped in AST→HIR lowering — 2026-07-19).
#[test]
fn jit_match_guards_select_arm_by_condition() {
    let dir = TestDir::new("jit_match_guards");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

grade(score: int) -> string
    var out: string = ""
    match score
    case n if n >= 90:
        out = "A"
    case n if n >= 80:
        out = "B"
    case else:
        out = "C"
    end
    return out
end

main()
    io.print(grade(95) + grade(85) + grade(50))
end
"#,
    );

    let out = run_jit(&dir.path("main.orl"));
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "ABC");
}
