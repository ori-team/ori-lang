//! Complex feature interaction matrix tests.
//!
//! Validates non-trivial interactions between language subsystems:
//! - `@align(N)` structs inside `array[T, N]` with arithmetic traits
//! - Generic functions with `for T: Trait` + closures with captures + `using` blocks
//! - Enums with generic payloads + match guards + `try` error propagation
//! - Scoped bump arenas (`mem.region`) holding value aggregates with bulk reset
//! - Inherent methods manipulating mixed SIMD and fixed array fields

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
            "ori_driver_interaction_test_{}_{}_{}",
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
fn interaction_aligned_struct_in_array_with_arithmetic_traits() {
    let dir = TestDir::new("aligned_array_traits");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io
import ori.mem as mem
import ori.core as core

@align(16)
struct Vec4
    x: float
    y: float
    z: float
    w: float
end

apply Vec4: core.Addable
    add(self, other: Vec4) -> Vec4
        return Vec4 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    end
end

struct ParticleCluster
    particles: array[Vec4, 4]
end

main()
    const p1: Vec4 = Vec4 { x: 1.0, y: 2.0, z: 3.0, w: 4.0 }
    const p2: Vec4 = Vec4 { x: 10.0, y: 20.0, z: 30.0, w: 40.0 }
    const sum: Vec4 = p1 + p2

    const cluster: ParticleCluster = ParticleCluster {
        particles: [p1, p2, sum, Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 }],
    }

    check mem.align_of(p1) == 16, "align vector"
    check mem.size_of(p1) == 32, "size vector 32"
    check mem.size_of(cluster.particles) == 128, "4 * 32 bytes contiguous"

    const third: Vec4 = cluster.particles[2]
    check third.x == 11.0, "x sum"
    check third.y == 22.0, "y sum"
    check third.z == 33.0, "z sum"
    check third.w == 44.0, "w sum"
    io.println("ALIGNED_ARRAY_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "aligned_array");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("ALIGNED_ARRAY_OK"));
}

#[test]
fn interaction_generic_trait_with_closures_and_using_cleanup() {
    let dir = TestDir::new("generic_closure_using");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io
import ori.core as core

struct ManagedResource
    tag: string
    active: bool

    deactivate(self) -> void
        io.println("DEACTIVATED_" + self.tag)
    end
end

apply ManagedResource: core.Disposable
    mut dispose(self) -> void
        self.deactivate()
    end
end

execute_with_resource[R, T](res: R, action: func(R) -> T) -> T
    return action(res)
end

main()
    using res: ManagedResource = ManagedResource { tag: "DB_CONN", active: true }
    
    const multiplier: int = 5
    const action: func(ManagedResource) -> int = (r: ManagedResource) => 42 * multiplier
    
    const outcome: int = execute_with_resource(res, action)
    check outcome == 210, "action computed"
    io.println("BEFORE_BLOCK_EXIT")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "closure_using");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("BEFORE_BLOCK_EXIT"));
    assert!(stdout.contains("DEACTIVATED_DB_CONN"));
}

#[test]
fn interaction_complex_enum_guards_and_try_propagation() {
    let dir = TestDir::new("enum_guards_try");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io

enum Token
    Number(val: int)
    Identifier(name: string)
    Operator(symbol: string)
    Eof
end

parse_positive_number(t: Token) -> result[int, string]
    return match t
        case Number(val) if val > 0: ok(val)
        case Number(val) if val == 0: err("zero not allowed")
        case Number(val): err("negative number")
        case Identifier(name): err("expected number, got ident: " + name)
        case Operator(symbol): err("expected number, got op: " + symbol)
        case Eof: err("unexpected eof")
    end
end

evaluate_pair(a: Token, b: Token) -> result[int, string]
    const x: int = try parse_positive_number(a)
    const y: int = try parse_positive_number(b)
    return ok(x * y)
end

main()
    const t1: Token = Token.Number(val: 12)
    const t2: Token = Token.Number(val: 5)
    const t_zero: Token = Token.Number(val: 0)
    const t_ident: Token = Token.Identifier(name: "foo")

    match evaluate_pair(t1, t2)
        case ok(val):
            check val == 60, "12 * 5"
            io.println("PAIR_EVAL_OK")
        case err(e):
            panic("should have succeeded: " + e)
    end

    match evaluate_pair(t1, t_zero)
        case ok(_):
            panic("should have failed on zero")
        case err(e):
            check e == "zero not allowed", "zero error caught"
            io.println("ZERO_CHECK_OK")
    end

    match evaluate_pair(t1, t_ident)
        case ok(_):
            panic("should have failed on ident")
        case err(e):
            check e == "expected number, got ident: foo", "ident error caught"
            io.println("IDENT_CHECK_OK")
    end
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "enum_guards");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("PAIR_EVAL_OK"));
    assert!(stdout.contains("ZERO_CHECK_OK"));
    assert!(stdout.contains("IDENT_CHECK_OK"));
}

#[test]
fn interaction_struct_with_simd_and_fixed_array_methods() {
    let dir = TestDir::new("simd_array_struct");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io

struct TransformState
    position: simd[float32, 4]
    matrix: array[float, 4]
    tick: int

    translate(self, offset: simd[float32, 4]) -> simd[float32, 4]
        return self.position + offset
    end

    matrix_sum(self) -> float
        var total: float = 0.0
        var i: int = 0
        while i < 4
            total = total + self.matrix[i]
            i = i + 1
        end
        return total
    end
end

main()
    const initial_pos: simd[float32, 4] = [10.0f32, 20.0f32, 30.0f32, 1.0f32]
    const delta: simd[float32, 4] = [1.0f32, 2.0f32, 3.0f32, 0.0f32]

    const state: TransformState = TransformState {
        position: initial_pos,
        matrix: [1.0, 0.0, 0.0, 1.0],
        tick: 1,
    }

    const new_pos: simd[float32, 4] = state.translate(delta)
    check new_pos[0] == 11.0f32, "x pos"
    check new_pos[1] == 22.0f32, "y pos"
    check new_pos[2] == 33.0f32, "z pos"

    const msum: float = state.matrix_sum()
    check msum == 2.0, "identity matrix diagonal sum"
    io.println("SIMD_ARRAY_STRUCT_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "simd_struct");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("SIMD_ARRAY_STRUCT_OK"));
}

#[test]
fn interaction_arena_region_nested_aggregates_and_bulk_resets() {
    let dir = TestDir::new("arena_nested");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io as io
import ori.mem as mem

struct Node
    id: int
    weight: float
end

struct FrameBatch
    count: int
    header: string
end

main()
    using region: mem.Region = mem.region()

    var frame: int = 0
    while frame < 10
        -- In each frame, simulate allocating multiple batch items
        var i: int = 0
        while i < 50
            const n: Node = Node { id: i, weight: float(i) * 1.5 }
            i = i + 1
        end

        const batch: FrameBatch = FrameBatch {
            count: 50,
            header: "FRAME_" + string(frame),
        }
        check batch.count == 50, "frame items"

        -- Reset memory arena instantaneously without touching object tree
        mem.reset(region)
        frame = frame + 1
    end

    io.println("ARENA_BATCHES_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "arena_nested");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("ARENA_BATCHES_OK"));
}
