//! Semantic linter pipeline (`ori lint`).
//!
//! Analyzes Ori source files for code quality, redundant constructs,
//! unused bindings, and actionable improvements without changing program semantics.

use ori_ast::expr::{ArgValue, BinaryOp, Expr, IndexExpr, UnaryOp};
use ori_ast::item::{FuncDecl, Item, ItemWithAttrs};
use ori_ast::pattern::Pattern;
use ori_ast::stmt::{
    CheckStmt, IfSomeStmt, IfStmt, LocalConst, LocalVar, MatchCase, MatchStmt, RepeatStmt,
    ReturnStmt, Stmt, UsingStmt, WhileSomeStmt, WhileStmt,
};
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, Severity, SourceCache, Span};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::frontend::check_loaded_sources;
use super::project::{load_and_resolve, load_and_resolve_with_entry_source_and_cfg, LoadedSource};

pub struct LintOutput {
    pub cache: SourceCache,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    pub has_warnings: bool,
}

pub fn run_lint(path: &Path) -> Result<LintOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let sources = load_and_resolve(path, &mut cache, &mut sink)?;
    finish_lint(cache, sink, sources.loaded, &sources.resolved)
}

/// Lint an in-memory source buffer using the same parser, resolver, checker,
/// and AST traversal as `ori lint`. This is used by editor clients so their
/// warnings cannot diverge from the command-line linter or scan comments and
/// strings as if they were code.
pub fn run_lint_source(path: &Path, source: String) -> Result<LintOutput, String> {
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let sources =
        load_and_resolve_with_entry_source_and_cfg(path, source, None, &mut cache, &mut sink)?;
    finish_lint(cache, sink, sources.loaded, &sources.resolved)
}

fn finish_lint(
    cache: SourceCache,
    mut sink: DiagnosticSink,
    loaded: Vec<LoadedSource>,
    resolved: &ori_types::resolve::ResolvedModule,
) -> Result<LintOutput, String> {
    // Run standard semantic checks first so the AST is known to be valid.
    if !sink.has_errors() {
        check_loaded_sources(&loaded, resolved, &mut sink);
    }

    if !sink.has_errors() {
        for source in &loaded {
            lint_source(source, &mut sink);
        }
    }

    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    let has_warnings = diagnostics.iter().any(|d| d.severity == Severity::Warning);

    Ok(LintOutput {
        cache,
        diagnostics,
        has_errors,
        has_warnings,
    })
}

fn lint_source(source: &LoadedSource, sink: &mut DiagnosticSink) {
    let file_id = source.file_id;
    for item in &source.ast.items {
        lint_item(item, file_id, sink);
    }
}

fn lint_item(item: &ItemWithAttrs, file_id: FileId, sink: &mut DiagnosticSink) {
    // Check attributes for redundant cfg
    for attr in &item.attrs {
        if attr.name.as_str() == "cfg" && attr.args.is_empty() {
            sink.emit(Diagnostic {
                severity: Severity::Warning,
                code: "lint.unnecessary_cfg",
                message: "empty `@cfg` attribute has no effect".to_string(),
                labels: vec![Label::primary(
                    file_id,
                    attr.span,
                    "empty attribute predicate",
                )],
                why: Some("A `@cfg` without arguments does not filter declarations.".to_string()),
                action: Some(
                    "Provide a target/feature predicate or remove the attribute.".to_string(),
                ),
                notes: Vec::new(),
            });
        }
    }

    if let Item::Func(func) = &item.item {
        lint_func(func, file_id, sink);
    }
}

fn lint_func(func: &FuncDecl, file_id: FileId, sink: &mut DiagnosticSink) {
    let mut context = LintContext::new(file_id, sink);

    // Parameters participate in name resolution and shadowing, but are not
    // reported as unused: public APIs often intentionally expose them for
    // callers even when a function body does not read every parameter.
    for param in &func.params {
        context.declare_parameter(&param.name);
    }

    for stmt in &func.body.stmts {
        lint_stmt(stmt, &mut context);
    }

    // Keep diagnostics tied to binding identity, not just spelling. A nested
    // `x` must not mark an outer `x` as used (or hide its unused warning).
    let bindings = context.bindings.clone();
    for (id, binding) in bindings.iter().enumerate() {
        if !binding.warn_unused || binding.name.starts_with('_') {
            continue;
        }
        if !context.used.contains(&id) {
            context.sink.emit(Diagnostic {
                severity: Severity::Warning,
                code: "lint.unused_variable",
                message: format!("variable `{}` is declared but never read", binding.name),
                labels: vec![Label::primary(file_id, binding.span, "unused variable")],
                why: Some("Variables that are never read consume memory and may indicate a typo or forgotten logic.".to_string()),
                action: Some(format!("Prefix with an underscore `_{}` if intentionally unused, or remove it.", binding.name)),
                notes: Vec::new(),
            });
        } else if binding.is_var && !context.mutated.contains(&id) {
            context.sink.emit(Diagnostic {
                severity: Severity::Warning,
                code: "lint.prefer_const",
                message: format!("variable `{}` is never mutated, prefer `const`", binding.name),
                labels: vec![Label::primary(file_id, binding.span, "never mutated")],
                why: Some("Immutable bindings declared with `const` make code intent clearer and prevent accidental mutation.".to_string()),
                action: Some(format!("Change `var {0}` to `const {0}`.", binding.name)),
                notes: Vec::new(),
            });
        }
    }
}

type BindingId = usize;

#[derive(Clone)]
struct LintBinding {
    name: String,
    span: Span,
    is_var: bool,
    warn_unused: bool,
}

struct LintContext<'a> {
    file_id: FileId,
    sink: &'a mut DiagnosticSink,
    bindings: Vec<LintBinding>,
    scopes: Vec<HashMap<String, BindingId>>,
    used: HashSet<BindingId>,
    mutated: HashSet<BindingId>,
}

impl<'a> LintContext<'a> {
    fn new(file_id: FileId, sink: &'a mut DiagnosticSink) -> Self {
        Self {
            file_id,
            sink,
            bindings: Vec::new(),
            scopes: vec![HashMap::new()],
            used: HashSet::new(),
            mutated: HashSet::new(),
        }
    }

    fn with_scope(&mut self, f: impl FnOnce(&mut Self)) {
        self.scopes.push(HashMap::new());
        f(self);
        self.scopes.pop();
    }

    fn lookup(&self, name: &str) -> Option<BindingId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn declare_parameter(&mut self, name: &ori_ast::common::Name) {
        self.declare(name, false, false, "parameter");
    }

    fn declare(
        &mut self,
        name: &ori_ast::common::Name,
        is_var: bool,
        warn_unused: bool,
        kind: &str,
    ) {
        let name_text = name.as_str().to_string();
        if self.lookup(&name_text).is_some() {
            report_shadowing(&name_text, name.span, self.file_id, self.sink, kind);
        }
        let id = self.bindings.len();
        self.bindings.push(LintBinding {
            name: name_text.clone(),
            span: name.span,
            is_var,
            warn_unused,
        });
        self.scopes
            .last_mut()
            .expect("the function scope is always present")
            .insert(name_text, id);
    }

    fn mark_used(&mut self, name: &str) {
        if let Some(id) = self.lookup(name) {
            self.used.insert(id);
        }
    }

    fn mark_mutated(&mut self, name: &str) {
        if let Some(id) = self.lookup(name) {
            self.mutated.insert(id);
        }
    }
}

fn lint_stmt(stmt: &Stmt, context: &mut LintContext<'_>) {
    match stmt {
        Stmt::Var(LocalVar { name, value, .. }) => {
            lint_expr(value, context);
            context.declare(name, true, true, "variable");
        }
        Stmt::Const(LocalConst { name, value, .. }) => {
            lint_expr(value, context);
            context.declare(name, false, true, "constant");
        }
        Stmt::Destructure(local) => {
            lint_expr(&local.value, context);
            for (_, binding) in &local.fields {
                context.declare(
                    binding,
                    local.is_mutable,
                    true,
                    if local.is_mutable {
                        "variable"
                    } else {
                        "constant"
                    },
                );
            }
        }
        Stmt::Assign(ori_ast::stmt::AssignStmt { lvalue, value, .. }) => {
            lint_lvalue(lvalue, context);
            lint_expr(value, context);
        }
        Stmt::CompoundAssign(ori_ast::stmt::CompoundAssignStmt { lvalue, value, .. }) => {
            lint_lvalue(lvalue, context);
            lint_expr(value, context);
        }
        Stmt::Expr(expr) => {
            lint_expr(expr, context);
        }
        Stmt::Return(ReturnStmt {
            value: Some(expr), ..
        }) => {
            lint_expr(expr, context);
        }
        Stmt::Check(CheckStmt { condition, .. }) => {
            // `check` conditions are ordinary expressions. Ignoring them
            // makes a binding used only in an assertion look unused in the
            // CLI and LSP, which is both noisy and semantically wrong.
            lint_expr(condition, context);
        }
        Stmt::While(WhileStmt {
            condition, body, ..
        }) => {
            lint_expr(condition, context);
            context.with_scope(|context| {
                for statement in &body.stmts {
                    lint_stmt(statement, context);
                }
            });
        }
        Stmt::For(for_stmt) => {
            lint_expr(&for_stmt.iterable, context);
            context.with_scope(|context| {
                context.declare(&for_stmt.binding, false, true, "binding");
                if let Some(second) = &for_stmt.second_binding {
                    context.declare(second, false, true, "binding");
                }
                for statement in &for_stmt.body.stmts {
                    lint_stmt(statement, context);
                }
            });
        }
        Stmt::WhileSome(WhileSomeStmt {
            binding,
            value,
            body,
            ..
        }) => {
            lint_expr(value, context);
            context.with_scope(|context| {
                context.declare(binding, false, true, "binding");
                for statement in &body.stmts {
                    lint_stmt(statement, context);
                }
            });
        }
        Stmt::Repeat(RepeatStmt { count, body, .. }) => {
            lint_expr(count, context);
            context.with_scope(|context| {
                for statement in &body.stmts {
                    lint_stmt(statement, context);
                }
            });
        }
        Stmt::Loop(loop_stmt) => {
            context.with_scope(|context| {
                for statement in &loop_stmt.body.stmts {
                    lint_stmt(statement, context);
                }
            });
        }
        Stmt::Match(MatchStmt {
            scrutinee, cases, ..
        }) => {
            lint_expr(scrutinee, context);
            for case in cases {
                let (pattern, guard, body) = match case {
                    MatchCase::Pattern {
                        pattern,
                        guard,
                        body,
                        ..
                    } => (pattern, guard.as_deref(), body.as_slice()),
                    MatchCase::Else { body, .. } => {
                        context.with_scope(|context| {
                            for statement in body {
                                lint_stmt(statement, context);
                            }
                        });
                        continue;
                    }
                };
                context.with_scope(|context| {
                    collect_pattern_bindings(pattern, context);
                    if let Some(guard) = guard {
                        lint_expr(guard, context);
                    }
                    for statement in body {
                        lint_stmt(statement, context);
                    }
                });
            }
        }
        Stmt::Using(UsingStmt { name, value, .. }) => {
            lint_expr(value, context);
            context.declare(name, false, true, "resource");
        }
        Stmt::If(IfStmt {
            condition,
            then_block,
            else_ifs,
            else_block,
            ..
        }) => {
            lint_expr(condition, context);
            context.with_scope(|context| {
                for statement in &then_block.stmts {
                    lint_stmt(statement, context);
                }
            });
            for (elif_cond, elif_block) in else_ifs {
                lint_expr(elif_cond, context);
                context.with_scope(|context| {
                    for statement in &elif_block.stmts {
                        lint_stmt(statement, context);
                    }
                });
            }
            if let Some(else_block) = else_block {
                context.with_scope(|context| {
                    for statement in &else_block.stmts {
                        lint_stmt(statement, context);
                    }
                });
            }
        }
        Stmt::IfSome(IfSomeStmt {
            binding,
            value,
            then_block,
            else_block,
            ..
        }) => {
            lint_expr(value, context);
            context.with_scope(|context| {
                context.declare(binding, false, true, "binding");
                for statement in &then_block.stmts {
                    lint_stmt(statement, context);
                }
            });
            if let Some(else_block) = else_block {
                context.with_scope(|context| {
                    for statement in &else_block.stmts {
                        lint_stmt(statement, context);
                    }
                });
            }
        }
        _ => {}
    }
}

fn report_shadowing(
    name: &str,
    span: Span,
    file_id: FileId,
    sink: &mut DiagnosticSink,
    binding_kind: &str,
) {
    sink.emit(Diagnostic {
        severity: Severity::Warning,
        code: "lint.shadowed_variable",
        message: format!("local {binding_kind} `{name}` shadows an existing binding"),
        labels: vec![Label::primary(file_id, span, "shadows outer binding")],
        why: Some(
            "Shadowing variables in inner scopes can lead to subtle bugs and confusion."
                .to_string(),
        ),
        action: Some("Rename the local variable to avoid shadowing.".to_string()),
        notes: Vec::new(),
    });
}

fn collect_pattern_bindings(pattern: &Pattern, context: &mut LintContext<'_>) {
    match pattern {
        Pattern::Binding(name) => context.declare(name, false, true, "binding"),
        Pattern::Literal(value) => lint_expr(value, context),
        Pattern::VariantNamed { fields, .. } => {
            for field in fields {
                collect_pattern_bindings(&field.pattern, context);
            }
        }
        Pattern::Some(inner, _) | Pattern::Ok(inner, _) | Pattern::Err(inner, _) => {
            collect_pattern_bindings(inner, context);
        }
        Pattern::Tuple(elements, _) | Pattern::Or(elements, _) => {
            for element in elements {
                collect_pattern_bindings(element, context);
            }
        }
        Pattern::Wildcard(_) | Pattern::VariantUnit { .. } | Pattern::None(_) => {}
    }
}

fn lint_lvalue(lvalue: &ori_ast::stmt::LValue, context: &mut LintContext<'_>) {
    match lvalue {
        ori_ast::stmt::LValue::Ident(name) => {
            context.mark_mutated(name.as_str());
        }
        ori_ast::stmt::LValue::Field { base, .. } => lint_lvalue(base, context),
        ori_ast::stmt::LValue::Index { base, index, .. } => {
            lint_lvalue(base, context);
            lint_expr(index, context);
        }
    }
}

fn lint_expr(expr: &Expr, context: &mut LintContext<'_>) {
    match expr {
        Expr::Ident(name) => {
            context.mark_used(name.as_str());
        }
        Expr::QualifiedIdent(qname) => {
            if let Some(first) = qname.parts.first() {
                context.mark_used(first.as_str());
            }
        }
        Expr::FStrLit { parts, .. } => {
            for part in parts {
                if let ori_ast::expr::FStrPart::Interpolated(e) = part {
                    lint_expr(e, context);
                }
            }
        }
        Expr::Range { start, end, .. } => {
            lint_expr(start, context);
            lint_expr(end, context);
        }
        Expr::Unary { op, operand, span } => {
            if *op == UnaryOp::Not {
                if let Expr::Unary {
                    op: UnaryOp::Not, ..
                } = &**operand
                {
                    context.sink.emit(Diagnostic {
                        severity: Severity::Warning,
                        code: "lint.double_negation",
                        message: "double logical negation (`not (not ...)`) is redundant".to_string(),
                        labels: vec![Label::primary(context.file_id, *span, "redundant double negation")],
                        why: Some("Negating a boolean value twice returns the original boolean condition.".to_string()),
                        action: Some("Remove both `not` operators.".to_string()),
                        notes: Vec::new(),
                    });
                }
            }
            lint_expr(operand, context);
        }
        Expr::Binary { op, lhs, rhs, span } => {
            if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                let left_is_bool = matches!(&**lhs, Expr::BoolLit(..));
                let right_is_bool = matches!(&**rhs, Expr::BoolLit(..));
                if left_is_bool || right_is_bool {
                    context.sink.emit(Diagnostic {
                        severity: Severity::Warning,
                        code: "lint.redundant_bool_comparison",
                        message: "comparison with a boolean literal is redundant".to_string(),
                        labels: vec![Label::primary(context.file_id, *span, "redundant boolean comparison")],
                        why: Some("Boolean values can be used directly in conditions without comparing against `true` or `false`.".to_string()),
                        action: Some("Use the condition directly or invert it with `not`.".to_string()),
                        notes: Vec::new(),
                    });
                }
            }
            lint_expr(lhs, context);
            lint_expr(rhs, context);
        }
        Expr::IfExpr {
            condition,
            then_expr,
            else_expr,
            span,
        } => {
            lint_expr(condition, context);
            lint_expr(then_expr, context);
            if let (Expr::BoolLit(true, _), Expr::BoolLit(false, _)) = (&**then_expr, &**else_expr)
            {
                context.sink.emit(Diagnostic {
                    severity: Severity::Warning,
                    code: "lint.redundant_if_boolean",
                    message: "`if condition then true else false` is redundant".to_string(),
                    labels: vec![Label::primary(context.file_id, *span, "redundant if-boolean expression")],
                    why: Some("An `if` expression that simply yields `true` for then and `false` for else evaluates to the condition itself.".to_string()),
                    action: Some("Replace with the condition expression directly.".to_string()),
                    notes: Vec::new(),
                });
            }
            lint_expr(else_expr, context);
        }
        Expr::Call { callee, args, .. } => {
            lint_expr(callee, context);
            for arg in args {
                match &arg.value {
                    ArgValue::Expr(e) | ArgValue::Spread(e) => lint_expr(e, context),
                }
            }
        }
        Expr::Pipe { value, func, .. } => {
            lint_expr(value, context);
            lint_expr(func, context);
        }
        Expr::Field { object, .. } => {
            lint_expr(object, context);
        }
        Expr::Index { object, index, .. } => {
            lint_expr(object, context);
            match index {
                IndexExpr::Single(idx) => lint_expr(idx, context),
                IndexExpr::Range { start, end } => {
                    if let Some(s) = start {
                        lint_expr(s, context);
                    }
                    if let Some(e) = end {
                        lint_expr(e, context);
                    }
                }
            }
        }
        Expr::List { elements, .. } => {
            for item in elements {
                lint_expr(item, context);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                lint_expr(k, context);
                lint_expr(v, context);
            }
        }
        Expr::Set { elements, .. } => {
            for item in elements {
                lint_expr(item, context);
            }
        }
        Expr::Tuple { elements, .. } => {
            for item in elements {
                lint_expr(item, context);
            }
        }
        Expr::StructLit { fields, .. } | Expr::AnonStructLit { fields, .. } => {
            for f in fields {
                lint_expr(&f.value, context);
            }
        }
        Expr::EnumVariantNamed { fields, .. } => {
            for f in fields {
                lint_expr(&f.value, context);
            }
        }
        _ => {}
    }
}
