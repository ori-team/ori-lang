//! Frontend entry points for the Ori driver.
//!
//! This module owns the source-facing lex, parse, and type-check operations.
//! Project loading stays in `project.rs`; HIR lowering stays in `lowering.rs`.

use ori_diagnostics::{Diagnostic, DiagnosticSink, SourceCache};
use ori_types::conditional::CfgContext;
use ori_types::resolve::ResolvedModule;
use std::path::Path;

use super::docs::validate_doc_tags;
use super::project::{load_and_resolve, namespace_of, read_file, LoadedSource};
use super::timing::report_internal_pipeline_timing;

pub struct LexOutput {
    pub cache: SourceCache,
    pub tokens: Vec<ori_lexer::Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct ParseOutput {
    pub cache: SourceCache,
    pub ast: ori_ast::item::SourceFile,
    pub tokens: Vec<ori_lexer::Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct CheckOutput {
    pub cache: SourceCache,
    pub resolved: ResolvedModule,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

/// Options for checking an in-memory source module.
#[derive(Clone, Debug, Default)]
pub struct CheckOptions {
    /// Override manifest/environment cfg selection for hosted callers.
    pub cfg: Option<CfgContext>,
}

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

pub fn run_check(path: &Path) -> Result<CheckOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let load_started = std::time::Instant::now();
    let sources = load_and_resolve(path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;
    report_internal_pipeline_timing("check.load_and_resolve", load_started.elapsed());

    // Type checking â€” only if no fatal parse errors so far
    let check_started = std::time::Instant::now();
    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
    }
    report_internal_pipeline_timing("check.type_check", check_started.elapsed());

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
    run_check_source_with_options(path, source, CheckOptions::default())
}

/// Check an in-memory source module with explicit hosted configuration.
pub fn run_check_source_with_options(
    path: &Path,
    source: String,
    options: CheckOptions,
) -> Result<CheckOutput, String> {
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

pub(super) fn check_loaded_sources(
    loaded: &[LoadedSource],
    resolved: &ResolvedModule,
    sink: &mut DiagnosticSink,
) {
    if loaded.len() > 1 {
        use rayon::prelude::*;
        let per_source_diags: Vec<Vec<Diagnostic>> = loaded
            .par_iter()
            .map(|source| {
                let mut local_sink = DiagnosticSink::default();
                validate_doc_tags(source, &mut local_sink);
                let namespace = namespace_of(&source.ast);
                let mut checker = ori_types::check::Checker::new(
                    resolved,
                    &namespace,
                    source.file_id,
                    &mut local_sink,
                );
                checker.check_file(&source.ast);
                local_sink.into_diagnostics()
            })
            .collect();

        for diags in per_source_diags {
            for diag in diags {
                sink.emit(diag);
            }
        }
    } else {
        for source in loaded {
            validate_doc_tags(source, sink);
            let namespace = namespace_of(&source.ast);
            let mut checker =
                ori_types::check::Checker::new(resolved, &namespace, source.file_id, sink);
            checker.check_file(&source.ast);
        }
    }
}
