//! String literal collection for the native backend.
//!
//! Extracted from `native_backend.rs` as part of Etapa 8.3 monolith
//! reduction. Pure HIR traversal — no Cranelift or codegen-specific
//! dependencies. The entry point `collect_all_strings` is called from
//! `native_backend.rs` to gather every string literal in a module so the
//! codegen can emit them as pre-interned runtime values.

use smol_str::SmolStr;

use ori_hir::hir::*;

struct StringCollector {
    out: Vec<SmolStr>,
    seen: std::collections::HashSet<SmolStr>,
}

impl StringCollector {
    fn new() -> Self {
        let mut seen = std::collections::HashSet::new();
        let empty = SmolStr::new("");
        seen.insert(empty.clone());
        Self {
            out: vec![empty],
            seen,
        }
    }

    fn add(&mut self, s: SmolStr) {
        if self.seen.insert(s.clone()) {
            self.out.push(s);
        }
    }
}

fn collect_strings_expr(expr: &HirExpr, out: &mut StringCollector) {
    match &expr.kind {
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::None_
        | HirExprKind::Var(_)
        | HirExprKind::Closure { .. } => {}
        HirExprKind::StrLit(s) => out.add(s.clone()),
        HirExprKind::Call { callee, args } => {
            collect_strings_expr(callee, out);
            for a in args {
                collect_strings_expr(&a.value, out);
            }
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            collect_strings_expr(lhs, out);
            collect_strings_expr(rhs, out);
        }
        HirExprKind::Unary { operand, .. } => collect_strings_expr(operand, out),
        HirExprKind::Field { object, .. } => collect_strings_expr(object, out),
        HirExprKind::IfExpr { cond, then, else_ } => {
            collect_strings_expr(cond, out);
            collect_strings_expr(then, out);
            collect_strings_expr(else_, out);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            collect_strings_expr(scrutinee, out);
            for arm in arms {
                collect_strings_pattern(&arm.pattern, out);
                if let Some(guard) = &arm.guard {
                    collect_strings_expr(guard, out);
                }
                collect_strings_expr(&arm.body, out);
            }
        }
        HirExprKind::Propagate(e)
        | HirExprKind::Await(e)
        | HirExprKind::Some_(e)
        | HirExprKind::Ok_(e)
        | HirExprKind::Err_(e) => collect_strings_expr(e, out),
        HirExprKind::ListLit { elements, .. } => {
            for e in elements {
                collect_strings_expr(e, out);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for e in elements {
                collect_strings_expr(&e.value, out);
            }
        }
        HirExprKind::TupleLit(elems) => {
            for e in elems {
                collect_strings_expr(e, out);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                match p {
                    HirStrPart::Literal(s) => out.add(s.clone()),
                    HirStrPart::Expr(e) => collect_strings_expr(e, out),
                }
            }
        }
        HirExprKind::Range { start, end } => {
            collect_strings_expr(start, out);
            collect_strings_expr(end, out);
        }
        HirExprKind::StructLit { fields, .. } => {
            for (_, e) in fields {
                collect_strings_expr(e, out);
            }
        }
        HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                collect_strings_expr(e, out);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_strings_expr(k, out);
                collect_strings_expr(v, out);
            }
        }
        HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                collect_strings_expr(e, out);
            }
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            collect_strings_expr(base, out);
            for (_, e) in updates {
                collect_strings_expr(e, out);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            collect_strings_expr(receiver, out);
            for a in args {
                collect_strings_expr(a, out);
            }
        }
        HirExprKind::Index { object, index } => {
            collect_strings_expr(object, out);
            collect_strings_expr(index, out);
        }
        HirExprKind::TupleIndex { object, .. } => {
            collect_strings_expr(object, out);
        }
        HirExprKind::IsCheck { value, .. } => collect_strings_expr(value, out),
    }
}

fn collect_strings_block(block: &HirBlock, out: &mut StringCollector) {
    for s in &block.stmts {
        collect_strings_stmt(s, out);
    }
}

fn collect_strings_stmt(stmt: &HirStmt, out: &mut StringCollector) {
    match stmt {
        HirStmt::Let { value, .. } => collect_strings_expr(value, out),
        HirStmt::Assign { lvalue, value, .. } => {
            collect_strings_lvalue(lvalue, out);
            collect_strings_expr(value, out);
        }
        HirStmt::Return(Some(e), _) => collect_strings_expr(e, out),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::Expr(e) => collect_strings_expr(e, out),
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            collect_strings_expr(cond, out);
            collect_strings_block(then, out);
            for (c, b) in else_ifs {
                collect_strings_expr(c, out);
                collect_strings_block(b, out);
            }
            if let Some(eb) = else_ {
                collect_strings_block(eb, out);
            }
        }
        HirStmt::While { cond, body, .. } => {
            collect_strings_expr(cond, out);
            collect_strings_block(body, out);
        }
        HirStmt::For { iterable, body, .. } => {
            collect_strings_expr(iterable, out);
            collect_strings_block(body, out);
        }
        HirStmt::Loop { body, .. } => collect_strings_block(body, out),
        HirStmt::Repeat { count, body, .. } => {
            collect_strings_expr(count, out);
            collect_strings_block(body, out);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            collect_strings_expr(scrutinee, out);
            for arm in arms {
                collect_strings_pattern(&arm.pattern, out);
                if let Some(guard) = &arm.guard {
                    collect_strings_expr(guard, out);
                }
                for s in &arm.body {
                    collect_strings_stmt(s, out);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            collect_strings_expr(value, out);
            collect_strings_block(then, out);
            if let Some(eb) = else_ {
                collect_strings_block(eb, out);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            collect_strings_expr(value, out);
            collect_strings_block(body, out);
        }
        HirStmt::Using { value, .. } => collect_strings_expr(value, out),
        HirStmt::Check {
            condition, message, ..
        } => {
            collect_strings_expr(condition, out);
            if let Some(message) = message {
                out.add(message.clone());
            }
        }
    }
}

fn collect_strings_lvalue(lvalue: &HirLValue, out: &mut StringCollector) {
    match lvalue {
        HirLValue::Var(_) => {}
        HirLValue::Field { base, .. } => collect_strings_lvalue(base, out),
        HirLValue::Index { base, index } => {
            collect_strings_lvalue(base, out);
            collect_strings_expr(index, out);
        }
    }
}

fn collect_strings_pattern(pat: &HirPattern, out: &mut StringCollector) {
    match pat {
        HirPattern::Or(alternatives) => {
            for alternative in alternatives {
                collect_strings_pattern(alternative, out);
            }
        }
        HirPattern::Wildcard
        | HirPattern::Binding(_, _)
        | HirPattern::BoolLit(_)
        | HirPattern::IntLit(_)
        | HirPattern::None_ => {}
        HirPattern::StrLit(s) => out.add(s.clone()),
        HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
            collect_strings_pattern(inner, out);
        }
        HirPattern::Variant { fields, .. } => {
            for (_, pat) in fields {
                collect_strings_pattern(pat, out);
            }
        }
        HirPattern::Tuple(patterns) => {
            for pat in patterns {
                collect_strings_pattern(pat, out);
            }
        }
    }
}

pub(super) fn collect_all_strings(hir: &HirModule) -> Vec<SmolStr> {
    let mut collector = StringCollector::new();
    for f in &hir.funcs {
        collect_strings_block(&f.body, &mut collector);
    }
    for c in &hir.consts {
        collect_strings_expr(&c.value, &mut collector);
    }
    collector.out
}
