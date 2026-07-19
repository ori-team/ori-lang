use crate::hir::*;
use ori_types::Ty;
use smol_str::SmolStr;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MonoKey {
    name: SmolStr,
    args: Vec<(u32, Ty)>,
}

struct FuncInstantiation {
    name: SmolStr,
    params: Vec<Ty>,
    return_ty: Ty,
}

struct MonoState {
    templates: HashMap<SmolStr, HirFunc>,
    specializations: HashMap<MonoKey, SmolStr>,
    generated: Vec<HirFunc>,
}

pub fn monomorphize_generics(module: &mut HirModule) {
    let templates: HashMap<SmolStr, HirFunc> = module
        .funcs
        .iter()
        .filter(|func| func_signature_has_generic_param(func))
        .map(|func| (func.name.clone(), func.clone()))
        .collect();

    if templates.is_empty() {
        return;
    }

    let mut state = MonoState {
        templates,
        specializations: HashMap::new(),
        generated: Vec::new(),
    };

    for func in &mut module.funcs {
        if !state.templates.contains_key(&func.name) {
            rewrite_block_calls(&mut func.body, &mut state);
        }
    }
    for konst in &mut module.consts {
        rewrite_expr_calls(&mut konst.value, &mut state);
    }

    module
        .funcs
        .retain(|func| !state.templates.contains_key(&func.name));
    module.funcs.append(&mut state.generated);
}

impl MonoState {
    fn specialize_call(&mut self, name: &SmolStr, args: &[HirArg]) -> Option<FuncInstantiation> {
        let template = self.templates.get(name)?.clone();
        let subst = infer_call_substitutions(&template, args);
        if subst.is_empty() {
            return None;
        }
        let key = mono_key(name, &subst);
        let params = template
            .params
            .iter()
            .map(|param| substitute_ty(&param.ty, &subst))
            .collect::<Vec<_>>();
        let return_ty = substitute_ty(&template.return_ty, &subst);

        if let Some(existing) = self.specializations.get(&key) {
            return Some(FuncInstantiation {
                name: existing.clone(),
                params,
                return_ty,
            });
        }

        let specialized_name = SmolStr::new(format!("{}.__mono_{}", name, mono_suffix(&key)));
        let mut specialized = template.clone();
        specialized.name = specialized_name.clone();
        substitute_func(&mut specialized, &subst);

        if func_has_generic_param(&specialized) {
            return None;
        }

        self.specializations
            .insert(key.clone(), specialized_name.clone());
        rewrite_block_calls(&mut specialized.body, self);
        self.generated.push(specialized);

        Some(FuncInstantiation {
            name: specialized_name,
            params,
            return_ty,
        })
    }
}

fn infer_call_substitutions(func: &HirFunc, args: &[HirArg]) -> HashMap<u32, Ty> {
    let mut subst = HashMap::new();
    let variadic_index = func.params.iter().position(|param| param.variadic);
    let fixed_count = variadic_index.unwrap_or(func.params.len());

    if args.iter().all(|arg| arg.label.is_none()) {
        for (arg, param) in args
            .iter()
            .take(fixed_count)
            .zip(func.params.iter().take(fixed_count))
        {
            infer_substitution(&param.ty, &arg.value.ty, &mut subst);
        }
        if let Some(index) = variadic_index {
            if let Some(param) = func.params.get(index) {
                for arg in args.iter().skip(index) {
                    infer_variadic_substitution(arg, &param.ty, &mut subst);
                }
            }
        }
    } else {
        let mut slots: Vec<Option<&HirArg>> = vec![None; func.params.len()];
        let mut extras = Vec::new();
        let mut next_positional = 0usize;

        for arg in args {
            if let Some(label) = &arg.label {
                if let Some(index) = func.params.iter().position(|param| param.name == *label) {
                    if slots[index].is_none() {
                        slots[index] = Some(arg);
                    } else {
                        extras.push(arg);
                    }
                } else {
                    extras.push(arg);
                }
            } else {
                while next_positional < slots.len() && slots[next_positional].is_some() {
                    next_positional += 1;
                }
                if next_positional < slots.len() {
                    slots[next_positional] = Some(arg);
                    next_positional += 1;
                } else {
                    extras.push(arg);
                }
            }
        }

        for (arg, param) in slots
            .iter()
            .take(fixed_count)
            .zip(func.params.iter().take(fixed_count))
        {
            if let Some(arg) = arg {
                infer_substitution(&param.ty, &arg.value.ty, &mut subst);
            }
        }
        if let Some(index) = variadic_index {
            if let Some(param) = func.params.get(index) {
                if let Some(Some(arg)) = slots.get(index) {
                    infer_variadic_substitution(arg, &param.ty, &mut subst);
                }
                for arg in extras {
                    infer_variadic_substitution(arg, &param.ty, &mut subst);
                }
            }
        }
    }

    subst
}

fn infer_variadic_substitution(arg: &HirArg, param_ty: &Ty, subst: &mut HashMap<u32, Ty>) {
    let expected = match (arg.spread, param_ty) {
        (false, Ty::List(elem_ty)) => elem_ty.as_ref(),
        _ => param_ty,
    };
    infer_substitution(expected, &arg.value.ty, subst);
}

fn infer_substitution(template: &Ty, actual: &Ty, subst: &mut HashMap<u32, Ty>) {
    match (template, actual) {
        (Ty::Param { index, .. }, actual) => {
            if actual.is_error() || actual.contains_infer() || ty_has_generic_param(actual) {
                return;
            }
            subst.entry(*index).or_insert_with(|| actual.clone());
        }
        (Ty::Optional(t), Ty::Optional(a))
        | (Ty::List(t), Ty::List(a))
        | (Ty::Set(t), Ty::Set(a))
        | (Ty::Range(t), Ty::Range(a))
        | (Ty::Lazy(t), Ty::Lazy(a))
        | (Ty::Future(t), Ty::Future(a))
        | (Ty::TaskJob(t), Ty::TaskJob(a))
        | (Ty::Channel(t), Ty::Channel(a)) => infer_substitution(t, a, subst),
        (Ty::Result(ok_t, err_t), Ty::Result(ok_a, err_a))
        | (Ty::Map(ok_t, err_t), Ty::Map(ok_a, err_a)) => {
            infer_substitution(ok_t, ok_a, subst);
            infer_substitution(err_t, err_a, subst);
        }
        (
            Ty::Opaque {
                kind: kind_t,
                args: args_t,
            },
            Ty::Opaque {
                kind: kind_a,
                args: args_a,
            },
        ) if kind_t == kind_a && args_t.len() == args_a.len() => {
            for (arg_t, arg_a) in args_t.iter().zip(args_a) {
                infer_substitution(arg_t, arg_a, subst);
            }
        }
        (Ty::Tuple(items_t), Ty::Tuple(items_a)) if items_t.len() == items_a.len() => {
            for (item_t, item_a) in items_t.iter().zip(items_a) {
                infer_substitution(item_t, item_a, subst);
            }
        }
        (
            Ty::Func {
                params: params_t,
                ret: ret_t,
            },
            Ty::Func {
                params: params_a,
                ret: ret_a,
            },
        ) if params_t.len() == params_a.len() => {
            for (param_t, param_a) in params_t.iter().zip(params_a) {
                infer_substitution(param_t, param_a, subst);
            }
            infer_substitution(ret_t, ret_a, subst);
        }
        (Ty::Named(id_t, args_t), Ty::Named(id_a, args_a))
            if id_t == id_a && args_t.len() == args_a.len() =>
        {
            for (arg_t, arg_a) in args_t.iter().zip(args_a) {
                infer_substitution(arg_t, arg_a, subst);
            }
        }
        _ => {}
    }
}

fn rewrite_block_calls(block: &mut HirBlock, state: &mut MonoState) {
    for stmt in &mut block.stmts {
        rewrite_stmt_calls(stmt, state);
    }
}

fn rewrite_stmt_calls(stmt: &mut HirStmt, state: &mut MonoState) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Using { value, .. } => {
            rewrite_expr_calls(value, state);
        }
        HirStmt::Assign { lvalue, value, .. } => {
            rewrite_lvalue_calls(lvalue, state);
            rewrite_expr_calls(value, state);
        }
        HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => rewrite_expr_calls(value, state),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            rewrite_expr_calls(cond, state);
            rewrite_block_calls(then, state);
            for (cond, block) in else_ifs {
                rewrite_expr_calls(cond, state);
                rewrite_block_calls(block, state);
            }
            if let Some(block) = else_ {
                rewrite_block_calls(block, state);
            }
        }
        HirStmt::While { cond, body, .. } => {
            rewrite_expr_calls(cond, state);
            rewrite_block_calls(body, state);
        }
        HirStmt::For { iterable, body, .. } => {
            rewrite_expr_calls(iterable, state);
            rewrite_block_calls(body, state);
        }
        HirStmt::Loop { body, .. } => rewrite_block_calls(body, state),
        HirStmt::Repeat { count, body, .. } => {
            rewrite_expr_calls(count, state);
            rewrite_block_calls(body, state);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            rewrite_expr_calls(scrutinee, state);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    rewrite_expr_calls(guard, state);
                }
                for stmt in &mut arm.body {
                    rewrite_stmt_calls(stmt, state);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            rewrite_expr_calls(value, state);
            rewrite_block_calls(then, state);
            if let Some(block) = else_ {
                rewrite_block_calls(block, state);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            rewrite_expr_calls(value, state);
            rewrite_block_calls(body, state);
        }
        HirStmt::Check { condition, .. } => rewrite_expr_calls(condition, state),
    }
}

fn rewrite_lvalue_calls(lvalue: &mut HirLValue, state: &mut MonoState) {
    match lvalue {
        HirLValue::Var(_) => {}
        HirLValue::Field { base, .. } => rewrite_lvalue_calls(base, state),
        HirLValue::Index { base, index } => {
            rewrite_lvalue_calls(base, state);
            rewrite_expr_calls(index, state);
        }
    }
}

fn rewrite_expr_calls(expr: &mut HirExpr, state: &mut MonoState) {
    match &mut expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            rewrite_expr_calls(lhs, state);
            rewrite_expr_calls(rhs, state);
        }
        HirExprKind::Unary { operand, .. } => rewrite_expr_calls(operand, state),
        HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
            rewrite_expr_calls(object, state);
        }
        HirExprKind::Index { object, index } => {
            rewrite_expr_calls(object, state);
            rewrite_expr_calls(index, state);
        }
        HirExprKind::Call { callee, args } => {
            rewrite_expr_calls(callee, state);
            for arg in args.iter_mut() {
                rewrite_expr_calls(&mut arg.value, state);
            }

            let callee_name = match &callee.kind {
                HirExprKind::Var(name) => Some(name.clone()),
                _ => None,
            };
            if let Some(name) = callee_name {
                if let Some(instantiation) = state.specialize_call(&name, args) {
                    callee.kind = HirExprKind::Var(instantiation.name);
                    callee.ty = Ty::Func {
                        params: instantiation.params,
                        ret: Box::new(instantiation.return_ty.clone()),
                    };
                    expr.ty = instantiation.return_ty;
                }
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            rewrite_expr_calls(receiver, state);
            for arg in args {
                rewrite_expr_calls(arg, state);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, value) in fields {
                rewrite_expr_calls(value, state);
            }
        }
        HirExprKind::ListLit { elements, .. } | HirExprKind::SetLit { elements, .. } => {
            for element in elements {
                rewrite_expr_calls(element, state);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for element in elements {
                rewrite_expr_calls(&mut element.value, state);
            }
        }
        HirExprKind::TupleLit(elements) => {
            for element in elements {
                rewrite_expr_calls(element, state);
            }
        }
        HirExprKind::Some_(inner)
        | HirExprKind::Ok_(inner)
        | HirExprKind::Err_(inner)
        | HirExprKind::Propagate(inner)
        | HirExprKind::Await(inner) => rewrite_expr_calls(inner, state),
        HirExprKind::IfExpr { cond, then, else_ } => {
            rewrite_expr_calls(cond, state);
            rewrite_expr_calls(then, state);
            rewrite_expr_calls(else_, state);
        }
        HirExprKind::Range { start, end } => {
            rewrite_expr_calls(start, state);
            rewrite_expr_calls(end, state);
        }
        HirExprKind::MapLit { entries, .. } => {
            for (key, value) in entries {
                rewrite_expr_calls(key, state);
                rewrite_expr_calls(value, state);
            }
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            rewrite_expr_calls(base, state);
            for (_, value) in updates {
                rewrite_expr_calls(value, state);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for part in parts {
                if let HirStrPart::Expr(value) = part {
                    rewrite_expr_calls(value, state);
                }
            }
        }
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::Var(_)
        | HirExprKind::None_
        | HirExprKind::IsCheck { .. }
        | HirExprKind::Closure { .. } => {}
    }
}

fn substitute_func(func: &mut HirFunc, subst: &HashMap<u32, Ty>) {
    for param in &mut func.params {
        substitute_param(param, subst);
    }
    for capture in &mut func.closure_captures {
        capture.ty = substitute_ty(&capture.ty, subst);
    }
    func.return_ty = substitute_ty(&func.return_ty, subst);
    substitute_block(&mut func.body, subst);
}

fn substitute_param(param: &mut HirParam, subst: &HashMap<u32, Ty>) {
    param.ty = substitute_ty(&param.ty, subst);
    if let Some(default) = &mut param.default {
        substitute_expr(default, subst);
    }
    if let Some(contract) = &mut param.contract {
        substitute_expr(contract, subst);
    }
}

fn substitute_block(block: &mut HirBlock, subst: &HashMap<u32, Ty>) {
    for stmt in &mut block.stmts {
        substitute_stmt(stmt, subst);
    }
}

fn substitute_stmt(stmt: &mut HirStmt, subst: &HashMap<u32, Ty>) {
    match stmt {
        HirStmt::Let { ty, value, .. } | HirStmt::Using { ty, value, .. } => {
            *ty = substitute_ty(ty, subst);
            substitute_expr(value, subst);
        }
        HirStmt::Assign { lvalue, value, .. } => {
            substitute_lvalue(lvalue, subst);
            substitute_expr(value, subst);
        }
        HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => substitute_expr(value, subst),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            substitute_expr(cond, subst);
            substitute_block(then, subst);
            for (cond, block) in else_ifs {
                substitute_expr(cond, subst);
                substitute_block(block, subst);
            }
            if let Some(block) = else_ {
                substitute_block(block, subst);
            }
        }
        HirStmt::While { cond, body, .. } => {
            substitute_expr(cond, subst);
            substitute_block(body, subst);
        }
        HirStmt::For {
            elem_ty,
            iterable,
            body,
            ..
        } => {
            *elem_ty = substitute_ty(elem_ty, subst);
            substitute_expr(iterable, subst);
            substitute_block(body, subst);
        }
        HirStmt::Loop { body, .. } => substitute_block(body, subst),
        HirStmt::Repeat { count, body, .. } => {
            substitute_expr(count, subst);
            substitute_block(body, subst);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            substitute_expr(scrutinee, subst);
            for arm in arms {
                substitute_pattern(&mut arm.pattern, subst);
                if let Some(guard) = &mut arm.guard {
                    substitute_expr(guard, subst);
                }
                for stmt in &mut arm.body {
                    substitute_stmt(stmt, subst);
                }
            }
        }
        HirStmt::IfSome {
            inner_ty,
            value,
            then,
            else_,
            ..
        } => {
            *inner_ty = substitute_ty(inner_ty, subst);
            substitute_expr(value, subst);
            substitute_block(then, subst);
            if let Some(block) = else_ {
                substitute_block(block, subst);
            }
        }
        HirStmt::WhileSome {
            inner_ty,
            value,
            body,
            ..
        } => {
            *inner_ty = substitute_ty(inner_ty, subst);
            substitute_expr(value, subst);
            substitute_block(body, subst);
        }
        HirStmt::Check { condition, .. } => substitute_expr(condition, subst),
    }
}

fn substitute_lvalue(lvalue: &mut HirLValue, subst: &HashMap<u32, Ty>) {
    match lvalue {
        HirLValue::Var(_) => {}
        HirLValue::Field { base, .. } => substitute_lvalue(base, subst),
        HirLValue::Index { base, index } => {
            substitute_lvalue(base, subst);
            substitute_expr(index, subst);
        }
    }
}

fn substitute_pattern(pattern: &mut HirPattern, subst: &HashMap<u32, Ty>) {
    match pattern {
        HirPattern::Binding(_, ty) => *ty = substitute_ty(ty, subst),
        HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
            substitute_pattern(inner, subst);
        }
        HirPattern::Variant { fields, .. } => {
            for (_, pattern) in fields {
                substitute_pattern(pattern, subst);
            }
        }
        HirPattern::Tuple(patterns) => {
            for pattern in patterns {
                substitute_pattern(pattern, subst);
            }
        }
        HirPattern::Wildcard
        | HirPattern::BoolLit(_)
        | HirPattern::IntLit(_)
        | HirPattern::StrLit(_)
        | HirPattern::None_ => {}
    }
}

fn substitute_expr(expr: &mut HirExpr, subst: &HashMap<u32, Ty>) {
    expr.ty = substitute_ty(&expr.ty, subst);
    match &mut expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            substitute_expr(lhs, subst);
            substitute_expr(rhs, subst);
        }
        HirExprKind::Unary { operand, .. } => substitute_expr(operand, subst),
        HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
            substitute_expr(object, subst);
        }
        HirExprKind::Index { object, index } => {
            substitute_expr(object, subst);
            substitute_expr(index, subst);
        }
        HirExprKind::Call { callee, args } => {
            substitute_expr(callee, subst);
            for arg in args {
                substitute_expr(&mut arg.value, subst);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            substitute_expr(receiver, subst);
            for arg in args {
                substitute_expr(arg, subst);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, value) in fields {
                substitute_expr(value, subst);
            }
        }
        HirExprKind::ListLit { elem_ty, elements } | HirExprKind::SetLit { elem_ty, elements } => {
            *elem_ty = substitute_ty(elem_ty, subst);
            for element in elements {
                substitute_expr(element, subst);
            }
        }
        HirExprKind::ListSpreadLit { elem_ty, elements } => {
            *elem_ty = substitute_ty(elem_ty, subst);
            for element in elements {
                substitute_expr(&mut element.value, subst);
            }
        }
        HirExprKind::TupleLit(elements) => {
            for element in elements {
                substitute_expr(element, subst);
            }
        }
        HirExprKind::Some_(inner)
        | HirExprKind::Ok_(inner)
        | HirExprKind::Err_(inner)
        | HirExprKind::Propagate(inner)
        | HirExprKind::Await(inner) => substitute_expr(inner, subst),
        HirExprKind::IfExpr { cond, then, else_ } => {
            substitute_expr(cond, subst);
            substitute_expr(then, subst);
            substitute_expr(else_, subst);
        }
        HirExprKind::Range { start, end } => {
            substitute_expr(start, subst);
            substitute_expr(end, subst);
        }
        HirExprKind::MapLit {
            key_ty,
            value_ty,
            entries,
        } => {
            *key_ty = substitute_ty(key_ty, subst);
            *value_ty = substitute_ty(value_ty, subst);
            for (key, value) in entries {
                substitute_expr(key, subst);
                substitute_expr(value, subst);
            }
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            substitute_expr(base, subst);
            for (_, value) in updates {
                substitute_expr(value, subst);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for part in parts {
                if let HirStrPart::Expr(value) = part {
                    substitute_expr(value, subst);
                }
            }
        }
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::Var(_)
        | HirExprKind::None_ => {}
        HirExprKind::Closure { captures, .. } => {
            for capture in captures {
                capture.ty = substitute_ty(&capture.ty, subst);
            }
        }
        HirExprKind::IsCheck { value, .. } => {
            substitute_expr(value, subst);
        }
    }
}

fn substitute_ty(ty: &Ty, subst: &HashMap<u32, Ty>) -> Ty {
    match ty {
        Ty::Param { index, .. } => subst.get(index).cloned().unwrap_or_else(|| ty.clone()),
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_ty(inner, subst))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(substitute_ty(ok, subst)),
            Box::new(substitute_ty(err, subst)),
        ),
        Ty::List(inner) => Ty::List(Box::new(substitute_ty(inner, subst))),
        Ty::Map(key, value) => Ty::Map(
            Box::new(substitute_ty(key, subst)),
            Box::new(substitute_ty(value, subst)),
        ),
        Ty::Set(inner) => Ty::Set(Box::new(substitute_ty(inner, subst))),
        Ty::Range(inner) => Ty::Range(Box::new(substitute_ty(inner, subst))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(substitute_ty(inner, subst))),
        Ty::Future(inner) => Ty::Future(Box::new(substitute_ty(inner, subst))),
        Ty::TaskJob(inner) => Ty::TaskJob(Box::new(substitute_ty(inner, subst))),
        Ty::Channel(inner) => Ty::Channel(Box::new(substitute_ty(inner, subst))),
        Ty::Opaque { kind, args } => Ty::Opaque {
            kind: *kind,
            args: args.iter().map(|arg| substitute_ty(arg, subst)).collect(),
        },
        Ty::Tuple(items) => Ty::Tuple(
            items
                .iter()
                .map(|item| substitute_ty(item, subst))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .iter()
                .map(|param| substitute_ty(param, subst))
                .collect(),
            ret: Box::new(substitute_ty(ret, subst)),
        },
        Ty::Named(def_id, args) => Ty::Named(
            *def_id,
            args.iter().map(|arg| substitute_ty(arg, subst)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn func_has_generic_param(func: &HirFunc) -> bool {
    func.params
        .iter()
        .any(|param| param_has_generic_param(param))
        || ty_has_generic_param(&func.return_ty)
        || block_has_generic_param(&func.body)
}

fn func_signature_has_generic_param(func: &HirFunc) -> bool {
    func.params.iter().any(param_has_generic_param) || ty_has_generic_param(&func.return_ty)
}

fn param_has_generic_param(param: &HirParam) -> bool {
    ty_has_generic_param(&param.ty)
        || param.default.as_ref().is_some_and(expr_has_generic_param)
        || param.contract.as_ref().is_some_and(expr_has_generic_param)
}

fn block_has_generic_param(block: &HirBlock) -> bool {
    block.stmts.iter().any(stmt_has_generic_param)
}

fn stmt_has_generic_param(stmt: &HirStmt) -> bool {
    match stmt {
        HirStmt::Let { ty, value, .. } | HirStmt::Using { ty, value, .. } => {
            ty_has_generic_param(ty) || expr_has_generic_param(value)
        }
        HirStmt::Assign { lvalue, value, .. } => {
            lvalue_has_generic_param(lvalue) || expr_has_generic_param(value)
        }
        HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => expr_has_generic_param(value),
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => false,
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            expr_has_generic_param(cond)
                || block_has_generic_param(then)
                || else_ifs.iter().any(|(cond, block)| {
                    expr_has_generic_param(cond) || block_has_generic_param(block)
                })
                || else_.as_ref().is_some_and(block_has_generic_param)
        }
        HirStmt::While { cond, body, .. } => {
            expr_has_generic_param(cond) || block_has_generic_param(body)
        }
        HirStmt::For {
            elem_ty,
            iterable,
            body,
            ..
        } => {
            ty_has_generic_param(elem_ty)
                || expr_has_generic_param(iterable)
                || block_has_generic_param(body)
        }
        HirStmt::Loop { body, .. } => block_has_generic_param(body),
        HirStmt::Repeat { count, body, .. } => {
            expr_has_generic_param(count) || block_has_generic_param(body)
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            expr_has_generic_param(scrutinee)
                || arms.iter().any(|arm| {
                    pattern_has_generic_param(&arm.pattern)
                        || arm.guard.as_ref().is_some_and(expr_has_generic_param)
                        || arm.body.iter().any(stmt_has_generic_param)
                })
        }
        HirStmt::IfSome {
            inner_ty,
            value,
            then,
            else_,
            ..
        } => {
            ty_has_generic_param(inner_ty)
                || expr_has_generic_param(value)
                || block_has_generic_param(then)
                || else_.as_ref().is_some_and(block_has_generic_param)
        }
        HirStmt::WhileSome {
            inner_ty,
            value,
            body,
            ..
        } => {
            ty_has_generic_param(inner_ty)
                || expr_has_generic_param(value)
                || block_has_generic_param(body)
        }
        HirStmt::Check { condition, .. } => expr_has_generic_param(condition),
    }
}

fn lvalue_has_generic_param(lvalue: &HirLValue) -> bool {
    match lvalue {
        HirLValue::Var(_) => false,
        HirLValue::Field { base, .. } => lvalue_has_generic_param(base),
        HirLValue::Index { base, index } => {
            lvalue_has_generic_param(base) || expr_has_generic_param(index)
        }
    }
}

fn pattern_has_generic_param(pattern: &HirPattern) -> bool {
    match pattern {
        HirPattern::Binding(_, ty) => ty_has_generic_param(ty),
        HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
            pattern_has_generic_param(inner)
        }
        HirPattern::Variant { fields, .. } => fields
            .iter()
            .any(|(_, pattern)| pattern_has_generic_param(pattern)),
        HirPattern::Tuple(patterns) => patterns.iter().any(pattern_has_generic_param),
        HirPattern::Wildcard
        | HirPattern::BoolLit(_)
        | HirPattern::IntLit(_)
        | HirPattern::StrLit(_)
        | HirPattern::None_ => false,
    }
}

fn expr_has_generic_param(expr: &HirExpr) -> bool {
    ty_has_generic_param(&expr.ty)
        || match &expr.kind {
            HirExprKind::Binary { lhs, rhs, .. } => {
                expr_has_generic_param(lhs) || expr_has_generic_param(rhs)
            }
            HirExprKind::Unary { operand, .. } => expr_has_generic_param(operand),
            HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
                expr_has_generic_param(object)
            }
            HirExprKind::Index { object, index } => {
                expr_has_generic_param(object) || expr_has_generic_param(index)
            }
            HirExprKind::Call { callee, args } => {
                expr_has_generic_param(callee)
                    || args.iter().any(|arg| expr_has_generic_param(&arg.value))
            }
            HirExprKind::MethodCall { receiver, args, .. } => {
                expr_has_generic_param(receiver) || args.iter().any(expr_has_generic_param)
            }
            HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
                fields
                    .iter()
                    .any(|(_, value)| expr_has_generic_param(value))
            }
            HirExprKind::ListLit { elem_ty, elements }
            | HirExprKind::SetLit { elem_ty, elements } => {
                ty_has_generic_param(elem_ty) || elements.iter().any(expr_has_generic_param)
            }
            HirExprKind::ListSpreadLit { elem_ty, elements } => {
                ty_has_generic_param(elem_ty)
                    || elements
                        .iter()
                        .any(|element| expr_has_generic_param(&element.value))
            }
            HirExprKind::TupleLit(elements) => elements.iter().any(expr_has_generic_param),
            HirExprKind::Some_(inner)
            | HirExprKind::Ok_(inner)
            | HirExprKind::Err_(inner)
            | HirExprKind::Propagate(inner)
            | HirExprKind::Await(inner) => expr_has_generic_param(inner),
            HirExprKind::IfExpr { cond, then, else_ } => {
                expr_has_generic_param(cond)
                    || expr_has_generic_param(then)
                    || expr_has_generic_param(else_)
            }
            HirExprKind::Range { start, end } => {
                expr_has_generic_param(start) || expr_has_generic_param(end)
            }
            HirExprKind::MapLit {
                key_ty,
                value_ty,
                entries,
            } => {
                ty_has_generic_param(key_ty)
                    || ty_has_generic_param(value_ty)
                    || entries.iter().any(|(key, value)| {
                        expr_has_generic_param(key) || expr_has_generic_param(value)
                    })
            }
            HirExprKind::StructUpdate { base, updates, .. } => {
                expr_has_generic_param(base)
                    || updates
                        .iter()
                        .any(|(_, value)| expr_has_generic_param(value))
            }
            HirExprKind::InterpolatedStr(parts) => parts.iter().any(|part| match part {
                HirStrPart::Literal(_) => false,
                HirStrPart::Expr(value) => expr_has_generic_param(value),
            }),
            HirExprKind::BoolLit(_)
            | HirExprKind::IntLit(_)
            | HirExprKind::FloatLit(_)
            | HirExprKind::StrLit(_)
            | HirExprKind::BytesLit(_)
            | HirExprKind::Unit
            | HirExprKind::Var(_)
            | HirExprKind::None_ => false,
            HirExprKind::Closure { captures, .. } => captures
                .iter()
                .any(|capture| ty_has_generic_param(&capture.ty)),
            HirExprKind::IsCheck { value, .. } => expr_has_generic_param(value),
        }
}

fn ty_has_generic_param(ty: &Ty) -> bool {
    match ty {
        Ty::Param { .. } => true,
        Ty::Optional(inner)
        | Ty::List(inner)
        | Ty::Set(inner)
        | Ty::Range(inner)
        | Ty::Lazy(inner)
        | Ty::Future(inner)
        | Ty::TaskJob(inner)
        | Ty::Channel(inner) => ty_has_generic_param(inner),
        Ty::Result(ok, err) | Ty::Map(ok, err) => {
            ty_has_generic_param(ok) || ty_has_generic_param(err)
        }
        Ty::Opaque { args, .. } => args.iter().any(ty_has_generic_param),
        Ty::Tuple(items) => items.iter().any(ty_has_generic_param),
        Ty::Func { params, ret } => {
            params.iter().any(ty_has_generic_param) || ty_has_generic_param(ret)
        }
        Ty::Named(_, args) => args.iter().any(ty_has_generic_param),
        _ => false,
    }
}

fn mono_key(name: &SmolStr, subst: &HashMap<u32, Ty>) -> MonoKey {
    let mut args: Vec<(u32, Ty)> = subst
        .iter()
        .map(|(index, ty)| (*index, ty.clone()))
        .collect();
    args.sort_by_key(|(index, _)| *index);
    MonoKey {
        name: name.clone(),
        args,
    }
}

fn mono_suffix(key: &MonoKey) -> String {
    key.args
        .iter()
        .map(|(index, ty)| format!("t{}_{}", index, sanitize_type_key(&ty.display())))
        .collect::<Vec<_>>()
        .join("_")
}

fn sanitize_type_key(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}
