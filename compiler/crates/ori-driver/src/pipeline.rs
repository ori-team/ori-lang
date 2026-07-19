use ori_ast::common::{TypeParams, WhereClause};
use ori_ast::item::{ExternMember, Item, Param, SourceFile, TraitMember};
use ori_ast::ty::Type;
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, SourceCache, Span};
use ori_hir::{HirArg, HirBlock, HirExpr, HirExprKind, HirFunc, HirStmt};
use ori_lexer::{Token, TokenKind};
use ori_types::{resolve::ResolvedModule, DefId, Ty};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const ORI_VERSION: &str = env!("CARGO_PKG_VERSION");
const ORI_DRIVER_ABI_VERSION: &str = ori_runtime::ORI_ABI_VERSION;
const NATIVE_RUNTIME_MISSING: &str = "native.runtime_missing";
const NATIVE_RUNTIME_METADATA_INVALID: &str = "native.runtime_metadata_invalid";
const NATIVE_RUNTIME_METADATA_MISMATCH: &str = "native.runtime_metadata_mismatch";
const NATIVE_ABI_MISMATCH: &str = "native.abi_mismatch";

mod doc_html;
mod fmt;
mod migrate_syntax;

pub use migrate_syntax::{
    migrate_source, run_migrate_syntax, MigrateSyntaxOptions, MigrateSyntaxReport, MigrateTextResult,
    MigratedFile,
};

// â”€â”€ Output types â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub struct LexOutput {
    pub cache: SourceCache,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct ParseOutput {
    pub cache: SourceCache,
    pub ast: SourceFile,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct CheckOutput {
    pub cache: SourceCache,
    pub resolved: ResolvedModule,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DocFormat {
    #[default]
    Markdown,
    Html,
}

pub struct DocOptions {
    pub format: DocFormat,
}

impl Default for DocOptions {
    fn default() -> Self {
        Self {
            format: DocFormat::Markdown,
        }
    }
}

pub struct DocOutput {
    pub cache: SourceCache,
    pub markdown: String,
    pub html: String,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

pub struct DocCheckOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

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

pub struct FmtOutput {
    pub cache: SourceCache,
    pub formatted: String,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

struct LoadedSource {
    path: PathBuf,
    file_id: FileId,
    source: String,
    tokens: Vec<Token>,
    ast: SourceFile,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct ProjectConfig {
    manifest_path: PathBuf,
    root: PathBuf,
    name: Option<String>,
    version: Option<String>,
    kind: ProjectKind,
    entry: PathBuf,
    source_root: Option<PathBuf>,
    root_namespace: Option<String>,
    dependencies: Vec<ProjectDependency>,
    doc_paths: Vec<PathBuf>,
    doc_mode: ProjectDocMode,
    require_public_docs: DocRequirement,
}

#[derive(Clone, Debug)]
struct ProjectDependency {
    name: String,
    path: Option<PathBuf>,
    version: Option<String>,
    git: Option<crate::package::GitDependencySpec>,
}

#[derive(Clone, Debug, Default)]
struct ImportContext {
    dependencies: Vec<ImportDependency>,
    native_libs: Vec<NativeLibContext>,
}

#[derive(Clone, Debug)]
struct NativeLibContext {
    name: String,
    package_root: PathBuf,
}

#[derive(Clone, Debug)]
struct ImportDependency {
    name: String,
    root: PathBuf,
    entry: PathBuf,
    source_root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectKind {
    App,
    Lib,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewProjectKind {
    App,
    Lib,
}

#[derive(Clone, Debug)]
pub struct NewProjectOptions {
    pub name: Option<String>,
    pub kind: NewProjectKind,
    pub is_init: bool,
}

#[derive(Clone, Debug)]
pub struct NewProjectOutput {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub entry: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectDocMode {
    SidecarFirst,
    InlineFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocRequirement {
    Off,
    Warn,
    Error,
}

// â”€â”€ Pipeline steps â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Read `path` from disk, lex it and return the token stream.
pub fn run_lex(path: &Path) -> Result<LexOutput, String> {
    let source = read_file(path)?;
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let file_id = cache.add(path, source.clone());
    let tokens = ori_lexer::lex(&source, file_id, &mut sink);
    let diags = sink.into_diagnostics();
    Ok(LexOutput {
        cache,
        tokens,
        diagnostics: diags,
    })
}

/// Read + lex + parse. Returns the AST (possibly partial on errors).
pub fn run_parse(path: &Path) -> Result<ParseOutput, String> {
    let source = read_file(path)?;
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let file_id = cache.add(path, source.clone());
    let tokens = ori_lexer::lex(&source, file_id, &mut sink);
    let ast = ori_parser::parse(&tokens, &source, file_id, &mut sink);
    let diags = sink.into_diagnostics();
    Ok(ParseOutput {
        cache,
        ast,
        tokens,
        diagnostics: diags,
    })
}

pub fn run_fmt(path: &Path) -> Result<FmtOutput, String> {
    let source = read_file(path)?;
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let file_id = cache.add(path, source.clone());
    let tokens = ori_lexer::lex(&source, file_id, &mut sink);
    let _ast = ori_parser::parse(&tokens, &source, file_id, &mut sink);
    let formatted = if !sink.has_errors() {
        format_source_text(&source)
    } else {
        String::new()
    };
    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(FmtOutput {
        cache,
        formatted,
        diagnostics,
        has_errors,
    })
}

pub struct CompileOutput {
    pub cache: SourceCache,
    pub exe_path: std::path::PathBuf,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub native_raw: bool,
    /// Emit a shared library with C ABI `@c_export` symbols (`ori compile --lib`).
    pub lib: bool,
}

#[derive(Clone, Debug)]
struct NativeRuntimeLink {
    runtime_lib: PathBuf,
    native_static_libs: Vec<String>,
}

impl NativeRuntimeLink {
    fn link_args(&self) -> Vec<PathBuf> {
        self.link_args_for(false)
    }

    /// Link line for executables (`shared = false`) or embeddable shared
    /// libraries (`shared = true`). Shared mode prefers the runtime **cdylib**
    /// (avoids rustc compiler-builtins vs libgcc clashes with `--whole-archive`
    /// on the staticlib) and drops executable-only flags like `-no-pie`.
    fn link_args_for(&self, shared: bool) -> Vec<PathBuf> {
        let mut args = Vec::with_capacity(1 + self.native_static_libs.len());
        if shared {
            let cdylib = runtime_cdylib_beside_static(&self.runtime_lib);
            if let Some(so) = cdylib {
                args.push(so);
            } else {
                // Fallback: staticlib without whole-archive (may miss unused symbols).
                args.push(self.runtime_lib.clone());
                args.push(PathBuf::from("-Wl,-u,ori_rt_init"));
                args.push(PathBuf::from("-Wl,-u,ori_rt_shutdown"));
            }
        } else {
            args.push(self.runtime_lib.clone());
        }
        for flag in &self.native_static_libs {
            if shared && (flag == "-no-pie" || flag == "-pie" || flag == "-lc") {
                continue;
            }
            args.push(PathBuf::from(flag));
        }
        args
    }
}

fn runtime_cdylib_beside_static(static_lib: &Path) -> Option<PathBuf> {
    let parent = static_lib.parent()?;
    let name = static_lib.file_name()?.to_str()?;
    // libori_runtime.a → libori_runtime.so / ori_runtime.dll / libori_runtime.dylib
    let so = if name.ends_with(".a") {
        let stem = name.trim_end_matches(".a");
        if cfg!(windows) {
            parent.join(format!("{}.dll", stem.trim_start_matches("lib")))
        } else if cfg!(target_os = "macos") {
            parent.join(format!("{stem}.dylib"))
        } else {
            parent.join(format!("{stem}.so"))
        }
    } else {
        return None;
    };
    so.is_file().then_some(so)
}

/// Full pipeline â†’ Cranelift object â†’ linker â†’ native binary.
pub fn run_compile(source_path: &Path, output: &Path) -> Result<CompileOutput, String> {
    run_compile_with_options(source_path, output, CompileOptions::default())
}

/// Default shared-library output path for `ori compile --lib`.
pub fn default_shared_lib_path(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ori_lib");
    let file_name = if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    };
    source
        .parent()
        .map(|p| p.join(&file_name))
        .unwrap_or_else(|| PathBuf::from(file_name))
}

pub fn run_compile_with_options(
    source_path: &Path,
    output: &Path,
    options: CompileOptions,
) -> Result<CompileOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved, import_context) = load_and_resolve(source_path, &mut cache, &mut sink)?;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    if !sink.has_errors() {
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        // LANG-PERF-2: HIR mid-end before native lower (ORI_OPT=none|default|aggressive).
        ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
        let obj_path = output.with_extension("o");
        let mut runtime_link = find_native_runtime_link()?;
        let target = native_target_triple();
        for lib in import_context.native_libs {
            let lib_name = native_lib_static_name(&target, &lib.name);
            let lib_path = lib.package_root.join("lib").join(&target).join(lib_name);
            runtime_link
                .native_static_libs
                .push(lib_path.to_string_lossy().to_string());
        }

        ori_codegen::emit_native_with_options(
            &hir,
            &obj_path,
            ori_codegen::NativeEmitOptions { lib: options.lib },
        )?;
        let extra = runtime_link.link_args_for(options.lib);
        ori_codegen::link_with_options(
            &obj_path,
            output,
            &extra,
            ori_codegen::NativeLinkOptions {
                raw_diagnostics: options.native_raw,
                shared: options.lib,
            },
        )?;
        let _ = std::fs::remove_file(&obj_path);
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(CompileOutput {
        cache,
        exe_path: output.to_owned(),
        diagnostics,
        has_errors,
    })
}

/// Project-oriented native build route used by `ori build`.
pub fn run_build_native(
    source_path: &Path,
    output: &Path,
    options: CompileOptions,
) -> Result<CompileOutput, String> {
    run_compile_with_options(source_path, output, options)
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

/// JIT execution pipeline: lex -> parse -> resolve -> type-check -> lower HIR
/// -> Cranelift JIT -> invoke `main` in-process. No `.o` file, no linker, no
/// subprocess. The runtime `ori_*` symbols are resolved from the staged
/// cdylib via `libloading`.
pub fn run_jit(source_path: &Path) -> Result<JitRunOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved, import_context) = load_and_resolve(source_path, &mut cache, &mut sink)?;

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

            exit_code = ori_codegen::run_jit(&hir, &cdylib, &native_libs)?;
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

/// Full pipeline: lex â†’ parse â†’ resolve names â†’ type-check.
fn find_native_runtime_link() -> Result<NativeRuntimeLink, String> {
    static CACHED: std::sync::OnceLock<Result<NativeRuntimeLink, String>> =
        std::sync::OnceLock::new();
    CACHED
        .get_or_init(find_native_runtime_link_uncached)
        .clone()
}

fn find_native_runtime_link_uncached() -> Result<NativeRuntimeLink, String> {
    if let Ok(path) = std::env::var("ORI_RUNTIME_LIB") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            let target = native_target_triple();
            let artifact = native_runtime_artifact_name(&target);
            native_runtime_link_for(path, &target, artifact)
        } else {
            Err(format!(
                "ORI_RUNTIME_LIB points to `{}`, but that file does not exist",
                path.display()
            ))
        };
    }

    let target = native_target_triple();
    let artifact = native_runtime_artifact_name(&target);
    let mut searched = Vec::new();
    let packaged_candidates = packaged_runtime_candidates(&target, artifact);

    for candidate in &packaged_candidates {
        if candidate.is_file() {
            return native_runtime_link_for(candidate.clone(), &target, artifact);
        }
    }
    searched.extend(packaged_candidates);

    if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") {
        return Err(missing_native_runtime_message(
            &target, artifact, &searched, true,
        ));
    }

    build_native_runtime_with_cargo()?;

    let cargo_candidates = cargo_runtime_candidates(&target, artifact);
    for candidate in &cargo_candidates {
        if candidate.is_file() {
            return native_runtime_link_for(candidate.clone(), &target, artifact);
        }
    }
    searched.extend(cargo_candidates);

    Err(missing_native_runtime_message(
        &target, artifact, &searched, false,
    ))
}

pub fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

/// Returns true when `ori run` should use the JIT path instead of AOT compile+link.
///
/// - Explicit opt-in: `ORI_USE_JIT=1`
/// - Explicit opt-out: `ORI_USE_AOT=1`
/// - Default: JIT when a runtime cdylib is available (packaged layout or cargo-built)
pub fn should_use_jit_for_run() -> bool {
    if env_flag("ORI_USE_AOT") {
        return false;
    }
    if env_flag("ORI_USE_JIT") {
        return true;
    }
    find_native_runtime_cdylib().is_ok()
}

fn find_native_runtime_cdylib() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("ORI_RUNTIME_CDYLIB") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "ORI_RUNTIME_CDYLIB points to `{}`, but that file does not exist",
                path.display()
            ))
        };
    }

    let target = native_target_triple();
    let cdylib_artifact = native_runtime_cdylib_name(&target);
    let mut searched = Vec::new();

    let packaged_candidates = packaged_runtime_candidates(&target, cdylib_artifact);
    for candidate in &packaged_candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    searched.extend(packaged_candidates);

    if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") {
        return Err(missing_native_runtime_message(
            &target,
            cdylib_artifact,
            &searched,
            true,
        ));
    }

    let cargo_candidates = cargo_runtime_candidates(&target, cdylib_artifact);
    for candidate in &cargo_candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    searched.extend(cargo_candidates);

    Err(missing_native_runtime_message(
        &target,
        cdylib_artifact,
        &searched,
        false,
    ))
}

fn missing_native_runtime_message(
    target: &str,
    artifact: &str,
    searched: &[PathBuf],
    packaged_only: bool,
) -> String {
    let mut message = format!(
        "{NATIVE_RUNTIME_MISSING}: native Ori runtime `{artifact}` for target `{target}` was not found."
    );
    if packaged_only {
        message.push_str(" Packaged runtime mode is enabled by ORI_REQUIRE_PACKAGED_RUNTIME=1.");
    }
    message.push_str(&format!(
        "\nexpected package path: runtime/{target}/{artifact}\nstaging command: .\\tools\\stage_native_runtime.ps1 -Target {target}"
    ));
    if !packaged_only {
        message.push_str("\nworkspace fallback: cargo build -p ori-runtime --lib");
    }
    if !searched.is_empty() {
        message.push_str("\nsearched paths:");
        for path in searched {
            message.push_str(&format!("\n- {}", path.display()));
        }
    }
    message
}

fn native_runtime_link_for(
    runtime_lib: PathBuf,
    target: &str,
    artifact: &str,
) -> Result<NativeRuntimeLink, String> {
    let metadata_path = runtime_lib
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime-link.json");
    let native_static_libs = if metadata_path.is_file() {
        let metadata = read_runtime_link_metadata(&metadata_path)?;
        if metadata.target != target {
            return Err(format!(
                "{NATIVE_RUNTIME_METADATA_MISMATCH}: runtime metadata `{}` targets `{}`, but the current target is `{target}`",
                metadata_path.display(),
                metadata.target
            ));
        }
        if metadata.runtime != artifact {
            return Err(format!(
                "{NATIVE_RUNTIME_METADATA_MISMATCH}: runtime metadata `{}` names runtime `{}`, but `{artifact}` was expected",
                metadata_path.display(),
                metadata.runtime
            ));
        }
        if metadata.ori_version != ORI_VERSION {
            return Err(format!(
                "{NATIVE_RUNTIME_METADATA_MISMATCH}: runtime metadata `{}` was staged for Ori {}, but the driver is Ori {}",
                metadata_path.display(),
                metadata.ori_version,
                ORI_VERSION
            ));
        }
        if metadata.abi_version != ORI_DRIVER_ABI_VERSION {
            return Err(format!(
                "{NATIVE_ABI_MISMATCH}: runtime metadata `{}` uses ABI {}, but the driver expects ABI {}",
                metadata_path.display(),
                metadata.abi_version,
                ORI_DRIVER_ABI_VERSION
            ));
        }
        metadata.native_static_libs
    } else {
        native_static_libs_for_target(target)
            .iter()
            .map(|lib| (*lib).to_string())
            .collect()
    };

    Ok(NativeRuntimeLink {
        runtime_lib,
        native_static_libs,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeLinkMetadata {
    target: String,
    runtime: String,
    runtime_cdylib: Option<String>,
    ori_version: String,
    abi_version: String,
    native_static_libs: Vec<String>,
}

fn read_runtime_link_metadata(path: &Path) -> Result<RuntimeLinkMetadata, String> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: cannot read runtime metadata `{}`: {e}",
            path.display()
        )
    })?;
    let target = json_string_field(&source, "target").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `target`",
            path.display()
        )
    })?;
    let runtime = json_string_field(&source, "runtime").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `runtime`",
            path.display()
        )
    })?;
    let runtime_cdylib =
        json_string_field(&source, "runtime_cdylib").filter(|value| !value.is_empty());
    let ori_version = json_string_field(&source, "ori_version").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `ori_version`",
            path.display()
        )
    })?;
    let abi_version = json_string_field(&source, "abi_version").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `abi_version`",
            path.display()
        )
    })?;
    let native_static_libs =
        json_string_array_field(&source, "native_static_libs").ok_or_else(|| {
            format!(
                "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string array field `native_static_libs`",
                path.display()
            )
        })?;
    Ok(RuntimeLinkMetadata {
        target,
        runtime,
        runtime_cdylib,
        ori_version,
        abi_version,
        native_static_libs,
    })
}

fn json_string_field(source: &str, field: &str) -> Option<String> {
    let rest = json_field_value(source, field)?;
    let (value, _) = parse_json_string(rest.trim_start())?;
    Some(value)
}

fn json_string_array_field(source: &str, field: &str) -> Option<Vec<String>> {
    let mut rest = json_field_value(source, field)?.trim_start();
    rest = rest.strip_prefix('[')?.trim_start();
    let mut values = Vec::new();
    loop {
        if rest.starts_with(']') {
            return Some(values);
        }
        let (value, consumed) = parse_json_string(rest)?;
        values.push(value);
        rest = rest[consumed..].trim_start();
        if let Some(next) = rest.strip_prefix(',') {
            rest = next.trim_start();
            continue;
        }
        rest.strip_prefix(']')?;
        return Some(values);
    }
}

fn json_field_value<'a>(source: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\"");
    let after_key = source.split_once(&key)?.1;
    let after_colon = after_key.split_once(':')?.1;
    Some(after_colon)
}

fn parse_json_string(source: &str) -> Option<(String, usize)> {
    let mut chars = source.char_indices();
    let (_, first) = chars.next()?;
    if first != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for (index, ch) in chars {
        if escaped {
            let value = match ch {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => ch,
            };
            out.push(value);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some((out, index + ch.len_utf8())),
            _ => out.push(ch),
        }
    }
    None
}

fn native_target_triple() -> String {
    std::env::var("ORI_TARGET_TRIPLE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(default_native_target_triple)
}

fn default_native_target_triple() -> String {
    if cfg!(all(windows, target_env = "msvc")) {
        "x86_64-pc-windows-msvc".to_string()
    } else if cfg!(all(windows, target_env = "gnu")) {
        "x86_64-pc-windows-gnu".to_string()
    } else if cfg!(target_os = "linux") {
        "x86_64-unknown-linux-gnu".to_string()
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin".to_string()
    } else {
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    }
}

fn native_runtime_artifact_name(target: &str) -> &'static str {
    if target.contains("windows-msvc") {
        "ori_runtime.lib"
    } else {
        "libori_runtime.a"
    }
}

fn native_runtime_cdylib_name(target: &str) -> &'static str {
    if target.contains("windows-msvc") {
        "ori_runtime.dll"
    } else if target.contains("apple-darwin") {
        "libori_runtime.dylib"
    } else {
        "libori_runtime.so"
    }
}

fn native_lib_cdylib_name(target: &str, name: &str) -> String {
    if target.contains("windows-msvc") {
        format!("{}.dll", name)
    } else if target.contains("apple-darwin") {
        format!("lib{}.dylib", name)
    } else {
        format!("lib{}.so", name)
    }
}

fn native_lib_static_name(target: &str, name: &str) -> String {
    if target.contains("windows-msvc") {
        format!("{}.lib", name)
    } else {
        format!("lib{}.a", name)
    }
}

fn packaged_runtime_candidates(target: &str, artifact: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("runtime").join(target).join(artifact));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("runtime").join(target).join(artifact));
            }
        }
    }
    candidates.push(repo_root().join("runtime").join(target).join(artifact));
    candidates
}

fn cargo_runtime_candidates(target: &str, artifact: &str) -> Vec<PathBuf> {
    let target_dir = cargo_target_dir();
    let preferred = if cfg!(debug_assertions) {
        ["debug", "release"]
    } else {
        ["release", "debug"]
    };
    let mut candidates = Vec::new();
    for profile in preferred {
        candidates.push(target_dir.join(profile).join(artifact));
        candidates.push(target_dir.join(target).join(profile).join(artifact));
    }
    candidates
}

fn cargo_target_dir() -> PathBuf {
    std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| cargo_workspace_root().join("target"))
}

/// The Cargo workspace holding this crate (`<repo>/compiler`): where the
/// fallback `cargo build -p ori-runtime` must run and where `target/` lives.
fn cargo_workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

/// The repository root (one level above the Cargo workspace): where
/// `tools/stage_native_runtime.sh` stages `runtime/<target>/` artifacts.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn build_native_runtime_with_cargo() -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = std::process::Command::new(&cargo);
    cmd.current_dir(cargo_workspace_root())
        .arg("build")
        .arg("-p")
        .arg("ori-runtime")
        .arg("--lib");
    if !cfg!(debug_assertions) {
        cmd.arg("--release");
    }

    let output = cmd
        .output()
        .map_err(|e| {
            format!(
                "{NATIVE_RUNTIME_MISSING}: failed to start Cargo while building the native Ori runtime: {e}"
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{NATIVE_RUNTIME_MISSING}: failed to build native Ori runtime with `{cargo} build -p ori-runtime --lib`\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
fn runtime_link_metadata_json(target: &str, artifact: &str) -> String {
    let native_static_libs = native_static_libs_for_target(target);
    format!(
        "{{\n  \"target\": \"{target}\",\n  \"runtime\": \"{artifact}\",\n  \"ori_version\": \"{ORI_VERSION}\",\n  \"abi_version\": \"{ORI_DRIVER_ABI_VERSION}\",\n  \"native_static_libs\": [{}]\n}}\n",
        native_static_libs
            .iter()
            .map(|lib| format!("\"{lib}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn native_static_libs_for_target(target: &str) -> &'static [&'static str] {
    if target.contains("windows-msvc") {
        &[
            "legacy_stdio_definitions.lib",
            "kernel32.lib",
            "ntdll.lib",
            "userenv.lib",
            "ws2_32.lib",
            "dbghelp.lib",
            "/defaultlib:msvcrt",
        ]
    } else if target.contains("linux") {
        &["-lpthread", "-ldl", "-lm", "-no-pie"]
    } else {
        &[]
    }
}

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

        let run_test = source_section(
            source,
            concat!("pub fn ", "run_test"),
            concat!("pub struct ", "BuildOutput"),
        );
        assert!(run_test.contains("run_native_tests"), "{run_test}");
        assert!(!run_test.contains("emit_c"), "{run_test}");

        let native_tests = source_section(
            source,
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
        for entry in super::COLLECTION_STDLIB_DOC_SIGNATURES {
            assert!(
                ori_types::stdlib::is_implemented_stdlib_module(entry.module),
                "COLLECTION_STDLIB_DOC_SIGNATURES references `{}` which is not an implemented stdlib module",
                entry.module
            );
        }
    }
}

pub fn run_check(path: &Path) -> Result<CheckOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved, _import_context) = load_and_resolve(path, &mut cache, &mut sink)?;

    // Type checking â€” only if no fatal parse errors so far
    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(CheckOutput {
        cache,
        resolved,
        diagnostics,
        has_errors,
    })
}

pub fn run_check_source(path: &Path, source: String) -> Result<CheckOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved) =
        load_and_resolve_with_entry_source(path, source, &mut cache, &mut sink)?;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(CheckOutput {
        cache,
        resolved,
        diagnostics,
        has_errors,
    })
}

pub fn run_new_project(
    root: &Path,
    options: NewProjectOptions,
) -> Result<NewProjectOutput, String> {
    if !options.is_init {
        if root.exists() {
            let mut entries = std::fs::read_dir(root)
                .map_err(|e| format!("cannot inspect `{}`: {e}", root.display()))?;
            if entries.next().is_some() {
                return Err(format!(
                    "project.new_exists: `{}` already exists and is not empty",
                    root.display()
                ));
            }
        }
        std::fs::create_dir_all(root)
            .map_err(|e| format!("cannot create project `{}`: {e}", root.display()))?;
    } else {
        if root.join("ori.proj").exists() || root.join("ori.pkg.toml").exists() {
            return Err(format!(
                "project.init_exists: `{}` already contains an ori.proj or ori.pkg.toml",
                root.display()
            ));
        }
        std::fs::create_dir_all(root)
            .map_err(|e| format!("cannot create project `{}`: {e}", root.display()))?;
    }

    // Optional docs tree for sidecars (`docs/<domain>/file.oridoc`). Domains
    // under the project root (`kanban-app/`, …) are user-created, not scaffolded.
    std::fs::create_dir_all(root.join("docs"))
        .map_err(|e| format!("cannot create `{}`: {e}", root.join("docs").display()))?;

    let name = options
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| default_project_name(root));
    let (kind_label, entry_rel, source) = match options.kind {
        // Layout (M2.layout): only `ori.proj` is required; `main.orl` at project
        // root is the recommended default entry. No forced `src/` / `app/`.
        NewProjectKind::App => (
            "app",
            "main.orl",
            "module app.main\n\nimport ori.io = io\n\nmain()\n    io.println(\"Hello, Ori!\")\nend\n",
        ),
        NewProjectKind::Lib => (
            "lib",
            "lib.orl",
            "module app.lib\n\npublic answer() -> int\n    return 42\nend\n",
        ),
    };

    let manifest = root.join("ori.proj");
    let entry = root.join(entry_rel);
    let manifest_source = format!(
        "manifest = 1\nname = \"{}\"\nversion = \"0.1.0\"\nkind = \"{}\"\nentry = \"{}\"\n\n[source]\nroot_namespace = \"app\"\n\n[docs]\npaths = [\"docs\"]\nmode = \"sidecar-first\"\nrequire_public = \"off\"\n",
        escape_manifest_string(&name),
        kind_label,
        escape_manifest_string(entry_rel),
    );

    std::fs::write(&manifest, manifest_source)
        .map_err(|e| format!("cannot write `{}`: {e}", manifest.display()))?;

    if !entry.exists() {
        std::fs::write(&entry, source)
            .map_err(|e| format!("cannot write `{}`: {e}", entry.display()))?;
    }

    Ok(NewProjectOutput {
        root: root.to_path_buf(),
        manifest,
        entry,
    })
}

pub fn run_doc(path: &Path) -> Result<DocOutput, String> {
    run_doc_with_options(path, DocOptions::default())
}

pub fn run_doc_with_options(path: &Path, options: DocOptions) -> Result<DocOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved, _import_context) = load_and_resolve(path, &mut cache, &mut sink)?;
    let mut external_docs = crate::oridoc::OridocIndex::default();
    let mut config = None;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    if !sink.has_errors() {
        config = project_config_for_docs(path)?;
        external_docs = load_oridoc_index(path, &loaded, config.as_ref(), &mut cache, &mut sink);
        if !sink.has_errors() {
            validate_oridoc_index(&loaded, &external_docs, config.as_ref(), &mut sink);
        }
    }

    let markdown = if !sink.has_errors() {
        let doc_mode = config
            .as_ref()
            .map(|config| config.doc_mode)
            .unwrap_or(ProjectDocMode::SidecarFirst);
        render_documentation_markdown(&loaded, &external_docs, doc_mode)
    } else {
        String::new()
    };
    let html = if !sink.has_errors() && options.format == DocFormat::Html {
        doc_html::render_static_html(&markdown)
    } else {
        String::new()
    };
    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(DocOutput {
        cache,
        markdown,
        html,
        diagnostics,
        has_errors,
    })
}

pub fn run_doc_check(path: &Path) -> Result<DocCheckOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved, _import_context) = load_and_resolve(path, &mut cache, &mut sink)?;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    if !sink.has_errors() {
        let config = project_config_for_docs(path)?;
        let external_docs =
            load_oridoc_index(path, &loaded, config.as_ref(), &mut cache, &mut sink);
        if !sink.has_errors() {
            validate_oridoc_index(&loaded, &external_docs, config.as_ref(), &mut sink);
        }
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(DocCheckOutput {
        cache,
        diagnostics,
        has_errors,
    })
}

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
    let (loaded, resolved, _import_context) = load_and_resolve(path, &mut cache, &mut sink)?;

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

pub struct BuildOutput {
    pub cache: SourceCache,
    pub c_source: String,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

/// Full pipeline + HIR lowering + C code generation.
///
/// Kept as a compatibility helper for existing C debug backend tests. Public
/// CLI access should use `ori emit c`, not `ori build`.
pub fn run_build(path: &Path) -> Result<BuildOutput, String> {
    run_emit_c(path)
}

/// Full pipeline + HIR lowering + C code generation for `ori emit c`.
pub fn run_emit_c(path: &Path) -> Result<BuildOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let (loaded, resolved, _import_context) = load_and_resolve(path, &mut cache, &mut sink)?;

    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }

    let c_source = if !sink.has_errors() {
        let mut hir = lower_loaded_sources(&loaded, &resolved, &mut sink);
        ori_hir::optimize_module(&mut hir, ori_hir::OptLevel::from_env());
        match ori_codegen::emit_c(&hir) {
            Ok(source) => source,
            Err(error) => {
                sink.emit(
                    Diagnostic::error(
                        "backend.c_unsupported",
                        "the C debug backend cannot generate this program correctly",
                    )
                    .with_why(
                        "the C backend is a secondary debug/transpile route with partial feature parity",
                    )
                    .with_action(
                        "use `ori compile` for the native backend, or remove the unsupported feature before generating C",
                    )
                    .with_note(error),
                );
                String::new()
            }
        }
    } else {
        String::new()
    };

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(BuildOutput {
        cache,
        c_source,
        diagnostics,
        has_errors,
    })
}

// â”€â”€ Utilities â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn read_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read `{}`: {}", path.display(), e))
}

fn resolve_entry_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_dir() {
        let manifest = path.join("ori.proj");
        if !manifest.is_file() {
            return Err(format!(
                "project manifest `{}` not found",
                manifest.display()
            ));
        }
        return read_project_config(&manifest).map(|config| config.entry);
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("ori.proj") {
        return read_project_config(path).map(|config| config.entry);
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("ori.pkg.toml") {
        return crate::package::load_package_manifest(path).map(|manifest| manifest.entry);
    }

    Ok(path.to_owned())
}

fn read_project_config(manifest: &Path) -> Result<ProjectConfig, String> {
    let source = read_file(manifest)?;
    let root = manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut entry = None;
    let mut name = None;
    let mut version = None;
    let mut kind = ProjectKind::App;
    let mut source_root = None;
    let mut root_namespace = None;
    let mut dependencies = Vec::new();
    let mut doc_paths = Vec::new();
    let mut doc_mode = ProjectDocMode::SidecarFirst;
    let mut require_public_docs = DocRequirement::Off;
    let mut section = ManifestSection::Root;

    for line in source.lines() {
        let line = strip_manifest_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = match &line[1..line.len() - 1] {
                "source" => ManifestSection::Source,
                "dependencies" => ManifestSection::Dependencies,
                "docs" => ManifestSection::Docs,
                _ => ManifestSection::Other,
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match (&section, key) {
            (ManifestSection::Root, "name") => {
                name = Some(parse_manifest_string(value, "name", manifest)?);
            }
            (ManifestSection::Root, "version") => {
                version = Some(parse_manifest_string(value, "version", manifest)?);
            }
            (ManifestSection::Root, "kind") => {
                kind =
                    parse_project_kind(&parse_manifest_string(value, "kind", manifest)?, manifest)?;
            }
            (ManifestSection::Root, "entry") => {
                entry = Some(root.join(parse_manifest_string(value, "entry", manifest)?));
            }
            (ManifestSection::Root, "manifest") => {
                let _ = parse_manifest_number(value, "manifest", manifest)?;
            }
            (ManifestSection::Source, "root") => {
                source_root =
                    Some(root.join(parse_manifest_string(value, "source.root", manifest)?));
            }
            (ManifestSection::Source, "root_namespace" | "namespace") => {
                root_namespace = Some(parse_manifest_string(
                    value,
                    "source.root_namespace",
                    manifest,
                )?);
            }
            (ManifestSection::Dependencies, name) => {
                dependencies.push(parse_project_dependency(name, value, manifest, root)?);
            }
            (ManifestSection::Docs, "paths") => {
                doc_paths = parse_manifest_string_array(value, "docs.paths", manifest)?
                    .into_iter()
                    .map(|path| root.join(path))
                    .collect();
            }
            (ManifestSection::Docs, "mode") => {
                doc_mode = parse_project_doc_mode(
                    &parse_manifest_string(value, "docs.mode", manifest)?,
                    manifest,
                )?;
            }
            (ManifestSection::Docs, "require_public") => {
                require_public_docs = parse_doc_requirement(
                    &parse_manifest_string(value, "docs.require_public", manifest)?,
                    manifest,
                )?;
            }
            _ => {}
        }
    }

    let Some(entry) = entry else {
        return Err(format!(
            "project manifest `{}` is missing `entry`",
            manifest.display()
        ));
    };
    if !entry.is_file() {
        return Err(format!(
            "project entry `{}` does not exist",
            entry.display()
        ));
    }
    Ok(ProjectConfig {
        manifest_path: manifest.to_path_buf(),
        root: root.to_path_buf(),
        name,
        version,
        kind,
        entry,
        source_root,
        root_namespace,
        dependencies,
        doc_paths,
        doc_mode,
        require_public_docs,
    })
}

#[derive(Debug)]
enum ManifestSection {
    Root,
    Source,
    Dependencies,
    Docs,
    Other,
}

fn strip_manifest_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut previous = '\0';
    for (index, ch) in line.char_indices() {
        if ch == '"' && previous != '\\' {
            in_string = !in_string;
        }
        if !in_string && ch == '-' && line[index..].starts_with("--") {
            return &line[..index];
        }
        previous = ch;
    }
    line
}

fn default_project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("app")
        .to_string()
}

fn escape_manifest_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_manifest_string(value: &str, key: &str, manifest: &Path) -> Result<String, String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return Ok(value[1..value.len() - 1].replace("\\\"", "\""));
    }
    Err(format!(
        "project manifest `{}` field `{key}` must be a quoted string",
        manifest.display()
    ))
}

fn parse_manifest_number(value: &str, key: &str, manifest: &Path) -> Result<u32, String> {
    value.trim().parse::<u32>().map_err(|_| {
        format!(
            "project manifest `{}` field `{key}` must be a number",
            manifest.display()
        )
    })
}

fn parse_manifest_string_array(
    value: &str,
    key: &str,
    manifest: &Path,
) -> Result<Vec<String>, String> {
    let value = value.trim();
    if !(value.starts_with('[') && value.ends_with(']')) {
        return Err(format!(
            "project manifest `{}` field `{key}` must be an array of quoted strings",
            manifest.display()
        ));
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|part| parse_manifest_string(part.trim(), key, manifest))
        .collect()
}

fn parse_project_dependency(
    name: &str,
    value: &str,
    manifest: &Path,
    root: &Path,
) -> Result<ProjectDependency, String> {
    let name = name.trim().to_string();
    validate_project_dependency_name(&name, manifest)?;
    let value = value.trim();
    if value.starts_with('"') {
        return Ok(ProjectDependency {
            name,
            path: None,
            version: Some(parse_manifest_string(
                value,
                "dependencies.version",
                manifest,
            )?),
            git: None,
        });
    }
    if value.starts_with('{') {
        let table = parse_manifest_inline_table(value, manifest)?;
        let version = table.get("version").cloned();
        if let Some(git_url) = table.get("git").cloned() {
            if table.contains_key("path") {
                return Err(format!(
                    "project manifest `{}` dependency `{name}` cannot combine `git` and `path`",
                    manifest.display()
                ));
            }
            let rev = table.get("rev").cloned();
            let tag = table.get("tag").cloned();
            let branch = table.get("branch").cloned();
            let pin_count = [rev.is_some(), tag.is_some(), branch.is_some()]
                .into_iter()
                .filter(|v| *v)
                .count();
            if pin_count > 1 {
                return Err(format!(
                    "project manifest `{}` dependency `{name}` may set only one of `rev`, `tag`, or `branch`",
                    manifest.display()
                ));
            }
            let git_version = version.clone();
            return Ok(ProjectDependency {
                name,
                path: None,
                version,
                git: Some(crate::package::GitDependencySpec {
                    url: git_url,
                    rev,
                    tag,
                    branch,
                    version: git_version,
                }),
            });
        }
        let path = table.get("path").map(|path| root.join(path));
        if path.is_none() {
            return Err(format!(
                "project manifest `{}` dependency `{name}` needs `path` or `git`",
                manifest.display()
            ));
        }
        return Ok(ProjectDependency {
            name,
            path,
            version,
            git: None,
        });
    }
    Err(format!(
        "project manifest `{}` dependency `{name}` must be a quoted version, `{{ path = \"...\" }}`, or `{{ git = \"...\" }}`",
        manifest.display()
    ))
}

fn validate_project_dependency_name(name: &str, manifest: &Path) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!(
            "project manifest `{}` has an empty dependency name",
            manifest.display()
        ));
    }
    for segment in name.split('.') {
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return Err(format!(
                "project manifest `{}` dependency `{name}` has an empty namespace segment",
                manifest.display()
            ));
        };
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return Err(format!(
                "project manifest `{}` dependency `{name}` must start each segment with a letter or `_`",
                manifest.display()
            ));
        }
        if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
            return Err(format!(
                "project manifest `{}` dependency `{name}` may contain only letters, digits, and `_`",
                manifest.display()
            ));
        }
    }
    Ok(())
}

fn parse_manifest_inline_table(
    value: &str,
    manifest: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(format!(
            "project manifest `{}` inline table must start with `{{` and end with `}}`",
            manifest.display()
        ));
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    let mut table = BTreeMap::new();
    if inner.is_empty() {
        return Ok(table);
    }
    for part in split_manifest_inline_items(inner) {
        let Some((key, raw_value)) = part.split_once('=') else {
            return Err(format!(
                "project manifest `{}` inline table item must use `key = value`",
                manifest.display()
            ));
        };
        let key = key.trim().to_string();
        let parsed_value = parse_manifest_string(raw_value.trim(), &key, manifest)?;
        table.insert(key, parsed_value);
    }
    Ok(table)
}

fn split_manifest_inline_items(value: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == ',' && !in_string {
            items.push(value[start..index].trim());
            start = index + 1;
        }
    }
    items.push(value[start..].trim());
    items
}

fn parse_project_kind(value: &str, manifest: &Path) -> Result<ProjectKind, String> {
    match value {
        "app" => Ok(ProjectKind::App),
        "lib" => Ok(ProjectKind::Lib),
        other => Err(format!(
            "project manifest `{}` field `kind` must be `app` or `lib`, got `{other}`",
            manifest.display()
        )),
    }
}

fn parse_project_doc_mode(value: &str, manifest: &Path) -> Result<ProjectDocMode, String> {
    match value {
        "sidecar-first" => Ok(ProjectDocMode::SidecarFirst),
        "inline-first" => Ok(ProjectDocMode::InlineFirst),
        other => Err(format!(
            "project manifest `{}` field `docs.mode` must be `sidecar-first` or `inline-first`, got `{other}`",
            manifest.display()
        )),
    }
}

fn parse_doc_requirement(value: &str, manifest: &Path) -> Result<DocRequirement, String> {
    match value {
        "off" => Ok(DocRequirement::Off),
        "warn" => Ok(DocRequirement::Warn),
        "error" => Ok(DocRequirement::Error),
        other => Err(format!(
            "project manifest `{}` field `docs.require_public` must be `off`, `warn`, or `error`, got `{other}`",
            manifest.display()
        )),
    }
}

fn project_config_for_docs(path: &Path) -> Result<Option<ProjectConfig>, String> {
    if path.is_dir() {
        let manifest = path.join("ori.proj");
        return manifest
            .is_file()
            .then(|| read_project_config(&manifest))
            .transpose();
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("ori.proj") {
        return read_project_config(path).map(Some);
    }

    let start = path.parent().unwrap_or_else(|| Path::new("."));
    find_project_root(start)
        .map(|root| read_project_config(&root.join("ori.proj")))
        .transpose()
}

fn import_context_for_entry(entry: &Path) -> Result<ImportContext, String> {
    let mut context = ImportContext::default();
    let start = entry.parent().unwrap_or_else(|| Path::new("."));

    if let Some(root) = find_project_root(start) {
        let config = read_project_config(&root.join("ori.proj"))?;
        add_project_dependencies(&config, &mut context)?;
        let package_manifest = root.join("ori.pkg.toml");
        if package_manifest.is_file() {
            add_package_manifest_dependencies(&package_manifest, &mut context)?;
        }
    } else if let Some(package_manifest) = find_package_manifest(start) {
        add_package_manifest_dependencies(&package_manifest, &mut context)?;
    }

    Ok(context)
}

fn add_project_dependencies(
    config: &ProjectConfig,
    context: &mut ImportContext,
) -> Result<(), String> {
    let cache_root = crate::package::default_package_cache_root().ok();
    for dependency in &config.dependencies {
        if let Some(git) = &dependency.git {
            let cache = cache_root.clone().ok_or_else(|| {
                "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve git dependencies"
                    .to_string()
            })?;
            let root =
                crate::package::ensure_git_dependency_cached(&dependency.name, git, &cache)?;
            let import_dependency =
                import_dependency_from_root(&dependency.name, &root, dependency.version.as_deref())?;
            add_import_dependency(context, import_dependency);
            // Transitive native libs (e.g. sqlite from path/git package).
            push_package_native_libs(context, &root)?;
            continue;
        }
        if let Some(path) = &dependency.path {
            let import_dependency =
                import_dependency_from_root(&dependency.name, path, dependency.version.as_deref())?;
            add_import_dependency(context, import_dependency);
            push_package_native_libs(context, path)?;
            continue;
        }
        if let Some(version) = &dependency.version {
            let cache = cache_root.clone().ok_or_else(|| {
                "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve version dependencies"
                    .to_string()
            })?;
            let root =
                crate::package::resolve_cached_version_package(&dependency.name, version, &cache)?;
            let import_dependency =
                import_dependency_from_root(&dependency.name, &root, Some(version))?;
            add_import_dependency(context, import_dependency);
            push_package_native_libs(context, &root)?;
        }
    }
    Ok(())
}

fn add_package_manifest_dependencies(
    manifest_path: &Path,
    context: &mut ImportContext,
) -> Result<(), String> {
    let manifest = crate::package::load_package_manifest(manifest_path)?;
    let cache_root = crate::package::default_package_cache_root().ok();
    for dependency in &manifest.dependencies {
        match &dependency.requirement {
            crate::package::DependencyRequirement::Path { path, version } => {
                let root = manifest.root.join(path);
                let import_dependency =
                    import_dependency_from_root(&dependency.name, &root, version.as_deref())?;
                add_import_dependency(context, import_dependency);
                push_package_native_libs(context, &root)?;
            }
            crate::package::DependencyRequirement::Git {
                url,
                rev,
                tag,
                branch,
                version,
            } => {
                let cache = cache_root.clone().ok_or_else(|| {
                    "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve git dependencies"
                        .to_string()
                })?;
                let spec = crate::package::GitDependencySpec {
                    url: url.clone(),
                    rev: rev.clone(),
                    tag: tag.clone(),
                    branch: branch.clone(),
                    version: version.clone(),
                };
                let root =
                    crate::package::ensure_git_dependency_cached(&dependency.name, &spec, &cache)?;
                let import_dependency =
                    import_dependency_from_root(&dependency.name, &root, version.as_deref())?;
                add_import_dependency(context, import_dependency);
                push_package_native_libs(context, &root)?;
            }
            crate::package::DependencyRequirement::Version(version) => {
                let cache = cache_root.clone().ok_or_else(|| {
                    "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve version dependencies"
                        .to_string()
                })?;
                let root = crate::package::resolve_cached_version_package(
                    &dependency.name,
                    version,
                    &cache,
                )?;
                let import_dependency =
                    import_dependency_from_root(&dependency.name, &root, Some(version))?;
                add_import_dependency(context, import_dependency);
                push_package_native_libs(context, &root)?;
            }
        }
    }
    for lib in manifest.native_libs {
        push_native_lib(context, lib, manifest.root.clone());
    }
    Ok(())
}

/// Register `native_libs` from a dependency package (and its nested package deps once).
fn push_package_native_libs(context: &mut ImportContext, package_root: &Path) -> Result<(), String> {
    let package_manifest = package_root.join("ori.pkg.toml");
    if !package_manifest.is_file() {
        return Ok(());
    }
    let manifest = crate::package::load_package_manifest(&package_manifest)?;
    for lib in &manifest.native_libs {
        push_native_lib(context, lib.clone(), manifest.root.clone());
    }
    // One level of nested package deps (adapter → sqlite).
    for dependency in &manifest.dependencies {
        if let crate::package::DependencyRequirement::Path { path, .. } = &dependency.requirement {
            let nested = manifest.root.join(path);
            let nested_pkg = nested.join("ori.pkg.toml");
            if nested_pkg.is_file() {
                let nested_manifest = crate::package::load_package_manifest(&nested_pkg)?;
                for lib in nested_manifest.native_libs {
                    push_native_lib(context, lib, nested_manifest.root.clone());
                }
            }
        }
    }
    Ok(())
}

fn push_native_lib(context: &mut ImportContext, name: String, package_root: PathBuf) {
    if context
        .native_libs
        .iter()
        .any(|existing| existing.name == name && existing.package_root == package_root)
    {
        return;
    }
    context.native_libs.push(NativeLibContext {
        name,
        package_root,
    });
}

fn import_dependency_from_root(
    expected_name: &str,
    root: &Path,
    expected_version: Option<&str>,
) -> Result<ImportDependency, String> {
    let package_manifest = root.join("ori.pkg.toml");
    if package_manifest.is_file() {
        let manifest = crate::package::load_package_manifest(&package_manifest)?;
        if manifest.name != expected_name {
            return Err(format!(
                "package.dependency_name_mismatch: dependency `{expected_name}` points to package `{}`",
                manifest.name
            ));
        }
        if let Some(version) = expected_version {
            if manifest.version != version {
                return Err(format!(
                    "package.dependency_version_mismatch: dependency `{expected_name}` expected `{version}`, found `{}`",
                    manifest.version
                ));
            }
        }
        return Ok(ImportDependency {
            name: manifest.name,
            root: manifest.root,
            source_root: manifest.entry.parent().map(Path::to_path_buf),
            entry: manifest.entry,
        });
    }

    let project_manifest = root.join("ori.proj");
    if project_manifest.is_file() {
        let config = read_project_config(&project_manifest)?;
        if let Some(version) = expected_version {
            if config.version.as_deref() != Some(version) {
                return Err(format!(
                    "package.dependency_version_mismatch: dependency `{expected_name}` expected `{version}`, found `{}`",
                    config.version.as_deref().unwrap_or("<missing>")
                ));
            }
        }
        return Ok(ImportDependency {
            name: expected_name.to_string(),
            root: config.root,
            entry: config.entry,
            source_root: config.source_root,
        });
    }

    Err(format!(
        "package.dependency_manifest_missing: dependency `{expected_name}` needs `ori.pkg.toml` or `ori.proj` under `{}`",
        root.display()
    ))
}

fn add_import_dependency(context: &mut ImportContext, dependency: ImportDependency) {
    if !context
        .dependencies
        .iter()
        .any(|existing| existing.name == dependency.name && existing.entry == dependency.entry)
    {
        context.dependencies.push(dependency);
    }
}

fn find_package_manifest(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let manifest = ancestor.join("ori.pkg.toml");
        if manifest.is_file() {
            return Some(manifest);
        }
    }
    None
}

fn load_and_resolve(
    path: &Path,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> Result<(Vec<LoadedSource>, ResolvedModule, ImportContext), String> {
    let entry = resolve_entry_path(path)?;
    let context = import_context_for_entry(&entry)?;
    let (loaded, resolved) = load_and_resolve_entry(&entry, None, &context, cache, sink)?;
    Ok((loaded, resolved, context))
}

fn load_and_resolve_with_entry_source(
    path: &Path,
    source: String,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> Result<(Vec<LoadedSource>, ResolvedModule), String> {
    let entry = resolve_entry_path(path)?;
    let entry = std::fs::canonicalize(entry).unwrap_or_else(|_| path.to_owned());
    let context = import_context_for_entry(&entry)?;
    load_and_resolve_entry(&entry, Some((&entry, &source)), &context, cache, sink)
}

fn load_and_resolve_entry(
    entry: &Path,
    entry_source: Option<(&Path, &str)>,
    context: &ImportContext,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> Result<(Vec<LoadedSource>, ResolvedModule), String> {
    let mut loaded = Vec::new();
    let mut seen = HashSet::new();
    let mut active = Vec::new();
    load_source_recursive(
        entry,
        cache,
        sink,
        &mut seen,
        &mut active,
        &mut loaded,
        entry_source,
        context,
    )?;
    let entry_namespace = loaded
        .first()
        .map(|s| namespace_of(&s.ast))
        .unwrap_or_default();
    let files: Vec<_> = loaded.iter().map(|s| (&s.ast, s.file_id)).collect();
    let resolved = ori_types::resolve::resolve_many(&files, entry_namespace, sink);
    Ok((loaded, resolved))
}

fn load_source_recursive(
    path: &Path,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
    seen: &mut HashSet<PathBuf>,
    active: &mut Vec<PathBuf>,
    loaded: &mut Vec<LoadedSource>,
    entry_source: Option<(&Path, &str)>,
    context: &ImportContext,
) -> Result<(), String> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    if !seen.insert(path.clone()) {
        return Ok(());
    }
    let source = match entry_source {
        Some((entry_path, source)) if entry_path == path.as_path() => source.to_string(),
        _ => read_file(&path)?,
    };
    let file_id = cache.add(&path, source.clone());
    let tokens = ori_lexer::lex(&source, file_id, sink);
    let ast = ori_parser::parse(&tokens, &source, file_id, sink);
    let imports: Vec<_> = ast
        .imports
        .iter()
        .map(|i| (i.path.to_string(), i.span, !i.selected.is_empty()))
        .collect();
    loaded.push(LoadedSource {
        path: path.clone(),
        file_id,
        source,
        tokens,
        ast,
    });
    active.push(path.clone());
    for (import, span, has_selected_items) in imports {
        match classify_stdlib_import(&import, has_selected_items) {
            StdlibImportStatus::Implemented => continue,
            StdlibImportStatus::StdlibSources(sources) => {
                for (source_path, expected_namespace) in sources {
                    // Hybrid stdlib pattern: `module ori.net` + `import ori.net = net`
                    // re-exports Layer 1 symbols. The import resolves to the same
                    // `.orl` file already being loaded — not a real cycle.
                    if source_path == *path {
                        continue;
                    }
                    if active.contains(&source_path) {
                        let cycle = import_cycle_description(active, loaded, &source_path, &import);
                        sink.emit(
                            Diagnostic::error(
                                "project.circular_import",
                                format!("import cycle detected: {}", cycle),
                            )
                            .with_label(Label::primary(file_id, span, "cyclic import here"))
                            .with_action(
                                "remove one import or move shared definitions into an acyclic module",
                            ),
                        );
                        validate_import_namespace(
                            loaded,
                            &source_path,
                            &expected_namespace,
                            file_id,
                            span,
                            sink,
                        );
                        continue;
                    }
                    load_source_recursive(
                        &source_path,
                        cache,
                        sink,
                        seen,
                        active,
                        loaded,
                        entry_source,
                        context,
                    )?;
                    validate_import_namespace(
                        loaded,
                        &source_path,
                        &expected_namespace,
                        file_id,
                        span,
                        sink,
                    );
                }
                continue;
            }
            StdlibImportStatus::Unknown => {
                sink.emit(
                    Diagnostic::error(
                        "bind.stdlib_module_unknown",
                        format!("standard library module `{}` is not known", import),
                    )
                    .with_label(Label::primary(file_id, span, "stdlib import here"))
                    .with_action("check the module name or use an implemented `ori.*` module"),
                );
                continue;
            }
            StdlibImportStatus::NotStdlib => {}
        }
        match resolve_import_path(&path, &import, context) {
            ImportResolution::Found(import_path) => {
                if active.contains(&import_path) {
                    let cycle = import_cycle_description(active, loaded, &import_path, &import);
                    sink.emit(
                        Diagnostic::error(
                            "project.circular_import",
                            format!("import cycle detected: {}", cycle),
                        )
                        .with_label(Label::primary(file_id, span, "cyclic import here"))
                        .with_action(
                            "remove one import or move shared definitions into an acyclic module",
                        ),
                    );
                    validate_import_namespace(loaded, &import_path, &import, file_id, span, sink);
                    continue;
                }
                load_source_recursive(
                    &import_path,
                    cache,
                    sink,
                    seen,
                    active,
                    loaded,
                    entry_source,
                    context,
                )?;
                validate_import_namespace(loaded, &import_path, &import, file_id, span, sink);
            }
            ImportResolution::Ambiguous(paths) => {
                let mut diagnostic = Diagnostic::error(
                    "bind.import_ambiguous",
                    format!("import `{}` matches more than one file", import),
                )
                    .with_label(Label::primary(file_id, span, "ambiguous import here"))
                    .with_why("the current import search policy found multiple matching `.orl` files")
                    .with_action("keep only one matching file or import through a path that resolves to a single file");
                for path in paths {
                    diagnostic = diagnostic.with_note(format!("candidate: {}", path.display()));
                }
                sink.emit(diagnostic);
            }
            ImportResolution::Missing => {
                sink.emit(
                    Diagnostic::error(
                        "bind.import_not_found",
                        format!("import `{}` not found", import),
                    )
                    .with_label(Label::primary(file_id, span, "imported here"))
                    .with_action("place the imported namespace in a matching `.orl` file"),
                );
            }
        }
    }
    active.pop();
    Ok(())
}

fn validate_import_namespace(
    loaded: &[LoadedSource],
    import_path: &Path,
    import: &str,
    file_id: FileId,
    span: ori_diagnostics::Span,
    sink: &mut DiagnosticSink,
) {
    if let Some(imported) = loaded.iter().find(|s| s.path == import_path) {
        let declared = namespace_of(&imported.ast);
        if declared != import {
            sink.emit(
                Diagnostic::error(
                    "project.namespace_file_mismatch",
                    format!(
                        "import `{}` resolved to file declaring `{}`",
                        import, declared
                    ),
                )
                .with_label(Label::primary(file_id, span, "imported here"))
                .with_action("make the imported file namespace match the import path"),
            );
        }
    }
}

fn import_cycle_description(
    active: &[PathBuf],
    loaded: &[LoadedSource],
    import_path: &Path,
    import: &str,
) -> String {
    let start = active.iter().position(|p| p == import_path).unwrap_or(0);
    let mut parts: Vec<String> = active[start..]
        .iter()
        .map(|path| {
            loaded
                .iter()
                .find(|s| s.path == *path)
                .map(|s| namespace_of(&s.ast))
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect();
    parts.push(import.to_string());
    parts.join(" -> ")
}

fn namespace_of(file: &SourceFile) -> String {
    file.namespace.name.to_string()
}

enum StdlibImportStatus {
    Implemented,
    StdlibSources(Vec<(PathBuf, String)>),
    Unknown,
    NotStdlib,
}

fn classify_stdlib_import(import: &str, _has_selected_items: bool) -> StdlibImportStatus {
    if import != "ori" && !import.starts_with("ori.") {
        return StdlibImportStatus::NotStdlib;
    }
    if ori_types::stdlib::is_implemented_stdlib_module(import) {
        if let Some(sources) = find_stdlib_selective_sources(import) {
            return StdlibImportStatus::StdlibSources(sources);
        }
        return StdlibImportStatus::Implemented;
    }
    if let Some(sources) = find_stdlib_selective_sources(import) {
        return StdlibImportStatus::StdlibSources(sources);
    }
    StdlibImportStatus::Unknown
}

/// Resolve a stdlib module import (`ori.string.utils`) to its `.orl` source path.
pub fn stdlib_source_path(import: &str) -> Option<PathBuf> {
    find_stdlib_selective_sources(import)
        .and_then(|sources| sources.into_iter().map(|(path, _)| path).next())
}

fn find_stdlib_selective_sources(import: &str) -> Option<Vec<(PathBuf, String)>> {
    if let Some(path) = find_stdlib_source_module(import) {
        return Some(vec![(path, import.to_string())]);
    }
    find_stdlib_flatten_submodules(import)
}

fn find_stdlib_flatten_submodules(import: &str) -> Option<Vec<(PathBuf, String)>> {
    let relative = import.strip_prefix("ori.")?;
    let stdlib_root = find_stdlib_root()?;
    let mut dir = stdlib_root.clone();
    for segment in relative.split('.') {
        dir.push(segment);
    }
    let mut sources = Vec::new();
    for sub in ["utils", "algorithms"] {
        let candidate = dir.join(format!("{sub}.orl"));
        if candidate.is_file() {
            sources.push((candidate, format!("{import}.{sub}")));
        }
    }
    if sources.is_empty() {
        None
    } else {
        Some(sources)
    }
}

fn find_stdlib_source_module(import: &str) -> Option<PathBuf> {
    let relative = import.strip_prefix("ori.")?;
    let stdlib_root = find_stdlib_root()?;
    let mut relative_path = PathBuf::new();
    for segment in relative.split('.') {
        relative_path.push(segment);
    }
    let candidate = stdlib_root.join(&relative_path).with_extension("orl");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

/// Resolve the stdlib root directory.
///
/// Order:
/// 1. `ORI_STDLIB_ROOT`
/// 2. End-user / local package next to the binary:
///    - `<exe_dir>/stdlib` (flat package)
///    - `<exe_dir>/../stdlib` (`…/bin/ori` + `…/stdlib`, e.g. `~/.local/share/ori`)
/// 3. Dev layout from `CARGO_MANIFEST_DIR` (only when still present; never preferred
///    over a package layout next to the running binary)
/// 4. Walk cwd parents for a `stdlib/` directory
pub fn find_stdlib_root() -> Option<PathBuf> {
    let dir_if_stdlib = |path: PathBuf| -> Option<PathBuf> {
        if path.is_dir() {
            Some(std::fs::canonicalize(&path).unwrap_or(path))
        } else {
            None
        }
    };

    if let Ok(path) = std::env::var("ORI_STDLIB_ROOT") {
        if let Some(root) = dir_if_stdlib(PathBuf::from(path)) {
            return Some(root);
        }
    }

    let package_near_exe = || -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;
        // Layout A: stdlib beside the binary
        if let Some(root) = dir_if_stdlib(exe_dir.join("stdlib")) {
            return Some(root);
        }
        // Layout B: `…/bin/ori` + `…/stdlib` (user install under ~/.local/share/ori)
        if let Some(prefix) = exe_dir.parent() {
            if let Some(root) = dir_if_stdlib(prefix.join("stdlib")) {
                return Some(root);
            }
        }
        None
    };

    // Always prefer a real install next to the running binary over the
    // compile-time worktree path baked into `CARGO_MANIFEST_DIR`.
    if let Some(path) = package_near_exe() {
        return Some(path);
    }

    let packaged_only = std::env::var_os("ORI_REQUIRE_PACKAGED_RUNTIME").is_some_and(|v| {
        let s = v.to_string_lossy();
        s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
    });
    if packaged_only {
        return None;
    }

    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_candidate = manifest_root.join("../../../stdlib");
    if let Some(root) = dir_if_stdlib(dev_candidate) {
        return Some(root);
    }

    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd;
        for _ in 0..8 {
            let candidate = dir.join("stdlib");
            // Accept either nested helpers (string/) or parent modules (string.orl).
            if candidate.is_dir()
                && (candidate.join("string.orl").is_file()
                    || candidate.join("string").is_dir()
                    || candidate.join("list.orl").is_file()
                    || candidate.join("list").is_dir())
            {
                return dir_if_stdlib(candidate);
            }
            if let Some(parent) = dir.parent() {
                dir = parent.to_owned();
            } else {
                break;
            }
        }
    }
    None
}

enum ImportResolution {
    Found(PathBuf),
    Ambiguous(Vec<PathBuf>),
    Missing,
}

fn resolve_import_path(importer: &Path, import: &str, context: &ImportContext) -> ImportResolution {
    // Directory of the file performing the import
    let Some(dir) = importer.parent() else {
        return ImportResolution::Missing;
    };

    // Determine the project root (if any). All candidate paths must reside in or below it.
    let project_root = find_project_root(dir).and_then(|p| std::fs::canonicalize(p).ok());

    let mut matches = Vec::new();

    // Walk ancestors from the importer's directory upwards.
    for base in dir.ancestors() {
        // Stop once we've climbed above the project root (if found)
        if let Some(ref root) = project_root {
            if let Ok(base_real) = std::fs::canonicalize(base) {
                // Always include the root itself in the search, then break.
                let reached_root = base_real == *root;
                for candidate in import_candidates(base, import) {
                    if candidate.is_file() {
                        let path = std::fs::canonicalize(&candidate).unwrap_or(candidate);
                        if !matches.contains(&path) {
                            matches.push(path);
                        }
                    }
                }
                if reached_root {
                    break;
                } else {
                    continue;
                }
            }
        }
        // If no project root was found, or we haven't reached it yet, continue searching.
        for candidate in import_candidates(base, import) {
            if candidate.is_file() {
                let path = std::fs::canonicalize(&candidate).unwrap_or(candidate);
                if !matches.contains(&path) {
                    matches.push(path);
                }
            }
        }
    }

    for dependency in &context.dependencies {
        for candidate in dependency_import_candidates(dependency, import) {
            if candidate.is_file() {
                let path = std::fs::canonicalize(&candidate).unwrap_or(candidate);
                if !matches.contains(&path) {
                    matches.push(path);
                }
            }
        }
    }

    match matches.len() {
        0 => ImportResolution::Missing,
        1 => ImportResolution::Found(matches.remove(0)),
        _ => ImportResolution::Ambiguous(matches),
    }
}

fn dependency_import_candidates(dependency: &ImportDependency, import: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if import == dependency.name {
        candidates.push(dependency.entry.clone());
    }

    let prefix = format!("{}.", dependency.name);
    let suffix = import.strip_prefix(&prefix);
    for base in dependency_search_bases(dependency) {
        candidates.extend(import_candidates(&base, import));
        if let Some(suffix) = suffix {
            candidates.extend(import_candidates(&base, suffix));
        }
    }
    candidates
}

fn dependency_search_bases(dependency: &ImportDependency) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Some(source_root) = &dependency.source_root {
        bases.push(source_root.clone());
    }
    if let Some(parent) = dependency.entry.parent() {
        bases.push(parent.to_path_buf());
    }
    bases.push(dependency.root.clone());
    dedup_paths(bases)
}

fn import_candidates(base: &Path, import: &str) -> Vec<PathBuf> {
    let parts: Vec<_> = import.split('.').filter(|p| !p.is_empty()).collect();
    let mut candidates = Vec::new();
    if !parts.is_empty() {
        let mut nested_dir = base.to_path_buf();
        for part in &parts {
            nested_dir.push(part);
        }
        let mut nested = nested_dir.clone();
        nested.set_extension("orl");
        candidates.push(nested.clone());
        candidates.push(nested_dir.join("mod.orl"));
        candidates.push(nested_dir.join("index.orl"));

        if let Some(last) = parts.last() {
            candidates.push(base.join(format!("{last}.orl")));
            candidates.push(base.join(last).join("mod.orl"));
            candidates.push(base.join(last).join("index.orl"));
        }
    }
    candidates
}

/// Walk ancestors upwards from `start` until an `ori.proj` file is found. The directory
/// that contains the manifest is considered the project root.
fn find_project_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let proj = ancestor.join("ori.proj");
        if proj.is_file() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn check_loaded_sources(
    loaded: &[LoadedSource],
    resolved: &ResolvedModule,
    sink: &mut DiagnosticSink,
) {
    for source in loaded {
        validate_doc_tags(source, sink);
        let namespace = namespace_of(&source.ast);
        let mut checker = ori_types::check::Checker::new(
            &resolved.def_map,
            &resolved.func_sigs,
            &resolved.value_sigs,
            &resolved.struct_sigs,
            &resolved.enum_sigs,
            &resolved.trait_sigs,
            &resolved.impl_sigs,
            &resolved.type_alias_sigs,
            &resolved.deprecated_sigs,
            &resolved.reexports,
            &namespace,
            source.file_id,
            sink,
        );
        checker.check_file(&source.ast);
    }
}

#[derive(Clone)]
struct DocSymbol {
    symbol: String,
    kind: String,
    signature: String,
    source_path: PathBuf,
    file_id: FileId,
    span: Span,
    param_names: HashSet<String>,
    return_requires_doc: bool,
    inline_doc: Option<ParsedDocComment>,
    has_inline_doc: bool,
    is_public: bool,
}

fn load_oridoc_index(
    _path: &Path,
    loaded: &[LoadedSource],
    config: Option<&ProjectConfig>,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> crate::oridoc::OridocIndex {
    let mut paths = Vec::new();
    for source in loaded {
        let sidecar = source.path.with_extension("oridoc");
        if sidecar.is_file() {
            paths.push(sidecar);
        }
    }

    let mut configured_paths = config
        .map(|config| config.doc_paths.clone())
        .unwrap_or_default();
    if configured_paths.is_empty() {
        if let Some(config) = config {
            let default_docs = config.root.join("docs/api");
            if default_docs.exists() {
                configured_paths.push(default_docs);
            }
        }
    }
    for path in configured_paths {
        collect_oridoc_paths(&path, &mut paths);
    }

    load_oridoc_index_from_paths(paths, cache, sink)
}

fn load_oridoc_index_from_paths(
    paths: Vec<PathBuf>,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> crate::oridoc::OridocIndex {
    let mut index = crate::oridoc::OridocIndex::default();
    for path in dedup_paths(paths) {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file_id = cache.add(&path, source.clone());
        let parsed = crate::oridoc::parse_oridoc(&path, &source);
        for diagnostic in parsed.diagnostics {
            sink.emit(
                Diagnostic::error("doc.syntax", diagnostic.message)
                    .with_label(Label::primary(
                        file_id,
                        diagnostic.span,
                        ".oridoc syntax here",
                    ))
                    .with_action(diagnostic.action),
            );
        }
        for mut entry in parsed.entries {
            entry.file_id = Some(file_id);
            index.insert(entry);
        }
    }
    index
}

fn collect_oridoc_paths(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "oridoc") {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_oridoc_paths(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "oridoc") {
            out.push(path);
        }
    }
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.insert(key) {
            out.push(path);
        }
    }
    out
}

fn validate_oridoc_index(
    loaded: &[LoadedSource],
    index: &crate::oridoc::OridocIndex,
    config: Option<&ProjectConfig>,
    sink: &mut DiagnosticSink,
) {
    let symbols = collect_doc_symbols(loaded);
    for entry in index.entries() {
        let Some(symbol) = symbols.get(&entry.symbol) else {
            let mut diagnostic = Diagnostic::error(
                "doc.symbol_not_found",
                format!("`.oridoc` documents unknown symbol `{}`", entry.symbol),
            )
            .with_action("rename the doc target or document a symbol that exists in the namespace");
            if let Some(file_id) = entry.file_id {
                diagnostic = diagnostic.with_label(Label::primary(
                    file_id,
                    entry.span,
                    "unknown documentation target",
                ));
            }
            sink.emit(diagnostic);
            continue;
        };

        for (name, _) in &entry.doc.params {
            if name.is_empty() || !symbol.param_names.contains(name) {
                let name = if name.is_empty() {
                    "missing parameter name"
                } else {
                    name.as_str()
                };
                let mut diagnostic = Diagnostic::warning(
                    "doc.param_name_mismatch",
                    format!(
                        "documentation tag `param {name}` does not match `{}`",
                        symbol.symbol
                    ),
                )
                .with_action("rename the param entry or remove it");
                if let Some(file_id) = entry.file_id {
                    diagnostic = diagnostic.with_label(Label::primary(
                        file_id,
                        entry.span,
                        "documentation entry here",
                    ));
                }
                sink.emit(diagnostic);
            }
        }

        if symbol.return_requires_doc && entry.doc.returns.is_none() {
            let mut diagnostic = Diagnostic::warning(
                "doc.missing_return",
                format!(
                    "documentation for `{}` is missing `returns:`",
                    symbol.symbol
                ),
            )
            .with_action("add a `returns:` section for the returned value");
            if let Some(file_id) = entry.file_id {
                diagnostic = diagnostic.with_label(Label::primary(
                    file_id,
                    entry.span,
                    "documentation entry here",
                ));
            }
            sink.emit(diagnostic);
        }
    }

    let requirement = config
        .map(|config| config.require_public_docs)
        .unwrap_or(DocRequirement::Off);
    if requirement == DocRequirement::Off {
        return;
    }
    let documented: HashSet<&str> = symbols
        .values()
        .filter(|symbol| symbol.has_inline_doc)
        .map(|symbol| symbol.symbol.as_str())
        .chain(index.symbols())
        .collect();
    for symbol in symbols.values() {
        if symbol.kind == "module"
            || !symbol.is_public
            || documented.contains(symbol.symbol.as_str())
        {
            continue;
        }
        let message = format!("public symbol `{}` has no documentation", symbol.symbol);
        let diagnostic = match requirement {
            DocRequirement::Warn => Diagnostic::warning("doc.missing_public", message),
            DocRequirement::Error => Diagnostic::error("doc.missing_public", message),
            DocRequirement::Off => continue,
        }
        .with_label(Label::primary(
            symbol.file_id,
            symbol.span,
            "public symbol without documentation",
        ))
        .with_action("add an inline doc comment or a matching `.oridoc` entry");
        sink.emit(diagnostic);
    }
}

fn collect_doc_symbols(loaded: &[LoadedSource]) -> BTreeMap<String, DocSymbol> {
    let mut symbols = BTreeMap::new();
    for source in loaded {
        let namespace = namespace_of(&source.ast);
        symbols.insert(
            namespace.clone(),
            DocSymbol {
                symbol: namespace.clone(),
                kind: "module".into(),
                signature: format!("module {namespace}"),
                source_path: source.path.clone(),
                file_id: source.file_id,
                span: source.ast.namespace.span,
                param_names: HashSet::new(),
                return_requires_doc: false,
                inline_doc: None,
                has_inline_doc: false,
                is_public: true,
            },
        );

        for item in &source.ast.items {
            let leading_start = item
                .attrs
                .first()
                .map(|attr| attr.span.start)
                .unwrap_or_else(|| item.item.span().start);
            let inline_doc = doc_comment_for(source, leading_start);
            match &item.item {
                Item::Func(func) => insert_doc_symbol(
                    &mut symbols,
                    source,
                    format!("{}.{}", namespace, func.name),
                    "function",
                    func_signature_text(source, func),
                    func.span,
                    &func.params,
                    func.return_ty.as_ref(),
                    inline_doc,
                    func.visibility.is_public(),
                ),
                Item::Struct(decl) => {
                    insert_doc_symbol_without_params(
                        &mut symbols,
                        source,
                        format!("{}.{}", namespace, decl.name),
                        "struct",
                        format!(
                            "{}struct {}{}{}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params),
                            where_text(source, decl.where_clause.as_ref())
                        ),
                        decl.span,
                        inline_doc,
                        decl.visibility.is_public(),
                    );
                    for method in &decl.methods {
                        insert_doc_symbol(
                            &mut symbols,
                            source,
                            format!("{}.{}.{}", namespace, decl.name, method.name),
                            "method",
                            func_signature_text(source, method),
                            method.span,
                            &method.params,
                            method.return_ty.as_ref(),
                            doc_comment_for(source, method.span.start),
                            method.visibility.is_public(),
                        );
                    }
                }
                Item::Enum(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    format!("{}.{}", namespace, decl.name),
                    "enum",
                    format!(
                        "{}enum {}{}",
                        visibility_prefix(decl.visibility),
                        decl.name,
                        type_params_text(&decl.type_params)
                    ),
                    decl.span,
                    inline_doc,
                    decl.visibility.is_public(),
                ),
                Item::Trait(decl) => {
                    insert_doc_symbol_without_params(
                        &mut symbols,
                        source,
                        format!("{}.{}", namespace, decl.name),
                        "trait",
                        format!(
                            "{}trait {}{}{}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params),
                            where_text(source, decl.where_clause.as_ref())
                        ),
                        decl.span,
                        inline_doc,
                        decl.visibility.is_public(),
                    );
                    for member in &decl.members {
                        match member {
                            TraitMember::Required(sig) => insert_doc_symbol(
                                &mut symbols,
                                source,
                                format!("{}.{}.{}", namespace, decl.name, sig.name),
                                "trait method",
                                func_signature_decl_text(source, sig),
                                sig.span,
                                &sig.params,
                                sig.return_ty.as_ref(),
                                doc_comment_for(source, sig.span.start),
                                sig.visibility.is_public(),
                            ),
                            TraitMember::Default(func) => insert_doc_symbol(
                                &mut symbols,
                                source,
                                format!("{}.{}.{}", namespace, decl.name, func.name),
                                "trait method",
                                func_signature_text(source, func),
                                func.span,
                                &func.params,
                                func.return_ty.as_ref(),
                                doc_comment_for(source, func.span.start),
                                func.visibility.is_public(),
                            ),
                            TraitMember::Type(_) => {}
                        }
                    }
                }
                Item::Apply(decl) => {
                    for member in &decl.free_members {
                        if let ori_ast::item::ApplyMember::Method(method) = member {
                            insert_doc_symbol(
                                &mut symbols,
                                source,
                                format!("{}.apply {}.{}", namespace, decl.for_type, method.name),
                                "apply free method",
                                func_signature_text(source, method),
                                method.span,
                                &method.params,
                                method.return_ty.as_ref(),
                                doc_comment_for(source, method.span.start),
                                method.visibility.is_public(),
                            );
                        }
                    }
                    for use_sec in &decl.uses {
                        for member in &use_sec.members {
                            if let ori_ast::item::ApplyMember::Method(method) = member {
                                insert_doc_symbol(
                                    &mut symbols,
                                    source,
                                    format!(
                                        "{}.apply {} use {}.{}",
                                        namespace, decl.for_type, use_sec.trait_name, method.name
                                    ),
                                    "apply method",
                                    func_signature_text(source, method),
                                    method.span,
                                    &method.params,
                                    method.return_ty.as_ref(),
                                    doc_comment_for(source, method.span.start),
                                    method.visibility.is_public(),
                                );
                            }
                        }
                    }
                }
                Item::Alias(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    format!("{}.{}", namespace, decl.name),
                    "alias",
                    format!(
                        "{}alias {}{} = {}",
                        visibility_prefix(decl.visibility),
                        decl.name,
                        type_params_text(&decl.type_params),
                        type_text(source, &decl.ty)
                    ),
                    decl.span,
                    inline_doc,
                    decl.visibility.is_public(),
                ),
                Item::Const(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    format!("{}.{}", namespace, decl.name),
                    "constant",
                    format!(
                        "{}const {}: {}",
                        visibility_prefix(decl.visibility),
                        decl.name,
                        type_text(source, &decl.ty)
                    ),
                    decl.span,
                    inline_doc,
                    decl.visibility.is_public(),
                ),
                Item::Var(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    format!("{}.{}", namespace, decl.name),
                    "variable",
                    format!(
                        "{}var {}: {}",
                        visibility_prefix(decl.visibility),
                        decl.name,
                        type_text(source, &decl.ty)
                    ),
                    decl.span,
                    inline_doc,
                    decl.visibility.is_public(),
                ),
                Item::Extern(decl) => {
                    for member in &decl.members {
                        match member {
                            ExternMember::Func {
                                visibility,
                                name,
                                params,
                                return_ty,
                                span,
                            } => insert_doc_symbol(
                                &mut symbols,
                                source,
                                format!("{}.{}", namespace, name),
                                "extern function",
                                func_signature_parts_text(
                                    source,
                                    *visibility,
                                    name.as_str(),
                                    params,
                                    return_ty.as_ref(),
                                    None,
                                ),
                                *span,
                                params,
                                return_ty.as_ref(),
                                doc_comment_for(source, span.start),
                                visibility.is_public(),
                            ),
                            ExternMember::Var {
                                visibility,
                                name,
                                ty,
                                span,
                            } => insert_doc_symbol_without_params(
                                &mut symbols,
                                source,
                                format!("{}.{}", namespace, name),
                                "extern variable",
                                format!(
                                    "{}var {}: {}",
                                    visibility_prefix(*visibility),
                                    name,
                                    type_text(source, ty)
                                ),
                                *span,
                                doc_comment_for(source, span.start),
                                visibility.is_public(),
                            ),
                        }
                    }
                }
            }
        }
    }
    symbols
}

fn insert_doc_symbol(
    symbols: &mut BTreeMap<String, DocSymbol>,
    source: &LoadedSource,
    symbol: String,
    kind: &str,
    signature: String,
    span: Span,
    params: &[Param],
    return_ty: Option<&Type>,
    inline_doc: Option<ParsedDocComment>,
    is_public: bool,
) {
    let has_inline_doc = inline_doc.is_some();
    symbols.insert(
        symbol.clone(),
        DocSymbol {
            symbol,
            kind: kind.into(),
            signature,
            source_path: source.path.clone(),
            file_id: source.file_id,
            span,
            param_names: params
                .iter()
                .map(|param| param.name.to_string())
                .collect::<HashSet<_>>(),
            return_requires_doc: return_type_requires_doc(return_ty),
            inline_doc,
            has_inline_doc,
            is_public,
        },
    );
}

fn insert_doc_symbol_without_params(
    symbols: &mut BTreeMap<String, DocSymbol>,
    source: &LoadedSource,
    symbol: String,
    kind: &str,
    signature: String,
    span: Span,
    inline_doc: Option<ParsedDocComment>,
    is_public: bool,
) {
    let has_inline_doc = inline_doc.is_some();
    symbols.insert(
        symbol.clone(),
        DocSymbol {
            symbol,
            kind: kind.into(),
            signature,
            source_path: source.path.clone(),
            file_id: source.file_id,
            span,
            param_names: HashSet::new(),
            return_requires_doc: false,
            inline_doc,
            has_inline_doc,
            is_public,
        },
    );
}

pub fn oridoc_hover_for_symbol(source_path: &Path, source: &str, symbol: &str) -> Option<String> {
    let namespace = namespace_from_source_text(source)?;
    let mut paths = Vec::new();
    let sidecar = source_path.with_extension("oridoc");
    if sidecar.is_file() {
        paths.push(sidecar);
    }
    if let Some(config) = project_config_for_docs(source_path).ok().flatten() {
        let mut configured_paths = config.doc_paths;
        if configured_paths.is_empty() {
            let default_docs = config.root.join("docs/api");
            if default_docs.exists() {
                configured_paths.push(default_docs);
            }
        }
        for path in configured_paths {
            collect_oridoc_paths(&path, &mut paths);
        }
    }

    let mut index = crate::oridoc::OridocIndex::default();
    for path in dedup_paths(paths) {
        let Ok(doc_source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let parsed = crate::oridoc::parse_oridoc(&path, &doc_source);
        if !parsed.diagnostics.is_empty() {
            continue;
        }
        for entry in parsed.entries {
            index.insert(entry);
        }
    }

    for candidate in hover_symbol_candidates(&namespace, symbol) {
        if let Some(entry) = index.get(&candidate) {
            return Some(crate::oridoc::hover_markdown(entry));
        }
    }
    None
}

fn namespace_from_source_text(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let line = line.trim();
        let rest = line
            .strip_prefix("module ")
            .or_else(|| line.strip_prefix("namespace "))?;
        rest.split_whitespace().next().map(str::to_string)
    })
}

fn hover_symbol_candidates(namespace: &str, symbol: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if symbol == namespace || symbol.starts_with(&format!("{namespace}.")) {
        candidates.push(symbol.to_string());
    } else {
        candidates.push(format!("{namespace}.{symbol}"));
        if symbol.contains('.') {
            candidates.push(symbol.to_string());
        }
    }
    candidates
}

#[derive(Clone)]
struct TestCase {
    name: String,
    span: ori_diagnostics::Span,
    is_async: bool,
}

fn collect_test_cases(
    loaded: &[LoadedSource],
    resolved: &ResolvedModule,
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

fn run_native_tests(
    hir: &ori_hir::HirModule,
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

fn inject_test_harness(module: &mut ori_hir::HirModule, test: &TestCase) {
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

pub fn format_source_text(source: &str) -> String {
    fmt::format_source_text(source)
}

fn validate_doc_tags(source: &LoadedSource, sink: &mut DiagnosticSink) {
    for item in &source.ast.items {
        let leading_start = item
            .attrs
            .first()
            .map(|attr| attr.span.start)
            .unwrap_or_else(|| item.item.span().start);
        match &item.item {
            Item::Func(func) => validate_func_doc_tags(
                source,
                leading_start,
                func.name.as_str(),
                &func.params,
                func.return_ty.as_ref(),
                sink,
            ),
            Item::Struct(decl) => {
                for method in &decl.methods {
                    validate_func_doc_tags(
                        source,
                        method.span.start,
                        method.name.as_str(),
                        &method.params,
                        method.return_ty.as_ref(),
                        sink,
                    );
                }
            }
            Item::Trait(decl) => {
                for member in &decl.members {
                    match member {
                        TraitMember::Required(sig) => validate_signature_doc_tags(
                            source,
                            sig.span.start,
                            sig.name.as_str(),
                            &sig.params,
                            sig.return_ty.as_ref(),
                            sink,
                        ),
                        TraitMember::Default(func) => validate_func_doc_tags(
                            source,
                            func.span.start,
                            func.name.as_str(),
                            &func.params,
                            func.return_ty.as_ref(),
                            sink,
                        ),
                        TraitMember::Type(_) => {}
                    }
                }
            }
            Item::Apply(decl) => {
                for member in decl
                    .free_members
                    .iter()
                    .chain(decl.uses.iter().flat_map(|u| u.members.iter()))
                {
                    if let ori_ast::item::ApplyMember::Method(method) = member {
                        validate_func_doc_tags(
                            source,
                            method.span.start,
                            method.name.as_str(),
                            &method.params,
                            method.return_ty.as_ref(),
                            sink,
                        );
                    }
                }
            }
            Item::Extern(decl) => {
                for member in &decl.members {
                    if let ExternMember::Func {
                        name,
                        params,
                        return_ty,
                        span,
                        ..
                    } = member
                    {
                        validate_signature_doc_tags(
                            source,
                            span.start,
                            name.as_str(),
                            params,
                            return_ty.as_ref(),
                            sink,
                        );
                    }
                }
            }
            Item::Enum(_) | Item::Alias(_) | Item::Const(_) | Item::Var(_) => {}
        }
    }
}

fn validate_func_doc_tags(
    source: &LoadedSource,
    leading_start: u32,
    func_name: &str,
    params: &[Param],
    return_ty: Option<&Type>,
    sink: &mut DiagnosticSink,
) {
    validate_doc_tags_for_signature(source, leading_start, func_name, params, return_ty, sink);
}

fn validate_signature_doc_tags(
    source: &LoadedSource,
    leading_start: u32,
    func_name: &str,
    params: &[Param],
    return_ty: Option<&Type>,
    sink: &mut DiagnosticSink,
) {
    validate_doc_tags_for_signature(source, leading_start, func_name, params, return_ty, sink);
}

fn validate_doc_tags_for_signature(
    source: &LoadedSource,
    leading_start: u32,
    func_name: &str,
    params: &[Param],
    return_ty: Option<&Type>,
    sink: &mut DiagnosticSink,
) {
    let Some(doc_span) = leading_block_comment_before(&source.tokens, leading_start) else {
        return;
    };
    let comment = &source.source[doc_span.as_range()];
    let param_names: HashSet<&str> = params.iter().map(|param| param.name.as_str()).collect();
    for tag in doc_param_tags(comment) {
        if tag.name.is_empty() || !param_names.contains(tag.name) {
            let name = if tag.name.is_empty() {
                "missing parameter name"
            } else {
                tag.name
            };
            sink.emit(
                Diagnostic::warning(
                    "doc.param_name_mismatch",
                    format!("documentation tag `@param {name}` does not match `{func_name}`"),
                )
                .with_label(Label::primary(
                    source.file_id,
                    doc_span,
                    "documentation comment here",
                ))
                .with_action("rename the @param tag or remove it"),
            );
        }
    }
    if return_type_requires_doc(return_ty) && !doc_has_return_tag(comment) {
        sink.emit(
            Diagnostic::warning(
                "doc.missing_return",
                format!("documentation for `{func_name}` is missing `@return`"),
            )
            .with_label(Label::primary(
                source.file_id,
                doc_span,
                "documentation comment here",
            ))
            .with_action("add `@return` or `@returns` for the returned value"),
        );
    }
}

fn leading_block_comment_before(
    tokens: &[Token],
    leading_start: u32,
) -> Option<ori_diagnostics::Span> {
    let item_index = tokens
        .iter()
        .position(|token| token.span.start >= leading_start)?;
    let mut index = item_index;
    while let Some(previous) = index.checked_sub(1) {
        let token = &tokens[previous];
        if token.kind == TokenKind::Public {
            index = previous;
            continue;
        }
        return (token.kind == TokenKind::BlockComment).then_some(token.span);
    }
    None
}

struct DocParamTag<'a> {
    name: &'a str,
}

fn doc_param_tags(comment: &str) -> Vec<DocParamTag<'_>> {
    let body = comment
        .strip_prefix("--|")
        .unwrap_or(comment)
        .strip_suffix("|--")
        .unwrap_or(comment);
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("@param")?;
            let rest = rest.trim_start();
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|ch: char| ch == ':' || ch == '-');
            Some(DocParamTag { name })
        })
        .collect()
}

fn return_type_requires_doc(return_ty: Option<&Type>) -> bool {
    !matches!(return_ty, None | Some(Type::Void(_)))
}

fn doc_has_return_tag(comment: &str) -> bool {
    cleaned_doc_lines(comment).iter().any(|line| {
        line.strip_prefix("@returns")
            .or_else(|| line.strip_prefix("@return"))
            .is_some_and(|text| !text.trim().is_empty())
    })
}

#[derive(Clone, Default)]
struct ParsedDocComment {
    body: Vec<String>,
    params: Vec<(String, String)>,
    returns: Option<String>,
}

struct StdlibDocSignature {
    module: &'static str,
    signature: &'static str,
}

const COLLECTION_STDLIB_DOC_SIGNATURES: &[StdlibDocSignature] = &[
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.new[T]() -> deque.Deque[T]",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.push_front[T](d: deque.Deque[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.push_back[T](d: deque.Deque[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.pop_front[T](d: deque.Deque[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.pop_back[T](d: deque.Deque[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.front[T](d: deque.Deque[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.back[T](d: deque.Deque[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.len[T](d: deque.Deque[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.is_empty[T](d: deque.Deque[T]) -> bool",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.clear[T](d: deque.Deque[T]) -> void",
    },
    StdlibDocSignature {
        module: "ori.deque",
        signature: "deque.to_list[T](d: deque.Deque[T]) -> list[T]",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.new[T]() -> queue.Queue[T]",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.enqueue[T](q: queue.Queue[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.dequeue[T](q: queue.Queue[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.peek[T](q: queue.Queue[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.len[T](q: queue.Queue[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.is_empty[T](q: queue.Queue[T]) -> bool",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.clear[T](q: queue.Queue[T]) -> void",
    },
    StdlibDocSignature {
        module: "ori.queue",
        signature: "queue.to_list[T](q: queue.Queue[T]) -> list[T]",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.new[T]() -> stack.Stack[T]",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.push[T](s: stack.Stack[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.pop[T](s: stack.Stack[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.peek[T](s: stack.Stack[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.len[T](s: stack.Stack[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.is_empty[T](s: stack.Stack[T]) -> bool",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.clear[T](s: stack.Stack[T]) -> void",
    },
    StdlibDocSignature {
        module: "ori.stack",
        signature: "stack.to_list[T](s: stack.Stack[T]) -> list[T]",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.new[T]() -> linked_list.LinkedList[T]",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.push_front[T](list: linked_list.LinkedList[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.push_back[T](list: linked_list.LinkedList[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.pop_front[T](list: linked_list.LinkedList[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.front[T](list: linked_list.LinkedList[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.len[T](list: linked_list.LinkedList[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.is_empty[T](list: linked_list.LinkedList[T]) -> bool",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.clear[T](list: linked_list.LinkedList[T]) -> void",
    },
    StdlibDocSignature {
        module: "ori.linked_list",
        signature: "linked_list.to_list[T](list: linked_list.LinkedList[T]) -> list[T]",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.new[T]() -> doubly_linked_list.DoublyLinkedList[T]",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.push_front[T](list: doubly_linked_list.DoublyLinkedList[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.push_back[T](list: doubly_linked_list.DoublyLinkedList[T], value: T) -> void",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.pop_front[T](list: doubly_linked_list.DoublyLinkedList[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.pop_back[T](list: doubly_linked_list.DoublyLinkedList[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.front[T](list: doubly_linked_list.DoublyLinkedList[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.back[T](list: doubly_linked_list.DoublyLinkedList[T]) -> optional[T]",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.len[T](list: doubly_linked_list.DoublyLinkedList[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.is_empty[T](list: doubly_linked_list.DoublyLinkedList[T]) -> bool",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.clear[T](list: doubly_linked_list.DoublyLinkedList[T]) -> void",
    },
    StdlibDocSignature {
        module: "ori.doubly_linked_list",
        signature: "doubly_linked_list.to_list[T](list: doubly_linked_list.DoublyLinkedList[T]) -> list[T]",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.new[T](root: T) -> tree.Tree[T]",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.root[T](t: tree.Tree[T]) -> tree.NodeId",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.value[T](t: tree.Tree[T], node: tree.NodeId) -> T",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.add_child[T](t: tree.Tree[T], parent: tree.NodeId, value: T) -> tree.NodeId",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.children[T](t: tree.Tree[T], node: tree.NodeId) -> list[tree.NodeId]",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.parent[T](t: tree.Tree[T], node: tree.NodeId) -> optional[tree.NodeId]",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.remove_subtree[T](t: tree.Tree[T], node: tree.NodeId) -> void",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.len[T](t: tree.Tree[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.depth[T](t: tree.Tree[T], node: tree.NodeId) -> int",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.pre_order[T](t: tree.Tree[T]) -> list[tree.NodeId]",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.post_order[T](t: tree.Tree[T]) -> list[tree.NodeId]",
    },
    StdlibDocSignature {
        module: "ori.tree",
        signature: "tree.breadth_first[T](t: tree.Tree[T]) -> list[tree.NodeId]",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.new[K, V]() -> hash_table.HashTable[K, V] for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.with_capacity[K, V](capacity: int) -> hash_table.HashTable[K, V] for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.set[K, V](table: hash_table.HashTable[K, V], key: K, value: V) -> void for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.get[K, V](table: hash_table.HashTable[K, V], key: K) -> optional[V] for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.remove[K, V](table: hash_table.HashTable[K, V], key: K) -> optional[V] for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.contains[K, V](table: hash_table.HashTable[K, V], key: K) -> bool for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.len[K, V](table: hash_table.HashTable[K, V]) -> int",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.capacity[K, V](table: hash_table.HashTable[K, V]) -> int",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.reserve[K, V](table: hash_table.HashTable[K, V], capacity: int) -> void",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.clear[K, V](table: hash_table.HashTable[K, V]) -> void",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.keys[K, V](table: hash_table.HashTable[K, V]) -> list[K]",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.values[K, V](table: hash_table.HashTable[K, V]) -> list[V]",
    },
    StdlibDocSignature {
        module: "ori.hash_table",
        signature: "hash_table.entries[K, V](table: hash_table.HashTable[K, V]) -> list[tuple[K, V]]",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.new[N](directed: bool) -> graph.Graph[N] for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.add_node[N](g: graph.Graph[N], node: N) -> void for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.remove_node[N](g: graph.Graph[N], node: N) -> void for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.add_edge[N](g: graph.Graph[N], from: N, to: N) -> void for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.remove_edge[N](g: graph.Graph[N], from: N, to: N) -> void for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.has_node[N](g: graph.Graph[N], node: N) -> bool for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.has_edge[N](g: graph.Graph[N], from: N, to: N) -> bool for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.neighbors[N](g: graph.Graph[N], node: N) -> list[N] for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.nodes[N](g: graph.Graph[N]) -> list[N]",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.edges[N](g: graph.Graph[N]) -> list[tuple[N, N]]",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.bfs[N](g: graph.Graph[N], start: N) -> list[N] for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.dfs[N](g: graph.Graph[N], start: N) -> list[N] for N: Hashable, N: Equatable",
    },
    StdlibDocSignature {
        module: "ori.graph",
        signature: "graph.topological_sort[N](g: graph.Graph[N]) -> list[N]",
    },
    StdlibDocSignature {
        module: "ori.heap",
        signature: "heap.new[T]() -> heap.Heap[T] for T: Comparable",
    },
    StdlibDocSignature {
        module: "ori.heap",
        signature: "heap.push[T](h: heap.Heap[T], value: T) -> void for T: Comparable",
    },
    StdlibDocSignature {
        module: "ori.heap",
        signature: "heap.pop[T](h: heap.Heap[T]) -> optional[T] for T: Comparable",
    },
    StdlibDocSignature {
        module: "ori.heap",
        signature: "heap.peek[T](h: heap.Heap[T]) -> optional[T] for T: Comparable",
    },
    StdlibDocSignature {
        module: "ori.heap",
        signature: "heap.len[T](h: heap.Heap[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.heap",
        signature: "heap.is_empty[T](h: heap.Heap[T]) -> bool",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.new[K, V]() -> map[K, V] for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.set[K, V](m: map[K, V], key: K, value: V) -> void for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.get[K, V](m: map[K, V], key: K) -> V for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.contains[K, V](m: map[K, V], key: K) -> bool for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.remove[K, V](m: map[K, V], key: K) -> void for K: Hashable, K: Equatable",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.len[K, V](m: map[K, V]) -> int",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.capacity[K, V](m: map[K, V]) -> int",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.reserve[K, V](m: map[K, V], capacity: int) -> void",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.clear[K, V](m: map[K, V]) -> void",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.keys[K, V](m: map[K, V]) -> list[K]",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.values[K, V](m: map[K, V]) -> list[V]",
    },
    StdlibDocSignature {
        module: "ori.map",
        signature: "maps.entries[K, V](m: map[K, V]) -> list[tuple[K, V]]",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.new[T]() -> set[T] for T: Hashable, T: Equatable",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.add[T](s: set[T], value: T) -> void for T: Hashable, T: Equatable",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.contains[T](s: set[T], value: T) -> bool for T: Hashable, T: Equatable",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.remove[T](s: set[T], value: T) -> void for T: Hashable, T: Equatable",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.len[T](s: set[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.capacity[T](s: set[T]) -> int",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.reserve[T](s: set[T], capacity: int) -> void",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.clear[T](s: set[T]) -> void",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.union[T](a: set[T], b: set[T]) -> set[T] for T: Hashable, T: Equatable",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.intersection[T](a: set[T], b: set[T]) -> set[T] for T: Hashable, T: Equatable",
    },
    StdlibDocSignature {
        module: "ori.set",
        signature: "sets.difference[T](a: set[T], b: set[T]) -> set[T] for T: Hashable, T: Equatable",
    },
];

/// Lookup a human-readable stdlib signature for hover/docs (Layer 1 collections + ops).
pub fn stdlib_doc_signature(canonical_path: &str) -> Option<&'static str> {
    let (module, func_name) = canonical_path.rsplit_once('.')?;
    COLLECTION_STDLIB_DOC_SIGNATURES
        .iter()
        .find(|entry| {
            entry.module == module
                && entry
                    .signature
                    .split('(')
                    .next()
                    .and_then(|prefix| prefix.rsplit('.').next())
                    == Some(func_name)
        })
        .map(|entry| entry.signature)
}

// ── Doctor ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.checks.iter().any(|c| c.status == DoctorStatus::Fail)
    }
}

/// Environment and toolchain sanity checks for the Ori native pipeline.
pub fn run_doctor() -> DoctorReport {
    let mut checks = Vec::new();

    match find_stdlib_root() {
        Some(path) => {
            let orl_count = count_orl_files(&path);
            checks.push(DoctorCheck {
                name: "stdlib root",
                status: DoctorStatus::Ok,
                detail: format!("{} ({} `.orl` modules)", path.display(), orl_count),
            });
        }
        None => checks.push(DoctorCheck {
            name: "stdlib root",
            status: DoctorStatus::Fail,
            detail: "not found — set ORI_STDLIB_ROOT or install the packaged stdlib/ layout".into(),
        }),
    }

    let target = native_target_triple();
    checks.push(DoctorCheck {
        name: "target triple",
        status: DoctorStatus::Ok,
        detail: target.clone(),
    });

    match find_native_runtime_link() {
        Ok(link) => checks.push(DoctorCheck {
            name: "native runtime (AOT)",
            status: DoctorStatus::Ok,
            detail: link.runtime_lib.display().to_string(),
        }),
        Err(err) => checks.push(DoctorCheck {
            name: "native runtime (AOT)",
            status: DoctorStatus::Fail,
            detail: err,
        }),
    }

    match find_native_runtime_cdylib() {
        Ok(path) => checks.push(DoctorCheck {
            name: "native runtime (JIT cdylib)",
            status: DoctorStatus::Ok,
            detail: path.display().to_string(),
        }),
        Err(err) => checks.push(DoctorCheck {
            name: "native runtime (JIT cdylib)",
            status: DoctorStatus::Warn,
            detail: format!("{err} (ori run falls back to AOT when unset)"),
        }),
    }

    let linker_detail = match ori_codegen::NativeLinker::discover() {
        Ok(linker) => {
            let name = linker.strategy_name();
            let suffix = if env_flag("ORI_NATIVE_LINKER") {
                " (ORI_NATIVE_LINKER)"
            } else if env_flag("ORI_USE_BUNDLED_RUST_LLD") {
                " (ORI_USE_BUNDLED_RUST_LLD=1)"
            } else if env_flag("ORI_USE_SYSTEM_LINKER") {
                " (ORI_USE_SYSTEM_LINKER=1)"
            } else if env_flag("ORI_USE_RUSTC_DRIVER") {
                " (ORI_USE_RUSTC_DRIVER=1)"
            } else {
                " (default)"
            };
            (DoctorStatus::Ok, format!("{name}{suffix}"))
        }
        Err(err) => (
            DoctorStatus::Warn,
            format!("{err} (AOT compile will fail until resolved)"),
        ),
    };
    checks.push(DoctorCheck {
        name: "linker strategy",
        status: linker_detail.0,
        detail: linker_detail.1,
    });

    let run_mode = if should_use_jit_for_run() {
        "JIT (in-process Cranelift)"
    } else {
        "AOT compile + link"
    };
    checks.push(DoctorCheck {
        name: "ori run mode",
        status: DoctorStatus::Ok,
        detail: run_mode.into(),
    });

    if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") {
        checks.push(DoctorCheck {
            name: "packaged runtime gate",
            status: DoctorStatus::Ok,
            detail: "ORI_REQUIRE_PACKAGED_RUNTIME=1 (release smoke mode)".into(),
        });
    }

    DoctorReport { checks }
}

// ── Project summary ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SummaryImport {
    pub path: String,
    pub alias: Option<String>,
    pub selected: Vec<SummaryImportItem>,
}

#[derive(Debug, Clone)]
pub struct SummaryImportItem {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SummaryModule {
    pub path: PathBuf,
    pub namespace: String,
    pub imports: Vec<SummaryImport>,
}

#[derive(Debug, Clone)]
pub struct SummaryOutput {
    pub entry: PathBuf,
    pub modules: Vec<SummaryModule>,
    pub diagnostic_count: usize,
}

/// Build a lightweight project overview: entry file, namespaces, import graph.
pub fn run_summary(path: &Path) -> Result<SummaryOutput, String> {
    let output = run_check(path)?;
    let entry = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut modules = Vec::new();

    for file in output.cache.all_files() {
        let mut sink = DiagnosticSink::default();
        let tokens = ori_lexer::lex(&file.content, file.id, &mut sink);
        let ast = ori_parser::parse(&tokens, &file.content, file.id, &mut sink);
        let imports = ast
            .imports
            .iter()
            .map(|import| SummaryImport {
                path: import.path.to_string(),
                alias: import.alias.as_ref().map(|a| a.text.to_string()),
                selected: import
                    .selected
                    .iter()
                    .map(|item| SummaryImportItem {
                        name: item.name.text.to_string(),
                        alias: item.alias.as_ref().map(|a| a.text.to_string()),
                    })
                    .collect(),
            })
            .collect();
        modules.push(SummaryModule {
            path: file.path.clone(),
            namespace: ast.namespace.name.to_string(),
            imports,
        });
    }

    modules.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(SummaryOutput {
        entry,
        modules,
        diagnostic_count: output.diagnostics.len(),
    })
}

pub fn format_summary_text(summary: &SummaryOutput) -> String {
    let mut out = String::new();
    out.push_str(&format!("entry: {}\n", summary.entry.display()));
    out.push_str(&format!(
        "modules: {} ({} diagnostic(s) from last check)\n\n",
        summary.modules.len(),
        summary.diagnostic_count
    ));
    for module in &summary.modules {
        out.push_str(&format!(
            "- {} → namespace {}\n",
            module.path.display(),
            module.namespace
        ));
        for import in &module.imports {
            if !import.selected.is_empty() {
                let selected = import
                    .selected
                    .iter()
                    .map(|item| {
                        if let Some(alias) = &item.alias {
                            format!("{} = {}", item.name, alias)
                        } else {
                            item.name.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("    import {} ({})\n", import.path, selected));
            } else if let Some(alias) = &import.alias {
                out.push_str(&format!("    import {} = {}\n", import.path, alias));
            } else {
                out.push_str(&format!("    import {}\n", import.path));
            }
        }
    }
    out
}

fn count_orl_files(root: &std::path::Path) -> usize {
    fn walk(dir: &std::path::Path, count: &mut usize) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, count);
            } else if path.extension().is_some_and(|e| e == "orl") {
                *count += 1;
            }
        }
    }
    let mut count = 0;
    walk(root, &mut count);
    count
}

fn render_documentation_markdown(
    loaded: &[LoadedSource],
    external_docs: &crate::oridoc::OridocIndex,
    doc_mode: ProjectDocMode,
) -> String {
    let mut out = String::from("# Ori API Documentation\n\n");
    let mut entry_count = 0usize;
    let symbols = collect_doc_symbols(loaded);
    let mut skip_inline = HashSet::new();

    if doc_mode == ProjectDocMode::SidecarFirst {
        for entry in external_docs.entries() {
            if let Some(symbol) = symbols.get(&entry.symbol) {
                append_oridoc_entry(&mut out, symbol, entry);
                skip_inline.insert(symbol.symbol.clone());
                entry_count += 1;
            }
        }
    }

    for source in loaded {
        entry_count += render_source_documentation(source, &mut out, &skip_inline);
    }

    if doc_mode == ProjectDocMode::InlineFirst {
        for entry in external_docs.entries() {
            if let Some(symbol) = symbols.get(&entry.symbol) {
                if symbol.has_inline_doc {
                    continue;
                }
                append_oridoc_entry(&mut out, symbol, entry);
                entry_count += 1;
            }
        }
    }

    if entry_count == 0 {
        out.push_str("No documentation comments found.\n\n");
    }

    append_stdlib_documentation(&mut out);

    out
}

fn append_stdlib_documentation(out: &mut String) {
    // Module list is derived from the stdlib manifest via the single source
    // of truth in `ori-types::stdlib`. Do not reimplement module derivation
    // here; `implemented_stdlib_modules()` covers canonical paths, `ori.*`
    // aliases (e.g. `ori.files`), and the module-only allowlist
    // (`ori`, `ori.core`, `ori.Error`, `ori.mem`, `ori.concurrent`).
    let modules: BTreeSet<&'static str> = ori_types::stdlib::implemented_stdlib_modules()
        .into_iter()
        .collect();

    out.push_str("## Standard Library\n\n");
    out.push_str("### Modules\n\n");
    for module in modules {
        let _ = writeln!(out, "- `{module}`");
    }
    out.push('\n');

    let mut by_module: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for entry in COLLECTION_STDLIB_DOC_SIGNATURES {
        by_module
            .entry(entry.module)
            .or_default()
            .push(entry.signature);
    }

    out.push_str("### Collection Signatures\n\n");
    for (module, signatures) in by_module {
        let _ = writeln!(out, "#### `{module}`\n");
        out.push_str("```ori\n");
        for signature in signatures {
            let _ = writeln!(out, "{signature}");
        }
        out.push_str("```\n\n");
    }
}

fn render_source_documentation(
    source: &LoadedSource,
    out: &mut String,
    skip_inline: &HashSet<String>,
) -> usize {
    if !skip_inline.is_empty() {
        return render_source_documentation_from_symbols(source, out, skip_inline);
    }

    let mut entry_count = 0usize;
    let namespace = namespace_of(&source.ast);

    for item in &source.ast.items {
        let leading_start = item
            .attrs
            .first()
            .map(|attr| attr.span.start)
            .unwrap_or_else(|| item.item.span().start);

        match &item.item {
            Item::Func(func) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, func.name),
                        "function",
                        &func_signature_text(source, func),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
            }
            Item::Struct(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "struct",
                        &format!(
                            "{}struct {}{}{}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params),
                            where_text(source, decl.where_clause.as_ref())
                        ),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
                for method in &decl.methods {
                    if let Some(doc) = doc_comment_for(source, method.span.start) {
                        append_doc_entry(
                            out,
                            &format!("{}.{}.{}", namespace, decl.name, method.name),
                            "method",
                            &func_signature_text(source, method),
                            &doc,
                            source,
                        );
                        entry_count += 1;
                    }
                }
            }
            Item::Enum(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "enum",
                        &format!(
                            "{}enum {}{}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params)
                        ),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
            }
            Item::Trait(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "trait",
                        &format!(
                            "{}trait {}{}{}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params),
                            where_text(source, decl.where_clause.as_ref())
                        ),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
                for member in &decl.members {
                    match member {
                        TraitMember::Required(sig) => {
                            if let Some(doc) = doc_comment_for(source, sig.span.start) {
                                append_doc_entry(
                                    out,
                                    &format!("{}.{}.{}", namespace, decl.name, sig.name),
                                    "trait method",
                                    &func_signature_decl_text(source, sig),
                                    &doc,
                                    source,
                                );
                                entry_count += 1;
                            }
                        }
                        TraitMember::Default(func) => {
                            if let Some(doc) = doc_comment_for(source, func.span.start) {
                                append_doc_entry(
                                    out,
                                    &format!("{}.{}.{}", namespace, decl.name, func.name),
                                    "trait method",
                                    &func_signature_text(source, func),
                                    &doc,
                                    source,
                                );
                                entry_count += 1;
                            }
                        }
                        TraitMember::Type(_) => {}
                    }
                }
            }
            Item::Apply(decl) => {
                for member in &decl.free_members {
                    if let ori_ast::item::ApplyMember::Method(method) = member {
                        if let Some(doc) = doc_comment_for(source, method.span.start) {
                            append_doc_entry(
                                out,
                                &format!("{}.apply {}.{}", namespace, decl.for_type, method.name),
                                "apply free method",
                                &func_signature_text(source, method),
                                &doc,
                                source,
                            );
                            entry_count += 1;
                        }
                    }
                }
                for use_sec in &decl.uses {
                    for member in &use_sec.members {
                        if let ori_ast::item::ApplyMember::Method(method) = member {
                            if let Some(doc) = doc_comment_for(source, method.span.start) {
                                append_doc_entry(
                                    out,
                                    &format!(
                                        "{}.apply {} use {}.{}",
                                        namespace, decl.for_type, use_sec.trait_name, method.name
                                    ),
                                    "apply method",
                                    &func_signature_text(source, method),
                                    &doc,
                                    source,
                                );
                                entry_count += 1;
                            }
                        }
                    }
                }
            }
            Item::Alias(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "alias",
                        &format!(
                            "{}alias {}{} = {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params),
                            type_text(source, &decl.ty)
                        ),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
            }
            Item::Const(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "constant",
                        &format!(
                            "{}const {}: {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_text(source, &decl.ty)
                        ),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
            }
            Item::Var(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "variable",
                        &format!(
                            "{}var {}: {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_text(source, &decl.ty)
                        ),
                        &doc,
                        source,
                    );
                    entry_count += 1;
                }
            }
            Item::Extern(decl) => {
                for member in &decl.members {
                    match member {
                        ExternMember::Func {
                            visibility,
                            name,
                            params,
                            return_ty,
                            span,
                        } => {
                            if let Some(doc) = doc_comment_for(source, span.start) {
                                append_doc_entry(
                                    out,
                                    &format!("{}.{}", namespace, name),
                                    "extern function",
                                    &func_signature_parts_text(
                                        source,
                                        *visibility,
                                        name.as_str(),
                                        params,
                                        return_ty.as_ref(),
                                        None,
                                    ),
                                    &doc,
                                    source,
                                );
                                entry_count += 1;
                            }
                        }
                        ExternMember::Var {
                            visibility,
                            name,
                            ty,
                            span,
                        } => {
                            if let Some(doc) = doc_comment_for(source, span.start) {
                                append_doc_entry(
                                    out,
                                    &format!("{}.{}", namespace, name),
                                    "extern variable",
                                    &format!(
                                        "{}var {}: {}",
                                        visibility_prefix(*visibility),
                                        name,
                                        type_text(source, ty)
                                    ),
                                    &doc,
                                    source,
                                );
                                entry_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    entry_count
}

fn render_source_documentation_from_symbols(
    source: &LoadedSource,
    out: &mut String,
    skip_inline: &HashSet<String>,
) -> usize {
    let mut entry_count = 0usize;
    let symbols = collect_doc_symbols(std::slice::from_ref(source));
    for symbol in symbols.values() {
        if skip_inline.contains(&symbol.symbol) {
            continue;
        }
        let Some(doc) = &symbol.inline_doc else {
            continue;
        };
        append_doc_entry_with_source_path(
            out,
            &symbol.symbol,
            &symbol.kind,
            &symbol.signature,
            doc,
            &symbol.source_path,
        );
        entry_count += 1;
    }
    entry_count
}

fn append_oridoc_entry(out: &mut String, symbol: &DocSymbol, entry: &crate::oridoc::OridocEntry) {
    let doc = ParsedDocComment {
        body: entry.doc.body.clone(),
        params: entry.doc.params.clone(),
        returns: entry.doc.returns.clone(),
    };
    append_doc_entry_with_source_path(
        out,
        &symbol.symbol,
        &symbol.kind,
        &symbol.signature,
        &doc,
        &symbol.source_path,
    );
}

fn doc_comment_for(source: &LoadedSource, leading_start: u32) -> Option<ParsedDocComment> {
    let span = leading_block_comment_before(&source.tokens, leading_start)?;
    Some(parse_doc_comment(&source.source[span.as_range()]))
}

fn parse_doc_comment(comment: &str) -> ParsedDocComment {
    let mut doc = ParsedDocComment::default();
    for line in cleaned_doc_lines(comment) {
        if let Some(rest) = line.strip_prefix("@param") {
            let rest = rest.trim_start();
            let mut parts = rest.splitn(2, char::is_whitespace);
            let name = parts
                .next()
                .unwrap_or("")
                .trim_matches(|ch| ch == ':' || ch == '-');
            let description = parts.next().unwrap_or("").trim();
            doc.params.push((name.to_string(), description.to_string()));
        } else if let Some(rest) = line
            .strip_prefix("@returns")
            .or_else(|| line.strip_prefix("@return"))
        {
            let text = rest.trim();
            if !text.is_empty() {
                doc.returns = Some(text.to_string());
            }
        } else {
            doc.body.push(line);
        }
    }
    trim_empty_doc_lines(&mut doc.body);
    doc
}

fn cleaned_doc_lines(comment: &str) -> Vec<String> {
    let body = comment
        .strip_prefix("--|")
        .unwrap_or(comment)
        .strip_suffix("|--")
        .unwrap_or(comment);
    let mut lines: Vec<String> = body
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix('*')
                .unwrap_or(trimmed)
                .trim()
                .to_string()
        })
        .collect();
    trim_empty_doc_lines(&mut lines);
    lines
}

fn trim_empty_doc_lines(lines: &mut Vec<String>) {
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
}

fn append_doc_entry(
    out: &mut String,
    name: &str,
    kind: &str,
    signature: &str,
    doc: &ParsedDocComment,
    source: &LoadedSource,
) {
    append_doc_entry_with_source_path(out, name, kind, signature, doc, &source.path);
}

fn append_doc_entry_with_source_path(
    out: &mut String,
    name: &str,
    kind: &str,
    signature: &str,
    doc: &ParsedDocComment,
    source_path: &Path,
) {
    let _ = writeln!(out, "## {name}");
    let _ = writeln!(out);
    let _ = writeln!(out, "- Kind: {kind}");
    let _ = writeln!(out, "- Source: {}", source_path.display());
    let _ = writeln!(out);
    let _ = writeln!(out, "```ori");
    let _ = writeln!(out, "{signature}");
    let _ = writeln!(out, "```");
    let _ = writeln!(out);

    if !doc.body.is_empty() {
        for line in &doc.body {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out);
    }

    if !doc.params.is_empty() {
        let _ = writeln!(out, "Parameters:");
        for (name, description) in &doc.params {
            if description.is_empty() {
                let _ = writeln!(out, "- `{name}`");
            } else {
                let _ = writeln!(out, "- `{name}`: {description}");
            }
        }
        let _ = writeln!(out);
    }

    if let Some(returns) = &doc.returns {
        let _ = writeln!(out, "Returns: {returns}");
        let _ = writeln!(out);
    }
}

fn func_signature_text(source: &LoadedSource, func: &ori_ast::item::FuncDecl) -> String {
    func_signature_parts_text(
        source,
        func.visibility,
        func.name.as_str(),
        &func.params,
        func.return_ty.as_ref(),
        func.where_clause.as_ref(),
    )
}

fn func_signature_decl_text(source: &LoadedSource, sig: &ori_ast::item::FuncSignature) -> String {
    func_signature_parts_text(
        source,
        sig.visibility,
        sig.name.as_str(),
        &sig.params,
        sig.return_ty.as_ref(),
        sig.where_clause.as_ref(),
    )
}

fn func_signature_parts_text(
    source: &LoadedSource,
    visibility: ori_ast::Visibility,
    name: &str,
    params: &[Param],
    return_ty: Option<&Type>,
    where_clause: Option<&WhereClause>,
) -> String {
    let params = params
        .iter()
        .map(|param| param_signature_text(source, param))
        .collect::<Vec<_>>()
        .join(", ");
    let mut signature = format!("{}{}({})", visibility_prefix(visibility), name, params);
    if let Some(return_ty) = return_ty {
        signature.push_str(" -> ");
        signature.push_str(&type_text(source, return_ty));
    }
    signature.push_str(&where_text(source, where_clause));
    signature
}

fn param_signature_text(source: &LoadedSource, param: &Param) -> String {
    let mut text = format!("{}: {}", param.name, type_text(source, &param.ty));
    if matches!(param.kind, ori_ast::ParamKind::Variadic) {
        text.push_str("...");
    }
    text
}

fn type_text(source: &LoadedSource, ty: &Type) -> String {
    clean_source_fragment(&source.source[ty.span().as_range()])
}

fn where_text(source: &LoadedSource, where_clause: Option<&WhereClause>) -> String {
    where_clause
        .map(|clause| {
            format!(
                " {}",
                clean_source_fragment(&source.source[clause.span.as_range()])
            )
        })
        .unwrap_or_default()
}

fn type_params_text(params: &TypeParams) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!(
            "[{}]",
            params
                .iter()
                .map(|param| param.name.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn clean_source_fragment(fragment: &str) -> String {
    fragment.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn visibility_prefix(visibility: ori_ast::Visibility) -> &'static str {
    if visibility.is_public() {
        "public "
    } else {
        ""
    }
}

fn lower_loaded_sources(
    loaded: &[LoadedSource],
    resolved: &ResolvedModule,
    sink: &mut DiagnosticSink,
) -> ori_hir::HirModule {
    let (first, rest) = loaded.split_first().expect("entry source is loaded");
    let first_namespace = namespace_of(&first.ast);
    let mut merged = ori_hir::lower(
        &first.ast,
        &resolved.def_map,
        &resolved.func_sigs,
        &resolved.value_sigs,
        &resolved.struct_sigs,
        &resolved.enum_sigs,
        &resolved.trait_sigs,
        &resolved.impl_sigs,
        &resolved.type_alias_sigs,
        &resolved.reexports,
        &first_namespace,
        first.file_id,
        sink,
    );
    for source in rest {
        let namespace = namespace_of(&source.ast);
        let mut hir = ori_hir::lower(
            &source.ast,
            &resolved.def_map,
            &resolved.func_sigs,
            &resolved.value_sigs,
            &resolved.struct_sigs,
            &resolved.enum_sigs,
            &resolved.trait_sigs,
            &resolved.impl_sigs,
            &resolved.type_alias_sigs,
            &resolved.reexports,
            &namespace,
            source.file_id,
            sink,
        );
        merged.structs.append(&mut hir.structs);
        merged.enums.append(&mut hir.enums);
        merged.traits.append(&mut hir.traits);
        merged.trait_impls.append(&mut hir.trait_impls);
        merged.funcs.append(&mut hir.funcs);
        merged.consts.append(&mut hir.consts);
        merged.externs.append(&mut hir.externs);
    }

    // Automatically append stdlib enums (e.g. ori.json.Value) to the HirModule
    // so they are correctly registered in layout computations in code generation backends.
    let json_val_def_id = resolved.def_map.lookup("ori.json.Value");
    if let Some(concrete_id) = json_val_def_id {
        if !merged.enums.iter().any(|e| e.def_id == concrete_id) {
            if let Some(sig) = resolved.enum_sigs.iter().find(|s| s.def_id == concrete_id) {
                let variants = sig
                    .variants
                    .iter()
                    .map(|v| {
                        let fields = v
                            .fields
                            .iter()
                            .map(|(fname, fty)| ori_hir::hir::HirField {
                                name: fname.clone(),
                                ty: fty.clone(),
                                contract: None,
                                span: ori_diagnostics::Span::DUMMY,
                            })
                            .collect();
                        ori_hir::hir::HirVariant {
                            name: v.name.clone(),
                            fields,
                            span: ori_diagnostics::Span::DUMMY,
                        }
                    })
                    .collect();
                let hir_enum = ori_hir::hir::HirEnum {
                    def_id: concrete_id,
                    name: smol_str::SmolStr::new("ori.json.Value"),
                    variants,
                    is_public: true,
                    span: ori_diagnostics::Span::DUMMY,
                };
                merged.enums.push(hir_enum);
            }
        }
    }

    ori_hir::insert_default_arguments(&mut merged);
    ori_hir::monomorphize_generics(&mut merged);
    merged
}

// The native route now links against the Rust ori-runtime static library. C
// emission remains available only as the explicit debug route `ori emit c`.
