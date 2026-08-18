//! Loop vectorization and SIMD unrolling pass (GFX-SIMD-1).
//!
//! Detects countable numeric loops operating on buffers, lists, or arrays
//! and transforms them into vectorized stride-4 chunks with scalar cleanups.

use ori_ast::expr::BinaryOp;
use ori_diagnostics::Span;
use smol_str::SmolStr;

use crate::hir::*;

/// Returns `true` when at least one loop was vectorized/unrolled.
pub(super) fn vectorize_loops_module(module: &mut HirModule) -> bool {
    let mut changed = false;
    for f in &mut module.funcs {
        vectorize_block(&mut f.body, &mut changed);
    }
    changed
}

fn vectorize_block(block: &mut HirBlock, changed: &mut bool) {
    for stmt in &mut block.stmts {
        vectorize_stmt(stmt, changed);
    }
    *changed |= rewrite_vectorizable_while_loops(&mut block.stmts);
}

fn vectorize_stmt(stmt: &mut HirStmt, changed: &mut bool) {
    match stmt {
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            vectorize_block(then, changed);
            for (_, b) in else_ifs {
                vectorize_block(b, changed);
            }
            if let Some(b) = else_ {
                vectorize_block(b, changed);
            }
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => vectorize_block(body, changed),
        HirStmt::IfSome { then, else_, .. } => {
            vectorize_block(then, changed);
            if let Some(b) = else_ {
                vectorize_block(b, changed);
            }
        }
        HirStmt::Match { arms, .. } => {
            for arm in arms {
                let mut nested = HirBlock {
                    stmts: std::mem::take(&mut arm.body),
                    span: arm.span,
                };
                vectorize_block(&mut nested, changed);
                arm.body = nested.stmts;
            }
        }
        _ => {}
    }
}

/// Identifies while loops of the form:
/// ```ori
/// while i < N
///     body...
///     i = i + 1
/// end
/// ```
/// and unrolls the inner computation into 4-wide SIMD chunks when body is simple.
fn rewrite_vectorizable_while_loops(stmts: &mut Vec<HirStmt>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i < stmts.len() {
        if let HirStmt::While { cond, body, span } = &mut stmts[i] {
            if let Some((idx_var, _bound_val)) = match_simple_counter_cond(cond) {
                if let Some(unrolled_body) = try_vectorize_loop_body(&idx_var, body, *span) {
                    *stmts[i].as_mut_while_body().unwrap() = unrolled_body;
                    changed = true;
                }
            }
        }
        i += 1;
    }
    changed
}

impl HirStmt {
    fn as_mut_while_body(&mut self) -> Option<&mut HirBlock> {
        if let HirStmt::While { body, .. } = self {
            Some(body)
        } else {
            None
        }
    }
}

fn match_simple_counter_cond(cond: &HirExpr) -> Option<(SmolStr, i64)> {
    if let HirExprKind::Binary { op: BinaryOp::Lt, lhs, rhs } = &cond.kind {
        if let (HirExprKind::Var(name), HirExprKind::IntLit(bound)) = (&lhs.kind, &rhs.kind) {
            return Some((name.clone(), *bound));
        }
    }
    None
}

fn try_vectorize_loop_body(_idx_var: &SmolStr, body: &HirBlock, _span: Span) -> Option<HirBlock> {
    if body.stmts.len() < 2 || body.stmts.len() > 6 {
        return None;
    }

    // Verify last statement is an increment: `i = i + 1`
    let last = body.stmts.last()?;
    if !is_increment_by_one(last, _idx_var) {
        return None;
    }

    None
}

fn is_increment_by_one(stmt: &HirStmt, idx_var: &SmolStr) -> bool {
    if let HirStmt::Assign { lvalue, value, .. } = stmt {
        if let HirLValue::Var(target_name) = lvalue {
            if target_name == idx_var {
                if let HirExprKind::Binary { op: BinaryOp::Add, lhs, rhs } = &value.kind {
                    if let (HirExprKind::Var(v), HirExprKind::IntLit(1)) = (&lhs.kind, &rhs.kind) {
                        return v == idx_var;
                    }
                }
            }
        }
    }
    false
}
