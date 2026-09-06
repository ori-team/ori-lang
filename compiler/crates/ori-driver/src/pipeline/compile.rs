use ori_diagnostics::{Diagnostic, DiagnosticSink, SourceCache};
use std::path::{Path, PathBuf};

use super::debug::collect_native_debug_functions;
use super::frontend::check_loaded_sources;
use super::lowering::{lower_loaded_sources, split_native_modules};
use super::project::load_and_resolve;
use super::runtime::{
    ensure_native_codegen_target, find_native_runtime_link, native_lib_static_name,
    native_target_triple,
};
use super::timing::report_internal_pipeline_timing;

pub struct CompileOutput {
    pub cache: SourceCache,
    pub exe_path: PathBuf,
    /// Generated public C header for `ori compile --lib`.
    pub header_path: Option<PathBuf>,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    /// True when the native output was reused from `.ori/incremental.json`.
    pub reused: bool,
    /// Native DWARF/PDB sidecar generated for this build, when available.
    pub debug_path: Option<PathBuf>,
    /// Number of source modules that changed since the last successful build.
    /// This is diagnostic metadata; native codegen still emits one link unit.
    pub changed_modules: usize,
}

pub struct BuildOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub native_raw: bool,
    /// Emit a shared library with C ABI `@c_export` symbols (`ori compile --lib`).
    pub lib: bool,
}

/// Pipeline up to HIR lowering and validation (used by tests and compiler passes).
pub fn run_build(path: &Path) -> Result<BuildOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let sources = load_and_resolve(path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    if !sink.has_errors() {
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        if !sink.has_errors() {
            ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
        }
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(BuildOutput {
        cache,
        diagnostics,
        has_errors,
    })
}

/// Full pipeline → Cranelift object → linker → native binary.
pub fn run_compile_with_options(
    source_path: &Path,
    output: &Path,
    options: CompileOptions,
) -> Result<CompileOutput, String> {
    ensure_native_codegen_target()?;
    let incremental_options = crate::incremental::BuildOptions {
        shared: options.lib,
    };
    if let Some(hit) = crate::incremental::try_reuse(source_path, output, incremental_options)? {
        let pdb_path = hit.output.with_extension("pdb");
        let debug_map_path = hit.output.with_extension("debug.json");
        let debug_path = pdb_path
            .is_file()
            .then_some(pdb_path)
            .or_else(|| debug_map_path.is_file().then_some(debug_map_path));
        return Ok(CompileOutput {
            cache: SourceCache::default(),
            exe_path: hit.output,
            header_path: hit.header,
            diagnostics: Vec::new(),
            has_errors: false,
            reused: true,
            debug_path,
            changed_modules: 0,
        });
    }
    let changed_modules = crate::incremental::changed_modules(source_path, incremental_options)?
        .into_iter()
        .filter(|module| module.changed)
        .count();
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let mut generated_header_path = None;
    let mut generated_debug_path = None;
    let load_started = std::time::Instant::now();
    let sources = load_and_resolve(source_path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;
    let import_context = sources.imports;
    report_internal_pipeline_timing("compile.load_and_resolve", load_started.elapsed());

    let check_started = std::time::Instant::now();
    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }
    report_internal_pipeline_timing("compile.type_check", check_started.elapsed());

    if !sink.has_errors() {
        let lower_started = std::time::Instant::now();
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        report_internal_pipeline_timing("compile.hir_lower", lower_started.elapsed());
        // Lowering can also reject the program (e.g. recursive iterators);
        // never write an object or a binary once anything failed.
        if !sink.has_errors() {
            // LANG-PERF-2: HIR mid-end before native lower (ORI_OPT=none|default|aggressive).
            let optimize_started = std::time::Instant::now();
            ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
            report_internal_pipeline_timing("compile.hir_optimize", optimize_started.elapsed());
            let generated_header = if options.lib {
                let header_path = output.with_extension("h");
                let header_file_name = header_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("ori_generated.h")
                    .to_owned();
                Some((
                    header_path,
                    ori_codegen::generate_c_header(&hir, &header_file_name)?,
                ))
            } else {
                None
            };
            let mut runtime_link = find_native_runtime_link()?;
            let target = native_target_triple();
            for lib in import_context.native_libs {
                let lib_name = native_lib_static_name(&target, &lib.name);
                let lib_path = lib.package_root.join("lib").join(&target).join(lib_name);
                runtime_link
                    .native_static_libs
                    .push(lib_path.to_string_lossy().to_string());
            }
            for config_ctx in import_context.native_configs {
                let resolved = crate::native_deps::resolve_native_config(
                    &config_ctx.config,
                    &target,
                    &config_ctx.package_root,
                )?;
                for arg in resolved.to_link_args(&target) {
                    runtime_link.native_static_libs.push(arg);
                }
            }

            let interface_fingerprint = crate::incremental::interface_fingerprint(&hir);
            let split_modules = !options.lib
                && std::env::var_os("ORI_DEBUG_INSTRUMENT").is_none()
                && !ori_codegen::has_runtime_global_initializers(&hir);
            let module_units = split_modules.then(|| split_native_modules(&hir, &loaded));
            let can_split_modules = module_units.as_ref().is_some_and(|units| {
                !units.is_empty()
                    && units
                        .iter()
                        .map(|(_, module)| module.funcs.len())
                        .sum::<usize>()
                        == hir.funcs.len()
            });
            let mut object_paths = Vec::new();
            let mut cached_artifacts = Vec::new();
            let mut temporary_objects = Vec::new();
            let codegen_started = std::time::Instant::now();
            if can_split_modules {
                for (index, (module_path, module_hir)) in module_units
                    .expect("split module units were checked above")
                    .into_iter()
                    .enumerate()
                {
                    let module_fingerprint = crate::incremental::module_fingerprint(&module_path)?;
                    let cached_path = crate::incremental::module_artifact_path(
                        source_path,
                        &module_path,
                        &module_fingerprint,
                        &interface_fingerprint,
                        incremental_options,
                    );
                    let object_path = if crate::incremental::cache_enabled() {
                        if !cached_path.is_file() {
                            if let Some(parent) = cached_path.parent() {
                                std::fs::create_dir_all(parent).map_err(|error| {
                                    format!(
                                        "create module cache {} failed: {error}",
                                        parent.display()
                                    )
                                })?;
                            }
                            ori_codegen::emit_native_with_options(
                                &module_hir,
                                &cached_path,
                                ori_codegen::NativeEmitOptions { lib: false },
                            )?;
                        }
                        cached_path
                    } else {
                        let extension = if cfg!(windows) { "obj" } else { "o" };
                        let stem = output
                            .file_stem()
                            .and_then(|name| name.to_str())
                            .unwrap_or("ori");
                        let temporary = output
                            .parent()
                            .unwrap_or_else(|| Path::new("."))
                            .join(format!(".{stem}.module-{index}.{extension}"));
                        ori_codegen::emit_native_with_options(
                            &module_hir,
                            &temporary,
                            ori_codegen::NativeEmitOptions { lib: false },
                        )?;
                        temporary_objects.push(temporary.clone());
                        temporary
                    };
                    object_paths.push(object_path.clone());
                    cached_artifacts.push(crate::incremental::ModuleArtifact {
                        path: module_path,
                        fingerprint: module_fingerprint,
                        object: object_path,
                    });
                }
            } else {
                let extension = if cfg!(windows) { "obj" } else { "o" };
                let obj_path = output.with_extension(extension);
                ori_codegen::emit_native_with_options(
                    &hir,
                    &obj_path,
                    ori_codegen::NativeEmitOptions { lib: options.lib },
                )?;
                temporary_objects.push(obj_path.clone());
                object_paths.push(obj_path);
            }
            report_internal_pipeline_timing("compile.native_codegen", codegen_started.elapsed());
            let extra = runtime_link.link_args_for(options.lib);
            let link_started = std::time::Instant::now();
            ori_codegen::link_many_with_options(
                &object_paths,
                output,
                &extra,
                ori_codegen::NativeLinkOptions {
                    raw_diagnostics: options.native_raw,
                    shared: options.lib,
                },
            )?;
            let debug_functions = collect_native_debug_functions(&hir, &loaded);
            // Debug metadata is an auxiliary artifact: a missing `objcopy`, an
            // unusual linker format, or a read-only output directory must not
            // turn an otherwise valid native build into a failed build.  The
            // JSON source map is written before platform-specific DWARF/PDB
            // work, so retain it when the richer format is unavailable.
            generated_debug_path =
                match ori_codegen::emit_native_debug_symbols(output, &debug_functions) {
                    Ok(path) => path,
                    Err(error) => {
                        eprintln!("ori: warning: {error}");
                        let map_path = output.with_extension("debug.json");
                        map_path.is_file().then_some(map_path)
                    }
                };
            report_internal_pipeline_timing("compile.native_link", link_started.elapsed());
            for temporary in temporary_objects {
                let _ = std::fs::remove_file(temporary);
            }
            if let Some((header_path, header)) = generated_header {
                std::fs::write(&header_path, header)
                    .map_err(|error| format!("write {} failed: {error}", header_path.display()))?;
                generated_header_path = Some(header_path);
            }
            crate::incremental::record_success_with_artifacts(
                source_path,
                output,
                generated_header_path.as_deref(),
                if can_split_modules {
                    &cached_artifacts
                } else {
                    &[]
                },
                incremental_options,
            )?;
        }
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(CompileOutput {
        cache,
        exe_path: output.to_owned(),
        header_path: generated_header_path,
        diagnostics,
        has_errors,
        reused: false,
        debug_path: generated_debug_path,
        changed_modules,
    })
}
