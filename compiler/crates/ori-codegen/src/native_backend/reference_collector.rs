//! Per-function reference discovery for native code generation.
//!
//! Cranelift function references belong to one function. Importing every
//! declared function into every body makes large modules quadratic even when
//! each body calls only one or two functions. This traversal records the user
//! functions and closure wrappers a body can name; runtime imports remain
//! available separately because several specialized lowering paths select
//! them from types rather than from an explicit HIR call.

use std::collections::HashSet;

use ori_ast::expr::BinaryOp;
use ori_hir::hir::*;
use ori_types::Ty;
use smol_str::SmolStr;

#[derive(Debug, Default)]
pub(super) struct FunctionReferences {
    pub(super) functions: HashSet<SmolStr>,
    pub(super) wrappers: HashSet<SmolStr>,
    pub(super) needs_trait_implementations: bool,
    pub(super) needs_all_runtime_symbols: bool,
}

impl FunctionReferences {
    fn collect_expr(&mut self, expr: &HirExpr) {
        if !is_runtime_scalar_type(&expr.ty) {
            self.needs_all_runtime_symbols = true;
        }

        match &expr.kind {
            HirExprKind::BoolLit(_)
            | HirExprKind::IntLit(_)
            | HirExprKind::FloatLit(_)
            | HirExprKind::StrLit(_)
            | HirExprKind::BytesLit(_)
            | HirExprKind::Unit
            | HirExprKind::None_ => {}
            HirExprKind::Var(name) => {
                if matches!(expr.ty, Ty::Func { .. }) {
                    self.functions.insert(name.clone());
                    self.wrappers.insert(name.clone());
                }
            }
            HirExprKind::Call { callee, args } => {
                if let HirExprKind::Var(name) = &callee.kind {
                    self.functions.insert(name.clone());
                } else {
                    self.collect_expr(callee);
                }
                for arg in args {
                    self.collect_expr(&arg.value);
                }
            }
            HirExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.functions.insert(method.clone());
                self.needs_trait_implementations = true;
                self.needs_all_runtime_symbols = true;
                self.collect_expr(receiver);
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            HirExprKind::AssociatedCall { method, args, .. } => {
                self.functions.insert(method.clone());
                self.needs_trait_implementations = true;
                self.needs_all_runtime_symbols = true;
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                // Integer `/` and `%` lower to a guarded division that calls
                // the abort helpers on a zero divisor or on `MIN / -1`.
                if matches!(op, BinaryOp::Div | BinaryOp::Rem) && lhs.ty.is_integer() {
                    self.functions
                        .insert(SmolStr::new("ori_abort_division_by_zero"));
                    self.functions
                        .insert(SmolStr::new("ori_abort_division_overflow"));
                }
                self.collect_expr(lhs);
                self.collect_expr(rhs);
            }
            HirExprKind::Unary { operand, .. }
            | HirExprKind::Field {
                object: operand, ..
            }
            | HirExprKind::TupleIndex {
                object: operand, ..
            }
            | HirExprKind::Propagate(operand)
            | HirExprKind::Await(operand)
            | HirExprKind::Some_(operand)
            | HirExprKind::Ok_(operand)
            | HirExprKind::Err_(operand)
            | HirExprKind::IsCheck { value: operand, .. } => self.collect_expr(operand),
            HirExprKind::Index { object, index } => {
                self.collect_expr(object);
                self.collect_expr(index);
            }
            HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
                for (_, value) in fields {
                    self.collect_expr(value);
                }
            }
            HirExprKind::ListLit { elements, .. }
            | HirExprKind::ArrayLit { elements, .. }
            | HirExprKind::SimdLit { elements, .. }
            | HirExprKind::TupleLit(elements)
            | HirExprKind::SetLit { elements, .. } => {
                for element in elements {
                    self.collect_expr(element);
                }
            }
            HirExprKind::ListSpreadLit { elements, .. } => {
                for element in elements {
                    self.collect_expr(&element.value);
                }
            }
            HirExprKind::IfExpr { cond, then, else_ } => {
                self.collect_expr(cond);
                self.collect_expr(then);
                self.collect_expr(else_);
            }
            HirExprKind::MatchExpr { scrutinee, arms } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(guard);
                    }
                    self.collect_expr(&arm.body);
                }
            }
            HirExprKind::Range { start, end } => {
                self.collect_expr(start);
                self.collect_expr(end);
            }
            HirExprKind::MapLit { entries, .. } => {
                for (key, value) in entries {
                    self.collect_expr(key);
                    self.collect_expr(value);
                }
            }
            HirExprKind::StructUpdate { base, updates, .. } => {
                self.collect_expr(base);
                for (_, value) in updates {
                    self.collect_expr(value);
                }
            }
            HirExprKind::Closure { func_name, .. } => {
                self.functions.insert(func_name.clone());
                self.wrappers.insert(func_name.clone());
                self.needs_all_runtime_symbols = true;
            }
            HirExprKind::InterpolatedStr(parts) => {
                for part in parts {
                    if let HirStrPart::Expr(value) = part {
                        self.collect_expr(value);
                    }
                }
            }
        }
    }

    fn collect_lvalue(&mut self, lvalue: &HirLValue) {
        match lvalue {
            HirLValue::Var(_) => {}
            HirLValue::Field { base, .. } => {
                self.needs_all_runtime_symbols = true;
                self.collect_lvalue(base);
            }
            HirLValue::Index { base, index } => {
                self.needs_all_runtime_symbols = true;
                self.collect_lvalue(base);
                self.collect_expr(index);
            }
        }
    }

    fn collect_block(&mut self, block: &HirBlock) {
        for statement in &block.stmts {
            self.collect_stmt(statement);
        }
    }

    fn collect_stmt(&mut self, statement: &HirStmt) {
        match statement {
            HirStmt::Let { value, .. } | HirStmt::Expr(value) => self.collect_expr(value),
            HirStmt::Using { value, .. } => {
                self.needs_all_runtime_symbols = true;
                self.collect_expr(value);
            }
            HirStmt::Assign { lvalue, value, .. } => {
                self.collect_lvalue(lvalue);
                self.collect_expr(value);
            }
            HirStmt::Return(Some(value), _) => self.collect_expr(value),
            HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
            HirStmt::If {
                cond,
                then,
                else_ifs,
                else_,
                ..
            } => {
                self.collect_expr(cond);
                self.collect_block(then);
                for (condition, block) in else_ifs {
                    self.collect_expr(condition);
                    self.collect_block(block);
                }
                if let Some(block) = else_ {
                    self.collect_block(block);
                }
            }
            HirStmt::While { cond, body, .. } => {
                self.collect_expr(cond);
                self.collect_block(body);
            }
            HirStmt::For { iterable, body, .. } => {
                self.needs_trait_implementations = true;
                self.needs_all_runtime_symbols = true;
                self.collect_expr(iterable);
                self.collect_block(body);
            }
            HirStmt::Loop { body, .. } => self.collect_block(body),
            HirStmt::Repeat { count, body, .. } => {
                self.collect_expr(count);
                self.collect_block(body);
            }
            HirStmt::Match {
                scrutinee, arms, ..
            } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(guard);
                    }
                    for statement in &arm.body {
                        self.collect_stmt(statement);
                    }
                }
            }
            HirStmt::IfSome {
                value, then, else_, ..
            } => {
                self.collect_expr(value);
                self.collect_block(then);
                if let Some(block) = else_ {
                    self.collect_block(block);
                }
            }
            HirStmt::WhileSome { value, body, .. } => {
                self.collect_expr(value);
                self.collect_block(body);
            }
            HirStmt::Check { condition, .. } => {
                self.needs_all_runtime_symbols = true;
                self.collect_expr(condition);
            }
        }
    }
}

pub(super) fn collect_function_references(function: &HirFunc) -> FunctionReferences {
    let mut references = FunctionReferences::default();
    references.functions.insert(function.name.clone());
    references.needs_all_runtime_symbols = function.is_async
        || !is_runtime_scalar_type(&function.return_ty)
        || function
            .params
            .iter()
            .any(|parameter| !is_runtime_scalar_type(&parameter.ty));
    for parameter in &function.params {
        if let Some(default) = &parameter.default {
            references.collect_expr(default);
        }
        if let Some(contract) = &parameter.contract {
            references.needs_all_runtime_symbols = true;
            references.collect_expr(contract);
        }
    }
    references.collect_block(&function.body);
    references
}

fn is_runtime_scalar_type(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Bool
            | Ty::Int
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Float
            | Ty::Float32
            | Ty::Float64
            | Ty::Void
            | Ty::Never
            | Ty::ConstInt(_, _)
    )
}
