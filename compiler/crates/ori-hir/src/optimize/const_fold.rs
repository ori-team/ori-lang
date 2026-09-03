//! Constant folding for pure scalar HIR expressions.

use ori_ast::expr::{BinaryOp, UnaryOp};
use ori_types::Ty;

use crate::hir::*;

/// Returns `true` when at least one expression was rewritten, so the pipeline
/// can detect its fixed point without re-serialising the module.
pub(super) fn fold_module(module: &mut HirModule) -> bool {
    let mut changed = false;
    for f in &mut module.funcs {
        fold_block(&mut f.body, &mut changed);
    }
    for c in &mut module.consts {
        fold_expr(&mut c.value, &mut changed);
    }
    changed
}

fn fold_block(block: &mut HirBlock, changed: &mut bool) {
    for stmt in &mut block.stmts {
        fold_stmt(stmt, changed);
    }
}

fn fold_stmt(stmt: &mut HirStmt, changed: &mut bool) {
    match stmt {
        HirStmt::Let { value, .. } => fold_expr(value, changed),
        HirStmt::Assign { value, .. } => fold_expr(value, changed),
        HirStmt::Return(Some(e), _) | HirStmt::Expr(e) => fold_expr(e, changed),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            fold_expr(cond, changed);
            fold_block(then, changed);
            for (c, b) in else_ifs {
                fold_expr(c, changed);
                fold_block(b, changed);
            }
            if let Some(b) = else_ {
                fold_block(b, changed);
            }
        }
        HirStmt::While { cond, body, .. } => {
            fold_expr(cond, changed);
            fold_block(body, changed);
        }
        HirStmt::For { iterable, body, .. } => {
            fold_expr(iterable, changed);
            fold_block(body, changed);
        }
        HirStmt::Loop { body, .. } => fold_block(body, changed),
        HirStmt::Repeat { count, body, .. } => {
            fold_expr(count, changed);
            fold_block(body, changed);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            fold_expr(scrutinee, changed);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    fold_expr(guard, changed);
                }
                for s in &mut arm.body {
                    fold_stmt(s, changed);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            fold_expr(value, changed);
            fold_block(then, changed);
            if let Some(b) = else_ {
                fold_block(b, changed);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            fold_expr(value, changed);
            fold_block(body, changed);
        }
        HirStmt::Using { value, .. } => fold_expr(value, changed),
        HirStmt::Check { condition, .. } => fold_expr(condition, changed),
    }
}

fn fold_expr(expr: &mut HirExpr, changed: &mut bool) {
    match &mut expr.kind {
        HirExprKind::Binary { op, lhs, rhs } => {
            fold_expr(lhs, changed);
            fold_expr(rhs, changed);
            if let (HirExprKind::IntLit(a), HirExprKind::IntLit(b)) = (&lhs.kind, &rhs.kind) {
                // The folded value keeps `expr.ty`: rewriting it to `Ty::Int`
                // would widen sized integers and desynchronise HIR from the
                // Cranelift slot the value is stored into.
                if let Some(v) = fold_int_bin(*op, *a, *b, &expr.ty) {
                    expr.kind = HirExprKind::IntLit(v);
                    *changed = true;
                    return;
                }
            }
            if let (HirExprKind::BoolLit(a), HirExprKind::BoolLit(b)) = (&lhs.kind, &rhs.kind) {
                if let Some(v) = fold_bool_bin(*op, *a, *b) {
                    expr.kind = HirExprKind::BoolLit(v);
                    expr.ty = Ty::Bool;
                    *changed = true;
                }
            }
        }
        HirExprKind::Unary { op, operand } => {
            fold_expr(operand, changed);
            match (&op, &operand.kind) {
                (UnaryOp::Neg, HirExprKind::IntLit(n)) => {
                    if let Some(v) = fold_int_neg(*n, &expr.ty) {
                        expr.kind = HirExprKind::IntLit(v);
                        *changed = true;
                    }
                }
                (UnaryOp::Not, HirExprKind::BoolLit(b)) => {
                    expr.kind = HirExprKind::BoolLit(!*b);
                    expr.ty = Ty::Bool;
                    *changed = true;
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
        | HirExprKind::Await(object) => fold_expr(object, changed),
        HirExprKind::Index { object, index } => {
            fold_expr(object, changed);
            fold_expr(index, changed);
        }
        HirExprKind::Call { callee, args } => {
            fold_expr(callee, changed);
            for a in args {
                fold_expr(&mut a.value, changed);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            fold_expr(receiver, changed);
            for a in args {
                fold_expr(a, changed);
            }
        }
        HirExprKind::AssociatedCall { args, .. } => {
            for arg in args {
                fold_expr(arg, changed);
            }
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            fold_expr(scrutinee, changed);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    fold_expr(guard, changed);
                }
                fold_expr(&mut arm.body, changed);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            fold_expr(cond, changed);
            fold_expr(then, changed);
            fold_expr(else_, changed);
            // Only collapse when the taken branch already carries the type the
            // surrounding code expects; otherwise the branch would need the
            // coercion the checker attached to the `if` itself.
            let taken = match cond.kind {
                HirExprKind::BoolLit(true) => Some(then.as_ref()),
                HirExprKind::BoolLit(false) => Some(else_.as_ref()),
                _ => None,
            };
            if let Some(branch) = taken.filter(|branch| branch.ty == expr.ty) {
                *expr = branch.clone();
                *changed = true;
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                fold_expr(e, changed);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                fold_expr(e, changed);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                fold_expr(&mut el.value, changed);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                fold_expr(k, changed);
                fold_expr(v, changed);
            }
        }
        HirExprKind::Range { start, end } => {
            fold_expr(start, changed);
            fold_expr(end, changed);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            fold_expr(base, changed);
            for (_, e) in updates {
                fold_expr(e, changed);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    fold_expr(e, changed);
                }
            }
        }
        HirExprKind::IsCheck { value, .. } => fold_expr(value, changed),
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

/// How the raw `i64` payload of an `IntLit` is interpreted for a scalar type.
///
/// HIR stores every integer literal in an `i64`, so folding has to know the
/// declared width and signedness to reproduce the wrapping the backend applies.
#[derive(Clone, Copy)]
struct IntDomain {
    bits: u32,
    signed: bool,
}

impl IntDomain {
    fn of(ty: &Ty) -> Option<Self> {
        let (bits, signed) = match ty {
            Ty::Int | Ty::Int64 => (64, true),
            Ty::Int8 => (8, true),
            Ty::Int16 => (16, true),
            Ty::Int32 => (32, true),
            Ty::U8 => (8, false),
            Ty::U16 => (16, false),
            Ty::U32 => (32, false),
            Ty::U64 => (64, false),
            _ => return None,
        };
        Some(Self { bits, signed })
    }

    fn mask(self) -> u128 {
        (1u128 << self.bits) - 1
    }

    /// Widen the stored payload into the mathematical value it denotes.
    fn decode(self, raw: i64) -> i128 {
        let truncated = (raw as u64 as u128) & self.mask();
        if self.signed {
            let shift = 128 - self.bits;
            ((truncated as i128) << shift) >> shift
        } else {
            truncated as i128
        }
    }

    /// Narrow a mathematical value back into the wrapped payload for this type.
    fn encode(self, value: i128) -> i64 {
        let wrapped = (value as u128) & self.mask();
        if self.signed {
            let shift = 128 - self.bits;
            (((wrapped as i128) << shift) >> shift) as i64
        } else {
            wrapped as u64 as i64
        }
    }

    fn min(self) -> i128 {
        if self.signed {
            -(1i128 << (self.bits - 1))
        } else {
            0
        }
    }
}

fn fold_int_bin(op: BinaryOp, a: i64, b: i64, ty: &Ty) -> Option<i64> {
    use BinaryOp::*;
    let domain = IntDomain::of(ty)?;
    let lhs = domain.decode(a);
    let rhs = domain.decode(b);
    let value = match op {
        Add => lhs.wrapping_add(rhs),
        Sub => lhs.wrapping_sub(rhs),
        Mul => lhs.wrapping_mul(rhs),
        // Division by zero and `MIN / -1` are trapping operations. Leaving them
        // in the program keeps the runtime guard responsible for reporting them
        // instead of silently inventing a value at compile time.
        Div | Rem if rhs == 0 => return None,
        Div | Rem if rhs == -1 && lhs == domain.min() => return None,
        Div => lhs / rhs,
        Rem => lhs % rhs,
        _ => return None,
    };
    Some(domain.encode(value))
}

fn fold_int_neg(operand: i64, ty: &Ty) -> Option<i64> {
    let domain = IntDomain::of(ty)?;
    Some(domain.encode(domain.decode(operand).wrapping_neg()))
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
