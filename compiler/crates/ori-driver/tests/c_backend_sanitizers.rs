mod common;

use std::ffi::OsString;
use std::process::{Command, Output};

use common::{exe_path, normalize_stdout, ori_exe, TestDir};

const COMPILER_ENV: &str = "ORI_C_SANITIZER_CC";
const REQUIRE_SANITIZERS_ENV: &str = "ORI_REQUIRE_C_SANITIZERS";

#[test]
fn hostile_check_message_runs_cleanly_under_c_sanitizers() {
    let dir = TestDir::new("c_backend_sanitizers");
    let compiler = match find_sanitizer_compiler(&dir) {
        Ok(compiler) => compiler,
        Err(reason) if sanitizers_are_required() => {
            panic!("C sanitizer gate is required but unavailable: {reason}")
        }
        Err(reason) => {
            eprintln!(
                "SKIP hostile_check_message_runs_cleanly_under_c_sanitizers: {reason}; \
                 set {REQUIRE_SANITIZERS_ENV}=1 to make this a required gate"
            );
            return;
        }
    };

    dir.write(
        "main.orl",
        r#"module app.main

main()
    check false, "quote \" slash \\ line\n tab\t percent %s %n"
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
        .expect("failed to spawn `ori emit c`");
    assert_success("emitting hostile C input", &emitted);

    let host = exe_path(&dir, "hostile_check");
    let mut compile = sanitizer_command(&compiler);
    compile
        .arg("-Wformat=2")
        .arg("-Werror=format-security")
        .arg(&generated);
    if !cfg!(windows) {
        compile.arg("-lm");
    }
    let compiled = compile
        .arg("-o")
        .arg(&host)
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn {compiler:?}: {error}"));
    assert_success("compiling generated C with ASan and UBSan", &compiled);

    // The scenario intentionally aborts before normal cleanup. Keep ASan's
    // memory checks enabled while excluding leak reports caused by that exit.
    let run = Command::new(&host)
        .env(
            "ASAN_OPTIONS",
            "detect_leaks=0:halt_on_error=1:abort_on_error=1",
        )
        .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
        .output()
        .expect("failed to run sanitized C executable");
    assert!(
        !run.status.success(),
        "a failed Ori check must terminate the C host"
    );

    let stderr = normalize_stdout(run.stderr);
    let expected = "quote \" slash \\ line\n tab\t percent %s %n";
    assert!(
        stderr.contains(expected),
        "hostile check text was not preserved as data; stderr: {stderr:?}"
    );
    for sanitizer_error in [
        "ERROR: AddressSanitizer",
        "AddressSanitizer:DEADLYSIGNAL",
        "UndefinedBehaviorSanitizer",
        "runtime error:",
    ] {
        assert!(
            !stderr.contains(sanitizer_error),
            "sanitizer reported `{sanitizer_error}`; stderr: {stderr:?}"
        );
    }
}

#[test]
fn stored_any_with_managed_fields_uses_a_live_vtable_under_c_sanitizers() {
    let dir = TestDir::new("c_backend_any_sanitizers");
    let compiler = match find_sanitizer_compiler(&dir) {
        Ok(compiler) => compiler,
        Err(reason) if sanitizers_are_required() => {
            panic!("C sanitizer gate is required but unavailable: {reason}")
        }
        Err(reason) => {
            eprintln!(
                "SKIP stored_any_with_managed_fields_uses_a_live_vtable_under_c_sanitizers: \
                 {reason}; set {REQUIRE_SANITIZERS_ENV}=1 to make this a required gate"
            );
            return;
        }
    };

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

make_box() -> any[Labelled]
    return Tag { label: "vtable-alive" }
end

main()
    const boxed: any[Labelled] = make_box()
    io.print(boxed.label())
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
        .expect("failed to spawn `ori emit c`");
    assert_success("emitting managed any C input", &emitted);

    let host = exe_path(&dir, "managed_any");
    let mut compile = sanitizer_command(&compiler);
    compile.arg(&generated);
    if !cfg!(windows) {
        compile.arg("-lm");
    }
    let compiled = compile
        .arg("-o")
        .arg(&host)
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn {compiler:?}: {error}"));
    assert_success("compiling managed any with ASan and UBSan", &compiled);

    let run = Command::new(&host)
        .env(
            "ASAN_OPTIONS",
            "detect_leaks=0:halt_on_error=1:abort_on_error=1",
        )
        .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
        .output()
        .expect("failed to run sanitized managed-any executable");
    assert_success("running managed any with ASan and UBSan", &run);
    assert_eq!(normalize_stdout(run.stdout).trim(), "vtable-alive");
}

fn find_sanitizer_compiler(dir: &TestDir) -> Result<OsString, String> {
    dir.write("sanitizer_probe.c", "int main(void) { return 0; }\n");
    let source = dir.path("sanitizer_probe.c");
    let host = exe_path(dir, "sanitizer_probe");

    if let Some(compiler) = std::env::var_os(COMPILER_ENV) {
        return match probe_sanitizer_compiler(&compiler, &source, &host) {
            Ok(()) => Ok(compiler),
            Err(reason) => Err(format!("{COMPILER_ENV}={compiler:?} is unusable: {reason}")),
        };
    }

    let mut failures = Vec::new();
    for compiler in [OsString::from("clang"), OsString::from("cc")] {
        match probe_sanitizer_compiler(&compiler, &source, &host) {
            Ok(()) => return Ok(compiler),
            Err(reason) => failures.push(format!("{compiler:?}: {reason}")),
        }
    }

    Err(format!(
        "neither `clang` nor `cc` can compile and run with ASan+UBSan ({})",
        failures.join("; ")
    ))
}

fn probe_sanitizer_compiler(
    compiler: &OsString,
    source: &std::path::Path,
    host: &std::path::Path,
) -> Result<(), String> {
    let output = sanitizer_command(compiler)
        .arg(source)
        .arg("-o")
        .arg(host)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(command_failure(&output));
    }

    let run = Command::new(host)
        .env("ASAN_OPTIONS", "detect_leaks=0:halt_on_error=1")
        .env("UBSAN_OPTIONS", "halt_on_error=1")
        .output()
        .map_err(|error| format!("linked sanitizer probe could not run: {error}"))?;
    if run.status.success() {
        Ok(())
    } else {
        Err(format!(
            "linked sanitizer probe failed to run: {}",
            command_failure(&run)
        ))
    }
}

fn sanitizer_command(compiler: &OsString) -> Command {
    let mut command = Command::new(compiler);
    command
        .arg("-std=c11")
        .arg("-fsanitize=address,undefined")
        .arg("-fno-sanitize-recover=all")
        .arg("-fno-omit-frame-pointer");
    command
}

fn sanitizers_are_required() -> bool {
    std::env::var_os(REQUIRE_SANITIZERS_ENV).is_some_and(|value| value == "1")
}

fn assert_success(action: &str, output: &Output) {
    assert!(
        output.status.success(),
        "failed while {action}:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn command_failure(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("exited with {}", output.status)
    } else {
        format!("exited with {}: {stderr}", output.status)
    }
}
