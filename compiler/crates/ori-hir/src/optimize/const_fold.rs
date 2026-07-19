//! Constant folding for pure scalar HIR expressions.

use ori_ast::expr::{BinaryOp, UnaryOp};
use ori_types::Ty;

use crate::hir::*;

pub(super) fn fold_module(module: &mut HirModule) {
    for f in &mut module.funcs {
        fold_block(&mut f.body);
    }
    for c in &mut module.consts {
        fold_expr(&mut c.value);
    }
}

fn fold_block(block: &mut HirBlock) {
    for stmt in &mut block.stmts {
        fold_stmt(stmt);
    }
}

fn fold_stmt(stmt: &mut HirStmt) {
    match stmt {
        HirStmt::Let { value, .. } => fold_expr(value),
        HirStmt::Assign { value, .. } => fold_expr(value),
        HirStmt::Return(Some(e), _) | HirStmt::Expr(e) => fold_expr(e),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            fold_expr(cond);
            fold_block(then);
            for (c, b) in else_ifs {
                fold_expr(c);
                fold_block(b);
            }
            if let Some(b) = else_ {
                fold_block(b);
            }
        }
        HirStmt::While { cond, body, .. } => {
            fold_expr(cond);
            fold_block(body);
        }
        HirStmt::For { iterable, body, .. } => {
            fold_expr(iterable);
            fold_block(body);
        }
        HirStmt::Loop { body, .. } => fold_block(body),
        HirStmt::Repeat { count, body, .. } => {
            fold_expr(count);
            fold_block(body);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            fold_expr(scrutinee);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    fold_expr(guard);
                }
                for s in &mut arm.body {
                    fold_stmt(s);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            fold_expr(value);
            fold_block(then);
            if let Some(b) = else_ {
                fold_block(b);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            fold_expr(value);
            fold_block(body);
        }
        HirStmt::Using { value, .. } => fold_expr(value),
        HirStmt::Check { condition, .. } => fold_expr(condition),
    }
}

fn fold_expr(expr: &mut HirExpr) {
    match &mut expr.kind {
        HirExprKind::Binary { op, lhs, rhs } => {
            fold_expr(lhs);
            fold_expr(rhs);
            if let (HirExprKind::IntLit(a), HirExprKind::IntLit(b)) = (&lhs.kind, &rhs.kind) {
                if let Some(v) = fold_int_bin(*op, *a, *b) {
                    expr.kind = HirExprKind::IntLit(v);
                    expr.ty = Ty::Int;
                    return;
                }
            }
            if let (HirExprKind::BoolLit(a), HirExprKind::BoolLit(b)) = (&lhs.kind, &rhs.kind) {
                if let Some(v) = fold_bool_bin(*op, *a, *b) {
                    expr.kind = HirExprKind::BoolLit(v);
                    expr.ty = Ty::Bool;
                }
            }
        }
        HirExprKind::Unary { op, operand } => {
            fold_expr(operand);
            match (&op, &operand.kind) {
                (UnaryOp::Neg, HirExprKind::IntLit(n)) => {
                    expr.kind = HirExprKind::IntLit(n.wrapping_neg());
                    expr.ty = Ty::Int;
                }
                (UnaryOp::Not, HirExprKind::BoolLit(b)) => {
                    expr.kind = HirExprKind::BoolLit(!*b);
                    expr.ty = Ty::Bool;
                }
                _ => {}
            }
        }
        HirExprKind::Field { object, .. }
        | HirExprKind::TupleIndex { object, .. }
        | HirExprKind::Some_(object)
        | HirExprKind::Ok_(object)
        | HirExprKind::Err_(object)
        | HirExprKind::Propagate(object)
        | HirExprKind::Await(object) => fold_expr(object),
        HirExprKind::Index { object, index } => {
            fold_expr(object);
            fold_expr(index);
        }
        HirExprKind::Call { callee, args } => {
            fold_expr(callee);
            for a in args {
                fold_expr(&mut a.value);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            fold_expr(receiver);
            for a in args {
                fold_expr(a);
            }
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            fold_expr(scrutinee);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    fold_expr(guard);
                }
                fold_expr(&mut arm.body);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            fold_expr(cond);
            fold_expr(then);
            fold_expr(else_);
            if let HirExprKind::BoolLit(true) = cond.kind {
                *expr = then.as_ref().clone();
            } else if let HirExprKind::BoolLit(false) = cond.kind {
                *expr = else_.as_ref().clone();
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                fold_expr(e);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                fold_expr(e);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                fold_expr(&mut el.value);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                fold_expr(k);
                fold_expr(v);
            }
        }
        HirExprKind::Range { start, end } => {
            fold_expr(start);
            fold_expr(end);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            fold_expr(base);
            for (_, e) in updates {
                fold_expr(e);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    fold_expr(e);
                }
            }
        }
        HirExprKind::IsCheck { value, .. } => fold_expr(value),
        HirExprKind::Closure { .. }
        | HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::Var(_)
        | HirExprKind::None_ => {}
    }
}

fn fold_int_bin(op: BinaryOp, a: i64, b: i64) -> Option<i64> {
    use BinaryOp::*;
    Some(match op {
        Add => a.wrapping_add(b),
        Sub => a.wrapping_sub(b),
        Mul => a.wrapping_mul(b),
        Div if b != 0 => a / b,
        Rem if b != 0 => a % b,
        _ => return None,
    })
}

fn fold_bool_bin(op: BinaryOp, a: bool, b: bool) -> Option<bool> {
    use BinaryOp::*;
    Some(match op {
        And => a && b,
        Or => a || b,
        Eq => a == b,
        Ne => a != b,
        _ => return None,
    })
}
