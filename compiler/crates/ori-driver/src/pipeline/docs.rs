//! Documentation extraction, validation, and rendering for the Ori driver.
//!
//! This module owns inline/sidecar documentation, the `.oridoc` index, and
//! Markdown/HTML rendering. The pipeline facade only re-exports its command
//! contract.

use ori_ast::common::{TypeParams, WhereClause};
use ori_ast::item::{ExternMember, Item, Param, TraitMember};
use ori_ast::ty::Type;
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, SourceCache, Span};
use ori_lexer::{Token, TokenKind};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::doc_html;
use super::frontend::check_loaded_sources;
use super::project::{
    dedup_paths, load_and_resolve, namespace_of, project_config_for_docs, DocRequirement,
    LoadedSource, ProjectConfig, ProjectDocMode,
};

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

pub fn run_doc(path: &Path) -> Result<DocOutput, String> {
    run_doc_with_options(path, DocOptions::default())
}

pub fn run_doc_with_options(path: &Path, options: DocOptions) -> Result<DocOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let sources = load_and_resolve(path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;
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
    let sources = load_and_resolve(path, &mut cache, &mut sink)?;
    let loaded = sources.loaded;
    let resolved = sources.resolved;

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

// â”€â”€ Utilities â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
                    DocSymbolInput {
                        symbol: format!("{}.{}", namespace, func.name),
                        kind: "function",
                        signature: func_signature_text(source, func),
                        span: func.span,
                        inline_doc,
                        is_public: func.visibility.is_public(),
                    },
                    &func.params,
                    func.return_ty.as_ref(),
                ),
                Item::Struct(decl) => {
                    insert_doc_symbol_without_params(
                        &mut symbols,
                        source,
                        DocSymbolInput {
                            symbol: format!("{}.{}", namespace, decl.name),
                            kind: "struct",
                            signature: format!(
                                "{}struct {}{}{}",
                                visibility_prefix(decl.visibility),
                                decl.name,
                                type_params_text(&decl.type_params),
                                where_text(source, decl.where_clause.as_ref())
                            ),
                            span: decl.span,
                            inline_doc,
                            is_public: decl.visibility.is_public(),
                        },
                    );
                    for method in &decl.methods {
                        insert_doc_symbol(
                            &mut symbols,
                            source,
                            DocSymbolInput {
                                symbol: format!("{}.{}.{}", namespace, decl.name, method.name),
                                kind: "method",
                                signature: func_signature_text(source, method),
                                span: method.span,
                                inline_doc: doc_comment_for(source, method.span.start),
                                is_public: method.visibility.is_public(),
                            },
                            &method.params,
                            method.return_ty.as_ref(),
                        );
                    }
                }
                Item::Enum(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    DocSymbolInput {
                        symbol: format!("{}.{}", namespace, decl.name),
                        kind: "enum",
                        signature: format!(
                            "{}enum {}{}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params)
                        ),
                        span: decl.span,
                        inline_doc,
                        is_public: decl.visibility.is_public(),
                    },
                ),
                Item::Trait(decl) => {
                    insert_doc_symbol_without_params(
                        &mut symbols,
                        source,
                        DocSymbolInput {
                            symbol: format!("{}.{}", namespace, decl.name),
                            kind: "trait",
                            signature: format!(
                                "{}trait {}{}{}",
                                visibility_prefix(decl.visibility),
                                decl.name,
                                type_params_text(&decl.type_params),
                                where_text(source, decl.where_clause.as_ref())
                            ),
                            span: decl.span,
                            inline_doc,
                            is_public: decl.visibility.is_public(),
                        },
                    );
                    for member in &decl.members {
                        match member {
                            TraitMember::Required(sig) => insert_doc_symbol(
                                &mut symbols,
                                source,
                                DocSymbolInput {
                                    symbol: format!("{}.{}.{}", namespace, decl.name, sig.name),
                                    kind: "trait method",
                                    signature: func_signature_decl_text(source, sig),
                                    span: sig.span,
                                    inline_doc: doc_comment_for(source, sig.span.start),
                                    is_public: sig.visibility.is_public(),
                                },
                                &sig.params,
                                sig.return_ty.as_ref(),
                            ),
                            TraitMember::Default(func) => insert_doc_symbol(
                                &mut symbols,
                                source,
                                DocSymbolInput {
                                    symbol: format!("{}.{}.{}", namespace, decl.name, func.name),
                                    kind: "trait method",
                                    signature: func_signature_text(source, func),
                                    span: func.span,
                                    inline_doc: doc_comment_for(source, func.span.start),
                                    is_public: func.visibility.is_public(),
                                },
                                &func.params,
                                func.return_ty.as_ref(),
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
                                DocSymbolInput {
                                    symbol: format!(
                                        "{}.apply {}.{}",
                                        namespace, decl.for_type, method.name
                                    ),
                                    kind: "apply free method",
                                    signature: func_signature_text(source, method),
                                    span: method.span,
                                    inline_doc: doc_comment_for(source, method.span.start),
                                    is_public: method.visibility.is_public(),
                                },
                                &method.params,
                                method.return_ty.as_ref(),
                            );
                        }
                    }
                    for use_sec in &decl.uses {
                        for member in &use_sec.members {
                            if let ori_ast::item::ApplyMember::Method(method) = member {
                                insert_doc_symbol(
                                    &mut symbols,
                                    source,
                                    DocSymbolInput {
                                        symbol: format!(
                                            "{}.apply {} use {}.{}",
                                            namespace,
                                            decl.for_type,
                                            use_sec.trait_name,
                                            method.name
                                        ),
                                        kind: "apply method",
                                        signature: func_signature_text(source, method),
                                        span: method.span,
                                        inline_doc: doc_comment_for(source, method.span.start),
                                        is_public: method.visibility.is_public(),
                                    },
                                    &method.params,
                                    method.return_ty.as_ref(),
                                );
                            }
                        }
                    }
                }
                Item::Alias(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    DocSymbolInput {
                        symbol: format!("{}.{}", namespace, decl.name),
                        kind: "alias",
                        signature: format!(
                            "{}alias {}{} = {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_params_text(&decl.type_params),
                            type_text(source, &decl.ty)
                        ),
                        span: decl.span,
                        inline_doc,
                        is_public: decl.visibility.is_public(),
                    },
                ),
                Item::Newtype(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    DocSymbolInput {
                        symbol: format!("{}.{}", namespace, decl.name),
                        kind: "newtype",
                        signature: format!(
                            "{}newtype {} = {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_text(source, &decl.repr)
                        ),
                        span: decl.span,
                        inline_doc,
                        is_public: decl.visibility.is_public(),
                    },
                ),
                Item::Const(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    DocSymbolInput {
                        symbol: format!("{}.{}", namespace, decl.name),
                        kind: "constant",
                        signature: format!(
                            "{}const {}: {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_text(source, &decl.ty)
                        ),
                        span: decl.span,
                        inline_doc,
                        is_public: decl.visibility.is_public(),
                    },
                ),
                Item::Var(decl) => insert_doc_symbol_without_params(
                    &mut symbols,
                    source,
                    DocSymbolInput {
                        symbol: format!("{}.{}", namespace, decl.name),
                        kind: "variable",
                        signature: format!(
                            "{}var {}: {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_text(source, &decl.ty)
                        ),
                        span: decl.span,
                        inline_doc,
                        is_public: decl.visibility.is_public(),
                    },
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
                                DocSymbolInput {
                                    symbol: format!("{}.{}", namespace, name),
                                    kind: "extern function",
                                    signature: func_signature_parts_text(
                                        source,
                                        *visibility,
                                        name.as_str(),
                                        params,
                                        return_ty.as_ref(),
                                        None,
                                    ),
                                    span: *span,
                                    inline_doc: doc_comment_for(source, span.start),
                                    is_public: visibility.is_public(),
                                },
                                params,
                                return_ty.as_ref(),
                            ),
                            ExternMember::Var {
                                visibility,
                                name,
                                ty,
                                span,
                            } => insert_doc_symbol_without_params(
                                &mut symbols,
                                source,
                                DocSymbolInput {
                                    symbol: format!("{}.{}", namespace, name),
                                    kind: "extern variable",
                                    signature: format!(
                                        "{}var {}: {}",
                                        visibility_prefix(*visibility),
                                        name,
                                        type_text(source, ty)
                                    ),
                                    span: *span,
                                    inline_doc: doc_comment_for(source, span.start),
                                    is_public: visibility.is_public(),
                                },
                            ),
                        }
                    }
                }
            }
        }
    }
    symbols
}

struct DocSymbolInput {
    symbol: String,
    kind: &'static str,
    signature: String,
    span: Span,
    inline_doc: Option<ParsedDocComment>,
    is_public: bool,
}

fn insert_doc_symbol(
    symbols: &mut BTreeMap<String, DocSymbol>,
    source: &LoadedSource,
    input: DocSymbolInput,
    params: &[Param],
    return_ty: Option<&Type>,
) {
    let DocSymbolInput {
        symbol,
        kind,
        signature,
        span,
        inline_doc,
        is_public,
    } = input;
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
    input: DocSymbolInput,
) {
    let DocSymbolInput {
        symbol,
        kind,
        signature,
        span,
        inline_doc,
        is_public,
    } = input;
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

pub(super) fn validate_doc_tags(source: &LoadedSource, sink: &mut DiagnosticSink) {
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
            Item::Enum(_) | Item::Alias(_) | Item::Newtype(_) | Item::Const(_) | Item::Var(_) => {}
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

pub(super) struct StdlibDocSignature {
    pub(super) module: &'static str,
    pub(super) signature: &'static str,
}

pub(super) const COLLECTION_STDLIB_DOC_SIGNATURES: &[StdlibDocSignature] = &[
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
            Item::Newtype(decl) => {
                if let Some(doc) = doc_comment_for(source, leading_start) {
                    append_doc_entry(
                        out,
                        &format!("{}.{}", namespace, decl.name),
                        "newtype",
                        &format!(
                            "{}newtype {} = {}",
                            visibility_prefix(decl.visibility),
                            decl.name,
                            type_text(source, &decl.repr)
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
