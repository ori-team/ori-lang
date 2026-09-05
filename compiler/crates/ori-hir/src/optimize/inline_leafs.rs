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
        if f.is_async || f.name.as_str() == "main" || f.is_no_inline {
            continue;
        }
        // Parameter contracts run at the call boundary. Substitution would
        // erase that boundary and silently skip the contract check.
        if f.params.iter().any(|param| param.contract.is_some()) {
            continue;
        }
        if !f.is_inline && f.body.stmts.len() > MAX_INLINE_STMTS {
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
    let mut temp_counter = 0;
    for f in &mut module.funcs {
        inline_in_block(&mut f.body, &leaves, &mut temp_counter);
    }
}

struct LeafFn {
    params: Vec<SmolStr>,
    body: HirBlock,
}

fn inline_in_block(
    block: &mut HirBlock,
    leaves: &HashMap<SmolStr, LeafFn>,
    temp_counter: &mut usize,
) {
    let mut new_stmts = Vec::with_capacity(block.stmts.len());
    for mut stmt in block.stmts.drain(..) {
        let mut pre = Vec::new();
        inline_in_stmt(&mut stmt, leaves, Some(&mut pre), temp_counter);
        new_stmts.extend(pre);
        new_stmts.push(stmt);
    }
    block.stmts = new_stmts;
}

fn inline_in_stmt(
    stmt: &mut HirStmt,
    leaves: &HashMap<SmolStr, LeafFn>,
    pre: Option<&mut Vec<HirStmt>>,
    temp_counter: &mut usize,
) {
    match stmt {
        HirStmt::Let { value, .. }
        | HirStmt::Assign { value, .. }
        | HirStmt::Expr(value)
        | HirStmt::Using { value, .. }
        | HirStmt::Check { condition: value, .. } => {
            inline_in_expr(value, leaves, pre, temp_counter);
        }
        HirStmt::Return(Some(e), _) => inline_in_expr(e, leaves, pre, temp_counter),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            inline_in_expr(cond, leaves, pre, temp_counter);
            inline_in_block(then, leaves, temp_counter);
            for (c, b) in else_ifs {
                inline_in_expr(c, leaves, None, temp_counter);
                inline_in_block(b, leaves, temp_counter);
            }
            if let Some(b) = else_ {
                inline_in_block(b, leaves, temp_counter);
            }
        }
        HirStmt::While { cond, body, .. } => {
            // Do not hoist temporaries outside loop condition: condition runs per iteration.
            inline_in_expr(cond, leaves, None, temp_counter);
            inline_in_block(body, leaves, temp_counter);
        }
        HirStmt::For { iterable, body, .. } => {
            inline_in_expr(iterable, leaves, pre, temp_counter);
            inline_in_block(body, leaves, temp_counter);
        }
        HirStmt::Loop { body, .. } => inline_in_block(body, leaves, temp_counter),
        HirStmt::Repeat { count, body, .. } => {
            inline_in_expr(count, leaves, pre, temp_counter);
            inline_in_block(body, leaves, temp_counter);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            inline_in_expr(scrutinee, leaves, pre, temp_counter);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    inline_in_expr(guard, leaves, None, temp_counter);
                }
                let mut new_arm_stmts = Vec::with_capacity(arm.body.len());
                for mut s in arm.body.drain(..) {
                    let mut arm_pre = Vec::new();
                    inline_in_stmt(&mut s, leaves, Some(&mut arm_pre), temp_counter);
                    new_arm_stmts.extend(arm_pre);
                    new_arm_stmts.push(s);
                }
                arm.body = new_arm_stmts;
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            inline_in_expr(value, leaves, pre, temp_counter);
            inline_in_block(then, leaves, temp_counter);
            if let Some(b) = else_ {
                inline_in_block(b, leaves, temp_counter);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            inline_in_expr(value, leaves, None, temp_counter);
            inline_in_block(body, leaves, temp_counter);
        }
    }
}

fn inline_in_expr(
    expr: &mut HirExpr,
    leaves: &HashMap<SmolStr, LeafFn>,
    pre: Option<&mut Vec<HirStmt>>,
    temp_counter: &mut usize,
) {
    // Recurse first
    match &mut expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            inline_in_expr(lhs, leaves, None, temp_counter);
            inline_in_expr(rhs, leaves, None, temp_counter);
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
        } => inline_in_expr(operand, leaves, None, temp_counter),
        HirExprKind::Index { object, index } => {
            inline_in_expr(object, leaves, None, temp_counter);
            inline_in_expr(index, leaves, None, temp_counter);
        }
        HirExprKind::Call { callee, args } => {
            for a in args.iter_mut() {
                inline_in_expr(&mut a.value, leaves, None, temp_counter);
            }
            inline_in_expr(callee, leaves, None, temp_counter);
            // Try inline: callee is Var(name) and leaf exists
            if let HirExprKind::Var(name) = &callee.kind {
                if let Some(leaf) = leaves.get(name) {
                    if args.len() == leaf.params.len() && args.iter().all(|a| !a.spread) {
                        if let Some(inlined) =
                            try_inline_return_expr(leaf, args, pre, temp_counter)
                        {
                            *expr = inlined;
                        }
                    }
                }
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            inline_in_expr(receiver, leaves, None, temp_counter);
            for a in args {
                inline_in_expr(a, leaves, None, temp_counter);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            inline_in_expr(cond, leaves, None, temp_counter);
            inline_in_expr(then, leaves, None, temp_counter);
            inline_in_expr(else_, leaves, None, temp_counter);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            inline_in_expr(scrutinee, leaves, None, temp_counter);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    inline_in_expr(guard, leaves, None, temp_counter);
                }
                inline_in_expr(&mut arm.body, leaves, None, temp_counter);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                inline_in_expr(e, leaves, None, temp_counter);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                inline_in_expr(e, leaves, None, temp_counter);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                inline_in_expr(&mut el.value, leaves, None, temp_counter);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                inline_in_expr(k, leaves, None, temp_counter);
                inline_in_expr(v, leaves, None, temp_counter);
            }
        }
        HirExprKind::Range { start, end } => {
            inline_in_expr(start, leaves, None, temp_counter);
            inline_in_expr(end, leaves, None, temp_counter);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            inline_in_expr(base, leaves, None, temp_counter);
            for (_, e) in updates {
                inline_in_expr(e, leaves, None, temp_counter);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    inline_in_expr(e, leaves, None, temp_counter);
                }
            }
        }
        _ => {}
    }
}

/// If leaf body is pure (single return or nested pure if-returns),
/// substitute params and return the inlined expression (PERF-INLINE-1).
fn try_inline_return_expr(
    leaf: &LeafFn,
    args: &[HirArg],
    mut pre: Option<&mut Vec<HirStmt>>,
    temp_counter: &mut usize,
) -> Option<HirExpr> {
    if leaf.body.stmts.is_empty() || leaf.body.stmts.len() > 8 {
        return None;
    }
    let ret = leaf_body_to_return_expr(&leaf.body.stmts)?;
    if expr_has_inline_barrier(&ret) {
        return None;
    }
    // A call boundary currently performs the retain/release bookkeeping for
    // runtime-managed values. Keep this pass scalar and value-types only.
    if args.iter().any(|arg| arg.value.ty.is_runtime_managed()) {
        return None;
    }

    let has_calls = expr_has_any_call(&ret);
    let all_args_pure = args.iter().all(|arg| is_pure_inline_argument(&arg.value));
    let has_var_args = args.iter().any(|arg| matches!(arg.value.kind, HirExprKind::Var(_)));

    // Fast path: direct textual substitution without temporary bindings.
    // Safe when:
    // 1. All arguments are pure (no side-effects, no allocation, no traps).
    // 2. Either the body has no calls, OR no arguments are variables (constants won't observe mutations).
    // 3. Number of parameter uses does not exceed max_reads.
    let can_subst_directly = all_args_pure && (!has_calls || !has_var_args);
    if can_subst_directly {
        let max_reads = if args.iter().all(|a| {
            matches!(
                a.value.kind,
                HirExprKind::Var(_)
                    | HirExprKind::StructLit { .. }
                    | HirExprKind::FloatLit(_)
                    | HirExprKind::IntLit(_)
                    | HirExprKind::BoolLit(_)
            )
        }) {
            4
        } else {
            1
        };
        if leaf
            .params
            .iter()
            .all(|param| count_var_uses(&ret, param.as_str()) <= max_reads)
        {
            let mut out = ret;
            for (param, arg) in leaf.params.iter().zip(args.iter()) {
                subst_var(&mut out, param.as_str(), &arg.value);
            }
            return Some(out);
        }
    }

    // Materialization path (OPT-INLINE-TEMP-1):
    // Requires a statement list (`pre`) into which we can hoist `HirStmt::Let`.
    let pre_stmts = pre.as_mut()?;

    // With temporaries, each parameter will be read as a variable, so allow up to 4 reads.
    if leaf
        .params
        .iter()
        .any(|param| count_var_uses(&ret, param.as_str()) > 4)
    {
        return None;
    }

    // Materialize arguments into temporary let-bindings before the inlined body
    let mut synthesized_args = Vec::with_capacity(args.len());
    for arg in args {
        // Trivial immutable constants do not need a temporary let-binding
        if matches!(
            arg.value.kind,
            HirExprKind::BoolLit(_)
                | HirExprKind::IntLit(_)
                | HirExprKind::FloatLit(_)
                | HirExprKind::Unit
                | HirExprKind::None_
        ) {
            synthesized_args.push(arg.value.clone());
        } else {
            let tmp_name = SmolStr::from(format!("__inlined_tmp_{}", *temp_counter));
            *temp_counter += 1;
            pre_stmts.push(HirStmt::Let {
                name: tmp_name.clone(),
                ty: arg.value.ty.clone(),
                mutable: false,
                value: arg.value.clone(),
                span: arg.value.span,
            });
            synthesized_args.push(HirExpr {
                kind: HirExprKind::Var(tmp_name),
                ty: arg.value.ty.clone(),
                span: arg.value.span,
            });
        }
    }

    let mut out = ret;
    for (param, arg_expr) in leaf.params.iter().zip(synthesized_args.iter()) {
        subst_var(&mut out, param.as_str(), arg_expr);
    }
    Some(out)
}

fn leaf_body_to_return_expr(stmts: &[HirStmt]) -> Option<HirExpr> {
    if stmts.is_empty() {
        return None;
    }
    if stmts.len() == 1 {
        let HirStmt::Return(Some(ret), _) = &stmts[0] else {
            return None;
        };
        return Some(ret.clone());
    }
    let HirStmt::If {
        cond,
        then,
        else_ifs,
        else_,
        span,
    } = &stmts[0]
    else {
        return None;
    };
    if !else_ifs.is_empty() || else_.is_some() {
        return None;
    }
    if then.stmts.len() != 1 {
        return None;
    }
    let HirStmt::Return(Some(then_ret), _) = &then.stmts[0] else {
        return None;
    };
    let else_ret = leaf_body_to_return_expr(&stmts[1..])?;
    Some(HirExpr {
        kind: HirExprKind::IfExpr {
            cond: Box::new(cond.clone()),
            then: Box::new(then_ret.clone()),
            else_: Box::new(else_ret),
        },
        ty: then_ret.ty.clone(),
        span: *span,
    })
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
        HirExprKind::Var(_) => !expr.ty.is_runtime_managed(),
        HirExprKind::StructLit { fields, .. } => {
            !expr.ty.is_runtime_managed()
                && fields.iter().all(|(_, val)| is_pure_inline_argument(val))
        }
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
        | HirExprKind::SimdLit { elements, .. }
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
        | HirExprKind::SimdLit { elements, .. }
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

fn expr_has_any_call(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Call { .. }
        | HirExprKind::MethodCall { .. }
        | HirExprKind::AssociatedCall { .. } => true,
        HirExprKind::Binary { lhs, rhs, .. } => {
            expr_has_any_call(lhs) || expr_has_any_call(rhs)
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
        } => expr_has_any_call(operand),
        HirExprKind::Index { object, index } => {
            expr_has_any_call(object) || expr_has_any_call(index)
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            expr_has_any_call(cond) || expr_has_any_call(then) || expr_has_any_call(else_)
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            fields.iter().any(|(_, value)| expr_has_any_call(value))
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => elements.iter().any(expr_has_any_call),
        HirExprKind::ListSpreadLit { elements, .. } => elements
            .iter()
            .any(|element| expr_has_any_call(&element.value)),
        HirExprKind::MapLit { entries, .. } => entries
            .iter()
            .any(|(key, value)| expr_has_any_call(key) || expr_has_any_call(value)),
        HirExprKind::Range { start, end } => {
            expr_has_any_call(start) || expr_has_any_call(end)
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            expr_has_any_call(base)
                || updates
                    .iter()
                    .any(|(_, value)| expr_has_any_call(value))
        }
        HirExprKind::InterpolatedStr(parts) => parts.iter().any(|part| match part {
            HirStrPart::Expr(value) => expr_has_any_call(value),
            HirStrPart::Literal(_) => false,
        }),
        _ => false,
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
        | HirExprKind::SimdLit { elements, .. }
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

        let mut counter = 0;
        assert!(
            try_inline_return_expr(&leaf, &args, None, &mut counter).is_none(),
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

        let mut counter = 0;
        assert!(
            try_inline_return_expr(&leaf, &[], None, &mut counter).is_none(),
            "propagation must remain scoped to the callee"
        );
    }
}
