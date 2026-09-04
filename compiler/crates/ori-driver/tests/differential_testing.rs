//! Differential testing suite (QA-DIFF-1).
//!
//! Asserts that Cranelift Native AOT, in-process JIT, and aggressive
//! optimizer passes produce 100% byte-for-byte identical observable behavior
//! across all language feature combinations.

use std::path::{Path, PathBuf};
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
            "ori_driver_diff_test_{}_{}_{}",
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

fn ori_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ori"))
}

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

fn run_jit(main_orl: &Path, opt_level: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(ori_exe());
    if let Some(opt) = opt_level {
        cmd.env("ORI_OPT", opt);
    }
    cmd.arg("run")
        .arg(main_orl)
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn JIT runner")
}

fn assert_aot_and_jit_equal(dir_name: &str, source: &str) {
    let dir = TestDir::new(dir_name);
    let src_path = dir.path("main.orl");
    dir.write("main.orl", source);

    // 1. AOT Compile and Run
    let exe = exe_path(&dir, "aot_bin");
    let compile_out = run_compile(&src_path, &exe).expect("AOT compilation failed");
    assert!(
        !compile_out.has_errors,
        "AOT compilation emitted diagnostics: {:?}",
        compile_out.diagnostics
    );
    let aot_output = Command::new(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to execute AOT binary");

    let aot_stdout = String::from_utf8_lossy(&aot_output.stdout).to_string();
    let aot_stderr = String::from_utf8_lossy(&aot_output.stderr).to_string();
    assert!(
        aot_output.status.success(),
        "AOT binary execution failed:\nstatus: {:?}\nstderr: {}",
        aot_output.status,
        aot_stderr
    );

    // 2. JIT Execution (default opt)
    let jit_output = run_jit(&src_path, None);
    let jit_stdout = String::from_utf8_lossy(&jit_output.stdout).to_string();
    let jit_stderr = String::from_utf8_lossy(&jit_output.stderr).to_string();
    assert!(
        jit_output.status.success(),
        "JIT execution failed:\nstatus: {:?}\nstderr: {}",
        jit_output.status,
        jit_stderr
    );

    // 3. JIT Execution (aggressive optimization)
    let opt_output = run_jit(&src_path, Some("aggressive"));
    let opt_stdout = String::from_utf8_lossy(&opt_output.stdout).to_string();
    let opt_stderr = String::from_utf8_lossy(&opt_output.stderr).to_string();
    assert!(
        opt_output.status.success(),
        "JIT aggressive execution failed:\nstatus: {:?}\nstderr: {}",
        opt_output.status,
        opt_stderr
    );

    // 4. Parity Assertions
    assert_eq!(
        aot_stdout, jit_stdout,
        "AOT and JIT stdout divergence for fixture `{dir_name}`:\nAOT:\n{aot_stdout}\nJIT:\n{jit_stdout}"
    );
    assert_eq!(
        jit_stdout, opt_stdout,
        "JIT standard and aggressive optimization stdout divergence for fixture `{dir_name}`:\nJIT:\n{jit_stdout}\nOPT:\n{opt_stdout}"
    );
}

#[test]
fn diff_test_multi_trait_colon_and_generic_dispatch() {
    assert_aot_and_jit_equal(
        "multi_trait_colon",
        r#"module app.main

import ori.io as io

trait Named
    name(self) -> string
end

trait Scored
    score(self) -> int
end

struct Player
    tag: string
    points: int
end

apply Player: Named, Scored
    name(self) -> string
        return self.tag
    end

    score(self) -> int
        return self.points
    end
end

get_tag for T: Named (v: T) -> string
    return v.name()
end

get_pts for T: Scored (v: T) -> int
    return v.score()
end

main()
    const p: Player = Player { tag: "ada", points: 42 }
    io.println(get_tag(p))
    io.println(string(get_pts(p)))
end
"#,
    );
}

#[test]
fn diff_test_simd_vectors_arithmetic_and_lanes() {
    assert_aot_and_jit_equal(
        "simd_diff",
        r#"module app.main

import ori.io as io

main()
    const a: simd[float32, 4] = [1.0f32, 2.0f32, 3.0f32, 4.0f32]
    const b: simd[float32, 4] = [10.0f32, 20.0f32, 30.0f32, 40.0f32]
    const c: simd[float32, 4] = a + b
    io.println(string(c[0]))
    io.println(string(c[1]))
    io.println(string(c[2]))
    io.println(string(c[3]))
end
"#,
    );
}

#[test]
fn diff_test_struct_alignment_and_size() {
    assert_aot_and_jit_equal(
        "align_diff",
        r#"module app.main

import ori.io as io
import ori.mem as mem

@align(32)
struct GpuBlock
    id: int
    val: float
end

main()
    const block: GpuBlock = GpuBlock { id: 7, val: 3.14 }
    io.println(string(mem.align_of(block)))
    io.println(string(mem.size_of(block)))
    io.println(string(block.id))
end
"#,
    );
}

#[test]
fn diff_test_scoped_memory_arena_bump_and_reset() {
    assert_aot_and_jit_equal(
        "arena_diff",
        r#"module app.main

import ori.io as io
import ori.mem as mem

struct Particle
    x: float
    y: float
end

main()
    using r: mem.Region = mem.region()
    var i: int = 0
    while i < 100
        const p: Particle = Particle { x: 1.0, y: 2.0 }
        i = i + 1
    end
    io.println("ARENA_OK")
    mem.reset(r)
    io.println("RESET_OK")
end
"#,
    );
}

#[test]
fn diff_test_closures_with_captures_and_higher_order_dispatch() {
    assert_aot_and_jit_equal(
        "closure_diff",
        r#"module app.main

import ori.io as io

apply_fn[T, R](val: T, f: func(T) -> R) -> R
    return f(val)
end

main()
    const factor: int = 10
    const add_and_mul = (x: int) => (x + 2) * factor
    const res: int = apply_fn(5, add_and_mul)
    io.println(string(res))
end
"#,
    );
}

#[test]
fn diff_test_enum_payloads_with_match_guards_and_patterns() {
    assert_aot_and_jit_equal(
        "enum_diff",
        r#"module app.main

import ori.io as io

enum Expr
    Num(val: int)
    Add(a: int, b: int)
    Tag(label: string)
end

eval_expr(e: Expr) -> string
    return match e
        case Num(val) if val > 10: "large_num"
        case Num(val): "small_num"
        case Add(a, b): string(a + b)
        case Tag(label): label
    end
end

main()
    io.println(eval_expr(Expr.Num(val: 5)))
    io.println(eval_expr(Expr.Num(val: 20)))
    io.println(eval_expr(Expr.Add(a: 30, b: 12)))
    io.println(eval_expr(Expr.Tag(label: "custom")))
end
"#,
    );
}

#[test]
fn diff_test_try_error_propagation_and_options() {
    assert_aot_and_jit_equal(
        "try_diff",
        r#"module app.main

import ori.io as io

parse_even(n: int) -> result[int, string]
    if n % 2 != 0
        return err("odd")
    end
    return ok(n)
end

compute(a: int, b: int) -> result[int, string]
    const x: int = try parse_even(a)
    const y: int = try parse_even(b)
    return ok(x + y)
end

main()
    match compute(10, 20)
        case ok(v): io.println(string(v))
        case err(e): io.println(e)
    end
    match compute(10, 21)
        case ok(v): io.println(string(v))
        case err(e): io.println(e)
    end
end
"#,
    );
}

#[test]
fn diff_test_fixed_arrays_and_iteration() {
    assert_aot_and_jit_equal(
        "array_diff",
        r#"module app.main

import ori.io as io

main()
    const arr: array[int, 4] = [10, 20, 30, 40]
    var sum: int = 0
    var i: int = 0
    while i < 4
        sum = sum + arr[i]
        i = i + 1
    end
    io.println(string(sum))
end
"#,
    );
}
