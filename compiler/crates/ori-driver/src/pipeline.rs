//! Public driver façade.
//!
//! Each child module owns one pipeline boundary and its result types. This
//! file coordinates the few cross-stage policies that must remain shared and
//! re-exports the stable CLI/LSP contract. Keeping the façade free of
//! documentation, formatting, loading, and backend implementation details
//! prevents new features from growing another monolith.

use std::path::{Path, PathBuf};

mod compile;
mod daemon;
mod debug;
mod doc_html;
mod docs;
mod doctest;
mod fmt;
mod frontend;
mod lint;
mod lowering;
mod migrate_syntax;
mod native;
mod project;
mod report;
mod runtime;
mod timing;

pub use compile::{
    run_build, run_compile_with_options, run_emit_c, BuildOutput, CompileOptions, CompileOutput,
};
pub use daemon::run_daemon;
pub use docs::{
    oridoc_hover_for_symbol, run_doc, run_doc_check, run_doc_with_options, stdlib_doc_signature,
};
pub use docs::{DocCheckOutput, DocFormat, DocOptions, DocOutput};
pub use doctest::{extract_doctests, run_doctests, DocTestCase};
pub use fmt::{format_source_text, run_fmt, run_fmt_path, FmtBatchOutput, FmtOptions, FmtOutput};
pub use frontend::{
    run_check, run_check_source, run_check_source_with_options, run_lex, run_parse, CheckOptions,
    CheckOutput, LexOutput, ParseOutput,
};
pub use lint::{run_lint, run_lint_source, LintOutput};
pub use native::{
    compile_jit_source_with_options, lower_jit_source_with_options, run_jit, run_jit_with_args,
    run_test, run_test_with_options, JitCompileOutput, JitLowerOutput, JitRunOutput, TestOptions,
    TestOutput, TestResult,
};
pub use project::{
    filter_intrinsic_source_for_current_configuration, filter_source_for_current_configuration,
    find_stdlib_root, stdlib_source_path,
};
pub use project::{run_new_project, NewProjectKind, NewProjectOptions, NewProjectOutput};
pub use report::{
    format_summary_text, run_doctor, run_summary, DoctorCheck, DoctorReport, DoctorStatus,
    SummaryImport, SummaryImportItem, SummaryModule, SummaryOutput,
};
pub(crate) use runtime::native_target_triple;
#[cfg(test)]
use runtime::{
    cargo_workspace_root, missing_native_runtime_message, native_runtime_artifact_name,
    native_runtime_link_for, native_static_libs_for_target, read_runtime_link_metadata, repo_root,
    runtime_link_metadata_json, ORI_DRIVER_ABI_VERSION, ORI_VERSION,
};
pub use runtime::{env_flag, should_use_jit_for_run};

pub use migrate_syntax::{
    migrate_source, run_migrate_syntax, MigrateSyntaxOptions, MigrateSyntaxReport,
    MigrateTextResult, MigratedFile,
};

/// Full pipeline â†’ Cranelift object â†’ linker â†’ native binary.
/// Stack the recursive front end needs to reach the parser's nesting bound.
///
/// The parser, resolver, and type checker all descend one frame per syntactic
/// level, and those frames are large in an unoptimised build. Platform defaults
/// vary too much to rely on (8 MiB for a Linux main thread, 1 MiB on Windows,
/// 2 MiB for a spawned thread), so any host that runs the pipeline off the main
/// thread — the language server, a test harness — must request at least this
/// much through [`with_frontend_stack`].
pub const FRONTEND_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Run `job` on a thread sized by [`FRONTEND_STACK_SIZE`], propagating a panic
/// in `job` to the caller.
pub fn with_frontend_stack<T, F>(job: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let worker = std::thread::Builder::new()
        .stack_size(FRONTEND_STACK_SIZE)
        .spawn(job)
        .expect("failed to spawn the compiler worker thread");
    match worker.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

pub fn run_compile(source_path: &Path, output: &Path) -> Result<CompileOutput, String> {
    run_compile_with_options(source_path, output, CompileOptions::default())
}

/// Default shared-library output path for `ori compile --lib`.
pub fn default_shared_lib_path(source: &Path) -> PathBuf {
    // A project root or an `ori.proj` manifest names the *directory* the
    // library belongs in, not the library itself; only a plain source file
    // contributes its stem.
    let is_manifest = source.file_name().and_then(|name| name.to_str()) == Some("ori.proj");
    let (directory, stem) = if source.is_dir() {
        (source.to_path_buf(), None)
    } else if is_manifest {
        (
            source.parent().unwrap_or(Path::new(".")).to_path_buf(),
            None,
        )
    } else {
        (
            source.parent().unwrap_or(Path::new(".")).to_path_buf(),
            source.file_stem().and_then(|s| s.to_str()),
        )
    };
    let stem = stem.unwrap_or("ori_lib");
    let file_name = if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    };
    directory.join(file_name)
}

/// Project-oriented native build route used by `ori build`.
pub fn run_build_native(
    source_path: &Path,
    output: &Path,
    options: CompileOptions,
) -> Result<CompileOutput, String> {
    run_compile_with_options(source_path, output, options)
}

// The native route now links against the Rust ori-runtime static library. C
// emission remains available only as the explicit debug route `ori emit c`.

#[cfg(test)]
mod tests {
    use super::{
        cargo_workspace_root, missing_native_runtime_message, native_runtime_artifact_name,
        native_runtime_link_for, native_static_libs_for_target, read_runtime_link_metadata,
        repo_root, runtime_link_metadata_json, ORI_DRIVER_ABI_VERSION, ORI_VERSION,
    };

    fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let start_index = source
            .find(start)
            .unwrap_or_else(|| panic!("source marker `{start}` not found"));
        let tail = &source[start_index..];
        let end_index = tail
            .find(end)
            .unwrap_or_else(|| panic!("source marker `{end}` not found after `{start}`"));
        &tail[..end_index]
    }

    #[test]
    fn runtime_bootstrap_roots_point_at_real_directories() {
        // The fallback `cargo build -p ori-runtime` must run inside the Cargo
        // workspace (`<repo>/compiler`), not the repo root — CI has no staged
        // runtime and died with "could not find Cargo.toml" when this drifted.
        assert!(
            cargo_workspace_root().join("Cargo.toml").is_file(),
            "cargo_workspace_root() must contain the workspace manifest: {}",
            cargo_workspace_root().display()
        );
        // Staged runtime artifacts live at `<repo>/runtime/<target>/`.
        assert!(
            repo_root().join("runtime").is_dir(),
            "repo_root() must contain the staged runtime dir: {}",
            repo_root().display()
        );
    }

    #[test]
    fn native_compile_and_test_pipeline_do_not_use_legacy_c_runtime_hooks() {
        let source = include_str!("pipeline.rs");
        // The legacy `ORI_RUNTIME_C` env var (pointing at a C runtime path) was
        // removed when the native pipeline switched to `find_native_runtime_link`.
        // We match `ORI_RUNTIME_C` followed by a closing quote so the new
        // `ORI_RUNTIME_CDYLIB` env var (Rust removal Phase 3, JIT runtime
        // resolution) is not flagged. The `concat!` split prevents the test
        // from matching its own source text.
        for forbidden in [
            concat!("ensure_", "cc_available"),
            concat!("build_", "runtime_lib"),
            concat!("ORI_", "RUNTIME_C", "\""),
        ] {
            assert!(
                !source.contains(forbidden),
                "native pipeline must not contain legacy C runtime hook `{forbidden}`"
            );
        }

        let native_source = include_str!("pipeline/native.rs");
        let run_test = source_section(
            native_source,
            concat!("pub fn ", "run_test"),
            concat!("pub(super) fn ", "run_native_tests"),
        );
        assert!(run_test.contains("run_native_tests"), "{run_test}");
        assert!(!run_test.contains("emit_c"), "{run_test}");

        let native_tests = source_section(
            native_source,
            concat!("fn ", "run_native_tests"),
            concat!("fn ", "inject_test_harness"),
        );
        assert!(
            native_tests.contains("find_native_runtime_link"),
            "{native_tests}"
        );
        assert!(
            native_tests.contains("ori_codegen::emit_native"),
            "{native_tests}"
        );
        assert!(native_tests.contains("ori_codegen::link"), "{native_tests}");
        assert!(!native_tests.contains("emit_c"), "{native_tests}");
    }

    #[test]
    fn native_pipeline_text_does_not_require_a_c_compiler() {
        let source = include_str!("pipeline.rs");
        for forbidden in [
            concat!("C ", "compiler"),
            concat!("C ", "toolchain"),
            concat!("requires `", "cc`"),
        ] {
            assert!(
                !source.contains(forbidden),
                "native pipeline text must not expose `{forbidden}` as a requirement"
            );
        }
    }

    #[test]
    fn native_runtime_artifact_names_are_platform_specific() {
        assert_eq!(
            native_runtime_artifact_name("x86_64-pc-windows-msvc"),
            "ori_runtime.lib"
        );
        assert_eq!(
            native_runtime_artifact_name("x86_64-pc-windows-gnu"),
            "libori_runtime.a"
        );
        assert_eq!(
            native_runtime_artifact_name("x86_64-unknown-linux-gnu"),
            "libori_runtime.a"
        );
    }

    #[test]
    fn runtime_link_metadata_names_rust_runtime_artifact() {
        let json = runtime_link_metadata_json(
            "x86_64-pc-windows-msvc",
            native_runtime_artifact_name("x86_64-pc-windows-msvc"),
        );

        assert!(json.contains("\"target\": \"x86_64-pc-windows-msvc\""));
        assert!(json.contains("\"runtime\": \"ori_runtime.lib\""));
        assert!(json.contains(&format!("\"ori_version\": \"{ORI_VERSION}\"")));
        assert!(json.contains(&format!("\"abi_version\": \"{ORI_DRIVER_ABI_VERSION}\"")));
        assert!(json.contains("legacy_stdio_definitions.lib"));
    }

    #[test]
    fn runtime_link_metadata_parser_reads_native_static_libs() {
        let dir = std::env::temp_dir().join(format!(
            "ori_runtime_link_metadata_parser_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let metadata_path = dir.join("runtime-link.json");
        std::fs::write(
            &metadata_path,
            runtime_link_metadata_json("x86_64-pc-windows-msvc", "ori_runtime.lib"),
        )
        .unwrap();

        let metadata = read_runtime_link_metadata(&metadata_path).unwrap();
        assert_eq!(metadata.target, "x86_64-pc-windows-msvc");
        assert_eq!(metadata.runtime, "ori_runtime.lib");
        assert_eq!(metadata.ori_version, ORI_VERSION);
        assert_eq!(metadata.abi_version, ORI_DRIVER_ABI_VERSION);
        assert!(metadata
            .native_static_libs
            .contains(&"kernel32.lib".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn packaged_runtime_link_reads_sibling_metadata() {
        let dir =
            std::env::temp_dir().join(format!("ori_packaged_runtime_link_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let runtime = dir.join("ori_runtime.lib");
        std::fs::write(&runtime, b"fake runtime").unwrap();
        std::fs::write(
            dir.join("runtime-link.json"),
            runtime_link_metadata_json("x86_64-pc-windows-msvc", "ori_runtime.lib"),
        )
        .unwrap();

        let link =
            native_runtime_link_for(runtime.clone(), "x86_64-pc-windows-msvc", "ori_runtime.lib")
                .unwrap();
        let args = link.link_args();

        assert_eq!(args.first(), Some(&runtime));
        assert!(link
            .native_static_libs
            .contains(&"kernel32.lib".to_string()));
        assert!(args
            .iter()
            .any(|arg| arg == std::path::Path::new("kernel32.lib")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runtime_target_mismatch_error_names_expected_and_actual_targets() {
        let dir = std::env::temp_dir().join(format!(
            "ori_runtime_target_mismatch_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let runtime = dir.join("ori_runtime.lib");
        std::fs::write(&runtime, b"fake runtime").unwrap();
        std::fs::write(
            dir.join("runtime-link.json"),
            runtime_link_metadata_json("x86_64-unknown-linux-gnu", "ori_runtime.lib"),
        )
        .unwrap();

        let err = native_runtime_link_for(runtime, "x86_64-pc-windows-msvc", "ori_runtime.lib")
            .expect_err("target mismatch should fail");

        assert!(err.contains("native.runtime_metadata_mismatch"), "{err}");
        assert!(err.contains("x86_64-unknown-linux-gnu"), "{err}");
        assert!(err.contains("x86_64-pc-windows-msvc"), "{err}");
        assert!(err.contains("runtime metadata"), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runtime_abi_version_is_shared_between_runtime_and_driver() {
        assert_eq!(ORI_DRIVER_ABI_VERSION, ori_runtime::ORI_ABI_VERSION);
        assert!(!ORI_DRIVER_ABI_VERSION.trim().is_empty());
    }

    #[test]
    fn runtime_abi_mismatch_error_has_stable_code() {
        let dir =
            std::env::temp_dir().join(format!("ori_runtime_abi_mismatch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let runtime = dir.join("ori_runtime.lib");
        std::fs::write(&runtime, b"fake runtime").unwrap();
        let json = runtime_link_metadata_json("x86_64-pc-windows-msvc", "ori_runtime.lib")
            .replace(ORI_DRIVER_ABI_VERSION, "ori-native-abi-test-mismatch");
        std::fs::write(dir.join("runtime-link.json"), json).unwrap();

        let err = native_runtime_link_for(runtime, "x86_64-pc-windows-msvc", "ori_runtime.lib")
            .expect_err("ABI mismatch should fail");

        assert!(err.contains("native.abi_mismatch"), "{err}");
        assert!(err.contains(ORI_DRIVER_ABI_VERSION), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_runtime_error_names_target_path_and_staging_command() {
        let searched = [
            std::path::PathBuf::from("package/runtime/x86_64-pc-windows-msvc/ori_runtime.lib"),
            std::path::PathBuf::from("target/debug/ori_runtime.lib"),
        ];
        let message = missing_native_runtime_message(
            "x86_64-pc-windows-msvc",
            "ori_runtime.lib",
            &searched,
            true,
        );

        assert!(message.contains("native.runtime_missing"), "{message}");
        assert!(message.contains("ori_runtime.lib"), "{message}");
        assert!(message.contains("x86_64-pc-windows-msvc"), "{message}");
        assert!(
            message.contains("runtime/x86_64-pc-windows-msvc/ori_runtime.lib"),
            "{message}"
        );
        assert!(message.contains("stage_native_runtime.ps1"), "{message}");
        assert!(
            message.contains("ORI_REQUIRE_PACKAGED_RUNTIME=1"),
            "{message}"
        );
        assert!(message.contains("package/runtime"), "{message}");
    }

    #[test]
    fn native_static_libs_are_known_for_msvc() {
        let libs = native_static_libs_for_target("x86_64-pc-windows-msvc");
        assert!(libs.contains(&"kernel32.lib"));
        assert!(libs.contains(&"/defaultlib:msvcrt"));
    }

    #[test]
    fn native_static_libs_are_known_for_linux() {
        let libs = native_static_libs_for_target("x86_64-unknown-linux-gnu");
        assert!(libs.contains(&"-lpthread"));
        assert!(libs.contains(&"-ldl"));
        assert!(libs.contains(&"-lm"));
        assert!(libs.contains(&"-no-pie"));
    }

    /// Parity guard: every module referenced by `COLLECTION_STDLIB_DOC_SIGNATURES`
    /// must be an implemented stdlib module according to the manifest-derived
    /// `is_implemented_stdlib_module`. Catches drift where a doc signature is
    /// added for a module that does not exist or is not importable.
    #[test]
    fn collection_stdlib_doc_signatures_reference_implemented_modules() {
        for entry in super::docs::COLLECTION_STDLIB_DOC_SIGNATURES {
            assert!(
                ori_types::stdlib::is_implemented_stdlib_module(entry.module),
                "COLLECTION_STDLIB_DOC_SIGNATURES references `{}` which is not an implemented stdlib module",
                entry.module
            );
        }
    }
}
