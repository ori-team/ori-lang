//! Dead binding elimination for pure, unused `const` bindings.

use std::collections::HashSet;

use ori_types::DefId;
use smol_str::SmolStr;

use crate::hir::*;

/// Returns `true` when at least one dead binding was removed, so the pipeline
/// can detect its fixed point without re-serialising the module.
pub(super) fn dce_module(module: &mut HirModule) -> bool {
    // Struct literals whose type has field contracts trap at runtime when a
    // contract is violated — an observable effect DCE must not remove.
    let mut effectful_structs: HashSet<DefId> = module
        .structs
        .iter()
        .filter(|s| s.fields.iter().any(|f| f.contract.is_some()))
        .map(|s| s.def_id)
        .collect();
    // Creating an otherwise-unused value with a custom destructor is also
    // observable: dropping the binding must still invoke user code.
    let destructor_traits = module
        .traits
        .iter()
        .filter(|trait_decl| trait_decl.name == "ori.core.Destructor")
        .map(|trait_decl| trait_decl.def_id)
        .collect::<HashSet<_>>();
    effectful_structs.extend(
        module
            .trait_impls
            .iter()
            .filter(|implementation| destructor_traits.contains(&implementation.trait_def_id))
            .map(|implementation| implementation.type_def_id),
    );
    let mut changed = false;
    for f in &mut module.funcs {
        dce_block(&mut f.body, &effectful_structs, &mut changed);
    }
    changed
}

fn dce_block(block: &mut HirBlock, contract_structs: &HashSet<DefId>, changed: &mut bool) {
    for stmt in &mut block.stmts {
        dce_stmt_nested(stmt, contract_structs, changed);
    }

    let mut used = HashSet::<SmolStr>::new();
    for stmt in &block.stmts {
        collect_stmt_uses(stmt, &mut used);
    }

    let before = block.stmts.len();
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
            !matches!(expr_effect(value, contract_structs), ExprEffect::Pure)
        }
        _ => true,
    });
    *changed |= block.stmts.len() != before;
}

fn dce_stmt_nested(stmt: &mut HirStmt, contract_structs: &HashSet<DefId>, changed: &mut bool) {
    match stmt {
        HirStmt::If {
            then,
            else_ifs,
            else_,
            ..
        } => {
            dce_block(then, contract_structs, changed);
            for (_, b) in else_ifs {
                dce_block(b, contract_structs, changed);
            }
            if let Some(b) = else_ {
                dce_block(b, contract_structs, changed);
            }
        }
        HirStmt::While { body, .. }
        | HirStmt::For { body, .. }
        | HirStmt::Loop { body, .. }
        | HirStmt::Repeat { body, .. }
        | HirStmt::WhileSome { body, .. } => dce_block(body, contract_structs, changed),
        HirStmt::IfSome { then, else_, .. } => {
            dce_block(then, contract_structs, changed);
            if let Some(b) = else_ {
                dce_block(b, contract_structs, changed);
            }
        }
        HirStmt::Match { arms, .. } => {
            for arm in arms {
                let mut nested = HirBlock {
                    stmts: std::mem::take(&mut arm.body),
                    span: arm.span,
                };
                dce_block(&mut nested, contract_structs, changed);
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
                if let Some(guard) = &arm.guard {
                    collect_expr_uses(guard, used);
                }
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
        // Associated calls have no receiver expression, but their arguments
        // are still evaluated at the call site. Missing this arm made DCE
        // delete bindings used only by `Type.method(value)`, leaving the
        // lowered call with a dangling variable reference.
        HirExprKind::AssociatedCall { args, .. } => {
            for arg in args {
                collect_expr_uses(arg, used);
            }
        }
        HirExprKind::IfExpr { cond, then, else_ } => {
            collect_expr_uses(cond, used);
            collect_expr_uses(then, used);
            collect_expr_uses(else_, used);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            collect_expr_uses(scrutinee, used);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_uses(guard, used);
                }
                collect_expr_uses(&arm.body, used);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, e) in fields {
                collect_expr_uses(e, used);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
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

/// Observable behavior of an expression when its value is unused.
///
/// `MayTrap` is deliberately separate from `Effectful`: allocation and bounds
/// checks must remain in the program, but they do not necessarily execute user
/// code. DCE may remove only `Pure` expressions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExprEffect {
    Pure,
    MayTrap,
    Effectful,
}

impl ExprEffect {
    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Effectful, _) | (_, Self::Effectful) => Self::Effectful,
            (Self::MayTrap, _) | (_, Self::MayTrap) => Self::MayTrap,
            (Self::Pure, Self::Pure) => Self::Pure,
        }
    }
}

fn combine_effects(effects: impl IntoIterator<Item = ExprEffect>) -> ExprEffect {
    effects
        .into_iter()
        .fold(ExprEffect::Pure, ExprEffect::combine)
}

fn expr_effect(expr: &HirExpr, contract_structs: &HashSet<DefId>) -> ExprEffect {
    use ori_ast::expr::BinaryOp;

    match &expr.kind {
        HirExprKind::Call { .. }
        | HirExprKind::MethodCall { .. }
        | HirExprKind::AssociatedCall { .. }
        | HirExprKind::Await(_)
        | HirExprKind::Propagate(_) => ExprEffect::Effectful,
        HirExprKind::Binary { op, lhs, rhs } => {
            let operands = combine_effects([
                expr_effect(lhs, contract_structs),
                expr_effect(rhs, contract_structs),
            ]);
            if matches!(
                op,
                BinaryOp::Div | BinaryOp::Rem | BinaryOp::Shl | BinaryOp::Shr
            ) && lhs.ty.is_integer()
            {
                operands.combine(ExprEffect::MayTrap)
            } else {
                operands
            }
        }
        HirExprKind::Unary { operand, .. }
        | HirExprKind::Field {
            object: operand, ..
        }
        | HirExprKind::TupleIndex {
            object: operand, ..
        }
        | HirExprKind::IsCheck { value: operand, .. } => expr_effect(operand, contract_structs),
        HirExprKind::Some_(operand) | HirExprKind::Ok_(operand) | HirExprKind::Err_(operand) => {
            expr_effect(operand, contract_structs).combine(ExprEffect::MayTrap)
        }
        // Indexing can execute a bounds guard for lists, strings, bytes and
        // slices. Arrays are retained too: their current backend path lacks a
        // dynamic guard, so dropping the expression would hide an invalid
        // memory access instead of fixing it.
        HirExprKind::Index { object, index } => combine_effects([
            expr_effect(object, contract_structs),
            expr_effect(index, contract_structs),
            ExprEffect::MayTrap,
        ]),
        HirExprKind::IfExpr { cond, then, else_ } => combine_effects([
            expr_effect(cond, contract_structs),
            expr_effect(then, contract_structs),
            expr_effect(else_, contract_structs),
        ]),
        HirExprKind::MatchExpr { scrutinee, arms } => combine_effects(
            std::iter::once(expr_effect(scrutinee, contract_structs)).chain(arms.iter().flat_map(
                |arm| {
                    arm.guard
                        .as_ref()
                        .into_iter()
                        .map(|expr| expr_effect(expr, contract_structs))
                        .chain(std::iter::once(expr_effect(&arm.body, contract_structs)))
                },
            )),
        ),
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::ArrayLit { elements, .. }
        | HirExprKind::SimdLit { elements, .. }
        | HirExprKind::TupleLit(elements)
        | HirExprKind::SetLit { elements, .. } => combine_effects(
            elements
                .iter()
                .map(|element| expr_effect(element, contract_structs))
                .chain(std::iter::once(ExprEffect::MayTrap)),
        ),
        HirExprKind::ListSpreadLit { elements, .. } => combine_effects(
            elements
                .iter()
                .map(|element| expr_effect(&element.value, contract_structs))
                .chain(std::iter::once(ExprEffect::MayTrap)),
        ),
        // Building a map or range allocates runtime storage even when all
        // operands are literals, so allocation failure remains observable.
        HirExprKind::MapLit { entries, .. } => combine_effects(
            entries
                .iter()
                .flat_map(|(key, value)| [key, value])
                .map(|expr| expr_effect(expr, contract_structs))
                .chain(std::iter::once(ExprEffect::MayTrap)),
        ),
        HirExprKind::Range { start, end } => combine_effects([
            expr_effect(start, contract_structs),
            expr_effect(end, contract_structs),
            ExprEffect::MayTrap,
        ]),
        // Building a struct whose type carries field contracts runs those
        // contracts. A custom destructor is also user-visible when the unused
        // value is dropped, so both cases remain fully effectful.
        HirExprKind::StructLit { def_id, fields }
        | HirExprKind::EnumVariant { def_id, fields, .. } => {
            let fields = fields
                .iter()
                .map(|(_, expr)| expr_effect(expr, contract_structs));
            let allocation = if def_id
                .as_ref()
                .is_some_and(|id| contract_structs.contains(id))
            {
                ExprEffect::Effectful
            } else {
                ExprEffect::MayTrap
            };
            combine_effects(fields.chain(std::iter::once(allocation)))
        }
        HirExprKind::StructUpdate {
            def_id,
            base,
            updates,
        } => {
            let values = updates
                .iter()
                .map(|(_, expr)| expr_effect(expr, contract_structs));
            let allocation = if def_id
                .as_ref()
                .is_some_and(|id| contract_structs.contains(id))
            {
                ExprEffect::Effectful
            } else {
                ExprEffect::MayTrap
            };
            combine_effects(
                std::iter::once(expr_effect(base, contract_structs))
                    .chain(values)
                    .chain(std::iter::once(allocation)),
            )
        }
        HirExprKind::InterpolatedStr(parts) => combine_effects(
            parts
                .iter()
                .filter_map(|part| match part {
                    HirStrPart::Expr(expr) => Some(expr_effect(expr, contract_structs)),
                    HirStrPart::Literal(_) => None,
                })
                .chain(std::iter::once(ExprEffect::MayTrap)),
        ),
        // Unlike `StrLit`, which points at a static module data block,
        // `BytesLit` allocates a managed payload in native codegen. Keep it
        // even when the value is unused so allocation failure and ownership
        // setup are not optimized away.
        HirExprKind::BytesLit(_) => ExprEffect::MayTrap,
        HirExprKind::Closure { .. } => ExprEffect::MayTrap,
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::Unit
        | HirExprKind::Var(_)
        | HirExprKind::None_ => ExprEffect::Pure,
    }
}
