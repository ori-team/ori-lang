//! Pure-loop strength reduction (LANG-PERF-2-3).
//!
//! Conservative patterns only — no side effects inside the loop body.
//! Enabled at `OptLevel::Default` and `OptLevel::Aggressive`.

use ori_ast::expr::BinaryOp;
use ori_diagnostics::Span;
use ori_types::Ty;
use smol_str::SmolStr;

use crate::hir::*;

/// Returns `true` when at least one loop was rewritten, so the pipeline can
/// detect its fixed point without re-serialising the module.
pub(super) fn strength_reduce_module(module: &mut HirModule) -> bool {
    let mut changed = false;
    for f in &mut module.funcs {
        strength_reduce_block(&mut f.body, &mut changed);
    }
    changed
}

fn strength_reduce_block(block: &mut HirBlock, changed: &mut bool) {
    for stmt in &mut block.stmts {
        strength_reduce_stmt(stmt, changed);
    }
    *changed |= rewrite_pure_while_sums(&mut block.stmts);
}

fn strength_reduce_stmt(stmt: &mut HirStmt, changed: &mut bool) {
    match stmt {
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            strength_reduce_block(then, changed);
            for (_, b) in else_ifs {
                strength_reduce_block(b, changed);
            }
            if let Some(b) = else_ {
                strength_reduce_block(b, changed);
            }
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => strength_reduce_block(body, changed),
        HirStmt::IfSome { then, else_, .. } => {
            strength_reduce_block(then, changed);
            if let Some(b) = else_ {
                strength_reduce_block(b, changed);
            }
        }
        HirStmt::Match { arms, .. } => {
            for arm in arms {
                let mut nested = HirBlock {
                    stmts: std::mem::take(&mut arm.body),
                    span: arm.span,
                };
                strength_reduce_block(&mut nested, changed);
                arm.body = nested.stmts;
            }
        }
        _ => {}
    }
}

/// Rewrite sequences:
///   var s = 0; var i = 0; while i < n { s = s + i; i = i + 1 }
/// into s = n*(n-1)/2 (when n is a const int binding or literal).
///
/// And:
///   var s = 0; var i = 0; while i < n { var j = 0; while j < n { s = s + 1; j = j + 1 }; i = i + 1 }
/// into s = n*n.
fn rewrite_pure_while_sums(stmts: &mut [HirStmt]) -> bool {
    if stmts.len() < 3 {
        return false;
    }
    let mut changed = false;
    let mut i = 0;
    while i + 2 < stmts.len() {
        if try_rewrite_at(stmts, i) {
            changed = true;
            i += 3;
            continue;
        }
        i += 1;
    }
    changed
}

/// Largest loop bound for which `n * (n - 1)` and `n * n` still fit in `int`.
///
/// Above it the closed form would wrap differently from the accumulating loop,
/// so the rewrite is declined and the original loop is kept.
const MAX_CLOSED_FORM_BOUND: i64 = 3_037_000_499;

fn try_rewrite_at(stmts: &mut [HirStmt], i: usize) -> bool {
    let Some((s_name, s_span)) = match_let_zero(&stmts[i]) else {
        return false;
    };
    let Some((i_name, i_span)) = match_let_zero(&stmts[i + 1]) else {
        return false;
    };

    // Clone pattern data before mutating stmts[i+2] (avoids borrow conflicts).
    // The clone also becomes the `else` branch of the guarded rewrite, so the
    // original loop still runs whenever the closed form does not apply.
    let original_loop = stmts[i + 2].clone();
    let HirStmt::While {
        cond,
        body,
        span: while_span,
    } = &original_loop
    else {
        return false;
    };

    let Some(n_expr) = match_i_lt_n(cond, &i_name) else {
        return false;
    };
    // The closed form reads the bound several times, so it must be a pure
    // expression, and it must be a plain `int` because the replacement builds
    // `Ty::Int` arithmetic.
    if !is_repeatable_int_bound(&n_expr) {
        return false;
    }

    // Pattern A: body is [Assign s = s + i, Assign i = i + 1]
    if body.stmts.len() == 2
        && match_assign_add_var(&body.stmts[0], &s_name, &i_name)
        && match_assign_add_one(&body.stmts[1], &i_name)
    {
        let replacement = make_sum_closed_form(
            &s_name,
            &i_name,
            n_expr.clone(),
            *while_span,
            s_span,
            i_span,
            original_loop.clone(),
        );
        stmts[i + 2] = replacement;
        return true;
    }

    // Pattern B: nested count
    // body: [Let j=0, While j < n { s = s + 1; j = j + 1 }, Assign i = i + 1]
    if body.stmts.len() == 3 {
        if let Some((j_name, _)) = match_let_zero(&body.stmts[0]) {
            if let HirStmt::While {
                cond: inner_cond,
                body: inner_body,
                ..
            } = &body.stmts[1]
            {
                if match_i_lt_n(inner_cond, &j_name).is_some()
                    && inner_body.stmts.len() == 2
                    && match_assign_add_one(&inner_body.stmts[0], &s_name)
                    && match_assign_add_one(&inner_body.stmts[1], &j_name)
                    && match_assign_add_one(&body.stmts[2], &i_name)
                {
                    if let Some(n2) = match_i_lt_n(inner_cond, &j_name) {
                        if expr_same_value(&n_expr, &n2) {
                            let replacement = make_nested_closed_form(
                                &s_name,
                                &i_name,
                                n_expr.clone(),
                                *while_span,
                                s_span,
                                i_span,
                                original_loop.clone(),
                            );
                            stmts[i + 2] = replacement;
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

fn match_let_zero(stmt: &HirStmt) -> Option<(SmolStr, Span)> {
    match stmt {
        HirStmt::Let {
            name,
            ty: Ty::Int,
            value,
            mutable: true,
            span,
        } => {
            if matches!(value.kind, HirExprKind::IntLit(0)) {
                Some((name.clone(), *span))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The bound may be substituted into the closed form only when re-evaluating it
/// is free of side effects and yields the same value every time.
fn is_repeatable_int_bound(bound: &HirExpr) -> bool {
    bound.ty == Ty::Int && matches!(bound.kind, HirExprKind::Var(_) | HirExprKind::IntLit(_))
}

fn match_i_lt_n(cond: &HirExpr, i_name: &str) -> Option<HirExpr> {
    match &cond.kind {
        HirExprKind::Binary {
            op: BinaryOp::Lt,
            lhs,
            rhs,
        } => {
            if matches!(&lhs.kind, HirExprKind::Var(n) if n.as_str() == i_name) {
                return Some(rhs.as_ref().clone());
            }
            None
        }
        _ => None,
    }
}

fn match_assign_add_var(stmt: &HirStmt, s_name: &str, i_name: &str) -> bool {
    match stmt {
        HirStmt::Assign { lvalue, value, .. } => {
            matches!(lvalue, HirLValue::Var(n) if n.as_str() == s_name)
                && matches!(
                    &value.kind,
                    HirExprKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    } if matches!(&lhs.kind, HirExprKind::Var(n) if n.as_str() == s_name)
                        && matches!(&rhs.kind, HirExprKind::Var(n) if n.as_str() == i_name)
                )
        }
        _ => false,
    }
}

fn match_assign_add_one(stmt: &HirStmt, name: &str) -> bool {
    match stmt {
        HirStmt::Assign { lvalue, value, .. } => {
            matches!(lvalue, HirLValue::Var(n) if n.as_str() == name)
                && matches!(
                    &value.kind,
                    HirExprKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    } if matches!(&lhs.kind, HirExprKind::Var(n) if n.as_str() == name)
                        && matches!(&rhs.kind, HirExprKind::IntLit(1))
                )
        }
        _ => false,
    }
}

fn expr_same_value(a: &HirExpr, b: &HirExpr) -> bool {
    match (&a.kind, &b.kind) {
        (HirExprKind::IntLit(x), HirExprKind::IntLit(y)) => x == y,
        (HirExprKind::Var(x), HirExprKind::Var(y)) => x == y,
        _ => false,
    }
}

/// Replace pure sum-while with `if <guard> { s = n*(n-1)/2; i = n } else { loop }`.
#[allow(clippy::too_many_arguments)]
fn make_sum_closed_form(
    s_name: &str,
    i_name: &str,
    n: HirExpr,
    span: Span,
    s_span: Span,
    i_span: Span,
    fallback: HirStmt,
) -> HirStmt {
    let n1 = n.clone();
    let n2 = n.clone();
    let one = HirExpr {
        kind: HirExprKind::IntLit(1),
        ty: Ty::Int,
        span,
    };
    let n_minus_1 = HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::Sub,
            lhs: Box::new(n1),
            rhs: Box::new(one),
        },
        ty: Ty::Int,
        span,
    };
    let prod = HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(n2),
            rhs: Box::new(n_minus_1),
        },
        ty: Ty::Int,
        span,
    };
    let two = HirExpr {
        kind: HirExprKind::IntLit(2),
        ty: Ty::Int,
        span,
    };
    let closed = HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::Div,
            lhs: Box::new(prod),
            rhs: Box::new(two),
        },
        ty: Ty::Int,
        span,
    };
    closed_form_if(s_name, i_name, closed, n, span, s_span, i_span, fallback)
}

/// Replace pure nested count with `if <guard> { s = n*n; i = n } else { loop }`.
#[allow(clippy::too_many_arguments)]
fn make_nested_closed_form(
    s_name: &str,
    i_name: &str,
    n: HirExpr,
    span: Span,
    s_span: Span,
    i_span: Span,
    fallback: HirStmt,
) -> HirStmt {
    let n1 = n.clone();
    let n2 = n.clone();
    let closed = HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(n1),
            rhs: Box::new(n2),
        },
        ty: Ty::Int,
        span,
    };
    closed_form_if(s_name, i_name, closed, n, span, s_span, i_span, fallback)
}

/// Build `bound > 0 and bound <= MAX_CLOSED_FORM_BOUND`.
///
/// A non-positive bound means the loop never runs, so the accumulators must keep
/// the zeros their declarations already assigned; an oversized bound would make
/// the closed-form multiplication wrap differently from the loop. Both cases
/// fall through to the untouched `else` path.
fn closed_form_guard(bound: &HirExpr, span: Span) -> HirExpr {
    let int_lit = |value: i64| HirExpr {
        kind: HirExprKind::IntLit(value),
        ty: Ty::Int,
        span,
    };
    let compare = |op: BinaryOp, rhs: HirExpr| HirExpr {
        kind: HirExprKind::Binary {
            op,
            lhs: Box::new(bound.clone()),
            rhs: Box::new(rhs),
        },
        ty: Ty::Bool,
        span,
    };
    HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::And,
            lhs: Box::new(compare(BinaryOp::Gt, int_lit(0))),
            rhs: Box::new(compare(BinaryOp::Le, int_lit(MAX_CLOSED_FORM_BOUND))),
        },
        ty: Ty::Bool,
        span,
    }
}

#[allow(clippy::too_many_arguments)]
fn closed_form_if(
    s_name: &str,
    i_name: &str,
    s_value: HirExpr,
    i_value: HirExpr,
    span: Span,
    s_span: Span,
    i_span: Span,
    fallback: HirStmt,
) -> HirStmt {
    HirStmt::If {
        cond: closed_form_guard(&i_value, span),
        then: HirBlock {
            stmts: vec![
                HirStmt::Assign {
                    lvalue: HirLValue::Var(SmolStr::new(s_name)),
                    value: s_value,
                    span: s_span,
                },
                HirStmt::Assign {
                    lvalue: HirLValue::Var(SmolStr::new(i_name)),
                    value: i_value,
                    span: i_span,
                },
            ],
            span,
        },
        else_ifs: vec![],
        else_: Some(HirBlock {
            stmts: vec![fallback],
            span,
        }),
        span,
    }
}
