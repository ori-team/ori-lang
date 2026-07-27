mod handlers;
mod index;
mod stdlib_catalog;
mod utils;

use index::project::ProjectManager;
use index::project_semantic::ProjectSemanticIndex;
use index::semantic::{CompletionContext, SemanticIndex, SemanticSymbol};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use stdlib_catalog::{import_alias_map, stdlib_catalog};
use tokio::sync::RwLock;
use tokio::time::Instant;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    CodeLens, CodeLensOptions, CodeLensParams, Command, CompletionItem, CompletionItemKind,
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentFormattingParams, DocumentRangeFormattingParams, DocumentSymbol, DocumentSymbolParams,
    DocumentSymbolResponse, ExecuteCommandParams, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, InitializeParams, InitializeResult, InitializedParams, InlayHint,
    InlayHintKind, InlayHintLabel, InlayHintParams, Location, MessageType, OneOf, Position,
    PrepareRenameResponse, Range, ReferenceParams, RenameParams, SaveOptions, SemanticToken,
    SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    ServerCapabilities, ServerInfo, SignatureHelp, SignatureHelpOptions, SignatureHelpParams,
    SignatureInformation, SymbolInformation, SymbolKind, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, TextEdit, Url, WorkDoneProgressOptions, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    project: Arc<RwLock<ProjectManager>>,
    last_change: Arc<RwLock<HashMap<Url, Instant>>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            project: Arc::new(RwLock::new(ProjectManager::new())),
            last_change: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn validate_uri(&self, uri: Url) {
        let source = {
            let project = self.project.read().await;
            project.document_content(&uri)
        };
        let path = utils::uri::document_path_from_uri(&uri);
        let result = match (path.as_deref(), source.clone()) {
            (Some(path), Some(source)) => ori_driver::pipeline::run_check_source(path, source),
            (Some(path), None) => ori_driver::pipeline::run_check(path),
            _ => return,
        };
        let mut diagnostics = match result {
            Ok(output) => {
                let diags = if let Some(target) = &path {
                    let mut d = handlers::diagnostics::diagnostics_for_path(
                        &output.cache,
                        &output.diagnostics,
                        target,
                    );
                    // Etapa 6.5: surface project-level diagnostics (e.g.
                    // circular imports) whose label sits on another file so
                    // the user sees them on the file they are editing.
                    d.extend(handlers::diagnostics::project_diagnostics_for_path(
                        &output.cache,
                        &output.diagnostics,
                        target,
                    ));
                    d
                } else {
                    Vec::new()
                };
                // Capture the cross-file semantic index (Etapa 6.1) so that
                // hover / go-to-definition / completion / find-references can
                // resolve symbols across imports. The index is rebuilt on
                // every successful `run_check_source`, which is debounced for
                // `didChange` and immediate for `didOpen` / `didSave`.
                if let Some(target) = &path {
                    let active_path =
                        std::fs::canonicalize(target).unwrap_or_else(|_| target.to_owned());
                    let index =
                        ProjectSemanticIndex::new(output.resolved, output.cache, active_path);
                    self.project
                        .write()
                        .await
                        .upsert_semantic_index(uri.clone(), index);
                }
                diags
            }
            Err(message) => {
                // Etapa 6.5: surface known project-configuration failures
                // under structured `project.*` codes; fall back to a generic
                // file error for anything else.
                if let Some(diag) = handlers::diagnostics::project_error_diagnostic(&message) {
                    vec![diag]
                } else {
                    vec![handlers::diagnostics::file_error_diagnostic(message)]
                }
            }
        };
        if let Some(ref src) = source {
            let config = handlers::lint::LintConfig::default();
            let lint_diags = handlers::lint::lint(src, &config);
            diagnostics.extend(lint_diags);
        }
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn schedule_debounced_validate(&self, uri: Url) {
        let now = Instant::now();
        {
            let mut last = self.last_change.write().await;
            last.insert(uri.clone(), now);
        }

        let client = self.client.clone();
        let project = Arc::clone(&self.project);
        let last_change = Arc::clone(&self.last_change);

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;

            let should_run = {
                let last = last_change.read().await;
                last.get(&uri).map(|t| *t <= now).unwrap_or(false)
            };
            if !should_run {
                return;
            }

            let source = {
                let proj = project.read().await;
                proj.document_content(&uri)
            };
            let path = utils::uri::document_path_from_uri(&uri);
            let result = match (path.as_deref(), source) {
                (Some(path), Some(source)) => ori_driver::pipeline::run_check_source(path, source),
                (Some(path), None) => ori_driver::pipeline::run_check(path),
                _ => return,
            };
            let diagnostics = match result {
                Ok(output) => {
                    let diags = if let Some(target) = &path {
                        let mut d = handlers::diagnostics::diagnostics_for_path(
                            &output.cache,
                            &output.diagnostics,
                            target,
                        );
                        d.extend(handlers::diagnostics::project_diagnostics_for_path(
                            &output.cache,
                            &output.diagnostics,
                            target,
                        ));
                        d
                    } else {
                        Vec::new()
                    };
                    // Capture the cross-file semantic index (Etapa 6.1).
                    if let Some(target) = &path {
                        let active_path =
                            std::fs::canonicalize(target).unwrap_or_else(|_| target.to_owned());
                        let index =
                            ProjectSemanticIndex::new(output.resolved, output.cache, active_path);
                        project
                            .write()
                            .await
                            .upsert_semantic_index(uri.clone(), index);
                    }
                    diags
                }
                Err(message) => {
                    // Etapa 6.5: map known project failures to `project.*`.
                    if let Some(diag) = handlers::diagnostics::project_error_diagnostic(&message) {
                        vec![diag]
                    } else {
                        vec![handlers::diagnostics::file_error_diagnostic(message)]
                    }
                }
            };
            client.publish_diagnostics(uri, diagnostics, None).await;
        });
    }

    async fn get_source_and_index(&self, uri: &Url) -> Option<(String, SemanticIndex)> {
        let source = {
            let project = self.project.read().await;
            project.document_content(uri)
        }?;
        let index = {
            let project = self.project.read().await;
            project
                .document_index(uri)
                .cloned()
                .unwrap_or_else(|| SemanticIndex::build(&source))
        };
        Some((source, index))
    }

    /// Fetch the cross-file `ProjectSemanticIndex` snapshot for `uri`, if one
    /// has been produced by a successful `run_check_source` since the last
    /// edit. Used by hover / go-to-definition / completion / find-references
    /// to resolve symbols across imports (Etapa 6.1 / 6.2).
    async fn get_semantic_index(&self, uri: &Url) -> Option<std::sync::Arc<ProjectSemanticIndex>> {
        let project = self.project.read().await;
        project.semantic_index(uri)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            let root = root_uri.to_file_path().ok();
            self.project.write().await.set_workspace_root(root);
        }
        Ok(InitializeResult {
            capabilities: server_capabilities(),
            server_info: Some(ServerInfo {
                name: "ori-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ori-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.project.write().await.upsert_document(
            uri.clone(),
            params.text_document.text,
            params.text_document.version,
        );
        self.validate_uri(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        {
            let mut project = self.project.write().await;
            for change in params.content_changes {
                if let Some(range) = change.range {
                    project.apply_change(&uri, range, &change.text, version);
                } else {
                    project.upsert_document(uri.clone(), change.text, version);
                }
            }
        }
        self.schedule_debounced_validate(uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.validate_uri(params.text_document.uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.project.write().await.remove_document(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    // ── Hover ────────────────────────────────────────────────────────────────

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };

        if let Some(qualified) = utils::uri::qualified_ident_at_position(&source, position) {
            if let Some(hover_text) = handlers::hover::stdlib_hover(&qualified) {
                return Ok(Some(handlers::hover::markdown_hover(hover_text)));
            }
            if let Some(hover_text) = resolve_stdlib_qualified_hover(&source, &qualified) {
                return Ok(Some(handlers::hover::markdown_hover(hover_text)));
            }
        }

        let Some(symbol) = utils::uri::word_at_position(&source, position) else {
            return Ok(None);
        };
        if let Some(hover_text) = handlers::hover::stdlib_hover(&symbol) {
            return Ok(Some(handlers::hover::markdown_hover(hover_text)));
        }
        if let Some(hover_text) = handlers::hover::builtin_type_hover(&symbol) {
            return Ok(Some(handlers::hover::markdown_hover(hover_text)));
        }
        if symbol == "it" && source.contains(" if it") {
            return Ok(Some(handlers::hover::markdown_hover(
                "`it`\n\nCurrent value checked by a contract on a field or parameter.".into(),
            )));
        }
        if let Some(path) = utils::uri::document_path_from_uri(&uri) {
            if let Some(hover_text) =
                ori_driver::pipeline::oridoc_hover_for_symbol(&path, &source, &symbol)
            {
                return Ok(Some(handlers::hover::markdown_hover(hover_text)));
            }
        }
        if let Some(local_symbol) = index.symbol_at(&source, position) {
            return Ok(Some(handlers::hover::markdown_hover(
                local_symbol.hover.clone(),
            )));
        }
        if let Some(hover_text) = index.hover(&symbol) {
            return Ok(Some(handlers::hover::markdown_hover(hover_text)));
        }
        // Etapa 6.1: fall back to the cross-file project semantic index for
        // symbols resolved by `run_check` (e.g. imported structs/enums/funcs).
        if let Some(psi) = self.get_semantic_index(&uri).await {
            if let Some(hover_text) = psi.cross_file_hover(&symbol) {
                return Ok(Some(handlers::hover::markdown_hover(hover_text)));
            }
        }
        Ok(None)
    }

    // ── Go-to-definition ─────────────────────────────────────────────────────

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };

        if let Some(qualified) = utils::uri::qualified_ident_at_position(&source, position) {
            if let Some((path, range)) = resolve_stdlib_definition(&source, &qualified) {
                if let Ok(target_uri) = Url::from_file_path(&path) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        target_uri, range,
                    ))));
                }
            }
        }

        let Some(symbol) = utils::uri::word_at_position(&source, position) else {
            return Ok(None);
        };
        if let Some(range) = index.definition(&symbol) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                uri, range,
            ))));
        }
        if let Some(target_uri) = self.resolve_import_target(&index, &symbol).await {
            if let Some((target_source, _)) = self.get_source_and_index(&target_uri).await {
                let target_index = SemanticIndex::build(&target_source);
                if let Some(range) = target_index.definition(&symbol) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        target_uri, range,
                    ))));
                }
            }
        }
        // Etapa 6.1: cross-file go-to-definition via the driver's `DefMap`.
        // Resolves symbols defined in transitively imported files without
        // needing the import target to be open in the workspace.
        if let Some(psi) = self.get_semantic_index(&uri).await {
            if let Some((path, range)) = psi.cross_file_definition(&symbol) {
                if let Ok(target_uri) = Url::from_file_path(&path) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        target_uri, range,
                    ))));
                }
            }
        }
        Ok(None)
    }

    // ── Find references ──────────────────────────────────────────────────────

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };
        let Some(symbol) = utils::uri::word_at_position(&source, position) else {
            return Ok(None);
        };
        let mut locations: Vec<Location> = index
            .find_references(&source, &symbol)
            .into_iter()
            .map(|range| Location::new(uri.clone(), range))
            .collect();

        // Etapa 6.2: cross-file find-references via the driver's
        // `SourceCache`, scanning every loaded source (entry + transitive
        // imports) for word-boundary occurrences of the symbol.
        if let Some(psi) = self.get_semantic_index(&uri).await {
            for (path, range) in psi.find_references_cross_file(&symbol) {
                if let Ok(ref_uri) = Url::from_file_path(&path) {
                    locations.push(Location::new(ref_uri, range));
                }
            }
        }

        // Deduplicate by (uri, range) since the local index and the project
        // index may both surface the occurrence in the active file.
        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });
        locations.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    // ── Completions ──────────────────────────────────────────────────────────

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let mut items: Vec<CompletionItem> = Vec::new();

        let context = if let Some((source, index)) = self.get_source_and_index(&uri).await {
            index.completion_context(&source, position)
        } else {
            CompletionContext::Default
        };

        let source_for_stdlib = self
            .get_source_and_index(&uri)
            .await
            .map(|(s, _)| s)
            .unwrap_or_default();

        match context {
            CompletionContext::AfterDot { receiver } => {
                // `ori.` as a path prefix (often mis-detected when buffer is
                // mid-edit) → list stdlib modules, not a phantom `ori.ori` member.
                if receiver == "ori" {
                    items.extend(handlers::completion::stdlib_import_completion_items("ori."));
                } else {
                    items.extend(handlers::completion::stdlib_dot_completion_items(
                        &receiver,
                        &source_for_stdlib,
                    ));
                    if let Some((source, _)) = self.get_source_and_index(&uri).await {
                        if let Some(psi) = self.get_semantic_index(&uri).await {
                            items.extend(psi.complete_after_dot(&receiver, &source));
                        }
                    }
                }
            }
            CompletionContext::Import => {
                let prefix = import_prefix_at_position(&source_for_stdlib, position);
                // Modules only — do not mix keywords here. Keywords pollute the
                // client fuzzy list (shorter names rank above `ori.io`) and hide
                // the stdlib under `max_suggestions`.
                items.extend(handlers::completion::stdlib_import_completion_items(
                    &prefix,
                ));
                // Import-clause keywords only when the path is empty / after space.
                if prefix.is_empty() || prefix.ends_with(' ') {
                    items.extend(handlers::completion::import_keyword_completion_items());
                }
            }
            CompletionContext::Default => {
                items.extend(handlers::completion::stdlib_completion_items());
                items.extend(handlers::completion::keyword_completion_items());
                items.extend(handlers::completion::snippet_completion_items());
                if let Some((source, index)) = self.get_source_and_index(&uri).await {
                    let partial =
                        utils::uri::word_at_position(&source, position).unwrap_or_default();
                    for import in index.imports() {
                        if import.alias.starts_with(&partial) || partial.is_empty() {
                            let detail = import
                                .file_path
                                .as_ref()
                                .map(|path| {
                                    format!("import {} ({})", import.namespace, path.display())
                                })
                                .unwrap_or_else(|| format!("import {}", import.namespace));
                            items.push(CompletionItem {
                                label: import.alias.clone(),
                                kind: Some(CompletionItemKind::MODULE),
                                detail: Some(detail),
                                ..CompletionItem::default()
                            });
                        }
                    }
                    for sym in index.all_symbols() {
                        if sym.name.starts_with(&partial) || partial.is_empty() {
                            items.push(CompletionItem {
                                label: sym.name.clone(),
                                kind: Some(symbol_kind_to_cik(&sym.kind)),
                                detail: Some(sym.summary.clone()),
                                ..CompletionItem::default()
                            });
                        }
                    }
                }
            }
        }

        items.sort_by(|a, b| a.label.cmp(&b.label));
        handlers::completion::dedupe_completion_items(&mut items);
        Ok(Some(CompletionResponse::Array(items)))
    }

    // ── Document Symbols ─────────────────────────────────────────────────────

    #[allow(deprecated)]
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some((_source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };
        let mut symbols: Vec<DocumentSymbol> = Vec::new();

        for sym in index.all_symbols() {
            if matches!(
                sym.kind,
                index::semantic::SymbolKind::Function
                    | index::semantic::SymbolKind::Struct
                    | index::semantic::SymbolKind::Enum
                    | index::semantic::SymbolKind::Trait
            ) {
                let children = if sym.kind == index::semantic::SymbolKind::Struct {
                    index
                        .all_symbols()
                        .filter(|s| {
                            s.kind == index::semantic::SymbolKind::Field
                                && s.range.start.line >= sym.range.start.line
                                && s.range.end.line <= sym.range.end.line + 5
                        })
                        .map(|s| DocumentSymbol {
                            name: s.name.clone(),
                            detail: Some(s.summary.clone()),
                            kind: SymbolKind::FIELD,
                            range: s.range,
                            selection_range: s.range,
                            children: None,
                            tags: None,
                            deprecated: None,
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                symbols.push(DocumentSymbol {
                    name: sym.name.clone(),
                    detail: Some(sym.summary.clone()),
                    kind: semantic_kind_to_lsp(&sym.kind),
                    range: sym.range,
                    selection_range: sym.range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                    tags: None,
                    deprecated: None,
                });
            }
        }

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        }
    }

    // ── Code Actions ─────────────────────────────────────────────────────────

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut actions: Vec<CodeActionOrCommand> = Vec::new();

        for diag in &params.context.diagnostics {
            let code = diag
                .code
                .as_ref()
                .and_then(|c| match c {
                    tower_lsp::lsp_types::NumberOrString::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");

            match code {
                "type.unused_result" => {
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Discard result explicitly with `const _ =`".into(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some({
                                let mut map = HashMap::new();
                                map.insert(
                                    params.text_document.uri.clone(),
                                    vec![TextEdit {
                                        range: Range::new(diag.range.start, diag.range.start),
                                        new_text: "const _ = ".into(),
                                    }],
                                );
                                map
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }));

                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Propagate result with `?`".into(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some({
                                let mut map = HashMap::new();
                                map.insert(
                                    params.text_document.uri.clone(),
                                    vec![TextEdit {
                                        range: Range::new(diag.range.end, diag.range.end),
                                        new_text: "?".into(),
                                    }],
                                );
                                map
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }));
                }
                "type.expected_bool" => {
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Wrap in boolean check".into(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        ..Default::default()
                    }));
                }
                _ => {}
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    // ── Inlay Hints ──────────────────────────────────────────────────────────

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let Some((_source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };
        let requested_range = params.range;
        let mut hints: Vec<InlayHint> = Vec::new();

        for sym in index.all_symbols() {
            match sym.kind {
                index::semantic::SymbolKind::Variable | index::semantic::SymbolKind::Parameter => {
                    let hint_pos = Position {
                        line: sym.range.end.line,
                        character: sym.range.end.character,
                    };
                    if !position_in_range(hint_pos, &requested_range) {
                        continue;
                    }
                    hints.push(InlayHint {
                        position: hint_pos,
                        label: InlayHintLabel::String(format!(": {}", sym.summary)),
                        kind: Some(InlayHintKind::TYPE),
                        padding_left: Some(true),
                        padding_right: Some(false),
                        text_edits: None,
                        tooltip: None,
                        data: None,
                    });
                }
                _ => {}
            }
        }

        if hints.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hints))
        }
    }

    // ── Semantic Tokens ──────────────────────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };

        let mut raw_tokens: Vec<(u32, u32, u32, SemanticTokenType, Vec<SemanticTokenModifier>)> =
            Vec::new();

        for sym in index.all_symbols() {
            let (token_type, modifiers) = classify_semantic_token(&sym);
            let line = sym.range.start.line;
            let start_char = sym.range.start.character;
            let length = sym.name.len() as u32;

            raw_tokens.push((line, start_char, length, token_type, modifiers));
        }

        for kw in ORI_KEYWORDS {
            let mut search_start = 0usize;
            while let Some(pos) = source[search_start..].find(kw) {
                let abs_pos = search_start + pos;
                let before = abs_pos == 0
                    || source
                        .as_bytes()
                        .get(abs_pos - 1)
                        .map_or(true, |b| !b.is_ascii_alphanumeric() && *b != b'_');
                let after = source
                    .as_bytes()
                    .get(abs_pos + kw.len())
                    .map_or(true, |b| !b.is_ascii_alphanumeric() && *b != b'_');
                if before && after {
                    let pos = utils::position::position_for_byte_offset(&source, abs_pos);
                    raw_tokens.push((
                        pos.line,
                        pos.character,
                        kw.len() as u32,
                        SemanticTokenType::KEYWORD,
                        vec![],
                    ));
                }
                search_start = abs_pos + 1;
                if search_start >= source.len() {
                    break;
                }
            }
        }

        raw_tokens.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let data = encode_semantic_tokens(&raw_tokens, &SEMANTIC_LEGEND);
        if data.is_empty() {
            return Ok(None);
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    // ── Workspace Symbols ────────────────────────────────────────────────────

    #[allow(deprecated)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let mut results: Vec<SymbolInformation> = Vec::new();

        let project = self.project.read().await;
        let open_docs: Vec<(Url, String)> = project.all_open_documents();

        for (uri, source) in &open_docs {
            let index = SemanticIndex::build(source);
            for sym in index.all_symbols() {
                if sym.name.to_lowercase().contains(&query) || query.is_empty() {
                    results.push(SymbolInformation {
                        name: sym.name.clone(),
                        kind: semantic_kind_to_lsp(&sym.kind),
                        location: Location::new(uri.clone(), sym.range),
                        container_name: None,
                        tags: None,
                        deprecated: None,
                    });
                }
            }
        }

        results.truncate(100);

        if results.is_empty() {
            Ok(None)
        } else {
            Ok(Some(results))
        }
    }

    // ── Rename ───────────────────────────────────────────────────────────────

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };
        let Some(old_name) = utils::uri::word_at_position(&source, position) else {
            return Ok(None);
        };

        if ORI_KEYWORDS.contains(&old_name.as_str()) || is_builtin_type(&old_name) {
            return Ok(None);
        }

        let refs = index.find_references(&source, &old_name);
        let mut edits = Vec::new();
        for range in &refs {
            edits.push(TextEdit {
                range: *range,
                new_text: new_name.clone(),
            });
        }

        if let Some(range) = index.definition(&old_name) {
            edits.push(TextEdit {
                range,
                new_text: new_name.clone(),
            });
        }

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        if !edits.is_empty() {
            changes.insert(uri.clone(), edits);
        }

        // Etapa 6.2: cross-file rename. Include occurrences in transitively
        // imported files via the project semantic index. The active file is
        // already covered above; imported files get their own edit buckets.
        if let Some(psi) = self.get_semantic_index(&uri).await {
            for (path, range) in psi.find_references_cross_file(&old_name) {
                if let Ok(ref_uri) = Url::from_file_path(&path) {
                    if ref_uri == uri {
                        continue;
                    }
                    changes.entry(ref_uri).or_default().push(TextEdit {
                        range,
                        new_text: new_name.clone(),
                    });
                }
            }
            // Also jump to the cross-file definition site when it lives in an
            // imported file, so the binding itself is renamed.
            if let Some((path, range)) = psi.cross_file_definition(&old_name) {
                if let Ok(def_uri) = Url::from_file_path(&path) {
                    if def_uri != uri {
                        changes.entry(def_uri).or_default().push(TextEdit {
                            range,
                            new_text: new_name.clone(),
                        });
                    }
                }
            }
        }

        if changes.is_empty() {
            return Ok(None);
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };
        let Some(word) = utils::uri::word_at_position(&source, position) else {
            return Ok(None);
        };

        if ORI_KEYWORDS.contains(&word.as_str()) || is_builtin_type(&word) {
            return Ok(None);
        }

        let range = index.definition(&word).unwrap_or_else(|| {
            let line = position.line as usize;
            if let Some(line_str) = source.lines().nth(line) {
                if let Some(col) = line_str.find(&word) {
                    Range {
                        start: Position {
                            line: position.line,
                            character: col as u32,
                        },
                        end: Position {
                            line: position.line,
                            character: (col + word.len()) as u32,
                        },
                    }
                } else {
                    Range {
                        start: position,
                        end: position,
                    }
                }
            } else {
                Range {
                    start: position,
                    end: position,
                }
            }
        });

        Ok(Some(PrepareRenameResponse::Range(range)))
    }

    // ── Signature Help ───────────────────────────────────────────────────────

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };

        let (func_name, open_paren_pos) = match find_enclosing_call(&source, position) {
            Some(v) => v,
            None => return Ok(None),
        };

        let import_map = import_alias_map(&source);
        if let Some(sig) = stdlib_catalog().signature_for_call(&func_name, &import_map) {
            let param_slice = sig.find('(').map(|i| &sig[i..]).unwrap_or("()");
            let params = parse_params_from_signature(param_slice);
            let active_param = count_commas_before(&source, open_paren_pos, position);
            let max_param = params.len().saturating_sub(1) as u32;
            let active = (active_param as u32).min(max_param);
            return Ok(Some(SignatureHelp {
                signatures: vec![SignatureInformation {
                    label: sig.clone(),
                    documentation: Some(tower_lsp::lsp_types::Documentation::String(
                        "Ori standard library".into(),
                    )),
                    parameters: Some(
                        params
                            .iter()
                            .map(|p| tower_lsp::lsp_types::ParameterInformation {
                                label: tower_lsp::lsp_types::ParameterLabel::Simple(p.clone()),
                                documentation: None,
                            })
                            .collect(),
                    ),
                    active_parameter: None,
                }],
                active_signature: Some(0),
                active_parameter: Some(active),
            }));
        }

        let func_sym = match index
            .all_symbols()
            .find(|s| s.name == func_name && s.kind == index::semantic::SymbolKind::Function)
        {
            Some(s) => s,
            None => return Ok(None),
        };

        let sig_str = &func_sym.summary;
        let params = parse_params_from_signature(sig_str);

        let active_param = count_commas_before(&source, open_paren_pos, position);

        let label = format!("{}{}", func_name, sig_str);
        let max_param = params.len().saturating_sub(1) as u32;
        let active = (active_param as u32).min(max_param);

        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label,
                documentation: None,
                parameters: Some(
                    params
                        .iter()
                        .map(|p| tower_lsp::lsp_types::ParameterInformation {
                            label: tower_lsp::lsp_types::ParameterLabel::Simple(p.clone()),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: None,
            }],
            active_signature: Some(0),
            active_parameter: Some(active),
        }))
    }

    // ── Code Lens ────────────────────────────────────────────────────────────

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let Some((source, index)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };

        let mut lenses: Vec<CodeLens> = Vec::new();

        for sym in index.all_symbols() {
            match sym.kind {
                index::semantic::SymbolKind::Function | index::semantic::SymbolKind::Method => {
                    let refs = index.find_references(&source, &sym.name);
                    let ref_count = refs.len();

                    lenses.push(CodeLens {
                        range: sym.range,
                        command: Some(Command {
                            title: format!("{} references", ref_count),
                            command: "ori.showReferences".into(),
                            arguments: None,
                        }),
                        data: None,
                    });
                }
                _ => {}
            }
        }

        if lenses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lenses))
        }
    }

    // ── Formatting ───────────────────────────────────────────────────────────

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some((source, _)) = self.get_source_and_index(&uri).await else {
            return Ok(None);
        };
        let formatted = ori_driver::pipeline::format_source_text(&source);
        if formatted == source {
            return Ok(None);
        }
        let range = range_for_whole_document(&source);
        Ok(Some(vec![TextEdit {
            range,
            new_text: formatted,
        }]))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        // For range formatting, format the whole file and return the changed range
        self.formatting(DocumentFormattingParams {
            text_document: params.text_document,
            options: params.options,
            work_done_progress_params: params.work_done_progress_params,
        })
        .await
    }

    // ── Execute Command (Test Runner) ────────────────────────────────────────

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "ori.runTests" => {
                let args: Vec<String> = params
                    .arguments
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if args.is_empty() {
                    self.client
                        .log_message(MessageType::WARNING, "ori.runTests requires a file path")
                        .await;
                    return Ok(None);
                }
                let file_path = &args[0];
                let test_filter = args.get(1).map(|s| s.as_str());

                // Run tests via ori driver
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ori_driver::pipeline::run_test_with_options(
                        &std::path::PathBuf::from(file_path),
                        ori_driver::pipeline::TestOptions {
                            filter: test_filter.map(str::to_string),
                        },
                    )
                }));

                match result {
                    Ok(Ok(output)) => {
                        let passed = output.results.iter().filter(|r| r.passed).count();
                        let failed = output.results.iter().filter(|r| !r.passed).count();
                        let filter_note = output
                            .filter
                            .as_deref()
                            .map(|filter| format!(", filter `{filter}`"))
                            .unwrap_or_default();
                        let msg = format!(
                            "Tests completed: {passed} passed, {failed} failed, {}/{} selected{filter_note}",
                            output.selected, output.discovered
                        );
                        self.client.log_message(MessageType::INFO, msg).await;
                    }
                    Ok(Err(err)) => {
                        self.client
                            .log_message(MessageType::ERROR, format!("Test run failed: {err}"))
                            .await;
                    }
                    Err(_) => {
                        self.client
                            .log_message(MessageType::ERROR, "Test run panicked")
                            .await;
                    }
                }
                Ok(None)
            }
            "ori.doctor" => {
                let report = ori_driver::pipeline::run_doctor();
                for check in &report.checks {
                    let (level, label) = match check.status {
                        ori_driver::pipeline::DoctorStatus::Ok => (MessageType::INFO, "OK"),
                        ori_driver::pipeline::DoctorStatus::Warn => (MessageType::WARNING, "WARN"),
                        ori_driver::pipeline::DoctorStatus::Fail => (MessageType::ERROR, "FAIL"),
                    };
                    self.client
                        .log_message(
                            level,
                            format!("[{label}] {} — {}", check.name, check.detail),
                        )
                        .await;
                }
                Ok(None)
            }
            _ => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("Unknown command: {}", params.command),
                    )
                    .await;
                Ok(None)
            }
        }
    }
}

impl Backend {
    async fn resolve_import_target(&self, index: &SemanticIndex, _name: &str) -> Option<Url> {
        let imports = index.imports();
        for import in imports {
            if let Some(file_path) = &import.file_path {
                if file_path.exists() {
                    if let Ok(url) = Url::from_file_path(file_path) {
                        return Some(url);
                    }
                }
            }
        }
        None
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

fn resolve_stdlib_qualified_path(source: &str, qualified: &str) -> Option<String> {
    if stdlib_catalog().lookup(qualified).is_some() {
        return Some(qualified.to_string());
    }
    if let Some((receiver, name)) = qualified.rsplit_once('.') {
        let import_map = import_alias_map(source);
        if let Some(module) = import_map.get(receiver) {
            let full = format!("{module}.{name}");
            if stdlib_catalog().lookup(&full).is_some() {
                return Some(full);
            }
        }
    }
    None
}

fn resolve_stdlib_qualified_hover(source: &str, qualified: &str) -> Option<String> {
    let path = resolve_stdlib_qualified_path(source, qualified)?;
    stdlib_catalog().hover_markdown(&path)
}

fn resolve_stdlib_definition(source: &str, qualified: &str) -> Option<(std::path::PathBuf, Range)> {
    let path = resolve_stdlib_qualified_path(source, qualified)?;
    let entry = stdlib_catalog().lookup(&path)?;
    Some((entry.source_path.clone()?, entry.name_range?))
}

fn import_prefix_at_position(source: &str, position: Position) -> String {
    let offset = utils::position::byte_offset_for_position(source, position);
    let before = &source[..offset.min(source.len())];
    let Some(import_pos) = before.rfind("import ") else {
        return String::new();
    };
    before[import_pos + 7..].trim().to_string()
}

fn symbol_kind_to_cik(kind: &index::semantic::SymbolKind) -> CompletionItemKind {
    match kind {
        index::semantic::SymbolKind::Function => CompletionItemKind::FUNCTION,
        index::semantic::SymbolKind::Method => CompletionItemKind::METHOD,
        index::semantic::SymbolKind::Struct => CompletionItemKind::STRUCT,
        index::semantic::SymbolKind::Enum => CompletionItemKind::ENUM,
        index::semantic::SymbolKind::Trait => CompletionItemKind::INTERFACE,
        index::semantic::SymbolKind::Variable => CompletionItemKind::VARIABLE,
        index::semantic::SymbolKind::Parameter => CompletionItemKind::VARIABLE,
        index::semantic::SymbolKind::Field => CompletionItemKind::FIELD,
        index::semantic::SymbolKind::Import => CompletionItemKind::MODULE,
    }
}

fn semantic_kind_to_lsp(kind: &index::semantic::SymbolKind) -> SymbolKind {
    match kind {
        index::semantic::SymbolKind::Function => SymbolKind::FUNCTION,
        index::semantic::SymbolKind::Method => SymbolKind::METHOD,
        index::semantic::SymbolKind::Struct => SymbolKind::STRUCT,
        index::semantic::SymbolKind::Enum => SymbolKind::ENUM,
        index::semantic::SymbolKind::Trait => SymbolKind::INTERFACE,
        index::semantic::SymbolKind::Variable => SymbolKind::VARIABLE,
        index::semantic::SymbolKind::Field => SymbolKind::FIELD,
        _ => SymbolKind::OBJECT,
    }
}

/// Semantic token classification.
fn classify_semantic_token(
    sym: &SemanticSymbol,
) -> (SemanticTokenType, Vec<SemanticTokenModifier>) {
    let token_type = match sym.kind {
        index::semantic::SymbolKind::Function => SemanticTokenType::FUNCTION,
        index::semantic::SymbolKind::Method => SemanticTokenType::METHOD,
        index::semantic::SymbolKind::Struct => SemanticTokenType::STRUCT,
        index::semantic::SymbolKind::Enum => SemanticTokenType::ENUM,
        index::semantic::SymbolKind::Trait => SemanticTokenType::INTERFACE,
        index::semantic::SymbolKind::Parameter => SemanticTokenType::PARAMETER,
        index::semantic::SymbolKind::Field => SemanticTokenType::PROPERTY,
        index::semantic::SymbolKind::Variable => SemanticTokenType::VARIABLE,
        index::semantic::SymbolKind::Import => SemanticTokenType::NAMESPACE,
    };
    let modifier = match sym.kind {
        index::semantic::SymbolKind::Function if sym.name.starts_with("_") => {
            vec![SemanticTokenModifier::DECLARATION]
        }
        _ => vec![],
    };
    (token_type, modifier)
}

/// The semantic token legend we advertise.
static SEMANTIC_LEGEND: std::sync::LazyLock<SemanticTokensLegend> =
    std::sync::LazyLock::new(|| SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::FUNCTION,
            SemanticTokenType::METHOD,
            SemanticTokenType::STRUCT,
            SemanticTokenType::ENUM,
            SemanticTokenType::INTERFACE,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::KEYWORD,
        ],
        token_modifiers: vec![SemanticTokenModifier::DECLARATION],
    });

/// Encode raw tokens into delta-encoded LSP SemanticToken structs.
fn encode_semantic_tokens(
    tokens: &[(u32, u32, u32, SemanticTokenType, Vec<SemanticTokenModifier>)],
    legend: &SemanticTokensLegend,
) -> Vec<SemanticToken> {
    let mut data = Vec::new();
    let mut prev_line: u32 = 0;
    let mut prev_char: u32 = 0;

    for (line, start_char, length, token_type, _modifiers) in tokens {
        let delta_line = *line - prev_line;
        let delta_char = if delta_line == 0 {
            *start_char - prev_char
        } else {
            *start_char
        };

        let type_idx = legend
            .token_types
            .iter()
            .position(|t| t == token_type)
            .unwrap_or(0) as u32;

        data.push(SemanticToken {
            delta_line,
            delta_start: delta_char,
            length: *length,
            token_type: type_idx,
            token_modifiers_bitset: 0,
        });

        prev_line = *line;
        prev_char = *start_char;
    }

    data
}

/// Ori keywords for semantic token scanning (S3 surface).
const ORI_KEYWORDS: &[&str] = &[
    "module",
    // `func` remains for callable types `func(T) -> R`.
    "func",
    "return",
    "end",
    "const",
    "var",
    "if",
    "else",
    "while",
    "for",
    "in",
    "repeat",
    "loop",
    "break",
    "continue",
    "match",
    "case",
    "struct",
    "trait",
    "apply",
    "use",
    "implement",
    "enum",
    "where",
    "is",
    "alias",
    "do",
    "and",
    "or",
    "not",
    "true",
    "false",
    "none",
    "ok",
    "err",
    "some",
    "mut",
    "self",
    "extern",
    "any",
    "optional",
    "result",
    "list",
    "map",
    "set",
    "range",
    "void",
    "using",
    "try",
    "check",
    "with",
    "then",
    "tuple",
    "lazy",
    "async",
    "await",
    "import",
    "imports",
    // Kept for removed-form highlighting during migration (`as` / `only`).
    "as",
    "only",
    "public",
];

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "float"
            | "bool"
            | "string"
            | "bytes"
            | "void"
            | "never"
            | "optional"
            | "result"
            | "list"
            | "map"
            | "set"
    )
}

/// Find the enclosing function call at a cursor position.
fn find_enclosing_call(source: &str, position: Position) -> Option<(String, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }
    let line = lines[line_idx];

    let char_pos = position.character as usize;
    let prefix = if char_pos <= line.len() {
        &line[..char_pos]
    } else {
        line
    };

    let open_paren = prefix.rfind('(')?;
    let before_paren = &prefix[..open_paren].trim_end();

    let func_name = before_paren
        .rsplit(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
        .next()?;

    if func_name.is_empty() {
        return None;
    }

    let abs_pos = lines[..line_idx].iter().map(|l| l.len() + 1).sum::<usize>() + open_paren;

    Some((func_name.to_string(), abs_pos))
}

/// Count commas between open_paren and cursor position.
fn count_commas_before(source: &str, open_pos: usize, cursor: Position) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    let cursor_abs = lines[..cursor.line as usize]
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        + cursor.character as usize;

    if cursor_abs <= open_pos {
        return 0;
    }

    let between = &source[open_pos..cursor_abs.min(source.len())];
    between.chars().filter(|&c| c == ',').count()
}

/// Parse parameter names from a function signature string.
fn parse_params_from_signature(sig: &str) -> Vec<String> {
    if !sig.starts_with('(') {
        return vec![];
    }
    let inner = sig
        .trim_start_matches('(')
        .split("->")
        .next()
        .unwrap_or("")
        .trim_end_matches(')');

    inner
        .split(',')
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                None
            } else {
                Some(p.split(':').next().unwrap_or(p).trim().to_string())
            }
        })
        .collect()
}

/// Build a Range covering the entire document.
fn range_for_whole_document(source: &str) -> Range {
    let lines = source.lines().count().saturating_sub(1);
    let last_len = source.lines().last().map(|l| l.len()).unwrap_or(0);
    Range {
        start: Position::new(0, 0),
        end: Position::new(lines as u32, last_len as u32),
    }
}

fn position_in_range(position: Position, range: &Range) -> bool {
    !position_is_before(position, range.start) && position_is_before(position, range.end)
}

fn position_is_before(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character < right.character)
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(false),
                })),
            },
        )),
        hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_string()]),
            ..CompletionOptions::default()
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(tower_lsp::lsp_types::CodeActionProviderCapability::Simple(
            true,
        )),
        inlay_hint_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(
            SemanticTokensOptions {
                legend: SEMANTIC_LEGEND.clone(),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                range: Some(false),
                ..Default::default()
            }
            .into(),
        ),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Left(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: None,
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
        }),
        code_lens_provider: Some(CodeLensOptions {
            resolve_provider: Some(false),
        }),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        execute_command_provider: Some(tower_lsp::lsp_types::ExecuteCommandOptions {
            commands: vec!["ori.runTests".to_string(), "ori.doctor".to_string()],
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
        }),
        ..ServerCapabilities::default()
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
