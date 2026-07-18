//! Dead binding elimination for pure, unused `const` bindings.

use std::collections::HashSet;

use ori_types::DefId;
use smol_str::SmolStr;

use crate::hir::*;

pub(super) fn dce_module(module: &mut HirModule) {
    // Struct literals whose type has field contracts trap at runtime when a
    // contract is violated — an observable effect DCE must not remove.
    let contract_structs: HashSet<DefId> = module
        .structs
        .iter()
        .filter(|s| s.fields.iter().any(|f| f.contract.is_some()))
        .map(|s| s.def_id)
        .collect();
    for f in &mut module.funcs {
        dce_block(&mut f.body, &contract_structs);
    }
}

fn dce_block(block: &mut HirBlock, contract_structs: &HashSet<DefId>) {
    for stmt in &mut block.stmts {
        dce_stmt_nested(stmt, contract_structs);
    }

    let mut used = HashSet::<SmolStr>::new();
    for stmt in &block.stmts {
        collect_stmt_uses(stmt, &mut used);
    }

    block.stmts.retain(|stmt| match stmt {
        HirStmt::Let {
            name,
            value,
            mutable,
            ..
        } => {
            if *mutable {
                return true;
            }
            if used.contains(name) {
                return true;
            }
            expr_may_effect(value, contract_structs)
        }
        _ => true,
    });
}

fn dce_stmt_nested(stmt: &mut HirStmt, contract_structs: &HashSet<DefId>) {
    match stmt {
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            dce_block(then, contract_structs);
            for (_, b) in else_ifs {
                dce_block(b, contract_structs);
            }
            if let Some(b) = else_ {
                dce_block(b, contract_structs);
            }
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => dce_block(body, contract_structs),
        HirStmt::IfSome { then, else_, .. } => {
            dce_block(then, contract_structs);
            if let Some(b) = else_ {
                dce_block(b, contract_structs);
            }
        }
        HirStmt::Match { arms, .. } => {
            for arm in arms {
                let mut nested = HirBlock {
                    stmts: std::mem::take(&mut arm.body),
                    span: arm.span,
                };
                dce_block(&mut nested, contract_structs);
                arm.body = nested.stmts;
            }
        }
        _ => {}
    }
}

fn collect_stmt_uses(stmt: &HirStmt, used: &mut HashSet<SmolStr>) {
    match stmt {
        HirStmt::Let { value, .. } => collect_expr_uses(value, used),
        HirStmt::Assign { lvalue, value, .. } => {
            collect_lvalue_uses(lvalue, used);
            collect_expr_uses(value, used);
        }
        HirStmt::Return(Some(e), _) | HirStmt::Expr(e) => collect_expr_uses(e, used),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            collect_expr_uses(cond, used);
            collect_block_uses(then, used);
            for (c, b) in else_ifs {
                collect_expr_uses(c, used);
                collect_block_uses(b, used);
            }
            if let Some(b) = else_ {
                collect_block_uses(b, used);
            }
        }
        HirStmt::While { cond, body, .. } => {
            collect_expr_uses(cond, used);
            collect_block_uses(body, used);
        }
        HirStmt::For { iterable, body, .. } => {
            collect_expr_uses(iterable, used);
            collect_block_uses(body, used);
        }
        HirStmt::Loop { body, .. } => collect_block_uses(body, used),
        HirStmt::Repeat { count, body, .. } => {
            collect_expr_uses(count, used);
            collect_block_uses(body, used);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_uses(scrutinee, used);
            for arm in arms {
                for s in &arm.body {
                    collect_stmt_uses(s, used);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            collect_expr_uses(value, used);
            collect_block_uses(then, used);
            if let Some(b) = else_ {
                collect_block_uses(b, used);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            collect_expr_uses(value, used);
            collect_block_uses(body, used);
        }
        HirStmt::Using { value, .. } => collect_expr_uses(value, used),
        HirStmt::Check { condition, .. } => collect_expr_uses(condition, used),
    }
}

fn collect_block_uses(block: &HirBlock, used: &mut HashSet<SmolStr>) {
    for s in &block.stmts {
        collect_stmt_uses(s, used);
    }
}

fn collect_lvalue_uses(lv: &HirLValue, used: &mut HashSet<SmolStr>) {
    match lv {
        HirLValue::Var(name) => {
            used.insert(name.clone());
        }
        HirLValue::Field { base, .. } => collect_lvalue_uses(base, used),
        HirLValue::Index { base, index } => {
            collect_lvalue_uses(base, used);
            collect_expr_uses(index, used);
        }
    }
}

fn collect_expr_uses(expr: &HirExpr, used: &mut HashSet<SmolStr>) {
    match &expr.kind {
        HirExprKind::Var(name) => {
            used.insert(name.clone());
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_uses(lhs, used);
            collect_expr_uses(rhs, used);
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::TupleIndex {
            object: operand, ..
        }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand)
        | HirExprKind::Propagate(operand)
        | HirExprKind::Await(operand)
        | HirExprKind::IsCheck { value: operand, .. } => collect_expr_uses(operand, used),
        HirExprKind::Index { object, index } => {
            collect_expr_uses(object, used);
            collect_expr_uses(index, used);
        }
        HirExprKind::Call { callee, args } => {
            collect_expr_uses(callee, used);
            for a in args {
                collect_expr_uses(&a.value, used);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_uses(receiver, used);
            for a in args {
                collect_expr_uses(a, used);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            collect_expr_uses(cond, used);
            collect_expr_uses(then, used);
            collect_expr_uses(else_, used);
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                collect_expr_uses(e, used);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => {
            for e in elements {
                collect_expr_uses(e, used);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for el in elements {
                collect_expr_uses(&el.value, used);
            }
        }
        HirExprKind::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_expr_uses(k, used);
                collect_expr_uses(v, used);
            }
        }
        HirExprKind::Range { start, end } => {
            collect_expr_uses(start, used);
            collect_expr_uses(end, used);
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            collect_expr_uses(base, used);
            for (_, e) in updates {
                collect_expr_uses(e, used);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for p in parts {
                if let HirStrPart::Expr(e) = p {
                    collect_expr_uses(e, used);
                }
            }
        }
        // A closure's captures are reads of the enclosing bindings even
        // though the body lives in a lifted function: without this, DCE
        // removed `const offset = 3` and closure creation failed with
        // "closure capture `offset` is not available in native codegen".
        HirExprKind::Closure { captures, .. } => {
            for capture in captures {
                used.insert(capture.name.clone());
            }
        }
        _ => {}
    }
}

fn expr_may_effect(expr: &HirExpr, contract_structs: &HashSet<DefId>) -> bool {
    match &expr.kind {
        HirExprKind::Call { .. }
        | HirExprKind::MethodCall { .. }
        | HirExprKind::Await(_)
        | HirExprKind::Propagate(_) => true,
        HirExprKind::Binary { lhs, rhs, .. } => {
            expr_may_effect(lhs, contract_structs) || expr_may_effect(rhs, contract_structs)
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::Some_(operand)
        | HirExprKind::Ok_(operand)
        | HirExprKind::Err_(operand) => expr_may_effect(operand, contract_structs),
        HirExprKind::Index { object, index } => {
            expr_may_effect(object, contract_structs) || expr_may_effect(index, contract_structs)
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            expr_may_effect(cond, contract_structs)
                || expr_may_effect(then, contract_structs)
                || expr_may_effect(else_, contract_structs)
        }
        HirExprKind::ListLit { elements, .. } | HirExprKind::TupleLit(elements) => elements
            .iter()
            .any(|e| expr_may_effect(e, contract_structs)),
        // Building a struct whose type carries field contracts runs those
        // contracts (and can trap): keep the binding even when unused.
        HirExprKind::StructLit { def_id, fields } => {
            contract_structs.contains(def_id)
                || fields
                    .iter()
                    .any(|(_, e)| expr_may_effect(e, contract_structs))
        }
        HirExprKind::StructUpdate {
            def_id,
            base,
            updates,
        } => {
            contract_structs.contains(def_id)
                || expr_may_effect(base, contract_structs)
                || updates
                    .iter()
                    .any(|(_, e)| expr_may_effect(e, contract_structs))
        }
        _ => false,
    }
}
