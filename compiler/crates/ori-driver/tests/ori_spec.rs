// Comprehensive Ori language spec tests, organized by the 10-part test prompt.
// Uses the same TestDir + pipeline helpers as the other ori-driver integration tests.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::package::{
    run_get_dependencies, run_install_package, run_publish_package, GetDependenciesOptions,
    InstallPackageOptions, PublishPackageOptions,
};
use ori_driver::pipeline::{
    run_build, run_check, run_compile, run_doc, run_fmt, run_new_project, CheckOutput,
    NewProjectKind, NewProjectOptions,
};

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_spec_test_{}_{}_{}",
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
        let full = self.path(name);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, source).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn diagnostic_codes(out: &CheckOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code).collect()
}

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

// ─── Part 1 — Lexical Structure ───────────────────────────────────────────────

#[test]
fn lex_accepts_line_comment_in_expression() {
    let dir = TestDir::new("lex_comment_in_expr");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const x: int = 1 + -- comment inline
        2
    io.print(string(x))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_accepts_multiline_block_comment() {
    let dir = TestDir::new("lex_block_comment");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
--|
line 1
line 2
line 3
line 4
line 5
|--
main()
    io.print("ok")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_rejects_unclosed_block_comment() {
    let dir = TestDir::new("lex_unclosed_block");
    dir.write(
        "main.orl",
        "module app.main\n--| unclosed block comment\nmain()\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"lex.unclosed_block_comment"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn lex_rejects_unterminated_string() {
    let dir = TestDir::new("lex_unterminated_string");
    dir.write(
        "main.orl",
        "module app.main\nmain()\n    const text: string = \"open\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.unterminated_string"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn parse_rejects_unterminated_block() {
    let dir = TestDir::new("parse_unterminated_block");
    dir.write(
        "main.orl",
        "module app.main\nmain()\n    const answer: int = 42\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.unterminated_block"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn doc_accepts_documentation_comment_with_param_and_returns() {
    let dir = TestDir::new("doc_comment_params");
    dir.write(
        "main.orl",
        r#"module app.main

--|
Computes an area.

@param width  Width in pixels.
@param height Height in pixels.
@returns The computed area.
|--
public area(width: int, height: int) -> int
    return width * height
end

main()
end
"#,
    );
    let out = run_doc(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.markdown.contains("Computes an area."));
    assert!(out.markdown.contains("`width`: Width in pixels."));
    assert!(out.markdown.contains("`height`: Height in pixels."));
    assert!(out.markdown.contains("Returns: The computed area."));
}

#[test]
fn doc_warns_param_name_mismatch() {
    let dir = TestDir::new("doc_param_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

--|
@param wrong_name This parameter does not exist.
@returns A value.
|--
public area(width: int, height: int) -> int
    return width * height
end

main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        diagnostic_codes(&out).contains(&"doc.param_name_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn doc_warns_missing_return_tag() {
    let dir = TestDir::new("doc_missing_return");
    dir.write(
        "main.orl",
        r#"module app.main

--|
Computes an area.

@param width Width in pixels.
@param height Height in pixels.
|--
public area(width: int, height: int) -> int
    return width * height
end

main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(codes.contains(&"doc.missing_return"), "{codes:?}");
}

#[test]
fn lex_accepts_integer_literal_variants() {
    let dir = TestDir::new("lex_int_literals");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const a: int = 1_000_000
    const b: int = 0xFF
    const c: int = 0b1010_1010
    const d: int = 0o755
    const e: int8 = 42i8
    const f: u64 = 42u64
    const g: u8 = 0u8
    io.print(string(a + b + c + d + int(e) + int(f) + int(g)))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_rejects_invalid_integer_width() {
    let dir = TestDir::new("lex_invalid_width");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const x: int = 42i128
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_accepts_string_escape_sequences() {
    let dir = TestDir::new("lex_string_escapes");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const s: string = "backslash: \\ quote: \" newline: \n return: \r tab: \t null: \0 smile: \u{1F600}"
    io.print(s)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_accepts_triple_quote_multiline_string() {
    let dir = TestDir::new("lex_triple_quote");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const s: string = """line one
    line two
        line three"""
    io.print(s)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_accepts_interpolated_string() {
    let dir = TestDir::new("lex_fstring");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct User
    name: string
end
main()
    const user: User = User { name: "Ada" }
    io.print(f"user: {user.name}")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_accepts_byte_string() {
    let dir = TestDir::new("lex_byte_string");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const b: bytes = b"\xFF\x00"
    io.print("ok")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_rejects_unicode_escape_in_byte_string() {
    let dir = TestDir::new("lex_byte_unicode");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const b: bytes = b"\u{0041}"
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.byte_unicode_escape"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn lex_accepts_range_literals() {
    let dir = TestDir::new("lex_ranges");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var total: int = 0
    for i in 0..9
        total = total + 1
    end
    for i in 9..0
        total = total + 1
    end
    for i in 5..5
        total = total + 1
    end
    io.print(string(total))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_accepts_times_as_contextual_identifier() {
    let dir = TestDir::new("lex_times_contextual");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var times: int = 1
    times = times + 2
    io.print(string(times))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn lex_rejects_loop_as_identifier() {
    let dir = TestDir::new("lex_loop_reserved");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    var loop: int = 1
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
}

// ─── Part 2 — Type System ─────────────────────────────────────────────────────

#[test]
fn type_accepts_all_primitives() {
    let dir = TestDir::new("type_primitives");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const b: bool = true
    const i: int = 42
    const v8: int8 = 42i8
    const v16: int16 = 42i16
    const v32: int32 = 42i32
    const v64: int64 = 42i64
    const w8: u8 = 42u8
    const w16: u16 = 42u16
    const w32: u32 = 42u32
    const w64: u64 = 42u64
    const f: float = 3.14
    const f64v: float64 = 3.14
    const s: string = "hello"
    io.print(s)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_default_int_and_float() {
    let dir = TestDir::new("type_defaults");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
takes_int(x: int)
end
takes_float(x: float)
end
main()
    takes_int(42)
    takes_float(3.14)
    io.print("ok")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_accepts_struct_literal_construction() {
    let dir = TestDir::new("type_struct_literal");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Point
    x: int
    y: int
end
main()
    const p: Point = Point { x: 1, y: 2 }
    io.print(string(p.x + p.y))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_accepts_local_nim_style_inference() {
    let dir = TestDir::new("type_local_nim_infer");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct User
    name: string
    age: int
end

main()
    const n = 1
    const name = "Ada"
    const flag = true
    const u = User { name: "Ada", age: 36 }
    const xs = [1, 2, 3]
    io.print(f"{n} {name} {flag} {u.name}")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_rejects_local_inference_on_try() {
    let dir = TestDir::new("type_local_infer_try");
    dir.write(
        "main.orl",
        r#"module app.main

alias TextResult = result[string, string]

load() -> TextResult
    return ok("ok")
end

main() -> TextResult
    const raw = try load()
    return ok(raw)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected inference failure on try");
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "type.local_inference_failed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn type_rejects_local_inference_on_empty_list() {
    let dir = TestDir::new("type_local_infer_empty_list");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const xs = []
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected inference failure on empty list");
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "type.local_inference_failed"),
        "{:?}",
        out.diagnostics
    );
}

/// Option B local inference: field / index / call / pipe with known return.
#[test]
fn type_accepts_local_inference_option_b_field_index_call_pipe() {
    let dir = TestDir::new("type_local_infer_b");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

struct User
    name: string
    age: int
end

double(x: int) -> int
    return x * 2
end

main()
    const u = User { name: "Ada", age: 36 }
    const n = u.name
    const a = u.age
    const d = double(21)
    const xs = [10, 20, 30]
    const first = xs[0]
    const upper = str.to_upper("hi")
    const via_pipe = 21 |> double
    io.print(f"{n} {a} {d} {first} {upper} {via_pipe}")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_rejects_local_inference_on_void_call() {
    let dir = TestDir::new("type_local_infer_void");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const x = io.print("hi")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected inference failure on void");
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.code == "type.local_inference_failed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn type_accepts_enum_named_variants() {
    let dir = TestDir::new("type_enum_named");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
enum Shape
    Circle(radius: float)
    Rectangle(width: float, height: float)
    Dot
end
main()
    const c: Shape = Shape.Circle(radius: 1.0)
    const r: Shape = Shape.Rectangle(width: 2.0, height: 3.0)
    const d: Shape = Shape.Dot()
    io.print("ok")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_accepts_contextual_enum_variant_call_shorthand() {
    let dir = TestDir::new("type_enum_variant_call_shorthand");
    dir.write(
        "main.orl",
        r#"module app.main
enum Shape
    Circle(radius: float)
    Rectangle(width: float, height: float)
end
main()
    const c: Shape = .Circle(radius: 1.0)
    const r: Shape = .Rectangle(width: 2.0, height: 3.0)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_rejects_braced_enum_variant_shorthand() {
    let dir = TestDir::new("type_enum_variant_braced_shorthand");
    dir.write(
        "main.orl",
        r#"module app.main
enum Shape
    Rectangle(width: float, height: float)
end
main()
    const r: Shape = .Rectangle{width: 2.0, height: 3.0}
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_accepts_tuple() {
    let dir = TestDir::new("type_tuple");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const pair: tuple[int, string] = tuple(1, "one")
    io.print(string(pair.0))
    io.print(pair.1)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_rejects_out_of_bounds_tuple_index() {
    let dir = TestDir::new("type_tuple_oob");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const pair: tuple[int, string] = tuple(1, "one")
    const x: int = pair.2
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.tuple_index_out_of_bounds"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn type_accepts_optional_some_and_none() {
    let dir = TestDir::new("type_optional");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const x: optional[int] = some(5)
    const y: optional[int] = none
    if some(v) = x
        io.print(string(v))
    end
    match y
        case some(v):
            io.print(string(v))
        case none:
            io.print("none")
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_accepts_result_success_and_error() {
    let dir = TestDir::new("type_result");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("zero")
    end
    return ok(a / b)
end
main()
    match divide(10, 2)
        case ok(v):
            io.print(string(v))
        case err(m):
            io.print(m)
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_rejects_success_without_payload_for_non_void_result() {
    let dir = TestDir::new("type_ok_void_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main
bad() -> result[int, string]
    return ok()
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"contract.ok_void_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn type_accepts_equality_on_int_and_string() {
    let dir = TestDir::new("type_equality");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    check 1 + 1 == 2, "int equality"
    check "ab" == "ab", "string equality"
    io.print("ok")
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_rejects_equality_on_function_types() {
    let dir = TestDir::new("type_eq_func");
    dir.write(
        "main.orl",
        r#"module app.main
id(x: int) -> int
    return x
end
main()
    check id == id, "eq"
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.comparison_not_supported")
            || diagnostic_codes(&out).contains(&"type.comparison_type_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn type_rejects_struct_equality_with_unsupported_field() {
    let dir = TestDir::new("type_struct_equality_unsupported_field");
    dir.write(
        "main.orl",
        r#"module app.main
struct Handler
    run: func(int) -> int
end
id(x: int) -> int
    return x
end
main()
    const a: Handler = Handler { run: id }
    const b: Handler = Handler { run: id }
    const same: bool = a == b
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.equality_unsupported_field"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn type_accepts_type_alias() {
    let dir = TestDir::new("type_alias");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
alias UserId = int
alias Callback = func(string) -> bool
takes_id(id: UserId)
    io.print(string(id))
end
main()
    takes_id(42)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn type_alias_accepts_underlying_type_interchangeably() {
    let dir = TestDir::new("type_alias_interchange");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
alias UserId = int
takes_int(x: int)
    io.print(string(x))
end
main()
    const uid: UserId = 100
    takes_int(uid)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

// ─── Part 3 — Expressions ─────────────────────────────────────────────────────

#[test]
fn expr_accepts_arithmetic_and_division() {
    let dir = TestDir::new("expr_arithmetic");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const a: int = 10 / 3
    const b: int = 10 % 3
    const c: int = -(-5)
    io.print(string(a))
    io.print(string(b))
    io.print(string(c))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_float_division_by_zero_is_infinity() {
    let dir = TestDir::new("expr_float_div_zero");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.math = math
main()
    const inf: float = 10.0 / 0.0
    io.print(string(inf))
end
"#,
    );
    let exe = exe_path(&dir, "float_div_zero");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("inf") || stdout.contains("Inf"), "{stdout}");
}

#[test]
fn expr_rejects_comparison_chaining() {
    let dir = TestDir::new("expr_chained_cmp");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const a: int = 1
    const b: int = 2
    const c: int = 3
    const ok: bool = a < b < c
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.chained_comparison"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_short_circuit_and_skips_side_effect() {
    let dir = TestDir::new("expr_short_circuit_and");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
check_bool(x: int) -> bool
    return x > 0
end
main()
    const value: bool = false and check_bool(10)
    io.print("ok")
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_rejects_propagate_on_non_result_in_void_function() {
    let dir = TestDir::new("expr_propagate_void");
    dir.write(
        "main.orl",
        r#"module app.main
produce() -> result[int, string]
    return ok(1)
end
main()
    const x: int = try produce()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"type.propagate_return_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_rejects_propagate_err_type_mismatch() {
    let dir = TestDir::new("expr_propagate_err_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main
a() -> result[int, string]
    return ok(1)
end
b() -> result[int, int]
    const x: int = try a()
    return ok(x)
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.propagate_err_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_accepts_try_prefix_for_result_propagation() {
    let dir = TestDir::new("expr_try_result_propagation");
    dir.write(
        "main.orl",
        r#"module app.main
produce() -> result[int, string]
    return ok(1)
end
wrapped() -> result[int, string]
    const x: int = try produce()
    return ok(x + 1)
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_accepts_try_prefix_for_optional_propagation() {
    let dir = TestDir::new("expr_try_optional_propagation");
    dir.write(
        "main.orl",
        r#"module app.main
maybe() -> optional[int]
    return some(1)
end
wrapped() -> optional[int]
    const x: int = try maybe()
    return some(x + 1)
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_rejects_try_prefix_on_non_result_or_optional() {
    let dir = TestDir::new("expr_try_non_result");
    dir.write(
        "main.orl",
        r#"module app.main
wrapped() -> result[int, string]
    const x: int = try 1
    return ok(x)
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"type.propagate_not_result_or_optional"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_accepts_pipe_operator() {
    let dir = TestDir::new("expr_pipe");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.list = lists
import ori.iter = iter
main()
    const items: list[int] = [1, 2, 3]
    const doubled: list[int] = iter.map(items, (x: int) => x * 2)
    io.print(string(lists.len(doubled)))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_inline_if_expression() {
    let dir = TestDir::new("expr_inline_if");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const label: string = if true then "pass" else "fail"
    io.print(label)
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let exe = exe_path(&dir, "inline_if");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "pass\n");
}

#[test]
fn expr_rejects_inline_if_branches_different_types() {
    let dir = TestDir::new("expr_inline_if_type_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const label: string = if true then "pass" else 42
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.if_branch_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_rejects_inline_if_without_else() {
    let dir = TestDir::new("expr_inline_if_missing_else");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const label: string = if true then "pass"
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"parse.missing_else_in_if_expr"),
        "{:?}",
        out.diagnostics
    );
    assert!(
        !codes.contains(&"parse.unexpected_token"),
        "inline if without else should use the dedicated diagnostic: {:?}",
        out.diagnostics
    );
}

#[test]
fn expr_accepts_anonymous_struct_literal() {
    let dir = TestDir::new("expr_anon_struct");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Vec2
    x: float
    y: float
end
main()
    const v: Vec2 = {x: 1.0, y: 2.0}
    io.print(f"{v.x} {v.y}")
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_typed_struct_brace_literal() {
    let dir = TestDir::new("expr_typed_struct_brace");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct User
    name: string
    age: int
end
main()
    const u: User = User { name: "Ada", age: 36 }
    io.print(u.name)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_disambiguates_map_literal_from_struct() {
    let dir = TestDir::new("expr_map_vs_struct");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct User
    name: string
    age: int
end
main()
    const u: User = { name: "Ada", age: 36 }
    const ages: map[string, int] = { "Ada": 36, "Bo": 20 }
    io.print(u.name)
    io.print(string(ages["Ada"]))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_rejects_removed_dot_struct_literal() {
    let dir = TestDir::new("expr_removed_dot_struct");
    dir.write(
        "main.orl",
        r#"module app.main
struct Vec2
    x: float
    y: float
end
main()
    const v: Vec2 = .{x: 1.0, y: 2.0}
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_struct_call_literal"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_rejects_removed_struct_call_literal() {
    let dir = TestDir::new("expr_removed_struct_call");
    dir.write(
        "main.orl",
        r#"module app.main
struct Point
    x: int
    y: int
end
main()
    const p: Point = Point(x: 1, y: 2)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_struct_call_literal"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_rejects_removed_guided_struct_literal() {
    let dir = TestDir::new("expr_removed_guided_struct");
    dir.write(
        "main.orl",
        r#"module app.main
struct Point
    x: int
    y: int
end
main()
    const p: Point = (x: 1, y: 2)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_struct_call_literal"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_accepts_context_typed_struct_on_assign() {
    let dir = TestDir::new("expr_assign_context_struct");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Point
    x: int
    y: int
end
main()
    var p: Point = { x: 1, y: 2 }
    p = { x: 3, y: 4 }
    io.print(string(p.x + p.y))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_accepts_context_typed_enum_on_assign() {
    let dir = TestDir::new("expr_assign_context_enum");
    dir.write(
        "main.orl",
        r#"module app.main
enum Shape
    Circle(radius: float)
    Dot
end
main()
    var s: Shape = .Dot
    s = .Circle(radius: 1.5)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn expr_rejects_duplicate_enum_variant_fields() {
    let dir = TestDir::new("expr_dup_enum_variant_fields");
    dir.write(
        "main.orl",
        r#"module app.main
enum Shape
    Circle(radius: float)
end
main()
    const c: Shape = .Circle(radius: 1.0, radius: 2.0)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"type.anon_struct_field_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_struct_call_poison_does_not_type_as_success() {
    // Removed Type(...) still diagnoses; type is Error so field access on the
    // result should not silently succeed as a real Point.
    let dir = TestDir::new("expr_struct_call_poison");
    dir.write(
        "main.orl",
        r#"module app.main
struct Point
    x: int
    y: int
end
main()
    const p: Point = Point(x: 1, y: 2)
    const z: int = p.x
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_struct_call_literal"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_rejects_anonymous_struct_missing_field() {
    let dir = TestDir::new("expr_anon_struct_missing");
    dir.write(
        "main.orl",
        r#"module app.main
struct Vec2
    x: float
    y: float
end
main()
    const v: Vec2 = {x: 1.0}
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.anon_struct_field_mismatch")
            || diagnostic_codes(&out).contains(&"type.missing_struct_field"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_accepts_struct_update_with() {
    let dir = TestDir::new("expr_struct_update");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Config
    timeout: int
    retries: int
    verbose: bool
end
main()
    const a: Config = Config { timeout: 30, retries: 3, verbose: false }
    const b: Config = a with {
        verbose: true,
    } end
    io.print(string(a.verbose))
    io.print(string(b.verbose))
    io.print(string(b.timeout))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_collection_literals() {
    let dir = TestDir::new("expr_collections");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const items: list[string] = ["a", "b", "c"]
    const scores: map[int, string] = {1: "one", 2: "two"}
    const empty: list[int] = []
    io.print(string(1))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_index_and_slice() {
    let dir = TestDir::new("expr_index_slice");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.list = lists
main()
    const items: list[int] = [10, 20, 30]
    io.print(string(items[0]))
    const sub: list[int] = items[1..3]
    io.print(string(lists.len(sub)))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

// ─── Part 4 — Statements and Control Flow ─────────────────────────────────────

#[test]
fn stmt_rejects_const_reassignment() {
    let dir = TestDir::new("stmt_const_reassign");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const x: int = 0
    x = 1
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"bind.const_reassignment"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_accepts_var_mutation() {
    let dir = TestDir::new("stmt_var_mutate");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var x: int = 0
    x += 5
    io.print(string(x))
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn stmt_rejects_same_scope_shadowing() {
    let dir = TestDir::new("stmt_same_scope_shadow");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    const x: int = 1
    const x: int = 2
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"bind.shadowing"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_accepts_if_some_binding() {
    let dir = TestDir::new("stmt_if_some");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
maybe_user() -> optional[string]
    return some("Ada")
end
main()
    if some(name) = maybe_user()
        io.print(name)
    else
        io.print("no user")
    end
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_accepts_while_some_loop() {
    let dir = TestDir::new("stmt_while_some");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var source: optional[int] = some(3)
    while some(n) = source
        io.print(string(n))
        source = none
    end
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_accepts_loop_with_break_continue() {
    let dir = TestDir::new("stmt_loop_break_continue");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var counter: int = 0
    loop
        counter = counter + 1
        if counter >= 10
            break
        end
    end
    io.print(string(counter))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_rejects_break_outside_loop() {
    let dir = TestDir::new("stmt_break_outside");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    break
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"control.loop_required"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_accepts_for_loop_with_index() {
    let dir = TestDir::new("stmt_for_index");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    for item, index in ["a", "b", "c"]
        io.print(f"{index}: {item}")
    end
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_accepts_repeat() {
    let dir = TestDir::new("stmt_repeat");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var total: int = 0
    repeat 3
        total = total + 1
    end
    io.print(string(total))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_accepts_repeat_with_times_keyword() {
    let dir = TestDir::new("stmt_repeat_times");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    var total: int = 0
    repeat 3 times
        total = total + 1
    end
    io.print(string(total))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_rejects_non_exhaustive_match() {
    let dir = TestDir::new("stmt_non_exhaustive");
    dir.write(
        "main.orl",
        r#"module app.main
enum Color
    Red
    Green
    Blue
end
describe(c: Color) -> string
    match c
        case Red:
            return "red"
        case Green:
            return "green"
    end
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"match.non_exhaustive"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_warns_duplicate_match_case() {
    let dir = TestDir::new("stmt_duplicate_match_case");
    dir.write(
        "main.orl",
        r#"module app.main
describe(value: bool) -> string
    match value
        case true:
            return "yes"
        case true:
            return "still yes"
        case false:
            return "no"
    end
    return "fallback"
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"match.duplicate_case"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_warns_unreachable_match_case() {
    let dir = TestDir::new("stmt_unreachable_match_case");
    dir.write(
        "main.orl",
        r#"module app.main
describe(value: bool) -> string
    match value
        case _:
            return "anything"
        case true:
            return "yes"
    end
    return "fallback"
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"match.unreachable_case"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_accepts_match_with_case_else() {
    let dir = TestDir::new("stmt_match_else");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
enum Color
    Red
    Green
    Blue
end
describe(c: Color) -> string
    match c
        case Red:
            return "red"
        case Green:
            return "green"
        case else:
            return "other"
    end
end
main()
    io.print(describe(Color.Blue()))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_accepts_check_assertion() {
    let dir = TestDir::new("stmt_check");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    check true, "should pass"
    io.print("ok")
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_check_failure_causes_panic() {
    let dir = TestDir::new("stmt_check_fail");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    io.print("start")
    check false, "intentional failure"
    io.print("end")
end
"#,
    );
    let exe = exe_path(&dir, "check_fail");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe));
    match out {
        Ok(compile) => {
            assert!(!compile.has_errors, "{:?}", compile.diagnostics);
            let output = Command::new(&exe).output().unwrap();
            assert!(!output.status.success(), "{output:?}");
        }
        Err(_msg) => {}
    }
}

// ─── Part 5 — Functions and Closures ──────────────────────────────────────────

#[test]
fn func_accepts_named_parameters_with_defaults() {
    let dir = TestDir::new("func_named_defaults");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
connect(host: string, port: int = 80)
    io.print(f"{host}:{port}")
end
main()
    connect("localhost")
    connect(host: "example.com", port: 443)
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn func_rejects_positional_arg_after_named_arg() {
    let dir = TestDir::new("func_pos_after_named");
    dir.write(
        "main.orl",
        r#"module app.main
connect(host: string, port: int = 80)
end
main()
    connect(host: "x", 443)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.positional_after_named_arg"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn func_rejects_const_receiver_on_mut_func() {
    let dir = TestDir::new("func_const_mut_call");
    dir.write(
        "main.orl",
        r#"module app.main
struct Counter
    value: int
    mut increment()
        self.value = self.value + 1
    end
end
main()
    const c: Counter = Counter { value: 0 }
    c.increment()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"mut.const_method_call"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn func_rejects_self_field_mutation_in_non_mut_method() {
    let dir = TestDir::new("func_self_field_mutation_non_mut");
    dir.write(
        "main.orl",
        r#"module app.main
struct Counter
    value: int
    increment()
        self.value = self.value + 1
    end
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"mut.field_mutation_in_func"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn func_rejects_closure_capturing_var() {
    let dir = TestDir::new("func_closure_capture_var");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.iter = iter
main()
    var total: int = 0
    const mapped: list[int] = iter.map([1, 2], (x: int) => x + total)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"mut.closure_captures_var"),
        "{:?}",
        out.diagnostics
    );
}

// ─── S3 rhythm: => bodies, poetic call, labeled end, (params)=> closures ─────

#[test]
fn func_accepts_fat_arrow_expression_body() {
    let dir = TestDir::new("func_fat_arrow_body");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
double(x: int) -> int => x * 2
main()
    io.print(string(double(21)))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_paren_arrow_closure() {
    let dir = TestDir::new("expr_paren_arrow_closure");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.list = lists
import ori.iter = iter
main()
    const items: list[int] = [1, 2, 3]
    const doubled: list[int] = iter.map(items, (x: int) => x * 2)
    io.print(string(lists.len(doubled)))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_long_paren_closure() {
    let dir = TestDir::new("expr_long_paren_closure");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.list = lists
import ori.iter = iter
main()
    const items: list[int] = [1, 2]
    const doubled: list[int] = iter.map(items, (x: int) -> int
        return x * 2
    end)
    io.print(string(lists.len(doubled)))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_rejects_do_closure_keyword() {
    let dir = TestDir::new("expr_do_removed");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.iter = iter
main()
    const mapped: list[int] = iter.map([1], do(x: int) => x)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.do_removed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn expr_accepts_poetic_call_and_parenthesized_arg_call() {
    let dir = TestDir::new("expr_poetic_call_ok");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
greet(name: string) -> string => name
main()
    const who: string = "Ori"
    io.print who
    io.print greet("hello")
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_rejects_nested_poetic_call() {
    let dir = TestDir::new("expr_poetic_call_nested");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
greet(name: string) -> string => name
main()
    io.print greet "hello"
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.poetic_call_nested"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn stmt_accepts_labeled_end() {
    let dir = TestDir::new("stmt_labeled_end");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    if true
        io.print("ok")
    end if
    match 1
        case 1:
            io.print("one")
        case else:
            io.print("other")
    end match
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn stmt_rejects_end_label_mismatch() {
    let dir = TestDir::new("stmt_end_label_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    if true
        return
    end match
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.end_label_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn trait_default_accepts_labeled_end_function() {
    let dir = TestDir::new("trait_default_end_function");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Greetable
    greet() -> string
        return "hi"
    end function
end
struct Person
    name: string
end
apply Person
    use Greetable
        greet() -> string
            return self.name
        end
    end
end
main()
    const p: Person = Person { name: "Ori" }
    const g: any[Greetable] = p
    io.print(g.greet())
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn trait_default_accepts_fat_arrow_body() {
    let dir = TestDir::new("trait_default_fat_arrow");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Doubler
    double(x: int) -> int => x * 2
end
struct S
end
apply S
    use Doubler
        double(x: int) -> int => x * 2
    end
end
main()
    const s: S = S {}
    const d: any[Doubler] = s
    io.print(string(d.double(21)))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn expr_accepts_struct_update_labeled_end_struct() {
    let dir = TestDir::new("struct_update_end_struct");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Point
    x: int
    y: int
end
main()
    const p: Point = Point { x: 1, y: 2 }
    const q: Point = p with { x: 10 } end struct
    io.print(string(q.x))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn func_rejects_await_outside_async() {
    let dir = TestDir::new("func_await_outside");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.task = task
main()
    const v: int = await task.sleep(1)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"async.await_outside_async"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn func_accepts_async_func_and_await() {
    let dir = TestDir::new("func_async");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.task = task
async compute() -> int
    await task.sleep(1)
    return 42
end
async main()
    const n: int = await compute()
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn func_allows_using_inside_async_func() {
    let dir = TestDir::new("func_async_using");
    dir.write(
        "main.orl",
        r#"module app.main
trait Disposable
    mut dispose(self)
end
struct Res
    id: int
end
apply Res
    use Disposable
        mut dispose(self)
        end
    end
end
async load() -> int
    using res: Res = Res { id: 1 }
    return 42
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn func_compile_runs_async_main_native() {
    let dir = TestDir::new("func_async_main_native");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.task = task
async answer() -> int
    await task.sleep(1)
    return 42
end
async main()
    const n: int = await answer()
    io.print(string(n))
end
"#,
    );
    let exe = exe_path(&dir, "async_main");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "42");
}

// ─── Part 6 — Traits and Implement ────────────────────────────────────────────

#[test]
fn trait_accepts_required_and_default_methods() {
    let dir = TestDir::new("trait_default");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Greetable
    name(self) -> string
    greet(self) -> string
        return f"Hello, {self.name()}!"
    end
end
struct User
    n: string
end
apply User
    use Greetable
        name(self) -> string
            return self.n
        end
    end
end
main()
    const u: User = User { n: "Ada" }
    io.print(u.greet())
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn trait_rejects_apply_missing_required_method() {
    let dir = TestDir::new("trait_missing_method");
    dir.write(
        "main.orl",
        r#"module app.main
trait Greetable
    name(self) -> string
    greet(self) -> string
        return f"Hello, {self.name()}!"
    end
end
struct User
    n: string
end
apply User
    use Greetable
    end
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"impl.missing_method"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn trait_accepts_any_dynamic_dispatch() {
    let dir = TestDir::new("trait_any_dispatch");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Drawable
    draw(self) -> string
end
struct Circle
    radius: float
end
struct Rect
    w: float
    h: float
end
apply Circle
    use Drawable
        draw(self) -> string
            return "circle"
        end
    end
end
apply Rect
    use Drawable
        draw(self) -> string
            return "rect"
        end
    end
end
main()
    const c: any[Drawable] = Circle { radius: 1.0 }
    const r: any[Drawable] = Rect { w: 2.0, h: 3.0 }
    io.print(c.draw())
    io.print(r.draw())
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn trait_object_equality_works() {
    let dir = TestDir::new("trait_any_equality");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Drawable
    draw(self) -> string
end
struct Circle
    radius: float
end
apply Circle
    use Drawable
        draw(self) -> string
            return "circle"
        end
    end
end
main()
    const a: any[Drawable] = Circle { radius: 1.0 }
    const b: any[Drawable] = Circle { radius: 1.0 }
    const c: any[Drawable] = Circle { radius: 2.0 }
    io.println(string(a == b))
    io.println(string(a != c))
end
"#,
    );
    let exe = exe_path(&dir, "trait_any_equality");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["true", "true"]);
}

#[test]
fn trait_rejects_ambiguous_method_call() {
    let dir = TestDir::new("trait_ambiguous");
    dir.write(
        "main.orl",
        r#"module app.main
trait Alpha
    output(self) -> string
end
trait Beta
    output(self) -> string
end
struct S end
apply S
    use Alpha
        output(self) -> string
            return "alpha"
        end
    end
end
apply S
    use Beta
        output(self) -> string
            return "beta"
        end
    end
end
main()
    const s: S = S {}
    const msg: string = s.output()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.ambiguous_method"),
        "{:?}",
        out.diagnostics
    );
}

// ─── Part 7 — Errors and Propagation ──────────────────────────────────────────

#[test]
fn error_accepts_result_with_propagation_chain() {
    let dir = TestDir::new("error_propagation");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
step1() -> result[int, string]
    return ok(1)
end
step2(x: int) -> result[int, string]
    return ok(x + 1)
end
pipeline() -> result[int, string]
    const a: int = try step1()
    const b: int = try step2(a)
    return ok(b)
end
main()
    match pipeline()
        case ok(v):
            io.print(string(v))
        case err(e):
            io.print(e)
    end
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn error_compile_runs_integer_division_by_zero_causes_panic() {
    let dir = TestDir::new("error_div_zero_panic");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    const x: int = 10 / 0
    io.print(string(x))
end
"#,
    );
    let exe = exe_path(&dir, "div_zero");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(
        !output.status.success(),
        "integer division by zero should panic, got {output:?}"
    );
}

#[test]
fn error_compile_runs_index_out_of_bounds_causes_panic() {
    let dir = TestDir::new("error_oob_panic");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
import ori.list = lists
main()
    const items: list[int] = [1, 2, 3]
    const x: int = items[99]
    io.print(string(x))
end
"#,
    );
    let exe = exe_path(&dir, "oob");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(
        !output.status.success(),
        "index out of bounds should panic, got {output:?}"
    );
}

#[test]
fn error_panic_explicit_causes_runtime_panic() {
    let dir = TestDir::new("error_panic");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    panic("test panic")
    io.print("unreachable")
end
"#,
    );
    let exe = exe_path(&dir, "panic_test");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("test panic"), "{stderr}");
}

#[test]
fn error_todo_causes_runtime_panic() {
    let dir = TestDir::new("error_todo");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    io.print("before")
    todo()
    io.print("after")
end
"#,
    );
    let exe = exe_path(&dir, "todo_test");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe));
    match out {
        Ok(compile) => {
            assert!(!compile.has_errors, "{:?}", compile.diagnostics);
            let output = Command::new(&exe).output().unwrap();
            assert!(!output.status.success(), "{:?}", output);
        }
        Err(msg) => {
            assert!(
                msg.contains("unresolved error type") || msg.contains("not implemented"),
                "unexpected error: {msg}"
            );
        }
    }
}

#[test]
fn error_unreachable_causes_runtime_panic() {
    let dir = TestDir::new("error_unreachable");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    io.print("before")
    unreachable()
    io.print("after")
end
"#,
    );
    let exe = exe_path(&dir, "unreachable_test");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe));
    match out {
        Ok(compile) => {
            assert!(!compile.has_errors, "{:?}", compile.diagnostics);
            let output = Command::new(&exe).output().unwrap();
            assert!(!output.status.success(), "{:?}", output);
        }
        Err(msg) => {
            assert!(
                msg.contains("unresolved error type") || msg.contains("not implemented"),
                "unexpected error: {msg}"
            );
        }
    }
}

// ─── Part 8 — Memory and Cleanup ──────────────────────────────────────────────

#[test]
fn mem_value_semantics_isolate_copies() {
    let dir = TestDir::new("mem_value_semantics");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Point
    x: int
    y: int
end
main()
    const a: Point = Point { x: 1, y: 2 }
    var b: Point = Point { x: a.x, y: a.y }
    b.x = 99
    check a.x == 1, "value semantics: a is unaffected"
    io.print("ok")
end
"#,
    );
    let exe = exe_path(&dir, "value_sem");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(
        output.status.success(),
        "value semantics check should pass, got {output:?}"
    );
}

#[test]
fn mem_using_calls_dispose_on_normal_return() {
    let dir = TestDir::new("mem_using_normal");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Disposable
    mut dispose(self)
end
struct Res
    name: string
end
apply Res
    use Disposable
        mut dispose(self)
        end
    end
end
main()
    using r: Res = Res { name: "test" }
    io.print(r.name)
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn mem_compile_multiple_using_lifo_order() {
    let dir = TestDir::new("mem_using_lifo");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Disposable
    mut dispose(self)
end
struct Logger
    label: string
end
apply Logger
    use Disposable
        mut dispose(self)
            io.print(f"disposed {self.label}")
        end
    end
end
main()
    using a: Logger = Logger { label: "A" }
    using b: Logger = Logger { label: "B" }
    using c: Logger = Logger { label: "C" }
    io.print("body")
end
"#,
    );
    let exe = exe_path(&dir, "using_lifo");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let body_pos = stdout.find("body").unwrap_or(usize::MAX);
    let c_pos = stdout.find("disposed C").unwrap_or(usize::MAX);
    let b_pos = stdout.find("disposed B").unwrap_or(usize::MAX);
    let a_pos = stdout.find("disposed A").unwrap_or(usize::MAX);
    assert!(body_pos < c_pos, "body should run before dispose of C");
    assert!(c_pos < b_pos, "C should be disposed before B");
    assert!(b_pos < a_pos, "B should be disposed before A");
}

#[test]
fn mem_rejects_using_on_non_disposable_type() {
    let dir = TestDir::new("mem_using_non_disposable");
    dir.write(
        "main.orl",
        r#"module app.main
main()
    using x: int = 5
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"using.not_disposable"),
        "{:?}",
        out.diagnostics
    );
}

// ─── Part 9 — Generics ────────────────────────────────────────────────────────

#[test]
fn generic_accepts_type_inference() {
    let dir = TestDir::new("generic_inference");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
wrap[T](value: T) -> optional[T]
    return some(value)
end
main()
    const a: int = 42
    const b: optional[int] = wrap(a)
    const c: optional[string] = wrap("hello")
    io.print("ok")
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn generic_accepts_where_constraint() {
    let dir = TestDir::new("generic_where");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Labelled
    label(self) -> string
end
struct User
    name: string
end
apply User
    use Labelled
        label(self) -> string
            return self.name
        end
    end
end
show for T: Labelled (value: T) -> string
    return value.label()
end
main()
    const u: User = User { name: "Ada" }
    io.print(show(u))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn generic_rejects_constraint_not_satisfied() {
    let dir = TestDir::new("generic_constraint_fail");
    dir.write(
        "main.orl",
        r#"module app.main
trait Comparable
    compare(self, other: Self) -> int
end
max for T: Comparable (a: T, b: T) -> T
    return a
end
struct Point
    x: int
    y: int
end
main()
    const p: Point = max(Point { x: 1, y: 2 }, Point { x: 3, y: 4 })
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"generic.constraint_not_satisfied"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn generic_accepts_negative_constraint() {
    let dir = TestDir::new("generic_negative");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
trait Disposable
    mut dispose(self)
end
raw_copy for T: not Disposable (src: T) -> T
    return src
end
main()
    const x: int = raw_copy(42)
    io.print(string(x))
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn generic_rejects_negative_constraint_violated() {
    let dir = TestDir::new("generic_neg_violation");
    dir.write(
        "main.orl",
        r#"module app.main
trait Disposable
    mut dispose(self)
end
struct Res end
apply Res
    use Disposable
        mut dispose(self)
        end
    end
end
raw_copy for T: not Disposable (src: T) -> T
    return src
end
main()
    const r: Res = raw_copy(Res {})
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"generic.negative_constraint_violated"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn generic_accepts_generic_struct() {
    let dir = TestDir::new("generic_struct");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
struct Pair[A, B]
    first: A
    second: B
end
main()
    io.print("generic struct defined")
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn generic_generic_struct_types_are_distinct() {
    let dir = TestDir::new("generic_struct_distinct");
    dir.write(
        "main.orl",
        r#"module app.main
struct Pair[A, B]
    first: A
    second: B
end
takes_int_string(p: Pair[int, string])
end
main()
    const p: Pair[string, int] = Pair { first: "one", second: 1 }
    takes_int_string(p)
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"type.arg_type_mismatch")
            || diagnostic_codes(&out).contains(&"type.type_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn generic_accepts_hkt() {
    let dir = TestDir::new("generic_hkt");
    dir.write(
        "main.orl",
        r#"module app.main
trait Functor[F[_]]
    fmap[A, B](fa: F[A], f: func(A) -> B) -> F[B]
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn generic_accepts_associated_type_in_trait() {
    let dir = TestDir::new("generic_assoc_type");
    dir.write(
        "main.orl",
        r#"module app.main
trait Container
    type Item
    get(self) -> Item
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn generic_accepts_const_generic_param() {
    let dir = TestDir::new("generic_const_generic");
    dir.write(
        "main.orl",
        r#"module app.main
struct Matrix[const N: int]
    value: int
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

// ─── Part 10 — Cross-Cutting Scenarios ────────────────────────────────────────

#[test]
fn crosscut_accepts_full_pipeline_program() {
    let dir = TestDir::new("crosscut_full_pipeline");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io

trait Disposable
    mut dispose(self)
end

trait Loggable
    to_log(self) -> string
end

struct User
    name: string
    age: int
end

struct Session
    user: User
end

apply Session
    use Disposable
        mut dispose(self)
            io.print(f"session of {self.user.name} disposed")
        end
    end
end

apply User
    use Loggable
        to_log(self) -> string
            return f"User({self.name}, {self.age})"
        end
    end
end

validate_age(age: int) -> result[int, string]
    if age < 0
        return err("age below zero")
    end
    return ok(age)
end

main()
    using session: Session = Session { user: User { name: "Ada", age: 30 } }

    match validate_age(30)
        case ok(age):
            const log: string = session.user.to_log()
            io.print(log)
            io.print(string(age))
        case err(msg):
            io.print(msg)
    end

    var sum: int = 0
    for item, index in [10, 20, 30]
        sum = sum + item
    end
    io.print(string(sum))

    check true, "should never fail"
end
"#,
    );
    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let exe = exe_path(&dir, "full_pipeline");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe));
    match out {
        Ok(compile) => {
            assert!(!compile.has_errors, "{:?}", compile.diagnostics);
            let output = Command::new(&exe).output().unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        Err(_msg) => {}
    }
}

#[test]
fn crosscut_rejects_private_access_from_other_namespace() {
    let dir = TestDir::new("crosscut_private");
    dir.write(
        "util.orl",
        r#"module app.util
hidden() -> int
    return 42
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main
import app.util = util
main()
    const x: int = util.hidden()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"name.private"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn crosscut_rejects_import_cycle() {
    let dir = TestDir::new("crosscut_import_cycle");
    dir.write(
        "a.orl",
        r#"module app.a
import app.b
main()
end
"#,
    );
    dir.write(
        "b.orl",
        r#"module app.b
import app.a
main()
end
"#,
    );
    let out = run_check(&dir.path("a.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"project.circular_import"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn crosscut_rejects_namespace_mismatch() {
    let dir = TestDir::new("crosscut_namespace_mismatch");
    dir.write(
        "app/bar.orl",
        r#"module app.foo
public answer() -> int
    return 42
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main
import app.bar
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"project.namespace_file_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn crosscut_accepts_public_import_reexport() {
    let dir = TestDir::new("crosscut_public_import");
    dir.write(
        "util.orl",
        r#"module app.util
public answer() -> int
    return 42
end
"#,
    );
    dir.write(
        "facade.orl",
        r#"module app.facade
public import app.util = util
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main
import app.facade = api
main()
    const x: int = api.util.answer()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn crosscut_variadic_rejects_not_last_parameter() {
    let dir = TestDir::new("crosscut_variadic_not_last");
    dir.write(
        "main.orl",
        r#"module app.main
trait Displayable
    display(self) -> string
end
apply string
    use Displayable
        display(self) -> string
            return self
        end
    end
end
log(values: any[Displayable]..., prefix: string)
end
main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.variadic_not_last"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn crosscut_rejects_unknown_extern_abi() {
    let dir = TestDir::new("crosscut_unknown_extern_abi");
    dir.write(
        "main.orl",
        r#"module app.main

extern wasm
    host_value() -> int
end

main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(codes.contains(&"extern.unknown_abi"), "{codes:?}");
}

#[test]
fn crosscut_rejects_managed_extern_ffi_types() {
    let dir = TestDir::new("crosscut_managed_extern_ffi_types");
    dir.write(
        "main.orl",
        r#"module app.main

extern c
    read_name(input: string) -> string
    var last_name: string
end

main()
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(codes.contains(&"extern.managed_type_in_ffi"), "{codes:?}");
}

#[test]
fn crosscut_fmt_preserves_valid_source_unchanged() {
    let dir = TestDir::new("crosscut_fmt_idempotent");
    let source = r#"module app.main

import ori.io = io

main()
    io.print("hello")
end
"#;
    dir.write("main.orl", source);

    let out = run_fmt(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.formatted.contains("io.print(\"hello\")"),
        "{}",
        out.formatted
    );
}

#[test]
fn tooling_new_project_creates_checkable_app_skeleton() {
    let dir = TestDir::new("tooling_new_project");
    let root = dir.path("demo");
    let out = run_new_project(
        &root,
        NewProjectOptions {
            name: Some("demo".to_string()),
            kind: NewProjectKind::App,
            is_init: false,
        },
    )
    .unwrap();

    assert!(out.manifest.is_file());
    assert!(out.entry.is_file());
    assert_eq!(
        out.manifest.file_name().and_then(|n| n.to_str()),
        Some("ori.proj")
    );
    assert!(root.join("main.orl").is_file());
    assert!(root.join("docs").is_dir());
    // No forced src/app/lib/bin layout (M2.layout).
    assert!(!root.join("src").exists());

    let check = run_check(&out.manifest).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);
}

#[test]
fn tooling_new_project_refuses_non_empty_directory() {
    let dir = TestDir::new("tooling_new_project_non_empty");
    dir.write("demo/existing.txt", "keep me");

    let err = run_new_project(
        &dir.path("demo"),
        NewProjectOptions {
            name: None,
            kind: NewProjectKind::App,
            is_init: false,
        },
    )
    .expect_err("non-empty directory must be rejected");

    assert!(err.contains("project.new_exists"), "{err}");
}

#[test]
fn package_install_caches_local_package_and_path_dependency() {
    let dir = TestDir::new("package_install_path_dependency");
    dir.write(
        "local_math/ori.pkg.toml",
        r#"[package]
name = "demo.math"
version = "0.1.0"
entry = "src/lib.orl"
ori_version = "0.2.0"
description = "Local math helpers"
"#,
    );
    dir.write(
        "local_math/src/lib.orl",
        r#"module demo.math

public one() -> int
    return 1
end
"#,
    );
    dir.write(
        "app/ori.pkg.toml",
        r#"[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.2.0"

[dependencies]
demo.math = { path = "../local_math", version = "0.1.0" }
"#,
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.math (one)
import ori.io = io

main()
    io.print(string(one()))
end
"#,
    );

    let cache = dir.path("cache");
    let out = run_install_package(InstallPackageOptions {
        name: "demo.app".to_string(),
        source: Some(dir.path("app")),
        cache_root: Some(cache.clone()),
    })
    .unwrap();

    assert_eq!(out.packages.len(), 2);
    assert!(cache.join("demo.math/0.1.0/ori.pkg.toml").is_file());
    assert!(cache.join("demo.app/0.1.0/src/main.orl").is_file());
}

#[test]
fn package_path_dependency_resolves_during_check_from_package_manifest() {
    let dir = TestDir::new("package_path_dependency_check");
    dir.write(
        "local_math/ori.pkg.toml",
        r#"[package]
name = "demo.math"
version = "0.1.0"
entry = "src/lib.orl"
ori_version = "0.2.0"
"#,
    );
    dir.write(
        "local_math/src/lib.orl",
        r#"module demo.math

public one() -> int
    return 1
end
"#,
    );
    dir.write(
        "app/ori.pkg.toml",
        r#"[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.2.0"

[dependencies]
demo.math = { path = "../local_math", version = "0.1.0" }
"#,
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.math (one)

main()
    const value: int = one()
end
"#,
    );

    let out = run_check(&dir.path("app/ori.pkg.toml")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn project_path_dependency_resolves_during_check_from_ori_proj() {
    let dir = TestDir::new("project_path_dependency_check");
    dir.write(
        "math/ori.proj",
        r#"manifest = 1
name = "demo.math"
version = "0.1.0"
kind = "lib"
entry = "src/lib.orl"

[source]
root = "src"
root_namespace = "demo.math"
"#,
    );
    dir.write(
        "math/src/lib.orl",
        r#"module demo.math

public two() -> int
    return 2
end
"#,
    );
    dir.write(
        "app/ori.proj",
        r#"manifest = 1
name = "demo.app"
version = "0.1.0"
kind = "app"
entry = "src/main.orl"

[source]
root = "src"
root_namespace = "demo.app"

[dependencies]
demo.math = { path = "../math", version = "0.1.0" }
"#,
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.math (two)

main()
    const value: int = two()
end
"#,
    );

    let out = run_check(&dir.path("app/ori.proj")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn package_install_rejects_remote_dependency_until_registry_exists() {
    let dir = TestDir::new("package_install_remote_dependency");
    dir.write(
        "app/ori.pkg.toml",
        r#"[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.2.0"

[dependencies]
other.lib = "1.0.0"
"#,
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

main()
end
"#,
    );

    let err = run_install_package(InstallPackageOptions {
        name: "demo.app".to_string(),
        source: Some(dir.path("app")),
        cache_root: Some(dir.path("cache")),
    })
    .expect_err("version dependency without cache/registry should fail");

    assert!(
        err.contains("package.cache_miss")
            || err.contains("package.registry_unconfigured")
            || err.contains("package.registry_miss")
            || err.contains("package.registry_unavailable"),
        "{err}"
    );
}

/// PKG-3: publish to a file registry, install by name@version, resolve imports on check.
#[test]
fn package_registry_publish_install_and_resolve_on_check() {
    let dir = TestDir::new("package_registry_publish_install");
    dir.write(
        "math/ori.pkg.toml",
        r#"[package]
name = "demo.math"
version = "0.4.0"
entry = "src/lib.orl"
ori_version = "0.3.0"
description = "registry math"
"#,
    );
    dir.write(
        "math/src/lib.orl",
        r#"module demo.math

public six() -> int
    return 6
end
"#,
    );

    let registry = dir.path("registry");
    let cache = dir.path("cache");
    std::fs::create_dir_all(&registry).unwrap();

    let published = run_publish_package(PublishPackageOptions {
        path: dir.path("math"),
        registry: Some(registry.display().to_string()),
        token: None,
        force: false,
    })
    .expect("publish to file registry");
    assert_eq!(published.name, "demo.math");
    assert_eq!(published.version, "0.4.0");
    assert!(registry
        .join("packages/demo.math/0.4.0/ori.pkg.toml")
        .is_file());
    assert!(registry.join("packages/demo.math/versions.json").is_file());
    assert!(registry.join("index.json").is_file());

    std::env::set_var("ORI_REGISTRY", &registry);
    std::env::set_var("ORI_PACKAGE_CACHE", &cache);

    let installed = run_install_package(InstallPackageOptions {
        name: "demo.math@0.4.0".to_string(),
        source: None,
        cache_root: Some(cache.clone()),
    })
    .expect("install from registry");
    assert!(
        installed
            .packages
            .iter()
            .any(|p| p.name == "demo.math" && p.version == "0.4.0"),
        "{:?}",
        installed.packages
    );
    assert!(cache.join("demo.math/0.4.0/src/lib.orl").is_file());

    // Fresh consumer: version-only dependency, resolved via registry → cache.
    let cache2 = dir.path("cache2");
    std::env::set_var("ORI_PACKAGE_CACHE", &cache2);
    dir.write(
        "app/ori.pkg.toml",
        r#"[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.3.0"

[dependencies]
demo.math = "0.4.0"
"#,
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.math (six)

main()
    const value: int = six()
end
"#,
    );

    let out = run_check(&dir.path("app/ori.pkg.toml")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(cache2.join("demo.math/0.4.0/ori.pkg.toml").is_file());

    // install by name alone uses latest from versions.json
    let cache3 = dir.path("cache3");
    let latest = run_install_package(InstallPackageOptions {
        name: "demo.math".to_string(),
        source: None,
        cache_root: Some(cache3.clone()),
    })
    .expect("install latest from registry");
    assert!(
        latest.packages.iter().any(|p| p.version == "0.4.0"),
        "{:?}",
        latest.packages
    );

    std::env::remove_var("ORI_REGISTRY");
    std::env::remove_var("ORI_PACKAGE_CACHE");
}

#[test]
fn package_manifest_rejects_git_and_path_together() {
    let dir = TestDir::new("package_manifest_git_path_conflict");
    dir.write(
        "pkg/ori.pkg.toml",
        r#"[package]
name = "demo.bad"
version = "0.1.0"
entry = "src/lib.orl"
ori_version = "0.3.0"

[dependencies]
other = { git = "https://example.com/x.git", path = "../x" }
"#,
    );
    dir.write(
        "pkg/src/lib.orl",
        r#"module demo.bad
public z() -> int
    return 0
end
"#,
    );
    let err = run_install_package(InstallPackageOptions {
        name: "demo.bad".to_string(),
        source: Some(dir.path("pkg")),
        cache_root: Some(dir.path("cache")),
    })
    .expect_err("git+path must fail");
    assert!(err.contains("git") && err.contains("path"), "{err}");
}

#[test]
fn package_manifest_rejects_invalid_version() {
    let dir = TestDir::new("package_manifest_bad_version");
    dir.write(
        "pkg/ori.pkg.toml",
        r#"[package]
name = "demo.bad"
version = "1.0"
entry = "src/lib.orl"
ori_version = "0.3.0"
"#,
    );
    dir.write(
        "pkg/src/lib.orl",
        r#"module demo.bad
public z() -> int
    return 0
end
"#,
    );
    let err = ori_driver::package::load_package_manifest(dir.path("pkg")).expect_err("bad version");
    assert!(err.contains("version"), "{err}");
}

#[test]
fn package_publish_refuses_overwrite_without_force() {
    let dir = TestDir::new("package_publish_no_overwrite");
    dir.write(
        "pkg/ori.pkg.toml",
        r#"[package]
name = "demo.once"
version = "1.0.0"
entry = "src/lib.orl"
ori_version = "0.3.0"
"#,
    );
    dir.write(
        "pkg/src/lib.orl",
        r#"module demo.once

public id() -> int
    return 1
end
"#,
    );
    let registry = dir.path("registry");
    std::fs::create_dir_all(&registry).unwrap();
    run_publish_package(PublishPackageOptions {
        path: dir.path("pkg"),
        registry: Some(registry.display().to_string()),
        token: None,
        force: false,
    })
    .unwrap();
    let err = run_publish_package(PublishPackageOptions {
        path: dir.path("pkg"),
        registry: Some(registry.display().to_string()),
        token: None,
        force: false,
    })
    .expect_err("second publish without --force");
    assert!(err.contains("package.publish_exists"), "{err}");
    run_publish_package(PublishPackageOptions {
        path: dir.path("pkg"),
        registry: Some(registry.display().to_string()),
        token: None,
        force: true,
    })
    .expect("force publish");
}

fn init_git_package_repo(root: &std::path::Path) {
    let status = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .arg(root)
        .status()
        .expect("git init");
    assert!(status.success(), "git init failed");
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "user.email", "ori-test@example.com"])
        .status();
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "user.name", "ori-test"])
        .status();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["add", "."])
        .status()
        .expect("git add");
    assert!(status.success(), "git add failed");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["commit", "-m", "initial"])
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit failed");
}

/// PKG-1/PKG-2: git dependency is fetched into the cache and imports resolve on check.
#[test]
fn package_git_dependency_fetches_and_resolves_during_check() {
    let dir = TestDir::new("package_git_dependency_check");
    dir.write(
        "remote_math/ori.pkg.toml",
        r#"[package]
name = "demo.math"
version = "0.2.0"
entry = "src/lib.orl"
ori_version = "0.3.0"
"#,
    );
    dir.write(
        "remote_math/src/lib.orl",
        r#"module demo.math

public three() -> int
    return 3
end
"#,
    );
    init_git_package_repo(&dir.path("remote_math"));

    let git_url = dir.path("remote_math").display().to_string();
    dir.write(
        "app/ori.pkg.toml",
        &format!(
            r#"[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.3.0"

[dependencies]
demo.math = {{ git = "{git_url}", branch = "main", version = "0.2.0" }}
"#
        ),
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.math (three)

main()
    const value: int = three()
end
"#,
    );

    let cache = dir.path("cache");
    std::env::set_var("ORI_PACKAGE_CACHE", &cache);

    let get_out = run_get_dependencies(GetDependenciesOptions {
        path: dir.path("app"),
        cache_root: Some(cache.clone()),
    })
    .expect("ori get should fetch git dependency");
    assert!(
        get_out
            .packages
            .iter()
            .any(|p| p.name == "demo.math" && p.version == "0.2.0"),
        "{:?}",
        get_out.packages
    );
    assert!(cache.join("demo.math/0.2.0/ori.pkg.toml").is_file());

    let out = run_check(&dir.path("app/ori.pkg.toml")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    std::env::remove_var("ORI_PACKAGE_CACHE");
}

#[test]
fn project_git_dependency_resolves_during_check_from_ori_proj() {
    let dir = TestDir::new("project_git_dependency_check");
    dir.write(
        "remote_lib/ori.pkg.toml",
        r#"[package]
name = "demo.util"
version = "1.0.0"
entry = "src/lib.orl"
ori_version = "0.3.0"
"#,
    );
    dir.write(
        "remote_lib/src/lib.orl",
        r#"module demo.util

public four() -> int
    return 4
end
"#,
    );
    init_git_package_repo(&dir.path("remote_lib"));

    let git_url = dir.path("remote_lib").display().to_string();
    dir.write(
        "app/ori.proj",
        &format!(
            r#"manifest = 1
name = "demo.app"
version = "0.1.0"
kind = "app"
entry = "src/main.orl"

[source]
root = "src"
root_namespace = "demo.app"

[dependencies]
demo.util = {{ git = "{git_url}", branch = "main" }}
"#
        ),
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.util (four)

main()
    const value: int = four()
end
"#,
    );

    let cache = dir.path("cache");
    std::env::set_var("ORI_PACKAGE_CACHE", &cache);

    let out = run_check(&dir.path("app/ori.proj")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(cache.join("demo.util/1.0.0/ori.pkg.toml").is_file());

    std::env::remove_var("ORI_PACKAGE_CACHE");
}

#[test]
fn package_version_dependency_resolves_from_cache_after_install() {
    let dir = TestDir::new("package_version_from_cache");
    dir.write(
        "math/ori.pkg.toml",
        r#"[package]
name = "demo.math"
version = "0.3.0"
entry = "src/lib.orl"
ori_version = "0.3.0"
"#,
    );
    dir.write(
        "math/src/lib.orl",
        r#"module demo.math

public five() -> int
    return 5
end
"#,
    );

    let cache = dir.path("cache");
    run_install_package(InstallPackageOptions {
        name: "demo.math".to_string(),
        source: Some(dir.path("math")),
        cache_root: Some(cache.clone()),
    })
    .unwrap();

    dir.write(
        "app/ori.pkg.toml",
        r#"[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.3.0"

[dependencies]
demo.math = "0.3.0"
"#,
    );
    dir.write(
        "app/src/main.orl",
        r#"module demo.app

import demo.math (five)

main()
    const value: int = five()
end
"#,
    );

    std::env::set_var("ORI_PACKAGE_CACHE", &cache);
    let out = run_check(&dir.path("app/ori.pkg.toml")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    std::env::remove_var("ORI_PACKAGE_CACHE");
}

#[test]
fn stdlib_real_project_helpers_typecheck() {
    let dir = TestDir::new("stdlib_real_project_helpers");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.args = args
import ori.config = config
import ori.log = log
import ori.time (Instant, Duration, add, between, duration_seconds, duration_to_millis, instant_from_unix_ms)

main()
    const start: Instant = instant_from_unix_ms(1000)
    const duration: Duration = duration_seconds(2)
    const finish: Instant = add(start, duration)
    const elapsed: Duration = between(start, finish)
    const elapsed_ms: int = duration_to_millis(elapsed)
    const program: string = args.program_name_or("ori")
    const text: string = config.read_text_or("missing.json", "{}")
    log.info(ori.string.concat(program, text))
    log.debug(string(elapsed_ms))
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn crosscut_build_generates_c_source_with_entry_point() {
    let dir = TestDir::new("crosscut_build_c");
    dir.write(
        "main.orl",
        r#"module app.main
import ori.io = io
main()
    io.print("c build")
end
"#,
    );
    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build.c_source.contains("int main(int argc, char** argv)"));
    assert!(build.c_source.contains("ori_io_print"));
}

// ─── Regression: duplicate struct fields and enum variants ────────────────────

#[test]
fn check_rejects_duplicate_struct_fields() {
    let dir = TestDir::new("dup_struct_fields");
    dir.write(
        "main.orl",
        "module app.test\nstruct S\n    x: int\n    x: int\nend\nmain()\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"bind.duplicate_field"),
        "expected bind.duplicate_field, got {:?}",
        out.diagnostics
    );
}

#[test]
fn check_rejects_duplicate_enum_variants() {
    let dir = TestDir::new("dup_enum_variants");
    dir.write(
        "main.orl",
        "module app.test\nenum E\n    A\n    A\nend\nmain()\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"bind.duplicate_variant"),
        "expected bind.duplicate_variant, got {:?}",
        out.diagnostics
    );
}

#[test]
fn check_rejects_duplicate_fields_in_enum_variant() {
    let dir = TestDir::new("dup_variant_fields");
    dir.write(
        "main.orl",
        "module app.test\nenum E\n    A(x: int, x: int)\nend\nmain()\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"bind.duplicate_field"),
        "expected bind.duplicate_field, got {:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_struct_with_unique_fields() {
    let dir = TestDir::new("ok_struct_unique");
    dir.write(
        "main.orl",
        "module app.test\nstruct S\n    x: int\n    y: string\nend\nmain()\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_accepts_enum_with_unique_variants() {
    let dir = TestDir::new("ok_enum_unique");
    dir.write(
        "main.orl",
        "module app.test\nenum E\n    A\n    B\nend\nmain()\nend\n",
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn fs_file_handle_native() {
    let dir = TestDir::new("fs_file_handle_native");
    let test_file = dir
        .path("test_file.txt")
        .to_string_lossy()
        .replace('\\', "/");
    dir.write(
        "main.orl",
        &format!(
            r#"module app.main

import ori.fs = fs
import ori.io = io
import ori.bytes = bytes_mod

write_helper(path: string) -> result[void, string]
    using file: fs.File = try fs.open_write(path)
    const n: int = try fs.write(file, b"hello using file")
    io.println(f"written: {{n}}")
    return ok()
end

read_helper(path: string) -> result[string, string]
    using file: fs.File = try fs.open_read(path)
    const data: bytes = try fs.read(file, 20)
    const s: string = try bytes_mod.decode_utf8(data)
    return ok(s)
end

main()
    const path: string = "{test_file}"
    match write_helper(path)
        case ok(_):
            match read_helper(path)
                case ok(s):
                    io.println(s)
                case err(err):
                    io.println(f"read error: {{err}}")
            end
        case err(err):
            io.println(f"write error: {{err}}")
    end
end
"#
        ),
    );

    let exe = exe_path(&dir, "fs_file_handle_native");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["written: 16", "hello using file"]);
}

#[test]
fn task_cancellation_native() {
    let dir = TestDir::new("task_cancellation_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.task = task
import ori.io = io

async worker(token: task.CancelToken)
    io.println("worker started")
    const fut: future[void] = task.sleep(5000)
    task.associate(token, fut)
    await fut
    io.println("worker finished")
end

main()
    const token: task.CancelToken = task.create_token()
    const job: task.Job[void] = task.spawn(() -> void
        task.block_on(worker(token))
    end)

    task.block_on(task.sleep(50))
    io.println("cancelling worker")
    task.cancel(token)

    match task.join(job)
        case ok(_):
            io.println("job joined successfully")
        case err(_):
            io.println("join error")
    end
end
"#,
    );

    let exe = exe_path(&dir, "task_cancellation_native");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(
        lines,
        [
            "worker started",
            "cancelling worker",
            "job joined successfully"
        ]
    );
}

#[test]
fn compile_runs_structural_equality_advanced_native() {
    let dir = TestDir::new("structural_equality_advanced_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps
import ori.core = core

struct Pair[A, B]
    first: A
    second: B
end

check_generic_eq for T: core.Equatable (left: T, right: T) -> bool
    return left == right
end

main()
    const p1: Pair[string, int] = { first: "hello", second: 42 }
    const p2: Pair[string, int] = { first: "hello", second: 42 }
    const p3: Pair[string, int] = { first: "world", second: 42 }
    const p4: Pair[string, int] = { first: "hello", second: 43 }

    io.println(string(p1 == p2))
    io.println(string(p1 != p3))
    io.println(string(p1 != p4))

    const map1: map[string, Pair[string, int]] = maps.new()
    const item1: Pair[string, int] = { first: "hello", second: 1 }
    const item2: Pair[string, int] = { first: "world", second: 2 }
    maps.set(map1, "key1", item1)
    maps.set(map1, "key2", item2)

    const map2: map[string, Pair[string, int]] = maps.new()
    maps.set(map2, "key2", item2)
    maps.set(map2, "key1", item1)

    const map3: map[string, Pair[string, int]] = maps.new()
    const item3: Pair[string, int] = { first: "world", second: 3 }
    maps.set(map3, "key1", item1)
    maps.set(map3, "key2", item3)

    io.println(string(map1 == map2))
    io.println(string(map1 != map3))

    io.println(string(check_generic_eq(p1, p2)))
    io.println(string(check_generic_eq(p1, p3)))
end
"#,
    );

    let exe = exe_path(&dir, "structural_equality_advanced_native");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(
        lines,
        ["true", "true", "true", "true", "true", "true", "false"]
    );
}

#[test]
fn build_c_backend_structural_equality_advanced() {
    let dir = TestDir::new("c_backend_structural_equality_advanced");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core

struct Pair[A, B]
    first: A
    second: B
end

main()
    const p1: Pair[string, int] = { first: "hello", second: 42 }
    const p2: Pair[string, int] = { first: "hello", second: 42 }
    const is_equal: bool = p1 == p2
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_string_eq"), "{}", out.c_source);
}

/// Regression: ABI zero-extension of sub-32-bit args to extern symbols.
/// A bool produced by `sete` carried garbage in the upper register bits;
/// optimized runtime code (rustc/LLVM assumes SysV caller-extension) computed
/// the string length from the wide register and printed "fals".
#[test]
fn compile_runs_bool_from_string_eq_prints_correctly() {
    let dir = TestDir::new("bool_string_eq_print");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    io.println(string("a" == "b"))
    io.println(string("same" == "same"))
end
"#,
    );
    let exe = exe_path(&dir, "bool_string_eq_print");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["false", "true"]);
}

/// Regression (LANG-FRONT-1): a local binding must shadow a bare stdlib
/// builtin of the same name. `const len = ...; return len` used to lower
/// the identifier to the stdlib runtime symbol (`ori_len`) and fail native
/// codegen with "undefined variable"; calls to the builtin keep working.
#[test]
fn compile_runs_local_binding_shadows_bare_builtin() {
    let dir = TestDir::new("local_shadows_bare_builtin");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists

helper() -> int
    var xs: list[string] = lists.new()
    lists.push(xs, "ab")
    const len: int = lists.len(xs)
    const first: string = lists.get(xs, 0)
    -- the builtin is still callable while the local exists
    return len + string_len(first)
end

string_len(s: string) -> int
    return len(s)
end

main()
    io.println(string(helper()))
end
"#,
    );
    let exe = exe_path(&dir, "local_shadows_bare_builtin");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
}

// ─── LANG-MEM-7 — ARC instrumentation (ORI_DUMP_ARC) ─────────────────────────

/// Plan F7/F4 acceptance: `ORI_DUMP_ARC=<file>` writes per-function counts
/// of the ARC ops inserted in the final CLIF, and the return-transfer
/// elision (LANG-MEM-4) keeps `make_list`-style builders at zero
/// retain/release. Spawns `ori` so the env var never leaks into (or races
/// with) other tests in this process.
#[test]
fn compile_dump_arc_reports_ops_and_return_transfer_elision() {
    let dir = TestDir::new("dump_arc_report");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists

make_list(n: int) -> list[int]
    const xs: list[int] = lists.with_capacity(2)
    lists.push(xs, n)
    return xs
end

main()
    const s: list[int] = make_list(3)
    io.print(string(lists.len(s)))
end
"#,
    );
    let dump_path = dir.path("arc_dump.txt");
    let exe = exe_path(&dir, "dump_arc_report");
    let output = Command::new(env!("CARGO_BIN_EXE_ori"))
        .args([
            "compile",
            dir.path("main.orl").to_str().unwrap(),
            "-o",
            exe.to_str().unwrap(),
        ])
        .env("ORI_DUMP_ARC", dump_path.to_str().unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = std::fs::read_to_string(&dump_path).unwrap();

    // Sections start with `;; ---- ARC ops for <func> ----`; splitting on the
    // prefix yields one chunk per function, ending before the next header.
    let make_list_section = dump
        .split(";; ---- ARC ops for ")
        .find(|s| s.starts_with("app.main.make_list "))
        .unwrap_or_else(|| panic!("missing make_list section in dump:\n{dump}"));
    // Return-transfer elision: the builder hands the binding's +1 to the
    // caller — no retain/release may be emitted in its body.
    assert!(
        !make_list_section.contains("ori_arc_retain"),
        "return-transfer elision regressed (retain in make_list):\n{make_list_section}"
    );
    assert!(
        !make_list_section.contains("ori_arc_release"),
        "return-transfer elision regressed (release in make_list):\n{make_list_section}"
    );
    assert!(
        make_list_section.contains("ori_arc_maybe_collect_cycles"),
        "expected the safe-point probe in make_list:\n{make_list_section}"
    );

    // main releases its bindings at scope exit — the dump must show that.
    let main_section = dump
        .split(";; ---- ARC ops for ")
        .find(|s| s.starts_with("app.main.main "))
        .unwrap_or_else(|| panic!("missing main section in dump:\n{dump}"));
    assert!(
        main_section.contains("ori_arc_release"),
        "expected releases in main's dump section:\n{main_section}"
    );
}

// ── Match guards (Spec 06 §match — guarded cases) ────────────────────────────
//
// Regression for the silent wrong-code bug found 2026-07-19: `case p if cond:`
// parsed and type-checked, but AST→HIR lowering dropped the guard, so the
// first binding arm captured every value at runtime in both backends.

#[test]
fn compile_runs_match_guards_select_arm_by_condition() {
    let dir = TestDir::new("match_guards_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

enum Shape
    Circle(radius: int)
    Square(side: int)
end

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

describe(shape: Shape) -> string
    var out: string = ""
    match shape
    case Circle(radius) if radius > 10:
        out = "big-circle"
    case Circle(radius):
        out = "small-circle"
    case Square(side) if side == 0:
        out = "point"
    case else:
        out = "square"
    end
    return out
end

label(name: string) -> string
    var out: string = ""
    match name
    case s if s == "ori":
        out = "lang"
    case else:
        out = "other"
    end
    return out
end

main()
    io.println(grade(95) + grade(85) + grade(50))
    io.println(describe(Shape.Circle(radius: 20)))
    io.println(describe(Shape.Circle(radius: 5)))
    io.println(describe(Shape.Square(side: 0)))
    io.println(describe(Shape.Square(side: 3)))
    io.println(label("ori") + "-" + label("x"))
end
"#,
    );

    let exe = exe_path(&dir, "match_guards");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        "ABC\nbig-circle\nsmall-circle\npoint\nsquare\nlang-other\n"
    );
}

#[test]
fn build_c_match_guard_falls_through_to_next_arm() {
    let dir = TestDir::new("build_c_match_guard");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

grade(score: int) -> string
    var out: string = ""
    match score
    case n if n >= 90:
        out = "A"
    case else:
        out = "C"
    end
    return out
end

main()
    io.println(grade(50))
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    // Guarded matches emit the goto shape: a false guard jumps past the arm
    // to the next pattern test instead of running the first binding arm.
    assert!(
        build.c_source.contains("_next0:;"),
        "guard reject label missing:\n{}",
        build.c_source
    );
    assert!(
        build.c_source.contains("_end:;"),
        "match end label missing:\n{}",
        build.c_source
    );
    assert!(
        build.c_source.contains("if (!("),
        "guard condition missing:\n{}",
        build.c_source
    );
}

// ── `match` as an expression (0.4 surface) ───────────────────────────────────
//
// Arms converge on one value through a Cranelift block parameter, so arms are
// never evaluated speculatively (unlike the `select` used by `if` expressions).

#[test]
fn compile_runs_match_expression_selects_arm_value() {
    let dir = TestDir::new("match_expr_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

enum Shape
    Circle(radius: int)
    Square(side: int)
end

label(n: int) -> string
    const text: string = match n
    case 1: "um"
    case 2: "dois"
    case else: "outro"
    end
    return text
end

grade(score: int) -> string
    return match score
    case n if n >= 90: "A"
    case n if n >= 80: "B"
    case else: "C"
    end
end

describe(s: Shape) -> string
    return match s
    case Circle(radius) if radius > 10: f"big-circle-{radius}"
    case Circle(radius): f"circle-{radius}"
    case Square(side): f"square-{side}"
    case else: "?"
    end
end

pick(flag: bool, a: string, b: string) -> string
    return match flag
    case true: a
    case else: b
    end
end

main()
    io.println(label(1) + label(2) + label(9))
    io.println(grade(95) + grade(85) + grade(10))
    io.println(describe(Shape.Circle(radius: 20)))
    io.println(describe(Shape.Circle(radius: 3)))
    io.println(describe(Shape.Square(side: 7)))
    io.println(pick(true, "left", "right") + pick(false, "left", "right"))
    const n: int = 2
    io.println(match n
    case 1: "one"
    case else: match n
        case 2: "two"
        case else: "many"
        end
    end)
end
"#,
    );

    let exe = exe_path(&dir, "match_expr");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        "umdoisoutro\nABC\nbig-circle-20\ncircle-3\nsquare-7\nleftright\ntwo\n"
    );
}

#[test]
fn check_rejects_match_expression_with_mismatched_arm_types() {
    let dir = TestDir::new("match_expr_arm_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const n: int = 1
    const bad = match n
    case 1: "texto"
    case else: 42
    end
    io.println("unused")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        diagnostic_codes(&out).contains(&"type.match_arm_mismatch"),
        "expected arm-type mismatch: {:?}",
        out.diagnostics
    );
}

#[test]
fn check_rejects_non_exhaustive_match_expression() {
    let dir = TestDir::new("match_expr_non_exhaustive");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

enum Shape
    Circle(radius: int)
    Square(side: int)
end

name(s: Shape) -> string
    return match s
    case Circle(radius): "circle"
    end
end

main()
    io.println(name(Shape.Square(side: 1)))
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        diagnostic_codes(&out)
            .iter()
            .any(|code| code.starts_with("match.")),
        "expected an exhaustiveness diagnostic: {:?}",
        out.diagnostics
    );
}

#[test]
fn build_c_match_expression_emits_result_temporary() {
    let dir = TestDir::new("match_expr_c");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

label(n: int) -> string
    return match n
    case 1: "um"
    case else: "outro"
    end
end

main()
    io.println(label(1))
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    // C has no multi-arm conditional expression: the arms assign one result
    // temporary and jump to a shared end label.
    assert!(
        build.c_source.contains("_end:;"),
        "match end label missing:\n{}",
        build.c_source
    );
    assert!(
        build.c_source.contains("goto "),
        "arm jump missing:\n{}",
        build.c_source
    );
}

// ── `if ok(v) =` / `if err(e) =` (0.4 surface) ──────────────────────────────
//
// Symmetry with the long-standing `if some(x) =`: same form, same node, only
// the inspected wrapper and the bound side differ. `err` takes the branch when
// the result is NOT ok.

#[test]
fn compile_runs_if_ok_and_if_err_bindings() {
    let dir = TestDir::new("if_ok_err_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("divide-by-zero")
    end
    return ok(a / b)
end

main()
    if ok(v) = divide(10, 2)
        io.println(f"ok={v}")
    else
        io.println("unexpected-ok-else")
    end

    if ok(v) = divide(1, 0)
        io.println(f"unexpected={v}")
    else
        io.println("ok-else")
    end

    if err(e) = divide(1, 0)
        io.println(f"err={e}")
    else
        io.println("unexpected-err-else")
    end

    if err(e) = divide(9, 3)
        io.println(f"unexpected={e}")
    else
        io.println("err-else")
    end

    if ok(v) = divide(8, 4)
        io.println(f"no-else={v}")
    end

    const maybe: optional[string] = some("still-works")
    if some(x) = maybe
        io.println(x)
    end
end
"#,
    );

    let exe = exe_path(&dir, "if_ok_err");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        "ok=5\nok-else\nerr=divide-by-zero\nerr-else\nno-else=2\nstill-works\n"
    );
}

#[test]
fn check_rejects_if_ok_on_non_result() {
    let dir = TestDir::new("if_ok_not_result");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const maybe: optional[int] = some(1)
    if ok(v) = maybe
        io.println(f"{v}")
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        diagnostic_codes(&out).contains(&"type.ifok_not_result"),
        "expected if-ok wrapper mismatch: {:?}",
        out.diagnostics
    );
}

#[test]
fn build_c_if_err_inverts_the_ok_flag() {
    let dir = TestDir::new("if_err_c");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

load() -> result[int, string]
    return err("boom")
end

main()
    if err(e) = load()
        io.println(e)
    end
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(
        build.c_source.contains(".is_ok)") && build.c_source.contains("if (!"),
        "expected an inverted is_ok test:\n{}",
        build.c_source
    );
    assert!(
        build.c_source.contains(".value.err"),
        "expected the err side of the payload union:\n{}",
        build.c_source
    );
}

// ── Or-patterns: `case North or South:` (0.4 surface) ───────────────────────
//
// The word `or` (not `|`): the language already spells boolean operators as
// words, and the comma is taken by payload fields.

#[test]
fn compile_runs_or_patterns_across_kinds() {
    let dir = TestDir::new("or_patterns_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

enum Direction
    North
    South
    East
    West
end

axis(d: Direction) -> string
    match d
    case North or South:
        return "vertical"
    case else:
        return "horizontal"
    end
end

size(n: int) -> string
    match n
    case 1 or 2 or 3:
        return "small"
    case else:
        return "big"
    end
end

label(n: int) -> string
    return match n
    case 0 or 1: "binary"
    case else: "other"
    end
end

kind(s: string) -> string
    match s
    case "a" or "b":
        return "letter"
    case else:
        return "?"
    end
end

main()
    io.println(axis(Direction.North) + axis(Direction.South) + axis(Direction.West))
    io.println(size(2) + size(9))
    io.println(label(1) + label(7))
    io.println(kind("b") + kind("z"))
end
"#,
    );

    let exe = exe_path(&dir, "or_patterns");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        "verticalverticalhorizontal\nsmallbig\nbinaryother\nletter?\n"
    );
}

#[test]
fn check_or_pattern_covers_enum_exhaustiveness() {
    let dir = TestDir::new("or_pattern_exhaustive");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

enum Direction
    North
    South
    East
    West
end

axis(d: Direction) -> string
    var out: string = ""
    match d
    case North or South:
        out = "vertical"
    case East or West:
        out = "horizontal"
    end
    return out
end

main()
    io.println(axis(Direction.East))
end
"#,
    );

    // No `case else`: the two or-patterns together cover all four variants.
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        !diagnostic_codes(&out).contains(&"match.non_exhaustive"),
        "or-patterns should count toward coverage: {:?}",
        out.diagnostics
    );
}

#[test]
fn check_rejects_bindings_inside_or_patterns() {
    let dir = TestDir::new("or_pattern_binding");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

enum Shape
    Circle(radius: int)
    Square(side: int)
end

describe(s: Shape) -> string
    match s
    case Circle(r) or Square(r):
        return "has-side"
    case else:
        return "?"
    end
end

main()
    io.println(describe(Shape.Circle(radius: 1)))
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        diagnostic_codes(&out).contains(&"match.or_pattern_binding"),
        "expected or-pattern binding rejection: {:?}",
        out.diagnostics
    );
}

#[test]
fn build_c_or_pattern_emits_disjunction() {
    let dir = TestDir::new("or_pattern_c");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

size(n: int) -> string
    match n
    case 1 or 2:
        return "small"
    case else:
        return "big"
    end
end

main()
    io.println(size(1))
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(
        build.c_source.contains("||"),
        "expected a disjunction for the or-pattern:\n{}",
        build.c_source
    );
}
