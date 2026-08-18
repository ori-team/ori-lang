//! Integration tests for DX scripting features (`DX-SCRIPT-1`):
//! - CLI argument forwarding in `ori run` (JIT and AOT)
//! - `ori fmt --write`, `ori fmt --check`, and recursive directory formatting
//! - `ori lint` semantic quality checks

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
            "ori_driver_dx_test_{}_{}_{}",
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

#[test]
fn e2e_cli_args_forwarding_jit() {
    let dir = TestDir::new("args_jit");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.args = args
    ori.io = io
end

main()
    const a1: string = args.get_or(1, "none")
    const a2: string = args.get_or(2, "none")
    io.println(a1)
    io.println(a2)
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .arg("--")
        .arg("hello")
        .arg("world")
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "JIT run with args failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("hello"), "stdout should contain 'hello': {stdout}");
    assert!(stdout.contains("world"), "stdout should contain 'world': {stdout}");
}

#[test]
fn e2e_cli_args_forwarding_aot() {
    let dir = TestDir::new("args_aot");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.args = args
    ori.io = io
end

main()
    const a1: string = args.get_or(1, "none")
    const a2: string = args.get_or(2, "none")
    io.println(a1)
    io.println(a2)
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .arg("--")
        .arg("foo")
        .arg("bar")
        .env("ORI_USE_AOT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run AOT");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AOT run with args failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("foo"), "stdout should contain 'foo': {stdout}");
    assert!(stdout.contains("bar"), "stdout should contain 'bar': {stdout}");
}

#[test]
fn e2e_fmt_check_and_write_directory() {
    let dir = TestDir::new("fmt_batch");
    let unformatted_a = "module app.a\n\nmain ( ) -> void\n  const x = 1\nend\n";
    let unformatted_b = "module app.b\n\nfoo ( ) -> void\nconst y = 2\nend\n";

    dir.write("src/a.orl", unformatted_a);
    dir.write("src/sub/b.orl", unformatted_b);

    // 1. Check should fail with unformatted files
    let check_out = Command::new(ori_exe())
        .arg("fmt")
        .arg(dir.path("src"))
        .arg("--check")
        .output()
        .expect("failed to spawn ori fmt --check");

    assert!(!check_out.status.success(), "fmt --check should fail on unformatted files");

    // 2. Format in-place with --write
    let write_out = Command::new(ori_exe())
        .arg("fmt")
        .arg(dir.path("src"))
        .arg("--write")
        .output()
        .expect("failed to spawn ori fmt --write");

    assert!(write_out.status.success(), "fmt --write should succeed");

    // 3. Check again should succeed
    let recheck_out = Command::new(ori_exe())
        .arg("fmt")
        .arg(dir.path("src"))
        .arg("--check")
        .output()
        .expect("failed to spawn ori fmt --check second time");

    let stderr = String::from_utf8_lossy(&recheck_out.stderr);
    assert!(recheck_out.status.success(), "fmt --check after --write should succeed: {stderr}");
}

#[test]
fn e2e_lint_detects_unused_variables_and_redundant_comparisons() {
    let dir = TestDir::new("lint_checks");
    dir.write(
        "bad.orl",
        r#"module app.bad

check_val(flag: bool) -> bool
    const unused_val = 42
    if flag == true
        return not (not flag)
    end
    return false
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("lint")
        .arg(dir.path("bad.orl"))
        .output()
        .expect("failed to spawn ori lint");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unused variable") || stderr.contains("lint.unused_variable"), "should report unused variable in {stderr}");
    assert!(stderr.contains("boolean comparison") || stderr.contains("lint.redundant_bool_comparison"), "should report redundant comparison in {stderr}");
    assert!(stderr.contains("double negation") || stderr.contains("lint.double_negation"), "should report double negation in {stderr}");
}

#[test]
fn e2e_buffer_operations() {
    let dir = TestDir::new("buffer_ops");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.buffer = buf
    ori.io = io
end

main()
    var b: buffer[int] = ori.buffer.new(4)
    ori.buffer.fill(b, 10)
    ori.buffer.set(b, 2, 42)
    const v0: int = ori.buffer.get(b, 0)
    const v2: int = ori.buffer.get(b, 2)
    const len: int = ori.buffer.len(b)
    io.println(string(v0))
    io.println(string(v2))
    io.println(string(len))
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for buffer test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "buffer test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("10"), "stdout should contain 10: {stdout}");
    assert!(stdout.contains("42"), "stdout should contain 42: {stdout}");
    assert!(stdout.contains("4"), "stdout should contain 4: {stdout}");
}

#[test]
fn e2e_buffer_stdlib_helpers() {
    let dir = TestDir::new("buffer_helpers");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.buffer = buf
    ori.io = io
end

main()
    const items = [100, 200, 300]
    var b = buf.from_list(items)
    const v1: int = buf.get_or(b, 1, -1)
    const v_out: int = buf.get_or(b, 99, -99)
    io.println(string(v1))
    io.println(string(v_out))
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for buffer helpers");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "buffer helpers failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("200"), "stdout should contain 200: {stdout}");
    assert!(stdout.contains("-99"), "stdout should contain -99: {stdout}");
}

#[test]
fn e2e_span_operations() {
    let dir = TestDir::new("span_ops");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.buffer = buf
    ori.span = sp
    ori.io = io
end

main()
    var b: buffer[int] = buf.new(8)
    buf.fill(b, 0)
    var view = sp.from_buffer(b, 2, 4)
    sp.fill(view, 77)
    sp.set_at(view, 1, 99)
    const val0: int = sp.get(view, 0)
    const val1: int = sp.get(view, 1)
    const b2: int = buf.get(b, 2)
    const b3: int = buf.get(b, 3)
    const b0: int = buf.get(b, 0)
    io.println(string(val0))
    io.println(string(val1))
    io.println(string(b2))
    io.println(string(b3))
    io.println(string(b0))
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for span test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "span test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("77"), "stdout should contain 77: {stdout}");
    assert!(stdout.contains("99"), "stdout should contain 99: {stdout}");
    assert!(stdout.contains("0"), "stdout should contain 0: {stdout}");
}

#[test]
fn e2e_extensible_namespaced_attributes() {
    let dir = TestDir::new("meta_attrs");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.io = io
end

@editor.inspect
@editor.range(min: 0, max: 100)
@schema.table(name: "users")
public struct User
    id: int
    name: string
end

@route.get(path: "/api/hello")
@auth.required
public handle_req() -> string
    return "ok"
end

main()
    io.println(handle_req())
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for meta attrs test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "meta attrs test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("ok"), "stdout should contain ok: {stdout}");
}

#[test]
fn e2e_runtime_control_rng_and_slotmap() {
    let dir = TestDir::new("runtime_ctrl");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.random = rnd
    ori.slotmap = sm
    ori.io = io
end

main()
    -- 1. Value-based independent RNG
    var rng1 = rnd.new_rng(42)
    var rng2 = rnd.new_rng(42)
    const pair1 = rnd.next_range(rng1, 1, 100)
    const pair2 = rnd.next_range(rng2, 1, 100)
    const v1_a = pair1.0
    const v2_a = pair2.0
    io.println(string(v1_a == v2_a))

    -- 2. Generational SlotMap
    var pool = sm.new()
    const k1 = sm.insert(pool, 111)
    const k2 = sm.insert(pool, 222)
    const val1 = sm.get(pool, k1)
    if some(v) = val1
        io.println(string(v))
    end

    -- Remove k1, then reuse slot with k3
    const removed = sm.remove(pool, k1)
    const k3 = sm.insert(pool, 333)

    -- Old key k1 must be rejected (stale generation)
    const stale_get = sm.get(pool, k1)
    io.println(string(stale_get == none))

    -- New key k3 must succeed
    if some(v3) = sm.get(pool, k3)
        io.println(string(v3))
    end
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for runtime ctrl test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "runtime ctrl test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("true"), "stdout should contain true: {stdout}");
    assert!(stdout.contains("111"), "stdout should contain 111: {stdout}");
    assert!(stdout.contains("333"), "stdout should contain 333: {stdout}");
}

#[test]
fn e2e_unicode_text_toolkit() {
    let dir = TestDir::new("unicode_text");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.string = str
    ori.io = io
end

main()
    const ascii_text = "hello_world"
    const unicode_text = "olá mundo 🚀"

    io.println(string(str.is_ascii(ascii_text)))
    io.println(string(str.is_ascii(unicode_text)))

    const folded = str.case_fold("GRÜSSEN")
    io.println(folded)

    const eq_ci = str.equals_ignore_case("HELLO", "hello")
    io.println(string(eq_ci))
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for unicode text test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "unicode text test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("true"), "stdout should contain true: {stdout}");
    assert!(stdout.contains("false"), "stdout should contain false: {stdout}");
    assert!(stdout.contains("grüssen"), "stdout should contain grüssen: {stdout}");
}

#[test]
fn e2e_web_runtime_foundation_http() {
    let dir = TestDir::new("web_foundation");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.net.http = http
    ori.io = io
end

main()
    -- 1. Build and parse request
    const req_str = http.build_request("POST", "/api/data", "example.com", "X-Custom: 123", "payload")
    if ok(parsed_req) = http.parse_request(req_str)
        io.println(parsed_req.method)
        io.println(parsed_req.path)
        io.println(parsed_req.body)
    end

    -- 2. Build and parse response
    const resp_str = http.build_response(200, "OK", "Content-Type: text/plain", "hello response")
    if ok(parsed_resp) = http.parse_response(resp_str)
        io.println(string(parsed_resp.status))
        io.println(parsed_resp.status_text)
        io.println(parsed_resp.body)
    end
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn ori run for web foundation test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "web foundation test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("POST"), "stdout should contain POST: {stdout}");
    assert!(stdout.contains("/api/data"), "stdout should contain /api/data: {stdout}");
    assert!(stdout.contains("payload"), "stdout should contain payload: {stdout}");
    assert!(stdout.contains("200"), "stdout should contain 200: {stdout}");
    assert!(stdout.contains("hello response"), "stdout should contain hello response: {stdout}");
}

#[test]
fn e2e_ffi_bindgen_generation() {
    let dir = TestDir::new("ffi_bindgen");
    let header_content = r#"// Sample C API header
#define MAX_BUFFER_SIZE 1024

typedef int status_t;

typedef struct Point2D {
    int x;
    int y;
} Point2D;

status_t calculate_distance(Point2D p1, Point2D p2);
void set_callback(char* name, int flags);
"#;
    dir.write("math_api.h", header_content);

    let output = Command::new(ori_exe())
        .arg("bindgen")
        .arg(dir.path("math_api.h"))
        .arg("-o")
        .arg(dir.path("math_bindings.orl"))
        .arg("-m")
        .arg("math_bindings")
        .output()
        .expect("failed to run ori bindgen");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "bindgen command failed:\nstdout: {stdout}\nstderr: {stderr}"
    );

    let generated = std::fs::read_to_string(dir.path("math_bindings.orl"))
        .expect("failed to read generated bindings file");
    assert!(generated.contains("module math_bindings"));
    assert!(generated.contains("public const MAX_BUFFER_SIZE: int = 1024"));
    assert!(generated.contains("public alias status_t = int"));
    assert!(generated.contains("@repr(\"C\")"));
    assert!(generated.contains("public struct Point2D"));
    assert!(generated.contains("x: int"));
    assert!(generated.contains("y: int"));
    assert!(generated.contains("extern \"c\""));
    assert!(generated.contains("public calculate_distance(p1: int, p2: int) -> int"));
    assert!(generated.contains("public set_callback(name: int, flags: int) -> void"));

    // Verify the generated file passes `ori check`
    let check_out = Command::new(ori_exe())
        .arg("check")
        .arg(dir.path("math_bindings.orl"))
        .output()
        .expect("failed to run ori check on generated bindings");
    assert!(
        check_out.status.success(),
        "generated bindings failed ori check:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check_out.stdout),
        String::from_utf8_lossy(&check_out.stderr)
    );
}

#[test]
fn e2e_extended_lints_prefer_const_and_shadowing() {
    let dir = TestDir::new("extended_lints");
    dir.write(
        "main.orl",
        r#"module app.main

public process(x: int) -> int
    var unmutated: int = 42
    if x > 0
        var x: int = 100
        return unmutated + x
    end
    return unmutated
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("lint")
        .arg(dir.path("main.orl"))
        .output()
        .expect("failed to run ori lint");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    assert!(combined.contains("lint.prefer_const"), "expected lint.prefer_const in output: {combined}");
    assert!(combined.contains("lint.shadowed_variable"), "expected lint.shadowed_variable in output: {combined}");
}

#[test]
fn e2e_image_ppm_and_bmp_export() {
    let dir = TestDir::new("image_export");
    let ppm_out = dir.path("output.ppm");
    let bmp_out = dir.path("output.bmp");
    let ppm_str_path = ppm_out.display().to_string();
    let bmp_str_path = bmp_out.display().to_string();

    dir.write(
        "main.orl",
        &format!(r#"module app.main

import ori.image = img
import ori.io = io

public main() -> void
    const width: int = 2
    const height: int = 2
    -- 4 pixels: red, green, blue, white
    const pixels: list[int] = [16711680, 65280, 255, 16777215]
    
    const ppm_str: string = img.encode_ppm(width, height, pixels)
    const res1 = img.write_ppm("{}", width, height, pixels)

    const bmp_bytes: bytes = img.encode_bmp(width, height, pixels)
    const res2 = img.write_bmp("{}", width, height, pixels)

    io.println("DONE")
end
"#, ppm_str_path, bmp_str_path),
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .output()
        .expect("failed to run image export test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "image export failed:\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("DONE"), "expected DONE in stdout: {stdout}");
    assert!(ppm_out.exists(), "ppm output should exist");
    assert!(bmp_out.exists(), "bmp output should exist");
}

#[test]
fn e2e_string_view_operations() {
    let dir = TestDir::new("string_view_ops");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.string_view = sv
import ori.io = io

public main() -> void
    const original: string = "Hello, Ori Language World!"
    const view = sv.from_string(original)
    
    if sv.len(view) != 26
        panic("expected len 26")
    end
    if not sv.starts_with(view, "Hello")
        panic("expected starts with Hello")
    end
    if not sv.ends_with(view, "World!")
        panic("expected ends with World!")
    end

    const sub = sv.subview(view, 7, 12)
    if sv.to_string(sub) != "Ori Language"
        panic("expected subview Ori Language")
    end

    io.println("STRING_VIEW_OK")
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .output()
        .expect("failed to run string view test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "string view failed:\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("STRING_VIEW_OK"), "expected STRING_VIEW_OK in stdout: {stdout}");
}

#[test]
fn e2e_hir_rc_elision_optimization() {
    let dir = TestDir::new("rc_elision");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

public compute(a: int, b: int) -> int
    const x: int = a + 10
    const y: int = x * 2
    const z: int = y + b
    return z
end

public main() -> void
    const res: int = compute(5, 7)
    if res != 37
        panic("computation mismatch")
    end
    io.println("RC_ELISION_OK")
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .output()
        .expect("failed to run rc elision test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "rc elision failed:\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("RC_ELISION_OK"), "expected RC_ELISION_OK in stdout: {stdout}");
}

#[test]
fn e2e_structured_concurrency_and_cancel_scope() {
    let dir = TestDir::new("struct_conc");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.cancel = cancel
import ori.concurrent = conc
import ori.io = io

public main() -> void
    const scope: cancel.CancelScope = cancel.create_scope()
    if cancel.is_cancelled(scope)
        panic("scope should initially not be cancelled")
    end

    cancel.cancel(scope)
    if not cancel.is_cancelled(scope)
        panic("scope should be cancelled after cancel()")
    end

    const val: int = conc.transfer_int(42)
    if val != 42
        panic("transfer_int failed")
    end

    const s: string = conc.transfer_string("async_msg")
    if s != "async_msg"
        panic("transfer_string failed")
    end

    io.println("STRUCT_CONC_OK")
end
"#,
    );

    let output = Command::new(ori_exe())
        .arg("run")
        .arg(dir.path("main.orl"))
        .env("ORI_USE_JIT", "1")
        .output()
        .expect("failed to run structured concurrency test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "structured concurrency failed:\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("STRUCT_CONC_OK"), "expected STRUCT_CONC_OK in stdout: {stdout}");
}

#[test]
fn e2e_doctest_extraction_and_execution() {
    let dir = TestDir::new("doctest_demo");
    dir.write(
        "math.oridoc",
        r#"# Math Utilities

Computes integer multiplication.

```ori
const x: int = 6 * 7
if x != 42
    panic("math doctest failure")
end
```
"#,
    );

    let output = Command::new(ori_exe())
        .arg("test")
        .arg("--doc")
        .arg(dir.path("math.oridoc"))
        .output()
        .expect("failed to run doctest command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "doctest failed:\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.contains("ok:") || stdout.contains("ok:"), "expected ok in test output:\nstdout: {stdout}\nstderr: {stderr}");
}

#[test]
fn e2e_daemon_jsonrpc_compilation_and_diagnostics() {
    use std::io::Write;

    let dir = TestDir::new("daemon_demo");
    dir.write(
        "sample.orl",
        r#"module app.main

import ori.io = io

public main() -> void
    io.println("DAEMON_EVAL_OK")
end
"#,
    );

    let mut child = Command::new(ori_exe())
        .arg("daemon")
        .arg("--stdio")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn daemon process");

    {
        let stdin = child.stdin.as_mut().expect("failed to get stdin");
        let check_req = format!(
            r#"{{"jsonrpc":"2.0","method":"check","params":{{"file":"{}"}},"id":1}}"#,
            dir.path("sample.orl").display()
        );
        writeln!(stdin, "{}", check_req).unwrap();

        let fmt_req = r#"{"jsonrpc":"2.0","method":"fmt","params":{"code":"module app.main\npublic foo()->int\nreturn 42\nend\n"},"id":2}"#;
        writeln!(stdin, "{}", fmt_req).unwrap();

        let shutdown_req = r#"{"jsonrpc":"2.0","method":"shutdown","id":3}"#;
        writeln!(stdin, "{}", shutdown_req).unwrap();
    }

    let output = child.wait_with_output().expect("daemon failed to exit");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "daemon process failed with status {:?}", output.status);
    assert!(stdout.contains(r#""has_errors":false"#), "expected has_errors:false in check response: {stdout}");
    assert!(stdout.contains(r#""result":{"status":"shutdown"}"#), "expected shutdown response: {stdout}");
}



