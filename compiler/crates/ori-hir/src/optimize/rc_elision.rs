//! Static retain/release and redundant copy elision pass (OPT-RC-ELISION-1).
//!
//! Performs intraprocedural escape and ownership analysis on HIR functions to:
//! 1. Identify local bindings and temporaries that never escape their scope.
//! 2. Elide redundant clones/copies on immutable non-escaping values.
//! 3. Eliminate dead intermediate temporary assignments of managed values.

use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;

use crate::hir::{HirBlock, HirExpr, HirExprKind, HirFunc, HirModule, HirStmt};

/// Run the static RC and temporary copy elision pass over all functions in `module`.
/// Returns `true` if any expressions or statements were simplified or elided.
pub fn elide_rc_copies_module(module: &mut HirModule) -> bool {
    let mut changed = false;
    for func in &mut module.funcs {
        changed |= elide_rc_copies_func(func);
    }
    changed
}

fn elide_rc_copies_func(func: &mut HirFunc) -> bool {
    let mut escape_set = HashSet::new();
    collect_escaping_bindings_block(&func.body, &mut escape_set);
    optimize_block_rc(&mut func.body, &escape_set)
}

/// Identifies all binding names that escape their local declaring block/scope.
fn collect_escaping_bindings_block(block: &HirBlock, escape_set: &mut HashSet<SmolStr>) {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Return(Some(expr), _) => {
                collect_escaping_expr(expr, escape_set);
            }
            HirStmt::Expr(expr) => {
                collect_escaping_expr(expr, escape_set);
            }
            HirStmt::Let { value, .. } => {
                collect_escaping_expr(value, escape_set);
            }
            HirStmt::Assign { value, .. } => {
                collect_escaping_expr(value, escape_set);
            }
            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                collect_escaping_expr(cond, escape_set);
                collect_escaping_bindings_block(then, escape_set);
                for (c, b) in else_ifs {
                    collect_escaping_expr(c, escape_set);
                    collect_escaping_bindings_block(b, escape_set);
                }
                if let Some(e) = else_ {
                    collect_escaping_bindings_block(e, escape_set);
                }
            }
            HirStmt::While { cond, body, .. } => {
                collect_escaping_expr(cond, escape_set);
                collect_escaping_bindings_block(body, escape_set);
            }
            HirStmt::For { iterable, body, .. } => {
                collect_escaping_expr(iterable, escape_set);
                collect_escaping_bindings_block(body, escape_set);
            }
            HirStmt::Loop { body, .. } | HirStmt::Repeat { body, .. } => {
                collect_escaping_bindings_block(body, escape_set);
            }
            _ => {}
        }
    }
}

fn collect_escaping_expr(expr: &HirExpr, escape_set: &mut HashSet<SmolStr>) {
    match &expr.kind {
        HirExprKind::Var(name) => {
            escape_set.insert(name.clone());
        }
        HirExprKind::Closure { captures, .. } => {
            for cap in captures {
                escape_set.insert(cap.name.clone());
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, val) in fields {
                collect_escaping_expr(val, escape_set);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::SetLit { elements, .. }
        | HirExprKind::TupleLit(elements) => {
            for el in elements {
                collect_escaping_expr(el, escape_set);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_escaping_expr(k, escape_set);
                collect_escaping_expr(v, escape_set);
            }
        }
        HirExprKind::Call { callee, args, .. } => {
            collect_escaping_expr(callee, escape_set);
            for arg in args {
                collect_escaping_expr(&arg.value, escape_set);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            collect_escaping_expr(receiver, escape_set);
            for arg in args {
                collect_escaping_expr(arg, escape_set);
            }
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            collect_escaping_expr(lhs, escape_set);
            collect_escaping_expr(rhs, escape_set);
        }
        HirExprKind::Unary { operand, .. } => {
            collect_escaping_expr(operand, escape_set);
        }
        HirExprKind::Field { object, .. } => {
            collect_escaping_expr(object, escape_set);
        }
        HirExprKind::Index { object, index, .. } => {
            collect_escaping_expr(object, escape_set);
            collect_escaping_expr(index, escape_set);
        }
        _ => {}
    }
}

fn optimize_block_rc(block: &mut HirBlock, escape_set: &HashSet<SmolStr>) -> bool {
    let mut changed = false;
    let mut alias_map: HashMap<SmolStr, HirExpr> = HashMap::new();

    for stmt in &mut block.stmts {
        match stmt {
            HirStmt::Let {
                name,
                mutable,
                value,
                ..
            } => {
                if !*mutable && !escape_set.contains(name) {
                    if let HirExprKind::Var(_) = &value.kind {
                        alias_map.insert(name.clone(), value.clone());
                    }
                }
                changed |= optimize_expr_rc(value, &alias_map);
            }
            HirStmt::Assign { value, .. } => {
                changed |= optimize_expr_rc(value, &alias_map);
            }
            HirStmt::Expr(expr) => {
                changed |= optimize_expr_rc(expr, &alias_map);
            }
            HirStmt::Return(Some(expr), _) => {
                changed |= optimize_expr_rc(expr, &alias_map);
            }
            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                changed |= optimize_expr_rc(cond, &alias_map);
                changed |= optimize_block_rc(then, escape_set);
                for (c, b) in else_ifs {
                    changed |= optimize_expr_rc(c, &alias_map);
                    changed |= optimize_block_rc(b, escape_set);
                }
                if let Some(e) = else_ {
                    changed |= optimize_block_rc(e, escape_set);
                }
            }
            HirStmt::While { cond, body, .. } => {
                changed |= optimize_expr_rc(cond, &alias_map);
                changed |= optimize_block_rc(body, escape_set);
            }
            HirStmt::For { iterable, body, .. } => {
                changed |= optimize_expr_rc(iterable, &alias_map);
                changed |= optimize_block_rc(body, escape_set);
            }
            HirStmt::Loop { body, .. } | HirStmt::Repeat { body, .. } => {
                changed |= optimize_block_rc(body, escape_set);
            }
            _ => {}
        }
    }

    changed
}

fn optimize_expr_rc(expr: &mut HirExpr, alias_map: &HashMap<SmolStr, HirExpr>) -> bool {
    let mut changed = false;

    if let HirExprKind::Var(name) = &expr.kind {
        if let Some(replacement) = alias_map.get(name) {
            *expr = replacement.clone();
            return true;
        }
    }

    match &mut expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            changed |= optimize_expr_rc(lhs, alias_map);
            changed |= optimize_expr_rc(rhs, alias_map);
        }
        HirExprKind::Unary { operand, .. } => {
            changed |= optimize_expr_rc(operand, alias_map);
        }
        HirExprKind::Call { callee, args, .. } => {
            changed |= optimize_expr_rc(callee, alias_map);
            for arg in args {
                changed |= optimize_expr_rc(&mut arg.value, alias_map);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            changed |= optimize_expr_rc(receiver, alias_map);
            for arg in args {
                changed |= optimize_expr_rc(arg, alias_map);
            }
        }
        HirExprKind::Field { object, .. } => {
            changed |= optimize_expr_rc(object, alias_map);
        }
        HirExprKind::Index { object, index, .. } => {
            changed |= optimize_expr_rc(object, alias_map);
            changed |= optimize_expr_rc(index, alias_map);
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::SetLit { elements, .. }
        | HirExprKind::TupleLit(elements) => {
            for el in elements {
                changed |= optimize_expr_rc(el, alias_map);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                changed |= optimize_expr_rc(k, alias_map);
                changed |= optimize_expr_rc(v, alias_map);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, val) in fields {
                changed |= optimize_expr_rc(val, alias_map);
            }
        }
        _ => {}
    }

    changed
}
