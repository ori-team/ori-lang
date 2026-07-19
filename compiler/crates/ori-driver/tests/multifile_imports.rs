use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::pipeline::{
    run_build, run_check, run_compile, run_compile_with_options, run_doc, run_doc_check,
    run_doc_with_options, run_fmt, run_test, run_test_with_options, CheckOutput, CompileOptions,
    DocCheckOutput, DocFormat, DocOptions, TestOptions,
};

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_test_{}_{}_{}",
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

fn diagnostic_codes(out: &CheckOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code).collect()
}

fn doc_diagnostic_codes(out: &DocCheckOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code).collect()
}

fn ori_path_literal(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let file_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&file_name)
}

fn compile_c_source(dir: &TestDir, name: &str, source: &str) {
    let c_path = dir.path(&format!("{name}.c"));
    let obj_path = dir.path(&format!("{name}.o"));
    std::fs::write(&c_path, source).unwrap();
    let output = match Command::new("cc")
        .arg("-std=gnu11")
        .arg("-c")
        .arg(&c_path)
        .arg("-o")
        .arg(&obj_path)
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => panic!("failed to run cc: {err}"),
    };
    assert!(
        output.status.success(),
        "generated C did not compile\nstderr:\n{}\nsource:\n{}",
        String::from_utf8_lossy(&output.stderr),
        source,
    );
}

fn normalize_stdout(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap().replace("\r\n", "\n")
}

fn compile_and_run(dir: &TestDir, exe_name: &str) -> String {
    let exe = exe_path(dir, exe_name);
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    normalize_stdout(output.stdout)
}

#[test]
fn check_loads_default_import_alias_and_imported_return_type() {
    let dir = TestDir::new("default_import");
    dir.write(
        "util.orl",
        r#"module app.util

public answer() -> int
    return 11
end
"#,
    );
    // S3 bare import: no implicit last-segment alias — full path only.
    dir.write(
        "main.orl",
        r#"module app.main

import app.util

main()
    const value: int = app.util.answer()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        !diagnostic_codes(&out).contains(&"bind.unused_import"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_bare_import_does_not_create_last_segment_alias() {
    let dir = TestDir::new("bare_import_no_alias");
    dir.write(
        "util.orl",
        r#"module app.util

public answer() -> int
    return 11
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util

main()
    const value: int = util.answer()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"name.undefined"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_path_eq_alias_and_imports_block() {
    let dir = TestDir::new("imports_block_s3");
    dir.write(
        "util.orl",
        r#"module app.util

public answer() -> int
    return 7
end
"#,
    );
    dir.write(
        "math.orl",
        r#"module app.math

public add(a: int, b: int) -> int
    return a + b
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

imports
  app.util = util, app.math (add)
end

main()
    const value: int = util.answer()
    const total: int = add(value, 1)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_removed_import_as_and_only() {
    let dir = TestDir::new("import_as_only_removed");
    dir.write(
        "util.orl",
        r#"module app.util

public answer() -> int
    return 1
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util as util
import app.util only (answer)

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"parse.import_as_removed"),
        "{:?}",
        out.diagnostics
    );
    assert!(
        codes.contains(&"parse.import_only_removed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn doc_extracts_markdown_from_documentation_comments() {
    let dir = TestDir::new("doc_extracts_markdown");
    dir.write(
        "main.orl",
        r#"module app.main

--|
Computes an area.

@param width Width in pixels.
@param height Height in pixels.
@returns The computed area.
|--
public area(width: int, height: int) -> int
    return width * height
end
"#,
    );

    let out = run_doc(&dir.path("main.orl")).unwrap();

    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.markdown.contains("# Ori API Documentation"));
    assert!(out.markdown.contains("## app.main.area"));
    assert!(out.markdown.contains("- Kind: function"));
    assert!(
        out.markdown
            .contains("public area(width: int, height: int) -> int"),
        "{}",
        out.markdown
    );
    assert!(out.markdown.contains("Computes an area."));
    assert!(out.markdown.contains("- `width`: Width in pixels."));
    assert!(out.markdown.contains("- `height`: Height in pixels."));
    assert!(out.markdown.contains("Returns: The computed area."));
}

#[test]
fn doc_lists_stdlib_modules_collection_signatures_and_constraints() {
    let dir = TestDir::new("doc_stdlib_collections");
    dir.write(
        "main.orl",
        r#"module app.main

main()
end
"#,
    );

    let out = run_doc(&dir.path("main.orl")).unwrap();

    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.markdown.contains("## Standard Library"));
    assert!(out.markdown.contains("### Modules"));
    assert!(out.markdown.contains("- `ori.map`"));
    assert!(out.markdown.contains("- `ori.heap`"));
    assert!(out.markdown.contains("### Collection Signatures"));
    assert!(
        out.markdown.contains("queue.new[T]() -> queue.Queue[T]"),
        "{}",
        out.markdown
    );
    assert!(
        out.markdown
            .contains("graph.topological_sort[N](g: graph.Graph[N]) -> list[N]"),
        "{}",
        out.markdown
    );
    assert!(
        out.markdown
            .contains("maps.new[K, V]() -> map[K, V] for K: Hashable, K: Equatable"),
        "{}",
        out.markdown
    );
    assert!(
        out.markdown
            .contains("heap.new[T]() -> heap.Heap[T] for T: Comparable"),
        "{}",
        out.markdown
    );
}

#[test]
fn doc_renders_static_html_output() {
    let dir = TestDir::new("doc_html_output");
    dir.write(
        "main.orl",
        r#"module app.main

--|
Greets the user.
|--
public greet(name: string) -> string
    return name
end
"#,
    );

    let out = run_doc_with_options(
        &dir.path("main.orl"),
        DocOptions {
            format: DocFormat::Html,
        },
    )
    .unwrap();

    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.html.contains("<!DOCTYPE html>"));
    assert!(out.html.contains("<h2 id=\"app.main.greet\">"));
    assert!(out.html.contains("Greets the user."));
    assert!(out.html.contains("<code>"));
}

#[test]
fn compile_runs_for_in_over_list_string_without_corruption() {
    let dir = TestDir::new("for_in_list_string");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.string = str

main()
    var acc: string = ""
    for ch in str.chars("AbC123")
        acc = str.concat(acc, ch)
    end
    io.print(acc)
    var tags: list[string] = []
    for piece in ["a", "bb", "c"]
        lists.push(tags, str.concat("-", piece))
    end
    io.print(str.join(tags, ","))
end
"#,
    );

    let stdout = compile_and_run(&dir, "for_in_list_string");
    assert_eq!(stdout, "AbC123\n-a,-bb,-c\n");
}

#[test]
fn check_resolves_public_reexported_import() {
    let dir = TestDir::new("public_reexport_check");
    dir.write(
        "util.orl",
        r#"module app.util

public answer(value: int) -> int
    return value + 1
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
    const value: int = api.util.answer(40)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        !diagnostic_codes(&out).contains(&"bind.unused_import"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_bare_public_import_does_not_reexport_last_segment() {
    let dir = TestDir::new("bare_public_import_no_reexport");
    dir.write(
        "util.orl",
        r#"module app.util

public answer(value: int) -> int
    return value + 1
end
"#,
    );
    dir.write(
        "facade.orl",
        r#"module app.facade

public import app.util
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.facade = api

main()
    const value: int = api.util.answer(40)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"name.undefined"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn build_lowers_public_reexported_import_to_real_symbol() {
    let dir = TestDir::new("public_reexport_build");
    dir.write(
        "model.orl",
        r#"module app.model

public struct User
    id: int
    name: string
end

public make_user() -> User
    return User { id: 34, name: "Ada" }
end
"#,
    );
    dir.write(
        "facade.orl",
        r#"module app.facade

public import app.model = model
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.facade = api

make_user() -> api.model.User
    return api.model.make_user()
end

main()
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build.c_source.contains("ORI__app_dot_model_dot_make_user"));
    assert!(build.c_source.contains("ORI__app_dot_main_dot_make_user"));
    assert!(!build
        .c_source
        .contains("ORI__app_dot_facade_dot_model_dot_make_user"));
    compile_c_source(&dir, "public_reexport_build", &build.c_source);
}

#[test]
fn check_reports_imported_function_argument_count_mismatch() {
    let dir = TestDir::new("imported_arg_count");
    dir.write(
        "util.orl",
        r#"module app.util

public add_one(value: int) -> int
    return value + 1
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = util

main()
    const value: int = util.add_one()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.arg_count_mismatch"));
}

#[test]
fn check_reports_imported_function_argument_type_mismatch() {
    let dir = TestDir::new("imported_arg_type");
    dir.write(
        "util.orl",
        r#"module app.util

public add_one(value: int) -> int
    return value + 1
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = util

main()
    const value: int = util.add_one("bad")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.arg_type_mismatch"));
}

#[test]
fn check_resolves_imported_types_in_signatures_and_builds_c() {
    let dir = TestDir::new("imported_types");
    dir.write(
        "model.orl",
        r#"module app.model

public struct User
    id: int
end

public same(user: User) -> User
    return user
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.model = model

pass(user: model.User) -> model.User
    return model.same(user)
end

main()
end
"#,
    );

    let check = run_check(&dir.path("main.orl")).unwrap();
    assert!(!check.has_errors, "{:?}", check.diagnostics);

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build.c_source.contains("ORI__app_dot_main_dot_pass"));
    assert!(build.c_source.contains("ORI__app_dot_model_dot_same"));
    assert!(build.c_source.contains("int main(int argc, char** argv)"));
}

#[test]
fn check_resolves_imported_struct_field_type() {
    let dir = TestDir::new("imported_struct_field");
    dir.write(
        "model.orl",
        r#"module app.model

public struct User
    id: int
    name: string
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.model = model

user_id(user: model.User) -> int
    return user.id
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_missing_field_on_imported_struct() {
    let dir = TestDir::new("imported_missing_field");
    dir.write(
        "model.orl",
        r#"module app.model

public struct User
    id: int
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.model = model

user_name(user: model.User) -> string
    return user.name
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.no_such_field"));
}

#[test]
fn build_lowers_imported_struct_literal() {
    let dir = TestDir::new("imported_struct_literal");
    dir.write(
        "model.orl",
        r#"module app.model

public struct User
    id: int
    name: string
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.model = model

make_user() -> model.User
    return model.User { id: 7, name: "Ada" }
end

main()
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build.c_source.contains("ORI__app_dot_main_dot_make_user"));
    assert!(build.c_source.contains(".id = INT64_C(7)"));
    assert!(build.c_source.contains(".name = ORI_STR(\"Ada\")"));
}

#[test]
fn build_lowers_imported_enum_variants() {
    let dir = TestDir::new("imported_enum_variant");
    dir.write(
        "model.orl",
        r#"module app.model

public enum Status
    Ready
    Done(code: int)
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.model = model

ready() -> model.Status
    return model.Status.Ready
end

done() -> model.Status
    return model.Status.Done(code: 2)
end

main()
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build.c_source.contains("__Ready"));
    assert!(build.c_source.contains("__Done"));
    assert!(build.c_source.contains(".payload.Done"));
}

#[test]
fn check_reports_field_access_on_non_struct() {
    let dir = TestDir::new("field_non_struct");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const value: int = 1.id
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.field_on_non_struct"));
}

#[test]
fn build_handles_same_type_name_in_distinct_imported_namespaces() {
    let dir = TestDir::new("same_type_name");
    std::fs::create_dir_all(dir.path("left")).unwrap();
    std::fs::create_dir_all(dir.path("right")).unwrap();
    dir.write(
        "left/user.orl",
        r#"module left.user

public struct User
    id: int
end

public same(user: User) -> User
    return user
end
"#,
    );
    dir.write(
        "right/user.orl",
        r#"module right.user

public struct User
    id: int
end

public same(user: User) -> User
    return user
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import left.user = left
import right.user = right

take_left(user: left.User) -> left.User
    return left.same(user)
end

take_right(user: right.User) -> right.User
    return right.same(user)
end

main()
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert_eq!(build.c_source.matches("int64_t id;").count(), 2);
    assert!(build.c_source.matches("\nstruct ori_def_").count() >= 2);
    assert!(build.c_source.matches("typedef struct ori_def_").count() >= 2);
    assert!(build.c_source.contains("ORI__left_dot_user_dot_same"));
    assert!(build.c_source.contains("ORI__right_dot_user_dot_same"));
}

#[test]
fn build_uses_qualified_names_for_imported_constants() {
    let dir = TestDir::new("imported_constants");
    dir.write(
        "config.orl",
        r#"module app.config

public const LIMIT: int = 21
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.config = config
import ori.io = io

main()
    const value: int = config.LIMIT
    io.print(string(value))
end
"#,
    );

    let build = run_build(&dir.path("main.orl")).unwrap();
    assert!(!build.has_errors, "{:?}", build.diagnostics);
    assert!(build
        .c_source
        .contains("static const int64_t app_dot_config_dot_LIMIT = INT64_C(21);"));
    assert!(build
        .c_source
        .contains("int64_t value = app_dot_config_dot_LIMIT;"));
}

#[test]
fn compile_uses_imported_constant_value() {
    let dir = TestDir::new("compile_imported_constant");
    dir.write(
        "config.orl",
        r#"module app.config

public const LIMIT: int = 21
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.config = config
import ori.io = io

main()
    io.print(string(config.LIMIT))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "const_main.exe"
    } else {
        "const_main"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "21");
}

#[test]
fn compile_uses_top_level_constant_global_data() {
    let dir = TestDir::new("compile_global_const_data");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

const LIMIT: int = 31

main()
    io.print(string(LIMIT))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "global_const.exe"
    } else {
        "global_const"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "31");
}

#[test]
fn compile_updates_top_level_mutable_global() {
    let dir = TestDir::new("compile_global_var_data");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

var counter: int = 2

bump()
    counter = counter + 5
end

main()
    bump()
    io.print(string(counter))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "global_var.exe"
    } else {
        "global_var"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn compile_runs_top_level_managed_globals_native() {
    let dir = TestDir::new("compile_global_managed_data");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.map = maps

const PREFIX: string = "start"
var current: string = "one"
var values: list[string] = ["a", "b"]
var labels: map[string, string] = { "x": "old" }

update()
    current = current + "-two"
    lists.push(values, current)
    maps.set(labels, "x", current)
end

main()
    update()
    io.print(PREFIX)
    io.print(current)
    io.print(lists.get(values, 2))
    io.print(maps.get(labels, "x"))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "global_managed.exe"
    } else {
        "global_managed"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "start\none-two\none-two\none-two\n"
    );
}

#[test]
fn compile_runs_string_stdlib_len_concat_and_slice() {
    let dir = TestDir::new("compile_string_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

main()
    const joined: string = str.concat("ab", "cdef")
    const part: string = str.slice(joined, 1, 4)
    io.print(part)
    io.print(string(str.len(part)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "string_stdlib.exe"
    } else {
        "string_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "bcd\n3\n");
}

#[test]
fn compile_runs_native_to_string_parts() {
    let dir = TestDir::new("compile_native_to_string_parts");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    io.print(string(-120))
    io.print(string(0))
    io.print(string(true))
    io.print(string(2.5))
    const stored: string = string(55)
    const stored_bool: string = string(false)
    const stored_float: string = string(3.25)
    io.print(stored)
    io.print(stored_bool)
    io.print(stored_float)
    io.print(f"{true} {2.5}")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "native_to_string_parts.exe"
    } else {
        "native_to_string_parts"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "-120\n0\ntrue\n2.5\n55\nfalse\n3.25\ntrue 2.5\n"
    );
}

#[test]
fn compile_runs_displayable_string_conversion_native() {
    let dir = TestDir::new("displayable_string_conversion_native");
    dir.write(
        "main.orl",
        r##"module app.main

import ori.core = core
import ori.io = io

struct Resource
    id: int
end

apply Resource use core.Displayable
    display(self) -> string
        return "Resource#" + string(self.id)
    end
end

main()
    const resource: Resource = Resource { id: 7 }
    io.print(string(resource))
    io.print(string("ready"))
    io.print(f"value={resource}")
end
"##,
    );

    let exe = dir.path(if cfg!(windows) {
        "displayable_string_conversion.exe"
    } else {
        "displayable_string_conversion"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "Resource#7\nready\nvalue=Resource#7\n"
    );
}

#[test]
fn build_c_backend_displayable_string_conversion() {
    let dir = TestDir::new("c_backend_displayable_string_conversion");
    dir.write(
        "main.orl",
        r##"module app.main

import ori.core = core
import ori.io = io

struct Resource
    id: int
end

apply Resource use core.Displayable
    display(self) -> string
        return "Resource#" + string(self.id)
    end
end

main()
    const resource: Resource = Resource { id: 7 }
    io.print(string(resource))
    io.print(string("ready"))
    io.print(f"value={resource}")
end
"##,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source
            .contains("ORI__app_dot_main_dot_Resource_dot_Displayable_dot_display"),
        "{}",
        out.c_source
    );

    compile_c_source(
        &dir,
        "c_backend_displayable_string_conversion",
        &out.c_source,
    );
}

#[test]
fn check_rejects_non_displayable_f_string_interpolation() {
    let dir = TestDir::new("non_displayable_f_string");
    dir.write(
        "main.orl",
        r#"module app.main

struct Secret
    id: int
end

main()
    const secret: Secret = Secret { id: 7 }
    const text: string = f"value={secret}"
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(diagnostic_codes(&out).contains(&"type.arg_type_mismatch"));
}

#[test]
fn compile_runs_native_interpolation_with_string_length_helper() {
    let dir = TestDir::new("compile_native_interpolation_string_len");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const name: string = "Ori"
    io.print(f"{name} language")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "native_interpolation_string_len.exe"
    } else {
        "native_interpolation_string_len"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "Ori language");
}

#[test]
fn compile_runs_extended_string_stdlib() {
    let dir = TestDir::new("compile_extended_string_stdlib");
    dir.write("main.orl", r#"module app.main

import ori.io = io
import ori.string = str

main()
    const trimmed: string = str.trim("  Abc Def  ")
    const lower: string = str.to_lower(trimmed)
    const upper: string = str.to_upper(lower)
    const replaced: string = str.replace(upper, "DEF", "XYZ")
    io.print(replaced)
    if str.contains(replaced, "XYZ") and str.starts_with(replaced, "ABC") and str.ends_with(replaced, "XYZ")
        io.print("ok")
    else
        io.print("bad")
    end
end
"#);

    let exe = dir.path(if cfg!(windows) {
        "extended_string_stdlib.exe"
    } else {
        "extended_string_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "ABC XYZ\nok\n");
}

#[test]
fn compile_runs_more_string_and_conversion_stdlib() {
    let dir = TestDir::new("compile_more_string_conversion_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.convert = conv
import ori.io = io
import ori.string = str

main()
    const parts: list[string] = str.split("a,b,c", ",")
    io.print(str.join(parts, "|"))
    io.print(str.repeat("ha", 3))
    io.print(str.pad_left("7", 3, "0"))
    io.print(str.pad_right("x", 3, "."))
    io.print(str.trim_start("  left"))
    io.print(str.trim_end("right  "))
    io.print(string(str.index_of("abcdef", "cd")))
    io.print(conv.float_to_string(2.5))
    io.print(conv.bool_to_string(false))
    if some(n) = conv.string_to_int("41")
        io.print(string(n + 1))
    end
    if some(f) = conv.string_to_float("3.5")
        io.print(conv.float_to_string(f))
    end
    match conv.string_to_int("not a number")
        case some(n):
            io.print(string(n))
        case none:
            io.print("int:none")
    end
    match conv.string_to_float("not a number")
        case some(f):
            io.print(conv.float_to_string(f))
        case none:
            io.print("float:none")
    end
    match str.parse_int("42")
        case ok(n):
            io.print(string(n + 1))
        case err(message):
            io.print(message)
    end
    match str.parse_int("not a number")
        case ok(n):
            io.print(string(n))
        case err(message):
            io.print(message)
    end
    match str.parse_float("6.25")
        case ok(f):
            io.print(conv.float_to_string(f))
        case err(message):
            io.print(message)
    end
    match str.parse_float("not a number")
        case ok(f):
            io.print(conv.float_to_string(f))
        case err(message):
            io.print(message)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "more_string_conversion_stdlib.exe"
    } else {
        "more_string_conversion_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "a|b|c\nhahaha\n007\nx..\nleft\nright\n2\n2.5\nfalse\n42\n3.5\nint:none\nfloat:none\n43\ninvalid int\n6.25\ninvalid float\n"
    );
}

#[test]
fn compile_runs_list_index_set_and_len() {
    let dir = TestDir::new("compile_list_index_set_len");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists

main()
    var values: list[int] = [10, 20, 30]
    values[1] = values[0] + values[2]
    io.print(string(values[1]))
    io.print(string(lists.len(values)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "list_index.exe"
    } else {
        "list_index"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "40\n3\n");
}

#[test]
fn compile_runs_io_read_line() {
    let dir = TestDir::new("compile_io_read_line");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    match io.read_line()
        case some(line):
            io.print(line)
        case none:
            io.print("")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "read_line.exe"
    } else {
        "read_line"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let mut child = Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"hello stdin\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "hello stdin\n");
}

#[test]
fn check_reports_stdlib_call_type_error() {
    let dir = TestDir::new("stdlib_call_type_error");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.string = str

main()
    const bad: string = str.concat("a", 1)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.arg_type_mismatch"));
}

#[test]
fn compile_runs_math_stdlib() {
    let dir = TestDir::new("compile_math_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.math = math

main()
    io.print(string(math.abs(-9)))
    io.print(string(math.min(8, 3) + math.max(8, 3)))
    if math.sqrt(9.0) == 3.0
        io.print("sqrt")
    else
        io.print("bad")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "math_stdlib.exe"
    } else {
        "math_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "9\n11\nsqrt\n");
}

#[test]
fn compile_runs_more_math_stdlib() {
    let dir = TestDir::new("compile_more_math_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.math = math

main()
    const floored: int = math.floor(3.9)
    const ceiled: int = math.ceil(3.1)
    const rounded: int = math.round(3.5)
    io.print(string(floored + ceiled + rounded))
    io.print(string(math.clamp(15, 0, 10)))
    if math.pow(2.0, 3.0) == 8.0 and math.log(1.0) == 0.0 and math.log2(1.0) == 0.0
        io.print("powlog")
    else
        io.print("bad")
    end
    if math.sin(0.0) == 0.0 and math.cos(0.0) == 1.0 and math.tan(0.0) == 0.0
        io.print("trig")
    else
        io.print("bad")
    end
    if math.pi > 3.0 and math.e > 2.0 and math.is_nan(math.nan) and math.is_infinite(math.infinity)
        io.print("special")
    else
        io.print("bad")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "more_math_stdlib.exe"
    } else {
        "more_math_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "11\n10\npowlog\ntrig\nspecial\n"
    );
}

#[test]
fn compile_runs_math_float_overloads() {
    let dir = TestDir::new("math_float_overloads");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.math = math

main()
    const a: float = math.abs(-2.5)
    const b: float = math.min(1.0, 2.0)
    const c: float = math.max(1.0, 2.0)
    if a == 2.5 and b == 1.0 and c == 2.0
        io.print("float-overloads")
    else
        io.print("bad")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "math_float_overloads.exe"
    } else {
        "math_float_overloads"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "float-overloads\n");
}

#[test]
fn build_c_backend_compiles_math_stdlib_surface() {
    let dir = TestDir::new("c_backend_math_stdlib_surface");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.math = math

main()
    const floored: int = math.floor(3.9)
    const clamped: int = math.clamp(15, 0, 10)
    const absf: float = math.abs(-2.5)
    const minf: float = math.min(1.0, 2.0)
    const maxf: float = math.max(1.0, 2.0)
    const logged: float = math.log2(1.0)
    const special: bool = math.is_nan(math.nan) and math.is_infinite(math.infinity)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(out.c_source.contains("ori_math_clamp"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_math_abs_float"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_math_min_float"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_math_max_float"),
        "{}",
        out.c_source
    );
    assert!(out.c_source.contains("ori_math_log2"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_math_is_nan"), "{}", out.c_source);
    compile_c_source(&dir, "c_backend_math_stdlib_surface", &out.c_source);
}

#[test]
fn compile_runs_string_split_and_chars() {
    let dir = TestDir::new("compile_string_split_chars");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.string = str

main()
    const parts: list[string] = str.split("red,blue", ",")
    const chars: list[string] = str.chars("abc")
    io.print(parts[0])
    io.print(parts[1])
    io.print(chars[2])
    io.print(string(lists.len(parts) + lists.len(chars)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "string_split_chars");
    assert_eq!(stdout, "red\nblue\nc\n5\n");
}

#[test]
fn compile_runs_set_and_map_stdlib() {
    let dir = TestDir::new("compile_set_map_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps
import ori.set = sets
import ori.core = core

struct Token
    id: int
end

apply Token use core.Hashable
end

main()
    const seen: set[int] = sets.new()
    sets.add(seen, 4)
    sets.add(seen, 4)
    sets.add(seen, 9)
    const scores: map[int, int] = maps.new()
    maps.set(scores, 4, 40)
    maps.set(scores, 9, 90)
    maps.set(scores, 4, 44)
    if sets.contains(seen, 9) and maps.contains(scores, 4)
        io.print(string(sets.len(seen) + maps.len(scores)))
        io.print(string(maps.get(scores, 4) + maps.get(scores, 9)))
    else
        io.print("bad")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "set_map_stdlib.exe"
    } else {
        "set_map_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "4\n134\n");
}

#[test]
fn check_accepts_string_map_keys_string_set_values_and_rejects_unsupported_hash_inputs() {
    let dir = TestDir::new("map_set_string_and_reject_unsupported_hash");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.map = maps
import ori.set = sets

main()
    const ok_map: map[string, int] = maps.new()
    maps.set(ok_map, "a", 1)
    const ok_lit: map[string, int] = { "b": 2 }

    const ok_set: set[string] = sets.new()
    sets.add(ok_set, "a")
    const ok_set_lit: set[string] = set { "b" }

    const bad_map: map[list[int], int] = maps.new()
    maps.set(bad_map, [1], 1)

    const bad_map_lit: map[list[int], int] = { [2]: 2 }
    const bad_set: set[list[int]] = sets.new()
    sets.add(bad_set, [1])
    const bad_set_lit: set[list[int]] = set { [2] }

    const bad_named_map: map[Token, int] = maps.new()
    const bad_named_set: set[Token] = sets.new()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"type.collection_hash_unsupported"),
        "got: {:?}",
        codes
    );
}

#[test]
fn check_list_stdlib_preserves_element_types() {
    let dir = TestDir::new("list_stdlib_element_types");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.list = lists

main()
    var values: list[int] = [1, 2]
    lists.push(values, "bad")
    lists.set(values, 0, "bad")
    lists.insert(values, 0, "bad")
    const has_bad: bool = lists.contains(values, "bad")
    const bad_index: int = lists.index_of(values, "bad")
    const first: string = lists.get(values, 0)
    const slice: list[string] = lists.slice(values, 0, 1)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected list generic mismatches");
    let mismatch_count = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.code, "type.type_mismatch" | "type.arg_type_mismatch"))
        .count();
    assert!(
        mismatch_count >= 6,
        "expected several element-type mismatches, got {:?}",
        out.diagnostics
    );
}

#[test]
fn check_map_set_stdlib_preserves_key_value_and_element_types() {
    let dir = TestDir::new("map_set_stdlib_generic_types");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.map = maps
import ori.set = sets

main()
    const labels: map[int, string] = maps.new()
    maps.set(labels, "bad", "one")
    maps.set(labels, 2, 20)
    const got_int: int = maps.get(labels, 1)
    const bad_keys: list[string] = maps.keys(labels)
    const bad_values: list[int] = maps.values(labels)
    const bad_entries: list[tuple[string, int]] = maps.entries(labels)

    const seen: set[int] = sets.new()
    sets.add(seen, "bad")
    const has_bad: bool = sets.contains(seen, "bad")
    sets.remove(seen, "bad")
    const other: set[int] = sets.new()
    const wrong_union: set[string] = sets.union(seen, other)
    const bad_union_arg: set[int] = sets.union(seen, "bad")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected map/set generic mismatches");
    let mismatch_count = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.code, "type.type_mismatch" | "type.arg_type_mismatch"))
        .count();
    assert!(
        mismatch_count >= 10,
        "expected several key/value/element mismatches, got {:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_unsupported_optional_result_helper_forms() {
    let dir = TestDir::new("unsupported_optional_result_helper_forms");
    dir.write(
        "main.orl",
        r#"module app.main

maybe() -> optional[int]
    return some(1)
end

parse() -> result[int, string]
    return ok(1)
end

main()
    const early: int = maybe().or_return(none)
    const wrong_context: result[int, string] = parse().or_wrap(123)
    const wrong_receiver: optional[int] = maybe().or_wrap("context")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        out.has_errors,
        "unsupported helper forms should not type-check"
    );
    let messages = out
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("`or_return`"), "{messages}");
    assert!(messages.contains("`.or_wrap()` context"), "{messages}");
    assert!(messages.contains("`.or_wrap()` can only"), "{messages}");
}

#[test]
fn compile_runs_optional_result_or_helper_native() {
    let dir = TestDir::new("optional_result_or_helper_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

maybe() -> optional[int]
    return some(7)
end

empty() -> optional[int]
    return none
end

parse(flag: bool) -> result[int, string]
    if flag
        return ok(9)
    end
    return err("bad")
end

unexpected() -> int
    io.print("unexpected")
    return 99
end

main()
    io.print(string(maybe().or(unexpected())))
    io.print(string(empty().or(2)))
    io.print(string(parse(true).or(unexpected())))
    io.print(string(parse(false).or(4)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "optional_result_or_helper");
    assert_eq!(stdout, "7\n2\n9\n4\n");
}

#[test]
fn build_c_backend_optional_result_or_helper() {
    let dir = TestDir::new("c_backend_optional_result_or_helper");
    dir.write(
        "main.orl",
        r#"module app.main

maybe(flag: bool) -> optional[int]
    if flag
        return some(1)
    end
    return none
end

parse(flag: bool) -> result[int, string]
    if flag
        return ok(2)
    end
    return err("bad")
end

main()
    const first: int = maybe(false).or(10)
    const second: int = parse(false).or(20)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains(".has_value ?"), "{}", out.c_source);
    assert!(out.c_source.contains(".is_ok ?"), "{}", out.c_source);
    compile_c_source(&dir, "c_backend_optional_result_or_helper", &out.c_source);
}

#[test]
fn compile_runs_result_or_wrap_helper_native() {
    let dir = TestDir::new("result_or_wrap_helper_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

parse(flag: bool) -> result[int, string]
    if flag
        return ok(7)
    end
    return err("bad")
end

wrapped(flag: bool) -> result[int, string]
    return parse(flag).or_wrap("loading")
end

main()
    match wrapped(true)
    case ok(value):
        io.print(string(value))
    case err(message):
        io.print(message)
    end

    match wrapped(false)
    case ok(value):
        io.print(string(value))
    case err(message):
        io.print(message)
    end
end
"#,
    );

    let stdout = compile_and_run(&dir, "result_or_wrap_helper");
    assert_eq!(stdout, "7\nloading: bad\n");
}

#[test]
fn build_c_backend_result_or_wrap_helper() {
    let dir = TestDir::new("c_backend_result_or_wrap_helper");
    dir.write(
        "main.orl",
        r#"module app.main

parse(flag: bool) -> result[int, string]
    if flag
        return ok(1)
    end
    return err("bad")
end

main()
    const first: result[int, string] = parse(true).or_wrap("load")
    const second: result[int, string] = parse(false).or_wrap("load")
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("ori_string_concat"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_result_or_wrap_helper", &out.c_source);
}

#[test]
fn check_reports_map_set_literal_element_mismatches() {
    let dir = TestDir::new("map_set_literal_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const bad_map: map[int, int] = { 1: 10, 2: "two" }
    const bad_set: set[int] = set { 1, "two" }
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"type.map_value_mismatch"),
        "got: {:?}",
        codes
    );
    assert!(
        codes.contains(&"type.set_element_mismatch"),
        "got: {:?}",
        codes
    );
}

#[path = "multifile_imports/collections.rs"]
mod collections;

#[test]
fn build_lowers_default_parameter_arguments_to_c() {
    let dir = TestDir::new("build_default_parameter");
    dir.write(
        "main.orl",
        r#"module app.main

add(base: int, step: int = 5) -> int
    return base + step
end

main()
    const value: int = add(7)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source
            .contains("ORI__app_dot_main_dot_add(INT64_C(7), INT64_C(5))"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "default_parameter", &out.c_source);
}

#[test]
fn compile_runs_default_parameters_native() {
    let dir = TestDir::new("compile_default_parameter_native");
    dir.write(
        "math.orl",
        r#"module app.math

public scale(value: int, factor: int = 2) -> int
    return value * factor
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.math = math
import ori.io = io

add(base: int, step: int = 5) -> int
    return base + step
end

main()
    io.print(string(add(7)))
    io.print(string(add(7, 3)))
    io.print(string(math.scale(4)))
    io.print(string(math.scale(4, 3)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "default_parameter.exe"
    } else {
        "default_parameter"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "12\n10\n8\n12\n");
}

#[test]
fn compile_runs_p4_grammar_forms_native() {
    let dir = TestDir::new("p4_grammar_forms_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Point
    x: int
    y: int
end

bounded(value: int = 4 if it > 0) -> int
    return value
end

main()
    const pair: tuple[int, string] = tuple(7, "seven")
    io.print(string(pair.0))
    io.print(pair.1)

    const p: Point = { x: bounded(), y: bounded(5) }
    io.print(string(p.x + p.y))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "p4_grammar_forms.exe"
    } else {
        "p4_grammar_forms"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "7\nseven\n9\n");
}

#[test]
fn build_lowers_named_arguments_to_c_order() {
    let dir = TestDir::new("build_named_arguments");
    dir.write(
        "main.orl",
        r#"module app.main

combine(left: int, right: int) -> int
    return left * 10 + right
end

main()
    const value: int = combine(right: 2, left: 4)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source
            .contains("ORI__app_dot_main_dot_combine(INT64_C(4), INT64_C(2))"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "named_arguments", &out.c_source);
}

#[test]
fn build_lowers_variadic_parameters_to_c() {
    let dir = TestDir::new("build_variadic_parameters");
    dir.write(
        "main.orl",
        r#"module app.main

sum(seed: int, values: int...) -> int
    var total: int = seed
    for value in values
        total = total + value
    end
    return total
end

main()
    const parts: list[int] = [2, 3]
    const value: int = sum(1, ..parts, 4)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source
            .contains("ORI__app_dot_main_dot_sum(INT64_C(1),"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "variadic_parameters", &out.c_source);
}

#[test]
fn check_reports_spread_outside_variadic_parameter() {
    let dir = TestDir::new("spread_outside_variadic");
    dir.write(
        "main.orl",
        r#"module app.main

take(value: int)
end

main()
    const parts: list[int] = [1]
    take(..parts)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.spread_non_variadic"));
}

#[test]
fn compile_runs_named_arguments_native() {
    let dir = TestDir::new("compile_named_arguments_native");
    dir.write(
        "math.orl",
        r#"module app.math

public mix(first: int, second: int = 2, third: int = 3) -> int
    return first * 100 + second * 10 + third
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.math = math
import ori.io = io

pair(left: int, right: int) -> int
    return left * 10 + right
end

main()
    io.print(string(pair(right: 8, left: 6)))
    io.print(string(math.mix(third: 9, first: 1)))
    io.print(string(math.mix(third: 7, second: 4, first: 2)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "named_arguments.exe"
    } else {
        "named_arguments"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "68\n129\n247\n");
}

#[test]
fn compile_runs_native_parameter_contracts() {
    let ok_dir = TestDir::new("compile_native_param_contract_ok");
    ok_dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

require_positive(value: int if it > 0) -> int
    return value
end

gap(value: int, start: int if it < value) -> int
    return value - start
end

main()
    io.print(string(require_positive(3)))
    io.print(string(gap(7, 5)))
end
"#,
    );

    let ok_exe = ok_dir.path(if cfg!(windows) {
        "param_contract_ok.exe"
    } else {
        "param_contract_ok"
    });
    let ok_out = run_compile(&ok_dir.path("main.orl"), Path::new(&ok_exe)).unwrap();
    assert!(!ok_out.has_errors, "{:?}", ok_out.diagnostics);

    let ok_output = Command::new(&ok_exe).output().unwrap();
    assert!(ok_output.status.success(), "{:?}", ok_output);
    let stdout = String::from_utf8(ok_output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "3\n2\n");

    let fail_dir = TestDir::new("compile_native_param_contract_fail");
    fail_dir.write(
        "main.orl",
        r#"module app.main

require_positive(value: int if it > 0) -> int
    return value
end

main()
    require_positive(0)
end
"#,
    );

    let fail_exe = fail_dir.path(if cfg!(windows) {
        "param_contract_fail.exe"
    } else {
        "param_contract_fail"
    });
    let fail_out = run_compile(&fail_dir.path("main.orl"), Path::new(&fail_exe)).unwrap();
    assert!(!fail_out.has_errors, "{:?}", fail_out.diagnostics);

    let fail_output = Command::new(&fail_exe).output().unwrap();
    assert!(!fail_output.status.success(), "{:?}", fail_output);
}

#[test]
fn compile_runs_native_struct_field_contracts() {
    let ok_dir = TestDir::new("compile_native_field_contract_ok");
    ok_dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Positive
    value: int if it > 0
end

main()
    const item: Positive = Positive { value: 4 }
    io.print(string(item.value))
end
"#,
    );

    let ok_exe = ok_dir.path(if cfg!(windows) {
        "field_contract_ok.exe"
    } else {
        "field_contract_ok"
    });
    let ok_out = run_compile(&ok_dir.path("main.orl"), Path::new(&ok_exe)).unwrap();
    assert!(!ok_out.has_errors, "{:?}", ok_out.diagnostics);

    let ok_output = Command::new(&ok_exe).output().unwrap();
    assert!(ok_output.status.success(), "{:?}", ok_output);
    let stdout = String::from_utf8(ok_output.stdout).unwrap();
    assert_eq!(stdout.trim(), "4");

    let fail_dir = TestDir::new("compile_native_field_contract_fail");
    fail_dir.write(
        "main.orl",
        r#"module app.main

struct Positive
    value: int if it > 0
end

main()
    const item: Positive = Positive { value: 0 }
end
"#,
    );

    let fail_exe = fail_dir.path(if cfg!(windows) {
        "field_contract_fail.exe"
    } else {
        "field_contract_fail"
    });
    let fail_out = run_compile(&fail_dir.path("main.orl"), Path::new(&fail_exe)).unwrap();
    assert!(!fail_out.has_errors, "{:?}", fail_out.diagnostics);

    let fail_output = Command::new(&fail_exe).output().unwrap();
    assert!(!fail_output.status.success(), "{:?}", fail_output);
}

#[test]
fn compile_runs_variadic_parameters_native() {
    let dir = TestDir::new("compile_variadic_parameters_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

sum(seed: int, values: int...) -> int
    var total: int = seed
    for value in values
        total = total + value
    end
    return total
end

count(values: int...) -> int
    var total: int = 0
    for value in values
        total = total + 1
    end
    return total
end

main()
    const parts: list[int] = [6, 7]
    io.print(string(sum(10, 1, 2, 3)))
    io.print(string(sum(1, ..parts, 8)))
    io.print(string(count()))
    io.print(string(count(4, 5)))
    io.print(string(count(..parts)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "variadic_parameters.exe"
    } else {
        "variadic_parameters"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "16\n22\n0\n2\n2\n");
}

#[test]
fn compile_runs_generic_function_monomorphization_native() {
    let dir = TestDir::new("compile_generic_monomorph_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

identity[T](value: T) -> T
    return value
end

pick_second[T](first: T, second: T) -> T
    return second
end

wrap[T](value: T) -> T
    return identity(value)
end

main()
    const answer: int = identity(41) + 1
    const label: string = identity("ok")
    io.print(string(answer))
    io.print(label)
    io.print(string(pick_second(3, 7)))
    io.print(wrap("done"))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "generic_monomorph.exe"
    } else {
        "generic_monomorph"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "42\nok\n7\ndone\n");
}

#[test]
fn compile_runs_managed_generic_trait_and_any_native() {
    let dir = TestDir::new("compile_managed_generic_trait_any_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

trait Labelled
    label(self) -> string
end

struct Tag
    label: string
end

apply Tag use Labelled
    label(self) -> string
        return self.label
    end
end

choose[T](first: T, second: T) -> T
    return second
end

generic_label for T: Labelled (value: T) -> string
    return value.label()
end

any_label(value: any[Labelled]) -> string
    return value.label()
end

same_any(value: any[Labelled]) -> any[Labelled]
    return value
end

main()
    const picked: Tag = choose(Tag { label: "old" }, Tag { label: "new" })
    const boxed: any[Labelled] = picked
    io.print(generic_label(picked))
    io.print(any_label(picked))
    io.print(same_any(boxed).label())
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "managed_generic_trait_any.exe"
    } else {
        "managed_generic_trait_any"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "new\nnew\nnew\n");
}

#[test]
fn compile_runs_transitive_imports_with_generic_traits_native() {
    let dir = TestDir::new("compile_transitive_generic_traits_native");
    dir.write(
        "traits.orl",
        r#"module app.traits

public trait Named
    name(self) -> string
end

public read_name for T: Named (value: T) -> string
    return value.name()
end
"#,
    );
    dir.write(
        "models.orl",
        r#"module app.models

public import app.traits = traits

public struct User
    name: string
end

apply User use traits.Named
    name(self) -> string
        return self.name
    end
end

public make_user(name: string) -> User
    return User { name: name }
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.models = models
import ori.io = io

main()
    const user: models.User = models.make_user("Ada")
    io.print(models.traits.read_name(user))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "transitive_generic_traits.exe"
    } else {
        "transitive_generic_traits"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "Ada\n");
}

#[test]
fn compile_runs_any_trait_dynamic_dispatch_native() {
    let dir = TestDir::new("compile_any_trait_dispatch_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Player
    score: int
end

struct Booster
    score: int
end

trait Scored
    score(self) -> int

    bonus(self) -> int
        return 5
    end
end

apply Player use Scored
    score(self) -> int
        return self.score
    end
end

apply Booster use Scored
    score(self) -> int
        return self.score
    end

    bonus(self) -> int
        return 9
    end
end

add_bonus(item: any[Scored]) -> int
    return item.score() + 5
end

identity(item: any[Scored]) -> any[Scored]
    return item
end

main()
    const player: Player = Player { score: 37 }
    const booster: Booster = Booster { score: 20 }
    const item: any[Scored] = player
    const boosted: any[Scored] = booster
    io.print(string(item.score()))
    io.print(string(add_bonus(player)))
    io.print(string(identity(player).score()))
    io.print(string(player.bonus()))
    io.print(string(item.bonus()))
    io.print(string(boosted.bonus()))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "any_trait_dispatch.exe"
    } else {
        "any_trait_dispatch"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "37\n42\n37\n5\n5\n9\n");
}

#[test]
fn build_c_backend_compiles_any_trait_dynamic_dispatch() {
    let dir = TestDir::new("build_any_trait_dispatch");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Player
    score: int
end

struct Booster
    score: int
end

trait Scored
    score(self) -> int

    bonus(self) -> int
        return 5
    end
end

apply Player use Scored
    score(self) -> int
        return self.score
    end
end

apply Booster use Scored
    score(self) -> int
        return self.score
    end

    bonus(self) -> int
        return 9
    end
end

add_bonus(item: any[Scored]) -> int
    return item.score() + 5
end

identity(item: any[Scored]) -> any[Scored]
    return item
end

main()
    const player: Player = Player { score: 37 }
    const booster: Booster = Booster { score: 20 }
    const item: any[Scored] = player
    const boosted: any[Scored] = booster
    io.print(string(item.score()))
    io.print(string(add_bonus(player)))
    io.print(string(identity(player).score()))
    io.print(string(player.bonus()))
    io.print(string(item.bonus()))
    io.print(string(boosted.bonus()))
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    compile_c_source(&dir, "any_trait_dispatch", &out.c_source);
}

#[test]
fn check_allows_any_trait_equality() {
    let dir = TestDir::new("any_trait_equality");
    dir.write(
        "main.orl",
        r#"module app.main

trait Scored
    score(self) -> int
end

struct Player
    score: int
end

apply Player use Scored
    score(self) -> int
        return self.score
    end
end

main()
    const a: any[Scored] = Player { score: 1 }
    const b: any[Scored] = Player { score: 1 }
    const same: bool = a == b
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    compile_c_source(&dir, "any_trait_equality", &out.c_source);
}

#[test]
fn check_reports_function_value_equality() {
    let dir = TestDir::new("function_value_equality");
    dir.write(
        "main.orl",
        r#"module app.main

struct Handler
    run: func(int) -> int
end

first(x: int) -> int
    return x
end

second(x: int) -> int
    return x + 1
end

main()
    const f: func(int) -> int = (x: int) => x
    const g: func(int) -> int = (x: int) => x + 1
    const closures_equal: bool = f == g
    const functions_equal: bool = first == second
    const a: Handler = Handler { run: f }
    const b: Handler = Handler { run: g }
    const handlers_equal: bool = a == b
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert!(
        codes
            .iter()
            .filter(|code| **code == "type.comparison_not_supported")
            .count()
            >= 2,
        "{:?}",
        out.diagnostics
    );
    assert!(
        codes.contains(&"type.equality_unsupported_field"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn compile_runs_struct_structural_equality_native() {
    let dir = TestDir::new("struct_structural_equality_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Address
    city: string
    zip: int
end

struct User
    id: int
    name: string
    scores: list[int]
    address: Address
end

main()
    const left: User = User { id: 1,
        name: "Ada",
        scores: [10, 20],
        address: Address { city: "Recife", zip: 50000 }, }
    const same: User = User { id: 1,
        name: "Ada",
        scores: [10, 20],
        address: Address { city: "Recife", zip: 50000 }, }
    const different_name: User = User { id: 1,
        name: "Bia",
        scores: [10, 20],
        address: Address { city: "Recife", zip: 50000 }, }
    const different_nested: User = User { id: 1,
        name: "Ada",
        scores: [10, 20],
        address: Address { city: "Olinda", zip: 50000 }, }
    const different_list: User = User { id: 1,
        name: "Ada",
        scores: [10, 21],
        address: Address { city: "Recife", zip: 50000 }, }

    io.print(string(left == same))
    io.print(string(left != different_name))
    io.print(string(left != different_nested))
    io.print(string(left == different_list))
end
"#,
    );

    let stdout = compile_and_run(&dir, "struct_structural_equality");
    assert_eq!(stdout, "true\ntrue\ntrue\nfalse\n");
}

#[test]
fn compile_runs_list_structural_equality_native() {
    let dir = TestDir::new("list_structural_equality_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const left: list[int] = [1, 2, 3]
    const same: list[int] = [1, 2, 3]
    const different_value: list[int] = [1, 2, 4]
    const different_len: list[int] = [1, 2]

    io.print(string(left == same))
    io.print(string(left == different_value))
    io.print(string(left != different_value))
    io.print(string(left != different_len))

    const words: list[string] = ["ori", "lang"]
    const same_words: list[string] = ["ori", "lang"]
    const other_words: list[string] = ["ori", "runtime"]

    io.print(string(words == same_words))
    io.print(string(words != other_words))
end
"#,
    );

    let stdout = compile_and_run(&dir, "list_structural_equality");
    assert_eq!(stdout, "true\nfalse\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn compile_runs_set_map_structural_equality_native() {
    let dir = TestDir::new("set_map_structural_equality_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps
import ori.set = sets

main()
    const left_set: set[int] = sets.new()
    sets.add(left_set, 1)
    sets.add(left_set, 2)

    const same_set: set[int] = sets.new()
    sets.add(same_set, 2)
    sets.add(same_set, 1)

    const different_set: set[int] = sets.new()
    sets.add(different_set, 1)
    sets.add(different_set, 3)

    io.print(string(left_set == same_set))
    io.print(string(left_set != different_set))

    const words: set[string] = sets.new()
    sets.add(words, "red")
    sets.add(words, "blue")

    const same_words: set[string] = sets.new()
    sets.add(same_words, "blue")
    sets.add(same_words, "red")

    const other_words: set[string] = sets.new()
    sets.add(other_words, "red")
    sets.add(other_words, "green")

    io.print(string(words == same_words))
    io.print(string(words == other_words))

    const scores: map[int, int] = maps.new()
    maps.set(scores, 1, 10)
    maps.set(scores, 2, 20)

    const same_scores: map[int, int] = maps.new()
    maps.set(same_scores, 2, 20)
    maps.set(same_scores, 1, 10)

    const changed_scores: map[int, int] = maps.new()
    maps.set(changed_scores, 1, 10)
    maps.set(changed_scores, 2, 99)

    io.print(string(scores == same_scores))
    io.print(string(scores != changed_scores))

    const labels: map[string, int] = maps.new()
    maps.set(labels, "a", 1)
    maps.set(labels, "b", 2)

    const same_labels: map[string, int] = maps.new()
    maps.set(same_labels, "b", 2)
    maps.set(same_labels, "a", 1)

    io.print(string(labels == same_labels))

    const buckets: map[int, list[int]] = maps.new()
    maps.set(buckets, 1, [1, 2])

    const same_buckets: map[int, list[int]] = maps.new()
    maps.set(same_buckets, 1, [1, 2])

    const changed_buckets: map[int, list[int]] = maps.new()
    maps.set(changed_buckets, 1, [1, 3])

    io.print(string(buckets == same_buckets))
    io.print(string(buckets != changed_buckets))
end
"#,
    );

    let stdout = compile_and_run(&dir, "set_map_structural_equality");
    assert_eq!(
        stdout,
        "true\ntrue\ntrue\nfalse\ntrue\ntrue\ntrue\ntrue\ntrue\n"
    );
}

#[test]
fn build_c_backend_list_structural_equality() {
    let dir = TestDir::new("c_backend_list_structural_equality");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const left: list[int] = [1, 2, 3]
    const same: list[int] = [1, 2, 3]
    const different: list[int] = [1, 2, 4]
    const words: list[string] = ["ori", "lang"]
    const same_words: list[string] = ["ori", "lang"]

    const ints_equal: bool = left == same
    const ints_different: bool = left != different
    const words_equal: bool = words == same_words
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("ori_list_at") && out.c_source.contains("ori_string_eq"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_list_structural_equality", &out.c_source);
}

#[test]
fn build_c_backend_struct_structural_equality() {
    let dir = TestDir::new("c_backend_struct_structural_equality");
    dir.write(
        "main.orl",
        r#"module app.main

struct Address
    city: string
    zip: int
end

struct User
    id: int
    name: string
    address: Address
end

main()
    const left: User = User { id: 1, name: "Ada", address: Address { city: "Recife", zip: 50000 } }
    const same: User = User { id: 1, name: "Ada", address: Address { city: "Recife", zip: 50000 } }
    const different: User = User { id: 1, name: "Ada", address: Address { city: "Olinda", zip: 50000 } }

    const users_equal: bool = left == same
    const users_different: bool = left != different
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("ori_string_eq")
            && out.c_source.contains(".address")
            && out.c_source.contains(".city"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_struct_structural_equality", &out.c_source);
}

#[test]
fn build_c_backend_set_map_structural_equality() {
    let dir = TestDir::new("c_backend_set_map_structural_equality");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.map = maps
import ori.set = sets

main()
    const left_set: set[int] = set { 1, 2 }
    const same_set: set[int] = set { 2, 1 }
    const different_set: set[int] = set { 1, 3 }
    const set_same: bool = left_set == same_set
    const set_different: bool = left_set != different_set

    const words: set[string] = set { "red", "blue" }
    const same_words: set[string] = set { "blue", "red" }
    sets.add(same_words, "red")
    const words_same: bool = words == same_words

    const labels: map[string, int] = { "a": 1, "b": 2 }
    const same_labels: map[string, int] = { "b": 2, "a": 1 }
    const labels_same: bool = labels == same_labels

    const buckets: map[int, list[int]] = maps.new()
    maps.set(buckets, 1, [1, 2])

    const same_buckets: map[int, list[int]] = maps.new()
    maps.set(same_buckets, 1, [1, 2])

    const changed_buckets: map[int, list[int]] = maps.new()
    maps.set(changed_buckets, 1, [1, 3])

    const nested_same: bool = buckets == same_buckets
    const nested_different: bool = buckets != changed_buckets
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("ori_map_set_string_value")
            && out.c_source.contains("ori_set_contains_string")
            && out.c_source.contains("ori_map_value_at"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_set_map_structural_equality", &out.c_source);
}

#[test]
fn compile_runs_optional_result_inequality_native() {
    let dir = TestDir::new("optional_result_inequality_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

fail_a() -> result[int, string]
    return err("a")
end

fail_b() -> result[int, string]
    return err("b")
end

main()
    const maybe_one: optional[int] = some(1)
    const maybe_two: optional[int] = some(2)
    const missing: optional[int] = none

    io.print(string(maybe_one != maybe_two))
    io.print(string(maybe_one != missing))
    io.print(string(missing != none))

    const ok_one: result[int, string] = ok(1)
    const ok_two: result[int, string] = ok(2)
    const err_a: result[int, string] = fail_a()
    const err_b: result[int, string] = fail_b()

    io.print(string(ok_one != ok_two))
    io.print(string(ok_one != err_a))
    io.print(string(err_a != err_b))
    io.print(string(err_a != fail_a()))
end
"#,
    );

    let stdout = compile_and_run(&dir, "optional_result_inequality");
    assert_eq!(stdout, "true\ntrue\nfalse\ntrue\ntrue\ntrue\nfalse\n");
}

#[test]
fn check_reports_non_numeric_ordering() {
    let dir = TestDir::new("non_numeric_ordering");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const bool_order: bool = true < false
    const string_order: bool = "a" < "b"
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "type.comparison_not_supported")
            .count(),
        2
    );
}

#[test]
fn build_lowers_operator_overloads_through_core_traits() {
    let dir = TestDir::new("operator_overload_traits");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core

struct Score
    value: int
end

apply Score use core.Addable
    add(self, other: Score) -> Score
        return Score {value: self.value + other.value}
    end
end

apply Score use core.Subtractable
    subtract(self, other: Score) -> Score
        return Score {value: self.value - other.value}
    end
end

apply Score use core.Equatable
    equals(self, other: Score) -> bool
        return self.value == other.value
    end
end

apply Score use core.Comparable
    compare(self, other: Score) -> int
        return self.value - other.value
    end
end

main()
    const left: Score = Score { value: 3 }
    const right: Score = Score { value: 5 }
    const sum: Score = left + right
    const diff: Score = right - left
    const same: bool = left == right
    const different: bool = left != right
    const smaller: bool = left < right
    const at_most: bool = left <= right
    const larger: bool = right > left
    const at_least: bool = right >= left
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out
        .c_source
        .contains("ORI__app_dot_main_dot_Score_dot_Addable_dot_add"));
    assert!(out
        .c_source
        .contains("ORI__app_dot_main_dot_Score_dot_Subtractable_dot_subtract"));
    assert!(out
        .c_source
        .contains("ORI__app_dot_main_dot_Score_dot_Equatable_dot_equals"));
    assert!(out
        .c_source
        .contains("ORI__app_dot_main_dot_Score_dot_Comparable_dot_compare"));
}

#[test]
fn compile_runs_operator_overloads_native() {
    let dir = TestDir::new("operator_overload_traits_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.io = io

struct Score
    value: int
end

apply Score use core.Addable
    add(self, other: Score) -> Score
        return Score {value: self.value + other.value}
    end
end

apply Score use core.Subtractable
    subtract(self, other: Score) -> Score
        return Score {value: self.value - other.value}
    end
end

apply Score use core.Equatable
    equals(self, other: Score) -> bool
        return self.value == other.value
    end
end

apply Score use core.Comparable
    compare(self, other: Score) -> int
        return self.value - other.value
    end
end

main()
    const left: Score = Score { value: 3 }
    const right: Score = Score { value: 5 }
    const sum: Score = left + right
    const diff: Score = right - left
    io.print(string(sum.value))
    io.print(string(diff.value))
    io.print(if left == Score { value: 3 } then "eq" else "bad")
    io.print(if left != right then "ne" else "bad")
    io.print(if left < right then "lt" else "bad")
    io.print(if left <= right then "le" else "bad")
    io.print(if right > left then "gt" else "bad")
    io.print(if right >= left then "ge" else "bad")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "operator_overloads.exe"
    } else {
        "operator_overloads"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "8\n2\neq\nne\nlt\nle\ngt\nge\n"
    );
}

#[test]
fn build_lowers_mul_div_operator_overloads_through_core_traits() {
    let dir = TestDir::new("mul_div_operator_overload_traits");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core

struct Vec2
    x: float
    y: float
end

apply Vec2 use core.Multiplicable
    multiply(self, other: Vec2) -> Vec2
        return Vec2 {x: self.x * other.x, y: self.y * other.y}
    end
end

apply Vec2 use core.Divisible
    divide(self, other: Vec2) -> Vec2
        return Vec2 {x: self.x / other.x, y: self.y / other.y}
    end
end

main()
    const a: Vec2 = Vec2 { x: 2.0, y: 3.0 }
    const b: Vec2 = Vec2 { x: 4.0, y: 6.0 }
    const product: Vec2 = a * b
    const quotient: Vec2 = b / a
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out
        .c_source
        .contains("ORI__app_dot_main_dot_Vec2_dot_Multiplicable_dot_multiply"));
    assert!(out
        .c_source
        .contains("ORI__app_dot_main_dot_Vec2_dot_Divisible_dot_divide"));
}

#[test]
fn compile_runs_mul_div_operator_overloads_native() {
    let dir = TestDir::new("mul_div_operator_overload_traits_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.io = io

struct Value
    x: int
    y: int
end

apply Value use core.Multiplicable
    multiply(self, other: Value) -> Value
        return Value {x: self.x * other.x, y: self.y * other.y}
    end
end

apply Value use core.Divisible
    divide(self, other: Value) -> Value
        return Value {x: self.x / other.x, y: self.y / other.y}
    end
end

main()
    const a: Value = Value { x: 2, y: 3 }
    const b: Value = Value { x: 4, y: 6 }
    const product: Value = a * b
    const quotient: Value = b / a
    io.print(string(product.x))
    io.print(string(product.y))
    io.print(string(quotient.x))
    io.print(string(quotient.y))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "mul_div_overloads.exe"
    } else {
        "mul_div_overloads"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "8\n18\n2\n2\n");
}

#[test]
fn compile_runs_managed_operator_overloads_native() {
    let dir = TestDir::new("managed_operator_overload_traits_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.io = io
import ori.string = strings

struct Label
    text: string
end

apply Label use core.Addable
    add(self, other: Label) -> Label
        return Label {text: self.text + other.text}
    end
end

apply Label use core.Equatable
    equals(self, other: Label) -> bool
        return self.text == other.text
    end
end

apply Label use core.Comparable
    compare(self, other: Label) -> int
        return strings.len(self.text) - strings.len(other.text)
    end
end

main()
    const left: Label = Label { text: "ori" }
    const right: Label = Label { text: "-lang" }
    const joined: Label = left + right
    io.print(joined.text)
    io.print(if joined == Label { text: "ori-lang" } then "eq" else "bad")
    io.print(if left < right then "lt" else "bad")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "managed_operator_overloads.exe"
    } else {
        "managed_operator_overloads"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "ori-lang\neq\nlt\n");
}

#[test]
fn build_c_backend_compiles_string_equality() {
    let dir = TestDir::new("c_string_equality");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const same: bool = "ori" == "ori"
    const different: bool = "ori" != "ora"
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_string_eq"));
    compile_c_source(&dir, "c_string_equality", &out.c_source);
}

/// LANG-2: C/debug compiles string helpers + int conversion builtins.
#[test]
fn build_c_backend_compiles_string_helpers_and_int_to_string() {
    let dir = TestDir::new("c_string_helpers_int");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

main()
    const n: int = 42
    const text: string = string(n)
    const upper: string = str.to_upper("ori")
    const has: bool = str.contains("hello", "ell")
    io.print(text)
    io.print(upper)
    io.print(string(has))
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_string_to_upper"));
    assert!(out.c_source.contains("ori_string_contains"));
    compile_c_source(&dir, "c_string_helpers_int", &out.c_source);
}

/// LANG-2: C/debug compiles convert + eprint + string trim/replace surface.
#[test]
fn build_c_backend_compiles_convert_eprint_and_string_surface() {
    let dir = TestDir::new("c_convert_eprint_string");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

main()
    -- Use L1 convert aliases without loading convert.orl helpers
    -- (param name `default` is a C keyword in generated signatures).
    const fs: string = float_to_string(1.5)
    const bs: string = bool_to_string(true)
    const trimmed: string = str.trim("  hi  ")
    const replaced: string = str.replace("aa", "a", "b")
    const n: int = str.len(trimmed)
    io.eprint(fs)
    io.eprint(bs)
    io.print(trimmed)
    io.print(replaced)
    io.print(string(n))
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_float_to_string"));
    assert!(out.c_source.contains("ori_bool_to_string"));
    assert!(out.c_source.contains("ori_io_eprint"));
    assert!(out.c_source.contains("ori_string_trim"));
    assert!(out.c_source.contains("ori_string_replace"));
    assert!(out.c_source.contains("ori_string_len"));
    compile_c_source(&dir, "c_convert_eprint_string", &out.c_source);
}

/// LANG-2: C/debug compiles format + math together (common debug path).
#[test]
fn build_c_backend_compiles_format_and_math_combo() {
    let dir = TestDir::new("c_format_math_combo");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.format = fmt
import ori.math = math

main()
    const h: string = fmt.hex(255)
    const a: float = math.abs(-3.5)
    const _u: string = h
    const _b: float = a
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    compile_c_source(&dir, "c_format_math_combo", &out.c_source);
}

#[test]
fn check_accepts_lazy_type_and_stdlib_once_force() {
    let dir = TestDir::new("lazy_type_once_force");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.lazy = lz

main()
    const delayed: lazy[int] = lz.once(() => 41)
    const value: int = lz.force(delayed)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(!diagnostic_codes(&out).contains(&"type.lazy_not_implemented"));
}

#[test]
fn build_c_backend_compiles_lazy_once_force() {
    let dir = TestDir::new("c_lazy_once_force");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.lazy = lz

main()
    const delayed: lazy[int] = lz.once(() => 41)
    const first: int = lz.force(delayed)
    const second: int = lz.force(delayed)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_lazy_i64_t"));
    assert!(out.c_source.contains("->forced"));
    compile_c_source(&dir, "c_lazy_once_force", &out.c_source);
}

#[test]
fn build_c_backend_emits_json_parse_extern_without_c_lowering() {
    let dir = TestDir::new("c_backend_json_extern_only");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.json = json

main()
    const parsed: result[json.Value, string] = json.parse("{}")
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(
        !out.has_errors,
        "C backend may emit JSON calls as runtime FFI stubs: {:?}",
        out.diagnostics
    );
    assert!(
        out.c_source.contains("ori_json_parse"),
        "expected C source to reference native JSON runtime symbol"
    );
}

#[test]
fn build_c_backend_reports_unsupported_feature_diagnostic() {
    let dir = TestDir::new("c_backend_unsupported_feature_diagnostic");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.lazy = lz

main()
    const delayed: lazy[void] = lz.once(() => io.print("x"))
    lz.force(delayed)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected C backend feature diagnostic");
    assert!(
        out.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "backend.c_unsupported"),
        "{:?}",
        out.diagnostics
    );
    assert!(out.c_source.is_empty());
}

#[test]
fn compile_runs_native_lazy_once_force_once() {
    let dir = TestDir::new("native_lazy_once_force");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.lazy = lz

var calls: int = 0

compute() -> int
    calls = calls + 1
    return 41
end

main()
    const delayed: lazy[int] = lz.once(() => compute())
    const first: int = lz.force(delayed)
    const second: int = lz.force(delayed)
    io.print(string(first + second + calls))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "native_lazy_once_force.exe"
    } else {
        "native_lazy_once_force"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "83");
}

#[test]
fn compile_runs_using_dispose_on_native_scope_exit() {
    let dir = TestDir::new("compile_using_dispose_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

var disposed: int = 0

trait Disposable
    mut dispose(self)
end

struct Resource
    id: int
end

apply Resource use Disposable
    mut dispose(self)
        disposed = disposed * 10 + self.id
    end
end

use_normal()
    using first: Resource = Resource { id: 1 }
    using second: Resource = Resource { id: 2 }
    io.print("inside")
end

use_return() -> int
    using third: Resource = Resource { id: 3 }
    return 7
end

fail() -> result[int, string]
    return err("fail")
end

use_propagate() -> result[int, string]
    using fourth: Resource = Resource { id: 4 }
    const value: int = try fail()
    return ok(value)
end

use_break()
    loop
        using fifth: Resource = Resource { id: 5 }
        break
    end
end

use_continue()
    var done: bool = false
    loop
        using sixth: Resource = Resource { id: 6 }
        if done
            break
        end
        done = true
        continue
    end
end

main()
    use_normal()
    io.print(string(disposed))

    const value: int = use_return()
    io.print(string(value))
    io.print(string(disposed))

    match use_propagate()
    case ok(ok):
        io.print(string(ok))
    case err(message):
        io.print(message)
    end
    io.print(string(disposed))

    use_break()
    io.print(string(disposed))

    use_continue()
    io.print(string(disposed))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "using_dispose.exe"
    } else {
        "using_dispose"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "inside\n21\n7\n213\nfail\n2134\n21345\n2134566\n"
    );
}

#[test]
fn compile_runs_using_dispose_before_native_check_trap() {
    let dir = TestDir::new("compile_using_dispose_native_trap");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

trait Disposable
    mut dispose(self)
end

struct Resource
    id: int
end

apply Resource use Disposable
    mut dispose(self)
        io.print("disposed")
    end
end

main()
    using resource: Resource = Resource { id: 1 }
    check false
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "using_dispose_trap.exe"
    } else {
        "using_dispose_trap"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(stdout, "disposed\n");
}

#[test]
fn compile_runs_using_dispose_before_native_panic() {
    let dir = TestDir::new("compile_using_dispose_native_panic");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

trait Disposable
    mut dispose(self)
end

struct Resource
    id: int
end

apply Resource use Disposable
    mut dispose(self)
        io.print("disposed")
    end
end

main()
    using resource: Resource = Resource { id: 1 }
    panic("boom")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "using_dispose_panic.exe"
    } else {
        "using_dispose_panic"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stdout, "disposed\n");
    assert!(stderr.contains("ori panic: boom"), "{stderr}");
}

#[test]
fn compile_runs_result_match_and_propagation() {
    let dir = TestDir::new("compile_result_match_propagation");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

parse(flag: bool) -> result[int, string]
    if flag
        return ok(7)
    end
    return err("no value")
end

add_one(flag: bool) -> result[int, string]
    const value: int = try parse(flag)
    return ok(value + 1)
end

main()
    match add_one(true)
    case ok(value):
        io.print(string(value))
    case err(message):
        io.print(message)
    end

    match add_one(false)
    case ok(value):
        io.print(string(value))
    case err(message):
        io.print(message)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "result_match.exe"
    } else {
        "result_match"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "8\nno value\n");
}

#[test]
fn compile_runs_native_composite_values_and_patterns() {
    let dir = TestDir::new("compile_native_composite_values");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct User
    id: int
    name: string
end

enum Status
    Ready
    Done(code: int)
end

make_user() -> User
    return User { id: 10, name: "Ada" }
end

pair() -> tuple[int, string]
    return (4, "ok")
end

status() -> Status
    return Status.Done(code: 9)
end

main()
    const user: User = make_user()
    io.print(string(user.id))
    match user.name
    case "Ada":
        io.print("name")
    case else:
        io.print("bad")
    end

    const item: tuple[int, string] = pair()
    io.print(string(item.0))
    io.print(item.1)
    match item
    case tuple(4, "ok"):
        io.print("tuple")
    case else:
        io.print("bad")
    end

    match status()
    case Done(code):
        io.print(string(code))
    case else:
        io.print("bad")
    end

    io.print(f"literal")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "native_composites.exe"
    } else {
        "native_composites"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "10\nname\n4\nok\ntuple\n9\nliteral\n"
    );
}

#[test]
fn compile_runs_deep_match_with_managed_enum_payload_native() {
    let dir = TestDir::new("compile_deep_match_managed_payload_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct User
    name: string
end

enum Event
    Empty
    Text(value: string)
    Record(user: User)
    Pair(data: tuple[int, string])
end

event(kind: int) -> Event
    if kind == 0
        return Event.Text(value: "ready")
    end
    if kind == 1
        return Event.Record(user: User { name: "Ada" })
    end
    if kind == 2
        return Event.Pair(data: tuple(7, "seven"))
    end
    return Event.Empty
end

main()
    match event(0)
    case Text(value):
        io.print(value)
    case else:
        io.print("bad")
    end

    match event(1)
    case Record(user):
        io.print(user.name)
    case else:
        io.print("bad")
    end

    match event(2)
    case Pair(data: tuple(id, label)):
        io.print(string(id))
        io.print(label)
    case else:
        io.print("bad")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "deep_match_managed_payload.exe"
    } else {
        "deep_match_managed_payload"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "ready\nAda\n7\nseven\n");
}

#[test]
fn compile_runs_native_showcase_example() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("ori-driver crate is inside compiler/crates");
    let source = repo_root.join("examples/native_showcase/main.orl");
    let dir = TestDir::new("compile_native_showcase_example");
    let exe = dir.path(if cfg!(windows) {
        "native_showcase.exe"
    } else {
        "native_showcase"
    });

    let out = run_compile(&source, Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    // `io.print(string(user))` renders via core.Displayable — the
    // "Grace <admin>" line was silently swallowed while closure captures
    // were broken (the old expectation was calibrated to that bug).
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "Grace:admin\nGrace:admin\nGrace <admin>\nboot\nGrace\n7\nseven\ndisposed-1\n"
    );
}

#[test]
fn check_infers_is_expression_as_bool() {
    let dir = TestDir::new("is_expression_bool");
    dir.write(
        "main.orl",
        r#"module app.main

struct User
    id: int
end

main()
    const user: User = User { id: 1 }
    const is_user: bool = user is User
    const is_int: bool = 1 is int
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn build_c_backend_compiles_is_check() {
    let dir = TestDir::new("c_backend_is_check");
    dir.write(
        "main.orl",
        r#"module app.main

trait Shape
    area(self) -> int
end

struct User
    id: int
end

struct Circle
    radius: int
end

apply Circle use Shape
    area(self) -> int
        return self.radius * self.radius
    end
end

describe(s: any[Shape]) -> bool
    return s is Circle
end

main()
    const user: User = User { id: 1 }
    const is_user: bool = user is User
    const is_circle: bool = user is Circle
    const is_int: bool = 1 is int
    const shape: any[Shape] = Circle { radius: 3 }
    const is_shape_circle: bool = describe(shape)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("vtable)[0]"), "{}", out.c_source);
    compile_c_source(&dir, "c_backend_is_check", &out.c_source);
}

#[test]
fn build_c_backend_compiles_propagation() {
    let dir = TestDir::new("c_backend_propagation");
    dir.write(
        "main.orl",
        r#"module app.main

parse(flag: bool) -> result[int, string]
    if flag
        return ok(7)
    end
    return err("no value")
end

maybe(flag: bool) -> optional[int]
    if flag
        return some(3)
    end
    return none
end

add_one(flag: bool) -> result[int, string]
    const value: int = try parse(flag)
    return ok(value + 1)
end

unwrap_optional(flag: bool) -> optional[int]
    const value: int = try maybe(flag)
    return some(value + 1)
end

main()
    const a: result[int, string] = add_one(true)
    const b: optional[int] = unwrap_optional(true)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("return ((ori_result_"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("return ((ori_opt_"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_propagation", &out.c_source);
}

#[test]
fn check_enforces_function_where_clause_at_call_site() {
    let dir = TestDir::new("where_constraint_call");
    dir.write(
        "main.orl",
        r#"module app.main

struct Good
    id: int
end

struct Plain
    id: int
end

trait Marker
    mark(self) -> int
end

apply Good use Marker
    mark(self) -> int
        return self.id
    end
end

require_marker for T: Marker (value: T) -> int
    return 1
end

main()
    const good: Good = Good { id: 1 }
    const plain: Plain = Plain { id: 2 }
    const ok: int = require_marker(good)
    const bad: int = require_marker(plain)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"generic.constraint_not_satisfied"));
}

#[test]
fn check_enforces_negative_function_where_clause_at_call_site() {
    let dir = TestDir::new("negative_where_constraint_call");
    dir.write(
        "main.orl",
        r#"module app.main

struct Plain
    id: int
end

struct Marked
    id: int
end

trait Marker
    mark(self) -> int
end

apply Marked use Marker
    mark(self) -> int
        return self.id
    end
end

reject_marker for T: not Marker (value: T) -> int
    return 1
end

main()
    const plain: Plain = Plain { id: 1 }
    const marked: Marked = Marked { id: 2 }
    const ok: int = reject_marker(plain)
    const bad: int = reject_marker(marked)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"generic.negative_constraint_violated"));
}

#[test]
fn check_reports_circular_generic_instantiation() {
    let dir = TestDir::new("generic_circular_instantiation");
    dir.write(
        "main.orl",
        r#"module app.main

recurse[T](value: T) -> T
    return recurse(value)
end

main()
    const value: int = recurse(1)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"generic.circular_instantiation"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_grouped_where_clause_with_and() {
    let dir = TestDir::new("grouped_where_and");
    dir.write(
        "main.orl",
        r#"module app.main

struct Good
    id: int
end

trait MarkerA
    a(self) -> int
end

trait MarkerB
    b(self) -> int
end

apply Good use MarkerA
    a(self) -> int
        return self.id
    end
end

apply Good use MarkerB
    b(self) -> int
        return self.id
    end
end

require_both for T: MarkerA, T: MarkerB (value: T) -> int
    return 1
end

main()
    const good: Good = Good { id: 1 }
    const ok: int = require_both(good)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_chained_comparison() {
    let dir = TestDir::new("chained_comparison");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const bad: bool = 1 < 2 < 3
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected chained comparison error");
    assert!(diagnostic_codes(&out).contains(&"parse.chained_comparison"));
}

#[test]
fn check_reports_invalid_lvalue_assignment() {
    let dir = TestDir::new("invalid_lvalue_assignment");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    1 = 2
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected invalid lvalue error");
    assert!(diagnostic_codes(&out).contains(&"parse.invalid_lvalue"));
}

#[test]
fn check_reports_variadic_parameter_not_last() {
    let dir = TestDir::new("variadic_not_last");
    dir.write(
        "main.orl",
        r#"module app.main

bad(values: int..., suffix: int) -> int
    return suffix
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected variadic parameter order error");
    assert!(diagnostic_codes(&out).contains(&"parse.variadic_not_last"));
}

#[test]
fn check_reports_required_parameter_after_default() {
    let dir = TestDir::new("default_before_required");
    dir.write(
        "main.orl",
        r#"module app.main

bad(left: int = 1, right: int) -> int
    return left + right
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected default parameter order error");
    assert!(diagnostic_codes(&out).contains(&"parse.default_before_required"));
}

#[test]
fn check_reports_duplicate_struct_fields_and_enum_variants() {
    let dir = TestDir::new("duplicate_fields_variants");
    dir.write(
        "main.orl",
        r#"module app.main

struct Point
    x: int
    x: int
end

enum Status
    Ready
    Ready
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected duplicate declaration errors");
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"bind.duplicate_field"), "{codes:?}");
    assert!(codes.contains(&"bind.duplicate_variant"), "{codes:?}");
}

#[test]
fn check_reports_unknown_names_calls_and_paths() {
    let dir = TestDir::new("unknown_names_calls_paths");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const missing_value: int = missing
    const missing_call: int = missing_func()
    const missing_path: int = unknown.module.value
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected unknown name diagnostics");
    let codes = diagnostic_codes(&out);
    let undefined_count = codes
        .iter()
        .filter(|code| **code == "name.undefined")
        .count();
    assert_eq!(undefined_count, 3, "{codes:?}");
}

#[test]
fn check_reports_self_outside_method() {
    let dir = TestDir::new("self_outside_method");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const value: int = self
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected self usage diagnostic");
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"bind.self_outside_method"), "{codes:?}");
    assert!(
        !codes.contains(&"name.undefined"),
        "`self` should not fall back to name.undefined: {codes:?}"
    );
}

#[test]
fn check_reports_logical_operator_non_bool_operands() {
    let dir = TestDir::new("logical_operator_non_bool_operands");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const bad_and: bool = 1 and true
    const bad_or: bool = false or 2
    const bad_not: bool = not 1
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected bool operand diagnostics");
    let codes = diagnostic_codes(&out);
    let expected_bool_count = codes
        .iter()
        .filter(|code| **code == "type.expected_bool")
        .count();
    assert_eq!(expected_bool_count, 3, "{codes:?}");
}

#[test]
fn check_reports_closure_capture_of_var_binding() {
    let dir = TestDir::new("closure_capture_of_var_binding");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    var counter: int = 0
    const snapshot: func() -> int = () => counter
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected closure capture diagnostic");
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"mut.closure_captures_var"), "{codes:?}");
}

#[test]
fn check_warns_when_result_expression_is_discarded() {
    let dir = TestDir::new("discarded_result_expression");
    dir.write(
        "main.orl",
        r#"module app.main

fail() -> result[int, string]
    return err("fail")
end

main()
    fail()
    const handled: result[int, string] = fail()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    let unused_result_count = codes
        .iter()
        .filter(|code| **code == "type.unused_result")
        .count();
    assert_eq!(unused_result_count, 1, "{codes:?}");
}

#[test]
fn check_treats_panic_todo_and_unreachable_as_never() {
    let dir = TestDir::new("panic_todo_unreachable_never");
    dir.write(
        "main.orl",
        r#"module app.main

die(flag: bool) -> int
    if flag
        return 1
    else
        panic("bad")
    end
end

later() -> int
    todo()
end

impossible() -> int
    unreachable("impossible")
end

choose(flag: bool) -> int
    const value: int = if flag then 1 else panic("bad")
    return value
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&out);
    assert!(!codes.contains(&"type.missing_return"), "{codes:?}");
    assert!(!codes.contains(&"type.if_branch_mismatch"), "{codes:?}");
}

#[test]
fn check_reports_non_exhaustive_bool_match() {
    let dir = TestDir::new("non_exhaustive_bool_match");
    dir.write(
        "main.orl",
        r#"module app.main

main(flag: bool)
    match flag
    case true:
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"match.non_exhaustive"));
}

#[test]
fn check_reports_non_exhaustive_enum_match() {
    let dir = TestDir::new("non_exhaustive_enum_match");
    dir.write(
        "main.orl",
        r#"module app.main

enum Status
    Ready
    Done
end

main(status: Status)
    match status
    case Ready:
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"match.non_exhaustive"));
}

#[test]
fn check_reports_payload_enum_variant_matched_as_unit() {
    let dir = TestDir::new("payload_enum_variant_as_unit");
    dir.write(
        "main.orl",
        r#"module app.main

enum Status
    Ready
    Done(code: int)
end

main(status: Status)
    match status
    case Done:
        return
    case Ready:
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"type.pattern_mismatch"));
    assert!(codes.contains(&"match.non_exhaustive"));
}

#[test]
fn check_validates_payload_enum_variant_fields() {
    let dir = TestDir::new("payload_enum_variant_fields");
    dir.write(
        "main.orl",
        r#"module app.main

enum Status
    Ready
    Done(code: int)
end

main(status: Status)
    match status
    case Done(missing):
        return
    case Ready:
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"type.pattern_mismatch"));
    assert!(codes.contains(&"match.non_exhaustive"));
}

#[test]
fn check_accepts_exhaustive_payload_enum_match() {
    let dir = TestDir::new("payload_enum_exhaustive");
    dir.write(
        "main.orl",
        r#"module app.main

enum Status
    Ready
    Done(code: int)
end

main(status: Status)
    match status
    case Done(code):
        const value: int = code
        return
    case Ready:
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_non_bool_function_parameter_contract() {
    let dir = TestDir::new("param_contract_type");
    dir.write(
        "main.orl",
        r#"module app.main

bounded(value: int if it + 1) -> int
    return value
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.expected_bool"));
}

#[test]
fn check_reports_non_bool_struct_field_contract() {
    let dir = TestDir::new("field_contract_type");
    dir.write(
        "main.orl",
        r#"module app.main

struct Port
    value: int if it + 1
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.expected_bool"));
}

#[test]
fn check_reports_variadic_argument_type_mismatch() {
    let dir = TestDir::new("variadic_arg_type");
    dir.write(
        "main.orl",
        r#"module app.main

sum(values: int...) -> int
    return 0
end

main()
    const bad: int = sum("no")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.arg_type_mismatch"));
}

#[test]
fn check_reports_variadic_spread_type_mismatch() {
    let dir = TestDir::new("variadic_spread_type");
    dir.write(
        "main.orl",
        r#"module app.main

sum(values: int...) -> int
    return 0
end

main()
    const words: list[string] = ["no"]
    const bad: int = sum(..words)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.arg_type_mismatch"));
}

#[test]
fn check_reports_duplicate_parameter_names() {
    let dir = TestDir::new("duplicate_param_names");
    dir.write(
        "main.orl",
        r#"module app.main

add(value: int, value: int) -> int
    return value
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.duplicate_param"));
}

#[test]
fn check_reports_import_after_declaration() {
    let dir = TestDir::new("import_after_declaration");
    dir.write(
        "main.orl",
        r#"module app.main

ready()
end

import ori.io = io
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.import_after_declaration"));
}

#[test]
fn check_reports_missing_module() {
    let dir = TestDir::new("missing_module");
    dir.write(
        "main.orl",
        r#"main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.module_missing"));
}

#[test]
fn check_reports_module_not_first() {
    let dir = TestDir::new("module_not_first");
    dir.write(
        "main.orl",
        r#"module app.main

ready()
end

module app.other
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.module_not_first"));
}

#[test]
fn check_reports_namespace_removed() {
    let dir = TestDir::new("namespace_removed");
    dir.write(
        "main.orl",
        r#"namespace app.main

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.namespace_removed"));
}

#[test]
fn check_reports_func_removed() {
    let dir = TestDir::new("func_removed");
    dir.write(
        "main.orl",
        r#"module app.main

func main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"parse.func_removed"));
}


#[test]
fn check_reports_removed_angle_type() {
    let dir = TestDir::new("removed_angle_type");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const xs: list<int> = [1, 2, 3]
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_angle_type"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_removed_of_type() {
    let dir = TestDir::new("removed_of_type");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const xs: list of int = [1]
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_of_type"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_removed_of_and_angle_in_same_block() {
    // Single-arg `of` recovery must return AST so the block continues and later
    // removed forms still emit diagnostics (Issue 2 from PR3 review).
    let dir = TestDir::new("removed_of_and_angle_same_block");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const a: list of int = [1]
    const b: list<int> = [1]
    const c: map of string to int = {"x": 1}
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"parse.removed_of_type"),
        "expected of-form errors: {:?}",
        out.diagnostics
    );
    assert!(
        codes.contains(&"parse.removed_angle_type"),
        "angle form after of must still be reported: {:?}",
        out.diagnostics
    );
    let of_count = out
        .diagnostics
        .iter()
        .filter(|d| d.code == "parse.removed_of_type")
        .count();
    assert!(
        of_count >= 2,
        "expected both `list of` and `map of` diagnostics, got {of_count}: {:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_removed_where_bound() {
    let dir = TestDir::new("removed_where_bound");
    dir.write(
        "main.orl",
        r#"module app.main

trait Marker
    id(self) -> int
end

show[T](value: T) -> int where T is Marker
    return 1
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.removed_where_bound"),
        "{:?}",
        out.diagnostics
    );
    let where_diag = out
        .diagnostics
        .iter()
        .find(|d| d.code == "parse.removed_where_bound")
        .expect("where diagnostic");
    assert!(
        where_diag.message.contains("`where T is Trait`"),
        "message must name the removed form, not the replacement: {}",
        where_diag.message
    );
    assert!(
        where_diag.message.contains("`for T: Trait`"),
        "message must suggest Auk9-style bounds: {}",
        where_diag.message
    );
    assert!(
        !where_diag
            .message
            .starts_with("`for T: Trait` bounds are removed"),
        "must not claim the new form is removed: {}",
        where_diag.message
    );
}

#[test]
fn check_accepts_bracket_types_and_for_bounds() {
    let dir = TestDir::new("bracket_types_for_bounds");
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

apply User use Labelled
    label(self) -> string
        return self.name
    end
end

struct Pair[A, B]
    left: A
    right: B
end

show for T: Labelled (value: T) -> string
    return value.label()
end

identity[T](value: T) -> T
    return value
end

main()
    const xs: list[int] = [1, 2]
    const m: map[string, int] = {"a": 1}
    const o: optional[int] = some(1)
    const r: result[int, string] = ok(1)
    const u: User = User { name: "Ada" }
    const s: string = show(u)
    const id: int = identity(42)
    io.print(s)
    io.print(string(id))
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        !out.has_errors,
        "bracket types and for-bounds must parse: {:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_question_propagate_removed() {
    let dir = TestDir::new("question_propagate_removed");
    dir.write(
        "main.orl",
        r#"module app.main

produce() -> result[int, string]
    return ok(1)
end

wrapped() -> result[int, string]
    const x: int = produce()?
    return ok(x)
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.question_propagate_removed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_else_if_removed() {
    let dir = TestDir::new("else_if_removed");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    if true
        return
    else if false
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.else_if_removed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_elif_chain() {
    let dir = TestDir::new("elif_chain");
    dir.write(
        "main.orl",
        r#"module app.main

grade(n: int) -> string
    if n > 0
        return "pos"
    elif n < 0
        return "neg"
    else
        return "zero"
    end
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_accepts_multi_elif_chain() {
    let dir = TestDir::new("multi_elif_chain");
    dir.write(
        "main.orl",
        r#"module app.main

letter(score: int) -> string
    if score >= 90
        return "A"
    elif score >= 80
        return "B"
    elif score >= 70
        return "C"
    elif score >= 60
        return "D"
    else
        return "F"
    end
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_case_dot_variant_removed() {
    let dir = TestDir::new("case_dot_variant_removed");
    dir.write(
        "main.orl",
        r#"module app.main

enum Status
    Ready
    Done
end

main(status: Status)
    match status
    case .Ready:
        return
    case Done:
        return
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(
        diagnostic_codes(&out).contains(&"parse.case_dot_variant_removed"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_match_enum_cases_without_leading_dot() {
    let dir = TestDir::new("match_enum_no_dot");
    dir.write(
        "main.orl",
        r#"module app.main

enum Shape
    Point
    Circle(radius: int)
end

label(s: Shape) -> string
    match s
    case Point:
        return "point"
    case Circle(radius):
        return "circle"
    end
    return ""
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_accepts_trait_default_body_starting_with_bare_call() {
    // S3: without `func`, `say("hi")` must be a body statement, not the next method.
    let dir = TestDir::new("trait_default_bare_call");
    dir.write(
        "main.orl",
        r#"module app.main

say(msg: string)
end

trait Greeter
    greet()
        say("hi")
    end
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        !out.has_errors,
        "bare call as first trait default body stmt must parse: {:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_trait_required_empty_methods_and_default_path_call() {
    let dir = TestDir::new("trait_default_path_and_required");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

trait Drawable
    draw()
    area() -> int
    paint()
        io.print("x")
    end
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        !out.has_errors,
        "required empty methods + default path-call body: {:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_type_error_inside_imported_top_level_const() {
    let dir = TestDir::new("imported_const_type_error");
    dir.write(
        "config.orl",
        r#"module app.config

const LIMIT: int = "bad"
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.config = config

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.type_mismatch"));
}

#[test]
fn check_uses_imported_top_level_const_type_at_use_site() {
    let dir = TestDir::new("imported_const_use_type");
    dir.write(
        "config.orl",
        r#"module app.config

public const LIMIT: int = 21
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.config = config

main()
    const value: string = config.LIMIT
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.type_mismatch"));
}

#[test]
fn check_reports_missing_local_import() {
    let dir = TestDir::new("missing_import");
    dir.write(
        "main.orl",
        r#"module app.main

import app.missing

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.import_not_found"));
}

#[test]
fn check_accepts_implemented_stdlib_import_allowlist() {
    let dir = TestDir::new("implemented_stdlib_imports");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.io = io
import ori.fs = fs
import ori.files = files
import ori.string = str
import ori.bytes = bytes_mod
import ori.list = lists
import ori.map = maps
import ori.set = sets
import ori.tree = tree
import ori.hash_table = hash_table
import ori.graph = graph
import ori.math = math
import ori.convert = conv
import ori.iter = iter
import ori.Error = StdError

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "got: {:?}", diagnostic_codes(&out));
}

#[test]
fn check_accepts_core_traits_and_using_core_disposable() {
    let dir = TestDir::new("core_traits_using");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core

struct Resource
    id: int
end

apply Resource use core.Disposable
    mut dispose(self)
    end
end

apply Resource use core.Hashable
end

require_hashable for T: core.Hashable (value: T) -> int
    return 1
end

main()
    using resource: Resource = Resource { id: 1 }
    const ok: int = require_hashable(resource)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "got: {:?}", out.diagnostics);
}

#[test]
fn check_accepts_json_stdlib_import() {
    let dir = TestDir::new("json_stdlib_import");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.json = json

main()
    const parsed: result[json.Value, string] = json.parse("{}")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "got: {:?}", out.diagnostics);
}

#[test]
fn compile_runs_standard_error_type_native() {
    let dir = TestDir::new("compile_standard_error_type_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.Error = StdError
import ori.io = io

main()
    const err: StdError = StdError { code: "E_TEST", message: "failed", cause: "" }
    io.print(err.code)
    io.print(err.message)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "standard_error_type.exe"
    } else {
        "standard_error_type"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "E_TEST\nfailed\n");
}

#[test]
fn build_c_backend_compiles_standard_error_type() {
    let dir = TestDir::new("c_backend_standard_error_type");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.Error = StdError

main()
    const err: StdError = StdError { code: "E_C", message: "compiled", cause: "" }
    const code: string = err.code
    const message: string = err.message
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(out.c_source.contains("struct ori_def_"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_string_t code;"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_string_t message;"),
        "{}",
        out.c_source
    );
    // Verifies that the generated struct includes the string cause field.
    assert!(out.c_source.contains("cause;"), "{}", out.c_source);
    compile_c_source(&dir, "c_backend_standard_error_type", &out.c_source);
}

#[test]
fn compile_runs_test_assert_stdlib_native() {
    let dir = TestDir::new("compile_test_assert_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.test = test

main()
    test.assert(1 + 1 == 2, "math still works")
    test.assert_eq(21 * 2, 42)
    test.assert_ne(21, 42)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "test_assert_stdlib.exe"
    } else {
        "test_assert_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn compile_runs_generic_test_assert_stdlib_native() {
    let dir = TestDir::new("compile_generic_test_assert_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.test = test

main()
    test.assert_eq("ori", "ori")
    test.assert_ne("ori", "lang")
    test.assert_eq(true, true)
    test.assert_ne(true, false)
    test.assert_eq(1.5, 1.5)
    test.assert_ne(1.5, 2.5)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "generic_test_assert_stdlib.exe"
    } else {
        "generic_test_assert_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
}

#[test]
fn test_runner_reports_test_fail_stdlib_failure() {
    let dir = TestDir::new("test_runner_stdlib_fail");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.test = test

@test
test_failure()
    test.fail("intentional")
end
"#,
    );

    let out = run_test(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_eq!(out.results.len(), 1);
    assert!(!out.results[0].passed, "{:#?}", out.results[0].stderr);
    assert!(
        out.results[0].stderr.contains("intentional"),
        "{:#?}",
        out.results[0].stderr
    );
}

#[test]
fn build_c_backend_compiles_test_assert_stdlib() {
    let dir = TestDir::new("c_backend_test_assert_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.test = test

main()
    test.assert(true, "ok")
    test.assert_eq(2 + 2, 4)
    test.assert_ne(2, 4)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_test_assert"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_test_assert_eq"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_test_assert_ne"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_test_assert_stdlib", &out.c_source);
}

#[test]
fn build_c_backend_compiles_generic_test_assert_stdlib() {
    let dir = TestDir::new("c_backend_generic_test_assert_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.test = test

main()
    test.assert_eq("ori", "ori")
    test.assert_ne("ori", "lang")
    test.assert_eq(true, true)
    test.assert_ne(true, false)
    test.assert_eq(1.5, 1.5)
    test.assert_ne(1.5, 2.5)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("ori_test_assert_eq_string"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_test_assert_ne_float"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_generic_test_assert_stdlib", &out.c_source);
}

#[test]
fn compile_runs_iter_stdlib_native() {
    let dir = TestDir::new("compile_iter_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.iter = iter
import ori.list = lists
import ori.map = maps

main()
    const values: list[int] = [1, 2, 3, 4]
    const doubled: list[int] = iter.map(values, (x: int) => x * 2)
    const filtered: list[int] = iter.filter(doubled, (x: int) => x > 4)
    const has_large: bool = iter.any(values, (x: int) => x > 3)
    const all_positive: bool = iter.all(values, (x: int) => x > 0)
    const even_count: int = iter.count_where(values, (x: int) => x % 2 == 0)
    const first_two: list[int] = iter.take(values, 2)
    const after_two: list[int] = iter.skip(values, 2)
    const reversed: list[int] = iter.reverse(values)
    const sum: int = iter.reduce(values, 0, (acc: int, x: int) => acc + x)
    const first_even: optional[int] = iter.find(values, (x: int) => x % 2 == 0)
    const sorted: list[int] = iter.sort([4, 1, 3, 2])
    const sorted_desc: list[int] = iter.sort_by([4, 1, 3, 2], (a: int, b: int) => b - a)
    const unique: list[int] = iter.unique([1, 2, 1, 3, 2])
    const expanded: list[int] = iter.flat_map([1, 2, 3], (x: int) => [x, x * 10])
    const zipped: list[tuple[int, int]] = iter.zip([1, 2, 3], [10, 20])
    const first_pair: tuple[int, int] = lists.get(zipped, 0)
    const second_pair: tuple[int, int] = lists.get(zipped, 1)
    const parts: tuple[list[int], list[int]] = iter.partition(values, (x: int) => x % 2 == 0)
    const evens: list[int] = parts.0
    const odds: list[int] = parts.1
    const grouped: map[int, list[int]] = iter.group_by(values, (x: int) => x % 2)
    const grouped_even: list[int] = maps.get(grouped, 0)
    const grouped_odd: list[int] = maps.get(grouped, 1)
    const nested: list[list[int]] = [[1, 2], [3], [], [4, 5]]
    const flat: list[int] = iter.flatten(nested)
    io.print(string(lists.len(filtered)))
    io.print(string(lists.get(filtered, 0)))
    io.print(string(lists.get(filtered, 1)))
    io.print(string(has_large))
    io.print(string(all_positive))
    io.print(string(even_count))
    io.print(string(lists.get(first_two, 1)))
    io.print(string(lists.get(after_two, 0)))
    io.print(string(lists.get(reversed, 0)))
    io.print(string(sum))
    if some(found) = first_even
    io.print(string(found))
    end
    io.print(string(lists.get(sorted, 0)))
    io.print(string(lists.get(sorted, 3)))
    io.print(string(lists.get(sorted_desc, 0)))
    io.print(string(lists.get(sorted_desc, 3)))
    io.print(string(lists.len(unique)))
    io.print(string(lists.get(unique, 2)))
    io.print(string(lists.len(flat)))
    io.print(string(lists.get(flat, 0)))
    io.print(string(lists.get(flat, 4)))
    io.print(string(lists.len(expanded)))
    io.print(string(lists.get(expanded, 0)))
    io.print(string(lists.get(expanded, 5)))
    io.print(string(lists.len(zipped)))
    io.print(string(first_pair.0))
    io.print(string(first_pair.1))
    io.print(string(second_pair.0))
    io.print(string(second_pair.1))
    io.print(string(lists.len(evens)))
    io.print(string(lists.get(evens, 0)))
    io.print(string(lists.get(evens, 1)))
    io.print(string(lists.len(odds)))
    io.print(string(lists.get(odds, 0)))
    io.print(string(lists.get(odds, 1)))
    io.print(string(lists.len(grouped_even)))
    io.print(string(lists.get(grouped_even, 0)))
    io.print(string(lists.get(grouped_even, 1)))
    io.print(string(lists.len(grouped_odd)))
    io.print(string(lists.get(grouped_odd, 0)))
    io.print(string(lists.get(grouped_odd, 1)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "iter_stdlib.exe"
    } else {
        "iter_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n").trim_end(),
        "2\n6\n8\ntrue\ntrue\n2\n2\n3\n4\n10\n2\n1\n4\n4\n1\n3\n3\n5\n1\n5\n6\n1\n30\n2\n1\n10\n2\n20\n2\n2\n4\n2\n1\n3\n2\n2\n4\n2\n1\n3"
    );
}

#[test]
fn compile_runs_generic_iter_stdlib_native() {
    let dir = TestDir::new("compile_generic_iter_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.iter = iter
import ori.list = lists
import ori.map = maps
import ori.string = strings

main()
    const words: list[string] = ["pear", "fig", "apple", "fig"]
    const lengths: list[int] = iter.map(words, (word: string) => strings.len(word))
    const short: list[string] = iter.filter(words, (word: string) => strings.len(word) < 5)
    const has_apple: bool = iter.any(words, (word: string) => word == "apple")
    const all_named: bool = iter.all(words, (word: string) => strings.len(word) > 0)
    const fig_count: int = iter.count_where(words, (word: string) => word == "fig")
    const first_two: list[string] = iter.take(words, 2)
    const after_two: list[string] = iter.skip(words, 2)
    const reversed: list[string] = iter.reverse(words)
    const total_len: int = iter.reduce(words, 0, (acc: int, word: string) => acc + strings.len(word))
    const found: optional[string] = iter.find(words, (word: string) => word == "apple")
    const expanded: list[string] = iter.flat_map(["x", "y"], (word: string) => [word, word])
    const sorted: list[string] = iter.sort(["pear", "apple", "fig"])
    const sorted_by_len: list[string] = iter.sort_by(["pear", "apple", "fig"], (a: string, b: string) => strings.len(a) - strings.len(b))
    const unique: list[string] = iter.unique(["fig", "fig", "pear"])
    const zipped: list[tuple[string, int]] = iter.zip(["a", "b"], [1, 2])
    const first_pair: tuple[string, int] = lists.get(zipped, 0)
    const parts: tuple[list[string], list[string]] = iter.partition(words, (word: string) => word == "fig")
    const figs: list[string] = parts.0
    const other: list[string] = parts.1
    const grouped: map[string, list[string]] = iter.group_by(words, (word: string) => word)
    const grouped_figs: list[string] = maps.get(grouped, "fig")
    const nested: list[list[string]] = [["a"], ["b", "c"]]
    const flat: list[string] = iter.flatten(nested)
    io.print(string(lists.get(lengths, 0)))
    io.print(string(lists.len(short)))
    io.print(string(has_apple))
    io.print(string(all_named))
    io.print(string(fig_count))
    io.print(lists.get(first_two, 1))
    io.print(lists.get(after_two, 0))
    io.print(lists.get(reversed, 0))
    io.print(string(total_len))
    if some(value) = found
        io.print(value)
    end
    io.print(lists.get(expanded, 3))
    io.print(lists.get(sorted, 0))
    io.print(lists.get(sorted_by_len, 0))
    io.print(string(lists.len(unique)))
    io.print(first_pair.0)
    io.print(string(first_pair.1))
    io.print(string(lists.len(figs)))
    io.print(string(lists.len(other)))
    io.print(string(lists.len(grouped_figs)))
    io.print(lists.get(flat, 2))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "generic_iter_stdlib.exe"
    } else {
        "generic_iter_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n").trim_end(),
        "4\n3\ntrue\ntrue\n2\nfig\napple\nfig\n15\napple\ny\napple\nfig\n2\na\n1\n2\n2\n2\nc"
    );
}

#[test]
fn build_c_backend_compiles_iter_stdlib() {
    let dir = TestDir::new("c_backend_iter_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.iter = iter

main()
    const values: list[int] = [1, 2, 3, 4]
    const doubled: list[int] = iter.map(values, (x: int) => x * 2)
    const filtered: list[int] = iter.filter(doubled, (x: int) => x > 4)
    const has_large: bool = iter.any(values, (x: int) => x > 3)
    const all_positive: bool = iter.all(values, (x: int) => x > 0)
    const even_count: int = iter.count_where(values, (x: int) => x % 2 == 0)
    const first_two: list[int] = iter.take(values, 2)
    const after_two: list[int] = iter.skip(values, 2)
    const reversed: list[int] = iter.reverse(values)
    const sum: int = iter.reduce(values, 0, (acc: int, x: int) => acc + x)
    const first_even: optional[int] = iter.find(values, (x: int) => x % 2 == 0)
    const sorted: list[int] = iter.sort([4, 1, 3, 2])
    const sorted_desc: list[int] = iter.sort_by([4, 1, 3, 2], (a: int, b: int) => b - a)
    const unique: list[int] = iter.unique([1, 2, 1, 3, 2])
    const expanded: list[int] = iter.flat_map([1, 2, 3], (x: int) => [x, x * 10])
    const zipped: list[tuple[int, int]] = iter.zip([1, 2, 3], [10, 20])
    const first_pair: tuple[int, int] = zipped[0]
    const second_pair: tuple[int, int] = zipped[1]
    const first_sum: int = first_pair.0 + first_pair.1
    const second_sum: int = second_pair.0 + second_pair.1
    const parts: tuple[list[int], list[int]] = iter.partition(values, (x: int) => x % 2 == 0)
    const evens: list[int] = parts.0
    const odds: list[int] = parts.1
    const partition_first_even: int = evens[0]
    const partition_first_odd: int = odds[0]
    const grouped: map[int, list[int]] = iter.group_by(values, (x: int) => x % 2)
    const nested: list[list[int]] = [[1, 2], [3], [], [4, 5]]
    const flat: list[int] = iter.flatten(nested)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_list_map"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_list_filter"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_iter_any"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_iter_all"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_iter_count_where"),
        "{}",
        out.c_source
    );
    assert!(out.c_source.contains("ori_iter_take"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_iter_skip"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_iter_reverse"),
        "{}",
        out.c_source
    );
    assert!(out.c_source.contains("ori_iter_reduce"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_iter_find"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_iter_sort"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_iter_sort_by"),
        "{}",
        out.c_source
    );
    assert!(out.c_source.contains("ori_iter_unique"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_iter_flat_map"),
        "{}",
        out.c_source
    );
    assert!(out.c_source.contains("ori_iter_zip"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_iter_partition"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_iter_group_by"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_iter_flatten"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_iter_stdlib", &out.c_source);
}

#[test]
fn compile_runs_format_stdlib_native() {
    let dir = TestDir::new("compile_format_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.format = fmt
import ori.io = io

main()
    io.print(fmt.number(12.345, 2))
    io.print(fmt.percent(0.125, 1))
    io.print(fmt.hex(255))
    io.print(fmt.binary(5))
    io.print(fmt.bytes_size(1536, "binary"))
    io.print(fmt.date(0, "iso"))
    io.print(fmt.datetime(0, "iso", ""))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "format_stdlib.exe"
    } else {
        "format_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n").trim_end(),
        "12.35\n12.5%\nff\n101\n1.5 KiB\n1970-01-01\n1970-01-01T00:00:00Z"
    );
}

#[test]
fn build_c_backend_compiles_format_stdlib() {
    let dir = TestDir::new("c_backend_format_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.format = fmt

main()
    const number: string = fmt.number(12.345, 2)
    const percent: string = fmt.percent(0.125, 1)
    const hexed: string = fmt.hex(255)
    const binary: string = fmt.binary(5)
    const size: string = fmt.bytes_size(1536, "binary")
    const date: string = fmt.date(0, "iso")
    const datetime: string = fmt.datetime(0, "iso", "")
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(
        out.c_source.contains("ori_format_number"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_format_datetime"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_format_stdlib", &out.c_source);
}

#[test]
fn compile_runs_os_stdlib_native() {
    let dir = TestDir::new("compile_os_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lst
import ori.os = os

main()
    const env_value: optional[string] = os.env("ORI_TEST_OS_VALUE")
    if some(value) = env_value
        io.print(value)
    else
        io.print("missing")
    end

    const args: list[string] = os.args()
    io.print(string(lst.len(args)))
    io.print(os.platform())
    io.print(os.arch())
    const pid: int = os.pid()
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "os_stdlib.exe"
    } else {
        "os_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe)
        .arg("alpha")
        .arg("beta")
        .env("ORI_TEST_OS_VALUE", "works")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout
        .replace("\r\n", "\n")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines[0], "works");
    assert_eq!(lines[1], "3");
    assert!(["windows", "linux", "macos", "unknown"].contains(&lines[2].as_str()));
    assert!(!lines[3].is_empty());
}

#[test]
fn build_c_backend_compiles_os_stdlib() {
    let dir = TestDir::new("c_backend_os_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.os = os

stop()
    os.exit(0)
end

main()
    const args: list[string] = os.args()
    const env_value: optional[string] = os.env("PATH")
    const pid: int = os.pid()
    const platform: string = os.platform()
    const arch: string = os.arch()
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(out.c_source.contains("ori_os_args"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_os_env"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_opt_str_t"), "{}", out.c_source);
    assert!(
        out.c_source.contains("int main(int argc, char** argv)"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_os_stdlib", &out.c_source);
}

#[test]
fn compile_runs_random_stdlib_native() {
    let dir = TestDir::new("compile_random_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.random = rng

main()
    const number: int = rng.int(1, 3)
    const ratio: float = rng.float(0.0, 1.0)
    const flag: bool = rng.bool()
    const items: list[int] = [10, 20, 30]
    const picked: optional[int] = rng.choice(items)
    const shuffled: list[int] = rng.shuffle(items)
    io.print(string(number))
    io.print(string(ratio >= 0.0 and ratio <= 1.0))
    io.print(string(flag or not flag))
    if some(value) = picked
        io.print(string(value == 10 or value == 20 or value == 30))
    end
    io.print(string(lists.len(shuffled)))
    io.print(string(lists.contains(shuffled, 10) and lists.contains(shuffled, 20) and lists.contains(shuffled, 30)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "random_stdlib.exe"
    } else {
        "random_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout
        .replace("\r\n", "\n")
        .lines()
        .map(str::to_owned)
        .collect();
    let number = lines[0].parse::<i64>().unwrap();
    assert!((1..=3).contains(&number));
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "true");
    assert_eq!(lines[4], "3");
    assert_eq!(lines[5], "true");
}

#[test]
fn compile_runs_generic_random_choice_and_shuffle_native() {
    let dir = TestDir::new("compile_generic_random_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.random = rng

main()
    const words: list[string] = ["alpha", "beta", "gamma"]
    const picked: optional[string] = rng.choice(words)
    if some(value) = picked
        io.print(string(value == "alpha" or value == "beta" or value == "gamma"))
    end
    const shuffled: list[string] = rng.shuffle(words)
    io.print(string(lists.len(shuffled)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "generic_random_stdlib.exe"
    } else {
        "generic_random_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "true\n3\n");
}

#[test]
fn compile_runs_json_stdlib_native() {
    let dir = TestDir::new("compile_json_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.json = json

main()
    const parsed: result[json.Value, string] = json.parse("{\"name\":\"ori\",\"ok\":true}")
    match parsed
    case ok(value):
        io.print(json.stringify(value))
        io.print(json.stringify_pretty(value))
    case err(message):
        io.print(message)
    end

    const invalid: result[json.Value, string] = json.parse("{")
    match invalid
    case ok(value):
        io.print(json.stringify(value))
    case err(message):
        io.print("invalid")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "json_stdlib.exe"
    } else {
        "json_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "{\"name\":\"ori\",\"ok\":true}\n{\n  \"name\": \"ori\",\n  \"ok\": true\n}\ninvalid\n"
    );
}

#[test]
fn build_c_backend_compiles_random_stdlib() {
    let dir = TestDir::new("c_backend_random_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.random = rng

main()
    const number: int = rng.int(1, 3)
    const ratio: float = rng.float(0.0, 1.0)
    const flag: bool = rng.bool()
    const items: list[int] = [10, 20, 30]
    const picked: optional[int] = rng.choice(items)
    const shuffled: list[int] = rng.shuffle(items)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(out.c_source.contains("ori_random_int"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_random_float"),
        "{}",
        out.c_source
    );
    assert!(out.c_source.contains("ori_random_bool"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_random_choice"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("ori_random_shuffle"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_random_stdlib", &out.c_source);
}

#[test]
fn compile_runs_time_stdlib_native() {
    let dir = TestDir::new("compile_time_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.time = time

main()
    time.sleep(0)
    io.print(string(time.duration_ms(10, 42)))
    io.print(string(time.now() > 0))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "time_stdlib.exe"
    } else {
        "time_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n").trim_end(), "32\ntrue");
}

#[test]
fn build_c_backend_compiles_time_stdlib() {
    let dir = TestDir::new("c_backend_time_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.time = time

main()
    time.sleep(0)
    const now: int = time.now()
    const delta: int = time.duration_ms(10, 42)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(out.c_source.contains("ori_time_now"), "{}", out.c_source);
    assert!(out.c_source.contains("ori_time_sleep"), "{}", out.c_source);
    assert!(
        out.c_source.contains("ori_time_duration_ms"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_time_stdlib", &out.c_source);
}

#[test]
fn compile_runs_mem_stdlib_intrinsics_native() {
    let dir = TestDir::new("compile_mem_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.mem = mem

main()
    const value: int = 41
    const flag: bool = true
    io.print(string(mem.size_of(value)))
    io.print(":")
    io.print(string(mem.align_of(value)))
    io.print(":")
    io.print(string(mem.size_of(flag)))
    io.print(":")
    io.print(string(mem.align_of(flag)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "mem_stdlib.exe"
    } else {
        "mem_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n").trim_end(),
        "8\n:\n8\n:\n1\n:\n1"
    );
}

#[test]
fn build_c_backend_compiles_mem_stdlib_intrinsics() {
    let dir = TestDir::new("c_backend_mem_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.mem = mem

main()
    const value: int = 41
    const size: int = mem.size_of(value)
    const align: int = mem.align_of(value)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(
        !out.c_source.contains("ori_mem_size_of"),
        "{}",
        out.c_source
    );
    assert!(
        !out.c_source.contains("ori_mem_align_of"),
        "{}",
        out.c_source
    );
    compile_c_source(&dir, "c_backend_mem_stdlib", &out.c_source);
}

#[test]
fn check_reports_unknown_stdlib_import() {
    let dir = TestDir::new("unknown_stdlib_import");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.nope = nope

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"bind.stdlib_module_unknown"),
        "got: {:?}",
        codes
    );
}

#[test]
fn check_reports_ambiguous_local_import_path() {
    let dir = TestDir::new("ambiguous_import");
    std::fs::create_dir_all(dir.path("app")).unwrap();
    dir.write(
        "util.orl",
        r#"module app.util

public answer() -> int
    return 1
end
"#,
    );
    dir.write(
        "app/util.orl",
        r#"module app.util

public answer() -> int
    return 2
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.import_ambiguous"));
}

#[test]
fn check_reports_import_namespace_mismatch() {
    let dir = TestDir::new("namespace_mismatch");
    dir.write(
        "util.orl",
        r#"module app.other

answer() -> int
    return 1
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"project.namespace_file_mismatch"));
}

#[test]
fn check_reports_local_import_cycle() {
    let dir = TestDir::new("import_cycle");
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

value() -> int
    return 1
end
"#,
    );

    let out = run_check(&dir.path("a.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"project.circular_import"));
}

#[test]
fn check_reports_duplicate_import_alias() {
    let dir = TestDir::new("dup_alias");
    dir.write(
        "a.orl",
        r#"module app.a

value() -> int
    return 1
end
"#,
    );
    dir.write(
        "b.orl",
        r#"module app.b

value() -> int
    return 2
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.a = m
import app.b = m

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.duplicate_alias"));
}

#[test]
fn check_reports_alias_shadowing_local_definition() {
    let dir = TestDir::new("alias_shadows_local");
    dir.write(
        "util.orl",
        r#"module app.util

helper() -> int
    return 3
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = helper

helper()
end

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.alias_shadows_local"));
}

#[test]
fn check_allows_distinct_aliases_for_same_short_name() {
    let dir = TestDir::new("distinct_aliases");
    dir.write(
        "a.orl",
        r#"module app.a

public value() -> int
    return 1
end
"#,
    );
    dir.write(
        "b.orl",
        r#"module app.b

public value() -> int
    return 2
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.a = a
import app.b = b

main()
    const x: int = a.value()
    const y: int = b.value()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_private_item_import() {
    let dir = TestDir::new("private_import");
    dir.write(
        "util.orl",
        r#"module app.util

secret() -> int
    return 42
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = util

main()
    const value: int = util.secret()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"name.private"));
}

#[test]
fn check_warns_unused_import() {
    let dir = TestDir::new("unused_import");
    dir.write(
        "util.orl",
        r#"module app.util

public helper() -> int
    return 1
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = util

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"bind.unused_import"));
}

#[test]
fn compile_runs_entry_namespace_main_with_imported_call() {
    let dir = TestDir::new("compile_import");
    dir.write(
        "util.orl",
        r#"module app.util

public answer() -> int
    return 13
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = util
import ori.io = io

main()
    io.print(string(util.answer()))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) { "main.exe" } else { "main" });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "13");
}

#[test]
fn compile_loads_transitive_local_imports() {
    let dir = TestDir::new("transitive_imports");
    dir.write(
        "c.orl",
        r#"module app.c

public value() -> int
    return 8
end
"#,
    );
    dir.write(
        "b.orl",
        r#"module app.b

import app.c = c

public value() -> int
    return c.value()
end
"#,
    );
    dir.write(
        "a.orl",
        r#"module app.a

import app.b = b

public value() -> int
    return b.value()
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.a = a
import ori.io = io

main()
    io.print(string(a.value()))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "transitive.exe"
    } else {
        "transitive"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "8");
}

// if some / while some / check

#[test]
fn check_if_some_type_checks() {
    let dir = TestDir::new("ifsome_check");
    dir.write(
        "main.orl",
        r#"module app.main

get_name(flag: bool) -> optional[int]
    if flag
        return some(42)
    end
    return none
end

main()
    const maybe: optional[int] = get_name(true)
    if some(n) = maybe
        const doubled: int = n + n
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_if_some_wrong_type_reports_error() {
    let dir = TestDir::new("ifsome_wrong_type");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const value: int = 5
    if some(n) = value
        const x: int = n
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        out.has_errors,
        "expected type error for if some on non-optional"
    );
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"type.ifsome_not_optional"),
        "got: {:?}",
        codes
    );
}

#[test]
fn check_while_some_type_checks() {
    let dir = TestDir::new("whilesome_check");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    var source: optional[int] = some(3)
    while some(n) = source
        source = none
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_while_some_wrong_type_reports_error() {
    let dir = TestDir::new("whilesome_wrong_type");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    var count: int = 0
    while some(n) = count
        count = 0
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(
        out.has_errors,
        "expected type error for while some on non-optional"
    );
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"type.whilesome_not_optional"),
        "got: {:?}",
        codes
    );
}

#[test]
fn check_check_stmt_type_checks() {
    let dir = TestDir::new("check_stmt");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const x: int = 5
    check x > 0
    check x > 0, "x must be positive"
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_check_stmt_non_bool_reports_error() {
    let dir = TestDir::new("check_stmt_non_bool");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const x: int = 5
    check x
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected type error for check on non-bool");
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"type.expected_bool"), "got: {:?}", codes);
}

#[test]
fn check_using_stmt_type_checks() {
    let dir = TestDir::new("using_stmt");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    using resource: int = 42
    const doubled: int = resource + resource
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected using to require Disposable");
    assert!(diagnostic_codes(&out).contains(&"using.not_disposable"));
}

#[test]
fn check_reports_const_reassignment_and_same_scope_shadowing() {
    let dir = TestDir::new("const_reassignment_shadowing");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const value: int = 1
    value = 2
    const value: int = 3
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"bind.const_reassignment"), "{codes:?}");
    assert!(codes.contains(&"bind.shadowing"), "{codes:?}");
}

#[test]
fn check_reports_missing_return_on_non_void_function() {
    let dir = TestDir::new("missing_return_non_void");
    dir.write(
        "main.orl",
        r#"module app.main

maybe(flag: bool) -> int
    if flag
        return 1
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"type.missing_return"));
}

#[test]
fn check_reports_loop_control_outside_loop() {
    let dir = TestDir::new("loop_control_outside_loop");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    if true
        break
    end
    match true
    case true:
        continue
    case false:
        return
    end
    continue
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "control.loop_required")
            .count(),
        3
    );
}

#[test]
fn check_accepts_loop_control_inside_loops() {
    let dir = TestDir::new("loop_control_inside_loops");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    var i: int = 0
    while i < 3
        i = i + 1
        if i == 1
            continue
        end
        break
    end

    repeat 2 times
        continue
    end

    loop
        break
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_reports_loop_control_inside_closure() {
    let dir = TestDir::new("loop_control_inside_closure");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    loop
        const stop: func() -> void = () -> void
            break
        end
        break
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"control.loop_required"));
}

#[test]
fn check_reports_numeric_literal_overflow() {
    let dir = TestDir::new("numeric_literal_overflow");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const too_big_default: int = 9223372036854775808
    const too_big_u8: u8 = 256u8
    const too_big_i8: int8 = 128i8
    const too_big_f32: float32 = 1.0e999f32
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "type.numeric_literal_out_of_range")
            .count(),
        4
    );
}

#[test]
fn check_reports_numeric_literal_invalid_suffix() {
    let dir = TestDir::new("numeric_literal_invalid_suffix");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const bad_float: float = 3.5f128
    const bad_int: int = 42u128
    const bad_hex: int = 0xFFg
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    let codes = diagnostic_codes(&out);
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "type.numeric_literal_invalid")
            .count(),
        3
    );
}

#[test]
fn check_accepts_numeric_literal_suffixes() {
    let dir = TestDir::new("numeric_literal_suffixes");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const a: u8 = 42u8
    const b: int16 = 42i16
    const c: int32 = 0x2Ai32
    const d: u16 = 0b101010u16
    const e: int64 = 0o52i64
    const f: float32 = 3.5f32
    const g: float = 3.5f64
    const h: u64 = 18446744073709551615u64
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn build_preserves_numeric_literal_suffix_values() {
    let dir = TestDir::new("numeric_literal_suffix_values");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const a: float = 3.5f64
    const b: int = 42i64
    const c: u8 = 0x2Au8
    io.print(string(a))
    io.print(string(b))
    io.print(string(c))
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("double a = 3.5"), "{}", out.c_source);
    assert!(
        out.c_source.contains("int64_t b = INT64_C(42)"),
        "{}",
        out.c_source
    );
    assert!(
        out.c_source.contains("uint8_t c = INT64_C(42)"),
        "{}",
        out.c_source
    );

    let exe = dir.path(if cfg!(windows) {
        "numeric_literal_suffix_values.exe"
    } else {
        "numeric_literal_suffix_values"
    });
    let compile = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!compile.has_errors, "{:?}", compile.diagnostics);
}

#[test]
fn check_reports_using_binding_reassignment() {
    let dir = TestDir::new("using_binding_reassignment");
    dir.write(
        "main.orl",
        r#"module app.main

trait Disposable
    mut dispose(self)
end

struct Resource
    id: int
end

apply Resource use Disposable
    mut dispose(self)
    end
end

main()
    using resource: Resource = Resource { id: 1 }
    resource = Resource { id: 2 }
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"mut.using_binding_mutated"));
}

#[test]
fn check_reports_mut_method_call_on_const_binding() {
    let dir = TestDir::new("mut_method_on_const");
    dir.write(
        "main.orl",
        r#"module app.main

struct Counter
    value: int

    mut increment(self)
        self.value = self.value + 1
    end
end

main()
    const locked: Counter = Counter { value: 0 }
    locked.increment()

    var open: Counter = Counter { value: 0 }
    open.increment()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors);
    assert!(diagnostic_codes(&out).contains(&"mut.const_method_call"));
}

#[test]
fn build_if_some_generates_c() {
    let dir = TestDir::new("ifsome_build");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

get_value(flag: bool) -> optional[int]
    if flag
        return some(7)
    end
    return none
end

main()
    const maybe: optional[int] = get_value(true)
    if some(n) = maybe
        io.print(string(n))
    end
end
"#,
    );
    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        out.c_source.contains("has_value"),
        "expected has_value in generated C: {}",
        out.c_source
    );
}

#[test]
fn build_c_backend_compiles_runtime_abi_values() {
    let dir = TestDir::new("c_backend_runtime_abi");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.string = str

maybe(flag: bool) -> optional[int]
    if flag
        return some(7)
    end
    return none
end

parse(flag: bool) -> result[int, string]
    if flag
        return ok(11)
    end
    return err("no value")
end

main()
    const bool_text: string = string(true)
    const float_text: string = string(2.5)
    const numbers: list[int] = [1, 2, 3]
    const first: int = numbers[0]
    const parsed_int: result[int, string] = str.parse_int("12")
    const parsed_float: result[float, string] = str.parse_float("1.5")
    const maybe_value: optional[int] = maybe(true)
    if some(value) = maybe_value
        const copied: int = value
    end

    match parse(false)
    case ok(value):
        const ok: int = value
    case err(message):
        const err: string = message
    end
end
"#,
    );
    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out
        .c_source
        .contains("typedef struct { bool has_value; int64_t value; } ori_opt_i64_t;"));
    assert!(out.c_source.contains("typedef struct ori_result_i64_str_t"));
    assert!(out.c_source.contains("typedef struct ori_result_f64_str_t"));
    assert!(out.c_source.contains("ori_list_at"));
    assert!(out.c_source.contains("ori_bool_to_string"));
    assert!(out.c_source.contains("ori_float_to_string"));
    assert!(out.c_source.contains("strtoll"));
    assert!(out.c_source.contains("strtod"));
    compile_c_source(&dir, "runtime_abi", &out.c_source);
}

#[test]
fn check_project_manifest_directory_uses_declared_entry() {
    let dir = TestDir::new("project_manifest_entry");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
entry = "src/main.orl"
"#,
    );
    dir.write(
        "src/app/util/mod.orl",
        r#"module app.util

public answer() -> int
    return 42
end
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

import app.util = util

main()
    const value: int = util.answer()
end
"#,
    );

    let out = run_check(&dir.path).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_project_manifest_file_uses_declared_entry() {
    let dir = TestDir::new("project_manifest_file_entry");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
entry = "src/main.orl"
"#,
    );
    dir.write(
        "src/app/model/index.orl",
        r#"module app.model

public struct User
    id: int
end
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

import app.model = model

id(user: model.User) -> int
    return user.id
end

main()
end
"#,
    );

    let out = run_check(&dir.path("ori.proj")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_project_manifest_missing_entry_reports_error() {
    let dir = TestDir::new("project_manifest_missing_entry");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
"#,
    );

    let err = match run_check(&dir.path) {
        Ok(_) => panic!("expected missing entry error"),
        Err(err) => err,
    };
    assert!(err.contains("missing `entry`"), "{err}");
}

#[test]
fn doc_file_includes_oridoc_sidecar() {
    let dir = TestDir::new("oridoc_sidecar_doc_file");
    dir.write(
        "ori.proj",
        r#"manifest = 1
name = "demo"
version = "0.1.0"
kind = "lib"
entry = "src/main.orl"

[source]
root = "src"
root_namespace = "app"

[docs]
paths = ["docs/api"]
mode = "sidecar-first"
require_public = "off"
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

public add(left: int, right: int) -> int
    return left + right
end
"#,
    );
    dir.write(
        "src/main.oridoc",
        r#"oridoc 1

namespace app.main

doc func add
    summary:
        Soma dois valores.
    param left:
        Primeiro valor.
    param right:
        Segundo valor.
    returns:
        Soma dos valores.
end
"#,
    );

    let out = run_doc(&dir.path("ori.proj")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.markdown.contains("## app.main.add"));
    assert!(out.markdown.contains("Soma dois valores."));
    assert!(out.markdown.contains("- `left`: Primeiro valor."));
    assert!(out.markdown.contains("Returns: Soma dos valores."));
}

#[test]
fn doc_check_reports_unknown_oridoc_symbol() {
    let dir = TestDir::new("oridoc_unknown_symbol");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
entry = "src/main.orl"
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

public add(left: int, right: int) -> int
    return left + right
end
"#,
    );
    dir.write(
        "src/main.oridoc",
        r#"oridoc 1

namespace app.main

doc func missing
    summary:
        Esta funcao nao existe.
end
"#,
    );

    let out = run_doc_check(&dir.path("ori.proj")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        doc_diagnostic_codes(&out).contains(&"doc.symbol_not_found"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn doc_check_warns_on_oridoc_param_mismatch() {
    let dir = TestDir::new("oridoc_param_mismatch");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
entry = "src/main.orl"
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

public add(left: int, right: int) -> int
    return left + right
end
"#,
    );
    dir.write(
        "src/main.oridoc",
        r#"oridoc 1

namespace app.main

doc func add
    summary:
        Soma dois valores.
    param wrong:
        Nome incorreto.
    returns:
        Soma dos valores.
end
"#,
    );

    let out = run_doc_check(&dir.path("ori.proj")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        doc_diagnostic_codes(&out).contains(&"doc.param_name_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn doc_check_enforces_public_docs_when_configured() {
    let dir = TestDir::new("oridoc_require_public");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
entry = "src/main.orl"

[docs]
require_public = "error"
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

public add(left: int, right: int) -> int
    return left + right
end
"#,
    );

    let out = run_doc_check(&dir.path("ori.proj")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        doc_diagnostic_codes(&out).contains(&"doc.missing_public"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn oridoc_hover_reads_sidecar_for_local_symbol() {
    let dir = TestDir::new("oridoc_hover");
    let source = r#"module app.main

public add(left: int, right: int) -> int
    return left + right
end
"#;
    dir.write("src/main.orl", source);
    dir.write(
        "src/main.oridoc",
        r#"oridoc 1

namespace app.main

doc func add
    summary:
        Soma dois valores.
    returns:
        Soma dos valores.
end
"#,
    );

    let hover =
        ori_driver::pipeline::oridoc_hover_for_symbol(&dir.path("src/main.orl"), source, "add")
            .expect("sidecar hover");
    assert!(hover.contains("app.main.add"));
    assert!(hover.contains("Soma dois valores."));
    assert!(hover.contains("Returns: Soma dos valores."));
}

#[test]
fn compile_project_tree_with_imported_structs_and_enums() {
    let dir = TestDir::new("project_tree_struct_enum_run");
    dir.write(
        "ori.proj",
        r#"name = "demo"
version = "0.1.0"
entry = "src/main.orl"
"#,
    );
    dir.write(
        "src/app/model/mod.orl",
        r#"module app.model

public struct User
    id: int
    name: string
end

public enum Status
    Ready
    Done(code: int)
end

public stable_code(status: Status) -> int
    return 8
end
"#,
    );
    dir.write(
        "src/main.orl",
        r#"module app.main

import app.model = model
import ori.io = io

main()
    const user: model.User = model.User { id: 34, name: "Ada" }
    const status: model.Status = model.Status.Done(code: 8)
    io.print(string(user.id + model.stable_code(status)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "project_tree.exe"
    } else {
        "project_tree"
    });
    let out = run_compile(&dir.path("ori.proj"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn compile_runs_native_closure_capture_and_higher_order_call() {
    let dir = TestDir::new("native_closure_capture");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

apply_twice(value: int, f: func(int) -> int) -> int
    return f(f(value))
end

double(n: int) -> int
    return n * 2
end

main()
    const offset: int = 3
    const add_offset: func(int) -> int = (x: int) => x + offset
    io.print(string(add_offset(4)))
    io.print(string(apply_twice(5, (x: int) => x * 2)))
    io.print(string(apply_twice(2, double)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "native_closure.exe"
    } else {
        "native_closure"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stdout = stdout.replace("\r\n", "\n");
    assert_eq!(stdout, "7\n20\n8\n");
}

#[test]
fn compile_runs_native_block_closure_with_arc_hooks() {
    let dir = TestDir::new("native_block_closure_arc");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const prefix: string = "value"
    const format: func(int) -> string = (x: int) -> string
        const next: int = x + 1
        return prefix
    end
    io.print(format(9))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "native_block_closure_arc.exe"
    } else {
        "native_block_closure_arc"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stdout = stdout.replace("\r\n", "\n");
    assert_eq!(stdout, "value\n");
}

#[test]
fn build_c_backend_compiles_block_closure_with_arc_edges() {
    let dir = TestDir::new("c_backend_block_closure_arc");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const prefix: string = "value"
    const format: func(int) -> string = (x: int) -> string
        const next: int = x + 1
        return prefix
    end
    const rendered: string = format(9)
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out.c_source.contains("ori_arc_register_edge"));
    assert!(out.c_source.contains("ori_arc_maybe_collect_cycles();"));

    compile_c_source(&dir, "c_backend_block_closure_arc", &out.c_source);
}

#[test]
fn check_type_alias_expands_in_hir_lowering() {
    // A type alias should expand transparently so that the aliased type's
    // codegen properties (e.g. int arithmetic, struct field access) work.
    let dir = TestDir::new("type_alias_expand");
    dir.write(
        "main.orl",
        r#"module app.main

alias Score = int
alias Name = string

struct Player
    name: Name
    score: Score
end

total(a: Score, b: Score) -> Score
    return a + b
end

main()
    const p: Player = Player { name: "Alice", score: 10 }
    const t: Score = total(p.score, 5)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn compile_type_alias_works_end_to_end_native() {
    let dir = TestDir::new("type_alias_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

alias Count = int

increment(n: Count) -> Count
    return n + 1
end

main()
    const value: Count = increment(41)
    io.print(string(value))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "type_alias.exe"
    } else {
        "type_alias"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "42\n");
}

#[test]
fn compile_runs_map_set_literals_native() {
    let dir = TestDir::new("map_set_literals");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.set = sets

main()
    const my_map: map[int, int] = { 10: 100, 20: 200 }
    const my_set: set[int] = set { 10, 20, 30 }
    io.print(string(maps.get(my_map, 20)))
    io.print(if sets.contains(my_set, 30) then "1" else "0")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "map_set_literals.exe"
    } else {
        "map_set_literals"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "200\n1\n");
}

#[test]
fn compile_runs_index_slicing_native() {
    let dir = TestDir::new("index_slicing");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const text: string = "hello world"
    const part: string = text[1..5]
    io.print(part)

    const arr: list[int] = [10, 20, 30, 40, 50]
    const sub: list[int] = arr[2..4]
    io.print(string(sub[0]))
    io.print(string(sub[1]))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "index_slicing.exe"
    } else {
        "index_slicing"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "ello\n30\n40\n");
}

#[test]
fn compile_runs_pipe_operator_native() {
    let dir = TestDir::new("pipe_operator");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Point
    x: int
    y: int
    z: int
end

double(p: Point) -> Point
    return Point { x: p.x * 2, y: p.y * 2, z: p.z * 2 }
end

extract_x(p: Point) -> int
    return p.x
end

main()
    const base: Point = Point { x: 1, y: 2, z: 3 }

    -- pipe operator `|>` allows calling functions like methods
    const answer: int = base |> double |> extract_x
    io.print(string(answer))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "pipe_operator.exe"
    } else {
        "pipe_operator"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "2\n");
}

#[test]
fn compile_runs_field_assignment_and_implicit_self_method_native() {
    let dir = TestDir::new("field_assignment_implicit_self_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Counter
    value: int

    mut increment()
        self.value = self.value + 1
    end
end

main()
    var counter: Counter = Counter { value: 1 }
    counter.value = 2
    counter.increment()
    io.print(string(counter.value))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "field_assignment_implicit_self_native.exe"
    } else {
        "field_assignment_implicit_self_native"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "3\n");
}

#[test]
fn check_reports_anonymous_struct_field_mismatch() {
    let dir = TestDir::new("anon_struct_field_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

struct Vec2
    x: float
    y: float
end

main()
    const bad: Vec2 = { x: 1.0 }
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
fn check_reports_anonymous_struct_without_expected_type() {
    let dir = TestDir::new("anon_struct_no_context");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    { x: 1.0, y: 2.0 }
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"type.anon_struct_type_unknown"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_struct_update_without_braces() {
    let dir = TestDir::new("struct_update_without_braces");
    dir.write(
        "main.orl",
        r#"module app.main

struct Config
    verbose: bool
end

main()
    const a: Config = Config { verbose: false }
    const b: Config = a with
        verbose: true
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"parse.unexpected_token"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_float_range_as_type_error() {
    let dir = TestDir::new("float_range_type_error");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const r: range[int] = 0.0..1.0
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"parse.invalid_range"), "{codes:?}");
    assert!(
        !codes.contains(&"type.type_mismatch"),
        "range endpoint errors should use the dedicated diagnostic: {codes:?}"
    );
}

#[test]
fn check_reports_ok_void_mismatch() {
    let dir = TestDir::new("ok_void_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

make() -> result[int, string]
    return ok()
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
fn check_reports_byte_unicode_escape() {
    let dir = TestDir::new("byte_unicode_escape");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const data: bytes = b"\u{0041}"
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"parse.byte_unicode_escape"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn compile_runs_struct_update_expression_native() {
    let dir = TestDir::new("struct_update_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

struct Point
    x: int
    y: int
    z: int
end

main()
    const base: Point = Point { x: 1, y: 2, z: 3 }
    const moved: Point = base with { y: 20 } end
    const shifted: Point = moved with { x: 7, z: moved.z + 4 } end

    io.print(string(base.x + base.y + base.z))
    io.print(string(moved.x + moved.y + moved.z))
    io.print(string(shifted.x + shifted.y + shifted.z))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "struct_update_native.exe"
    } else {
        "struct_update_native"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "6\n24\n34\n");
}

#[test]
fn build_lowers_struct_update_expression_to_c() {
    let dir = TestDir::new("struct_update_c");
    dir.write(
        "main.orl",
        r#"module app.main

struct Point
    x: int
    y: int
    z: int
end

main()
    const base: Point = Point { x: 1, y: 2, z: 3 }
    const moved: Point = base with { y: 20 } end
    const shifted: Point = moved with { x: 7, z: moved.z + 4 } end
    const total: int = base.x + moved.y + shifted.z
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    compile_c_source(&dir, "struct_update_c", &out.c_source);
}

#[test]
fn compile_is_check_on_any_trait_native() {
    let dir = TestDir::new("is_check_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

trait Shape
    area(self) -> int
end

struct Circle
    radius: int
end

apply Circle use Shape
    area(self) -> int
        return self.radius * self.radius
    end
end

struct Square
    side: int
end

apply Square use Shape
    area(self) -> int
        return self.side * self.side
    end
end

describe(s: any[Shape])
    if s is Circle
        io.print("circle")
    else
        io.print("other")
    end
end

main()
    const c: any[Shape] = Circle { radius: 3 }
    const sq: any[Shape] = Square { side: 4 }
    describe(c)
    describe(sq)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "is_check.exe"
    } else {
        "is_check"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "circle\nother\n");
}

#[test]
fn compile_runs_fs_stdlib_canonical_and_compat_aliases() {
    let dir = TestDir::new("compile_fs_stdlib");
    let input_path = dir.path("input.txt");
    let output_path = dir.path("output.txt");
    let bytes_output_path = dir.path("output.bin");
    std::fs::write(&input_path, "hello fs").unwrap();

    let input = ori_path_literal(&input_path);
    let output = ori_path_literal(&output_path);
    let bytes_output = ori_path_literal(&bytes_output_path);

    dir.write(
        "main.orl",
        &format!(
            r#"module app.main

import ori.bytes = bytes_mod
import ori.fs = fs
import ori.files = files
import ori.io = io

main()
    const input_path: string = "{input}"
    const output_path: string = "{output}"
    const bytes_output_path: string = "{bytes_output}"

    match fs.read_text(input_path)
        case ok(text):
            io.print(text)
        case err(e):
            io.print("read failed: " + e)
    end

    match fs.exists(input_path)
        case ok(exists):
            io.print(if exists then "exists" else "missing")
        case err(_):
            io.print("missing")
    end

    match files.exists(input_path)
        case ok(exists):
            io.print(if exists then "compat" else "no compat")
        case err(_):
            io.print("no compat")
    end

    match fs.read_text(output_path)
        case ok(_):
            io.print("unexpected")
        case err(_):
            io.print("missing ok")
    end

    match fs.write_text(output_path, "new fs")
        case ok(_):
            io.print("wrote")
        case err(e):
            io.print("write failed: " + e)
    end

    match fs.read_all(input_path)
        case ok(text):
            io.print(text + " all")
        case err(e):
            io.print("read_all failed: " + e)
    end

    match fs.read_bytes(input_path)
        case ok(raw):
            io.print(string(bytes_mod.len(raw)))
            match fs.write_bytes(bytes_output_path, raw)
                case ok(_):
                    io.print("bytes wrote")
                case err(e):
                    io.print("bytes write failed: " + e)
            end
        case err(e):
            io.print("bytes read failed: " + e)
    end
end
"#
        ),
    );

    let exe = dir.path(if cfg!(windows) {
        "fs_stdlib.exe"
    } else {
        "fs_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output_run = Command::new(&exe).output().unwrap();
    assert!(output_run.status.success(), "{:?}", output_run);
    let stdout = String::from_utf8(output_run.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "hello fs\nexists\ncompat\nmissing ok\nwrote\nhello fs all\n8\nbytes wrote\n"
    );
    assert_eq!(std::fs::read_to_string(output_path).unwrap(), "new fs");
    assert_eq!(std::fs::read(bytes_output_path).unwrap(), b"hello fs");
}

#[test]
fn compile_runs_fs_bytes_preserve_nul_native() {
    let dir = TestDir::new("compile_fs_bytes_preserve_nul");
    let input_path = dir.path("binary-input.bin");
    let output_path = dir.path("binary-output.bin");
    std::fs::write(&input_path, b"A\0B").unwrap();

    let input = ori_path_literal(&input_path);
    let output = ori_path_literal(&output_path);

    dir.write(
        "main.orl",
        &format!(
            r#"module app.main

import ori.fs = fs
import ori.io = io

main()
    match fs.read_bytes("{input}")
        case ok(raw):
            io.print("len=" + string(raw.len()))
            io.print(raw.to_hex())
            match fs.write_bytes("{output}", raw)
                case ok(_):
                    io.print("wrote")
                case err(e):
                    io.print("write_error=" + e)
            end
        case err(e):
            io.print("read_error=" + e)
    end
end
"#
        ),
    );

    let exe = dir.path(if cfg!(windows) {
        "fs_bytes_preserve_nul.exe"
    } else {
        "fs_bytes_preserve_nul"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output_run = Command::new(&exe).output().unwrap();
    assert!(output_run.status.success(), "{:?}", output_run);
    let stdout = String::from_utf8(output_run.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "len=3\n410042\nwrote\n");
    assert_eq!(std::fs::read(output_path).unwrap(), b"A\0B");
}

#[test]
fn compile_runs_escaped_literals_and_fstrings() {
    let dir = TestDir::new("escaped_literals_fstrings");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    io.print("line\nnext")

    const raw: bytes = b"\x68\x69\x21"
    match raw.decode_utf8()
        case ok(text):
            io.print(text)
        case err(e):
            io.print(e)
    end

    const name: string = "Ori"
    const n: int = 3
    io.print(f"hello {name} {n + 2}")
    io.print(f"brace {{ {name} }}")
    io.print("""
        alpha
        beta
        """)
    io.print(f"""
        multi {name}
        """)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "escaped_literals_fstrings.exe"
    } else {
        "escaped_literals_fstrings"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "line\nnext\nhi!\nhello Ori 5\nbrace { Ori }\nalpha\nbeta\nmulti Ori\n"
    );
}

#[test]
fn compile_runs_triple_string_baseline_and_f_triple_string() {
    let dir = TestDir::new("triple_string_baseline");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const name: string = "Ada"
    io.print("""
        line one
          line two
        """)
    io.print(f"""
        hello {name}
          score {1 + 2}
        """)
end
"#,
    );

    let exe = exe_path(&dir, "triple_string_baseline");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "line one\n  line two\nhello Ada\n  score 3\n"
    );
}

#[test]
fn compile_runs_short_circuit_without_rhs_side_effects() {
    let dir = TestDir::new("short_circuit_side_effect");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

explode() -> bool
    panic("short-circuit failed")
    return true
end

main()
    if false and explode()
        io.print("bad-and")
    end
    if true or explode()
        io.print("ok")
    end
end
"#,
    );

    let exe = exe_path(&dir, "short_circuit_side_effect");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "ok\n");
}

#[test]
fn compile_runtime_panics_on_integer_division_by_zero() {
    let dir = TestDir::new("runtime_int_div_zero");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    var zero: int = 0
    io.print(string(10 / zero))
end
"#,
    );

    let exe = exe_path(&dir, "runtime_int_div_zero");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
}

#[test]
fn compile_runs_float_division_by_zero_as_infinity() {
    let dir = TestDir::new("runtime_float_div_zero");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    var zero: float = 0.0
    io.print(string(10.0 / zero))
end
"#,
    );

    let exe = exe_path(&dir, "runtime_float_div_zero");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.to_ascii_lowercase().contains("inf"), "{stdout}");
}

#[test]
fn check_reports_exception_words_as_plain_missing_names() {
    let dir = TestDir::new("exception_words_missing_names");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    throw("bad")
    catch("bad")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    let codes = diagnostic_codes(&out);
    assert!(
        !codes.iter().any(|code| code.starts_with("parse.")),
        "{:?}",
        out.diagnostics
    );
    assert!(
        codes
            .iter()
            .filter(|code| **code == "name.undefined")
            .count()
            >= 2,
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn compile_runs_prompt_full_pipeline_program() {
    let dir = TestDir::new("prompt_full_pipeline_program");
    dir.write(
        "util.orl",
        r#"module app.util

public seed() -> int
    return 2
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util = util
import ori.io = io
import ori.iter = iter

var disposed: int = 0

struct Resource
    id: int if it > 0
    name: string
end

enum AppError
    Validation(message: string)
end

trait Disposable
    mut dispose(self)
end

trait Named
    name(self) -> string

    kind(self) -> string
        return "resource"
    end
end

apply Resource use Disposable
    mut dispose(self)
        disposed = disposed + self.id
    end
end

apply Resource use Named
    name(self) -> string
        return self.name
    end
end

make_resource(id: int) -> result[Resource, AppError]
    if id > 0
        return ok(Resource {id: id, name: "item-" + string(id)})
    end
    return err(AppError.Validation(message: "bad id"))
end

load() -> result[Resource, AppError]
    const resource: Resource = try make_resource(util.seed())
    return ok(resource)
end

describe for T: Named (item: T) -> string
    return item.name()
end

main()
    match load()
    case ok(resource):
        using cleanup: Resource = resource
        io.print(describe(resource))
        io.print(resource.kind())
        const maybe_name: optional[string] = some(resource.name)
        if some(name) = maybe_name
            io.print(name)
        end

        const doubled: list[int] = iter.map([1, 2, 3], (x: int) => x * util.seed())
        var total: int = 0
        for value, index in doubled
            total = total + value + index
        end
        check total == 15
        io.print(string(total))

        match total
        case n if n >= 15:
            io.print("high")
        case else:
            io.print("low")
        end
    case err(err):
        match err
        case Validation(message):
            io.print(message)
        end
    end

    io.print(string(disposed))
end
"#,
    );

    let exe = exe_path(&dir, "prompt_full_pipeline_program");
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "item-2\nresource\nitem-2\n15\nhigh\n2\n"
    );
}

#[test]
fn check_official_examples() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // M2.layout: each example is a mini-project (`ori.proj` + `main.orl`).
    // Kept in sync with the shipping `examples/` catalog (LANG-DOC
    // consolidated/renamed the earlier hello_world/calculator/
    // logic_and_matching/task_cli variants).
    let examples = [
        "examples/hello",
        "examples/bytes_usage",
        "examples/collections_demo",
        "examples/error_handling",
        "examples/file_organizer",
        "examples/json_validator",
        "examples/language_features",
        "examples/log_analyzer",
        "examples/process_runner",
        "examples/string_toolkit",
    ];

    for example in examples {
        let path = root.join(example);
        assert!(
            path.join("ori.proj").is_file() || path.join("main.orl").is_file(),
            "missing official example project: {example}"
        );
        let entry = if path.join("ori.proj").is_file() {
            path.join("ori.proj")
        } else {
            path.join("main.orl")
        };
        let out = run_check(&entry).unwrap();
        assert!(
            !out.has_errors,
            "{example} should type-check: {:?}",
            out.diagnostics
        );
    }
}

#[test]
fn check_readme_quick_example() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert!(
        !readme.contains("(\\namespace)") && !readme.contains("optional, \\nesult)"),
        "README has broken line splits inside important language terms"
    );
    // S3: modules are introduced with `module`, not `namespace`.
    assert!(
        readme.contains("`module`") || readme.contains("(module)"),
        "README should mention modules (S3)"
    );
    assert!(
        readme.contains("optional") && readme.contains("result"),
        "README should mention optional/result"
    );

    let examples = extract_ori_code_fences(&readme);
    assert!(!examples.is_empty(), "README should include an Ori example");

    for (index, source) in examples.iter().enumerate() {
        let dir = TestDir::new(&format!("readme_quick_example_{index}"));
        dir.write("main.orl", source);

        let check = run_check(&dir.path("main.orl")).unwrap();
        assert!(
            !check.has_errors,
            "README Ori example {index} should type-check: {:?}",
            check.diagnostics
        );

        let build = run_build(&dir.path("main.orl")).unwrap();
        assert!(
            !build.has_errors,
            "README Ori example {index} should build with the C backend: {:?}",
            build.diagnostics
        );
    }
}

fn extract_ori_code_fences(markdown: &str) -> Vec<String> {
    let mut examples = Vec::new();
    let mut current = String::new();
    let mut in_ori = false;

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_ori {
                examples.push(current.trim().to_string());
                current.clear();
                in_ori = false;
            } else {
                in_ori = matches!(trimmed, "```ori" | "```orl");
            }
            continue;
        }

        if in_ori {
            current.push_str(line);
            current.push('\n');
        }
    }

    examples
}

#[test]
fn compile_runs_unicode_identifier_and_contextual_times() {
    let dir = TestDir::new("unicode_identifier_contextual_times");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    -- Literal count: poetic call would treat `n times` as a call.
    const café: string = "ok"

    repeat 2 times
        io.print(café)
    end

    io.print("2")
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "unicode_identifier_contextual_times.exe"
    } else {
        "unicode_identifier_contextual_times"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "ok\nok\n2\n");
}

#[test]
fn check_reports_invalid_escapes_and_fstring_diagnostics() {
    let dir = TestDir::new("invalid_escapes_fstrings");
    let source = r#"module app.main

main()
    const bad_string: string = "\q"
    const bad_bytes: bytes = b"\xG0"
    const unclosed: string = f"hello {name"
    const empty: string = f"{}"
    const unmatched: string = f"hello }"
    const trailing: string = f"{1 2}"
end
"#;
    dir.write("main.orl", source);

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected parser diagnostics");
    let codes = diagnostic_codes(&out);
    assert!(codes.contains(&"parse.invalid_escape"), "{codes:?}");
    assert!(codes.contains(&"parse.fstring_unclosed_expr"), "{codes:?}");
    assert!(codes.contains(&"parse.fstring_empty_expr"), "{codes:?}");
    assert!(
        codes.contains(&"parse.fstring_unmatched_brace"),
        "{codes:?}"
    );
    assert!(
        codes.contains(&"parse.fstring_expr_trailing_tokens"),
        "{codes:?}"
    );
    let trailing = out
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "parse.fstring_expr_trailing_tokens")
        .expect("trailing-token f-string diagnostic should exist");
    let expected_start = source.find("1 2").expect("fixture should contain `1 2`") + 2;
    assert_eq!(
        trailing
            .labels
            .first()
            .map(|label| label.span.start as usize),
        Some(expected_start),
        "{trailing:?}"
    );
}

#[test]
fn compile_runs_bytes_stdlib() {
    let dir = TestDir::new("compile_bytes_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

main()
    const b1: bytes = "hello".to_bytes()
    const b2: bytes = " world".to_bytes()
    const combined: bytes = b1.concat(b2)

    io.print("Length 1: " + string(b1.len()))
    io.print("Combined: " + string(combined.len()))

    match combined.decode_utf8()
        case ok(s):
            io.print("Decoded: " + s)
        case err(e):
            io.print("Failed: " + e)
    end

    const hex: string = b1.to_hex()
    io.print("Hex: " + hex)

    match hex.from_hex()
        case ok(b):
            match b.decode_utf8()
                case ok(s):
                    io.print("FromHex: " + s)
                case err(_):
                    io.print("Err1")
            end
        case err(e):
            io.print("Err2: " + e)
    end

    match str.from_bytes(b1)
        case ok(s):
            io.print("FromBytes: " + s)
        case err(_):
            io.print("ErrBytes")
    end

    match "abc".from_hex()
        case ok(_):
            io.print("BadHex")
        case err(_):
            io.print("HexErr")
    end

    const sliced: bytes = combined.slice(0, 5)
    match sliced.decode_utf8()
        case ok(s):
            io.print("Sliced: " + s)
        case err(_):
            io.print("Err3")
    end

    const first: u8 = b1.get(0)
    io.print("First: " + string(int(first)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "bytes_stdlib.exe"
    } else {
        "bytes_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let expected = "Length 1: 5\nCombined: 11\nDecoded: hello world\nHex: 68656c6c6f\nFromHex: hello\nFromBytes: hello\nHexErr\nSliced: hello\nFirst: 104\n";
    assert_eq!(stdout.replace("\r\n", "\n"), expected);
}

#[test]
fn compile_runs_bytes_preserve_nul_native() {
    let dir = TestDir::new("compile_bytes_preserve_nul");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str

main()
    const raw: bytes = b"\x41\x00\x42"
    io.print("len=" + string(raw.len()))
    io.print(raw.to_hex())

    match raw.decode_utf8()
        case ok(_):
            io.print("decode_unexpected")
        case err(_):
            io.print("decode_nul_error")
    end

    match str.from_bytes(raw)
        case ok(_):
            io.print("from_bytes_unexpected")
        case err(_):
            io.print("from_bytes_nul_error")
    end

    match "410042".from_hex()
        case ok(decoded):
            io.print("decoded_len=" + string(decoded.len()))
            io.print(decoded.to_hex())
        case err(e):
            io.print("error=" + e)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "bytes_preserve_nul.exe"
    } else {
        "bytes_preserve_nul"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let expected = "len=3\n410042\ndecode_nul_error\nfrom_bytes_nul_error\ndecoded_len=3\n410042\n";
    assert_eq!(stdout.replace("\r\n", "\n"), expected);
}

#[test]
fn compile_runs_unicode_string_len_and_slice_native() {
    let dir = TestDir::new("compile_unicode_string_len_slice");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const text: string = "\u{00e1}\u{00e9}"
    io.print("len=" + string(text.len()))
    io.print(text.slice(0, 1))
    io.print("index=" + string(text.index_of("\u{00e9}")))
    io.print("emoji_index=" + string("\u{1f642}x".index_of("x")))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "unicode_string_len_slice.exe"
    } else {
        "unicode_string_len_slice"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let expected = "len=2\n\u{00e1}\nindex=1\nemoji_index=1\n";
    assert_eq!(stdout.replace("\r\n", "\n"), expected);
}

#[test]
fn compile_runtime_panics_on_list_index_out_of_bounds() {
    let dir = TestDir::new("runtime_list_index_oob");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const values: list[int] = [1]
    values[1]
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "runtime_list_oob.exe"
    } else {
        "runtime_list_oob"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ori list index out of bounds"), "{stderr}");
}

#[test]
fn compile_runtime_panics_on_bytes_index_out_of_bounds() {
    let dir = TestDir::new("runtime_bytes_index_oob");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const raw: bytes = "a".to_bytes()
    raw.get(1)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "runtime_bytes_oob.exe"
    } else {
        "runtime_bytes_oob"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ori bytes index out of bounds"), "{stderr}");
}

#[test]
fn compile_runtime_panics_on_negative_repeat_count() {
    let dir = TestDir::new("runtime_negative_repeat");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    repeat -1
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "runtime_negative_repeat.exe"
    } else {
        "runtime_negative_repeat"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
}

#[test]
fn compile_runtime_panics_on_invalid_string_slice_bounds() {
    let dir = TestDir::new("runtime_string_slice_bounds");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.string = str

main()
    str.slice("abc", -1, 1)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "runtime_string_slice_bounds.exe"
    } else {
        "runtime_string_slice_bounds"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ori string slice bounds out of range"),
        "{stderr}"
    );
}

#[test]
fn compile_runtime_panics_on_invalid_list_slice_bounds() {
    let dir = TestDir::new("runtime_list_slice_bounds");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.list = lists

main()
    const values: list[int] = [1]
    lists.slice(values, 0, 2)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "runtime_list_slice_bounds.exe"
    } else {
        "runtime_list_slice_bounds"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ori list slice bounds out of range"),
        "{stderr}"
    );
}

#[test]
fn compile_runtime_panics_on_invalid_bytes_slice_bounds() {
    let dir = TestDir::new("runtime_bytes_slice_bounds");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const raw: bytes = "a".to_bytes()
    raw.slice(0, 2)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "runtime_bytes_slice_bounds.exe"
    } else {
        "runtime_bytes_slice_bounds"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ori bytes slice bounds out of range"),
        "{stderr}"
    );
}

#[test]
fn check_accepts_inert_test_attribute_until_test_runner_lands() {
    let dir = TestDir::new("inert_test_attr");
    dir.write(
        "main.orl",
        r#"module app.main

@test
test_addition()
    check 1 + 1 == 2
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_eq!(diagnostic_codes(&out), Vec::<&'static str>::new());
}

#[test]
fn test_runner_executes_test_attribute_functions() {
    let dir = TestDir::new("test_runner_executes");
    dir.write(
        "main.orl",
        r#"module app.main

add(left: int, right: int) -> int
    return left + right
end

@test
test_addition()
    check add(1, 2) == 3
end

@test
test_second_case()
    check add(2, 2) == 4
end
"#,
    );

    let out = run_test(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_eq!(out.results.len(), 2, "{:?}", out.diagnostics);
    assert!(
        out.results.iter().all(|result| result.passed),
        "{:#?}",
        out.results
            .iter()
            .map(|result| (&result.name, result.passed, &result.stderr))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_runner_filters_by_test_name() {
    let dir = TestDir::new("test_runner_filter");
    dir.write(
        "main.orl",
        r#"module app.main

add(left: int, right: int) -> int
    return left + right
end

@test
test_addition()
    check add(1, 2) == 3
end

@test
test_second_case()
    check add(2, 2) == 4
end
"#,
    );

    let out = run_test_with_options(
        &dir.path("main.orl"),
        TestOptions {
            filter: Some("second".to_string()),
        },
    )
    .unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_eq!(out.discovered, 2);
    assert_eq!(out.selected, 1);
    assert_eq!(out.results.len(), 1);
    assert_eq!(out.results[0].name, "app.main.test_second_case");
    assert!(out.results[0].passed, "{:#?}", out.results[0].stderr);
}

#[test]
fn test_runner_reports_failed_check() {
    let dir = TestDir::new("test_runner_failed_check");
    dir.write(
        "main.orl",
        r#"module app.main

@test
test_failure()
    check 1 == 2
end
"#,
    );

    let out = run_test(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_eq!(out.results.len(), 1);
    assert!(!out.results[0].passed, "{:#?}", out.results[0].stderr);
}

#[test]
fn test_runner_rejects_non_concrete_test_signature() {
    let dir = TestDir::new("test_runner_invalid_signature");
    dir.write(
        "main.orl",
        r#"module app.main

@test
test_with_param(value: int)
    check value == 1
end
"#,
    );

    let out = run_test(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    let codes: Vec<_> = out.diagnostics.iter().map(|diag| diag.code).collect();
    assert!(
        codes.contains(&"attr.invalid_test_signature"),
        "{:?}",
        out.diagnostics
    );
    assert!(out.results.is_empty());
}

#[test]
fn fmt_normalizes_new_block_syntax_indentation() {
    let dir = TestDir::new("fmt_new_syntax");
    dir.write(
        "main.orl",
        r#"module app.main

@test
test_formatting()
check 1 == 1
if 1 == 1
check 2 == 2
elif 2 == 3
check false
else
check true
end
match 1
case 1:
check true
case else:
check false
end
end
"#,
    );

    let out = run_fmt(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_eq!(
        out.formatted,
        r#"module app.main

@test
test_formatting()
    check 1 == 1
    if 1 == 1
        check 2 == 2
    elif 2 == 3
        check false
    else
        check true
    end
    match 1
    case 1:
        check true
    case else:
        check false
    end
end
"#
    );
}

#[test]
fn fmt_preserves_collection_syntax_semantics() {
    let dir = TestDir::new("fmt_collection_syntax");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps
import ori.queue = queue
import ori.set = sets

main()
const values: map[int, int] = {1: 10, 2: 20}
const seen: set[int] = set {1, 2}
const todo: queue.Queue[string] = queue.new()
queue.enqueue(todo, "ready")
if sets.contains(seen, 2)
io.print(string(maps.get(values, 1)))
end
match queue.dequeue(todo)
case some(item):
io.print(item)
case none:
io.print("empty")
end
end
"#,
    );

    let out = run_fmt(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out
        .formatted
        .contains("    const values: map[int, int] = {1: 10, 2: 20}"));
    assert!(out
        .formatted
        .contains("    const seen: set[int] = set {1, 2}"));
    assert!(out.formatted.contains("    match queue.dequeue(todo)"));
    assert!(out.formatted.contains("    case some(item):"));
    assert!(out.formatted.contains("        io.print(item)"));

    dir.write("formatted.orl", &out.formatted);
    let checked = run_check(&dir.path("formatted.orl")).unwrap();
    assert!(!checked.has_errors, "{:?}", checked.diagnostics);
}

#[test]
fn check_reports_deprecated_attribute_use_site_warning() {
    let dir = TestDir::new("deprecated_attr_warning");
    dir.write(
        "main.orl",
        r#"module app.main

@deprecated("use new_api() instead")
old_api() -> int
    return 1
end

main()
    const value: int = old_api()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"attr.deprecated"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_imported_deprecated_attribute_use_site_warning() {
    let dir = TestDir::new("imported_deprecated_attr_warning");
    dir.write(
        "legacy.orl",
        r#"module app.legacy

@deprecated("use app.newer.value instead")
public const value: int = 1
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.legacy = legacy

main()
    const current: int = legacy.value
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"attr.deprecated"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_warns_on_doc_param_tag_name_mismatch() {
    let dir = TestDir::new("doc_param_mismatch");
    dir.write(
        "main.orl",
        r#"module app.main

--|
Adds two numbers.
@param wrong Missing real parameter name.
|--
public add(left: int, right: int) -> int
    return left + right
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"doc.param_name_mismatch"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_unknown_attribute() {
    let dir = TestDir::new("unknown_attr");
    dir.write(
        "main.orl",
        r#"module app.main

@custom_marker
main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"attr.unknown"),
        "{:?}",
        out.diagnostics
    );
}


#[test]
fn compile_lib_c_export_produces_shared_object_on_linux() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let dir = TestDir::new("compile_lib_c_export");
    dir.write(
        "lib.orl",
        r#"module app.lib_export

@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end
"#,
    );
    let out_so = dir.path("libexport.so");
    let out = run_compile_with_options(
        &dir.path("lib.orl"),
        &out_so,
        CompileOptions {
            native_raw: false,
            lib: true,
        },
    )
    .expect("compile --lib");
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(out_so.is_file(), "expected shared library at {}", out_so.display());
}

#[test]
fn check_c_export_requires_public() {
    let dir = TestDir::new("c_export_not_public");
    dir.write(
        "main.orl",
        r#"module app.main

@c_export
add_scores(a: int, b: int) -> int
    return a + b
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"attr.c_export_not_public"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_invalid_attribute_target_and_args() {
    let dir = TestDir::new("invalid_attr_target_args");
    dir.write(
        "main.orl",
        r#"module app.main

@test
struct Suite
    value: int
end

@deprecated(reason: old)
old_api()
end

@inline("always")
hot_path()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    let codes = diagnostic_codes(&out);
    assert!(
        codes.contains(&"attr.invalid_target"),
        "{:?}",
        out.diagnostics
    );
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "attr.invalid_arg")
            .count(),
        2,
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_duplicate_attribute_as_warning() {
    let dir = TestDir::new("duplicate_attr_warning");
    dir.write(
        "main.orl",
        r#"module app.main

@inline
@inline
hot_path()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"attr.duplicate"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn compile_runs_stdlib_source_module_string_utils() {
    let dir = TestDir::new("stdlib_source_string_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = su

main()
    io.print(string(su.is_empty("")))
    io.print(string(su.is_empty("hi")))
    io.print(string(su.blank("   ")))
    io.print(string(su.blank("x")))
    io.print(su.replicate("ab", 3))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_string_utils");
    assert_eq!(stdout, "true\nfalse\ntrue\nfalse\nababab\n");
}

#[test]
fn compile_runs_stdlib_source_module_string_utils_layer2() {
    let dir = TestDir::new("stdlib_source_string_utils_layer2");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = su

main()
    io.print(su.default("", "fb"))
    io.print(su.default("x", "fb"))
    io.print(string(su.equals_ignore_case("Hello", "hello")))
    io.print(string(su.equals_ignore_case("ABC", "abd")))
    io.print(su.center("hi", 6))
    io.print(su.center("hello", 3))
    io.print(string(su.count("ababab", "ab")))
    io.print(string(su.count("aaa", "aa")))
    io.print(string(su.count("hello", "x")))
    io.print(string(su.count("hello", "")))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_string_utils_layer2");
    assert_eq!(stdout, "fb\nx\ntrue\nfalse\n  hi  \nhello\n3\n1\n0\n0\n");
}

#[test]
fn compile_runs_stdlib_source_module_string_utils_layer2_expanded() {
    let dir = TestDir::new("stdlib_source_string_utils_layer2_expanded");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = su

main()
    io.print(su.reverse("abc"))
    io.print(su.capitalize("hello"))
    io.print(su.capitalize(""))
    io.print(su.title("hello world"))
    io.print(su.swap_case("AbC123"))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_string_utils_layer2_expanded");
    assert_eq!(stdout, "cba\nHello\n\nHello World\naBc123\n");
}

#[test]
fn compile_runs_stdlib_source_module_list_utils() {
    let dir = TestDir::new("stdlib_source_list_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lu

main()
    const items: list[string] = ["a", "b", "c"]
    io.print(lu.first_or(items, "missing"))
    io.print(lu.last_or(items, "missing"))
    io.print(lu.get_or(items, 1, "missing"))
    io.print(lu.get_or(items, 9, "missing"))
    const empty: list[string] = []
    io.print(lu.first_or(empty, "empty"))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_list_utils");
    assert_eq!(stdout, "a\nc\nb\nmissing\nempty\n");
}

#[test]
fn compile_runs_stdlib_source_module_convert_utils() {
    let dir = TestDir::new("stdlib_source_convert_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.convert = conv
import ori.convert.utils = cu
import ori.io = io

main()
    io.print(string(cu.parse_int_or("41", 0) + 1))
    io.print(string(cu.parse_int_or("nope", 7)))
    io.print(conv.float_to_string(cu.parse_float_or("3.5", 0.0)))
    io.print(conv.float_to_string(cu.parse_float_or("bad", 1.25)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_convert_utils");
    assert_eq!(stdout, "42\n7\n3.5\n1.25\n");
}

#[test]
fn compile_runs_stdlib_source_module_map_utils() {
    let dir = TestDir::new("stdlib_source_map_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = mu

main()
    const scores: map[string, int] = { "a": 1, "b": 2 }
    io.print(string(mu.get_or(scores, "a", 0)))
    io.print(string(mu.get_or(scores, "z", 9)))
    io.print(string(mu.contains_key(scores, "b")))
    io.print(string(mu.contains_key(scores, "z")))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_map_utils");
    assert_eq!(stdout, "1\n9\ntrue\nfalse\n");
}

#[test]
fn compile_runs_stdlib_source_module_set_utils() {
    let dir = TestDir::new("stdlib_source_set_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.set.utils = su

main()
    const tags: set[string] = su.from_list(["a", "b", "a"])
    io.print(string(su.contains_all(tags, ["a", "b"])))
    io.print(string(su.contains_all(tags, ["a", "c"])))
    const subset: set[string] = su.from_list(["a"])
    io.print(string(su.is_subset(subset, tags)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_set_utils");
    assert_eq!(stdout, "true\nfalse\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_bytes_utils() {
    let dir = TestDir::new("stdlib_source_bytes_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.bytes = bytes_mod
import ori.bytes.utils = bu
import ori.io = io
import ori.string = str

main()
    const a: bytes = str.to_bytes("ab")
    const b: bytes = str.to_bytes("ab")
    const c: bytes = str.to_bytes("ac")
    io.print(string(bu.is_empty(bu.empty_bytes())))
    io.print(string(bu.equals(a, b)))
    io.print(string(bu.equals(a, c)))
    const fb: bytes = bu.empty_bytes()
    io.print(string(bytes_mod.len(bu.from_hex_or("deadbeef", fb))))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_bytes_utils");
    assert_eq!(stdout, "true\ntrue\nfalse\n4\n");
}

#[test]
fn compile_runs_stdlib_source_module_math_utils() {
    let dir = TestDir::new("stdlib_source_math_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.math.utils = mu

main()
    io.print(string(mu.sign(-3)))
    io.print(string(mu.sign(0)))
    io.print(string(mu.sign(5)))
    io.print(string(mu.clamp_int(10, 0, 7)))
    io.print(string(mu.approx_eq(1.0, 1.0000001, 0.001)))
    io.print(string(mu.approx_eq(1.0, 2.0, 0.001)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_math_utils");
    assert_eq!(stdout, "-1\n0\n1\n7\ntrue\nfalse\n");
}

#[test]
fn compile_runs_stdlib_source_module_string_utils_layer2_full() {
    let dir = TestDir::new("stdlib_source_string_utils_layer2_full");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.string = su

main()
    io.print(su.left("hello", 2))
    io.print(su.right("hello", 3))
    io.print(su.trim_all("  a   b  c  "))
    const parts: list[string] = su.words("one two  three")
    io.print(string(lists.len(parts)))
    const rows: list[string] = su.lines("a\nb")
    io.print(string(lists.len(rows)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_string_utils_layer2_full");
    assert_eq!(stdout, "he\nllo\na b c\n3\n2\n");
}

#[test]
fn compile_runs_stdlib_source_module_list_utils_expanded() {
    let dir = TestDir::new("stdlib_source_list_utils_expanded");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lu

main()
    const one: list[int] = lu.singleton(42)
    io.print(string(lu.first_or(one, 0)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_list_utils_expanded");
    assert_eq!(stdout, "42\n");
}

#[test]
fn compile_runs_stdlib_source_module_convert_utils_expanded() {
    let dir = TestDir::new("stdlib_source_convert_utils_expanded");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.convert.utils = cu
import ori.io = io

main()
    io.print(string(cu.parse_bool_or("true", false)))
    io.print(string(cu.parse_bool_or("nope", true)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_convert_utils_expanded");
    assert_eq!(stdout, "true\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_list_algorithms() {
    let dir = TestDir::new("stdlib_source_list_algorithms");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = la

main()
    const nums: list[int] = [1, 2, 3, 4]
    io.print(string(la.sum_int(nums)))
    const sorted: list[int] = [1, 3, 5, 7, 9]
    io.print(string(la.binary_search_int(sorted, 5)))
    io.print(string(la.binary_search_int(sorted, 4)))
    io.print(string(la.all_equal_int([2, 2, 2], 2)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_list_algorithms");
    assert_eq!(stdout, "10\n2\n-1\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_tree_algorithms() {
    let dir = TestDir::new("stdlib_source_tree_algorithms");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.tree = tree
import ori.tree.algorithms = ta

main()
    const outline: tree.Tree[string] = tree.new("root")
    const root: tree.NodeId = tree.root(outline)
    const left: tree.NodeId = tree.add_child(outline, root, "left")
    tree.add_child(outline, root, "right")
    tree.add_child(outline, left, "leaf")
    io.print(string(ta.leaf_count(outline)))
    io.print(string(ta.is_leaf(outline, left)))
    io.print(string(ta.is_leaf(outline, root)))
    const values: list[string] = ta.values_preorder(outline)
    io.print(string(lists.len(values)))
    io.print(string(ta.max_depth_from(outline, root)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_tree_algorithms");
    assert_eq!(stdout, "2\nfalse\nfalse\n4\n2\n");
}

#[test]
fn compile_runs_stdlib_source_module_graph_algorithms() {
    let dir = TestDir::new("stdlib_source_graph_algorithms");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.graph = graph
import ori.graph.algorithms = ga
import ori.io = io

main()
    const g: graph.Graph[string] = graph.new(false)
    graph.add_edge(g, "a", "b")
    graph.add_edge(g, "b", "c")
    io.print(string(ga.has_path(g, "a", "c")))
    io.print(string(ga.has_path(g, "c", "a")))
    io.print(string(ga.reachable_count(g, "a")))
    io.print(string(ga.is_reachable(g, "b", "c")))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_graph_algorithms");
    assert_eq!(stdout, "true\ntrue\n3\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_validate() {
    let dir = TestDir::new("stdlib_source_validate");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.validate = validate

main()
    io.print(string(validate.between(5, 1, 10)))
    io.print(string(validate.positive(0)))
    io.print(string(validate.positive(3)))
    io.print(string(validate.not_empty("x")))
    io.print(string(validate.length_between("abc", 2, 4)))
    io.print(string(validate.one_of(2, [1, 2, 3])))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_validate");
    assert_eq!(stdout, "true\nfalse\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_path() {
    let dir = TestDir::new("stdlib_source_path");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.path = path

main()
    io.print(path.base_name("foo/bar/baz.txt"))
    io.print(path.extension("foo/bar/baz.txt"))
    io.print(path.name_without_extension("baz.txt"))
    io.print(string(path.is_absolute("/tmp/x")))
    io.print(string(path.is_relative("tmp/x")))
    io.print(path.join(["a", "b", "c"]))
    io.print(path.change_extension("dir/file.orl", "txt"))
    io.print(path.relative("a/b/c", "a/b"))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_path");
    assert_eq!(
        stdout,
        "baz.txt\ntxt\nbaz\ntrue\ntrue\na/b/c\ndir/file.txt\nc\n"
    );
}

#[test]
fn compile_runs_stdlib_source_module_path_edge_cases() {
    let dir = TestDir::new("stdlib_source_path_edge");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.path = path

main()
    io.print(path.relative("a/b", "a/b/c"))
    io.print(path.relative("a/b", "a/b"))
    io.print(path.relative("x", "y"))
    io.print(path.relative("a/b/c/d", "a/b"))
    io.print(path.relative("a/b", "a/d/e"))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_path_edge");
    assert_eq!(stdout, "..\n.\n../x\nc/d\n../../b\n");
}

#[test]
fn compile_runs_stdlib_source_module_json_utils() {
    let dir = TestDir::new("stdlib_source_json_utils");
    let json_path = ori_path_literal(&dir.path("data.json"));
    dir.write(
        "main.orl",
        &format!(
            r#"module app.main

import ori.io = io
import ori.json = json
import ori.json.utils = ju

main()
    const path: string = "{json_path}"
    match json.parse("{{}}")
        case ok(doc):
            match ju.write(path, doc)
                case ok(_):
                    match ju.read(path)
                        case ok(_):
                            io.print("true")
                        case err(_):
                            io.print("false")
                    end
                case err(_):
                    io.print("false")
            end
        case err(_):
            io.print("false")
    end
end
"#
        ),
    );

    let stdout = compile_and_run(&dir, "stdlib_source_json_utils");
    assert_eq!(stdout, "true\n");
}

#[test]
fn compile_runs_stdlib_source_module_io_and_time_utils() {
    let dir = TestDir::new("stdlib_source_io_time_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.io.utils = iu
import ori.time.utils = tu

main()
    iu.print_line("line")
    io.print(string(tu.seconds(2)))
    io.print(string(tu.minutes(1)))
    io.print(string(tu.hours(1)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_io_time_utils");
    assert_eq!(stdout, "line\n2000\n60000\n3600000\n");
}

#[test]
fn compile_runs_stdlib_source_module_fs_utils() {
    let dir = TestDir::new("stdlib_source_fs_utils");
    let data_path = ori_path_literal(&dir.path("nested/data.txt"));
    let dir_path = ori_path_literal(&dir.path("nested"));
    dir.write(
        "main.orl",
        &format!(
            r#"module app.main

import ori.fs.utils = fu
import ori.io = io

main()
    match fu.create_dir_all("{dir_path}")
        case ok(_):
            match fu.write_text_result("{data_path}", "payload")
                case ok(_):
                    io.print(fu.read_text_or("{data_path}", "missing"))
                    match fu.exists_result("{data_path}")
                        case ok(exists):
                            io.print(string(exists))
                        case err(_):
                            io.print("false")
                    end
                case err(_):
                    io.print("fail")
            end
        case err(_):
            io.print("fail")
    end
end
"#
        ),
    );

    let stdout = compile_and_run(&dir, "stdlib_source_fs_utils");
    assert_eq!(stdout, "payload\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_string_utils_gap_parity() {
    let dir = TestDir::new("stdlib_source_string_utils_gap");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = su

main()
    io.print(string(su.last_index_of("abab", "ab")))
    io.print(string(su.is_digits("123")))
    io.print(string(su.is_digits("12a")))
    io.print(string(su.has_whitespace("a b")))
    io.print(su.limit("hello", 3))
    io.print(su.replace_all("a-a", "a", "b"))
    io.print(string(su.has_prefix("abc", "ab")))
    io.print(string(su.has_suffix("abc", "bc")))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_string_utils_gap");
    assert_eq!(stdout, "2\ntrue\nfalse\ntrue\nhel\nb-b\ntrue\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_bytes_utils_gap_parity() {
    let dir = TestDir::new("stdlib_source_bytes_utils_gap");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.bytes = bytes_mod
import ori.bytes.utils = bu
import ori.io = io
import ori.list = lists
import ori.string = str

main()
    const payload: bytes = str.to_bytes("hello")
    const prefix: bytes = str.to_bytes("he")
    const suffix: bytes = str.to_bytes("lo")
    const part: bytes = str.to_bytes("ell")
    io.print(string(bu.starts_with(payload, prefix)))
    io.print(string(bu.ends_with(payload, suffix)))
    io.print(string(bu.contains(payload, part)))
    const values: list[int] = bu.to_list(payload)
    io.print(string(lists.len(values)))
    const packed: bytes = bu.from_list([65, 66])
    io.print(string(bytes_mod.len(packed)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_bytes_utils_gap");
    assert_eq!(stdout, "true\ntrue\ntrue\n5\n2\n");
}

#[test]
fn compile_runs_stdlib_source_module_math_utils_gap_parity() {
    let dir = TestDir::new("stdlib_source_math_utils_gap");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.math.utils = mu

main()
    io.print(string(mu.deg_to_rad(180.0) > 3.0))
    io.print(string(mu.rad_to_deg(3.14159) > 179.0))
    io.print(string(mu.trunc_float(3.9) == 3.0))
    io.print(string(mu.log10(100.0) == 2.0))
    io.print(string(mu.abs_float(-2.5) == 2.5))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_math_utils_gap");
    assert_eq!(stdout, "true\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn compile_runs_stdlib_source_module_map_utils_gap_parity() {
    let dir = TestDir::new("stdlib_source_map_utils_gap");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = mu

main()
    const empty: map[string, int] = {}
    io.print(string(mu.is_empty_int(empty)))
    const data: map[string, int] = { "x": 1 }
    io.print(string(mu.has_key(data, "x")))
    io.print(string(mu.has_key(data, "y")))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_source_map_utils_gap");
    assert_eq!(stdout, "true\ntrue\nfalse\n");
}

#[test]
fn compile_runs_stdlib_layer1_os_current_dir_and_lazy_is_consumed() {
    let dir = TestDir::new("stdlib_layer1_os_lazy");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.lazy = lz
import ori.os = os_mod
import ori.string = string_mod

main()
    match os_mod.current_dir()
        case ok(cwd):
            io.print(string(string_mod.len(cwd) > 0))
        case err(_):
            io.print("false")
    end
    const delayed: lazy[int] = lz.once(() => 7)
    io.print(string(lz.is_consumed(delayed)))
    const value: int = lz.force(delayed)
    io.print(string(value))
    io.print(string(lz.is_consumed(delayed)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_layer1_os_lazy");
    assert_eq!(stdout, "true\nfalse\n7\ntrue\n");
}

#[test]
fn compile_runs_stdlib_layer1_math_extensions() {
    let dir = TestDir::new("stdlib_layer1_math_ext");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.math = math

main()
    io.print(string(math.trunc(3.9) == 3.0))
    io.print(string(math.log10(1000.0) == 3.0))
    io.print(string(math.is_finite(1.0)))
    io.print(string(math.is_finite(0.0 / 0.0)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_layer1_math_ext");
    assert_eq!(stdout, "true\ntrue\ntrue\nfalse\n");
}

#[test]
fn compile_runs_stdlib_layer1_process_utils() {
    let dir = TestDir::new("stdlib_layer1_process_utils");
    #[cfg(windows)]
    let source = r#"module app.main

import ori.bytes = bytes_mod
import ori.io = io
import ori.process = proc
import ori.process.utils = pu
import ori.string = string_mod

main()
    var c_flag: string = "c"
    match string_mod.from_bytes(bytes_mod.from_list([47, 99]))
        case ok(flag):
            c_flag = flag
        case err(_):
            c_flag = "c"
    end
    match proc.run_capture("cmd", [c_flag, "echo", "hi"])
        case ok(capture):
            io.print(pu.stdout(capture))
            io.print(string(pu.exit_code(capture) == 0))
        case err(_):
            io.print("fail")
    end
end
"#;
    #[cfg(not(windows))]
    let source = r#"module app.main

import ori.io = io
import ori.process = proc
import ori.process.utils = pu

main()
    match proc.run_capture("echo", ["hi"])
        case ok(capture):
            io.print(pu.stdout(capture))
            io.print(string(pu.exit_code(capture) == 0))
        case err(_):
            io.print("fail")
    end
end
"#;
    dir.write("main.orl", source);

    let stdout = compile_and_run(&dir, "stdlib_layer1_process_utils");
    assert!(stdout.contains("hi"), "stdout: {stdout}");
    assert!(stdout.contains("true"), "stdout: {stdout}");
}

#[test]
fn check_accepts_stdlib_gap_parity_imports() {
    let dir = TestDir::new("stdlib_gap_parity_imports");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.validate = validate
import ori.path = path
import ori.json.utils = json_utils
import ori.io.utils = io_utils
import ori.fs.utils = fs_utils
import ori.time.utils = time_utils
import ori.test.utils = test_utils
import ori.process.utils = process_utils
import ori.concurrent.utils = concurrent_utils
import ori.format.utils = format_utils
import ori.iter.utils = iter_utils
import ori.net.utils = net_utils
import ori.os.utils = os_utils
import ori.random.utils = random_utils
import ori.queue.utils = queue_utils
import ori.stack.utils = stack_utils
import ori.deque.utils = deque_utils
import ori.heap.utils = heap_utils
import ori.hash_table.utils = hash_table_utils
import ori.linked_list.utils = linked_list_utils
import ori.doubly_linked_list.utils = doubly_linked_list_utils
import ori.map = map_algorithms
import ori.set.algorithms = set_algorithms
import ori.string = string_algorithms
import ori.bytes.algorithms = bytes_algorithms
import ori.math.algorithms = math_algorithms

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_stdlib_source_module_unknown_still_reports_error() {
    let dir = TestDir::new("stdlib_source_unknown");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.string.nonexistent = sn
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"bind.stdlib_module_unknown"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn compile_runs_stdlib_layer2_remaining_utils() {
    let dir = TestDir::new("stdlib_layer2_remaining");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.deque = deque
import ori.deque.utils = deque_utils
import ori.format.utils = format_utils
import ori.hash_table = hash_table
import ori.hash_table.utils = hash_table_utils
import ori.heap = heap
import ori.heap.utils = heap_utils
import ori.io = io
import ori.iter.utils = iter_utils
import ori.linked_list = linked_list
import ori.linked_list.utils = linked_list_utils
import ori.os.utils = os_utils
import ori.path = path
import ori.queue = queue
import ori.queue.utils = queue_utils
import ori.random.utils = random_utils
import ori.stack = stack
import ori.stack.utils = stack_utils
import ori.validate = validate

main()
    io.print(format_utils.hex(255))
    io.print(format_utils.number_int(42))
    io.print(string(iter_utils.sum_int([1, 2, 3])))
    io.print(string(iter_utils.contains_int([1, 2, 3], 2)))
    io.print(string(os_utils.pid() > 0))
    io.print(string(random_utils.seeded_int(7, 1, 3) >= 1))
    const q: queue.Queue[int] = queue_utils.from_list([10, 20])
    io.print(string(queue_utils.peek_or(q, -1)))
    const s: stack.Stack[int] = stack_utils.from_list([5, 6])
    io.print(string(stack_utils.peek_or(s, -1)))
    const d: deque.Deque[int] = deque_utils.from_list([1, 2, 3])
    io.print(string(deque_utils.front_or(d, -1)))
    const h: heap.Heap[int] = heap_utils.from_list_int([30, 10, 20])
    io.print(string(heap_utils.peek_or_int(h, -1)))
    const ll: linked_list.LinkedList[int] = linked_list_utils.from_list([9, 8])
    io.print(string(linked_list_utils.front_or(ll, -1)))
    var table: hash_table.HashTable[string, int] = hash_table.new()
    hash_table.set(table, "a", 1)
    io.print(string(hash_table_utils.get_or_string_int(table, "a", 0)))
    io.print(path.relative("a/b/c", "a/b"))
    io.print(string(validate.even(4)))
    io.print(string(validate.blank("   ")))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_layer2_remaining");
    assert_eq!(
        stdout,
        "ff\n42\n6\ntrue\ntrue\ntrue\n10\n6\n1\n10\n9\n1\nc\ntrue\ntrue\n"
    );
}

#[test]
fn compile_runs_stdlib_layer3_algorithms_extensions() {
    let dir = TestDir::new("stdlib_layer3_extensions");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.bytes.algorithms = bytes_algorithms
import ori.io = io
import ori.map = maps
import ori.map = map_algorithms
import ori.math.algorithms = math_algorithms
import ori.set.algorithms = set_algorithms
import ori.set = sets
import ori.string = str
import ori.string = string_algorithms

main()
    var base: map[string, int] = maps.new()
    maps.set(base, "a", 1)
    var overlay: map[string, int] = maps.new()
    maps.set(overlay, "b", 2)
    const merged: map[string, int] = map_algorithms.merge_string_int(base, overlay)
    io.print(string(map_algorithms.key_count_string_int(merged)))
    io.print(string(set_algorithms.intersection_size_string(sets.from_list(["x", "y"]), sets.from_list(["y", "z"]))))
    io.print(string(string_algorithms.equals_any("ok", ["no", "ok"])))
    io.print(string(bytes_algorithms.is_prefix_of(str.to_bytes("he"), str.to_bytes("hello"))))
    io.print(string(math_algorithms.is_approx_zero(0.000001)))
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_layer3_extensions");
    assert_eq!(stdout, "2\n1\ntrue\ntrue\ntrue\n");
}

#[test]
fn check_accepts_selective_imports_with_aliases() {
    let dir = TestDir::new("selective_imports_aliases");
    dir.write(
        "app/math.orl",
        r#"module app.math

public add(a: int, b: int) -> int
    return a + b
end
"#,
    );
    dir.write(
        "app/model.orl",
        r#"module app.model

public struct User
    name: string
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.math (add = plus)
import app.model (User)

main()
    const total: int = plus(1, 2)
    const user: User = User { name: "Ori" }
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert!(
        !diagnostic_codes(&out).contains(&"bind.unused_import"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_duplicate_selective_import_names() {
    let dir = TestDir::new("selective_import_duplicate");
    dir.write(
        "app/a.orl",
        r#"module app.a

public value() -> int
    return 1
end
"#,
    );
    dir.write(
        "app/b.orl",
        r#"module app.b

public value() -> int
    return 2
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.a (value)
import app.b (value)

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"bind.duplicate_alias"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_reports_unknown_selective_import_member() {
    let dir = TestDir::new("selective_import_unknown_member");
    dir.write(
        "app/math.orl",
        r#"module app.math

public add(a: int, b: int) -> int
    return a + b
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.math (missing)

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "{:?}", out.diagnostics);
    assert!(
        diagnostic_codes(&out).contains(&"bind.import_member_unknown"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn check_accepts_flattened_stdlib_selective_imports() {
    let dir = TestDir::new("flattened_stdlib_selective");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.fs (read_text_or)
import ori.list (singleton, sum_int)
import ori.string (is_empty, truncate = cut)

main()
    const empty: bool = is_empty("")
    const text: string = cut("abcdef", 3)
    const one: list[int] = singleton(3)
    const total: int = sum_int(one)
    const fallback: string = read_text_or("missing.txt", "fallback")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_keeps_stdlib_flattened_paths_compatible() {
    let dir = TestDir::new("flattened_stdlib_paths_compat");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.fs.utils = fu
import ori.list = lu
import ori.string = su

main()
    const empty: bool = su.is_empty("")
    const text: string = su.truncate("abcdef", 3)
    const one: list[int] = lu.singleton(3)
    const total: int = lu.sum_int(one)
    const fallback: string = fu.read_text_or("missing.txt", "fallback")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn check_accepts_flattened_stdlib_parent_selective_imports() {
    let dir = TestDir::new("flattened_stdlib_parent_selective");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.map = maps
import ori.map (get_or, merge_string_int)
import ori.bytes = bytes_mod
import ori.bytes (compare_lex, is_empty)

main()
    const m: map[string, int] = maps.new()
    maps.set(m, "k", 1)
    const overlay: map[string, int] = maps.new()
    maps.set(overlay, "k", 2)
    const merged: map[string, int] = merge_string_int(m, overlay)
    const value: int = get_or(merged, "k", 0)
    const left: bytes = bytes_mod.from_list([1, 2])
    const right: bytes = bytes_mod.from_list([1, 3])
    const cmp: int = compare_lex(left, right)
    const empty: bool = is_empty(bytes_mod.from_list([]))
    const _unused: int = value + cmp
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

/// STDLIB-1: L1 + true L2 helpers are available via `ori.X` without nested imports.
#[test]
fn compile_runs_stdlib_parent_canonical_no_utils_import() {
    let dir = TestDir::new("stdlib_parent_canonical_no_utils");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.bytes = bytes_mod
import ori.format = fmt
import ori.fs = fs
import ori.graph = graph
import ori.io = io
import ori.list = lists
import ori.math = math
import ori.os = os
import ori.queue = queue
import ori.random = random
import ori.set = sets
import ori.tree = tree

main()
    -- L1 via parent path (no .utils)
    io.print(fmt.hex(255))
    io.print(string(os.pid() > 0))
    io.print(string(math.log10(100.0) > 1.9))
    const shuffled: list[int] = random.shuffle([1, 2, 3])
    io.print(string(lists.len(shuffled) == 3))
    match fs.create_dir_all(".")
        case ok(_):
            io.print("dir_ok")
        case err(_):
            io.print("dir_err")
    end

    -- L2 helpers that lived under utils/algorithms (parent only)
    io.print(fmt.number_int(42))
    io.print(string(os.env_or("PATH", "") != ""))
    const q: queue.Queue[int] = queue.from_list([10, 20])
    io.print(string(queue.peek_or(q, -1)))
    const left: bytes = bytes_mod.from_list([1, 2])
    const right: bytes = bytes_mod.from_list([1, 3])
    io.print(string(bytes_mod.compare_lex(left, right) < 0))
    io.print(string(bytes_mod.is_prefix_of(bytes_mod.from_list([1]), left)))
    const g: graph.Graph[string] = graph.new(false)
    graph.add_edge(g, "a", "b")
    graph.add_edge(g, "b", "c")
    io.print(string(graph.has_path(g, "a", "c")))
    io.print(string(sets.intersection_size_string(sets.from_list(["x", "y"]), sets.from_list(["y", "z"]))))
    const t: tree.Tree[int] = tree.new(1)
    io.print(string(tree.is_leaf(t, tree.root(t))))
    io.print_line("ok")
end
"#,
    );

    let stdout = compile_and_run(&dir, "stdlib_parent_canonical_no_utils");
    let normalized = stdout.replace("\r\n", "\n");
    assert!(
        normalized.contains("ff") || normalized.contains("FF"),
        "expected hex output, got: {stdout:?}"
    );
    assert!(normalized.contains("true"), "stdout: {stdout:?}");
    assert!(normalized.contains("42"), "stdout: {stdout:?}");
    assert!(normalized.contains("10"), "queue peek, stdout: {stdout:?}");
    assert!(normalized.contains("1\n") || normalized.contains("\n1\n"), "set size, stdout: {stdout:?}");
    assert!(normalized.contains("ok"), "stdout: {stdout:?}");
}

#[test]
fn compile_runs_stdlib_io_streams_native() {
    let dir = TestDir::new("compile_stdlib_io_streams_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.string = str
import ori.io (write_bytes, flush, close_output)

main()
    const out: ori.io.Output = io.stdout()
    const payload: bytes = str.to_bytes("stream\n")
    match write_bytes(out, payload)
        case ok(_):
            match flush(out)
                case ok(_):
                    close_output(out)
                case err(_):
            end
        case err(_):
    end
end
"#,
    );

    let exe = exe_path(&dir, "stdlib_io_streams");
    let out = match run_compile(&dir.path("main.orl"), Path::new(&exe)) {
        Ok(o) => o,
        Err(e) => panic!("compile error: {e}"),
    };
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "stream\n");
}

#[test]
fn compile_runs_net_tcp_listen_accept_loopback() {
    let dir = TestDir::new("net_tcp_listen_accept");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.net = net
import ori.string = str
import ori.task = task

serve_once(listener: net.Listener)
    match net.accept(listener)
        case ok(server_conn):
            match net.read_some(server_conn, 64)
                case ok(_):
                    match net.write_all(server_conn, str.to_bytes("pong"))
                        case ok(_):
                            net.close(server_conn)
                        case err(_):
                    end
                case err(_):
            end
        case err(_):
    end
    net.close_listener(listener)
end

main()
    match net.listen("127.0.0.1", 0)
        case ok(listener):
            const port: int = net.listener_port(listener)
            const server_job: task.Job[void] = task.run_blocking(() -> void
                serve_once(listener)
            end)
            match net.connect("127.0.0.1", port, 5000)
                case ok(client):
                    match net.write_all(client, str.to_bytes("ping"))
                        case ok(_):
                            match net.read_some(client, 64)
                                case ok(data):
                                    match str.from_bytes(data)
                                        case ok(text):
                                            io.print(text)
                                        case err(_):
                                            io.print("decode_err")
                                    end
                                case err(_):
                                    io.print("read_err")
                            end
                        case err(_):
                            io.print("write_err")
                    end
                    net.close(client)
                case err(_):
                    io.print("connect_err")
            end
            match task.join(server_job)
                case ok(_):
                case err(_):
            end
        case err(_):
            io.print("listen_err")
    end
end
"#,
    );

    let stdout = compile_and_run(&dir, "net_tcp_loopback");
    assert_eq!(stdout.trim(), "pong");
}

#[test]
fn compile_runs_net_udp_loopback() {
    let dir = TestDir::new("net_udp_loopback");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.net = net
import ori.string = str

main()
    match net.udp_bind("127.0.0.1", 0)
        case ok(sock):
            const port: int = net.udp_local_port(sock)
            match net.udp_send_to(sock, "127.0.0.1", port, str.to_bytes("udp"))
                case ok(_):
                    match net.udp_recv_from(sock, 64)
                        case ok(data):
                            match str.from_bytes(data)
                                case ok(text):
                                    io.print(text)
                                case err(_):
                                    io.print("decode_err")
                            end
                        case err(_):
                            io.print("recv_err")
                    end
                case err(_):
                    io.print("send_err")
            end
            net.udp_close(sock)
        case err(_):
            io.print("bind_err")
    end
end
"#,
    );

    let stdout = compile_and_run(&dir, "net_udp_loopback");
    assert_eq!(stdout.trim(), "udp");
}

#[test]
fn compile_runs_net_connect_tls_reports_error_on_refused_port() {
    let dir = TestDir::new("net_tls_refused");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.net = net

main()
    match net.connect_tls("127.0.0.1", 59999, 500)
        case ok(_):
            io.print("unexpected_success")
        case err(_):
            io.print("tls_err")
    end
end
"#,
    );

    let stdout = compile_and_run(&dir, "net_tls_refused");
    assert_eq!(stdout.trim(), "tls_err");
}

#[test]
fn check_accepts_net_v2_flatten_selective_imports() {
    let dir = TestDir::new("net_v2_flatten_imports");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.net (connect_tls, listen, udp_bind, listener_port)

main()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn compile_runs_stdlib_io_utils_native() {
    let dir = TestDir::new("compile_stdlib_io_utils_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.io.utils = iu
import ori.string = str

main()
    const out: ori.io.Output = io.stdout()
    const payload: bytes = str.to_bytes("utils ok\n")
    match iu.write_bytes(out, payload)
        case ok(_):
            match iu.flush(out)
                case ok(_):
                    iu.close_output(out)
                case err(_):
            end
        case err(_):
    end
end
"#,
    );

    let exe = exe_path(&dir, "stdlib_io_utils");
    let out = match run_compile(&dir.path("main.orl"), Path::new(&exe)) {
        Ok(o) => o,
        Err(e) => panic!("compile error: {e}"),
    };
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "utils ok\n");
}

#[test]
fn compile_runs_vec2_stdlib_native() {
    let dir = TestDir::new("vec2_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.math.vec2 = vec2
import ori.io = io

main()
    const a: vec2.Vec2 = vec2.Vec2 { x: 1.0, y: 2.0 }
    const b: vec2.Vec2 = vec2.Vec2 { x: 3.0, y: 4.0 }
    const sum: vec2.Vec2 = a + b
    const diff: vec2.Vec2 = b - a
    const product: vec2.Vec2 = a * b
    const quotient: vec2.Vec2 = b / a
    const d: float = vec2.dot(a, b)
    io.print(string(sum.x))
    io.print(string(sum.y))
    io.print(string(diff.x))
    io.print(string(diff.y))
    io.print(string(product.x))
    io.print(string(product.y))
    io.print(string(quotient.x))
    io.print(string(quotient.y))
    io.print(string(d))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "vec2_test.exe"
    } else {
        "vec2_test"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    // a = (1,2), b = (3,4)
    // sum = (4,6), diff = (2,2), product = (3,8), quotient = (3,2)
    // dot = 1*3 + 2*4 = 11
    assert_eq!(stdout.replace("\r\n", "\n"), "4\n6\n2\n2\n3\n8\n3\n2\n11\n");
}

#[test]
fn build_accepts_repr_c_attribute() {
    let dir = TestDir::new("repr_c_attribute");
    dir.write(
        "main.orl",
        r#"module app.main

@repr("C")
struct SDL_Rect
    x: int
    y: int
    w: int
    h: int
end

main()
    const r: SDL_Rect = SDL_Rect { x: 0, y: 0, w: 100, h: 200 }
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn build_accepts_handle_type() {
    let dir = TestDir::new("handle_type");
    dir.write(
        "main.orl",
        r#"module app.main

return_handle() -> handle[int]
    return return_handle()
end

use_handle(h: handle[int]) -> handle[int]
    return h
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn build_accepts_buffer_type() {
    let dir = TestDir::new("buffer_type");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.buffer = buf

struct MyBuffer
    data: buf.Buffer[int]
end

main()
    const length: int = 0
end
"#,
    );

    let out = run_build(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn compile_runs_io_file_stream_adapters_native() {
    // STDLIB-3: open_input / open_output + using dispose for streams
    let dir = TestDir::new("io_file_stream_adapters");
    let path = dir.path("note.txt");
    let path_lit = path.to_string_lossy().replace('\\', "/");
    dir.write(
        "main.orl",
        &format!(
            r#"module app.main

import ori.io = io
import ori.string = str
import ori.io (open_input, open_output, write_text, read_text, flush, close_input, close_output)

main()
    match open_output("{path}")
        case ok(out):
            match write_text(out, "hello-stream")
                case ok(_):
                    match flush(out)
                        case ok(_):
                            close_output(out)
                        case err(m):
                            io.println(m)
                    end
                case err(m):
                    io.println(m)
            end
        case err(m):
            io.println(m)
    end
    match open_input("{path}")
        case ok(input):
            match read_text(input, 64)
                case ok(maybe):
                    if some(text) = maybe
                        io.println(text)
                    end
                    close_input(input)
                case err(m):
                    io.println(m)
            end
        case err(m):
            io.println(m)
    end
    match open_output("{path}")
        case ok(out2_raw):
            using out2: ori.io.Output = out2_raw
            match write_text(out2, "using-ok")
                case ok(_):
                    match flush(out2)
                        case ok(_):
                        case err(_):
                    end
                case err(_):
            end
        case err(_):
    end
end
"#,
            path = path_lit
        ),
    );

    let exe = exe_path(&dir, "io_file_streams");
    let out = match run_compile(&dir.path("main.orl"), Path::new(&exe)) {
        Ok(o) => o,
        Err(e) => panic!("compile error: {e}"),
    };
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("hello-stream"),
        "stdout={stdout:?}"
    );
}

#[test]
fn check_accepts_http_parse_and_build_request() {
    // STDLIB-2: pure parse/build without network
    let dir = TestDir::new("http_parse_build");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.net.http = http
import ori.io = io
import ori.string = str

main()
    const req: string = http.build_request("GET", "/hi", "example.com", "", "")
    if not str.contains(req, "GET /hi HTTP/1.1")
        io.println("bad request")
        return
    end
    if not str.contains(req, "Host: example.com")
        io.println("bad host")
        return
    end
    const raw: string = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello"
    match http.parse_response(raw)
        case ok(resp):
            if resp.status != 200
                io.println("bad status")
                return
            end
            if resp.body != "hello"
                io.println("bad body")
                return
            end
            io.println("http-ok")
        case err(msg):
            io.println(msg)
    end
end
"#,
    );
    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}

#[test]
fn compile_runs_http_get_loopback_native() {
    // STDLIB-2: thin client against a local TCP server
    let dir = TestDir::new("http_get_loopback");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.net = net
import ori.net.http = http
import ori.string = str
import ori.task = task

serve_http_once(listener: net.Listener)
    match net.accept(listener)
        case ok(conn):
            match net.read_some(conn, 1024)
                case ok(_):
                    const body: string = "pong"
                    const resp: string = "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\npong"
                    match net.write_all(conn, str.to_bytes(resp))
                        case ok(_):
                            net.close(conn)
                        case err(_):
                    end
                case err(_):
            end
        case err(_):
    end
    net.close_listener(listener)
end

main()
    match net.listen("127.0.0.1", 0)
        case ok(listener):
            const port: int = net.listener_port(listener)
            const job: task.Job[void] = task.run_blocking(() -> void
                serve_http_once(listener)
            end)
            match http.get_plain("127.0.0.1", port, "/", 5000)
                case ok(resp):
                    if resp.status == 200
                        if resp.body == "pong"
                            io.println("http-loopback-ok")
                        else
                            io.println(resp.body)
                        end
                    else
                        io.println(string(resp.status))
                    end
                case err(msg):
                    io.println(msg)
            end
            match task.join(job)
                case ok(_):
                case err(_):
            end
        case err(msg):
            io.println(msg)
    end
end
"#,
    );
    let exe = exe_path(&dir, "http_loopback");
    let out = match run_compile(&dir.path("main.orl"), Path::new(&exe)) {
        Ok(o) => o,
        Err(e) => panic!("compile error: {e}"),
    };
    assert!(!out.has_errors, "{:?}", out.diagnostics);
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("http-loopback-ok"),
        "stdout={stdout:?} stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_accepts_stdlib_public_type_aliases() {
    // S3 culture 1.3/2.4: domain `public alias` on stdlib parents (M2 residual closed).
    let dir = TestDir::new("stdlib_public_type_aliases");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.fs (TextResult, BoolResult, IoResult, write_text_result, exists_result, remove_file)
import ori.io (WriteResult)
import ori.net (ConnectionResult, TextResult = NetTextResult)
import ori.json (ValueResult, TextResult = JsonTextResult)
import ori.config (JsonResult, TextResult = ConfigTextResult)

uses_fs(path: string) -> TextResult
    return write_text_result(path, "x")
end

uses_bool(path: string) -> BoolResult
    return exists_result(path)
end

uses_io(path: string) -> IoResult
    return remove_file(path)
end

write_count_err(msg: string) -> WriteResult
    return err(msg)
end

net_text_err(msg: string) -> NetTextResult
    return err(msg)
end

json_value_err(msg: string) -> ValueResult
    return err(msg)
end

json_text_err(msg: string) -> JsonTextResult
    return err(msg)
end

config_json_err(msg: string) -> JsonResult
    return err(msg)
end

config_text_err(msg: string) -> ConfigTextResult
    return err(msg)
end

conn_err(msg: string) -> ConnectionResult
    return err(msg)
end

main()
    const _w: WriteResult = write_count_err("x")
    const _nt: NetTextResult = net_text_err("x")
    const _jv: ValueResult = json_value_err("x")
    const _jt: JsonTextResult = json_text_err("x")
    const _cj: JsonResult = config_json_err("x")
    const _ct: ConfigTextResult = config_text_err("x")
    const _c: ConnectionResult = conn_err("x")
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);
}
