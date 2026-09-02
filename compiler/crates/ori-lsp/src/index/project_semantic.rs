//! Project-wide semantic index backed by the driver's `run_check` output.
//!
//! Whereas `super::semantic::SemanticIndex` is built syntactically from a
//! single file's AST, `ProjectSemanticIndex` captures the `ResolvedModule` +
//! `SourceCache` produced by `ori_driver::pipeline::run_check_source`. This
//! gives the LSP access to:
//!
//! - the cross-file `DefMap` — top-level definitions of the entry file AND its
//!   transitive imports, each carrying a span that resolves to a file URI,
//! - resolved type signatures (`struct_sigs`, `enum_sigs`, `trait_sigs`,
//!   `impl_sigs`, `func_sigs`, `value_sigs`).
//!
//! These power cross-file go-to-definition (Etapa 6.1), cross-file
//! find-references and type-aware dot-completion (Etapa 6.2), and richer
//! hover that includes resolved signatures.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ori_ast::expr::Expr;
use ori_ast::item::Item;
use ori_ast::stmt::{Block, MatchCase, Stmt};
use ori_ast::ty::Type;
use ori_diagnostics::{SourceCache, Span};
use ori_lexer::TokenKind;
use ori_types::resolve::ResolvedModule;
use ori_types::{Def, DefId, Ty};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position, Range};

use super::semantic::SemanticIndex;
use crate::stdlib_catalog::stdlib_catalog;
use crate::utils::position;

/// A snapshot of the driver's resolved project state, keyed to a single
/// "active" file (the one the user is editing).
///
/// All queries are read-only and share the inner `Arc`s, so a stale snapshot
/// can be held by a handler while a newer one is being produced.
pub struct ProjectSemanticIndex {
    pub resolved: Arc<ResolvedModule>,
    pub cache: Arc<SourceCache>,
    pub active_path: PathBuf,
}

struct FileResolutionContext {
    file_id: ori_diagnostics::FileId,
    tokens: Vec<ori_lexer::Token>,
    ast: ori_ast::item::SourceFile,
    local_index: SemanticIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectSymbolIdentity {
    Definition(DefId),
    SelectiveImportAlias {
        file_id: ori_diagnostics::FileId,
        declaration_start: u32,
        target: DefId,
    },
}

impl ProjectSymbolIdentity {
    fn target(self) -> DefId {
        match self {
            Self::Definition(target) | Self::SelectiveImportAlias { target, .. } => target,
        }
    }
}

impl ProjectSemanticIndex {
    pub fn new(resolved: ResolvedModule, cache: SourceCache, active_path: PathBuf) -> Self {
        Self {
            resolved: Arc::new(resolved),
            cache: Arc::new(cache),
            active_path,
        }
    }

    // ── Cross-file go-to-definition (Etapa 6.1) ────────────────────────────

    /// Resolve the exact top-level identity under the cursor, then return its
    /// defining file. Duplicate simple names in other modules cannot win.
    pub fn cross_file_definition_at(
        &self,
        path: &std::path::Path,
        source: &str,
        cursor: Position,
    ) -> Option<(PathBuf, Range)> {
        let offset = position::byte_offset_for_position(source, cursor).ok()? as u32;
        let def_id = self.def_id_at_offset(path, source, offset)?;
        self.def_to_location(self.resolved.def_map.get(def_id))
    }

    // ── Cross-file hover (Etapa 6.1) ───────────────────────────────────────

    pub fn cross_file_hover_at(
        &self,
        path: &std::path::Path,
        source: &str,
        cursor: Position,
    ) -> Option<String> {
        let offset = position::byte_offset_for_position(source, cursor).ok()? as u32;
        let def_id = self.def_id_at_offset(path, source, offset)?;
        let def = self.resolved.def_map.get(def_id);
        let symbol = def.name.as_str();

        if let Some(signature) = self
            .resolved
            .struct_sigs
            .iter()
            .find(|signature| signature.def_id == def_id)
        {
            let fields = signature
                .fields
                .iter()
                .map(|(name, ty)| format!("- `{name}`: {}", ty_to_str(ty, &self.resolved)))
                .collect::<Vec<_>>()
                .join("\n");
            return Some(format!("```ori\nstruct {symbol}\n```\n\nFields:\n{fields}"));
        }
        if let Some(signature) = self
            .resolved
            .enum_sigs
            .iter()
            .find(|signature| signature.def_id == def_id)
        {
            let variants = signature
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "```ori\nenum {symbol}\n```\n\nVariants: {variants}"
            ));
        }
        if let Some(signature) = self
            .resolved
            .func_sigs
            .iter()
            .find(|signature| signature.def_id == def_id)
        {
            let params = signature
                .params
                .iter()
                .map(|ty| ty_to_str(ty, &self.resolved))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "```ori\n{symbol}({params}) -> {}\n```\n\nTop-level function.",
                ty_to_str(&signature.return_ty, &self.resolved),
            ));
        }
        self.resolved
            .value_sigs
            .iter()
            .find(|signature| signature.def_id == def_id)
            .map(|signature| {
                format!(
                    "```ori\n{symbol}: {}\n```\n\nTop-level value.",
                    ty_to_str(&signature.ty, &self.resolved),
                )
            })
    }

    // ── Type-aware dot completion (Etapa 6.2) ──────────────────────────────

    /// Produce completion items for a `receiver.` position by resolving the
    /// receiver's declared type and listing its fields / variants / methods.
    ///
    /// The receiver's type is inferred syntactically from the active source:
    /// we look for an explicit type annotation on a binding (`var x: T`,
    /// `const x: T`, `using x: T`) or a function parameter (`x: T`). Inferred
    /// bindings without annotations fall back to "no completions" rather than
    /// guessing.
    pub fn complete_after_dot(&self, receiver: &str, source: &str) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        let type_name = self
            .infer_receiver_type_name(receiver, source)
            .or_else(|| self.infer_receiver_from_value_sig(receiver))
            .or_else(|| self.infer_receiver_from_opaque_name(receiver, source));

        let Some(type_name) = type_name else {
            return items;
        };

        if let Some(s) = self.find_struct_by_name(&type_name) {
            for (name, ty) in &s.fields {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(format!("field: {}", ty_to_str(ty, &self.resolved))),
                    ..Default::default()
                });
            }
        }

        if let Some(e) = self.find_enum_by_name(&type_name) {
            for v in &e.variants {
                items.push(CompletionItem {
                    label: v.name.to_string(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(format!("variant of {type_name}")),
                    ..Default::default()
                });
            }
        }

        if let Some(type_def) = self.find_def_by_name(&type_name) {
            for imp in &self.resolved.impl_sigs {
                if imp.type_def_id == type_def.id {
                    for method in &imp.methods {
                        let ret = self
                            .resolved
                            .func_sigs
                            .iter()
                            .find(|f| f.def_id == method.func_def_id)
                            .map(|f| ty_to_str(&f.return_ty, &self.resolved))
                            .unwrap_or_else(|| "()".to_string());
                        items.push(CompletionItem {
                            label: method.name.to_string(),
                            kind: Some(CompletionItemKind::METHOD),
                            detail: Some(format!("method -> {ret}")),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        items.extend(self.complete_opaque_methods(&type_name));

        items
    }

    // ── Cross-file find references (Etapa 6.2) ─────────────────────────────

    /// Find references by resolved `DefId`, not by spelling. Lexer tokens
    /// exclude comments and string contents; per-file local indexes exclude
    /// shadowing bindings before import/top-level resolution.
    pub fn find_references_cross_file_at(
        &self,
        path: &std::path::Path,
        source: &str,
        cursor: Position,
    ) -> Vec<(PathBuf, Range)> {
        let Some(offset) = position::byte_offset_for_position(source, cursor)
            .ok()
            .map(|offset| offset as u32)
        else {
            return Vec::new();
        };
        let Some(target) = self.symbol_identity_at_offset(path, source, offset) else {
            return Vec::new();
        };

        let mut references = Vec::new();
        for file in self.cache.all_files() {
            let context = self.file_resolution_context(&file.path, &file.content, file.id);
            for token in context
                .tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Ident)
            {
                if context
                    .local_index
                    .is_local_identifier_at_offset(token.span.start)
                {
                    continue;
                }
                if self.symbol_identity_in_context(&file.content, token.span.start, &context)
                    == Some(target)
                {
                    references.push((
                        file.path.clone(),
                        Range::new(
                            position::position_for_byte_offset(
                                &file.content,
                                token.span.start as usize,
                            ),
                            position::position_for_byte_offset(
                                &file.content,
                                token.span.end as usize,
                            ),
                        ),
                    ));
                }
            }
        }
        references
    }

    // ── helpers: def lookup ────────────────────────────────────────────────

    fn find_def_by_name(&self, name: &str) -> Option<&Def> {
        self.resolved
            .def_map
            .all_defs()
            .iter()
            .find(|d| d.name == name)
    }

    fn def_id_at_offset(&self, path: &std::path::Path, source: &str, offset: u32) -> Option<DefId> {
        self.symbol_identity_at_offset(path, source, offset)
            .map(ProjectSymbolIdentity::target)
    }

    fn symbol_identity_at_offset(
        &self,
        path: &std::path::Path,
        source: &str,
        offset: u32,
    ) -> Option<ProjectSymbolIdentity> {
        let file_id = self
            .cache
            .all_files()
            .iter()
            .find(|file| same_path(&file.path, path))
            .map(|file| file.id)
            .unwrap_or(ori_diagnostics::FileId(0));
        let context = self.file_resolution_context(path, source, file_id);
        self.symbol_identity_in_context(source, offset, &context)
    }

    fn file_resolution_context(
        &self,
        path: &std::path::Path,
        source: &str,
        file_id: ori_diagnostics::FileId,
    ) -> FileResolutionContext {
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, file_id, &mut sink);
        let mut parse_sink = ori_diagnostics::DiagnosticSink::default();
        let ast = ori_parser::parse(&tokens, source, file_id, &mut parse_sink);
        FileResolutionContext {
            file_id,
            tokens,
            ast,
            local_index: SemanticIndex::build(source, Some(path)),
        }
    }

    fn symbol_identity_in_context(
        &self,
        source: &str,
        offset: u32,
        context: &FileResolutionContext,
    ) -> Option<ProjectSymbolIdentity> {
        if context.local_index.is_local_identifier_at_offset(offset) {
            return None;
        }
        let token_index = context.tokens.iter().position(|token| {
            token.kind == TokenKind::Ident && token.span.start <= offset && offset < token.span.end
        })?;
        let name = &source[context.tokens[token_index].span.start as usize
            ..context.tokens[token_index].span.end as usize];
        let namespace = context.ast.namespace.name.to_string();

        if let Some(qualified) = qualified_token_path(source, &context.tokens, token_index) {
            if let Some((head, tail)) = qualified.split_once('.') {
                for import in &context.ast.imports {
                    if import
                        .alias
                        .as_ref()
                        .is_some_and(|alias| alias.text == head)
                    {
                        let candidate = format!("{}.{}", import.path, tail);
                        if let Some(def_id) = self.resolved.def_map.lookup(&candidate) {
                            return Some(ProjectSymbolIdentity::Definition(def_id));
                        }
                    }
                }
            }
            if let Some(def_id) = self.resolved.def_map.lookup(&qualified) {
                return Some(ProjectSymbolIdentity::Definition(def_id));
            }
            if let Some(def_id) = self
                .resolved
                .def_map
                .lookup(&format!("{namespace}.{qualified}"))
            {
                return Some(ProjectSymbolIdentity::Definition(def_id));
            }
            return None;
        }

        if let Some(def_id) = self.resolved.def_map.lookup(&format!("{namespace}.{name}")) {
            return Some(ProjectSymbolIdentity::Definition(def_id));
        }
        for import in &context.ast.imports {
            for selected in &import.selected {
                let local_name = selected
                    .alias
                    .as_ref()
                    .map_or(selected.name.text.as_str(), |alias| alias.text.as_str());
                if local_name == name {
                    let candidate = format!("{}.{}", import.path, selected.name.text);
                    if let Some(def_id) = self.resolved.def_map.lookup(&candidate) {
                        if let Some(alias) = &selected.alias {
                            return Some(ProjectSymbolIdentity::SelectiveImportAlias {
                                file_id: context.file_id,
                                declaration_start: alias.span.start,
                                target: def_id,
                            });
                        }
                        return Some(ProjectSymbolIdentity::Definition(def_id));
                    }
                }
            }
        }

        let mut matching = self
            .resolved
            .def_map
            .all_defs()
            .iter()
            .filter(|definition| definition.name == name);
        let only = matching.next()?;
        if matching.next().is_none() {
            Some(ProjectSymbolIdentity::Definition(only.id))
        } else {
            None
        }
    }

    fn find_struct_by_name(&self, name: &str) -> Option<&ori_types::resolve::StructSig> {
        let def = self.find_def_by_name(name)?;
        self.resolved
            .struct_sigs
            .iter()
            .find(|s| s.def_id == def.id)
    }

    fn find_enum_by_name(&self, name: &str) -> Option<&ori_types::resolve::EnumSig> {
        let def = self.find_def_by_name(name)?;
        self.resolved.enum_sigs.iter().find(|e| e.def_id == def.id)
    }

    fn find_value_by_name(&self, name: &str) -> Option<&ori_types::resolve::ValueSig> {
        let def = self.find_def_by_name(name)?;
        self.resolved.value_sigs.iter().find(|v| v.def_id == def.id)
    }

    fn infer_receiver_from_value_sig(&self, receiver: &str) -> Option<String> {
        let value = self.find_value_by_name(receiver)?;
        ty_simple_name(&value.ty, &self.resolved)
    }

    /// Match opaque stdlib types like `deque.Deque` from a binding annotation.
    fn infer_receiver_from_opaque_name(&self, receiver: &str, source: &str) -> Option<String> {
        let file_id = ori_diagnostics::FileId(0);
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, file_id, &mut sink);
        let mut source_file = ori_parser::parse(&tokens, source, file_id, &mut sink);
        if let Err(error) = ori_driver::pipeline::filter_source_for_current_configuration(
            &self.active_path,
            &mut source_file,
            file_id,
            &mut sink,
        ) {
            eprintln!(
                "ori-lsp: cannot inspect `{}`: {error}",
                self.active_path.display()
            );
            return None;
        }

        for item in &source_file.items {
            if let Item::Var(v) = &item.item {
                if v.name.text == receiver {
                    if let Type::Named(qn) = &v.ty {
                        return Some(qn.to_string());
                    }
                }
            }
        }
        None
    }

    fn complete_opaque_methods(&self, type_name: &str) -> Vec<CompletionItem> {
        let prefix = if type_name.contains('.') {
            type_name.to_string()
        } else {
            format!("ori.{type_name}")
        };
        stdlib_catalog()
            .entries_for_module(&prefix)
            .into_iter()
            .map(|entry| CompletionItem {
                label: entry.name.clone(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(entry.signature.clone()),
                ..Default::default()
            })
            .collect()
    }

    // ── helpers: span → location ───────────────────────────────────────────

    fn def_to_location(&self, def: &Def) -> Option<(PathBuf, Range)> {
        let file = self.cache.get(def.file_id)?;
        let range = locate_name_span(&file.content, &def.name, def.span)?;
        Some((file.path.clone(), range))
    }

    // ── helpers: receiver type inference ───────────────────────────────────

    /// Walk the active source's AST and return the simple type name annotated
    /// on a binding named `receiver` (a local `var`/`const`/`using`, a
    /// function parameter, or a top-level `var`/`const`).
    fn infer_receiver_type_name(&self, receiver: &str, source: &str) -> Option<String> {
        let file_id = ori_diagnostics::FileId(0);
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, file_id, &mut sink);
        let mut source_file = ori_parser::parse(&tokens, source, file_id, &mut sink);
        if let Err(error) = ori_driver::pipeline::filter_source_for_current_configuration(
            &self.active_path,
            &mut source_file,
            file_id,
            &mut sink,
        ) {
            eprintln!(
                "ori-lsp: cannot inspect `{}`: {error}",
                self.active_path.display()
            );
            return None;
        }

        let mut bindings: HashMap<String, String> = HashMap::new();

        for item in &source_file.items {
            self.collect_bindings_from_item(&item.item, &mut bindings);
        }

        bindings.get(receiver).cloned()
    }

    fn collect_bindings_from_item(&self, item: &Item, out: &mut HashMap<String, String>) {
        match item {
            Item::Func(func) => {
                for param in &func.params {
                    if let Some(tn) = named_type_simple_name(&param.ty) {
                        out.insert(param.name.text.to_string(), tn);
                    }
                }
                self.collect_bindings_from_block(&func.body, out);
            }
            Item::Const(c) => {
                if let Some(tn) = named_type_simple_name(&c.ty) {
                    out.insert(c.name.text.to_string(), tn);
                }
            }
            Item::Var(v) => {
                if let Some(tn) = named_type_simple_name(&v.ty) {
                    out.insert(v.name.text.to_string(), tn);
                }
            }
            Item::Struct(s) => {
                for field in &s.fields {
                    if let Some(tn) = named_type_simple_name(&field.ty) {
                        out.insert(field.name.text.to_string(), tn);
                    }
                }
            }
            Item::Apply(apply) => {
                for member in apply
                    .free_members
                    .iter()
                    .chain(apply.uses.iter().flat_map(|u| u.members.iter()))
                {
                    let ori_ast::item::ApplyMember::Method(method) = member else {
                        continue;
                    };
                    for param in &method.params {
                        if let Some(tn) = named_type_simple_name(&param.ty) {
                            out.insert(param.name.text.to_string(), tn);
                        }
                    }
                    self.collect_bindings_from_block(&method.body, out);
                }
            }
            _ => {}
        }
    }

    fn collect_bindings_from_block(&self, block: &Block, out: &mut HashMap<String, String>) {
        for stmt in &block.stmts {
            self.collect_bindings_from_stmt(stmt, out);
        }
    }

    fn collect_bindings_from_stmt(&self, stmt: &Stmt, out: &mut HashMap<String, String>) {
        match stmt {
            Stmt::Const(c) => {
                if let Some(ast_ty) = &c.ty {
                    if let Some(tn) = named_type_simple_name(ast_ty) {
                        out.insert(c.name.text.to_string(), tn);
                    }
                } else if let Some(tn) = infer_type_from_expr(&c.value) {
                    out.insert(c.name.text.to_string(), tn);
                }
            }
            // `const Point { x, y } = …` — field types need the struct's
            // signature, which this index does not carry; the bindings are
            // still known to exist, just not their types.
            Stmt::Destructure(_) => {}
            Stmt::Var(v) => {
                if let Some(ast_ty) = &v.ty {
                    if let Some(tn) = named_type_simple_name(ast_ty) {
                        out.insert(v.name.text.to_string(), tn);
                    }
                } else if let Some(tn) = infer_type_from_expr(&v.value) {
                    out.insert(v.name.text.to_string(), tn);
                }
            }
            Stmt::Using(u) => {
                if let Some(tn) = named_type_simple_name(&u.ty) {
                    out.insert(u.name.text.to_string(), tn);
                }
            }
            Stmt::If(i) => {
                self.collect_bindings_from_block(&i.then_block, out);
                for (_, blk) in &i.else_ifs {
                    self.collect_bindings_from_block(blk, out);
                }
                if let Some(eb) = &i.else_block {
                    self.collect_bindings_from_block(eb, out);
                }
            }
            Stmt::IfSome(i) => {
                self.collect_bindings_from_block(&i.then_block, out);
                if let Some(eb) = &i.else_block {
                    self.collect_bindings_from_block(eb, out);
                }
            }
            Stmt::While(w) => self.collect_bindings_from_block(&w.body, out),
            Stmt::WhileSome(w) => self.collect_bindings_from_block(&w.body, out),
            Stmt::For(f) => self.collect_bindings_from_block(&f.body, out),
            Stmt::Repeat(r) => self.collect_bindings_from_block(&r.body, out),
            Stmt::Loop(l) => self.collect_bindings_from_block(&l.body, out),
            Stmt::Match(m) => {
                for case in &m.cases {
                    match case {
                        MatchCase::Pattern { body, .. } | MatchCase::Else { body, .. } => {
                            for s in body {
                                self.collect_bindings_from_stmt(s, out);
                            }
                        }
                    }
                }
            }
            Stmt::Expr(_)
            | Stmt::Assign(_)
            | Stmt::CompoundAssign(_)
            | Stmt::Return(_)
            | Stmt::Suspend(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Check(_) => {}
        }
    }
}

// ── free helpers ────────────────────────────────────────────────────────────

/// Extract the simple name of a user-defined `Type::Named`. Returns `None`
/// for primitives and generic wrappers, which cannot be completed via dot.
fn named_type_simple_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(qn) => Some(qn.to_string()),
        _ => None,
    }
}

/// Best-effort type name from a binding initializer (struct lit, enum, stdlib ctor).
fn infer_type_from_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StructLit { ty, .. } => Some(ty.to_string()),
        Expr::EnumVariantUnit { ty: Some(qn), .. }
        | Expr::EnumVariantNamed { ty: Some(qn), .. } => Some(qn.to_string()),
        Expr::Call { callee, .. } => infer_type_from_call_callee(callee),
        _ => None,
    }
}

fn infer_type_from_call_callee(callee: &Expr) -> Option<String> {
    match callee {
        Expr::QualifiedIdent(qn) => {
            let path = qn.to_string();
            if path.ends_with(".new") {
                let module = path.strip_suffix(".new")?;
                Some(format!("{module}.Deque"))
            } else {
                None
            }
        }
        Expr::Ident(name) if name.text == "new" => None,
        _ => None,
    }
}

fn ty_simple_name(ty: &Ty, resolved: &ResolvedModule) -> Option<String> {
    match ty {
        Ty::Named(def_id, _) => Some(resolved.def_map.get(*def_id).name.to_string()),
        Ty::Opaque { kind, .. } => Some(kind.display_name().into()),
        _ => None,
    }
}

/// Render a `Ty` to a compact string for hover/completion details.
fn ty_to_str(ty: &Ty, resolved: &ResolvedModule) -> String {
    match ty {
        Ty::ConstInt(name, value) => format!("{name}: {value}"),
        Ty::Bool => "bool".into(),
        Ty::Int => "int".into(),
        Ty::Int8 => "int8".into(),
        Ty::Int16 => "int16".into(),
        Ty::Int32 => "int32".into(),
        Ty::Int64 => "int64".into(),
        Ty::U8 => "u8".into(),
        Ty::U16 => "u16".into(),
        Ty::U32 => "u32".into(),
        Ty::U64 => "u64".into(),
        Ty::Float => "float".into(),
        Ty::Float32 => "float32".into(),
        Ty::Float64 => "float64".into(),
        Ty::String => "string".into(),
        Ty::Bytes => "bytes".into(),
        Ty::Void => "void".into(),
        Ty::Never => "never".into(),
        Ty::Error => "?".into(),
        Ty::Optional(inner) => format!("optional[{}]", ty_to_str(inner, resolved)),
        Ty::Result(ok, err) => {
            format!(
                "result[{}, {}]",
                ty_to_str(ok, resolved),
                ty_to_str(err, resolved)
            )
        }
        Ty::List(inner) => format!("list[{}]", ty_to_str(inner, resolved)),
        Ty::Buffer(inner) => format!("buffer[{}]", ty_to_str(inner, resolved)),
        Ty::Slice(inner) => format!("slice[{}]", ty_to_str(inner, resolved)),
        Ty::Array(inner, len) => format!(
            "array[{}, size: {}]",
            ty_to_str(inner, resolved),
            ty_to_str(len, resolved)
        ),
        Ty::Map(k, v) => format!(
            "map[{}, {}]",
            ty_to_str(k, resolved),
            ty_to_str(v, resolved)
        ),
        Ty::Set(inner) => format!("set[{}]", ty_to_str(inner, resolved)),
        Ty::Range(inner) => format!("range[{}]", ty_to_str(inner, resolved)),
        Ty::Lazy(inner) => format!("lazy[{}]", ty_to_str(inner, resolved)),
        Ty::Handle(inner) => format!("handle[{}]", ty_to_str(inner, resolved)),
        Ty::Future(inner) => format!("future[{}]", ty_to_str(inner, resolved)),
        Ty::TaskJob(inner) => format!("task.Job[{}]", ty_to_str(inner, resolved)),
        Ty::Channel(inner) => format!("channel.Channel[{}]", ty_to_str(inner, resolved)),
        Ty::AtomicInt => "atomic_int".into(),
        Ty::TaskJoinError => "task.JoinError".into(),
        Ty::ChannelSendError => "channel.SendError".into(),
        Ty::ChannelReceiveError => "channel.ReceiveError".into(),
        Ty::Opaque { kind, .. } => kind.display_name().into(),
        Ty::Any(def_id) => {
            let name = resolved.def_map.get(*def_id).name.to_string();
            format!("any[{name}]")
        }
        Ty::Tuple(parts) => {
            let inner = parts
                .iter()
                .map(|t| ty_to_str(t, resolved))
                .collect::<Vec<_>>()
                .join(", ");
            format!("tuple[{inner}]")
        }
        Ty::Func { params, ret } => {
            let ps = params
                .iter()
                .map(|t| ty_to_str(t, resolved))
                .collect::<Vec<_>>()
                .join(", ");
            format!("func({ps}) -> {}", ty_to_str(ret, resolved))
        }
        Ty::Named(def_id, args) => {
            let name = resolved.def_map.get(*def_id).name.to_string();
            if args.is_empty() {
                name
            } else {
                let inner = args
                    .iter()
                    .map(|t| ty_to_str(t, resolved))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}[{inner}]")
            }
        }
        Ty::Param { name, .. } => name.to_string(),
        Ty::Infer(_) => "_".into(),
    }
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn qualified_token_path(source: &str, tokens: &[ori_lexer::Token], index: usize) -> Option<String> {
    let mut start = index;
    while start >= 2
        && tokens[start - 1].kind == TokenKind::Dot
        && tokens[start - 2].kind == TokenKind::Ident
    {
        start -= 2;
    }

    let mut end = index;
    while end + 2 < tokens.len()
        && tokens[end + 1].kind == TokenKind::Dot
        && tokens[end + 2].kind == TokenKind::Ident
    {
        end += 2;
    }
    if start == end {
        return None;
    }

    Some(
        (start..=end)
            .step_by(2)
            .map(|token_index| {
                let span = tokens[token_index].span;
                &source[span.start as usize..span.end as usize]
            })
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// The resolver keeps the whole declaration span. Locate the identifier token
/// inside it without treating comments or strings as candidate definitions.
fn locate_name_span(content: &str, name: &str, span: Span) -> Option<Range> {
    let mut sink = ori_diagnostics::DiagnosticSink::default();
    let tokens = ori_lexer::lex(content, ori_diagnostics::FileId(0), &mut sink);
    let token = tokens.iter().find(|token| {
        token.kind == TokenKind::Ident
            && span.start <= token.span.start
            && token.span.end <= span.end
            && &content[token.span.start as usize..token.span.end as usize] == name
    })?;
    Some(Range::new(
        position::position_for_byte_offset(content, token.span.start as usize),
        position::position_for_byte_offset(content, token.span.end as usize),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_token_path_keeps_unicode_segments() {
        let source = "café.東京.valor";
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, ori_diagnostics::FileId(0), &mut sink);
        assert_eq!(
            qualified_token_path(source, &tokens, 2).as_deref(),
            Some("café.東京.valor")
        );
    }
}
