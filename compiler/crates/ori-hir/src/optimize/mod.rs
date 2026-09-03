//! HIR mid-end optimisations (LANG-PERF-2).
//!
//! Semantics-preserving rewrites between monomorphization and native lower.
//! Default: const fold + pure-loop strength reduction + DCE.
//! Aggressive: + monomorphic leaf inlining (`ORI_OPT=aggressive`).

mod const_fold;
mod dce;
mod inline_leafs;
mod pipeline;
mod strength_reduce;

pub use pipeline::{optimize_module, OptLevel};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::*;
    use ori_diagnostics::Span;
    use ori_types::{DefId, Ty};
    use smol_str::SmolStr;

    fn span() -> Span {
        Span::DUMMY
    }

    fn int_lit(n: i64) -> HirExpr {
        HirExpr {
            kind: HirExprKind::IntLit(n),
            ty: Ty::Int,
            span: span(),
        }
    }

    fn bytes_lit(bytes: &[u8]) -> HirExpr {
        HirExpr {
            kind: HirExprKind::BytesLit(bytes.to_vec()),
            ty: Ty::Bytes,
            span: span(),
        }
    }

    fn var(name: &str) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Var(SmolStr::new(name)),
            ty: Ty::Int,
            span: span(),
        }
    }

    fn bin(op: ori_ast::expr::BinaryOp, lhs: HirExpr, rhs: HirExpr) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: Ty::Int,
            span: span(),
        }
    }

    fn let_mut(name: &str, value: HirExpr) -> HirStmt {
        HirStmt::Let {
            name: SmolStr::new(name),
            ty: Ty::Int,
            mutable: true,
            value,
            span: span(),
        }
    }

    fn assign(name: &str, value: HirExpr) -> HirStmt {
        HirStmt::Assign {
            lvalue: HirLValue::Var(SmolStr::new(name)),
            value,
            span: span(),
        }
    }

    fn empty_module_with_body(stmts: Vec<HirStmt>) -> HirModule {
        HirModule {
            namespace: SmolStr::new("app"),
            structs: vec![],
            enums: vec![],
            traits: vec![],
            trait_impls: vec![],
            funcs: vec![HirFunc {
                def_id: DefId(1),
                name: SmolStr::new("main"),
                params: vec![],
                return_ty: Ty::Void,
                body: HirBlock {
                    stmts,
                    span: span(),
                },
                closure_captures: vec![],
                is_public: true,
                is_async: false,
                is_mut: false,
                is_inline: false,
                is_no_inline: false,
                c_export_name: None,
                span: span(),
            }],
            consts: vec![],
            externs: vec![],
        }
    }

    #[test]
    fn const_fold_adds_literals() {
        let mut module = empty_module_with_body(vec![
            HirStmt::Let {
                name: SmolStr::new("x"),
                ty: Ty::Int,
                mutable: false,
                value: bin(ori_ast::expr::BinaryOp::Add, int_lit(2), int_lit(3)),
                span: span(),
            },
            HirStmt::Return(Some(var("x")), span()),
        ]);
        optimize_module(&mut module, OptLevel::Default);
        match &module.funcs[0].body.stmts[0] {
            HirStmt::Let { value, .. } => match value.kind {
                HirExprKind::IntLit(5) => {}
                ref other => panic!("expected IntLit(5), got {other:?}"),
            },
            other => panic!("expected Let, got {other:?}"),
        }
    }

    #[test]
    fn strength_reduce_sum_loop_to_closed_form() {
        // var s = 0; var i = 0; while i < 10 { s = s + i; i = i + 1 }
        let mut module = empty_module_with_body(vec![
            let_mut("s", int_lit(0)),
            let_mut("i", int_lit(0)),
            HirStmt::While {
                cond: bin(ori_ast::expr::BinaryOp::Lt, var("i"), int_lit(10)),
                body: HirBlock {
                    stmts: vec![
                        assign("s", bin(ori_ast::expr::BinaryOp::Add, var("s"), var("i"))),
                        assign("i", bin(ori_ast::expr::BinaryOp::Add, var("i"), int_lit(1))),
                    ],
                    span: span(),
                },
                span: span(),
            },
        ]);
        optimize_module(&mut module, OptLevel::Default);
        let stmts = &module.funcs[0].body.stmts;
        // While should be replaced by if true { s = ...; i = ... }
        assert!(
            !stmts.iter().any(|s| matches!(s, HirStmt::While { .. })),
            "expected while to be strength-reduced, got {stmts:?}"
        );
        assert!(
            stmts.iter().any(|s| matches!(s, HirStmt::If { .. })),
            "expected closed-form if, got {stmts:?}"
        );
    }

    #[test]
    fn leaf_inline_replaces_call_under_aggressive() {
        let add_one = HirFunc {
            def_id: DefId(2),
            name: SmolStr::new("add_one"),
            params: vec![HirParam {
                name: SmolStr::new("x"),
                ty: Ty::Int,
                default: None,
                contract: None,
                variadic: false,
                span: span(),
            }],
            return_ty: Ty::Int,
            body: HirBlock {
                stmts: vec![HirStmt::Return(
                    Some(bin(ori_ast::expr::BinaryOp::Add, var("x"), int_lit(1))),
                    span(),
                )],
                span: span(),
            },
            closure_captures: vec![],
            is_public: false,
            is_async: false,
            is_mut: false,
            is_inline: false,
            is_no_inline: false,
            c_export_name: None,
            span: span(),
        };
        let mut module = HirModule {
            namespace: SmolStr::new("app"),
            structs: vec![],
            enums: vec![],
            traits: vec![],
            trait_impls: vec![],
            funcs: vec![
                add_one,
                HirFunc {
                    def_id: DefId(1),
                    name: SmolStr::new("main"),
                    params: vec![],
                    return_ty: Ty::Int,
                    body: HirBlock {
                        stmts: vec![HirStmt::Return(
                            Some(HirExpr {
                                kind: HirExprKind::Call {
                                    callee: Box::new(HirExpr {
                                        kind: HirExprKind::Var(SmolStr::new("add_one")),
                                        ty: Ty::Int,
                                        span: span(),
                                    }),
                                    args: vec![HirArg {
                                        label: None,
                                        value: int_lit(41),
                                        spread: false,
                                    }],
                                },
                                ty: Ty::Int,
                                span: span(),
                            }),
                            span(),
                        )],
                        span: span(),
                    },
                    closure_captures: vec![],
                    is_public: true,
                    is_async: false,
                    is_mut: false,
                    is_inline: false,
                    is_no_inline: false,
                    c_export_name: None,
                    span: span(),
                },
            ],
            consts: vec![],
            externs: vec![],
        };
        optimize_module(&mut module, OptLevel::Aggressive);
        // After inline + fold: return 42
        match &module.funcs[1].body.stmts[0] {
            HirStmt::Return(Some(e), _) => match e.kind {
                HirExprKind::IntLit(42) => {}
                ref other => panic!("expected IntLit(42) after inline+fold, got {other:?}"),
            },
            other => panic!("expected Return, got {other:?}"),
        }
    }

    #[test]
    fn leaf_inline_respects_no_inline_under_aggressive() {
        let add_one = HirFunc {
            def_id: DefId(2),
            name: SmolStr::new("add_one"),
            params: vec![HirParam {
                name: SmolStr::new("x"),
                ty: Ty::Int,
                default: None,
                contract: None,
                variadic: false,
                span: span(),
            }],
            return_ty: Ty::Int,
            body: HirBlock {
                stmts: vec![HirStmt::Return(
                    Some(bin(ori_ast::expr::BinaryOp::Add, var("x"), int_lit(1))),
                    span(),
                )],
                span: span(),
            },
            closure_captures: vec![],
            is_public: false,
            is_async: false,
            is_mut: false,
            is_inline: false,
            is_no_inline: true,
            c_export_name: None,
            span: span(),
        };
        let mut module = HirModule {
            namespace: SmolStr::new("app"),
            structs: vec![],
            enums: vec![],
            traits: vec![],
            trait_impls: vec![],
            funcs: vec![
                add_one,
                HirFunc {
                    def_id: DefId(1),
                    name: SmolStr::new("main"),
                    params: vec![],
                    return_ty: Ty::Int,
                    body: HirBlock {
                        stmts: vec![HirStmt::Return(
                            Some(HirExpr {
                                kind: HirExprKind::Call {
                                    callee: Box::new(HirExpr {
                                        kind: HirExprKind::Var(SmolStr::new("add_one")),
                                        ty: Ty::Int,
                                        span: span(),
                                    }),
                                    args: vec![HirArg {
                                        label: None,
                                        value: int_lit(41),
                                        spread: false,
                                    }],
                                },
                                ty: Ty::Int,
                                span: span(),
                            }),
                            span(),
                        )],
                        span: span(),
                    },
                    closure_captures: vec![],
                    is_public: true,
                    is_async: false,
                    is_mut: false,
                    is_inline: false,
                    is_no_inline: false,
                    c_export_name: None,
                    span: span(),
                },
            ],
            consts: vec![],
            externs: vec![],
        };
        optimize_module(&mut module, OptLevel::Aggressive);
        match &module.funcs[1].body.stmts[0] {
            HirStmt::Return(Some(e), _) => match &e.kind {
                HirExprKind::Call { .. } => {}
                other => panic!("expected Call preserved due to no_inline, got {other:?}"),
            },
            other => panic!("expected Return, got {other:?}"),
        }
    }

    #[test]
    fn leaf_inline_skipped_under_default() {
        let add_one = HirFunc {
            def_id: DefId(2),
            name: SmolStr::new("add_one"),
            params: vec![HirParam {
                name: SmolStr::new("x"),
                ty: Ty::Int,
                default: None,
                contract: None,
                variadic: false,
                span: span(),
            }],
            return_ty: Ty::Int,
            body: HirBlock {
                stmts: vec![HirStmt::Return(
                    Some(bin(ori_ast::expr::BinaryOp::Add, var("x"), int_lit(1))),
                    span(),
                )],
                span: span(),
            },
            closure_captures: vec![],
            is_public: false,
            is_async: false,
            is_mut: false,
            is_inline: false,
            is_no_inline: false,
            c_export_name: None,
            span: span(),
        };
        let mut module = HirModule {
            namespace: SmolStr::new("app"),
            structs: vec![],
            enums: vec![],
            traits: vec![],
            trait_impls: vec![],
            funcs: vec![
                add_one,
                HirFunc {
                    def_id: DefId(1),
                    name: SmolStr::new("main"),
                    params: vec![],
                    return_ty: Ty::Int,
                    body: HirBlock {
                        stmts: vec![HirStmt::Return(
                            Some(HirExpr {
                                kind: HirExprKind::Call {
                                    callee: Box::new(HirExpr {
                                        kind: HirExprKind::Var(SmolStr::new("add_one")),
                                        ty: Ty::Int,
                                        span: span(),
                                    }),
                                    args: vec![HirArg {
                                        label: None,
                                        value: int_lit(41),
                                        spread: false,
                                    }],
                                },
                                ty: Ty::Int,
                                span: span(),
                            }),
                            span(),
                        )],
                        span: span(),
                    },
                    closure_captures: vec![],
                    is_public: true,
                    is_async: false,
                    is_mut: false,
                    is_inline: false,
                    is_no_inline: false,
                    c_export_name: None,
                    span: span(),
                },
            ],
            consts: vec![],
            externs: vec![],
        };
        optimize_module(&mut module, OptLevel::Default);
        match &module.funcs[1].body.stmts[0] {
            HirStmt::Return(Some(e), _) => {
                assert!(
                    matches!(e.kind, HirExprKind::Call { .. }),
                    "Default must not inline leafs, got {:?}",
                    e.kind
                );
            }
            other => panic!("expected Return, got {other:?}"),
        }
    }

    #[test]
    fn leaf_inline_skips_allocating_bytes_arguments() {
        let ignore_bytes = HirFunc {
            def_id: DefId(2),
            name: SmolStr::new("ignore_bytes"),
            params: vec![HirParam {
                name: SmolStr::new("value"),
                ty: Ty::Bytes,
                default: None,
                contract: None,
                variadic: false,
                span: span(),
            }],
            return_ty: Ty::Int,
            body: HirBlock {
                stmts: vec![HirStmt::Return(Some(int_lit(1)), span())],
                span: span(),
            },
            closure_captures: vec![],
            is_public: false,
            is_async: false,
            is_mut: false,
            is_inline: false,
            is_no_inline: false,
            c_export_name: None,
            span: span(),
        };
        let call = HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(HirExpr {
                    kind: HirExprKind::Var(SmolStr::new("ignore_bytes")),
                    ty: Ty::Func {
                        params: vec![Ty::Bytes],
                        ret: Box::new(Ty::Int),
                    },
                    span: span(),
                }),
                args: vec![HirArg {
                    label: None,
                    value: bytes_lit(b"payload"),
                    spread: false,
                }],
            },
            ty: Ty::Int,
            span: span(),
        };
        let mut module = HirModule {
            namespace: SmolStr::new("app"),
            structs: vec![],
            enums: vec![],
            traits: vec![],
            trait_impls: vec![],
            funcs: vec![
                ignore_bytes,
                HirFunc {
                    def_id: DefId(1),
                    name: SmolStr::new("main"),
                    params: vec![],
                    return_ty: Ty::Int,
                    body: HirBlock {
                        stmts: vec![HirStmt::Return(Some(call), span())],
                        span: span(),
                    },
                    closure_captures: vec![],
                    is_public: true,
                    is_async: false,
                    is_mut: false,
                    is_inline: false,
                    is_no_inline: false,
                    c_export_name: None,
                    span: span(),
                },
            ],
            consts: vec![],
            externs: vec![],
        };

        optimize_module(&mut module, OptLevel::Aggressive);

        assert!(
            matches!(
                &module.funcs[1].body.stmts[0],
                HirStmt::Return(
                    Some(HirExpr {
                        kind: HirExprKind::Call { .. },
                        ..
                    }),
                    _
                )
            ),
            "aggressive inlining must retain an allocating bytes argument"
        );
    }

    #[test]
    fn dce_removes_unused_pure_binding() {
        let mut module = empty_module_with_body(vec![HirStmt::Let {
            name: SmolStr::new("unused"),
            ty: Ty::Int,
            mutable: false,
            value: bin(ori_ast::expr::BinaryOp::Add, int_lit(2), int_lit(3)),
            span: span(),
        }]);

        optimize_module(&mut module, OptLevel::Default);

        assert!(
            module.funcs[0].body.stmts.is_empty(),
            "DCE should remove an unused pure binding"
        );
    }

    #[test]
    fn dce_keeps_unused_integer_division_that_may_trap() {
        let mut module = empty_module_with_body(vec![HirStmt::Let {
            name: SmolStr::new("unused"),
            ty: Ty::Int,
            mutable: false,
            value: bin(ori_ast::expr::BinaryOp::Div, int_lit(1), int_lit(0)),
            span: span(),
        }]);

        optimize_module(&mut module, OptLevel::Default);

        assert!(
            matches!(
                module.funcs[0].body.stmts.first(),
                Some(HirStmt::Let { .. })
            ),
            "DCE must retain an unused integer division because it can trap"
        );
    }

    #[test]
    fn dce_keeps_all_integer_operations_with_backend_guards() {
        for op in [
            ori_ast::expr::BinaryOp::Div,
            ori_ast::expr::BinaryOp::Rem,
            ori_ast::expr::BinaryOp::Shl,
            ori_ast::expr::BinaryOp::Shr,
        ] {
            let mut module = empty_module_with_body(vec![HirStmt::Let {
                name: SmolStr::new("unused"),
                ty: Ty::Int,
                mutable: false,
                value: bin(op, int_lit(1), int_lit(0)),
                span: span(),
            }]);

            optimize_module(&mut module, OptLevel::Default);

            assert!(
                matches!(
                    module.funcs[0].body.stmts.first(),
                    Some(HirStmt::Let { .. })
                ),
                "DCE removed guarded operation {op:?}"
            );
        }
    }

    #[test]
    fn dce_keeps_bindings_used_by_associated_call_arguments() {
        // Associated calls have no receiver expression. Their arguments still
        // read enclosing bindings and must participate in DCE's use scan.
        let associated_call = HirExpr {
            kind: HirExprKind::AssociatedCall {
                receiver_ty: Ty::Int,
                method: SmolStr::new("consume"),
                args: vec![var("argument")],
            },
            ty: Ty::Int,
            span: span(),
        };
        let mut module = empty_module_with_body(vec![
            HirStmt::Let {
                name: SmolStr::new("argument"),
                ty: Ty::Int,
                mutable: false,
                value: int_lit(7),
                span: span(),
            },
            HirStmt::Let {
                name: SmolStr::new("unused_result"),
                ty: Ty::Int,
                mutable: false,
                value: associated_call,
                span: span(),
            },
        ]);

        optimize_module(&mut module, OptLevel::Default);

        let names = module.funcs[0]
            .body
            .stmts
            .iter()
            .filter_map(|stmt| match stmt {
                HirStmt::Let { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(names, ["argument", "unused_result"]);
    }

    #[test]
    fn dce_keeps_unused_runtime_allocation() {
        let mut module = empty_module_with_body(vec![HirStmt::Let {
            name: SmolStr::new("unused"),
            ty: Ty::List(Box::new(Ty::Int)),
            mutable: false,
            value: HirExpr {
                kind: HirExprKind::ListLit {
                    elem_ty: Ty::Int,
                    elements: vec![],
                },
                ty: Ty::List(Box::new(Ty::Int)),
                span: span(),
            },
            span: span(),
        }]);

        optimize_module(&mut module, OptLevel::Default);

        assert!(
            matches!(
                module.funcs[0].body.stmts.first(),
                Some(HirStmt::Let { .. })
            ),
            "DCE must retain an unused runtime allocation"
        );
    }

    #[test]
    fn dce_keeps_unused_bytes_literal_allocation() {
        let mut module = empty_module_with_body(vec![HirStmt::Let {
            name: SmolStr::new("unused"),
            ty: Ty::Bytes,
            mutable: false,
            value: bytes_lit(b"payload"),
            span: span(),
        }]);

        optimize_module(&mut module, OptLevel::Default);

        assert!(
            matches!(
                module.funcs[0].body.stmts.first(),
                Some(HirStmt::Let { .. })
            ),
            "DCE must retain a bytes literal because native codegen allocates it"
        );
    }
}
