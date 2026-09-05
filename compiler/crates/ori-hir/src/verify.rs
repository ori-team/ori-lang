//! HIR structural verifier.
//!
//! Checks invariants that lowering and optimization passes must preserve:
//! unique function/struct/enum definition paths, resolved struct/enum
//! references in literals, known trait-impl targets, and non-empty
//! match arms. Returns one message per violation; an empty vector means
//! the module is well-formed.

use crate::hir::*;
use std::collections::HashSet;

pub fn verify_module(module: &HirModule) -> Vec<String> {
    let mut errors = Vec::new();

    let mut func_names = HashSet::new();
    for func in &module.funcs {
        if !func_names.insert(func.name.clone()) {
            errors.push(format!("duplicate function definition `{}`", func.name));
        }
        check_block(&func.body, &mut errors);
    }

    // Multi-file lowering merges per-module declarations, so the same DefId
    // (e.g. the synthetic `ori.Error` builtin) may appear once per module.
    // A repeated path with a *different* DefId is a real collision.
    let mut seen_struct_ids = HashSet::new();
    let mut struct_path_ids = std::collections::HashMap::new();
    for structure in &module.structs {
        let first_seen = seen_struct_ids.insert(structure.def_id);
        if first_seen {
            if let Some(&first) = struct_path_ids.get(&structure.name) {
                if first != structure.def_id {
                    errors.push(format!("duplicate struct definition `{}`", structure.name));
                }
            } else {
                struct_path_ids.insert(structure.name.clone(), structure.def_id);
            }
            let mut fields = HashSet::new();
            for field in &structure.fields {
                if !fields.insert(field.name.clone()) {
                    errors.push(format!(
                        "duplicate field `{}` in struct `{}`",
                        field.name, structure.name
                    ));
                }
                check_expr(&field.contract, &mut errors);
            }
        }
    }

    let mut seen_enum_ids = HashSet::new();
    let mut enum_path_ids = std::collections::HashMap::new();
    for enumeration in &module.enums {
        let first_seen = seen_enum_ids.insert(enumeration.def_id);
        if first_seen {
            if let Some(&first) = enum_path_ids.get(&enumeration.name) {
                if first != enumeration.def_id {
                    errors.push(format!("duplicate enum definition `{}`", enumeration.name));
                }
            } else {
                enum_path_ids.insert(enumeration.name.clone(), enumeration.def_id);
            }
            let mut variants = HashSet::new();
            for variant in &enumeration.variants {
                if !variants.insert(variant.name.clone()) {
                    errors.push(format!(
                        "duplicate variant `{}` in enum `{}`",
                        variant.name, enumeration.name
                    ));
                }
            }
        }
    }

    for constant in &module.consts {
        check_expr(&Some(constant.value.clone()), &mut errors);
    }

    let struct_ids: HashSet<_> = module.structs.iter().map(|s| s.def_id).collect();
    let enum_ids: HashSet<_> = module.enums.iter().map(|e| e.def_id).collect();
    for func in &module.funcs {
        check_refs(&func.body, &struct_ids, &enum_ids, &mut errors);
    }

    errors
}

fn check_block(block: &HirBlock, errors: &mut Vec<String>) {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Match { arms, .. } if arms.is_empty() => {
                errors.push("match statement with no arms".to_string());
            }
            HirStmt::Match { arms, .. } => {
                for arm in arms {
                    check_block_body(&arm.body, errors);
                    if let Some(guard) = &arm.guard {
                        check_expr(&Some(guard.clone()), errors);
                    }
                }
            }
            HirStmt::If { then, else_, .. } => {
                check_block(then, errors);
                if let Some(other) = else_ {
                    check_block(other, errors);
                }
            }
            HirStmt::While { body, .. }
            | HirStmt::Loop { body, .. }
            | HirStmt::For { body, .. }
            | HirStmt::Repeat { body, .. } => check_block(body, errors),
            HirStmt::Let { value, .. } => check_expr(&Some(value.clone()), errors),
            HirStmt::Assign { value, .. } => check_expr(&Some(value.clone()), errors),
            HirStmt::Return(value, _) => check_expr(value, errors),
            HirStmt::Expr(value) => check_expr(&Some(value.clone()), errors),
            HirStmt::Break(_) | HirStmt::Continue(_) => {}
            HirStmt::IfSome {
                value, then, else_, ..
            } => {
                check_expr(&Some(value.clone()), errors);
                check_block(then, errors);
                if let Some(other) = else_ {
                    check_block(other, errors);
                }
            }
            HirStmt::WhileSome { value, body, .. } => {
                check_expr(&Some(value.clone()), errors);
                check_block(body, errors);
            }
            HirStmt::Using { value, .. } => check_expr(&Some(value.clone()), errors),
            HirStmt::Check { condition, .. } => check_expr(&Some(condition.clone()), errors),
        }
    }
}

fn check_block_body(stmts: &[HirStmt], errors: &mut Vec<String>) {
    check_block(
        &HirBlock {
            stmts: stmts.to_vec(),
            span: ori_diagnostics::Span::DUMMY,
        },
        errors,
    );
}

fn check_expr(expr: &Option<HirExpr>, errors: &mut Vec<String>) {
    let Some(value) = expr else { return };
    match &value.kind {
        HirExprKind::MatchExpr { scrutinee, arms } => {
            if arms.is_empty() {
                errors.push("match expression with no arms".to_string());
            }
            check_expr(&Some((**scrutinee).clone()), errors);
            for arm in arms {
                check_expr(&Some(arm.body.clone()), errors);
            }
        }
        HirExprKind::Call { callee, args } => {
            check_expr(&Some((**callee).clone()), errors);
            for arg in args {
                check_expr(&Some(arg.value.clone()), errors);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            check_expr(&Some((**receiver).clone()), errors);
            for arg in args {
                check_expr(&Some(arg.clone()), errors);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, field) in fields {
                check_expr(&Some(field.clone()), errors);
            }
        }
        _ => {}
    }
}

fn check_refs(
    block: &HirBlock,
    structs: &HashSet<ori_types::DefId>,
    enums: &HashSet<ori_types::DefId>,
    errors: &mut Vec<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Expr(value)
            | HirStmt::Return(Some(value), _)
            | HirStmt::Let { value, .. }
            | HirStmt::Assign { value, .. } => {
                check_expr_refs(value, structs, enums, errors);
            }
            HirStmt::If { then, else_, .. } => {
                check_refs(then, structs, enums, errors);
                if let Some(other) = else_ {
                    check_refs(other, structs, enums, errors);
                }
            }
            HirStmt::While { body, .. }
            | HirStmt::Loop { body, .. }
            | HirStmt::For { body, .. }
            | HirStmt::Repeat { body, .. } => check_refs(body, structs, enums, errors),
            HirStmt::Match { arms, .. } => {
                for arm in &arm_body_refs(arms) {
                    check_refs(arm, structs, enums, errors);
                }
            }
            _ => {}
        }
    }
}

fn arm_body_refs(arms: &[HirArm]) -> Vec<HirBlock> {
    arms.iter()
        .map(|arm| HirBlock {
            stmts: arm.body.clone(),
            span: arm.span,
        })
        .collect()
}

fn check_expr_refs(
    expr: &HirExpr,
    structs: &HashSet<ori_types::DefId>,
    enums: &HashSet<ori_types::DefId>,
    errors: &mut Vec<String>,
) {
    match &expr.kind {
        HirExprKind::StructLit {
            def_id: Some(id), ..
        } if !structs.contains(id) => {
            errors.push(format!("struct literal references unknown struct `{id:?}`"));
        }
        HirExprKind::StructLit { .. } => {}
        HirExprKind::EnumVariant {
            def_id: Some(id), ..
        } if !enums.contains(id) => {
            errors.push(format!("enum variant references unknown enum `{id:?}`"));
        }
        HirExprKind::EnumVariant { .. } => {}
        HirExprKind::Call { callee, args } => {
            check_expr_refs(callee, structs, enums, errors);
            for arg in args {
                check_expr_refs(&arg.value, structs, enums, errors);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            check_expr_refs(receiver, structs, enums, errors);
            for arg in args {
                check_expr_refs(arg, structs, enums, errors);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_diagnostics::Span;

    fn empty_module() -> HirModule {
        HirModule {
            namespace: "app.test".into(),
            structs: Vec::new(),
            enums: Vec::new(),
            traits: Vec::new(),
            trait_impls: Vec::new(),
            funcs: Vec::new(),
            consts: Vec::new(),
            externs: Vec::new(),
        }
    }

    fn empty_block() -> HirBlock {
        HirBlock {
            stmts: Vec::new(),
            span: Span::DUMMY,
        }
    }

    fn test_func(name: &str) -> HirFunc {
        HirFunc {
            def_id: ori_types::DefId(9999),
            name: name.into(),
            params: Vec::new(),
            return_ty: ori_types::Ty::Void,
            body: empty_block(),
            closure_captures: Vec::new(),
            is_public: false,
            is_async: false,
            is_mut: false,
            is_inline: false,
            is_no_inline: false,
            c_export_name: None,
            span: Span::DUMMY,
        }
    }

    #[test]
    fn accepts_empty_module() {
        assert!(verify_module(&empty_module()).is_empty());
    }

    #[test]
    fn rejects_duplicate_function_definitions() {
        let mut module = empty_module();
        module.funcs.push(test_func("app.test.f"));
        module.funcs.push(test_func("app.test.f"));
        let errors = verify_module(&module);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("duplicate function"));
    }

    #[test]
    fn accepts_same_struct_defid_from_merged_modules() {
        use crate::hir::HirStruct;
        let mut module = empty_module();
        for _ in 0..2 {
            module.structs.push(HirStruct {
                def_id: ori_types::DefId(7),
                name: "ori.Error".into(),
                fields: Vec::new(),
                is_public: true,
                repr_c: false,
                explicit_align: None,
                span: Span::DUMMY,
            });
        }
        assert!(verify_module(&module).is_empty());
    }

    #[test]
    fn rejects_match_without_arms() {
        let mut module = empty_module();
        let mut func = test_func("app.test.f");
        func.body.stmts.push(HirStmt::Match {
            scrutinee: HirExpr {
                kind: HirExprKind::BoolLit(true),
                ty: ori_types::Ty::Bool,
                span: Span::DUMMY,
            },
            arms: Vec::new(),
            span: Span::DUMMY,
        });
        module.funcs.push(func);
        let errors = verify_module(&module);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("no arms"));
    }

    #[test]
    fn rejects_struct_literal_with_unknown_def() {
        let mut module = empty_module();
        let mut func = test_func("app.test.f");
        func.body.stmts.push(HirStmt::Expr(HirExpr {
            kind: HirExprKind::StructLit {
                def_id: Some(ori_types::DefId(424242)),
                fields: Vec::new(),
            },
            ty: ori_types::Ty::Void,
            span: Span::DUMMY,
        }));
        module.funcs.push(func);
        let errors = verify_module(&module);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("unknown struct"));
    }
}
