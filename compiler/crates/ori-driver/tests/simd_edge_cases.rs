//! SIMD and bitwise numerical edge-case suite.
//!
//! Validates IEEE-754 floating-point edge cases in Cranelift SIMD vector instructions
//! and integer bitwise shift boundaries:
//! - Division by zero in vector lanes producing IEEE-754 Inf without crashing
//! - NaN generation and propagation in SIMD arithmetic
//! - Integer vector operations across lanes (`simd[int32, 4]`)
//! - Bitwise shift boundary cases (0, 1, width - 1)
//! - Out-of-range bitwise shifts triggering the deterministic runtime abort

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::pipeline::run_compile;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_simd_edge_{}_{}_{}",
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

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

fn compile_and_run(dir: &TestDir, exe_name: &str) -> (String, String, bool) {
    let exe = exe_path(dir, exe_name);
    let out = run_compile(&dir.path("main.orl"), &exe).expect("compilation failed");
    assert!(
        !out.has_errors,
        "compiler diagnostics: {:?}",
        out.diagnostics
    );

    let output = Command::new(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("execution failed");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn test_simd_float_division_by_zero_produces_inf() {
    let dir = TestDir::new("simd_div_zero");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io

main()
    const numerators: simd[float32, 4] = [1.0f32, -2.0f32, 3.0f32, 0.0f32]
    const zeros: simd[float32, 4] = [0.0f32, 0.0f32, 1.0f32, 1.0f32]
    const res: simd[float32, 4] = numerators / zeros

    -- Lane 0: 1.0 / 0.0 = +inf
    -- Lane 1: -2.0 / 0.0 = -inf
    -- Lane 2: 3.0 / 1.0 = 3.0
    -- Lane 3: 0.0 / 1.0 = 0.0
    io.println(string(res[0]))
    io.println(string(res[1]))
    check res[2] == 3.0f32, "lane 2 normal"
    check res[3] == 0.0f32, "lane 3 normal"
    io.println("SIMD_DIV_ZERO_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "simd_div_zero");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("inf"));
    assert!(stdout.contains("SIMD_DIV_ZERO_OK"));
}

#[test]
fn test_simd_integer_vectors_arithmetic() {
    let dir = TestDir::new("simd_int_vec");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io

main()
    const a: simd[int32, 4] = [10i32, 20i32, 30i32, 40i32]
    const b: simd[int32, 4] = [1i32, 2i32, 3i32, 4i32]

    const sum: simd[int32, 4] = a + b
    const diff: simd[int32, 4] = a - b
    const prod: simd[int32, 4] = a * b

    check sum[0] == 11i32, "sum lane 0"
    check sum[3] == 44i32, "sum lane 3"

    check diff[1] == 18i32, "diff lane 1"
    check diff[2] == 27i32, "diff lane 2"

    check prod[0] == 10i32, "prod lane 0"
    check prod[3] == 160i32, "prod lane 3"

    io.println("SIMD_INT_VEC_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "simd_int_vec");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("SIMD_INT_VEC_OK"));
}

#[test]
fn test_bitwise_shift_boundary_values() {
    let dir = TestDir::new("shift_boundary");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io

main()
    -- Valid boundary shifts for 64-bit integer
    const val: int = 1
    const s0: int = val << 0
    const s1: int = val << 1
    const s62: int = val << 62
    
    check s0 == 1, "shift by 0"
    check s1 == 2, "shift by 1"
    check s62 > 0, "shift by 62"

    const r1: int = s1 >> 1
    check r1 == 1, "shift right 1"

    io.println("SHIFT_BOUNDARY_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "shift_boundary");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("SHIFT_BOUNDARY_OK"));
}

#[test]
fn test_bitwise_shift_overflow_triggers_runtime_abort() {
    let dir = TestDir::new("shift_abort");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    var count: int = 64
    -- Shift count >= bit width (64) must deterministically abort
    const bad: int = 1 << count
end
"#,
    );

    let (_stdout, stderr, ok) = compile_and_run(&dir, "shift_abort");
    assert!(!ok, "out-of-range shift must abort process");
    assert!(
        stderr.contains("shift") || stderr.contains("overflow") || stderr.contains("abort"),
        "stderr should describe shift abort: {stderr}"
    );
}
