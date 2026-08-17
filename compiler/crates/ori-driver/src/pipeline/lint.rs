//! Semantic linter pipeline (`ori lint`).
//!
//! Analyzes Ori source files for code quality, redundant constructs,
//! unused bindings, and actionable improvements without changing program semantics.

use ori_ast::expr::{ArgValue, BinaryOp, Expr, IndexExpr, UnaryOp};
use ori_ast::item::{FuncDecl, Item, ItemWithAttrs};
use ori_ast::stmt::{IfSomeStmt, IfStmt, LocalConst, LocalVar, ReturnStmt, Stmt, WhileStmt};
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, Severity, SourceCache, Span};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::frontend::check_loaded_sources;
use super::project::{load_and_resolve, LoadedSource};

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
    let loaded = sources.loaded;
    let resolved = sources.resolved;

    // Run standard semantic checks first so the AST is known to be valid.
    if !sink.has_errors() {
        check_loaded_sources(&loaded, &resolved, &mut sink);
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
                labels: vec![Label::primary(file_id, attr.span, "empty attribute predicate")],
                why: Some("A `@cfg` without arguments does not filter declarations.".to_string()),
                action: Some("Provide a target/feature predicate or remove the attribute.".to_string()),
                notes: Vec::new(),
            });
        }
    }

    if let Item::Func(func) = &item.item {
        lint_func(func, file_id, sink);
    }
}

fn lint_func(func: &FuncDecl, file_id: FileId, sink: &mut DiagnosticSink) {
    let mut declared_bindings: HashMap<String, (Span, bool)> = HashMap::new(); // name -> (span, is_var)
    let mut used_identifiers: HashSet<String> = HashSet::new();
    let mut mutated_identifiers: HashSet<String> = HashSet::new();
    let mut outer_scope: HashSet<String> = HashSet::new();

    // Include function parameters in outer scope
    for param in &func.params {
        outer_scope.insert(param.name.as_str().to_string());
    }

    // Traverse statements to find declarations, mutations and usages
    for stmt in &func.body.stmts {
        collect_declarations_and_usages(
            stmt,
            &mut declared_bindings,
            &mut used_identifiers,
            &mut mutated_identifiers,
            &outer_scope,
            file_id,
            sink,
        );
    }

    // Emit unused variable warnings for bindings not starting with `_`
    for (name, (span, is_var)) in declared_bindings {
        if !name.starts_with('_') && !used_identifiers.contains(&name) {
            sink.emit(Diagnostic {
                severity: Severity::Warning,
                code: "lint.unused_variable",
                message: format!("variable `{name}` is declared but never read"),
                labels: vec![Label::primary(file_id, span, "unused variable")],
                why: Some("Variables that are never read consume memory and may indicate a typo or forgotten logic.".to_string()),
                action: Some(format!("Prefix with an underscore `_{name}` if intentionally unused, or remove it.")),
                notes: Vec::new(),
            });
        } else if is_var && !name.starts_with('_') && !mutated_identifiers.contains(&name) && used_identifiers.contains(&name) {
            sink.emit(Diagnostic {
                severity: Severity::Warning,
                code: "lint.prefer_const",
                message: format!("variable `{name}` is never mutated, prefer `const`"),
                labels: vec![Label::primary(file_id, span, "never mutated")],
                why: Some("Immutable bindings declared with `const` make code intent clearer and prevent accidental mutation.".to_string()),
                action: Some(format!("Change `var {name}` to `const {name}`.")),
                notes: Vec::new(),
            });
        }
    }
}

fn collect_declarations_and_usages(
    stmt: &Stmt,
    declarations: &mut HashMap<String, (Span, bool)>,
    usages: &mut HashSet<String>,
    mutations: &mut HashSet<String>,
    outer_scope: &HashSet<String>,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) {
    match stmt {
        Stmt::Var(LocalVar { name, value, .. }) => {
            let var_name = name.as_str().to_string();
            if outer_scope.contains(&var_name) {
                sink.emit(Diagnostic {
                    severity: Severity::Warning,
                    code: "lint.shadowed_variable",
                    message: format!("local variable `{var_name}` shadows an existing binding"),
                    labels: vec![Label::primary(file_id, name.span, "shadows outer binding")],
                    why: Some("Shadowing variables in inner scopes can lead to subtle bugs and confusion.".to_string()),
                    action: Some("Rename the local variable to avoid shadowing.".to_string()),
                    notes: Vec::new(),
                });
            }
            declarations.insert(var_name, (name.span, true));
            lint_expr(value, usages, file_id, sink);
        }
        Stmt::Const(LocalConst { name, value, .. }) => {
            let const_name = name.as_str().to_string();
            if outer_scope.contains(&const_name) {
                sink.emit(Diagnostic {
                    severity: Severity::Warning,
                    code: "lint.shadowed_variable",
                    message: format!("local constant `{const_name}` shadows an existing binding"),
                    labels: vec![Label::primary(file_id, name.span, "shadows outer binding")],
                    why: Some("Shadowing variables in inner scopes can lead to subtle bugs and confusion.".to_string()),
                    action: Some("Rename the local constant to avoid shadowing.".to_string()),
                    notes: Vec::new(),
                });
            }
            declarations.insert(const_name, (name.span, false));
            lint_expr(value, usages, file_id, sink);
        }
        Stmt::Assign(ori_ast::stmt::AssignStmt { lvalue, value, .. }) => {
            collect_lvalue_mutation(lvalue, mutations);
            lint_expr(value, usages, file_id, sink);
        }
        Stmt::CompoundAssign(ori_ast::stmt::CompoundAssignStmt { lvalue, value, .. }) => {
            collect_lvalue_mutation(lvalue, mutations);
            lint_expr(value, usages, file_id, sink);
        }
        Stmt::Expr(expr) => {
            lint_expr(expr, usages, file_id, sink);
        }
        Stmt::Return(ReturnStmt { value: Some(expr), .. }) => {
            lint_expr(expr, usages, file_id, sink);
        }
        Stmt::While(WhileStmt { condition, body, .. }) => {
            lint_expr(condition, usages, file_id, sink);
            let mut inner_scope = outer_scope.clone();
            for k in declarations.keys() {
                inner_scope.insert(k.clone());
            }
            for s in &body.stmts {
                collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
            }
        }
        Stmt::For(for_stmt) => {
            lint_expr(&for_stmt.iterable, usages, file_id, sink);
            let mut inner_scope = outer_scope.clone();
            inner_scope.insert(for_stmt.binding.as_str().to_string());
            if let Some(second) = &for_stmt.second_binding {
                inner_scope.insert(second.as_str().to_string());
            }
            for s in &for_stmt.body.stmts {
                collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
            }
        }
        Stmt::If(IfStmt {
            condition,
            then_block,
            else_ifs,
            else_block,
            ..
        }) => {
            lint_expr(condition, usages, file_id, sink);
            let mut inner_scope = outer_scope.clone();
            for k in declarations.keys() {
                inner_scope.insert(k.clone());
            }
            for s in &then_block.stmts {
                collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
            }
            for (elif_cond, elif_block) in else_ifs {
                lint_expr(elif_cond, usages, file_id, sink);
                for s in &elif_block.stmts {
                    collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
                }
            }
            if let Some(else_block) = else_block {
                for s in &else_block.stmts {
                    collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
                }
            }
        }
        Stmt::IfSome(IfSomeStmt {
            binding,
            value,
            then_block,
            else_block,
            ..
        }) => {
            let mut inner_scope = outer_scope.clone();
            inner_scope.insert(binding.as_str().to_string());
            declarations.insert(binding.as_str().to_string(), (binding.span, false));
            lint_expr(value, usages, file_id, sink);
            for s in &then_block.stmts {
                collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
            }
            if let Some(else_block) = else_block {
                for s in &else_block.stmts {
                    collect_declarations_and_usages(s, declarations, usages, mutations, &inner_scope, file_id, sink);
                }
            }
        }
        _ => {}
    }
}

fn collect_lvalue_mutation(lvalue: &ori_ast::stmt::LValue, mutations: &mut HashSet<String>) {
    match lvalue {
        ori_ast::stmt::LValue::Ident(name) => {
            mutations.insert(name.as_str().to_string());
        }
        ori_ast::stmt::LValue::Field { base, .. } => collect_lvalue_mutation(base, mutations),
        ori_ast::stmt::LValue::Index { base, .. } => collect_lvalue_mutation(base, mutations),
    }
}

fn lint_expr(expr: &Expr, usages: &mut HashSet<String>, file_id: FileId, sink: &mut DiagnosticSink) {
    match expr {
        Expr::Ident(name) => {
            usages.insert(name.as_str().to_string());
        }
        Expr::QualifiedIdent(qname) => {
            if let Some(first) = qname.parts.first() {
                usages.insert(first.as_str().to_string());
            }
        }
        Expr::FStrLit { parts, .. } => {
            for part in parts {
                if let ori_ast::expr::FStrPart::Interpolated(e) = part {
                    lint_expr(e, usages, file_id, sink);
                }
            }
        }
        Expr::Range { start, end, .. } => {
            lint_expr(start, usages, file_id, sink);
            lint_expr(end, usages, file_id, sink);
        }
        Expr::Unary {
            op,
            operand,
            span,
        } => {
            if *op == UnaryOp::Not {
                if let Expr::Unary {
                    op: UnaryOp::Not, ..
                } = &**operand
                {
                    sink.emit(Diagnostic {
                        severity: Severity::Warning,
                        code: "lint.double_negation",
                        message: "double logical negation (`not (not ...)`) is redundant".to_string(),
                        labels: vec![Label::primary(file_id, *span, "redundant double negation")],
                        why: Some("Negating a boolean value twice returns the original boolean condition.".to_string()),
                        action: Some("Remove both `not` operators.".to_string()),
                        notes: Vec::new(),
                    });
                }
            }
            lint_expr(operand, usages, file_id, sink);
        }
        Expr::Binary {
            op,
            lhs,
            rhs,
            span,
        } => {
            if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                let left_is_bool = matches!(&**lhs, Expr::BoolLit(..));
                let right_is_bool = matches!(&**rhs, Expr::BoolLit(..));
                if left_is_bool || right_is_bool {
                    sink.emit(Diagnostic {
                        severity: Severity::Warning,
                        code: "lint.redundant_bool_comparison",
                        message: "comparison with a boolean literal is redundant".to_string(),
                        labels: vec![Label::primary(file_id, *span, "redundant boolean comparison")],
                        why: Some("Boolean values can be used directly in conditions without comparing against `true` or `false`.".to_string()),
                        action: Some("Use the condition directly or invert it with `not`.".to_string()),
                        notes: Vec::new(),
                    });
                }
            }
            lint_expr(lhs, usages, file_id, sink);
            lint_expr(rhs, usages, file_id, sink);
        }
        Expr::IfExpr {
            condition,
            then_expr,
            else_expr,
            span,
        } => {
            lint_expr(condition, usages, file_id, sink);
            lint_expr(then_expr, usages, file_id, sink);
            if let (Expr::BoolLit(true, _), Expr::BoolLit(false, _)) = (&**then_expr, &**else_expr) {
                sink.emit(Diagnostic {
                    severity: Severity::Warning,
                    code: "lint.redundant_if_boolean",
                    message: "`if condition then true else false` is redundant".to_string(),
                    labels: vec![Label::primary(file_id, *span, "redundant if-boolean expression")],
                    why: Some("An `if` expression that simply yields `true` for then and `false` for else evaluates to the condition itself.".to_string()),
                    action: Some("Replace with the condition expression directly.".to_string()),
                    notes: Vec::new(),
                });
            }
            lint_expr(else_expr, usages, file_id, sink);
        }
        Expr::Call { callee, args, .. } => {
            lint_expr(callee, usages, file_id, sink);
            for arg in args {
                match &arg.value {
                    ArgValue::Expr(e) | ArgValue::Spread(e) => lint_expr(e, usages, file_id, sink),
                }
            }
        }
        Expr::Pipe { value, func, .. } => {
            lint_expr(value, usages, file_id, sink);
            lint_expr(func, usages, file_id, sink);
        }
        Expr::Field { object, .. } => {
            lint_expr(object, usages, file_id, sink);
        }
        Expr::Index { object, index, .. } => {
            lint_expr(object, usages, file_id, sink);
            match index {
                IndexExpr::Single(idx) => lint_expr(idx, usages, file_id, sink),
                IndexExpr::Range { start, end } => {
                    if let Some(s) = start {
                        lint_expr(s, usages, file_id, sink);
                    }
                    if let Some(e) = end {
                        lint_expr(e, usages, file_id, sink);
                    }
                }
            }
        }
        Expr::List { elements, .. } => {
            for item in elements {
                lint_expr(item, usages, file_id, sink);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                lint_expr(k, usages, file_id, sink);
                lint_expr(v, usages, file_id, sink);
            }
        }
        Expr::Set { elements, .. } => {
            for item in elements {
                lint_expr(item, usages, file_id, sink);
            }
        }
        Expr::Tuple { elements, .. } => {
            for item in elements {
                lint_expr(item, usages, file_id, sink);
            }
        }
        Expr::StructLit { fields, .. } | Expr::AnonStructLit { fields, .. } => {
            for f in fields {
                lint_expr(&f.value, usages, file_id, sink);
            }
        }
        Expr::EnumVariantNamed { fields, .. } => {
            for f in fields {
                lint_expr(&f.value, usages, file_id, sink);
            }
        }
        _ => {}
    }
}
