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
    run_jit_at_opt_level(main_orl, None)
}

fn run_jit_at_opt_level(
    main_orl: &std::path::Path,
    opt_level: Option<&str>,
) -> std::process::Output {
    let mut command = Command::new(ori_exe());
    if let Some(opt_level) = opt_level {
        command.env("ORI_OPT", opt_level);
    }
    command
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

#[test]
fn jit_optimizer_keeps_unused_integer_division_trap() {
    let dir = TestDir::new("jit_unused_division");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const unused: int = 1 / 0
    io.print("unreachable")
end
"#,
    );

    let output = run_jit(&dir.path("main.orl"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "JIT must preserve an unused division trap, got stdout={:?} stderr={stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("ori integer division or remainder by zero"),
        "unexpected JIT trap diagnostic: {stderr}"
    );
}

#[test]
fn jit_optimizer_keeps_remaining_traps_at_every_opt_level() {
    let cases = [
        (
            "remainder",
            r#"module app.main

import ori.io = io

main()
    var divisor: int = 0
    const unused: int = 1 % divisor
    io.print("unreachable")
end
"#,
            "ori integer division or remainder by zero",
        ),
        (
            "shift",
            r#"module app.main

import ori.io = io

main()
    var count: int = 64
    const unused: int = 1 << count
    io.print("unreachable")
end
"#,
            "ori shift count out of range",
        ),
        (
            "index",
            r#"module app.main

import ori.io = io

main()
    const values: list[int] = [1]
    const unused: int = values[2]
    io.print("unreachable")
end
"#,
            "ori list index out of bounds",
        ),
        (
            "field_contract",
            r#"module app.main

import ori.io = io

struct Positive
    value: int if it > 0
end

main()
    const unused: Positive = Positive { value: 0 }
    io.print("unreachable")
end
"#,
            "contract.field_violation",
        ),
    ];

    for opt_level in ["none", "default", "aggressive"] {
        for (case_name, source, expected_diagnostic) in cases {
            let dir = TestDir::new(&format!("jit_unused_{case_name}_{opt_level}"));
            dir.write("main.orl", source);
            let output = run_jit_at_opt_level(&dir.path("main.orl"), Some(opt_level));
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success(),
                "JIT must preserve `{case_name}` at ORI_OPT={opt_level}, stdout={:?}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(
                stderr.contains(expected_diagnostic),
                "unexpected `{case_name}` diagnostic at ORI_OPT={opt_level}: {stderr}"
            );
        }
    }
}

#[test]
fn jit_aggressive_inlining_preserves_call_boundaries() {
    let dir = TestDir::new("jit_aggressive_inline_boundaries");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

var current: int = 1
var calls: int = 0

change() -> int
    current = 2
    return 0
end

produce() -> int
    calls = calls + 1
    return 4
end

combine(first: int, second: int) -> int
    return first + change() + second
end

twice(value: int) -> int
    return value + value
end

positive(value: int if it > 0) -> int
    return value
end

main()
    io.println(f"{combine(current, current)}")
    io.println(f"{twice(produce())}")
    io.println(f"{calls}")
    io.println(f"{positive(0)}")
end
"#,
    );

    let output = run_jit_at_opt_level(&dir.path("main.orl"), Some("aggressive"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "parameter contract must still abort under JIT aggressive inlining"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "2\n8\n1\n"
    );
    assert!(
        stderr.contains("contract.param_violation"),
        "aggressive inlining erased the parameter contract: {stderr}"
    );
}

#[test]
fn jit_aggressive_inlining_keeps_unused_trapping_arguments() {
    let dir = TestDir::new("jit_aggressive_inline_unused_trap");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

ignore(value: int) -> int
    return 1
end

main()
    io.print(f"{ignore(1 / 0)}")
end
"#,
    );

    let output = run_jit_at_opt_level(&dir.path("main.orl"), Some("aggressive"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "unused trapping argument was omitted"
    );
    assert!(
        stderr.contains("ori integer division or remainder by zero"),
        "unexpected JIT trap diagnostic: {stderr}"
    );
}

#[test]
fn jit_run_uses_the_manifest_cfg_selection() {
    let dir = TestDir::new("jit_cfg_selection");
    dir.write(
        "ori.proj",
        r#"manifest = 1
name = "jit_cfg"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[features]
default = ["selected"]
selected = []
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

@cfg(feature: selected)
configured_value() -> int
    return 42
end

@cfg(not(feature: selected))
configured_value() -> string
    return missing_name
end

main()
    io.print(string(configured_value()))
end
"#,
    );

    let output = run_jit(&dir.path("main.orl"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ori run (JIT cfg) failed: status={:?} stderr={stderr}",
        output.status
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "42\n");
}

#[test]
fn jit_path_relative_keeps_list_segments_alive() {
    let dir = TestDir::new("jit_path_relative_lifetime");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.path = path

main()
    io.print(path.relative("a/b/c", "a/b"))
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
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "c\n");
}

#[test]
fn jit_run_custom_destructor_before_field_cleanup() {
    let dir = TestDir::new("jit_custom_destructor");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.io = io

struct Resource
    label: string
end

apply Resource use core.Destructor
    mut destroy(self)
        io.println("destroy:" + self.label)
    end
end

consume()
    const resource: Resource = Resource { label: "jit" + "-resource" }
end

main()
    consume()
    io.println("done")
end
"#,
    );

    for opt_level in ["none", "default", "aggressive"] {
        let output = run_jit_at_opt_level(&dir.path("main.orl"), Some(opt_level));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "ori run (JIT) failed at ORI_OPT={opt_level}: status={:?} stderr={stderr}",
            output.status
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "destroy:jit-resource\ndone\n",
            "custom destructor changed at ORI_OPT={opt_level}"
        );
    }
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

/// `match` used as a value must pick exactly one arm (0.4 surface).
#[test]
fn jit_match_expression_selects_arm_value() {
    let dir = TestDir::new("jit_match_expr");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

grade(score: int) -> string
    return match score
    case n if n >= 90: "A"
    case n if n >= 80: "B"
    case else: "C"
    end
end

main()
    io.print(grade(95) + grade(85) + grade(10))
end
"#,
    );

    let out = run_jit(&dir.path("main.orl"));
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "ABC");
}
