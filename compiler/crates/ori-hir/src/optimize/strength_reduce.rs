//! Pure-loop strength reduction (LANG-PERF-2-3).
//!
//! Conservative patterns only — no side effects inside the loop body.
//! Enabled at `OptLevel::Default` and `OptLevel::Aggressive`.

use ori_ast::expr::BinaryOp;
use ori_diagnostics::Span;
use ori_types::Ty;
use smol_str::SmolStr;

use crate::hir::*;

pub(super) fn strength_reduce_module(module: &mut HirModule) {
    for f in &mut module.funcs {
        strength_reduce_block(&mut f.body);
    }
}

fn strength_reduce_block(block: &mut HirBlock) {
    for stmt in &mut block.stmts {
        strength_reduce_stmt(stmt);
    }
    rewrite_pure_while_sums(&mut block.stmts);
}

fn strength_reduce_stmt(stmt: &mut HirStmt) {
    match stmt {
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            strength_reduce_block(then);
            for (_, b) in else_ifs {
                strength_reduce_block(b);
            }
            if let Some(b) = else_ {
                strength_reduce_block(b);
            }
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => strength_reduce_block(body),
        HirStmt::IfSome { then, else_, .. } => {
            strength_reduce_block(then);
            if let Some(b) = else_ {
                strength_reduce_block(b);
            }
        }
        HirStmt::Match { arms, .. } => {
            for arm in arms {
                let mut nested = HirBlock {
                    stmts: std::mem::take(&mut arm.body),
                    span: arm.span,
                };
                strength_reduce_block(&mut nested);
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
fn rewrite_pure_while_sums(stmts: &mut Vec<HirStmt>) {
    if stmts.len() < 3 {
        return;
    }
    let mut i = 0;
    while i + 2 < stmts.len() {
        if try_rewrite_at(stmts, i) {
            i += 3;
            continue;
        }
        i += 1;
    }
}

fn try_rewrite_at(stmts: &mut Vec<HirStmt>, i: usize) -> bool {
    let Some((s_name, s_span)) = match_let_zero(&stmts[i]) else {
        return false;
    };
    let Some((i_name, i_span)) = match_let_zero(&stmts[i + 1]) else {
        return false;
    };

    // Clone pattern data before mutating stmts[i+2] (avoids borrow conflicts).
    let HirStmt::While {
        cond,
        body,
        span: while_span,
    } = &stmts[i + 2]
    else {
        return false;
    };

    let Some(n_expr) = match_i_lt_n(cond, &i_name) else {
        return false;
    };

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
            value,
            mutable: true,
            span,
            ..
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

/// Replace pure sum-while with `if true { s = n*(n-1)/2; i = n }`.
fn make_sum_closed_form(
    s_name: &str,
    i_name: &str,
    n: HirExpr,
    span: Span,
    s_span: Span,
    i_span: Span,
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
    closed_form_if(s_name, i_name, closed, n, span, s_span, i_span)
}

/// Replace pure nested count with `if true { s = n*n; i = n }`.
fn make_nested_closed_form(
    s_name: &str,
    i_name: &str,
    n: HirExpr,
    span: Span,
    s_span: Span,
    i_span: Span,
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
    closed_form_if(s_name, i_name, closed, n, span, s_span, i_span)
}

fn closed_form_if(
    s_name: &str,
    i_name: &str,
    s_value: HirExpr,
    i_value: HirExpr,
    span: Span,
    s_span: Span,
    i_span: Span,
) -> HirStmt {
    HirStmt::If {
        cond: HirExpr {
            kind: HirExprKind::BoolLit(true),
            ty: Ty::Bool,
            span,
        },
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
        else_: None,
        span,
    }
}
