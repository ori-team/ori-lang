use ori_ast::expr::{ArgValue, ClosureBody, Expr, FStrPart, IndexExpr};
use ori_ast::pattern::Pattern;
use ori_ast::stmt::{Block, LValue, MatchCase, Stmt};
use ori_diagnostics::Span;
use ori_lexer::TokenKind;
use std::collections::HashMap;
use std::path::PathBuf;
use tower_lsp::lsp_types::{Position, Range};

use crate::utils::position;
use crate::utils::uri;

/// A symbol extracted from source code for hover and navigation.
#[derive(Clone, Debug)]
pub struct SemanticSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub hover: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Variable,
    Parameter,
    Field,
    Import,
}

impl SymbolKind {
    pub fn display(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Variable => "variable",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Field => "field",
            SymbolKind::Import => "import",
        }
    }
}

/// Information about a resolved import for cross-file navigation.
#[derive(Clone, Debug)]
pub struct ResolvedImport {
    pub alias: String,
    pub namespace: String,
    /// File path where the imported symbols are defined.
    pub file_path: Option<PathBuf>,
}

/// AST-based semantic index for a single file.
#[derive(Default, Clone)]
pub struct SemanticIndex {
    symbols: HashMap<String, Vec<SemanticSymbol>>,
    symbols_by_kind: HashMap<SymbolKind, Vec<SemanticSymbol>>,
    /// All import paths discovered in the file (for cross-file resolution).
    imports: Vec<ResolvedImport>,
    local_bindings: Vec<LocalBinding>,
    identifiers: Vec<IdentifierOccurrence>,
}

#[derive(Clone, Debug)]
struct LocalBinding {
    id: u32,
    name: String,
    declaration: Span,
    scope: Span,
    visible_from: u32,
}

#[derive(Clone, Debug)]
struct IdentifierOccurrence {
    name: String,
    span: Span,
    can_reference_binding: bool,
}

impl SemanticIndex {
    pub fn build(source: &str, path: Option<&std::path::Path>) -> Self {
        let mut index = Self::default();
        index.index_ast(source, path);
        index
    }

    pub fn hover(&self, symbol: &str) -> Option<String> {
        let entries = self.symbols.get(symbol)?;
        if entries.len() == 1 {
            return Some(entries[0].hover.clone());
        }
        let summaries: Vec<_> = entries
            .iter()
            .map(|entry| format!("- {}: {}", entry.kind.display(), entry.summary))
            .collect();
        Some(format!(
            "Multiple local symbols named `{symbol}`:\n\n{}",
            summaries.join("\n")
        ))
    }

    pub fn definition(&self, symbol: &str) -> Option<Range> {
        self.symbols
            .get(symbol)
            .and_then(|entries| entries.first())
            .map(|entry| entry.range)
    }

    /// Resolve the binding under the cursor and return its declaration. Local
    /// identities take precedence over same-named top-level declarations.
    pub fn definition_at(&self, source: &str, position: Position) -> Option<Range> {
        let occurrence = self.identifier_at(source, position)?;
        let binding = self.binding_for_occurrence(occurrence)?;
        Some(span_to_range(source, binding.declaration))
    }

    pub fn local_symbol_at(&self, source: &str, position: Position) -> Option<&SemanticSymbol> {
        let occurrence = self.identifier_at(source, position)?;
        let binding = self.binding_for_occurrence(occurrence)?;
        let declaration_range = span_to_range(source, binding.declaration);
        self.symbols.get(&binding.name)?.iter().find(|symbol| {
            symbol.range == declaration_range
                && matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Parameter)
        })
    }

    /// Find references that resolve to the same local binding as the cursor.
    /// When the cursor is not on a local, returns no ranges so the caller can
    /// use the project-wide `DefId` index instead.
    pub fn find_local_references_at(&self, source: &str, position: Position) -> Vec<Range> {
        let Some(target) = self
            .identifier_at(source, position)
            .and_then(|occurrence| self.binding_for_occurrence(occurrence))
        else {
            return Vec::new();
        };

        self.identifiers
            .iter()
            .filter(|occurrence| {
                occurrence.name == target.name
                    && self
                        .binding_for_occurrence(occurrence)
                        .is_some_and(|binding| binding.id == target.id)
            })
            .map(|occurrence| span_to_range(source, occurrence.span))
            .collect()
    }

    pub(crate) fn is_local_identifier_at_offset(&self, offset: u32) -> bool {
        self.identifiers
            .iter()
            .find(|occurrence| occurrence.span.start <= offset && offset < occurrence.span.end)
            .and_then(|occurrence| self.binding_for_occurrence(occurrence))
            .is_some()
    }

    /// Find identifier tokens with this spelling. Comments and string
    /// contents are excluded by construction. Identity-aware handlers should
    /// prefer `find_local_references_at` or the project `DefId` index.
    pub fn find_references(&self, source: &str, symbol: &str) -> Vec<Range> {
        self.identifiers
            .iter()
            .filter(|occurrence| occurrence.name == symbol)
            .map(|occurrence| span_to_range(source, occurrence.span))
            .collect()
    }

    /// Returns import information for cross-file navigation.
    pub fn imports(&self) -> &[ResolvedImport] {
        &self.imports
    }

    /// Find a symbol by its position in the source (for context-aware operations).
    pub fn symbol_at(&self, source: &str, pos: Position) -> Option<&SemanticSymbol> {
        let word = uri::word_at_position(source, pos)?;
        self.symbols
            .get(&word)?
            .iter()
            .find(|entry| position_in_range(pos, &entry.range))
    }

    /// Determine completion context based on cursor position.
    ///
    /// Import lines take priority over after-dot so `import ori.` completes
    /// modules instead of treating `ori` as a value receiver (S3 IDE UX).
    pub fn completion_context(&self, source: &str, pos: Position) -> CompletionContext {
        let Ok(offset) = position::byte_offset_for_position(source, pos) else {
            return CompletionContext::Default;
        };
        let before = &source[..offset.min(source.len())];

        // Restrict to the current line so dots/import from earlier lines
        // do not leak into the context.
        let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = &before[line_start..];
        let trimmed = line.trim_start();

        // `import ...` (including path dots) → module completion
        if trimmed.starts_with("import ") || trimmed == "import" {
            return CompletionContext::Import;
        }

        // After a dot: `receiver.` or `receiver.partial`
        if let Some(rel_dot) = line.rfind('.') {
            let after_dot = &line[rel_dot + 1..];
            if after_dot.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let before_dot = &line[..rel_dot];
                if let Some(receiver) = before_dot
                    .rsplit(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                {
                    if !receiver.is_empty() && !receiver.chars().all(|c| c.is_ascii_digit()) {
                        return CompletionContext::AfterDot {
                            receiver: receiver.to_string(),
                        };
                    }
                }
            }
        }

        CompletionContext::Default
    }

    /// All symbols in the index.
    pub fn all_symbols(&self) -> impl Iterator<Item = &SemanticSymbol> {
        self.symbols.values().flat_map(|v| v.iter())
    }

    fn add(&mut self, symbol: SemanticSymbol) {
        self.symbols
            .entry(symbol.name.clone())
            .or_default()
            .push(symbol.clone());
        self.symbols_by_kind
            .entry(symbol.kind.clone())
            .or_default()
            .push(symbol);
    }

    fn index_ast(&mut self, source: &str, path: Option<&std::path::Path>) {
        let file_id = ori_diagnostics::FileId(0);
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, file_id, &mut sink);
        let mut source_file = ori_parser::parse(&tokens, source, file_id, &mut sink);
        if let Some(path) = path {
            if let Err(error) = ori_driver::pipeline::filter_source_for_current_configuration(
                path,
                &mut source_file,
                file_id,
                &mut sink,
            ) {
                eprintln!("ori-lsp: cannot index `{}`: {error}", path.display());
                return;
            }
        }

        for item_with_attrs in &source_file.items {
            self.index_item(&item_with_attrs.item, source);
            self.collect_item_bindings(&item_with_attrs.item);
        }

        self.identifiers = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.kind == TokenKind::Ident)
            .map(|(index, token)| IdentifierOccurrence {
                name: source[token.span.start as usize..token.span.end as usize].to_string(),
                span: token.span,
                can_reference_binding: (index == 0 || tokens[index - 1].kind != TokenKind::Dot)
                    && tokens
                        .get(index + 1)
                        .is_none_or(|next| next.kind != TokenKind::Colon),
            })
            .collect();

        for import in &source_file.imports {
            let namespace = import.path.to_string();
            let file_path = ori_driver::pipeline::stdlib_source_path(&namespace);
            if !import.selected.is_empty() {
                for item in &import.selected {
                    let alias = item
                        .alias
                        .as_ref()
                        .map(|n| n.text.to_string())
                        .unwrap_or_else(|| item.name.text.to_string());
                    let selected_namespace = format!("{}.{}", namespace, item.name.text);
                    let selection = if let Some(item_alias) = item.alias.as_ref() {
                        format!("{} = {}", item.name.text, item_alias.text)
                    } else {
                        item.name.text.to_string()
                    };
                    self.imports.push(ResolvedImport {
                        alias: alias.clone(),
                        namespace: selected_namespace.clone(),
                        file_path: file_path.clone(),
                    });
                    self.add(SemanticSymbol {
                        name: alias,
                        kind: SymbolKind::Import,
                        range: span_to_range(source, item.span),
                        hover: format!(
                            "```ori\nimport {namespace} ({selection})\n```\n\nSelective import."
                        ),
                        summary: format!("import {selected_namespace}"),
                    });
                }
            } else if let Some(alias_name) = import.alias.as_ref() {
                // S3: `import path = alias` — short key only when explicit.
                let alias = alias_name.text.to_string();
                self.imports.push(ResolvedImport {
                    alias: alias.clone(),
                    namespace: namespace.clone(),
                    file_path: file_path.clone(),
                });
                self.add(SemanticSymbol {
                    name: alias,
                    kind: SymbolKind::Import,
                    range: span_to_range(source, alias_name.span),
                    hover: format!(
                        "```ori\nimport {namespace} = {}\n```\n\nModule import.",
                        alias_name.text
                    ),
                    summary: format!("import {namespace}"),
                });
            } else {
                // Bare whole-module import: full path only (no last-segment alias).
                self.imports.push(ResolvedImport {
                    alias: namespace.clone(),
                    namespace: namespace.clone(),
                    file_path: file_path.clone(),
                });
                self.add(SemanticSymbol {
                    name: namespace.clone(),
                    kind: SymbolKind::Import,
                    range: span_to_range(source, import.path.span),
                    hover: format!(
                        "```ori\nimport {namespace}\n```\n\nModule import (full path only)."
                    ),
                    summary: format!("import {namespace}"),
                });
            }
        }
    }

    fn identifier_at(&self, source: &str, position: Position) -> Option<&IdentifierOccurrence> {
        let offset = position::byte_offset_for_position(source, position).ok()? as u32;
        self.identifiers
            .iter()
            .find(|occurrence| occurrence.span.start <= offset && offset < occurrence.span.end)
    }

    fn binding_for_occurrence(&self, occurrence: &IdentifierOccurrence) -> Option<&LocalBinding> {
        if let Some(binding) = self.local_bindings.iter().find(|binding| {
            binding.name == occurrence.name && binding.declaration == occurrence.span
        }) {
            return Some(binding);
        }
        if !occurrence.can_reference_binding {
            return None;
        }

        self.local_bindings
            .iter()
            .filter(|binding| {
                binding.name == occurrence.name
                    && binding.visible_from <= occurrence.span.start
                    && binding.scope.start <= occurrence.span.start
                    && occurrence.span.end <= binding.scope.end
            })
            .max_by_key(|binding| (binding.scope.start, binding.visible_from))
    }

    fn collect_item_bindings(&mut self, item: &ori_ast::item::Item) {
        match item {
            ori_ast::item::Item::Func(function) => {
                for parameter in &function.params {
                    self.add_local_binding(
                        &parameter.name,
                        function.body.span,
                        function.body.span.start,
                    );
                }
                self.collect_block_bindings(&function.body);
            }
            ori_ast::item::Item::Apply(apply) => {
                for member in apply
                    .free_members
                    .iter()
                    .chain(apply.uses.iter().flat_map(|section| section.members.iter()))
                {
                    let ori_ast::item::ApplyMember::Method(method) = member else {
                        continue;
                    };
                    for parameter in &method.params {
                        self.add_local_binding(
                            &parameter.name,
                            method.body.span,
                            method.body.span.start,
                        );
                    }
                    self.collect_block_bindings(&method.body);
                }
            }
            _ => {}
        }
    }

    fn collect_block_bindings(&mut self, block: &Block) {
        for statement in &block.stmts {
            match statement {
                Stmt::Const(local) => {
                    self.collect_expression_bindings(&local.value);
                    self.add_local_binding(&local.name, block.span, local.span.end);
                }
                Stmt::Var(local) => {
                    self.collect_expression_bindings(&local.value);
                    self.add_local_binding(&local.name, block.span, local.span.end);
                }
                Stmt::Destructure(local) => {
                    self.collect_expression_bindings(&local.value);
                    for (_, binding) in &local.fields {
                        self.add_local_binding(binding, block.span, local.span.end);
                    }
                }
                Stmt::Using(local) => {
                    self.collect_expression_bindings(&local.value);
                    self.add_local_binding(&local.name, block.span, local.span.end);
                }
                Stmt::If(statement) => {
                    self.collect_expression_bindings(&statement.condition);
                    self.collect_block_bindings(&statement.then_block);
                    for (_, branch) in &statement.else_ifs {
                        self.collect_block_bindings(branch);
                    }
                    if let Some(branch) = &statement.else_block {
                        self.collect_block_bindings(branch);
                    }
                }
                Stmt::IfSome(statement) => {
                    self.collect_expression_bindings(&statement.value);
                    self.add_local_binding(
                        &statement.binding,
                        statement.then_block.span,
                        statement.then_block.span.start,
                    );
                    self.collect_block_bindings(&statement.then_block);
                    if let Some(branch) = &statement.else_block {
                        self.collect_block_bindings(branch);
                    }
                }
                Stmt::While(statement) => {
                    self.collect_expression_bindings(&statement.condition);
                    self.collect_block_bindings(&statement.body);
                }
                Stmt::WhileSome(statement) => {
                    self.collect_expression_bindings(&statement.value);
                    self.add_local_binding(
                        &statement.binding,
                        statement.body.span,
                        statement.body.span.start,
                    );
                    self.collect_block_bindings(&statement.body);
                }
                Stmt::For(statement) => {
                    self.collect_expression_bindings(&statement.iterable);
                    self.add_local_binding(
                        &statement.binding,
                        statement.body.span,
                        statement.body.span.start,
                    );
                    if let Some(binding) = &statement.second_binding {
                        self.add_local_binding(
                            binding,
                            statement.body.span,
                            statement.body.span.start,
                        );
                    }
                    self.collect_block_bindings(&statement.body);
                }
                Stmt::Repeat(statement) => {
                    self.collect_expression_bindings(&statement.count);
                    self.collect_block_bindings(&statement.body);
                }
                Stmt::Loop(statement) => self.collect_block_bindings(&statement.body),
                Stmt::Match(statement) => {
                    self.collect_expression_bindings(&statement.scrutinee);
                    for case in &statement.cases {
                        match case {
                            MatchCase::Pattern {
                                pattern,
                                body,
                                span,
                                ..
                            } => {
                                self.collect_pattern_bindings(pattern, *span);
                                self.collect_statement_bindings(body, *span);
                            }
                            MatchCase::Else { body, span } => {
                                self.collect_statement_bindings(body, *span);
                            }
                        }
                    }
                }
                Stmt::Assign(statement) => {
                    self.collect_lvalue_bindings(&statement.lvalue);
                    self.collect_expression_bindings(&statement.value);
                }
                Stmt::CompoundAssign(statement) => {
                    self.collect_lvalue_bindings(&statement.lvalue);
                    self.collect_expression_bindings(&statement.value);
                }
                Stmt::Return(statement) => {
                    if let Some(value) = &statement.value {
                        self.collect_expression_bindings(value);
                    }
                }
                Stmt::Suspend(statement) => self.collect_expression_bindings(&statement.value),
                Stmt::Check(statement) => {
                    self.collect_expression_bindings(&statement.condition);
                }
                Stmt::Expr(expression) => self.collect_expression_bindings(expression),
                Stmt::Break(_) | Stmt::Continue(_) => {}
            }
        }
    }

    fn collect_statement_bindings(&mut self, statements: &[Stmt], scope: Span) {
        let synthetic_block = Block {
            stmts: statements.to_vec(),
            span: scope,
        };
        self.collect_block_bindings(&synthetic_block);
    }

    fn collect_pattern_bindings(&mut self, pattern: &Pattern, scope: Span) {
        match pattern {
            Pattern::Binding(name) => self.add_local_binding(name, scope, scope.start),
            Pattern::VariantNamed { fields, .. } => {
                for field in fields {
                    self.collect_pattern_bindings(&field.pattern, scope);
                }
            }
            Pattern::Some(inner, _) | Pattern::Ok(inner, _) | Pattern::Err(inner, _) => {
                self.collect_pattern_bindings(inner, scope);
            }
            Pattern::Tuple(parts, _) | Pattern::Or(parts, _) => {
                for part in parts {
                    self.collect_pattern_bindings(part, scope);
                }
            }
            Pattern::Literal(expression) => self.collect_expression_bindings(expression),
            Pattern::Wildcard(_) | Pattern::VariantUnit { .. } | Pattern::None(_) => {}
        }
    }

    fn collect_lvalue_bindings(&mut self, lvalue: &LValue) {
        match lvalue {
            LValue::Ident(_) => {}
            LValue::Field { base, .. } => self.collect_lvalue_bindings(base),
            LValue::Index { base, index, .. } => {
                self.collect_lvalue_bindings(base);
                self.collect_expression_bindings(index);
            }
        }
    }

    fn collect_expression_bindings(&mut self, expression: &Expr) {
        match expression {
            Expr::FStrLit { parts, .. } => {
                for part in parts {
                    if let FStrPart::Interpolated(expression) = part {
                        self.collect_expression_bindings(expression);
                    }
                }
            }
            Expr::Range { start, end, .. }
            | Expr::Binary {
                lhs: start,
                rhs: end,
                ..
            } => {
                self.collect_expression_bindings(start);
                self.collect_expression_bindings(end);
            }
            Expr::List { elements, .. }
            | Expr::Set { elements, .. }
            | Expr::Tuple { elements, .. } => {
                for element in elements {
                    self.collect_expression_bindings(element);
                }
            }
            Expr::Map { entries, .. } => {
                for (key, value) in entries {
                    self.collect_expression_bindings(key);
                    self.collect_expression_bindings(value);
                }
            }
            Expr::StructLit { fields, .. }
            | Expr::AnonStructLit { fields, .. }
            | Expr::EnumVariantNamed { fields, .. } => {
                for field in fields {
                    self.collect_expression_bindings(&field.value);
                }
            }
            Expr::Unary { operand, .. }
            | Expr::Try { expr: operand, .. }
            | Expr::Await { expr: operand, .. } => self.collect_expression_bindings(operand),
            Expr::Field { object, .. } | Expr::TupleIndex { object, .. } => {
                self.collect_expression_bindings(object);
            }
            Expr::Call { callee, args, .. } => {
                self.collect_expression_bindings(callee);
                for argument in args {
                    match &argument.value {
                        ArgValue::Expr(value) | ArgValue::Spread(value) => {
                            self.collect_expression_bindings(value);
                        }
                    }
                }
            }
            Expr::Index { object, index, .. } => {
                self.collect_expression_bindings(object);
                match index {
                    IndexExpr::Single(index) => self.collect_expression_bindings(index),
                    IndexExpr::Range { start, end } => {
                        if let Some(start) = start {
                            self.collect_expression_bindings(start);
                        }
                        if let Some(end) = end {
                            self.collect_expression_bindings(end);
                        }
                    }
                }
            }
            Expr::Pipe { value, func, .. } => {
                self.collect_expression_bindings(value);
                self.collect_expression_bindings(func);
            }
            Expr::IfExpr {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.collect_expression_bindings(condition);
                self.collect_expression_bindings(then_expr);
                self.collect_expression_bindings(else_expr);
            }
            Expr::MatchExpr {
                scrutinee, arms, ..
            } => {
                self.collect_expression_bindings(scrutinee);
                for arm in arms {
                    if let Some(pattern) = &arm.pattern {
                        self.collect_pattern_bindings(pattern, arm.span);
                    }
                    if let Some(guard) = &arm.guard {
                        self.collect_expression_bindings(guard);
                    }
                    self.collect_expression_bindings(&arm.body);
                }
            }
            Expr::Closure(closure) => {
                for parameter in &closure.params {
                    self.add_local_binding(&parameter.name, closure.span, closure.span.start);
                }
                match &closure.body {
                    ClosureBody::Expr(body) => self.collect_expression_bindings(body),
                    ClosureBody::Block(body) => self.collect_block_bindings(body),
                }
            }
            Expr::StructUpdate { base, updates, .. } => {
                self.collect_expression_bindings(base);
                for field in updates {
                    self.collect_expression_bindings(&field.value);
                }
            }
            Expr::IsCheck { value, .. } => self.collect_expression_bindings(value),
            Expr::BoolLit(_, _)
            | Expr::IntLit { .. }
            | Expr::FloatLit { .. }
            | Expr::StrLit { .. }
            | Expr::BytesLit { .. }
            | Expr::None(_)
            | Expr::Ident(_)
            | Expr::QualifiedIdent(_)
            | Expr::SelfExpr(_)
            | Expr::EnumVariantUnit { .. } => {}
        }
    }

    fn add_local_binding(&mut self, name: &ori_ast::Name, scope: Span, visible_from: u32) {
        self.local_bindings.push(LocalBinding {
            id: self.local_bindings.len() as u32,
            name: name.text.to_string(),
            declaration: name.span,
            scope,
            visible_from,
        });
    }

    fn index_item(&mut self, item: &ori_ast::item::Item, source: &str) {
        match item {
            ori_ast::item::Item::Func(func) => {
                let range = span_to_range(source, func.span);
                let signature = func_signature(func);
                let hover = format!("```ori\n{signature}\n```\n\nUser-defined function.");
                self.add(SemanticSymbol {
                    name: func.name.text.to_string(),
                    kind: SymbolKind::Function,
                    range,
                    hover,
                    summary: format!("function {}", func.name.text),
                });
                for param in &func.params {
                    let p_range = span_to_range(source, param.span);
                    self.add(SemanticSymbol {
                        name: param.name.text.to_string(),
                        kind: SymbolKind::Parameter,
                        range: p_range,
                        hover: format!(
                            "```ori\n{}: {}\n```\n\nFunction parameter.",
                            param.name.text,
                            type_to_string(&param.ty)
                        ),
                        // Inlay shows `: {summary}` — use the type name for params.
                        summary: type_to_string(&param.ty),
                    });
                }
                // Index local bindings so inlay can show inferred types (0.3.1).
                self.index_local_bindings(&func.body, source);
            }
            ori_ast::item::Item::Struct(s) => {
                let range = span_to_range(source, s.span);
                let field_list: Vec<_> = s
                    .fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name.text, type_to_string(&f.ty)))
                    .collect();
                let hover = format!(
                    "```ori\nstruct {}\n```\n\nFields:\n{}",
                    s.name.text,
                    field_list
                        .iter()
                        .map(|f| format!("- `{f}`"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                self.add(SemanticSymbol {
                    name: s.name.text.to_string(),
                    kind: SymbolKind::Struct,
                    range,
                    hover,
                    summary: format!("struct {}", s.name.text),
                });
                for field in &s.fields {
                    let f_range = span_to_range(source, field.span);
                    self.add(SemanticSymbol {
                        name: field.name.text.to_string(),
                        kind: SymbolKind::Field,
                        range: f_range,
                        hover: format!(
                            "```ori\n{}: {}\n```\n\nField of `struct {}`.",
                            field.name.text,
                            type_to_string(&field.ty),
                            s.name.text
                        ),
                        summary: format!("field {}.{}", s.name.text, field.name.text),
                    });
                }
            }
            ori_ast::item::Item::Enum(e) => {
                let range = span_to_range(source, e.span);
                let variant_list: Vec<_> =
                    e.variants.iter().map(|v| v.name.text.to_string()).collect();
                let hover = format!(
                    "```ori\nenum {}\n```\n\nVariants:\n{}",
                    e.name.text,
                    variant_list
                        .iter()
                        .map(|v| format!("- `{v}`"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                self.add(SemanticSymbol {
                    name: e.name.text.to_string(),
                    kind: SymbolKind::Enum,
                    range,
                    hover,
                    summary: format!("enum {}", e.name.text),
                });
            }
            ori_ast::item::Item::Trait(t) => {
                let range = span_to_range(source, t.span);
                self.add(SemanticSymbol {
                    name: t.name.text.to_string(),
                    kind: SymbolKind::Trait,
                    range,
                    hover: format!("```ori\ntrait {}\n```\n\nUser-defined trait.", t.name.text),
                    summary: format!("trait {}", t.name.text),
                });
            }
            ori_ast::item::Item::Const(c) => {
                let range = span_to_range(source, c.span);
                self.add(SemanticSymbol {
                    name: c.name.text.to_string(),
                    kind: SymbolKind::Variable,
                    range,
                    hover: format!(
                        "```ori\nconst {}: {}\n```\n\nLocal constant binding.",
                        c.name.text,
                        type_to_string(&c.ty)
                    ),
                    summary: format!("const {}", c.name.text),
                });
            }
            ori_ast::item::Item::Var(v) => {
                let range = span_to_range(source, v.span);
                self.add(SemanticSymbol {
                    name: v.name.text.to_string(),
                    kind: SymbolKind::Variable,
                    range,
                    hover: format!(
                        "```ori\nvar {}: {}\n```\n\nLocal mutable binding.",
                        v.name.text,
                        type_to_string(&v.ty)
                    ),
                    summary: format!("var {}", v.name.text),
                });
            }
            ori_ast::item::Item::Apply(apply) => {
                for member in apply
                    .free_members
                    .iter()
                    .chain(apply.uses.iter().flat_map(|u| u.members.iter()))
                {
                    let ori_ast::item::ApplyMember::Method(method) = member else {
                        continue;
                    };
                    let range = span_to_range(source, method.span);
                    let sig = func_signature(method);
                    let hover = format!("```ori\n{sig}\n```\n\nApply method.");
                    self.add(SemanticSymbol {
                        name: method.name.text.to_string(),
                        kind: SymbolKind::Method,
                        range,
                        hover,
                        summary: format!(
                            "method {}.{}",
                            apply.for_type.last().text,
                            method.name.text
                        ),
                    });
                }
            }
            _ => {}
        }
    }

    fn index_local_bindings(&mut self, block: &ori_ast::stmt::Block, source: &str) {
        self.index_local_stmts(&block.stmts, source);
    }

    fn index_local_stmts(&mut self, stmts: &[ori_ast::stmt::Stmt], source: &str) {
        for stmt in stmts {
            match stmt {
                ori_ast::stmt::Stmt::Const(c) => {
                    let ty_str =
                        c.ty.as_ref()
                            .map(type_to_string)
                            .or_else(|| syntactic_type_hint(&c.value))
                            .unwrap_or_else(|| "_".to_string());
                    let range = span_to_range(source, c.name.span);
                    self.add(SemanticSymbol {
                        name: c.name.text.to_string(),
                        kind: SymbolKind::Variable,
                        range,
                        hover: format!(
                            "```ori\nconst {}: {}\n```\n\nLocal constant binding.",
                            c.name.text, ty_str
                        ),
                        summary: ty_str,
                    });
                }
                ori_ast::stmt::Stmt::Var(v) => {
                    let ty_str =
                        v.ty.as_ref()
                            .map(type_to_string)
                            .or_else(|| syntactic_type_hint(&v.value))
                            .unwrap_or_else(|| "_".to_string());
                    let range = span_to_range(source, v.name.span);
                    self.add(SemanticSymbol {
                        name: v.name.text.to_string(),
                        kind: SymbolKind::Variable,
                        range,
                        hover: format!(
                            "```ori\nvar {}: {}\n```\n\nLocal mutable binding.",
                            v.name.text, ty_str
                        ),
                        summary: ty_str,
                    });
                }
                ori_ast::stmt::Stmt::If(i) => {
                    self.index_local_bindings(&i.then_block, source);
                    for (_, b) in &i.else_ifs {
                        self.index_local_bindings(b, source);
                    }
                    if let Some(eb) = &i.else_block {
                        self.index_local_bindings(eb, source);
                    }
                }
                ori_ast::stmt::Stmt::IfSome(i) => {
                    let range = span_to_range(source, i.binding.span);
                    self.add(SemanticSymbol {
                        name: i.binding.text.to_string(),
                        kind: SymbolKind::Variable,
                        range,
                        hover: format!(
                            "```ori\nif some({})\n```\n\nOptional binding.",
                            i.binding.text
                        ),
                        summary: "_".to_string(),
                    });
                    self.index_local_bindings(&i.then_block, source);
                    if let Some(eb) = &i.else_block {
                        self.index_local_bindings(eb, source);
                    }
                }
                ori_ast::stmt::Stmt::While(w) => self.index_local_bindings(&w.body, source),
                ori_ast::stmt::Stmt::WhileSome(w) => {
                    let range = span_to_range(source, w.binding.span);
                    self.add(SemanticSymbol {
                        name: w.binding.text.to_string(),
                        kind: SymbolKind::Variable,
                        range,
                        hover: format!(
                            "```ori\nwhile some({})\n```\n\nOptional binding.",
                            w.binding.text
                        ),
                        summary: "_".to_string(),
                    });
                    self.index_local_bindings(&w.body, source);
                }
                ori_ast::stmt::Stmt::For(f) => {
                    let range = span_to_range(source, f.binding.span);
                    self.add(SemanticSymbol {
                        name: f.binding.text.to_string(),
                        kind: SymbolKind::Variable,
                        range,
                        hover: format!("```ori\nfor {}\n```\n\nLoop binding.", f.binding.text),
                        summary: "_".to_string(),
                    });
                    if let Some(second) = &f.second_binding {
                        let range = span_to_range(source, second.span);
                        self.add(SemanticSymbol {
                            name: second.text.to_string(),
                            kind: SymbolKind::Variable,
                            range,
                            hover: format!("```ori\nfor _, {}\n```\n\nLoop binding.", second.text),
                            summary: "_".to_string(),
                        });
                    }
                    self.index_local_bindings(&f.body, source);
                }
                ori_ast::stmt::Stmt::Loop(l) => self.index_local_bindings(&l.body, source),
                ori_ast::stmt::Stmt::Repeat(r) => self.index_local_bindings(&r.body, source),
                ori_ast::stmt::Stmt::Match(m) => {
                    for case in &m.cases {
                        match case {
                            ori_ast::stmt::MatchCase::Pattern { body, .. }
                            | ori_ast::stmt::MatchCase::Else { body, .. } => {
                                self.index_local_stmts(body, source);
                            }
                        }
                    }
                }
                ori_ast::stmt::Stmt::Using(u) => {
                    // `using name: Type = expr` is a single statement (no nested block).
                    let range = span_to_range(source, u.name.span);
                    let ty_str = type_to_string(&u.ty);
                    self.add(SemanticSymbol {
                        name: u.name.text.to_string(),
                        kind: SymbolKind::Variable,
                        range,
                        hover: format!(
                            "```ori\nusing {}: {}\n```\n\nResource binding.",
                            u.name.text, ty_str
                        ),
                        summary: ty_str,
                    });
                }
                _ => {}
            }
        }
    }
}

/// Describes what kind of completion the user expects at the cursor position.
#[derive(Debug, Clone)]
pub enum CompletionContext {
    /// After a dot: `receiver.` — suggest fields or methods.
    AfterDot { receiver: String },
    /// Inside an import statement: `import ` — suggest modules.
    Import,
    /// Default context — suggest everything.
    Default,
}

fn func_signature(func: &ori_ast::item::FuncDecl) -> String {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.text, type_to_string(&p.ty)))
        .collect();
    let ret = func
        .return_ty
        .as_ref()
        .map(|t| format!(" -> {}", type_to_string(t)))
        .unwrap_or_default();
    let mut prefix = String::new();
    if func.is_async {
        prefix.push_str("async ");
    }
    if func.is_mut {
        prefix.push_str("mut ");
    }
    format!("{}{}({}){}", prefix, func.name.text, params.join(", "), ret)
}

/// Lightweight display type for omitted local annotations (inlay only).
fn syntactic_type_hint(expr: &ori_ast::expr::Expr) -> Option<String> {
    use ori_ast::expr::Expr;
    match expr {
        Expr::BoolLit(..) => Some("bool".into()),
        Expr::IntLit { .. } => Some("int".into()),
        Expr::FloatLit { .. } => Some("float".into()),
        Expr::StrLit { .. } | Expr::FStrLit { .. } => Some("string".into()),
        Expr::BytesLit { .. } => Some("bytes".into()),
        Expr::StructLit { ty, .. } => Some(ty.to_string()),
        Expr::List { elements, .. } if !elements.is_empty() => {
            syntactic_type_hint(&elements[0]).map(|e| format!("list[{e}]"))
        }
        _ => None,
    }
}

fn type_to_string(ty: &ori_ast::ty::Type) -> String {
    match ty {
        ori_ast::ty::Type::Bool(_) => "bool".to_string(),
        ori_ast::ty::Type::Int(_) => "int".to_string(),
        ori_ast::ty::Type::Int8(_) => "int8".to_string(),
        ori_ast::ty::Type::Int16(_) => "int16".to_string(),
        ori_ast::ty::Type::Int32(_) => "int32".to_string(),
        ori_ast::ty::Type::Int64(_) => "int64".to_string(),
        ori_ast::ty::Type::U8(_) => "u8".to_string(),
        ori_ast::ty::Type::U16(_) => "u16".to_string(),
        ori_ast::ty::Type::U32(_) => "u32".to_string(),
        ori_ast::ty::Type::U64(_) => "u64".to_string(),
        ori_ast::ty::Type::Float(_) => "float".to_string(),
        ori_ast::ty::Type::Float32(_) => "float32".to_string(),
        ori_ast::ty::Type::Float64(_) => "float64".to_string(),
        ori_ast::ty::Type::String(_) => "string".to_string(),
        ori_ast::ty::Type::Bytes(_) => "bytes".to_string(),
        ori_ast::ty::Type::Void(_) => "void".to_string(),
        ori_ast::ty::Type::Named(q) => q.to_string(),
        ori_ast::ty::Type::Optional(t, _) => format!("optional[{}]", type_to_string(t)),
        ori_ast::ty::Type::Result(ok, err, _) => {
            format!("result[{}, {}]", type_to_string(ok), type_to_string(err))
        }
        ori_ast::ty::Type::List(t, _) => format!("list[{}]", type_to_string(t)),
        ori_ast::ty::Type::Map(k, v, _) => {
            format!("map[{}, {}]", type_to_string(k), type_to_string(v))
        }
        ori_ast::ty::Type::Set(t, _) => format!("set[{}]", type_to_string(t)),
        ori_ast::ty::Type::Range(t, _) => format!("range[{}]", type_to_string(t)),
        ori_ast::ty::Type::Tuple(types, _) => {
            let inner: Vec<_> = types.iter().map(type_to_string).collect();
            format!("({})", inner.join(", "))
        }
        ori_ast::ty::Type::Func {
            params, return_ty, ..
        } => {
            let p: Vec<_> = params.iter().map(type_to_string).collect();
            let ret = return_ty
                .as_ref()
                .map(|t| format!(" -> {}", type_to_string(t)))
                .unwrap_or_default();
            format!("func({}){}", p.join(", "), ret)
        }
        ori_ast::ty::Type::Generic { name, args, .. } => {
            let a: Vec<_> = args.iter().map(type_to_string).collect();
            if a.is_empty() {
                name.to_string()
            } else {
                format!("{}[{}]", name, a.join(", "))
            }
        }
        _ => "?".to_string(),
    }
}

fn span_to_range(source: &str, span: ori_diagnostics::Span) -> Range {
    let start = position::position_for_byte_offset(source, span.start as usize);
    let end = position::position_for_byte_offset(source, span.end as usize);
    Range::new(start, end)
}

fn position_in_range(pos: Position, range: &Range) -> bool {
    !position_is_before(pos, range.start) && position_is_before(pos, range.end)
}

fn position_is_before(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character < right.character)
}

#[cfg(test)]
mod tests {
    use super::{type_to_string, SemanticIndex};
    use crate::utils::position;
    use ori_ast::common::{Name, QualifiedName};
    use ori_ast::ty::Type;
    use ori_diagnostics::Span;

    #[test]
    fn semantic_index_hides_inactive_cfg_declarations() {
        let root = std::env::temp_dir().join(format!(
            "ori_lsp_cfg_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create LSP cfg fixture");
        let source = r#"module app.main

active_symbol()
end

@cfg(feature: hidden)
inactive_symbol()
end

main()
end
"#;
        std::fs::write(root.join("main.orl"), source).expect("write LSP cfg source");
        std::fs::write(
            root.join("ori.proj"),
            "manifest = 1\nname = \"lsp_cfg\"\nversion = \"0.1.0\"\nkind = \"app\"\nentry = \"main.orl\"\n\n[features]\ndefault = []\nhidden = []\n",
        )
        .expect("write LSP cfg manifest");

        let index = SemanticIndex::build(source, Some(&root.join("main.orl")));
        assert!(index.hover("active_symbol").is_some());
        assert!(index.hover("inactive_symbol").is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn local_references_follow_shadowing_identity() {
        let source = r#"module app.main

main()
    const value = 1
    value
    if true
        const value = 2
        value
    end
    value
end
"#;
        let index = SemanticIndex::build(source, None);
        let inner_use = source
            .find("        value\n")
            .expect("fixture contains inner use");
        let outer_use = source
            .rfind("    value\n")
            .expect("fixture contains outer use");

        let inner = index.find_local_references_at(
            source,
            position::position_for_byte_offset(source, inner_use + 8),
        );
        let outer = index.find_local_references_at(
            source,
            position::position_for_byte_offset(source, outer_use + 4),
        );

        assert_eq!(inner.len(), 2, "inner declaration and inner use");
        assert_eq!(outer.len(), 3, "outer declaration and two outer uses");
    }

    #[test]
    fn hover_type_rendering_uses_canonical_bracket_generics() {
        let span = Span::DUMMY;
        let name = QualifiedName::single(Name::new("Box", span));
        let generic = Type::Generic {
            name,
            args: vec![Type::Optional(Box::new(Type::Int(span)), span)],
            span,
        };

        assert_eq!(type_to_string(&generic), "Box[optional[int]]");
        assert!(!type_to_string(&generic).contains('<'));
        assert!(!type_to_string(&generic).contains('?'));
    }

    #[test]
    fn lexical_references_exclude_comments_and_strings() {
        let source = r#"module app.main

value()
end

main()
    -- value is only comment text
    const text = "value"
    value()
end
"#;
        let index = SemanticIndex::build(source, None);
        assert_eq!(index.find_references(source, "value").len(), 2);
    }

    #[test]
    fn local_definition_works_after_mixed_unicode_text() {
        let source =
            "module app.main\n\nmain()\n    const note = \"é界e\\u{301}🙂\"\n    note\nend\n";
        let index = SemanticIndex::build(source, None);
        let usage = source.rfind("note").expect("fixture contains usage");
        let declaration = source.find("note").expect("fixture contains declaration");
        let range = index
            .definition_at(source, position::position_for_byte_offset(source, usage))
            .expect("local definition resolves");

        assert_eq!(
            range.start,
            position::position_for_byte_offset(source, declaration)
        );
    }

    #[test]
    fn closure_parameter_has_its_own_binding_identity() {
        let source = r#"module app.main

main()
    const value = 10
    const increment: func(int) -> int = (value) => value + 1
    check value == 10
end
"#;
        let index = SemanticIndex::build(source, None);
        let closure_use = source
            .find("value + 1")
            .expect("fixture contains closure use");
        let outer_use = source
            .rfind("value == 10")
            .expect("fixture contains outer use");

        assert_eq!(
            index
                .find_local_references_at(
                    source,
                    position::position_for_byte_offset(source, closure_use),
                )
                .len(),
            2
        );
        assert_eq!(
            index
                .find_local_references_at(
                    source,
                    position::position_for_byte_offset(source, outer_use),
                )
                .len(),
            2
        );
    }

    #[test]
    fn field_and_named_label_do_not_alias_a_same_named_local() {
        let source = r#"module app.main

struct Box
    value: int
end

consume(value: int)
end

main()
    const value = 10
    const boxed: Box = Box { value: 20 }
    check boxed.value == value
    consume(value: value)
end
"#;
        let index = SemanticIndex::build(source, None);
        let final_value = source
            .rfind("value)")
            .expect("fixture contains named argument value");
        let references = index.find_local_references_at(
            source,
            position::position_for_byte_offset(source, final_value),
        );

        assert_eq!(references.len(), 3, "declaration and two value uses");
    }
}
