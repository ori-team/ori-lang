//! Regressions for integer semantics, the HIR mid-end, and the recursion and
//! capacity bounds that used to kill the process instead of reporting an error.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{diagnostic_codes, exe_path, normalize_stdout, ori_exe, TestDir};
use ori_driver::pipeline::{default_shared_lib_path, run_check, with_frontend_stack};

const OPT_LEVELS: [&str; 3] = ["none", "default", "aggressive"];

/// Compile `source` through the `ori` binary so `ORI_OPT` reaches the mid-end,
/// then run it. Returns the executable's trimmed stdout.
///
/// The compiler runs as a subprocess on purpose: `ORI_OPT` is read from the
/// environment, and mutating the environment of the test process would race
/// with the other tests sharing it.
fn compile_and_run(dir: &TestDir, opt_level: &str) -> String {
    let exe = exe_path(dir, "app");
    let compiled = Command::new(ori_exe())
        .env("ORI_OPT", opt_level)
        .arg("compile")
        .arg(dir.path("main.orl"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "compilation failed at ORI_OPT={opt_level}\nstdout:\n{}\nstderr:\n{}",
        normalize_stdout(compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );

    let output = Command::new(&exe).output().unwrap();
    let stdout = normalize_stdout(output.stdout);
    assert!(
        output.status.success(),
        "program failed at ORI_OPT={opt_level}\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout.trim().to_string()
}

/// Run `source` at every optimisation level and assert they all print
/// `expected`: the mid-end may change how a program runs, never what it prints.
fn assert_same_output_at_every_opt_level(name: &str, source: &str, expected: &str) {
    for level in OPT_LEVELS {
        let dir = TestDir::new(&format!("{name}_{level}"));
        dir.write("main.orl", source);
        assert_eq!(
            compile_and_run(&dir, level),
            expected,
            "`{name}` disagrees with the expected output at ORI_OPT={level}"
        );
    }
}

/// Build `source` and return the stderr of a run that is expected to abort.
fn stderr_of_aborting_program(name: &str, source: &str) -> String {
    output_of_aborting_program(name, source).1
}

/// Build `source`, run it, and return the `(stdout, stderr)` of a run that is
/// expected to abort. Both pipes are captured, so buffered stdout only shows up
/// when the runtime flushes it before aborting.
fn output_of_aborting_program(name: &str, source: &str) -> (String, String) {
    output_of_aborting_program_at_opt_level(name, source, None)
}

/// Build `source` with an explicit optimizer level, run it, and return the
/// `(stdout, stderr)` of a process that is expected to abort.
fn output_of_aborting_program_at_opt_level(
    name: &str,
    source: &str,
    opt_level: Option<&str>,
) -> (String, String) {
    let dir = TestDir::new(name);
    dir.write("main.orl", source);
    let exe = exe_path(&dir, "app");
    let mut compile = Command::new(ori_exe());
    if let Some(opt_level) = opt_level {
        compile.env("ORI_OPT", opt_level);
    }
    let compiled = compile
        .arg("compile")
        .arg(dir.path("main.orl"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "`{name}` failed to compile: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let output = Command::new(&exe).output().unwrap();
    assert!(
        !output.status.success(),
        "`{name}` was expected to abort but exited cleanly"
    );
    (
        normalize_stdout(output.stdout),
        String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n"),
    )
}

#[test]
fn constant_folding_wraps_sized_integers_exactly_like_the_backend() {
    // Folding used to retype every result as `int`, which both widened the
    // value and desynchronised HIR from the narrower Cranelift slot.
    let source = r#"module app.main

import ori.io = io

main()
    const small: int32 = 2i32 + 3i32
    const wrapped_i8: int8 = 100i8 + 100i8
    const wrapped_u8: u8 = 250u8 + 10u8
    const wrapped_i32: int32 = 2000000000i32 + 2000000000i32
    io.println(f"{small} {wrapped_i8} {wrapped_u8} {wrapped_i32}")
end
"#;
    assert_same_output_at_every_opt_level("sized_integer_fold", source, "5 -56 4 -294967296");
}

#[test]
fn constant_folding_declines_trapping_division_instead_of_panicking() {
    // Folding `MIN / -1` and `x / 0` overflowed inside the folder itself, so
    // the compiler aborted with a Rust panic while lowering the program.
    let source = r#"module app.main

import ori.io = io

unreachable_division() -> int
    return (-9223372036854775807 - 1) / -1
end

zero_division() -> int
    return 1 / 0
end

main()
    io.println("compiled")
end
"#;
    let dir = TestDir::new("trapping_division_fold");
    dir.write("main.orl", source);
    for level in OPT_LEVELS {
        let exe = exe_path(&dir, &format!("app_{level}"));
        let compiled = Command::new(ori_exe())
            .env("ORI_OPT", level)
            .arg("compile")
            .arg(dir.path("main.orl"))
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "trapping division should compile at ORI_OPT={level}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

#[test]
fn dce_keeps_unused_integer_division_trap_at_every_opt_level() {
    let source = r#"module app.main

import ori.io = io

main()
    const unused: int = 1 / 0
    io.println("unreachable")
end
"#;

    for level in OPT_LEVELS {
        let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
            &format!("unused_division_{level}"),
            source,
            Some(level),
        );
        assert!(
            stderr.contains("ori integer division or remainder by zero"),
            "unused division must keep its runtime trap at ORI_OPT={level}: {stderr}"
        );
    }
}

#[test]
fn dce_keeps_unused_integer_remainder_trap_at_every_opt_level() {
    let source = r#"module app.main

import ori.io = io

main()
    var divisor: int = 0
    const unused: int = 1 % divisor
    io.println("unreachable")
end
"#;

    for level in OPT_LEVELS {
        let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
            &format!("unused_remainder_{level}"),
            source,
            Some(level),
        );
        assert!(
            stderr.contains("ori integer division or remainder by zero"),
            "unused remainder must keep its runtime trap at ORI_OPT={level}: {stderr}"
        );
    }
}

#[test]
fn dce_keeps_unused_shift_bounds_trap_at_every_opt_level() {
    let source = r#"module app.main

import ori.io = io

main()
    var count: int = 64
    const unused: int = 1 << count
    io.println("unreachable")
end
"#;

    for level in OPT_LEVELS {
        let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
            &format!("unused_shift_{level}"),
            source,
            Some(level),
        );
        assert!(
            stderr.contains("ori shift count out of range"),
            "unused shift must keep its runtime guard at ORI_OPT={level}: {stderr}"
        );
    }
}

#[test]
fn dce_keeps_unused_index_bounds_trap_at_every_opt_level() {
    let source = r#"module app.main

import ori.io = io

main()
    const values: list[int] = [1]
    const unused: int = values[2]
    io.println("unreachable")
end
"#;

    for level in OPT_LEVELS {
        let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
            &format!("unused_index_{level}"),
            source,
            Some(level),
        );
        assert!(
            stderr.contains("ori list index out of bounds"),
            "unused index must keep its runtime guard at ORI_OPT={level}: {stderr}"
        );
    }
}

#[test]
fn aggressive_inlining_keeps_an_unused_trapping_argument() {
    let source = r#"module app.main

import ori.io = io

ignore(value: int) -> int
    return 1
end

main()
    io.println(f"{ignore(1 / 0)}")
end
"#;

    let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
        "inline_unused_trapping_argument",
        source,
        Some("aggressive"),
    );
    assert!(
        stderr.contains("ori integer division or remainder by zero"),
        "aggressive inlining must not omit an unused trapping argument: {stderr}"
    );
}

#[test]
fn aggressive_inlining_preserves_argument_snapshots_and_single_evaluation() {
    // Call arguments are evaluated before entering the callee. Textually
    // substituting `current` into `combine` used to move its second read past
    // `change()`, while substituting `produce()` twice would duplicate it.
    let source = r#"module app.main

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

main()
    io.println(f"{combine(current, current)}")
    io.println(f"{twice(produce())}")
    io.println(f"{calls}")
end
"#;

    assert_same_output_at_every_opt_level("inline_argument_evaluation", source, "2\n8\n1");
}

#[test]
fn aggressive_inlining_keeps_parameter_contract_checks() {
    let source = r#"module app.main

import ori.io = io

positive(value: int if it > 0) -> int
    return value
end

main()
    io.println(f"{positive(0)}")
end
"#;

    let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
        "inline_parameter_contract",
        source,
        Some("aggressive"),
    );
    assert!(
        stderr.contains("contract.param_violation"),
        "aggressive inlining must not erase parameter contracts: {stderr}"
    );
}

#[test]
fn dce_keeps_associated_call_argument_bindings_at_every_opt_level() {
    // Associated calls carry no receiver expression in HIR. DCE must still
    // see their arguments, or it can delete `argument` before the static
    // dispatch is lowered.
    let source = r#"module app.main

import ori.io = io

struct User
    seed: int
end

apply User
    consume(value: int) -> int
        return value
    end
end

main()
    const argument: int = 41
    const unused_result: int = User.consume(argument)
    io.println("associated call survived")
end
"#;

    assert_same_output_at_every_opt_level(
        "associated_call_argument_dce",
        source,
        "associated call survived",
    );
}

#[test]
fn dce_keeps_bindings_used_only_by_match_guards_at_every_opt_level() {
    let source = r#"module app.main

import ori.io = io

main()
    const threshold: int = 10
    match 11
    case value if value > threshold:
        io.println("guard kept")
    case else:
        io.println("wrong arm")
    end
end
"#;

    assert_same_output_at_every_opt_level("match_guard_binding_dce", source, "guard kept");
}

#[test]
fn dce_keeps_failed_field_contracts_at_every_opt_level() {
    let source = r#"module app.main

import ori.io = io

struct Positive
    value: int if it > 0
end

main()
    const unused: Positive = Positive { value: 0 }
    io.println("unreachable")
end
"#;

    for level in OPT_LEVELS {
        let (_stdout, stderr) = output_of_aborting_program_at_opt_level(
            &format!("unused_field_contract_{level}"),
            source,
            Some(level),
        );
        assert!(
            stderr.contains("contract.field_violation"),
            "DCE must retain failed field contracts at ORI_OPT={level}: {stderr}"
        );
    }
}

#[test]
fn dce_keeps_custom_destructors_at_every_opt_level() {
    let source = r#"module app.main

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
    const resource: Resource = Resource { label: "optimizer" + "-resource" }
end

main()
    consume()
    io.println("done")
end
"#;

    assert_same_output_at_every_opt_level(
        "unused_custom_destructor_dce",
        source,
        "destroy:optimizer-resource\ndone",
    );
}

#[test]
fn strength_reduction_keeps_loop_results_for_every_bound() {
    // The closed form `n * (n - 1) / 2` only holds for a positive bound that
    // does not overflow; the rewrite now guards itself and keeps the loop.
    let source = r#"module app.main

import ori.io = io

sum_to(n: int) -> int
    var total: int = 0
    var index: int = 0
    while index < n
        total = total + index
        index = index + 1
    end
    return total
end

main()
    io.println(f"{sum_to(10)} {sum_to(0)} {sum_to(-5)} {sum_to(1)}")
end
"#;
    assert_same_output_at_every_opt_level("strength_reduction_bounds", source, "45 0 0 0");
}

#[test]
fn unsigned_integers_use_unsigned_division_comparison_and_formatting() {
    // `u64` fills the whole slot, so the signed instructions read every value
    // above `i64::MAX` as negative.
    let source = r#"module app.main

import ori.io = io

main()
    const big: u64 = 18446744073709551615u64
    const two: u64 = 2u64
    io.println(f"{big}")
    io.println(f"{big / two} {big % two}")
    io.println(f"{big > two} {big < two}")
    const wide: u32 = 4294967295u32
    const pair: u32 = 2u32
    io.println(f"{wide / pair} {wide % pair} {wide > pair}")
end
"#;
    assert_same_output_at_every_opt_level(
        "unsigned_arithmetic",
        source,
        "18446744073709551615\n9223372036854775807 1\ntrue false\n2147483647 1 true",
    );
}

#[test]
fn integer_division_by_zero_reports_a_message_instead_of_a_bare_signal() {
    let source = r#"module app.main

import ori.io = io

main()
    var divisor: int = 0
    var value: int = 10
    io.println(f"{value / divisor}")
end
"#;
    let stderr = stderr_of_aborting_program("division_by_zero", source);
    assert!(
        stderr.contains("ori integer division or remainder by zero"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn integer_division_overflow_reports_a_message_instead_of_a_bare_signal() {
    let source = r#"module app.main

import ori.io = io

main()
    var smallest: int = -9223372036854775807 - 1
    var minus_one: int = -1
    io.println(f"{smallest / minus_one}")
end
"#;
    let stderr = stderr_of_aborting_program("division_overflow", source);
    assert!(
        stderr.contains("ori integer division overflow"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn oversized_collection_capacity_reports_a_message_instead_of_crashing() {
    // The growth loop used to multiply past `isize::MAX` and hand a wrapped
    // byte count to `realloc`.
    let source = r#"module app.main

import ori.io = io
import ori.list = lists

main()
    var huge: list[int] = lists.with_capacity(2305843009213693952)
    io.println(f"{lists.len(huge)}")
end
"#;
    let stderr = stderr_of_aborting_program("oversized_capacity", source);
    assert!(
        stderr.contains("ori collection capacity exceeds the addressable maximum"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn error_typed_expressions_do_not_cascade_into_a_second_mismatch() {
    let dir = TestDir::new("error_type_cascade");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const value: int = undefined_thing
    io.println(f"{value}")
end
"#,
    );

    let checked = run_check(&dir.path("main.orl")).unwrap();
    let codes = diagnostic_codes(&checked);
    assert!(
        codes.contains(&"name.undefined"),
        "the undefined name should still be reported, got {codes:?}"
    );
    assert!(
        !codes.contains(&"type.type_mismatch"),
        "an `<error>` type must not produce a follow-on mismatch, got {codes:?}"
    );
}

#[test]
fn deeply_nested_expressions_report_a_diagnostic_instead_of_exhausting_the_stack() {
    let mut source = String::from("module app.main\n\nmain()\n    const value: int = ");
    source.push_str(&"(".repeat(50_000));
    source.push('1');
    source.push_str(&")".repeat(50_000));
    source.push_str("\nend\n");

    let dir = TestDir::new("deep_expression_nesting");
    dir.write("main.orl", &source);

    // Same stack contract the `ori` binary and the language server give the
    // front end; a libtest worker thread alone is far below it.
    let path = dir.path("main.orl");
    let checked = with_frontend_stack(move || run_check(&path).unwrap());
    let codes = diagnostic_codes(&checked);
    assert_eq!(
        codes
            .iter()
            .filter(|c| **c == "parse.nesting_too_deep")
            .count(),
        1,
        "the nesting limit should be reported exactly once, got {codes:?}"
    );
}

#[test]
fn deeply_nested_blocks_report_a_diagnostic_instead_of_exhausting_the_stack() {
    let depth = 5_000;
    let mut source = String::from("module app.main\n\nmain()\n");
    source.push_str(&"if true\n".repeat(depth));
    source.push_str("    const value: int = 1\n");
    source.push_str(&"end\n".repeat(depth));
    source.push_str("end\n");

    let dir = TestDir::new("deep_block_nesting");
    dir.write("main.orl", &source);

    let path = dir.path("main.orl");
    let checked = with_frontend_stack(move || run_check(&path).unwrap());
    let codes = diagnostic_codes(&checked);
    assert!(
        codes.contains(&"parse.nesting_too_deep"),
        "expected the nesting limit diagnostic, got {codes:?}"
    );
}

#[test]
fn compile_derives_an_output_path_for_bare_file_names_and_project_roots() {
    let dir = TestDir::new("compile_output_paths");
    dir.write(
        "ori.proj",
        "name = \"outpaths\"\nversion = \"0.1.0\"\nentry = \"main.orl\"\n",
    );
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    io.println("ok")
end
"#,
    );
    let root = dir.path("");

    // A bare file name has an *empty* parent, which used to make the
    // incremental input scan fail before any code was generated.
    let bare = Command::new(ori_exe())
        .current_dir(&root)
        .args(["compile", "main.orl"])
        .output()
        .unwrap();
    assert!(
        bare.status.success(),
        "compiling a bare file name failed: {}",
        String::from_utf8_lossy(&bare.stderr)
    );
    let bare_exe = exe_path(&dir, "main");
    assert!(bare_exe.is_file(), "expected `{}`", bare_exe.display());

    // A project root is a *directory*: `with_extension("")` used to hand the
    // directory itself to the linker as the output file.
    let project = Command::new(ori_exe())
        .current_dir(&root)
        .args(["compile", "."])
        .output()
        .unwrap();
    assert!(
        project.status.success(),
        "compiling a project root failed: {}",
        String::from_utf8_lossy(&project.stderr)
    );
    let project_exe = exe_path(&dir, "app");
    assert!(
        project_exe.is_file(),
        "expected `{}`",
        project_exe.display()
    );

    let run = Command::new(&project_exe).output().unwrap();
    assert!(run.status.success());
    assert_eq!(normalize_stdout(run.stdout).trim(), "ok");
}

#[test]
fn default_shared_library_path_stays_inside_a_project_root() {
    let dir = TestDir::new("shared_lib_default_path");
    let root = dir.path("");
    assert_eq!(
        default_shared_lib_path(&root),
        root.join(shared_library_name("ori_lib")),
        "a project root names the directory, not the library"
    );
    assert_eq!(
        default_shared_lib_path(&root.join("ori.proj")),
        root.join(shared_library_name("ori_lib")),
        "a manifest names the directory, not the library"
    );
    assert_eq!(
        default_shared_lib_path(&root.join("greeter.orl")),
        root.join(shared_library_name("greeter")),
        "a source file contributes its stem"
    );
}

fn shared_library_name(stem: &str) -> PathBuf {
    let name = if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    };
    PathBuf::from(name)
}

#[test]
fn buffered_output_survives_a_runtime_abort() {
    // `abort` skips the handlers that flush stdout, so everything printed
    // before the failure used to vanish whenever stdout was a pipe.
    let source = r#"module app.main

import ori.io = io

main()
    io.println("printed before the failure")
    var divisor: int = 0
    io.println(f"{7 / divisor}")
end
"#;
    let (stdout, stderr) = output_of_aborting_program("flush_before_abort", source);
    assert_eq!(stdout.trim(), "printed before the failure");
    assert!(
        stderr.contains("ori integer division or remainder by zero"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn string_conversion_renders_every_integer_width_with_its_own_signedness() {
    // `string(x)` lowers to a type-directed runtime symbol; the unsigned types
    // must not reach the signed formatter.
    let source = r#"module app.main

import ori.io = io

main()
    const big: u64 = 18446744073709551615u64
    const wide: u32 = 4294967295u32
    const byte: u8 = 250u8
    const negative: int = -42
    io.println(string(big))
    io.println(string(wide))
    io.println(string(byte))
    io.println(string(negative))
end
"#;
    assert_same_output_at_every_opt_level(
        "string_conversion_signedness",
        source,
        "18446744073709551615\n4294967295\n250\n-42",
    );
}

#[test]
fn emitted_c_compiles_under_a_strict_standard_dialect() {
    if !cfg!(target_os = "linux") {
        return;
    }
    if Command::new("cc").arg("--version").output().is_err() {
        return;
    }

    // The generated runtime calls `nanosleep`, `gmtime_r`, and `getline`, which
    // a strict `-std=c11` hides unless the file requests the POSIX feature set
    // itself.
    let dir = TestDir::new("emitted_c_strict_dialect");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

main()
    const big: u64 = 18446744073709551615u64
    io.println(f"{big}")
end
"#,
    );
    let generated = dir.path("main.c");
    let emitted = Command::new(ori_exe())
        .arg("emit")
        .arg("c")
        .arg(dir.path("main.orl"))
        .arg("-o")
        .arg(&generated)
        .output()
        .unwrap();
    assert!(
        emitted.status.success(),
        "emitting C failed: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );

    let host = exe_path(&dir, "chost");
    let compiled = Command::new("cc")
        .arg("-std=c11")
        .arg("-Werror=implicit-function-declaration")
        .arg("-o")
        .arg(&host)
        .arg(&generated)
        .arg("-lm")
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "emitted C failed under -std=c11:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let run = Command::new(&host).output().unwrap();
    assert!(run.status.success());
    assert_eq!(normalize_stdout(run.stdout).trim(), "18446744073709551615");
}

#[test]
fn emitted_c_check_message_is_not_a_format_string() {
    if !cfg!(target_os = "linux") || Command::new("cc").arg("--version").output().is_err() {
        return;
    }

    let dir = TestDir::new("emitted_c_check_message");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    check false, "quote \" slash \\ line\n %s %n"
end
"#,
    );
    let generated = dir.path("main.c");
    let emitted = Command::new(ori_exe())
        .arg("emit")
        .arg("c")
        .arg(dir.path("main.orl"))
        .arg("-o")
        .arg(&generated)
        .output()
        .unwrap();
    assert!(
        emitted.status.success(),
        "emitting C failed: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    let host = exe_path(&dir, "check_host");
    let compiled = Command::new("cc")
        .arg("-std=c11")
        .arg("-Wformat")
        .arg("-Werror=format-security")
        .arg("-o")
        .arg(&host)
        .arg(&generated)
        .arg("-lm")
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "emitted C failed to compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let run = Command::new(&host).output().unwrap();
    assert!(
        !run.status.success(),
        "failed check must terminate the C host"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("quote \" slash \\ line\n"),
        "stderr: {stderr:?}"
    );
    assert!(
        stderr.contains("%s %n"),
        "percent sequences must stay data: {stderr:?}"
    );
}

#[test]
fn aggressive_inlining_materializes_temporaries_for_mutated_globals() {
    let source = r#"module app.main

import ori.io = io

var state: int = 10

step() -> int
    state = state + 5
    return 2
end

calculate(x: int, y: int) -> int
    return x * step() + y
end

main()
    io.println(f"{calculate(state, state)}")
    io.println(f"{state}")
end
"#;

    // With materialization, `state` (10) is captured into temporaries before `calculate` runs.
    // Inside calculate: `10 * step() + 10 = 10 * 2 + 10 = 30`.
    // Then `state` was mutated by `step()` to 15.
    assert_same_output_at_every_opt_level(
        "inlining_materialized_temps",
        source,
        "30\n15",
    );
}
