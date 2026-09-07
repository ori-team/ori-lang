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
                scalar_signature: is_inline_scalar(&f.return_ty)
                    && f.params
                        .iter()
                        .all(|param| is_inline_scalar(&param.ty) && !param.variadic)
                    && f.closure_captures.is_empty(),
                body: f.body.clone(),
            },
        );
    }
    if leaves.is_empty() {
        return;
    }
    let mut next_temporary = 0;
    for f in &mut module.funcs {
        reserve_temporaries(&mut f.body, &mut next_temporary);
    }
    for f in &mut module.funcs {
        materialize_in_block(&mut f.body, &leaves, &mut next_temporary);
        inline_in_block(&mut f.body, &leaves);
    }
}

struct LeafFn {
    params: Vec<SmolStr>,
    scalar_signature: bool,
    body: HirBlock,
}

fn is_inline_scalar(ty: &ori_types::Ty) -> bool {
    ty.is_numeric() || matches!(ty, ori_types::Ty::Bool)
}

fn nested_blocks(stmt: &mut HirStmt, visit: &mut impl FnMut(&mut HirBlock)) {
    match stmt {
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            visit(then);
            for (_, block) in else_ifs {
                visit(block);
            }
            if let Some(block) = else_ {
                visit(block);
            }
        }
        HirStmt::IfSome { then, else_, .. } => {
            visit(then);
            if let Some(block) = else_ {
                visit(block);
            }
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => visit(body),
        HirStmt::Match { arms, .. } => {
            for arm in arms {
                let mut block = HirBlock {
                    stmts: std::mem::take(&mut arm.body),
                    span: arm.span,
                };
                visit(&mut block);
                arm.body = block.stmts;
            }
        }
        _ => {}
    }
}

fn reserve_temporaries(block: &mut HirBlock, next: &mut usize) {
    for stmt in &mut block.stmts {
        if let HirStmt::Let { name, .. } = stmt {
            if let Some(index) = name
                .strip_prefix("$ori.inline.")
                .and_then(|index| index.parse::<usize>().ok())
            {
                *next = (*next).max(index + 1);
            }
        }
        nested_blocks(stmt, &mut |block| reserve_temporaries(block, next));
    }
}

fn materialize_in_block(block: &mut HirBlock, leaves: &HashMap<SmolStr, LeafFn>, next: &mut usize) {
    let mut stmts = Vec::with_capacity(block.stmts.len());
    for mut stmt in std::mem::take(&mut block.stmts) {
        nested_blocks(&mut stmt, &mut |block| {
            materialize_in_block(block, leaves, next)
        });
        let value = match &mut stmt {
            HirStmt::Let { value, .. } | HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => {
                Some(value)
            }
            _ => None,
        };
        if let Some(value) = value {
            if let Some((bindings, result)) = materialize_call(value, leaves, next) {
                stmts.extend(bindings);
                *value = result;
            }
        }
        stmts.push(stmt);
    }
    block.stmts = stmts;
}

fn materialize_call(
    expr: &HirExpr,
    leaves: &HashMap<SmolStr, LeafFn>,
    next: &mut usize,
) -> Option<(Vec<HirStmt>, HirExpr)> {
    let HirExprKind::Call { callee, args } = &expr.kind else {
        return None;
    };
    let HirExprKind::Var(name) = &callee.kind else {
        return None;
    };
    let leaf = leaves.get(name)?;
    if !leaf.scalar_signature
        || leaf.body.stmts.len() > MAX_INLINE_STMTS
        || args.len() != leaf.params.len()
        || !is_inline_scalar(&expr.ty)
        || args.iter().any(|arg| {
            arg.spread || !is_inline_scalar(&arg.value.ty) || expr_has_inline_barrier(&arg.value)
        })
        || try_inline_return_expr(leaf, args).is_some()
    {
        return None;
    }
    let mut result = leaf_body_to_return_expr(&leaf.body.stmts)?;
    if !is_inline_scalar(&result.ty) || expr_has_inline_barrier(&result) {
        return None;
    }
    let mut bindings = Vec::with_capacity(args.len());
    let mut replacements = Vec::with_capacity(args.len());
    for arg in args {
        let name = SmolStr::new(format!("$ori.inline.{}", *next));
        *next += 1;
        bindings.push(HirStmt::Let {
            name: name.clone(),
            ty: arg.value.ty.clone(),
            mutable: false,
            value: arg.value.clone(),
            span: arg.value.span,
        });
        replacements.push(HirArg {
            label: None,
            spread: false,
            value: HirExpr {
                kind: HirExprKind::Var(name),
                ty: arg.value.ty.clone(),
                span: arg.value.span,
            },
        });
    }
    subst_vars(&mut result, &leaf.params, &replacements);
    Some((bindings, result))
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
        | HirExprKind::SimdLit { elements, .. }
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

/// If leaf body is pure (single return or nested pure if-returns),
/// substitute params and return the inlined expression (PERF-INLINE-1).
fn try_inline_return_expr(leaf: &LeafFn, args: &[HirArg]) -> Option<HirExpr> {
    if leaf.body.stmts.is_empty() || leaf.body.stmts.len() > 8 {
        return None;
    }
    let ret = leaf_body_to_return_expr(&leaf.body.stmts)?;

    // Substitution has no temporary-binding representation yet. Restrict
    // arguments to expressions whose evaluation is repeatable and cannot
    // trap, allocate, call user code, or observe an evaluation-order change.
    if args.iter().any(|arg| !is_pure_inline_argument(&arg.value)) {
        return None;
    }

    // When all arguments are pure variables or constants, allow up to 4 reads per parameter.
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
        .any(|param| count_var_uses(&ret, param.as_str()) > max_reads)
    {
        return None;
    }
    if expr_has_inline_barrier(&ret) {
        return None;
    }
    // If the return expression contains function calls, textual substitution can
    // evaluate variable arguments across the call boundary, observing side effects
    // or reading mutated globals. Inlining is only valid when arguments are constants.
    if expr_has_any_call(&ret)
        && args.iter().any(|arg| {
            !matches!(
                arg.value.kind,
                HirExprKind::BoolLit(_)
                    | HirExprKind::IntLit(_)
                    | HirExprKind::FloatLit(_)
                    | HirExprKind::Unit
                    | HirExprKind::None_
            )
        })
    {
        return None;
    }

    let mut out = ret;
    subst_vars(&mut out, &leaf.params, args);
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
        HirExprKind::Binary { lhs, rhs, .. } => expr_has_any_call(lhs) || expr_has_any_call(rhs),
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
        HirExprKind::Range { start, end } => expr_has_any_call(start) || expr_has_any_call(end),
        HirExprKind::StructUpdate { base, updates, .. } => {
            expr_has_any_call(base) || updates.iter().any(|(_, value)| expr_has_any_call(value))
        }
        HirExprKind::InterpolatedStr(parts) => parts.iter().any(|part| match part {
            HirStrPart::Expr(value) => expr_has_any_call(value),
            HirStrPart::Literal(_) => false,
        }),
        _ => false,
    }
}

fn subst_vars(expr: &mut HirExpr, params: &[SmolStr], arguments: &[HirArg]) {
    match &mut expr.kind {
        HirExprKind::Var(name) => {
            if let Some(index) = params.iter().position(|param| param == name) {
                *expr = arguments[index].value.clone();
            }
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            subst_vars(lhs, params, arguments);
            subst_vars(rhs, params, arguments);
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
            subst_vars(operand, params, arguments);
        }
        HirExprKind::Index { object, index } => {
            subst_vars(object, params, arguments);
            subst_vars(index, params, arguments);
        }
        HirExprKind::Call { callee, args } => {
            subst_vars(callee, params, arguments);
            for a in args {
                subst_vars(&mut a.value, params, arguments);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            subst_vars(receiver, params, arguments);
            for a in args {
                subst_vars(a, params, arguments);
            }
        }
        HirExprKind::AssociatedCall { args, .. } => {
            for arg in args {
                subst_vars(arg, params, arguments);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            subst_vars(cond, params, arguments);
            subst_vars(then, params, arguments);
            subst_vars(else_, params, arguments);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            subst_vars(scrutinee, params, arguments);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    subst_vars(guard, params, arguments);
                }
                subst_vars(&mut arm.body, params, arguments);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                subst_vars(e, params, arguments);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                subst_vars(e, params, arguments);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                subst_vars(&mut el.value, params, arguments);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                subst_vars(k, params, arguments);
                subst_vars(v, params, arguments);
            }
        }
        HirExprKind::Range { start, end } => {
            subst_vars(start, params, arguments);
            subst_vars(end, params, arguments);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            subst_vars(base, params, arguments);
            for (_, e) in updates {
                subst_vars(e, params, arguments);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    subst_vars(e, params, arguments);
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
            scalar_signature: true,
            body: HirBlock {
                stmts: vec![HirStmt::Return(Some(expr), Span::DUMMY)],
                span: Span::DUMMY,
            },
        }
    }

    fn variable(name: &str) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Var(name.into()),
            ty: Ty::Int,
            span: Span::DUMMY,
        }
    }

    fn call(name: &str, values: Vec<HirExpr>) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(variable(name)),
                args: values
                    .into_iter()
                    .map(|value| HirArg {
                        label: None,
                        value,
                        spread: false,
                    })
                    .collect(),
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        }
    }

    #[test]
    fn materializes_all_arguments_once_in_source_order() {
        let leaves = HashMap::from([(
            SmolStr::new("pick"),
            leaf_returning(variable("second"), &["first", "second", "ignored"]),
        )]);
        for statement in [
            HirStmt::Return(
                Some(call(
                    "pick",
                    vec![
                        call("produce", vec![]),
                        variable("first"),
                        call("ignored", vec![]),
                    ],
                )),
                Span::DUMMY,
            ),
            HirStmt::Expr(call(
                "pick",
                vec![
                    call("produce", vec![]),
                    variable("first"),
                    call("ignored", vec![]),
                ],
            )),
            HirStmt::Let {
                name: "result".into(),
                ty: Ty::Int,
                mutable: false,
                value: call(
                    "pick",
                    vec![
                        call("produce", vec![]),
                        variable("first"),
                        call("ignored", vec![]),
                    ],
                ),
                span: Span::DUMMY,
            },
        ] {
            let mut block = HirBlock {
                stmts: vec![statement],
                span: Span::DUMMY,
            };
            materialize_in_block(&mut block, &leaves, &mut 0);
            assert_eq!(block.stmts.len(), 4, "every argument needs a binding");
            let names = block.stmts[..3]
                .iter()
                .map(|stmt| {
                    let HirStmt::Let { name, mutable, .. } = stmt else {
                        panic!("missing temporary")
                    };
                    assert!(!mutable);
                    name.clone()
                })
                .collect::<Vec<_>>();
            assert_ne!(names[0], names[1]);
            assert_ne!(names[1], names[2]);
            assert!(
                matches!(&block.stmts[0], HirStmt::Let { value, .. } if expr_calls_name(value, "produce"))
            );
            assert!(
                matches!(&block.stmts[1], HirStmt::Let { value: HirExpr { kind: HirExprKind::Var(name), .. }, .. } if name == "first")
            );
            assert!(
                matches!(&block.stmts[2], HirStmt::Let { value, .. } if expr_calls_name(value, "ignored"))
            );
            let result = match &block.stmts[3] {
                HirStmt::Return(Some(value), _)
                | HirStmt::Expr(value)
                | HirStmt::Let { value, .. } => value,
                _ => panic!("unexpected statement"),
            };
            assert!(matches!(&result.kind, HirExprKind::Var(name) if name == &names[1]));
        }
    }

    #[test]
    fn materialization_reuses_one_binding_for_repeated_parameter_reads() {
        let body = HirExpr {
            kind: HirExprKind::Binary {
                op: ori_ast::expr::BinaryOp::Add,
                lhs: Box::new(variable("value")),
                rhs: Box::new(variable("value")),
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let leaves = HashMap::from([(SmolStr::new("twice"), leaf_returning(body, &["value"]))]);
        let (bindings, result) = materialize_call(
            &call("twice", vec![call("produce", vec![])]),
            &leaves,
            &mut 0,
        )
        .unwrap();
        assert_eq!(bindings.len(), 1);
        let HirStmt::Let { name, value, .. } = &bindings[0] else {
            panic!("missing binding")
        };
        assert!(expr_calls_name(value, "produce"));
        assert_eq!(count_var_uses(&result, name), 2);
        assert!(!expr_has_any_call(&result));
    }

    #[test]
    fn materialization_keeps_nested_calls_conditions_and_assignments_in_place() {
        let leaves = HashMap::from([(
            SmolStr::new("pick"),
            leaf_returning(variable("value"), &["value"]),
        )]);
        let invocation = call("pick", vec![call("produce", vec![])]);
        let mut block = HirBlock {
            stmts: vec![
                HirStmt::Expr(call("outer", vec![invocation.clone()])),
                HirStmt::Assign {
                    lvalue: HirLValue::Index {
                        base: Box::new(HirLValue::Var("values".into())),
                        index: Box::new(call("index", vec![])),
                    },
                    value: invocation.clone(),
                    span: Span::DUMMY,
                },
                HirStmt::While {
                    cond: invocation.clone(),
                    body: HirBlock {
                        stmts: vec![],
                        span: Span::DUMMY,
                    },
                    span: Span::DUMMY,
                },
                HirStmt::Expr(HirExpr {
                    kind: HirExprKind::IfExpr {
                        cond: Box::new(variable("condition")),
                        then: Box::new(invocation.clone()),
                        else_: Box::new(int_lit(0)),
                    },
                    ty: Ty::Int,
                    span: Span::DUMMY,
                }),
                HirStmt::Expr(HirExpr {
                    kind: HirExprKind::Binary {
                        op: ori_ast::expr::BinaryOp::And,
                        lhs: Box::new(variable("condition")),
                        rhs: Box::new(invocation),
                    },
                    ty: Ty::Bool,
                    span: Span::DUMMY,
                }),
            ],
            span: Span::DUMMY,
        };
        let before = format!("{block:?}");
        materialize_in_block(&mut block, &leaves, &mut 0);
        assert_eq!(format!("{block:?}"), before);
    }

    #[test]
    fn temporary_namespace_is_distinct_and_reserved_across_nested_blocks() {
        let leaves = HashMap::from([(
            SmolStr::new("pick"),
            leaf_returning(variable("value"), &["value"]),
        )]);
        let mut block = HirBlock {
            stmts: vec![
                HirStmt::Loop {
                    body: HirBlock {
                        stmts: vec![HirStmt::Let {
                            name: "$ori.inline.41".into(),
                            ty: Ty::Int,
                            mutable: false,
                            value: int_lit(1),
                            span: Span::DUMMY,
                        }],
                        span: Span::DUMMY,
                    },
                    span: Span::DUMMY,
                },
                HirStmt::Return(
                    Some(call(
                        "pick",
                        vec![call("produce", vec![variable("_ori_inline_42")])],
                    )),
                    Span::DUMMY,
                ),
            ],
            span: Span::DUMMY,
        };
        let mut next = 0;
        reserve_temporaries(&mut block, &mut next);
        assert_eq!(next, 42);
        materialize_in_block(&mut block, &leaves, &mut next);
        assert!(matches!(&block.stmts[1], HirStmt::Let { name, .. } if name == "$ori.inline.42"));
        assert_eq!(next, 43);
    }

    #[test]
    fn materialization_rejects_managed_signatures_and_arguments() {
        let mut leaf = leaf_returning(variable("value"), &["value"]);
        leaf.scalar_signature = false;
        let mut leaves = HashMap::from([(SmolStr::new("pick"), leaf)]);
        let invocation = call("pick", vec![call("produce", vec![])]);
        assert!(materialize_call(&invocation, &leaves, &mut 0).is_none());
        leaves.get_mut("pick").unwrap().scalar_signature = true;
        let mut managed = call("produce", vec![]);
        managed.ty = Ty::String;
        assert!(materialize_call(&call("pick", vec![managed]), &leaves, &mut 0).is_none());
    }

    #[test]
    fn materialized_ignored_trap_survives_dce() {
        let leaves = HashMap::from([(
            SmolStr::new("ignore"),
            leaf_returning(int_lit(1), &["value"]),
        )]);
        let trap = HirExpr {
            kind: HirExprKind::Binary {
                op: ori_ast::expr::BinaryOp::Div,
                lhs: Box::new(int_lit(1)),
                rhs: Box::new(int_lit(0)),
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let mut block = HirBlock {
            stmts: vec![HirStmt::Expr(call("ignore", vec![trap]))],
            span: Span::DUMMY,
        };
        materialize_in_block(&mut block, &leaves, &mut 0);
        assert_eq!(block.stmts.len(), 2);
        let mut module = HirModule {
            namespace: "test".into(),
            structs: vec![],
            enums: vec![],
            traits: vec![],
            trait_impls: vec![],
            consts: vec![],
            externs: vec![],
            funcs: vec![HirFunc {
                def_id: ori_types::DefId(0),
                name: "main".into(),
                params: vec![],
                return_ty: Ty::Void,
                body: block,
                closure_captures: vec![],
                is_public: false,
                is_async: false,
                is_mut: false,
                is_inline: false,
                is_no_inline: false,
                c_export_name: None,
                span: Span::DUMMY,
            }],
        };
        super::super::dce::dce_module(&mut module);
        assert_eq!(module.funcs[0].body.stmts.len(), 2);
        assert!(matches!(
            &module.funcs[0].body.stmts[0],
            HirStmt::Let {
                value: HirExpr {
                    kind: HirExprKind::Binary {
                        op: ori_ast::expr::BinaryOp::Div,
                        ..
                    },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn substitution_does_not_capture_another_parameter_name() {
        let variable = |name: &str| HirExpr {
            kind: HirExprKind::Var(name.into()),
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let leaf = leaf_returning(variable("first"), &["first", "second"]);
        let args = [variable("second"), int_lit(9)].map(|value| HirArg {
            label: None,
            value,
            spread: false,
        });
        let result = try_inline_return_expr(&leaf, &args).unwrap();
        assert!(matches!(result.kind, HirExprKind::Var(name) if name == "second"));
    }

    #[test]
    fn compound_variable_argument_cannot_cross_a_call() {
        let variable = HirExpr {
            kind: HirExprKind::Var("current".into()),
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let call = HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(variable.clone()),
                args: vec![],
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        };
        let leaf = leaf_returning(call, &["value"]);
        let arg = HirArg {
            label: None,
            value: HirExpr {
                kind: HirExprKind::Unary {
                    op: ori_ast::expr::UnaryOp::Neg,
                    operand: Box::new(variable),
                },
                ty: Ty::Int,
                span: Span::DUMMY,
            },
            spread: false,
        };
        assert!(try_inline_return_expr(&leaf, &[arg]).is_none());
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
