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
use ori_types::resolve::ResolvedModule;
use ori_types::{Def, Ty};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Range};

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

impl ProjectSemanticIndex {
    pub fn new(resolved: ResolvedModule, cache: SourceCache, active_path: PathBuf) -> Self {
        Self {
            resolved: Arc::new(resolved),
            cache: Arc::new(cache),
            active_path,
        }
    }

    // ── Cross-file go-to-definition (Etapa 6.1) ────────────────────────────

    /// Resolve `symbol` to its defining location, which may live in an
    /// imported file. Returns `(path, range)` ready to become an LSP
    /// `Location`.
    pub fn cross_file_definition(&self, symbol: &str) -> Option<(PathBuf, Range)> {
        let def = self.find_def_by_name(symbol)?;
        self.def_to_location(def)
    }

    // ── Cross-file hover (Etapa 6.1) ───────────────────────────────────────

    /// Build a hover string for `symbol` from the resolved signatures.
    /// Returns `None` when the symbol is not a known top-level definition.
    pub fn cross_file_hover(&self, symbol: &str) -> Option<String> {
        if let Some(s) = self.find_struct_by_name(symbol) {
            let fields = s
                .fields
                .iter()
                .map(|(n, t)| format!("- `{n}`: {}", ty_to_str(t, &self.resolved)))
                .collect::<Vec<_>>()
                .join("\n");
            return Some(format!("```ori\nstruct {symbol}\n```\n\nFields:\n{fields}"));
        }
        if let Some(e) = self.find_enum_by_name(symbol) {
            let variants = e
                .variants
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "```ori\nenum {symbol}\n```\n\nVariants: {variants}"
            ));
        }
        if let Some(f) = self.find_func_by_name(symbol) {
            let params = f
                .params
                .iter()
                .map(|t| ty_to_str(t, &self.resolved))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "```ori\n{symbol}({params}) -> {}\n```\n\nTop-level function.",
                ty_to_str(&f.return_ty, &self.resolved),
            ));
        }
        if let Some(v) = self.find_value_by_name(symbol) {
            return Some(format!(
                "```ori\n{symbol}: {}\n```\n\nTop-level value.",
                ty_to_str(&v.ty, &self.resolved),
            ));
        }
        None
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

    /// Find every occurrence of `symbol` across all loaded sources (the
    /// active file and its transitive imports). Returns `(path, range)`
    /// pairs. Word boundaries are respected so that `User` does not match
    /// `UserName`.
    pub fn find_references_cross_file(&self, symbol: &str) -> Vec<(PathBuf, Range)> {
        let mut out = Vec::new();
        let needle = symbol.as_bytes();
        if needle.is_empty() {
            return out;
        }
        for file in self.cache.all_files() {
            let content = &file.content;
            let bytes = content.as_bytes();
            let mut i = 0usize;
            while i + needle.len() <= bytes.len() {
                if &bytes[i..i + needle.len()] == needle
                    && is_word_boundary(bytes, i, i + needle.len())
                {
                    let start = position::position_for_byte_offset(content, i);
                    let end = position::position_for_byte_offset(content, i + needle.len());
                    out.push((file.path.clone(), Range::new(start, end)));
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
        }
        out
    }

    // ── helpers: def lookup ────────────────────────────────────────────────

    fn find_def_by_name(&self, name: &str) -> Option<&Def> {
        self.resolved
            .def_map
            .all_defs()
            .iter()
            .find(|d| d.name == name)
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

    fn find_func_by_name(&self, name: &str) -> Option<&ori_types::resolve::FuncSig> {
        let def = self.find_def_by_name(name)?;
        self.resolved.func_sigs.iter().find(|f| f.def_id == def.id)
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
        let source_file = ori_parser::parse(&tokens, source, file_id, &mut sink);

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
        // `DefMap` does not currently tag each `Def` with its origin `FileId`,
        // so we scan every loaded source for an occurrence of `def.name` at
        // the byte range `def.span`. This is unambiguous in practice because
        // `def.span` is the exact byte range of the defining identifier
        // within its own file.
        if let Some(active_file) = self
            .cache
            .all_files()
            .iter()
            .find(|candidate| candidate.path == self.active_path)
        {
            if let Some(name_range) = locate_name_span(&active_file.content, &def.name, def.span) {
                return Some((active_file.path.clone(), name_range));
            }
        }
        for candidate in self.cache.all_files() {
            if candidate.path == self.active_path {
                continue;
            }
            if let Some(name_range) = locate_name_span(&candidate.content, &def.name, def.span) {
                return Some((candidate.path.clone(), name_range));
            }
        }
        None
    }

    // ── helpers: receiver type inference ───────────────────────────────────

    /// Walk the active source's AST and return the simple type name annotated
    /// on a binding named `receiver` (a local `var`/`const`/`using`, a
    /// function parameter, or a top-level `var`/`const`).
    fn infer_receiver_type_name(&self, receiver: &str, source: &str) -> Option<String> {
        let file_id = ori_diagnostics::FileId(0);
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, file_id, &mut sink);
        let source_file = ori_parser::parse(&tokens, source, file_id, &mut sink);

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
        Ty::Optional(inner) => format!("{}?", ty_to_str(inner, resolved)),
        Ty::Result(ok, err) => {
            format!(
                "result[{}, {}]",
                ty_to_str(ok, resolved),
                ty_to_str(err, resolved)
            )
        }
        Ty::List(inner) => format!("list[{}]", ty_to_str(inner, resolved)),
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
        Ty::Channel(inner) => format!("channel<{}>", ty_to_str(inner, resolved)),
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
                format!("{name}<{inner}>")
            }
        }
        Ty::Param { name, .. } => name.to_string(),
        Ty::Infer(_) => "_".into(),
    }
}

/// Check whether `[start, end)` in `bytes` is bounded by non-identifier
/// characters on both sides, so that searching for `User` does not match
/// `UserName` or `myUser`.
fn is_word_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let left_ok = start == 0 || !is_ident(bytes[start - 1]);
    let right_ok = end >= bytes.len() || !is_ident(bytes[end]);
    left_ok && right_ok
}

/// `DefMap` does not currently record which `FileId` a def was registered
/// against, so cross-file go-to-definition scans each loaded source for the
/// defining name occurrence. The resolver registers each top-level def with
/// the span of its whole declaration (e.g. the entire `struct Point … end`
/// block), not the span of the identifier alone, so we search for `name` as a
/// word-boundary occurrence WITHIN `[span.start, span.end)` and return its
/// position. This yields the identifier location regardless of whether the
/// registered span is a name span or a declaration span.
fn locate_name_span(content: &str, name: &str, span: Span) -> Option<Range> {
    let bytes = content.as_bytes();
    let s = (span.start as usize).min(bytes.len());
    let e = (span.end as usize).min(bytes.len());
    if s > e || e > bytes.len() || name.is_empty() {
        return None;
    }
    let needle = name.as_bytes();
    let mut i = s;
    while i + needle.len() <= e {
        if &bytes[i..i + needle.len()] == needle && is_word_boundary(bytes, i, i + needle.len()) {
            let start = position::position_for_byte_offset(content, i);
            let end = position::position_for_byte_offset(content, i + needle.len());
            return Some(Range::new(start, end));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_boundary_rejects_substring_identifiers() {
        let bytes = b"UserName User myUser";
        // "User" appears at byte 9..13 only as a standalone word.
        assert!(is_word_boundary(bytes, 9, 13));
        // "User" substring inside "UserName" (0..4) is NOT a word boundary.
        assert!(!is_word_boundary(bytes, 0, 4));
        // "User" substring inside "myUser" (14..18) is NOT a word boundary.
        assert!(!is_word_boundary(bytes, 14, 18));
    }
}
