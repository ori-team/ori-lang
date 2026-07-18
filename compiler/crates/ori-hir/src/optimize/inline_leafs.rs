//! Monomorphic leaf inlining within a module (LANG-PERF-2-4).
//!
//! Only inlines small, non-recursive, same-module functions whose body is a
//! single `return expr` (or block ending in return), with no nested calls to
//! themselves.

use std::collections::HashMap;

use smol_str::SmolStr;

use crate::hir::*;

const MAX_INLINE_STMTS: usize = 8;

pub(super) fn inline_leafs_module(module: &mut HirModule) {
    // Collect leaf candidates: name -> (params, body stmts clone, return_ty)
    let mut leaves: HashMap<SmolStr, LeafFn> = HashMap::new();
    for f in &module.funcs {
        if f.is_async || f.name.as_str() == "main" {
            continue;
        }
        if f.body.stmts.len() > MAX_INLINE_STMTS {
            continue;
        }
        if func_calls_name(&f.body, f.name.as_str()) {
            continue; // recursive
        }
        // Only pure-ish: no using/async await in body
        if block_has_using_or_await(&f.body) {
            continue;
        }
        leaves.insert(
            f.name.clone(),
            LeafFn {
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                body: f.body.clone(),
            },
        );
    }
    if leaves.is_empty() {
        return;
    }
    for f in &mut module.funcs {
        inline_in_block(&mut f.body, &leaves);
    }
}

struct LeafFn {
    params: Vec<SmolStr>,
    body: HirBlock,
}

fn inline_in_block(block: &mut HirBlock, leaves: &HashMap<SmolStr, LeafFn>) {
    for stmt in &mut block.stmts {
        inline_in_stmt(stmt, leaves);
    }
}

fn inline_in_stmt(stmt: &mut HirStmt, leaves: &HashMap<SmolStr, LeafFn>) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Assign { value, .. } | HirStmt::Expr(value) => {
            inline_in_expr(value, leaves);
        }
        HirStmt::Return(Some(e), _) => inline_in_expr(e, leaves),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            inline_in_expr(cond, leaves);
            inline_in_block(then, leaves);
            for (c, b) in else_ifs {
                inline_in_expr(c, leaves);
                inline_in_block(b, leaves);
            }
            if let Some(b) = else_ {
                inline_in_block(b, leaves);
            }
        }
        HirStmt::While { cond, body, .. } => {
            inline_in_expr(cond, leaves);
            inline_in_block(body, leaves);
        }
        HirStmt::For { iterable, body, .. } => {
            inline_in_expr(iterable, leaves);
            inline_in_block(body, leaves);
        }
        HirStmt::Loop { body, .. } => inline_in_block(body, leaves),
        HirStmt::Repeat { count, body, .. } => {
            inline_in_expr(count, leaves);
            inline_in_block(body, leaves);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            inline_in_expr(scrutinee, leaves);
            for arm in arms {
                for s in &mut arm.body {
                    inline_in_stmt(s, leaves);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            inline_in_expr(value, leaves);
            inline_in_block(then, leaves);
            if let Some(b) = else_ {
                inline_in_block(b, leaves);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            inline_in_expr(value, leaves);
            inline_in_block(body, leaves);
        }
        HirStmt::Using { value, .. }
        | HirStmt::Check {
            condition: value, ..
        } => {
            inline_in_expr(value, leaves);
        }
    }
}

fn inline_in_expr(expr: &mut HirExpr, leaves: &HashMap<SmolStr, LeafFn>) {
    // Recurse first
    match &mut expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            inline_in_expr(lhs, leaves);
            inline_in_expr(rhs, leaves);
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand)
        | HirExprKind::Propagate(operand)
        | HirExprKind::Await(operand)
        | HirExprKind::IsCheck { value: operand, .. }
        | HirExprKind::TupleIndex {
            object: operand, ..
        } => inline_in_expr(operand, leaves),
        HirExprKind::Index { object, index } => {
            inline_in_expr(object, leaves);
            inline_in_expr(index, leaves);
        }
        HirExprKind::Call { callee, args } => {
            for a in args.iter_mut() {
                inline_in_expr(&mut a.value, leaves);
            }
            inline_in_expr(callee, leaves);
            // Try inline: callee is Var(name) and leaf exists
            if let HirExprKind::Var(name) = &callee.kind {
                if let Some(leaf) = leaves.get(name) {
                    if args.len() == leaf.params.len() && args.iter().all(|a| !a.spread) {
                        if let Some(inlined) = try_inline_return_expr(leaf, args) {
                            *expr = inlined;
                            return;
                        }
                    }
                }
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            inline_in_expr(receiver, leaves);
            for a in args {
                inline_in_expr(a, leaves);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            inline_in_expr(cond, leaves);
            inline_in_expr(then, leaves);
            inline_in_expr(else_, leaves);
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                inline_in_expr(e, leaves);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                inline_in_expr(e, leaves);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                inline_in_expr(&mut el.value, leaves);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                inline_in_expr(k, leaves);
                inline_in_expr(v, leaves);
            }
        }
        HirExprKind::Range { start, end } => {
            inline_in_expr(start, leaves);
            inline_in_expr(end, leaves);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            inline_in_expr(base, leaves);
            for (_, e) in updates {
                inline_in_expr(e, leaves);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    inline_in_expr(e, leaves);
                }
            }
        }
        _ => {}
    }
}

/// If leaf body is only `return expr;` (optionally with pure lets we skip),
/// substitute params and return the expr.
fn try_inline_return_expr(leaf: &LeafFn, args: &[HirArg]) -> Option<HirExpr> {
    // Only single return statement leaves for safety.
    if leaf.body.stmts.len() != 1 {
        return None;
    }
    let HirStmt::Return(Some(ret), _) = &leaf.body.stmts[0] else {
        return None;
    };
    let mut out = ret.clone();
    for (param, arg) in leaf.params.iter().zip(args.iter()) {
        subst_var(&mut out, param.as_str(), &arg.value);
    }
    Some(out)
}

fn subst_var(expr: &mut HirExpr, name: &str, replacement: &HirExpr) {
    match &mut expr.kind {
        HirExprKind::Var(n) if n.as_str() == name => {
            *expr = replacement.clone();
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            subst_var(lhs, name, replacement);
            subst_var(rhs, name, replacement);
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand)
        | HirExprKind::Propagate(operand)
        | HirExprKind::Await(operand)
        | HirExprKind::IsCheck { value: operand, .. }
        | HirExprKind::TupleIndex {
            object: operand, ..
        } => {
            subst_var(operand, name, replacement);
        }
        HirExprKind::Index { object, index } => {
            subst_var(object, name, replacement);
            subst_var(index, name, replacement);
        }
        HirExprKind::Call { callee, args } => {
            subst_var(callee, name, replacement);
            for a in args {
                subst_var(&mut a.value, name, replacement);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            subst_var(receiver, name, replacement);
            for a in args {
                subst_var(a, name, replacement);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            subst_var(cond, name, replacement);
            subst_var(then, name, replacement);
            subst_var(else_, name, replacement);
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                subst_var(e, name, replacement);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                subst_var(e, name, replacement);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                subst_var(&mut el.value, name, replacement);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                subst_var(k, name, replacement);
                subst_var(v, name, replacement);
            }
        }
        HirExprKind::Range { start, end } => {
            subst_var(start, name, replacement);
            subst_var(end, name, replacement);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            subst_var(base, name, replacement);
            for (_, e) in updates {
                subst_var(e, name, replacement);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    subst_var(e, name, replacement);
                }
            }
        }
        _ => {}
    }
}

fn func_calls_name(block: &HirBlock, name: &str) -> bool {
    block.stmts.iter().any(|s| stmt_calls_name(s, name))
}

fn stmt_calls_name(stmt: &HirStmt, name: &str) -> bool {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Assign { value, .. } | HirStmt::Expr(value) => {
            expr_calls_name(value, name)
        }
        HirStmt::Return(Some(e), _) => expr_calls_name(e, name),
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            expr_calls_name(cond, name)
                || func_calls_name(then, name)
                || else_ifs
                    .iter()
                    .any(|(c, b)| expr_calls_name(c, name) || func_calls_name(b, name))
                || else_.as_ref().is_some_and(|b| func_calls_name(b, name))
        }
        HirStmt::While { cond, body, .. } => {
            expr_calls_name(cond, name) || func_calls_name(body, name)
        }
        HirStmt::For { iterable, body, .. } => {
            expr_calls_name(iterable, name) || func_calls_name(body, name)
        }
        HirStmt::Loop { body, .. } | HirStmt::Repeat { body, .. } => func_calls_name(body, name),
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            expr_calls_name(scrutinee, name)
                || arms
                    .iter()
                    .any(|a| a.body.iter().any(|s| stmt_calls_name(s, name)))
        }
        _ => false,
    }
}

fn expr_calls_name(expr: &HirExpr, name: &str) -> bool {
    match &expr.kind {
        HirExprKind::Call { callee, args } => {
            matches!(&callee.kind, HirExprKind::Var(n) if n.as_str() == name)
                || expr_calls_name(callee, name)
                || args.iter().any(|a| expr_calls_name(&a.value, name))
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            expr_calls_name(lhs, name) || expr_calls_name(rhs, name)
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand) => expr_calls_name(operand, name),
        HirExprKind::IfExpr { cond, then, else_ } => {
            expr_calls_name(cond, name)
                || expr_calls_name(then, name)
                || expr_calls_name(else_, name)
        }
        _ => false,
    }
}

fn block_has_using_or_await(block: &HirBlock) -> bool {
    block.stmts.iter().any(|s| match s {
        HirStmt::Using { .. } => true,
        HirStmt::Expr(e) | HirStmt::Let { value: e, .. } | HirStmt::Return(Some(e), _) => {
            expr_has_await(e)
        }
        HirStmt::While { body, .. } | HirStmt::Loop { body, .. } => block_has_using_or_await(body),
        _ => false,
    })
}

fn expr_has_await(expr: &HirExpr) -> bool {
    matches!(expr.kind, HirExprKind::Await(_))
}
