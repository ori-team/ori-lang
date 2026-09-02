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
        // Parameter contracts run at the call boundary. Substitution would
        // erase that boundary and silently skip the contract check.
        if f.params.iter().any(|param| param.contract.is_some()) {
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
                if let Some(guard) = &mut arm.guard {
                    inline_in_expr(guard, leaves);
                }
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
        HirExprKind::MatchExpr { scrutinee, arms } => {
            inline_in_expr(scrutinee, leaves);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    inline_in_expr(guard, leaves);
                }
                inline_in_expr(&mut arm.body, leaves);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                inline_in_expr(e, leaves);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
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

    // Substitution has no temporary-binding representation yet. Restrict
    // arguments to expressions whose evaluation is repeatable and cannot
    // trap, allocate, call user code, or observe an evaluation-order change.
    // This is intentionally conservative: a missed inline is safe, while an
    // omitted or duplicated argument is a language-semantics bug.
    if args.iter().any(|arg| !is_pure_inline_argument(&arg.value)) {
        return None;
    }

    // Even a pure expression should not be cloned into multiple parameter
    // uses until the HIR can materialize one argument binding. Reading a
    // parameter once keeps this pass correct for managed values and future
    // effect annotations alike.
    if leaf
        .params
        .iter()
        .any(|param| count_var_uses(ret, param.as_str()) > 1)
    {
        return None;
    }
    // Closures store capture names, `match` introduces textual bindings, and
    // propagation/await carry control flow tied to the callee. None can be
    // moved safely by textual substitution.
    if expr_has_inline_barrier(ret) {
        return None;
    }

    let mut out = ret.clone();
    for (param, arg) in leaf.params.iter().zip(args.iter()) {
        subst_var(&mut out, param.as_str(), &arg.value);
    }
    Some(out)
}

fn is_pure_inline_argument(expr: &HirExpr) -> bool {
    use ori_ast::expr::BinaryOp;

    // A call boundary currently performs the retain/release bookkeeping for
    // runtime-managed values. Substituting a managed variable directly would
    // remove that ownership boundary without an equivalent HIR temporary.
    // Keep this pass scalar-only until ownership-aware temporaries exist.
    if expr.ty.is_runtime_managed() {
        return false;
    }

    match &expr.kind {
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::Unit
        | HirExprKind::None_ => true,
        // A scalar read is not a stable value across the inlined body. For
        // example, another call in the return expression may mutate a global
        // after arguments were supposed to have been evaluated. An explicit
        // HIR temporary is required before variable arguments are safe here.
        HirExprKind::Var(_) => false,
        HirExprKind::Binary { op, lhs, rhs } => {
            !matches!(
                op,
                BinaryOp::Div | BinaryOp::Rem | BinaryOp::Shl | BinaryOp::Shr
            ) && !matches!(expr.ty, ori_types::Ty::String | ori_types::Ty::Bytes)
                && is_pure_inline_argument(lhs)
                && is_pure_inline_argument(rhs)
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::TupleIndex {
            object: operand, ..
        }
        | HirExprKind::IsCheck { value: operand, .. } => is_pure_inline_argument(operand),
        _ => false,
    }
}

fn count_var_uses(expr: &HirExpr, name: &str) -> usize {
    match &expr.kind {
        HirExprKind::Var(candidate) => usize::from(candidate.as_str() == name),
        HirExprKind::Binary { lhs, rhs, .. } => {
            count_var_uses(lhs, name).saturating_add(count_var_uses(rhs, name))
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
        } => count_var_uses(operand, name),
        HirExprKind::Index { object, index } => {
            count_var_uses(object, name).saturating_add(count_var_uses(index, name))
        }
        HirExprKind::Call { callee, args } => args
            .iter()
            .fold(count_var_uses(callee, name), |count, arg| {
                count.saturating_add(count_var_uses(&arg.value, name))
            }),
        HirExprKind::MethodCall { receiver, args, .. } => args
            .iter()
            .fold(count_var_uses(receiver, name), |count, arg| {
                count.saturating_add(count_var_uses(arg, name))
            }),
        HirExprKind::AssociatedCall { args, .. } => args.iter().fold(0, |count, arg| {
            count.saturating_add(count_var_uses(arg, name))
        }),
        HirExprKind::IfExpr { cond, then, else_ } => [cond, then, else_]
            .into_iter()
            .map(|branch| count_var_uses(branch, name))
            .fold(0, usize::saturating_add),
        HirExprKind::MatchExpr { scrutinee, arms } => {
            let arm_uses = arms.iter().fold(0usize, |count, arm| {
                let guard_uses = arm
                    .guard
                    .as_ref()
                    .map_or(0, |guard| count_var_uses(guard, name));
                count
                    .saturating_add(guard_uses)
                    .saturating_add(count_var_uses(&arm.body, name))
            });
            count_var_uses(scrutinee, name).saturating_add(arm_uses)
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => fields
            .iter()
            .map(|(_, value)| count_var_uses(value, name))
            .fold(0, usize::saturating_add),
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => elements
            .iter()
            .map(|element| count_var_uses(element, name))
            .fold(0, usize::saturating_add),
        HirExprKind::ListSpreadLit { elements, .. } => elements
            .iter()
            .map(|element| count_var_uses(&element.value, name))
            .fold(0, usize::saturating_add),
        HirExprKind::MapLit { entries, .. } => entries
            .iter()
            .map(|(key, value)| {
                count_var_uses(key, name).saturating_add(count_var_uses(value, name))
            })
            .fold(0, usize::saturating_add),
        HirExprKind::Range { start, end } => {
            count_var_uses(start, name).saturating_add(count_var_uses(end, name))
        }
        HirExprKind::StructUpdate { base, updates, .. } => updates
            .iter()
            .map(|(_, value)| count_var_uses(value, name))
            .fold(count_var_uses(base, name), usize::saturating_add),
        HirExprKind::InterpolatedStr(parts) => parts
            .iter()
            .filter_map(|part| match part {
                HirStrPart::Expr(value) => Some(count_var_uses(value, name)),
                HirStrPart::Literal(_) => None,
            })
            .fold(0, usize::saturating_add),
        HirExprKind::Closure { captures, .. } => captures
            .iter()
            .filter(|capture| capture.name.as_str() == name)
            .count(),
        HirExprKind::None_
        | HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit => 0,
    }
}

fn expr_has_inline_barrier(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Closure { .. }
        | HirExprKind::Propagate(_)
        | HirExprKind::Await(_)
        | HirExprKind::MatchExpr { .. } => true,
        HirExprKind::Binary { lhs, rhs, .. } => {
            expr_has_inline_barrier(lhs) || expr_has_inline_barrier(rhs)
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand)
        | HirExprKind::IsCheck { value: operand, .. }
        | HirExprKind::TupleIndex {
            object: operand, ..
        } => expr_has_inline_barrier(operand),
        HirExprKind::Index { object, index } => {
            expr_has_inline_barrier(object) || expr_has_inline_barrier(index)
        }
        HirExprKind::Call { callee, args } => {
            expr_has_inline_barrier(callee)
                || args.iter().any(|arg| expr_has_inline_barrier(&arg.value))
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            expr_has_inline_barrier(receiver) || args.iter().any(expr_has_inline_barrier)
        }
        HirExprKind::AssociatedCall { args, .. } => args.iter().any(expr_has_inline_barrier),
        HirExprKind::IfExpr { cond, then, else_ } => {
            expr_has_inline_barrier(cond)
                || expr_has_inline_barrier(then)
                || expr_has_inline_barrier(else_)
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_has_inline_barrier(value)),
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => elements.iter().any(expr_has_inline_barrier),
        HirExprKind::ListSpreadLit { elements, .. } => elements
            .iter()
            .any(|element| expr_has_inline_barrier(&element.value)),
        HirExprKind::MapLit { entries, .. } => entries
            .iter()
            .any(|(key, value)| expr_has_inline_barrier(key) || expr_has_inline_barrier(value)),
        HirExprKind::Range { start, end } => {
            expr_has_inline_barrier(start) || expr_has_inline_barrier(end)
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            expr_has_inline_barrier(base)
                || updates
                    .iter()
                    .any(|(_, value)| expr_has_inline_barrier(value))
        }
        HirExprKind::InterpolatedStr(parts) => parts.iter().any(|part| match part {
            HirStrPart::Expr(value) => expr_has_inline_barrier(value),
            HirStrPart::Literal(_) => false,
        }),
        HirExprKind::None_
        | HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::Var(_) => false,
    }
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
        HirExprKind::AssociatedCall { args, .. } => {
            for arg in args {
                subst_var(arg, name, replacement);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            subst_var(cond, name, replacement);
            subst_var(then, name, replacement);
            subst_var(else_, name, replacement);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            subst_var(scrutinee, name, replacement);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    subst_var(guard, name, replacement);
                }
                subst_var(&mut arm.body, name, replacement);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                subst_var(e, name, replacement);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
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
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(|g| expr_calls_name(g, name))
                        || a.body.iter().any(|s| stmt_calls_name(s, name))
                })
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
        HirExprKind::MatchExpr { scrutinee, arms } => {
            expr_calls_name(scrutinee, name)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|g| expr_calls_name(g, name))
                        || expr_calls_name(&arm.body, name)
                })
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

#[cfg(test)]
mod tests {
    use super::*;
    use ori_diagnostics::Span;
    use ori_types::Ty;

    fn int_lit(value: i64) -> HirExpr {
        HirExpr {
            kind: HirExprKind::IntLit(value),
            ty: Ty::Int,
            span: Span::DUMMY,
        }
    }

    fn leaf_returning(expr: HirExpr, params: &[&str]) -> LeafFn {
        LeafFn {
            params: params.iter().map(|name| SmolStr::new(*name)).collect(),
            body: HirBlock {
                stmts: vec![HirStmt::Return(Some(expr), Span::DUMMY)],
                span: Span::DUMMY,
            },
        }
    }

    #[test]
    fn match_binding_scope_blocks_textual_substitution() {
        let match_expr = HirExpr {
            kind: HirExprKind::MatchExpr {
                scrutinee: Box::new(int_lit(7)),
                arms: vec![HirExprArm {
                    pattern: HirPattern::Binding(SmolStr::new("value"), Ty::Int),
                    guard: None,
                    body: HirExpr {
                        kind: HirExprKind::Var(SmolStr::new("value")),
                        ty: Ty::Int,
                        span: Span::DUMMY,
                    },
                    span: Span::DUMMY,
                }],
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let leaf = leaf_returning(match_expr, &["value"]);
        let args = [HirArg {
            label: None,
            value: int_lit(99),
            spread: false,
        }];

        assert!(
            try_inline_return_expr(&leaf, &args).is_none(),
            "a match binding can shadow a parameter and must block textual substitution"
        );
    }

    #[test]
    fn propagation_control_flow_blocks_inlining() {
        let propagated = HirExpr {
            kind: HirExprKind::Propagate(Box::new(HirExpr {
                kind: HirExprKind::Ok_(Box::new(int_lit(1))),
                ty: Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
                span: Span::DUMMY,
            })),
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let leaf = leaf_returning(propagated, &[]);

        assert!(
            try_inline_return_expr(&leaf, &[]).is_none(),
            "propagation must remain scoped to the callee"
        );
    }
}
