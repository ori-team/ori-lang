use ori_ast::item::Item;
use ori_diagnostics::{Diagnostic, DiagnosticSink, Label, SourceCache};
use ori_hir::{HirArg, HirBlock, HirExpr, HirExprKind, HirFunc, HirModule, HirStmt};
use ori_types::{DefId, Ty};
use std::path::{Path, PathBuf};

use super::frontend::{check_loaded_sources, CheckOptions};
use super::lowering::lower_loaded_sources;
use super::project::{load_and_resolve, namespace_of, LoadedSource};
use super::runtime::{
    find_native_runtime_cdylib, find_native_runtime_link, native_lib_cdylib_name,
    native_target_triple,
};

pub struct TestOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    pub results: Vec<TestResult>,
    pub discovered: usize,
    pub selected: usize,
    pub filter: Option<String>,
}

pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    pub stdout: String,
    pub stderr: String,
    pub status: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct TestOptions {
    pub filter: Option<String>,
}

/// Output of a JIT `run` (Rust removal Phase 3). When `has_errors` is false,
/// `exit_code` is the value returned by the Ori `main` wrapper executed
/// in-process via Cranelift JIT. When the Ori program calls `os.exit(code)`,
/// the driver process terminates immediately with `code` and this struct is
/// never returned.
pub struct JitRunOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    pub exit_code: i32,
}

/// Result of compiling an in-memory source graph into a persistent JIT module.
///
/// The module owns its finalized Cranelift code and must remain alive for any
/// host function handle derived from it. Source diagnostics are returned in the
/// same shape as the check pipeline; backend/runtime failures remain `Err`.
pub struct JitCompileOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    pub module: Option<ori_codegen::CompiledJitModule>,
}

/// Checked and lowered source graph ready for persistent JIT finalization.
///
/// The HIR is kept separate from the Cranelift module so callers that need a
/// large frontend stack can perform parsing/type checking on a worker thread,
/// then finalize executable code on their owning thread.
pub struct JitLowerOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    pub hir: Option<HirModule>,
    pub cdylib: Option<PathBuf>,
    pub native_libs: Vec<PathBuf>,
}

#[derive(Clone)]
pub(super) struct TestCase {
    name: String,
    span: ori_diagnostics::Span,
    is_async: bool,
}

/// Discover, filter, lower, and execute `@test` functions through the native
/// backend. Keeping this orchestration beside the native harness makes the
/// pipeline facade responsible only for the public command contract.
pub fn run_test(path: &Path) -> Result<TestOutput, String> {
    run_test_with_options(path, TestOptions::default())
}

pub fn run_test_with_options(path: &Path, options: TestOptions) -> Result<TestOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let filter = options
        .filter
        .map(|filter| filter.trim().to_string())
        .filter(|filter| !filter.is_empty());
    let sources = load_and_resolve(path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    let tests = if !sink.has_errors() {
        collect_test_cases(&loaded, &resolved, &mut sink)
    } else {
        Vec::new()
    };
    let discovered = tests.len();
    let selected_tests = filter_test_cases(tests, filter.as_deref());
    let selected = selected_tests.len();

    let results = if !sink.has_errors() && !selected_tests.is_empty() {
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        if sink.has_errors() {
            Vec::new()
        } else {
            ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
            run_native_tests(&hir, &selected_tests)?
        }
    } else {
        Vec::new()
    };

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(TestOutput {
        cache,
        diagnostics,
        has_errors,
        results,
        discovered,
        selected,
        filter,
    })
}

fn collect_test_cases(
    loaded: &[LoadedSource],
    resolved: &ori_types::resolve::ResolvedModule,
    sink: &mut DiagnosticSink,
) -> Vec<TestCase> {
    let mut tests = Vec::new();
    for source in loaded {
        let namespace = namespace_of(&source.ast);
        for item in &source.ast.items {
            if !item.attrs.iter().any(|attr| attr.name.text == "test") {
                continue;
            }
            let Item::Func(func) = &item.item else {
                continue;
            };
            let name = format!("{}.{}", namespace, func.name.text);
            let Some(def_id) = resolved.def_map.lookup(&name) else {
                continue;
            };
            let Some(sig) = resolved.func_sigs.iter().find(|sig| sig.def_id == def_id) else {
                continue;
            };
            let valid_return = if func.is_async {
                sig.return_ty == Ty::Future(Box::new(Ty::Void))
            } else {
                sig.return_ty == Ty::Void
            };
            if !func.type_params.is_empty() || !sig.params.is_empty() || !valid_return {
                sink.emit(
                    Diagnostic::error(
                        "attr.invalid_test_signature",
                        format!("test function `{}` has an invalid signature", func.name.text),
                    )
                    .with_label(Label::primary(
                        source.file_id,
                        func.span,
                        "test functions must be concrete functions with no parameters and no return value",
                    ))
                    .with_action(
                        "use `@test` on a function shaped like `test_name() ... end` or `async test_name() ... end`",
                    ),
                );
                continue;
            }
            tests.push(TestCase {
                name,
                span: func.span,
                is_async: func.is_async,
            });
        }
    }
    tests
}

fn filter_test_cases(tests: Vec<TestCase>, filter: Option<&str>) -> Vec<TestCase> {
    let Some(filter) = filter.map(str::trim).filter(|filter| !filter.is_empty()) else {
        return tests;
    };
    tests
        .into_iter()
        .filter(|test| test.name.contains(filter) || test.name.rsplit('.').next() == Some(filter))
        .collect()
}

/// JIT execution pipeline: lex -> parse -> resolve -> type-check -> lower HIR
/// -> Cranelift JIT -> invoke `main` in-process. No `.o` file, no linker, no
/// subprocess. The runtime `ori_*` symbols are resolved from the staged
/// cdylib via `libloading`.
pub fn run_jit(source_path: &Path) -> Result<JitRunOutput, String> {
    run_jit_with_args(source_path, &[])
}

/// Run JIT with custom arguments forwarded to `ori.os.args` / `ori.args`.
pub fn run_jit_with_args(source_path: &Path, args: &[String]) -> Result<JitRunOutput, String> {
    let mut cache = ori_diagnostics::SourceCache::default();
    let mut sink = ori_diagnostics::DiagnosticSink::default();
    let sources = load_and_resolve(source_path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;
    let import_context = sources.imports;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    let mut exit_code = 0;
    if !sink.has_errors() {
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        if !sink.has_errors() {
            ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
            let cdylib = find_native_runtime_cdylib()?;

            let target = native_target_triple();
            let mut native_libs = Vec::new();
            for lib in import_context.native_libs {
                let lib_name = native_lib_cdylib_name(&target, &lib.name);
                let lib_path = lib.package_root.join("lib").join(&target).join(lib_name);
                native_libs.push(lib_path);
            }

            let full_args: Vec<String> = std::iter::once(source_path.display().to_string())
                .chain(args.iter().cloned())
                .collect();
            ori_runtime::set_custom_args(Some(full_args.clone()));
            let result = ori_codegen::run_jit_with_args(&hir, &cdylib, &native_libs, &full_args);
            ori_runtime::set_custom_args(None);
            exit_code = result?;
        }
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(JitRunOutput {
        cache,
        diagnostics,
        has_errors,
        exit_code,
    })
}

/// Compile an in-memory source graph into a finalized persistent JIT module.
///
/// This is the compiler-side foundation for hosted sessions. It intentionally
/// does not execute `main`; callers choose whether to keep the module and
/// invoke an explicitly supported function or use the legacy `run_jit` path.
pub fn compile_jit_source_with_options(
    path: &Path,
    source: String,
    options: CheckOptions,
) -> Result<JitCompileOutput, String> {
    let lowered = lower_jit_source_with_options(path, source, options)?;
    let JitLowerOutput {
        cache,
        diagnostics,
        has_errors,
        hir,
        cdylib,
        native_libs,
    } = lowered;
    let module = match (has_errors, hir, cdylib) {
        (false, Some(hir), Some(cdylib)) => Some(ori_codegen::CompiledJitModule::compile(
            &hir,
            &cdylib,
            &native_libs,
        )?),
        _ => None,
    };
    Ok(JitCompileOutput {
        cache,
        diagnostics,
        has_errors,
        module,
    })
}

/// Check and lower an in-memory source graph without transferring JIT code
/// across the compiler's large-stack worker boundary.
pub fn lower_jit_source_with_options(
    path: &Path,
    source: String,
    options: CheckOptions,
) -> Result<JitLowerOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let sources = super::project::load_and_resolve_with_entry_source_and_cfg(
        path,
        source,
        options.cfg,
        &mut cache,
        &mut sink,
    )?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;
    let import_context = sources.imports;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    let (hir, cdylib, native_libs) = if !sink.has_errors() {
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        if sink.has_errors() {
            (None, None, Vec::new())
        } else {
            ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
            let cdylib = find_native_runtime_cdylib()?;
            let target = native_target_triple();
            let native_libs = import_context
                .native_libs
                .iter()
                .map(|lib| {
                    lib.package_root
                        .join("lib")
                        .join(&target)
                        .join(native_lib_cdylib_name(&target, &lib.name))
                })
                .collect::<Vec<_>>();
            (Some(hir), Some(cdylib), native_libs)
        }
    } else {
        (None, None, Vec::new())
    };

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(JitLowerOutput {
        cache,
        diagnostics,
        has_errors,
        hir,
        cdylib,
        native_libs,
    })
}

pub(super) fn run_native_tests(
    hir: &HirModule,
    tests: &[TestCase],
) -> Result<Vec<TestResult>, String> {
    let runtime_link = find_native_runtime_link()?;
    let mut results = Vec::new();

    for test in tests {
        let (obj_path, exe_path) = temp_test_paths();
        let mut test_hir = hir.clone();
        inject_test_harness(&mut test_hir, test);

        let run_result = (|| {
            ori_codegen::emit_native(&test_hir, &obj_path)?;
            let extra = runtime_link.link_args();
            ori_codegen::link(&obj_path, &exe_path, &extra)?;
            let output = std::process::Command::new(&exe_path)
                .output()
                .map_err(|e| format!("failed to run test `{}`: {e}", test.name))?;
            Ok::<TestResult, String>(TestResult {
                name: test.name.clone(),
                passed: output.status.success() || output.status.code() == Some(77),
                skipped: output.status.code() == Some(77),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                status: output.status.code(),
            })
        })();

        let _ = std::fs::remove_file(&obj_path);
        let _ = std::fs::remove_file(&exe_path);

        match run_result {
            Ok(result) => results.push(result),
            Err(error) => {
                results.push(TestResult {
                    name: test.name.clone(),
                    passed: false,
                    skipped: false,
                    stdout: String::new(),
                    stderr: error,
                    status: Some(1),
                });
            }
        }
    }

    Ok(results)
}

fn inject_test_harness(module: &mut HirModule, test: &TestCase) {
    let span = test.span;
    let test_ret_ty = if test.is_async {
        Ty::Future(Box::new(Ty::Void))
    } else {
        Ty::Void
    };
    let callee_ty = Ty::Func {
        params: Vec::new(),
        ret: Box::new(test_ret_ty.clone()),
    };
    let call = HirExpr {
        kind: HirExprKind::Call {
            callee: Box::new(HirExpr {
                kind: HirExprKind::Var(test.name.as_str().into()),
                ty: callee_ty,
                span,
            }),
            args: Vec::new(),
        },
        ty: test_ret_ty.clone(),
        span,
    };
    let test_expr = if test.is_async {
        HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(HirExpr {
                    kind: HirExprKind::Var("ori_task_block_on".into()),
                    ty: Ty::Func {
                        params: vec![test_ret_ty.clone()],
                        ret: Box::new(Ty::Void),
                    },
                    span,
                }),
                args: vec![HirArg {
                    label: None,
                    spread: false,
                    value: call,
                }],
            },
            ty: Ty::Void,
            span,
        }
    } else {
        call
    };
    let harness_name = if module.namespace.is_empty() {
        "main".to_string()
    } else {
        format!("{}.main", module.namespace)
    };
    let harness = HirFunc {
        def_id: DefId(u32::MAX - 1),
        name: harness_name.into(),
        params: Vec::new(),
        return_ty: Ty::Void,
        body: HirBlock {
            stmts: vec![HirStmt::Expr(test_expr)],
            span,
        },
        closure_captures: Vec::new(),
        is_public: false,
        is_async: false,
        is_mut: false,
        c_export_name: None,
        span,
    };
    module.funcs.insert(0, harness);
}

fn temp_test_paths() -> (PathBuf, PathBuf) {
    static NEXT_TEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let id = NEXT_TEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stem = format!("ori_test_{}_{}", std::process::id(), id);
    let tmp_dir = std::env::temp_dir();
    let obj_path = tmp_dir.join(format!("{stem}.o"));
    let exe_name = if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem
    };
    (obj_path, tmp_dir.join(exe_name))
}
