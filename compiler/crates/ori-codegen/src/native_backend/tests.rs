use super::*;
use ori_diagnostics::Span;
use ori_types::{stdlib::stdlib_native_abi, DefId};
use std::collections::{BTreeSet, HashSet};

struct NativeHirCoverage {
    variant: &'static str,
    evidence: &'static [&'static str],
    note: &'static str,
}

const NATIVE_EXPR_COVERAGE: &[NativeHirCoverage] = &[
    NativeHirCoverage {
        variant: "BoolLit",
        evidence: &["HirExprKind::BoolLit"],
        note: "literal bool direto",
    },
    NativeHirCoverage {
        variant: "IntLit",
        evidence: &["HirExprKind::IntLit"],
        note: "literal inteiro direto",
    },
    NativeHirCoverage {
        variant: "FloatLit",
        evidence: &["HirExprKind::FloatLit"],
        note: "literal float direto",
    },
    NativeHirCoverage {
        variant: "StrLit",
        evidence: &["HirExprKind::StrLit"],
        note: "string constante gerenciada pelo runtime",
    },
    NativeHirCoverage {
        variant: "InterpolatedStr",
        evidence: &["HirExprKind::InterpolatedStr"],
        note: "interpolacao via helper de partes string",
    },
    NativeHirCoverage {
        variant: "BytesLit",
        evidence: &["HirExprKind::BytesLit"],
        note: "bytes gerenciados pelo runtime",
    },
    NativeHirCoverage {
        variant: "Unit",
        evidence: &["HirExprKind::Unit"],
        note: "valor direto sem payload",
    },
    NativeHirCoverage {
        variant: "Var",
        evidence: &["HirExprKind::Var"],
        note: "local, global ou constante",
    },
    NativeHirCoverage {
        variant: "Binary",
        evidence: &["HirExprKind::Binary"],
        note: "operadores escalares e chamadas auxiliares",
    },
    NativeHirCoverage {
        variant: "Unary",
        evidence: &["HirExprKind::Unary"],
        note: "negacao numerica e logica",
    },
    NativeHirCoverage {
        variant: "Field",
        evidence: &["HirExprKind::Field"],
        note: "load por layout nativo de struct",
    },
    NativeHirCoverage {
        variant: "Index",
        evidence: &["HirExprKind::Index"],
        note: "list, string e bytes",
    },
    NativeHirCoverage {
        variant: "TupleIndex",
        evidence: &["HirExprKind::TupleIndex"],
        note: "load por layout nativo de tuple",
    },
    NativeHirCoverage {
        variant: "Call",
        evidence: &["HirExprKind::Call"],
        note: "funcoes Ori, runtime e closures",
    },
    NativeHirCoverage {
        variant: "MethodCall",
        evidence: &["HirExprKind::MethodCall"],
        note: "slice, trait object e chamada resolvida",
    },
    NativeHirCoverage {
        variant: "StructLit",
        evidence: &["HirExprKind::StructLit"],
        note: "alloc + stores por layout nativo",
    },
    NativeHirCoverage {
        variant: "EnumVariant",
        evidence: &["HirExprKind::EnumVariant"],
        note: "tag + payload por layout nativo",
    },
    NativeHirCoverage {
        variant: "ListLit",
        evidence: &["HirExprKind::ListLit"],
        note: "ori.list runtime",
    },
    NativeHirCoverage {
        variant: "ListSpreadLit",
        evidence: &["HirExprKind::ListSpreadLit"],
        note: "ori.list runtime com spread",
    },
    NativeHirCoverage {
        variant: "TupleLit",
        evidence: &["HirExprKind::TupleLit"],
        note: "alloc + stores por layout nativo",
    },
    NativeHirCoverage {
        variant: "Some_",
        evidence: &["HirExprKind::Some_"],
        note: "optional runtime-managed",
    },
    NativeHirCoverage {
        variant: "None_",
        evidence: &["HirExprKind::None_"],
        note: "optional runtime-managed sem payload",
    },
    NativeHirCoverage {
        variant: "Ok_",
        evidence: &["HirExprKind::Ok_"],
        note: "result runtime-managed ok",
    },
    NativeHirCoverage {
        variant: "Err_",
        evidence: &["HirExprKind::Err_"],
        note: "result runtime-managed err",
    },
    NativeHirCoverage {
        variant: "Propagate",
        evidence: &["HirExprKind::Propagate"],
        note: "`?` em optional/result",
    },
    NativeHirCoverage {
        variant: "Await",
        evidence: &["HirExprKind::Await"],
        note: "executor minimo atual",
    },
    NativeHirCoverage {
        variant: "IfExpr",
        evidence: &["HirExprKind::IfExpr"],
        note: "select Cranelift",
    },
    NativeHirCoverage {
        variant: "MatchExpr",
        evidence: &["HirExprKind::MatchExpr", "self.emit_match_expr("],
        note: "bracos convergem num block param (phi); resultado sempre owned",
    },
    NativeHirCoverage {
        variant: "Range",
        evidence: &["HirExprKind::Range"],
        note: "handle runtime-managed start/end",
    },
    NativeHirCoverage {
        variant: "MapLit",
        evidence: &["HirExprKind::MapLit"],
        note: "ori.map runtime",
    },
    NativeHirCoverage {
        variant: "SetLit",
        evidence: &["HirExprKind::SetLit"],
        note: "ori.set runtime",
    },
    NativeHirCoverage {
        variant: "StructUpdate",
        evidence: &["HirExprKind::StructUpdate"],
        note: "copy + override por layout nativo",
    },
    NativeHirCoverage {
        variant: "Closure",
        evidence: &["HirExprKind::Closure"],
        note: "closure object com ambiente capturado",
    },
    NativeHirCoverage {
        variant: "IsCheck",
        evidence: &["HirExprKind::IsCheck"],
        note: "`is` estatico ou via vtable any<Trait>",
    },
];

fn hir_expr(kind: HirExprKind, ty: Ty) -> HirExpr {
    HirExpr {
        kind,
        ty,
        span: Span::DUMMY,
    }
}

fn simple_async_func(stmts: Vec<HirStmt>) -> HirFunc {
    HirFunc {
        def_id: DefId(0),
        name: SmolStr::new("app.delayed"),
        params: Vec::new(),
        return_ty: Ty::Future(Box::new(Ty::Int)),
        body: HirBlock {
            stmts,
            span: Span::DUMMY,
        },
        closure_captures: Vec::new(),
        is_public: false,
        is_async: true,
        is_mut: false,
        c_export_name: None,
        span: Span::DUMMY,
    }
}

#[test]
fn simple_async_state_machine_plan_accepts_single_await_call_then_return() {
    let callee = hir_expr(HirExprKind::Var(SmolStr::new("task.sleep")), Ty::Int);
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(callee),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Void)),
    );
    let await_expr = hir_expr(HirExprKind::Await(Box::new(await_future)), Ty::Void);
    let ret = hir_expr(HirExprKind::IntLit(41), Ty::Int);
    let func = simple_async_func(vec![
        HirStmt::Expr(await_expr),
        HirStmt::Return(Some(ret), Span::DUMMY),
    ]);

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.inner_ty, Ty::Int);
    assert_eq!(plan.awaits.len(), 1);
    assert!(matches!(
        plan.awaits[0].await_future.kind,
        HirExprKind::Call { .. }
    ));
    assert!(matches!(
        plan.return_expr.as_ref().map(|expr| &expr.kind),
        Some(HirExprKind::IntLit(41))
    ));
}

#[test]
fn simple_async_state_machine_plan_accepts_scalar_await_binding_then_return() {
    let callee = hir_expr(HirExprKind::Var(SmolStr::new("delayed")), Ty::Int);
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(callee),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Int)),
    );
    let await_expr = hir_expr(HirExprKind::Await(Box::new(await_future)), Ty::Int);
    let ret = hir_expr(HirExprKind::Var(SmolStr::new("value")), Ty::Int);
    let func = simple_async_func(vec![
        HirStmt::Let {
            name: SmolStr::new("value"),
            ty: Ty::Int,
            mutable: false,
            value: await_expr,
            span: Span::DUMMY,
        },
        HirStmt::Return(Some(ret), Span::DUMMY),
    ]);

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    let binding = plan.awaits[0].binding.as_ref().expect("await binding");
    assert_eq!(binding.name.as_str(), "value");
    assert_eq!(binding.ty, Ty::Int);
}

#[test]
fn simple_async_state_machine_plan_accepts_two_scalar_await_bindings_then_return() {
    let left_call = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(HirExprKind::Var(SmolStr::new("left")), Ty::Int)),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Int)),
    );
    let right_call = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(HirExprKind::Var(SmolStr::new("right")), Ty::Int)),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Int)),
    );
    let ret = hir_expr(
        HirExprKind::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(hir_expr(HirExprKind::Var(SmolStr::new("a")), Ty::Int)),
            rhs: Box::new(hir_expr(HirExprKind::Var(SmolStr::new("b")), Ty::Int)),
        },
        Ty::Int,
    );
    let func = simple_async_func(vec![
        HirStmt::Let {
            name: SmolStr::new("a"),
            ty: Ty::Int,
            mutable: false,
            value: hir_expr(HirExprKind::Await(Box::new(left_call)), Ty::Int),
            span: Span::DUMMY,
        },
        HirStmt::Let {
            name: SmolStr::new("b"),
            ty: Ty::Int,
            mutable: false,
            value: hir_expr(HirExprKind::Await(Box::new(right_call)), Ty::Int),
            span: Span::DUMMY,
        },
        HirStmt::Return(Some(ret), Span::DUMMY),
    ]);

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.awaits.len(), 2);
    assert_eq!(plan.awaits[0].binding.as_ref().unwrap().name.as_str(), "a");
    assert_eq!(plan.awaits[1].binding.as_ref().unwrap().name.as_str(), "b");
}

#[test]
fn simple_async_state_machine_plan_accepts_void_tail_expression() {
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(
                HirExprKind::Var(SmolStr::new("task.sleep")),
                Ty::Int,
            )),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Void)),
    );
    let func = HirFunc {
        def_id: DefId(0),
        name: SmolStr::new("app.main"),
        params: Vec::new(),
        return_ty: Ty::Future(Box::new(Ty::Void)),
        body: HirBlock {
            stmts: vec![
                HirStmt::Expr(hir_expr(
                    HirExprKind::Await(Box::new(await_future)),
                    Ty::Void,
                )),
                HirStmt::Expr(hir_expr(HirExprKind::Unit, Ty::Void)),
            ],
            span: Span::DUMMY,
        },
        closure_captures: Vec::new(),
        is_public: false,
        is_async: true,
        is_mut: false,
        c_export_name: None,
        span: Span::DUMMY,
    };

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.awaits.len(), 1);
    assert!(plan.tail_expr.is_some());
}

#[test]
fn simple_async_state_machine_plan_accepts_prefix_local_and_tail_control_flow() {
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(
                HirExprKind::Var(SmolStr::new("task.sleep")),
                Ty::Int,
            )),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Void)),
    );
    let func = HirFunc {
        def_id: DefId(0),
        name: SmolStr::new("app.main"),
        params: Vec::new(),
        return_ty: Ty::Future(Box::new(Ty::Void)),
        body: HirBlock {
            stmts: vec![
                HirStmt::Let {
                    name: SmolStr::new("label"),
                    ty: Ty::String,
                    mutable: false,
                    value: hir_expr(HirExprKind::StrLit(SmolStr::new("ready")), Ty::String),
                    span: Span::DUMMY,
                },
                HirStmt::Expr(hir_expr(
                    HirExprKind::Await(Box::new(await_future)),
                    Ty::Void,
                )),
                HirStmt::If {
                    cond: hir_expr(HirExprKind::BoolLit(true), Ty::Bool),
                    then: HirBlock {
                        stmts: vec![HirStmt::Expr(hir_expr(
                            HirExprKind::Var(SmolStr::new("label")),
                            Ty::String,
                        ))],
                        span: Span::DUMMY,
                    },
                    else_ifs: Vec::new(),
                    else_: None,
                    span: Span::DUMMY,
                },
            ],
            span: Span::DUMMY,
        },
        closure_captures: Vec::new(),
        is_public: false,
        is_async: true,
        is_mut: false,
        c_export_name: None,
        span: Span::DUMMY,
    };

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.locals.len(), 1);
    assert_eq!(plan.locals[0].name.as_str(), "label");
    assert_eq!(plan.awaits.len(), 1);
    assert_eq!(plan.tail_stmts.len(), 1);
    assert!(plan.tail_expr.is_some());
}

#[test]
fn simple_async_state_machine_plan_accepts_scalar_params() {
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(
                HirExprKind::Var(SmolStr::new("task.sleep")),
                Ty::Int,
            )),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::Void)),
    );
    let ret = hir_expr(HirExprKind::Var(SmolStr::new("base")), Ty::Int);
    let mut func = simple_async_func(vec![
        HirStmt::Expr(hir_expr(
            HirExprKind::Await(Box::new(await_future)),
            Ty::Void,
        )),
        HirStmt::Return(Some(ret), Span::DUMMY),
    ]);
    func.params.push(HirParam {
        name: SmolStr::new("base"),
        ty: Ty::Int,
        default: None,
        contract: None,
        variadic: false,
        span: Span::DUMMY,
    });

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.params.len(), 1);
    assert_eq!(plan.params[0].name.as_str(), "base");
}

#[test]
fn simple_async_state_machine_plan_accepts_managed_param_and_binding() {
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(HirExprKind::Var(SmolStr::new("load")), Ty::String)),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::String)),
    );
    let mut func = simple_async_func(vec![
        HirStmt::Let {
            name: SmolStr::new("loaded"),
            ty: Ty::String,
            mutable: false,
            value: hir_expr(HirExprKind::Await(Box::new(await_future)), Ty::String),
            span: Span::DUMMY,
        },
        HirStmt::Return(
            Some(hir_expr(
                HirExprKind::Var(SmolStr::new("loaded")),
                Ty::String,
            )),
            Span::DUMMY,
        ),
    ]);
    func.return_ty = Ty::Future(Box::new(Ty::String));
    func.params.push(HirParam {
        name: SmolStr::new("prefix"),
        ty: Ty::String,
        default: None,
        contract: None,
        variadic: false,
        span: Span::DUMMY,
    });

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.params[0].ty, Ty::String);
    assert_eq!(plan.awaits[0].binding.as_ref().unwrap().ty, Ty::String);
}

#[test]
fn simple_async_liveness_marks_dead_and_live_frame_values_after_await() {
    let first_call = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(HirExprKind::Var(SmolStr::new("load")), Ty::String)),
            args: Vec::new(),
        },
        Ty::Future(Box::new(Ty::String)),
    );
    let second_call = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(
                HirExprKind::Var(SmolStr::new("consume")),
                Ty::String,
            )),
            args: vec![HirArg {
                label: None,
                spread: false,
                value: hir_expr(HirExprKind::Var(SmolStr::new("first")), Ty::String),
            }],
        },
        Ty::Future(Box::new(Ty::String)),
    );
    let mut func = simple_async_func(vec![
        HirStmt::Let {
            name: SmolStr::new("dead_before_resume"),
            ty: Ty::String,
            mutable: false,
            value: hir_expr(HirExprKind::StrLit(SmolStr::new("drop")), Ty::String),
            span: Span::DUMMY,
        },
        HirStmt::Let {
            name: SmolStr::new("first"),
            ty: Ty::String,
            mutable: false,
            value: hir_expr(HirExprKind::Await(Box::new(first_call)), Ty::String),
            span: Span::DUMMY,
        },
        HirStmt::Let {
            name: SmolStr::new("second"),
            ty: Ty::String,
            mutable: false,
            value: hir_expr(HirExprKind::Await(Box::new(second_call)), Ty::String),
            span: Span::DUMMY,
        },
        HirStmt::Return(
            Some(hir_expr(
                HirExprKind::Var(SmolStr::new("second")),
                Ty::String,
            )),
            Span::DUMMY,
        ),
    ]);
    func.return_ty = Ty::Future(Box::new(Ty::String));
    func.params.push(HirParam {
        name: SmolStr::new("unused_param"),
        ty: Ty::String,
        default: None,
        contract: None,
        variadic: false,
        span: Span::DUMMY,
    });

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert!(!simple_async_name_used_after_await(
        &plan,
        &SmolStr::new("dead_before_resume"),
        0
    ));
    assert!(!simple_async_name_used_after_await(
        &plan,
        &SmolStr::new("unused_param"),
        0
    ));
    assert!(simple_async_name_used_after_await(
        &plan,
        &SmolStr::new("first"),
        0
    ));
    assert!(!simple_async_name_used_after_await(
        &plan,
        &SmolStr::new("first"),
        1
    ));
    assert!(simple_async_name_used_after_await(
        &plan,
        &SmolStr::new("second"),
        1
    ));
}

#[test]
fn simple_async_state_machine_plan_accepts_nested_await_return_expression() {
    let callee = hir_expr(HirExprKind::Var(SmolStr::new("compute")), Ty::Int);
    let awaited_value = hir_expr(
        HirExprKind::Await(Box::new(hir_expr(
            HirExprKind::Call {
                callee: Box::new(callee),
                args: Vec::new(),
            },
            Ty::Future(Box::new(Ty::Int)),
        ))),
        Ty::Int,
    );
    let awaited_return = hir_expr(
        HirExprKind::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(awaited_value),
            rhs: Box::new(hir_expr(HirExprKind::IntLit(1), Ty::Int)),
        },
        Ty::Int,
    );
    let first_await = hir_expr(
        HirExprKind::Await(Box::new(hir_expr(
            HirExprKind::Call {
                callee: Box::new(hir_expr(
                    HirExprKind::Var(SmolStr::new("task.sleep")),
                    Ty::Int,
                )),
                args: Vec::new(),
            },
            Ty::Future(Box::new(Ty::Void)),
        ))),
        Ty::Void,
    );
    let func = simple_async_func(vec![
        HirStmt::Expr(first_await),
        HirStmt::Return(Some(awaited_return), Span::DUMMY),
    ]);

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.awaits.len(), 2);
    assert!(matches!(
        plan.return_expr.as_ref().map(|expr| &expr.kind),
        Some(HirExprKind::Binary { .. })
    ));
}

#[test]
fn simple_async_state_machine_plan_accepts_return_await_call() {
    let callee = hir_expr(HirExprKind::Var(SmolStr::new("compute")), Ty::Int);
    let awaited_return = hir_expr(
        HirExprKind::Await(Box::new(hir_expr(
            HirExprKind::Call {
                callee: Box::new(callee),
                args: Vec::new(),
            },
            Ty::Future(Box::new(Ty::Int)),
        ))),
        Ty::Int,
    );
    let func = simple_async_func(vec![HirStmt::Return(Some(awaited_return), Span::DUMMY)]);

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.awaits.len(), 1);

    let first_await = hir_expr(
        HirExprKind::Await(Box::new(hir_expr(
            HirExprKind::Call {
                callee: Box::new(hir_expr(
                    HirExprKind::Var(SmolStr::new("task.sleep")),
                    Ty::Int,
                )),
                args: Vec::new(),
            },
            Ty::Future(Box::new(Ty::Void)),
        ))),
        Ty::Void,
    );
    let func = simple_async_func(vec![
        HirStmt::Expr(first_await),
        HirStmt::Return(
            Some(hir_expr(
                HirExprKind::Await(Box::new(hir_expr(
                    HirExprKind::Call {
                        callee: Box::new(hir_expr(
                            HirExprKind::Var(SmolStr::new("compute")),
                            Ty::Int,
                        )),
                        args: Vec::new(),
                    },
                    Ty::Future(Box::new(Ty::Int)),
                ))),
                Ty::Int,
            )),
            Span::DUMMY,
        ),
    ]);

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.awaits.len(), 2);
    assert!(matches!(
        plan.return_expr.as_ref().map(|expr| &expr.kind),
        Some(HirExprKind::Var(name)) if name.as_str() == ".__async_return_value"
    ));
}

#[test]
fn simple_async_state_machine_plan_accepts_await_result_question_mark_binding() {
    let result_ty = Ty::Result(Box::new(Ty::Int), Box::new(Ty::String));
    let await_future = hir_expr(
        HirExprKind::Call {
            callee: Box::new(hir_expr(
                HirExprKind::Var(SmolStr::new("compute")),
                result_ty.clone(),
            )),
            args: Vec::new(),
        },
        Ty::Future(Box::new(result_ty.clone())),
    );
    let await_expr = hir_expr(
        HirExprKind::Await(Box::new(await_future)),
        result_ty.clone(),
    );
    let propagate = hir_expr(HirExprKind::Propagate(Box::new(await_expr)), Ty::Int);
    let ret = hir_expr(
        HirExprKind::Ok_(Box::new(hir_expr(
            HirExprKind::Var(SmolStr::new("value")),
            Ty::Int,
        ))),
        result_ty.clone(),
    );
    let mut func = simple_async_func(vec![
        HirStmt::Let {
            name: SmolStr::new("value"),
            ty: Ty::Int,
            mutable: false,
            value: propagate,
            span: Span::DUMMY,
        },
        HirStmt::Return(Some(ret), Span::DUMMY),
    ]);
    func.return_ty = Ty::Future(Box::new(result_ty));

    let plan = simple_async_state_machine_plan(&func).expect("simple async state machine plan");
    assert_eq!(plan.awaits.len(), 1);
    assert!(plan.awaits[0].propagate_result_ty.is_some());
}

const NATIVE_STMT_COVERAGE: &[NativeHirCoverage] = &[
    NativeHirCoverage {
        variant: "Let",
        evidence: &["HirStmt::Let"],
        note: "binding local com ARC lexical",
    },
    NativeHirCoverage {
        variant: "Assign",
        evidence: &["HirStmt::Assign"],
        note: "var, campo e indice list",
    },
    NativeHirCoverage {
        variant: "Return",
        evidence: &["HirStmt::Return"],
        note: "retorno direto, future ready e cleanup",
    },
    NativeHirCoverage {
        variant: "Break",
        evidence: &["HirStmt::Break"],
        note: "salto com cleanup de escopo",
    },
    NativeHirCoverage {
        variant: "Continue",
        evidence: &["HirStmt::Continue"],
        note: "salto com cleanup de escopo",
    },
    NativeHirCoverage {
        variant: "Expr",
        evidence: &["HirStmt::Expr"],
        note: "expressao com descarte do resultado",
    },
    NativeHirCoverage {
        variant: "If",
        evidence: &["HirStmt::If"],
        note: "branches nativos",
    },
    NativeHirCoverage {
        variant: "While",
        evidence: &["HirStmt::While"],
        note: "loop condicional",
    },
    NativeHirCoverage {
        variant: "For",
        evidence: &["HirStmt::For"],
        note: "range, list, set, map, string e bytes",
    },
    NativeHirCoverage {
        variant: "Loop",
        evidence: &["HirStmt::Loop"],
        note: "loop infinito com break/continue",
    },
    NativeHirCoverage {
        variant: "Repeat",
        evidence: &["HirStmt::Repeat"],
        note: "contador i64 com trap para negativo",
    },
    NativeHirCoverage {
        variant: "Match",
        evidence: &["HirStmt::Match"],
        note: "patterns HIR suportados pelo binder nativo",
    },
    NativeHirCoverage {
        variant: "IfSome",
        evidence: &["HirStmt::IfSome"],
        note: "desempacotamento optional",
    },
    NativeHirCoverage {
        variant: "WhileSome",
        evidence: &["HirStmt::WhileSome"],
        note: "loop optional",
    },
    NativeHirCoverage {
        variant: "Using",
        evidence: &["HirStmt::Using"],
        note: "cleanup lexical de recurso",
    },
    NativeHirCoverage {
        variant: "Check",
        evidence: &["HirStmt::Check"],
        note: "runtime trap em contrato/check",
    },
];

#[test]
fn native_backend_declares_manifest_runtime_symbols() {
    let mut checked = HashSet::new();
    let mut missing = Vec::new();
    for entry in stdlib_runtime_functions()
        .iter()
        .filter(|entry| entry.native_runtime)
    {
        if checked.insert(entry.runtime_symbol) && stdlib_native_abi(entry.runtime_symbol).is_none()
        {
            missing.push(entry.runtime_symbol);
        }
    }

    assert!(
        missing.is_empty(),
        "manifest runtime symbols missing native backend ABI metadata: {missing:#?}"
    );
}

#[test]
fn direct_internal_runtime_imports_are_documented() {
    let source = include_str!("../native_backend.rs");
    let manifest_symbols: HashSet<_> = stdlib_runtime_functions()
        .iter()
        .filter(|entry| entry.native_runtime)
        .map(|entry| entry.runtime_symbol)
        .collect();
    let internal_symbols: HashSet<_> = INTERNAL_NATIVE_RUNTIME_IMPORTS.iter().copied().collect();
    let direct_imports = direct_declared_runtime_symbols(source);
    let mut undocumented = Vec::new();

    for symbol in direct_imports {
        if symbol.starts_with("ori_")
            && !manifest_symbols.contains(symbol.as_str())
            && !internal_symbols.contains(symbol.as_str())
        {
            undocumented.push(symbol);
        }
    }

    undocumented.sort();
    undocumented.dedup();
    assert!(
            undocumented.is_empty(),
            "direct native runtime imports outside the stdlib manifest must be documented as internal helpers: {undocumented:#?}"
        );
}

#[test]
fn native_runtime_imports_are_deduplicated_before_cranelift_declaration() {
    let source = include_str!("../native_backend.rs");
    let dedup = source
        .find("if let Some(existing) = declared_imports.get(name).copied()")
        .expect("declare_stdlib must check for existing native imports");
    let declaration = source
        .find(".declare_function(name, Linkage::Import, &sig)")
        .expect("declare_stdlib must declare Cranelift imports");

    assert!(
        dedup < declaration,
        "native runtime imports must be deduplicated before calling Cranelift declare_function"
    );
}

fn direct_declared_runtime_symbols(source: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find("decl(") {
        rest = &rest[index + "decl(".len()..];
        let after_decl = rest.trim_start();
        let Some(after_quote) = after_decl.strip_prefix('"') else {
            continue;
        };
        if let Some((symbol, tail)) = after_quote.split_once('"') {
            symbols.push(symbol.to_string());
            rest = tail;
        } else {
            break;
        }
    }
    symbols
}

#[test]
fn native_hir_expression_coverage_matrix_matches_hir_enum() {
    let hir_source = include_str!("../../../ori-hir/src/hir.rs");
    let actual = enum_variant_names(hir_source, "HirExprKind");
    let documented = coverage_variants(NATIVE_EXPR_COVERAGE);

    assert_eq!(
        documented, actual,
        "native HIR expression coverage must be updated when HirExprKind changes"
    );
}

#[test]
fn native_hir_statement_coverage_matrix_matches_hir_enum() {
    let hir_source = include_str!("../../../ori-hir/src/hir.rs");
    let actual = enum_variant_names(hir_source, "HirStmt");
    let documented = coverage_variants(NATIVE_STMT_COVERAGE);

    assert_eq!(
        documented, actual,
        "native HIR statement coverage must be updated when HirStmt changes"
    );
}

#[test]
fn native_hir_expression_coverage_has_codegen_evidence() {
    let source = include_str!("../native_backend.rs");
    let emit_expr = source_section(source, "fn emit_expr(", "fn str_len_from_ptr");

    for entry in NATIVE_EXPR_COVERAGE {
        assert!(
            !entry.note.trim().is_empty(),
            "missing note for {}",
            entry.variant
        );
        for marker in entry.evidence {
            assert!(
                emit_expr.contains(marker),
                "coverage entry `{}` points at missing native expression marker `{}`",
                entry.variant,
                marker
            );
        }
    }
}

#[test]
fn native_hir_statement_coverage_has_codegen_evidence() {
    let source = include_str!("../native_backend.rs");
    let emit_stmt = source_section(source, "fn emit_stmt(", "fn emit_if(");

    for entry in NATIVE_STMT_COVERAGE {
        assert!(
            !entry.note.trim().is_empty(),
            "missing note for {}",
            entry.variant
        );
        for marker in entry.evidence {
            assert!(
                emit_stmt.contains(marker),
                "coverage entry `{}` points at missing native statement marker `{}`",
                entry.variant,
                marker
            );
        }
    }
}

#[test]
fn native_string_collectors_are_exhaustive_over_hir_shapes() {
    let source = include_str!("string_collector.rs");
    let expr_collector = source_section(
        source,
        "fn collect_strings_expr",
        "fn collect_strings_block",
    );
    let stmt_collector =
        source_section(source, "fn collect_strings_stmt", "fn collect_all_strings");
    let pattern_collector = source_section(
        source,
        "fn collect_strings_pattern",
        "pub(super) fn collect_all_strings",
    );

    for (name, section) in [
        ("collect_strings_expr", expr_collector),
        ("collect_strings_stmt", stmt_collector),
        ("collect_strings_pattern", pattern_collector),
    ] {
        assert!(
            !section
                .lines()
                .any(|line| line.trim_start().starts_with("_ =>")),
            "{name} must stay exhaustive; wildcard arms silently hide new HIR variants"
        );
    }
}

#[test]
fn native_codegen_unsupported_errors_are_coded() {
    let message = native_codegen_unsupported("example native gap");
    assert!(
        message.starts_with("backend.native_unsupported:"),
        "{message}"
    );

    let source = source_section(
        include_str!("../native_backend.rs"),
        "// == Type mapping ==",
        "#[cfg(test)]",
    );
    for raw in [
        "\"unsupported indexed assignment base",
        "\"unsupported `for` iterable type",
        "\"unsupported map runtime call",
        "\"unsupported set runtime call",
    ] {
        assert!(
            !source.contains(raw),
            "native unsupported path must use backend.native_unsupported helper: {raw}"
        );
    }
}

fn coverage_variants(entries: &[NativeHirCoverage]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| entry.variant.to_string())
        .collect()
}

fn enum_variant_names(source: &str, enum_name: &str) -> BTreeSet<String> {
    let marker = format!("pub enum {enum_name} {{");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("enum `{enum_name}` not found"));
    let body_start = start + marker.len();
    let body_end = matching_brace(source, body_start - 1)
        .unwrap_or_else(|| panic!("enum `{enum_name}` has no matching closing brace"));
    let body = strip_rust_line_comments(&source[body_start..body_end]);
    split_top_level_enum_items(&body)
        .into_iter()
        .filter_map(|item| leading_identifier(&item))
        .collect()
}

fn strip_rust_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split_once("//").map(|(code, _)| code).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn matching_brace(source: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in source[open_index..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_index + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_enum_items(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;

    for ch in body.chars() {
        match ch {
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ',' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                if !current.trim().is_empty() {
                    items.push(current.clone());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }

    if !current.trim().is_empty() {
        items.push(current);
    }
    items
}

fn leading_identifier(item: &str) -> Option<String> {
    let cleaned = item
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("//")
                && !line.starts_with("///")
                && !line.starts_with("#[")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut ident = String::new();
    for ch in cleaned.chars().skip_while(|ch| ch.is_whitespace()) {
        if ident.is_empty() {
            if ch == '_' || ch.is_ascii_alphabetic() {
                ident.push(ch);
            } else {
                return None;
            }
        } else if ch == '_' || ch.is_ascii_alphanumeric() {
            ident.push(ch);
        } else {
            break;
        }
    }
    (!ident.is_empty()).then_some(ident)
}

#[test]
fn missing_raw_native_linker_reports_native_linker_not_c_compiler() {
    let err = link_with_raw_native_command(
        Path::new("__ori_missing_native_linker_for_test__"),
        Path::new("input.o"),
        Path::new("output.exe"),
        &[],
        NativeLinkOptions::default(),
    )
    .expect_err("missing raw linker should produce an error");

    assert!(err.contains("native.linker_missing"), "{err}");
    assert!(err.contains("native linker"), "{err}");
    assert!(!err.contains("C compiler"), "{err}");
    assert!(!err.contains("C toolchain"), "{err}");
}

#[test]
fn env_flag_treats_truthy_values_as_set() {
    // `env_flag` reads the process env, so we cannot assert positive cases
    // without racing with parallel tests. We can at least assert that an
    // unset variable returns false.
    let name = "__ORI_ENV_FLAG_UNSET_FOR_TEST__";
    assert!(!env_flag(name), "unset env flag should be false");
}

#[test]
fn msvc_arch_dir_matches_target_pointer_width() {
    let arch = msvc_arch_dir();
    if cfg!(target_pointer_width = "64") {
        assert_eq!(arch, "x64", "64-bit targets should use x64 MSVC lib dir");
    } else if cfg!(target_pointer_width = "32") {
        assert_eq!(arch, "x86", "32-bit targets should use x86 MSVC lib dir");
    }
}

#[test]
fn discover_bundled_rust_lld_next_to_exe_returns_none_when_absent() {
    // The current test binary almost certainly does not have bundled
    // `rust-lld`/`rust-lld.exe`, so this should return None. If a future test
    // environment bundles rust-lld next to every exe, this test would need to
    // be adjusted.
    let result = discover_bundled_rust_lld_next_to_exe();
    // We cannot assert `None` unconditionally because some test runners copy
    // binaries into shared bin dirs. The invariant we care about is that the
    // function does not panic and returns a PathBuf only when the file exists.
    if let Some(path) = result {
        assert!(
            path.is_file(),
            "returned path must exist: {}",
            path.display()
        );
    }
}

#[test]
fn discover_bundled_rust_lld_supports_release_runtime_bin_layout() {
    // Discovery only accepts a candidate that runs `--version` successfully
    // (validate_rust_lld_runs), so the fixture must be executable. There is no
    // portable way to fake a runnable `.exe`, so this scenario is unix-only.
    #[cfg(not(unix))]
    {
        return;
    }
    #[cfg(unix)]
    {
        let dir =
            std::env::temp_dir().join(format!("ori_lld_release_layout_{}", std::process::id()));
        let bin_dir = dir.join("runtime").join("bin");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&bin_dir).unwrap();

        let lld = bin_dir.join("rust-lld");
        std::fs::write(&lld, b"#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&lld, std::fs::Permissions::from_mode(0o755)).unwrap();

        let discovered = discover_bundled_rust_lld_from_exe_dir(&dir)
            .expect("release package runtime/bin layout should be discovered");
        assert_eq!(discovered, lld);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(windows)]
#[test]
fn vswhere_discovers_vs_install_or_reports_clear_error() {
    match find_vs_install_via_vswhere() {
        Ok(path) => {
            assert!(
                path.is_dir(),
                "vswhere path should be a directory: {}",
                path.display()
            );
        }
        Err(reason) => {
            // The error must mention vswhere or Visual Studio so users know
            // what to install; it must not be an opaque diagnostic.
            assert!(
                reason.contains("vswhere") || reason.contains("Visual Studio"),
                "vswhere error should be actionable: {reason}"
            );
        }
    }
}

#[cfg(windows)]
#[test]
fn msvc_crt_lib_dirs_resolve_to_existing_directories() {
    // On a Windows machine with VS Build Tools installed (the CI baseline),
    // CRT discovery must succeed and return three existing lib directories.
    // On machines without VS, this test is a no-op that records the missing
    // toolchain — it does not fail the suite.
    match discover_msvc_crt_lib_dirs() {
        Ok(dirs) => {
            assert_eq!(dirs.len(), 3, "MSVC CRT discovery should return 3 lib dirs");
            for dir in &dirs {
                assert!(dir.is_dir(), "lib dir must exist: {}", dir.display());
            }
        }
        Err(reason) => {
            // Do not fail: this test machine may not have VS Build Tools.
            // We still assert the error is actionable.
            assert!(
                reason.contains("vswhere")
                    || reason.contains("Visual Studio")
                    || reason.contains("Windows SDK")
                    || reason.contains("MSVC"),
                "CRT discovery error should be actionable: {reason}"
            );
        }
    }
}

#[test]
fn bundled_rust_lld_strategy_falls_back_on_non_windows() {
    // On Windows MSVC, Linux GNU, and macOS, the bundled strategy should
    // engage (or return an actionable error if the toolchain is missing).
    // On other platforms, it must return a "not implemented" error so
    // callers fall back to the default RustcDriver.
    match discover_bundled_rust_lld() {
        Ok(strategy) => match strategy {
            NativeLinkerStrategy::BundledRustLld {
                flavor,
                lib_dirs,
                dynamic_linker,
                extra_args,
                ..
            } => {
                if cfg!(windows) {
                    assert_eq!(flavor, "link", "Windows MSVC should use link flavor");
                    assert!(!lib_dirs.is_empty(), "lib_dirs must be populated");
                    assert!(dynamic_linker.is_none(), "Windows has no dynamic linker");
                } else if cfg!(target_os = "linux") {
                    assert_eq!(flavor, "gnu", "Linux GNU should use gnu flavor");
                    assert!(!lib_dirs.is_empty(), "lib_dirs must be populated");
                    assert!(dynamic_linker.is_some(), "Linux must have a dynamic linker");
                } else if cfg!(target_os = "macos") {
                    assert_eq!(flavor, "darwin", "macOS should use darwin flavor");
                    assert!(
                        extra_args.iter().any(|a| a == "-arch"),
                        "macOS extra_args must include -arch"
                    );
                    assert!(
                        extra_args.iter().any(|a| a == "-platform_version"),
                        "macOS extra_args must include -platform_version"
                    );
                    assert!(
                        extra_args.iter().any(|a| a == "-syslibroot"),
                        "macOS extra_args must include -syslibroot"
                    );
                    assert!(
                        dynamic_linker.is_none(),
                        "macOS has no explicit dynamic linker"
                    );
                }
            }
            _ => panic!("discover_bundled_rust_lld returned wrong strategy variant"),
        },
        Err(reason) => {
            if cfg!(windows) {
                // On Windows, the error must be actionable (VS/SDK missing).
                assert!(
                    reason.contains("vswhere")
                        || reason.contains("Visual Studio")
                        || reason.contains("Windows SDK")
                        || reason.contains("MSVC")
                        || reason.contains("rust-lld"),
                    "Windows error should be actionable: {reason}"
                );
            } else if cfg!(target_os = "linux") {
                // On Linux, the error must mention cc or libc or rust-lld.
                assert!(
                    reason.contains("cc")
                        || reason.contains("libc")
                        || reason.contains("rust-lld")
                        || reason.contains("dynamic linker"),
                    "Linux error should be actionable: {reason}"
                );
            } else if cfg!(target_os = "macos") {
                // On macOS, the error must mention xcrun or SDK or rust-lld.
                assert!(
                    reason.contains("xcrun")
                        || reason.contains("SDK")
                        || reason.contains("rust-lld")
                        || reason.contains("Command Line Tools"),
                    "macOS error should be actionable: {reason}"
                );
            } else {
                assert!(
                    reason.contains("not implemented"),
                    "unsupported OS error should mention not-implemented: {reason}"
                );
            }
        }
    }
}

#[test]
fn system_linker_strategy_engages_on_supported_os_or_reports_actionable_error() {
    // On Windows MSVC, Linux GNU, and macOS, the system linker strategy should
    // engage (or return an actionable error if the toolchain is missing).
    // On other platforms, it must return a "not implemented" error.
    match discover_system_linker() {
        Ok(strategy) => match strategy {
            NativeLinkerStrategy::SystemLinker {
                linker,
                lib_dirs,
                dynamic_linker,
                extra_args,
                ..
            } => {
                assert!(linker.is_file(), "linker must exist: {}", linker.display());
                if cfg!(windows) {
                    assert!(!lib_dirs.is_empty(), "lib_dirs must be populated");
                    assert!(dynamic_linker.is_none(), "Windows has no dynamic linker");
                    assert!(
                        extra_args.iter().any(|a| a == "/NOLOGO"),
                        "Windows extra_args must include /NOLOGO"
                    );
                    assert!(
                        extra_args.iter().any(|a| a == "/SUBSYSTEM:CONSOLE"),
                        "Windows extra_args must include /SUBSYSTEM:CONSOLE"
                    );
                } else if cfg!(target_os = "linux") {
                    assert!(!lib_dirs.is_empty(), "lib_dirs must be populated");
                    assert!(dynamic_linker.is_some(), "Linux must have a dynamic linker");
                    assert!(
                        extra_args.iter().any(|a| a == "-no-pie"),
                        "Linux extra_args must include -no-pie"
                    );
                } else if cfg!(target_os = "macos") {
                    assert!(
                        extra_args.iter().any(|a| a == "-arch"),
                        "macOS extra_args must include -arch"
                    );
                    assert!(
                        extra_args.iter().any(|a| a == "-platform_version"),
                        "macOS extra_args must include -platform_version"
                    );
                    assert!(
                        extra_args.iter().any(|a| a == "-syslibroot"),
                        "macOS extra_args must include -syslibroot"
                    );
                    assert!(
                        dynamic_linker.is_none(),
                        "macOS has no explicit dynamic linker"
                    );
                }
            }
            _ => panic!("discover_system_linker returned wrong strategy variant"),
        },
        Err(reason) => {
            if cfg!(windows) {
                assert!(
                    reason.contains("vswhere")
                        || reason.contains("Visual Studio")
                        || reason.contains("Windows SDK")
                        || reason.contains("MSVC")
                        || reason.contains("link.exe")
                        || reason.contains("ORI_SYSTEM_LINKER"),
                    "Windows error should be actionable: {reason}"
                );
            } else if cfg!(target_os = "linux") {
                assert!(
                    reason.contains("cc")
                        || reason.contains("libc")
                        || reason.contains("ld")
                        || reason.contains("dynamic linker")
                        || reason.contains("ORI_SYSTEM_LINKER"),
                    "Linux error should be actionable: {reason}"
                );
            } else if cfg!(target_os = "macos") {
                assert!(
                    reason.contains("xcrun")
                        || reason.contains("SDK")
                        || reason.contains("ld")
                        || reason.contains("Command Line Tools")
                        || reason.contains("ORI_SYSTEM_LINKER"),
                    "macOS error should be actionable: {reason}"
                );
            } else {
                assert!(
                    reason.contains("not implemented"),
                    "unsupported OS error should mention not-implemented: {reason}"
                );
            }
        }
    }
}

#[test]
fn linux_system_linker_path_prefers_mold_then_lld_then_ld() {
    // Bare-linker fallback order (after CC/cc/gcc preference in find_linux_ld).
    let names = linux_system_linker_path_candidates();
    assert_eq!(names, &["mold", "ld.lld", "ld"]);
}

#[cfg(windows)]
#[test]
fn windows_link_exe_discovery_resolves_existing_path() {
    match discover_msvc_crt_lib_dirs() {
        Ok(dirs) => {
            assert!(
                !dirs.is_empty(),
                "MSVC CRT discovery should return lib dirs"
            );
            match find_windows_link_exe(&dirs[0]) {
                Ok(linker) => {
                    assert!(
                        linker.is_file(),
                        "link.exe path must exist: {}",
                        linker.display()
                    );
                    assert!(
                        linker
                            .file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.eq_ignore_ascii_case("link.exe")),
                        "discovered linker should be link.exe: {}",
                        linker.display()
                    );
                }
                Err(reason) => {
                    assert!(
                        reason.contains("link.exe")
                            || reason.contains("ORI_SYSTEM_LINKER")
                            || reason.contains("MSVC"),
                        "link.exe discovery error should be actionable: {reason}"
                    );
                }
            }
        }
        Err(reason) => {
            assert!(
                reason.contains("vswhere")
                    || reason.contains("Visual Studio")
                    || reason.contains("Windows SDK")
                    || reason.contains("MSVC"),
                "CRT discovery error should be actionable: {reason}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_system_linker_discovery_resolves_existing_paths() {
    match discover_system_linker() {
        Ok(strategy) => match strategy {
            NativeLinkerStrategy::SystemLinker {
                linker,
                crt_pre,
                crt_post,
                dynamic_linker,
                ..
            } => {
                assert!(linker.is_file(), "ld must exist: {}", linker.display());
                assert_eq!(crt_pre.len(), 2, "crt_pre should have crt1.o + crti.o");
                assert_eq!(crt_post.len(), 1, "crt_post should have crtn.o");
                for obj in crt_pre.iter().chain(crt_post.iter()) {
                    assert!(obj.is_file(), "CRT object must exist: {}", obj.display());
                }
                assert!(
                    dynamic_linker.as_ref().is_some_and(|p| p.is_file()),
                    "dynamic linker must exist"
                );
            }
            _ => panic!("discover_system_linker returned wrong strategy variant"),
        },
        Err(reason) => {
            assert!(
                reason.contains("cc")
                    || reason.contains("libc")
                    || reason.contains("ld")
                    || reason.contains("dynamic linker")
                    || reason.contains("ORI_SYSTEM_LINKER"),
                "Linux system linker discovery error should be actionable: {reason}"
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_system_linker_discovery_resolves_existing_ld() {
    match discover_system_linker() {
        Ok(strategy) => match strategy {
            NativeLinkerStrategy::SystemLinker {
                linker, extra_args, ..
            } => {
                assert!(linker.is_file(), "ld must exist: {}", linker.display());
                assert!(
                    extra_args.iter().any(|a| a == "-arch"),
                    "macOS extra_args must include -arch"
                );
                assert!(
                    extra_args.iter().any(|a| a == "-syslibroot"),
                    "macOS extra_args must include -syslibroot"
                );
            }
            _ => panic!("discover_system_linker returned wrong strategy variant"),
        },
        Err(reason) => {
            assert!(
                reason.contains("xcrun")
                    || reason.contains("SDK")
                    || reason.contains("ld")
                    || reason.contains("Command Line Tools")
                    || reason.contains("ORI_SYSTEM_LINKER"),
                "macOS system linker discovery error should be actionable: {reason}"
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_crt_discovery_resolves_existing_sdk() {
    // On a macOS machine with Xcode Command Line Tools installed (the CI
    // baseline), CRT/SDK discovery must succeed and return an existing SDK
    // directory plus a non-empty SDK version. On minimal containers without
    // `xcrun`, this test records the missing toolchain — it does not fail
    // the suite.
    match discover_macos_crt() {
        Ok(crt) => {
            assert!(
                crt.sdk_path.is_dir(),
                "macOS SDK path must exist: {}",
                crt.sdk_path.display()
            );
            assert!(
                !crt.sdk_version.is_empty(),
                "macOS SDK version must be non-empty"
            );
            assert!(
                crt.arch == "x86_64" || crt.arch == "arm64",
                "macOS arch must be x86_64 or arm64, got: {}",
                crt.arch
            );
            assert!(
                !crt.deployment_target.is_empty(),
                "macOS deployment target must be non-empty"
            );
        }
        Err(reason) => {
            assert!(
                reason.contains("xcrun")
                    || reason.contains("SDK")
                    || reason.contains("Command Line Tools"),
                "macOS CRT discovery error should be actionable: {reason}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_gnu_crt_discovery_resolves_existing_paths() {
    // On a Linux machine with `cc` and libc6-dev installed (the CI baseline),
    // CRT discovery must succeed and return existing crt1.o/crti.o/crtn.o
    // paths plus a dynamic linker. On minimal containers without `cc`, this
    // test records the missing toolchain — it does not fail the suite.
    match discover_linux_gnu_crt() {
        Ok(crt) => {
            assert!(
                !crt.lib_dirs.is_empty(),
                "Linux CRT discovery should return lib dirs"
            );
            assert_eq!(crt.crt_pre.len(), 2, "crt_pre should have crt1.o + crti.o");
            assert_eq!(crt.crt_post.len(), 1, "crt_post should have crtn.o");
            for obj in crt.crt_pre.iter().chain(crt.crt_post.iter()) {
                assert!(obj.is_file(), "CRT object must exist: {}", obj.display());
            }
            assert!(
                crt.dynamic_linker.is_file(),
                "dynamic linker must exist: {}",
                crt.dynamic_linker.display()
            );
        }
        Err(reason) => {
            assert!(
                reason.contains("cc") || reason.contains("libc"),
                "Linux CRT discovery error should be actionable: {reason}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_gnu_crt_common_path_fallback_resolves_when_available() {
    match discover_linux_gnu_crt_from_common_paths() {
        Ok(crt) => {
            assert!(
                !crt.lib_dirs.is_empty(),
                "common Linux CRT fallback should return lib dirs"
            );
            for obj in crt.crt_pre.iter().chain(crt.crt_post.iter()) {
                assert!(obj.is_file(), "CRT object must exist: {}", obj.display());
            }
            assert!(
                crt.dynamic_linker.is_file(),
                "dynamic linker must exist: {}",
                crt.dynamic_linker.display()
            );
        }
        Err(reason) => {
            assert!(
                reason.contains("libc") || reason.contains("dynamic linker"),
                "Linux CRT fallback error should be actionable: {reason}"
            );
        }
    }
}

#[test]
fn unresolved_native_symbol_error_adds_runtime_abi_hint() {
    let err = format_native_link_failure(
            "driver",
            Path::new("rustc"),
            "exit status: 1",
            b"",
            b"error LNK2019: unresolved external symbol ori_missing_runtime_func referenced in function ORI__main",
            NativeLinkOptions::default(),
        );

    assert!(err.contains("native.runtime_symbol_missing"), "{err}");
    assert!(err.contains("native symbol was not resolved"), "{err}");
    assert!(err.contains("ori-runtime"), "{err}");
    assert!(err.contains("target and ABI"), "{err}");
    assert!(err.contains("ori_missing_runtime_func"), "{err}");
    assert!(err.contains("--native-raw"), "{err}");
    assert!(!err.contains("C compiler"), "{err}");
}

#[test]
fn raw_native_link_failure_includes_full_streams() {
    let err = format_native_link_failure(
        "driver",
        Path::new("rustc"),
        "exit status: 1",
        b"raw stdout line",
        b"raw stderr line",
        NativeLinkOptions {
            raw_diagnostics: true,
            shared: false,
        },
    );

    assert!(err.contains("native.link_failed"), "{err}");
    assert!(err.contains("stdout:\nraw stdout line"), "{err}");
    assert!(err.contains("stderr:\nraw stderr line"), "{err}");
}

#[test]
fn native_hir_validator_rejects_invalid_logical_operands() {
    let hir = module_with_body(vec![HirStmt::Expr(HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::And,
            lhs: Box::new(int_expr(1, 10)),
            rhs: Box::new(int_expr(2, 12)),
        },
        ty: Ty::Bool,
        span: Span::new(10, 13),
    })]);

    let err = validate_native_hir(&hir).expect_err("invalid HIR should fail preflight");

    assert!(err.contains("invalid HIR for native backend"));
    assert!(err.contains("logical operator left operand must be bool"));
    assert!(
        !err.contains("Cranelift"),
        "preflight error should not leak verifier details: {err}"
    );
}

#[test]
fn native_hir_validator_accepts_valid_logical_operands() {
    let hir = module_with_body(vec![HirStmt::Expr(HirExpr {
        kind: HirExprKind::Binary {
            op: BinaryOp::And,
            lhs: Box::new(bool_expr(true, 10)),
            rhs: Box::new(bool_expr(false, 17)),
        },
        ty: Ty::Bool,
        span: Span::new(10, 22),
    })]);

    validate_native_hir(&hir).expect("valid bool HIR should pass preflight");
}

#[test]
fn option_and_result_layouts_are_stable_for_native_abi() {
    let ptr_ty = types::I64;

    assert_eq!(optional_layout(&Ty::Int, ptr_ty), (8, 16));
    assert_eq!(optional_layout(&Ty::Bool, ptr_ty), (1, 2));
    assert_eq!(optional_layout(&Ty::String, ptr_ty), (8, 16));

    assert_eq!(result_layout(&Ty::Int, &Ty::String, ptr_ty), (8, 8, 16));
    assert_eq!(result_layout(&Ty::String, &Ty::String, ptr_ty), (8, 8, 16));
}

#[test]
fn tuple_and_closure_layouts_are_stable_for_native_abi() {
    let ptr_ty = types::I64;
    let (tuple_fields, tuple_size, tuple_align) =
        tuple_layout(&[Ty::Int, Ty::Bool, Ty::String], ptr_ty);

    assert_eq!(tuple_fields.len(), 3);
    assert_eq!(tuple_fields[0].0, 0);
    assert_eq!(tuple_fields[1].0, 8);
    assert_eq!(tuple_fields[2].0, 16);
    assert_eq!(tuple_size, 24);
    assert_eq!(tuple_align, 8);

    let captures = vec![
        HirClosureCapture {
            name: "count".into(),
            ty: Ty::Int,
        },
        HirClosureCapture {
            name: "label".into(),
            ty: Ty::String,
        },
    ];
    let (capture_offsets, capture_size) = closure_env_layout(&captures, ptr_ty);

    assert_eq!(capture_offsets, vec![0, 8]);
    assert_eq!(capture_size, 16);
}

#[test]
fn async_and_concurrency_handle_layouts_are_native_pointers() {
    let ptr_ty = types::I64;

    assert_eq!(
        cl_type(&Ty::Future(Box::new(Ty::Int)), ptr_ty),
        Some(ptr_ty)
    );
    assert_eq!(
        cl_type(&Ty::TaskJob(Box::new(Ty::Int)), ptr_ty),
        Some(ptr_ty)
    );
    assert_eq!(
        cl_type(&Ty::Channel(Box::new(Ty::Int)), ptr_ty),
        Some(ptr_ty)
    );
    assert_eq!(cl_type(&Ty::AtomicInt, ptr_ty), Some(ptr_ty));
    assert_eq!(lazy_layout(&Ty::Int, ptr_ty), (16, 24));
}

#[test]
fn managed_type_audit_matches_native_abi_contract() {
    let managed = vec![
        Ty::String,
        Ty::Bytes,
        Ty::List(Box::new(Ty::Int)),
        Ty::Map(Box::new(Ty::String), Box::new(Ty::Int)),
        Ty::Set(Box::new(Ty::String)),
        Ty::Range(Box::new(Ty::Int)),
        Ty::Optional(Box::new(Ty::String)),
        Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        Ty::Tuple(vec![Ty::Int, Ty::String]),
        Ty::Named(DefId(42), Vec::new()),
        Ty::Any(DefId(7)),
        Ty::Func {
            params: vec![Ty::String],
            ret: Box::new(Ty::Void),
        },
        Ty::Lazy(Box::new(Ty::String)),
        Ty::Future(Box::new(Ty::String)),
        Ty::TaskJob(Box::new(Ty::Int)),
        Ty::Channel(Box::new(Ty::String)),
        Ty::AtomicInt,
        Ty::TaskJoinError,
        Ty::ChannelSendError,
        Ty::ChannelReceiveError,
    ];
    let direct = vec![
        Ty::Bool,
        Ty::Int,
        Ty::Int8,
        Ty::Int16,
        Ty::Int32,
        Ty::Int64,
        Ty::U8,
        Ty::U16,
        Ty::U32,
        Ty::U64,
        Ty::Float,
        Ty::Float32,
        Ty::Float64,
        Ty::Void,
        Ty::Never,
        Ty::Error,
        Ty::Param {
            index: 0,
            name: "T".into(),
        },
        Ty::Infer(0),
    ];

    for ty in managed {
        assert!(is_managed_ty(&ty), "`{}` must be managed", ty.display());
    }
    for ty in direct {
        assert!(
            !is_managed_ty(&ty),
            "`{}` must remain a direct/non-managed value",
            ty.display()
        );
    }
}

#[test]
fn managed_return_retain_happens_before_scope_cleanup() {
    let source = include_str!("../native_backend.rs");
    // "fn emit_return(" with the paren: a plain "fn emit_return" prefix now
    // matches emit_return_scope_cleanup first, scanning the wrong section.
    let emit_return = source_section(source, "fn emit_return(", "fn emit_future_ready");
    let future_return = source_section(
        emit_return,
        "if let Ty::Future(inner) = return_ty",
        "let return_value = val",
    );
    let normal_return = source_section(emit_return, "let return_value = val", "Ok(())");

    // Since LANG-MEM-4 the normal path cleans up through
    // emit_return_scope_cleanup (transfer-aware), not the raw
    // emit_scope_cleanup_calls_from.
    assert_order(
        normal_return,
        "self.emit_arc_retain_if_managed(&return_ty, value)?;",
        "self.emit_return_scope_cleanup(transfer_var)?;",
    );
    assert_order(
        future_return,
        "self.emit_arc_retain_if_managed(&Ty::Future(Box::new(inner_ty)), future)?;",
        "self.emit_scope_cleanup_calls_from(0, 0)?;",
    );
}

#[test]
fn simple_async_state_machine_cleans_frame_on_terminal_paths() {
    let source = include_str!("../native_backend.rs");
    let step = source_section(
        source,
        "fn emit_simple_async_step(",
        "fn emit_simple_async_frame_cleanup(",
    );
    let cleanup = source_section(
        source,
        "fn emit_simple_async_frame_cleanup(",
        "fn emit_await(",
    );

    assert_eq!(
        step.matches("self.emit_async_terminal_cleanup(plan, frame, index)?;")
            .count(),
        3,
        "step cleanup must run on `?`, failed future and cancelled future paths"
    );
    assert!(
        step.contains("self.emit_async_terminal_cleanup(plan, frame, await_count)?;"),
        "invalid-state completion must cleanup the async frame"
    );
    assert!(
        step.contains("self.emit_scope_cleanup_calls_from(0, 0)?;"),
        "normal completion must run using cleanup"
    );
    let terminal_cleanup = source_section(
        source,
        "fn emit_async_terminal_cleanup(",
        "fn dispose_func_name_for_ty(",
    );
    assert!(
        terminal_cleanup.contains("self.emit_async_frame_dispose_live_values(plan, frame,"),
        "terminal async cleanup must dispose live frame bindings"
    );
    for expected in [
        "ASYNC_FRAME_RESULT_OFFSET",
        "simple_async_frame_param_offset",
        "simple_async_frame_local_offset",
        "simple_async_frame_binding_offset",
        "self.emit_arc_unregister_edge(frame, result_future)?;",
        "self.emit_arc_unregister_edge(frame, value)?;",
    ] {
        assert!(cleanup.contains(expected), "missing `{expected}`");
    }
}

#[test]
fn simple_async_state_machine_releases_dead_managed_frame_values_after_resume() {
    let source = include_str!("../native_backend.rs");
    let step = source_section(
        source,
        "fn emit_simple_async_step(",
        "fn emit_simple_async_frame_cleanup(",
    );
    let drop_dead = source_section(
        source,
        "fn emit_simple_async_drop_dead_frame_values_after_await(",
        "fn emit_await(",
    );

    assert!(
        step.matches("emit_simple_async_drop_dead_frame_values_after_await")
            .count()
            >= 2,
        "state machine must release dead frame edges on pending and ready paths"
    );
    assert!(
        drop_dead.contains("simple_async_uses_after_await(plan, await_index)"),
        "drop logic must be driven by calculated liveness after each await"
    );
    assert!(
        drop_dead.contains("self.emit_simple_async_drop_frame_edge(frame"),
        "drop logic must unregister frame ARC edges"
    );
    assert!(
        drop_dead.contains("self.emit_arc_maybe_collect_cycles()?;"),
        "drop logic must allow amortized cycle collection after early frame releases"
    );
}

#[test]
fn native_await_lowering_no_longer_uses_task_block_on() {
    let source = include_str!("../native_backend.rs");
    let emit_await = source_section(source, "fn emit_await(", "fn emit_never_call_stmt(");
    let async_wrapper = source_section(
        source,
        "fn emit_async_wrapper(",
        "fn emit_simple_async_state_machine_wrapper(",
    );

    assert!(
        !emit_await.contains("ori_task_block_on"),
        "`emit_await` must not lower to the synchronous block_on bridge"
    );
    assert!(
        !async_wrapper.contains("ori_async_spawn"),
        "async wrappers must not use executor-backed spawn fallback"
    );
}

#[test]
fn managed_assignment_updates_arc_before_overwrite() {
    let source = include_str!("../native_backend.rs");
    let emit_stmt = source_section(source, "fn emit_stmt(", "fn emit_if(");
    let assign = source_section(
        emit_stmt,
        "HirStmt::Assign { lvalue, value, .. } =>",
        "HirStmt::Return",
    );

    assert_order(
        assign,
        "self.emit_arc_retain_if_managed(&ty, val)?;",
        "self.emit_arc_release_if_managed(&ty, old)?;",
    );
    assert_order(
        assign,
        "self.emit_arc_release_if_managed(&ty, old)?;",
        "self.builder.def_var(var, val);",
    );
    assert_order(
        assign,
        "self.emit_arc_update_edge_if_managed(&elem_ty, container, old, val)?;",
        ".get(\"ori_list_set\")",
    );
    assert_order(
        assign,
        "self.emit_arc_update_edge_if_managed(&field_layout.ty, owner, old, val)?;",
        ".store(MemFlags::new(), val, addr, 0);",
    );
}

#[test]
fn managed_aggregate_literals_register_arc_edges() {
    let source = include_str!("../native_backend.rs");
    let expr_codegen = source_section(source, "fn emit_expr(", "fn str_len_from_ptr");
    let list_push = source_section(
        source,
        "fn emit_list_push_value",
        "fn emit_list_extend_from",
    );

    for expected in [
        "self.emit_arc_register_edge_if_managed(inner_ty, base, val)?;",
        "self.emit_arc_register_edge_if_managed(ok_ty, base, val)?;",
        "self.emit_arc_register_edge_if_managed(err_ty, base, val)?;",
        "self.emit_arc_register_edge_if_managed(&fi.ty, base, val)?;",
        "self.emit_arc_register_edge_if_managed(&elem_ty, base, v)?;",
        "self.emit_arc_register_edge_if_managed(&key_ty, map_ptr, key_value)?;",
        "self.emit_arc_register_edge_if_managed(&value_ty, map_ptr, map_value)?;",
        "self.emit_arc_register_edge_if_managed(&elem.ty, set_ptr, v)?;",
    ] {
        assert!(expr_codegen.contains(expected), "missing `{expected}`");
    }
    assert!(
        list_push.contains("self.emit_arc_register_edge_if_managed(elem_ty, list, value)?;"),
        "{list_push}"
    );
}

#[test]
fn managed_closure_captures_are_retained_and_edge_registered() {
    let source = include_str!("../native_backend.rs");
    let prologue = source_section(
        source,
        "fn emit_closure_capture_prologue",
        "fn emit_value_contract",
    );
    let capture_object = source_section(source, "fn emit_closure_value", "fn emit_closure_call");

    assert!(prologue.contains("self.emit_arc_retain_if_managed(&capture.ty, value)?;"));
    assert!(prologue.contains("self.managed_stack.push(ManagedCleanup"));
    assert!(
        capture_object
            .contains("self.emit_arc_register_edge_if_managed(&capture.ty, env, value)?;"),
        "{capture_object}"
    );
}

fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("source marker `{start}` not found"));
    let tail = &source[start_index..];
    let end_index = tail
        .find(end)
        .unwrap_or_else(|| panic!("source marker `{end}` not found after `{start}`"));
    &tail[..end_index]
}

fn assert_order(source: &str, before: &str, after: &str) {
    let before_index = source
        .find(before)
        .unwrap_or_else(|| panic!("marker `{before}` not found in source section"));
    let after_index = source
        .find(after)
        .unwrap_or_else(|| panic!("marker `{after}` not found in source section"));
    assert!(
        before_index < after_index,
        "`{before}` must appear before `{after}`"
    );
}

fn module_with_body(stmts: Vec<HirStmt>) -> HirModule {
    HirModule {
        namespace: "test".into(),
        structs: Vec::new(),
        enums: Vec::new(),
        traits: Vec::new(),
        trait_impls: Vec::new(),
        funcs: vec![HirFunc {
            def_id: DefId(0),
            name: "main".into(),
            params: Vec::new(),
            return_ty: Ty::Void,
            body: HirBlock {
                stmts,
                span: Span::new(0, 30),
            },
            closure_captures: Vec::new(),
            is_public: true,
            is_async: false,
            is_mut: false,
            c_export_name: None,
            span: Span::new(0, 30),
        }],
        consts: Vec::new(),
        externs: Vec::new(),
    }
}

fn int_expr(value: i64, start: usize) -> HirExpr {
    HirExpr {
        kind: HirExprKind::IntLit(value),
        ty: Ty::Int,
        span: Span::new(start, start + 1),
    }
}

fn bool_expr(value: bool, start: usize) -> HirExpr {
    HirExpr {
        kind: HirExprKind::BoolLit(value),
        ty: Ty::Bool,
        span: Span::new(start, start + if value { 4 } else { 5 }),
    }
}
