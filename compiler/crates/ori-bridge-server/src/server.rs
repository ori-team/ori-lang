use crate::protocol::{
    BridgeErrorPayload, CompileModuleRequest, CompileModuleResponse, HandshakeRequest,
    HandshakeResponse, RequestEnvelope, ResponseEnvelope, SerializedExpr, SerializedFunc,
    SerializedModule, SerializedStmt, SerializedTy, CURRENT_PROTOCOL_VERSION,
};
use ori_ast::expr::BinaryOp;
use ori_codegen::{emit_native_with_options, NativeEmitOptions};
use ori_diagnostics::Span;
use ori_hir::hir::{
    HirArg, HirBlock, HirExpr, HirExprKind, HirFunc, HirModule, HirParam, HirStmt,
};
use ori_types::{DefId, Ty};
use smol_str::SmolStr;
use std::path::Path;

pub struct BridgeServer;

impl Default for BridgeServer {
    fn default() -> Self {
        Self::new()
    }
}

impl BridgeServer {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_request(&self, req: RequestEnvelope) -> ResponseEnvelope {
        if req.protocol_version != CURRENT_PROTOCOL_VERSION {
            return ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.unsupported_version".to_string(),
                    message: format!(
                        "protocol version {} not supported (expected {})",
                        req.protocol_version, CURRENT_PROTOCOL_VERSION
                    ),
                }),
            };
        }

        match req.command.as_str() {
            "handshake" => self.handle_handshake(req),
            "compile_module" | "compile_empty_module" => self.handle_compile_module(req),
            unknown => ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.unknown_command".to_string(),
                    message: format!("unknown command: {}", unknown),
                }),
            },
        }
    }

    fn handle_handshake(&self, req: RequestEnvelope) -> ResponseEnvelope {
        let _handshake: HandshakeRequest = match serde_json::from_value(req.payload) {
            Ok(h) => h,
            Err(e) => {
                return ResponseEnvelope {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id: req.request_id,
                    status: "error".to_string(),
                    data: None,
                    error: Some(BridgeErrorPayload {
                        code: "bridge.invalid_payload".to_string(),
                        message: e.to_string(),
                    }),
                }
            }
        };

        let response_data = HandshakeResponse {
            server_version: "0.3.8".to_string(),
            protocol_version: CURRENT_PROTOCOL_VERSION,
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            features: vec![
                "cranelift".to_string(),
                "native_aot".to_string(),
                "system_linker".to_string(),
            ],
        };

        ResponseEnvelope {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: req.request_id,
            status: "ok".to_string(),
            data: Some(serde_json::to_value(response_data).unwrap()),
            error: None,
        }
    }

    fn handle_compile_module(&self, req: RequestEnvelope) -> ResponseEnvelope {
        let compile_req: CompileModuleRequest = match serde_json::from_value(req.payload) {
            Ok(c) => c,
            Err(e) => {
                return ResponseEnvelope {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id: req.request_id,
                    status: "error".to_string(),
                    data: None,
                    error: Some(BridgeErrorPayload {
                        code: "bridge.invalid_payload".to_string(),
                        message: e.to_string(),
                    }),
                }
            }
        };

        let hir = lower_serialized_module(&compile_req.module);
        let out_path = Path::new(&compile_req.output_path);

        let emit_opts = NativeEmitOptions {
            lib: compile_req.lib_mode,
        };

        match emit_native_with_options(&hir, out_path, emit_opts) {
            Ok(()) => {
                let bytes = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
                let res = CompileModuleResponse {
                    output_path: compile_req.output_path,
                    bytes_written: bytes,
                };
                ResponseEnvelope {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id: req.request_id,
                    status: "ok".to_string(),
                    data: Some(serde_json::to_value(res).unwrap()),
                    error: None,
                }
            }
            Err(e) => ResponseEnvelope {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: req.request_id,
                status: "error".to_string(),
                data: None,
                error: Some(BridgeErrorPayload {
                    code: "bridge.compilation_failed".to_string(),
                    message: e,
                }),
            },
        }
    }
}

fn lower_serialized_module(sm: &SerializedModule) -> HirModule {
    let mut funcs = Vec::new();
    for (i, f) in sm.funcs.iter().enumerate() {
        funcs.push(lower_func(f, DefId(i as u32 + 1)));
    }

    HirModule {
        namespace: SmolStr::new(&sm.namespace),
        structs: vec![],
        enums: vec![],
        traits: vec![],
        trait_impls: vec![],
        funcs,
        consts: vec![],
        externs: vec![],
    }
}

fn lower_ty(ty: &SerializedTy) -> Ty {
    match ty {
        SerializedTy::Int => Ty::Int,
        SerializedTy::Float => Ty::Float,
        SerializedTy::Bool => Ty::Bool,
        SerializedTy::String => Ty::String,
        SerializedTy::Void => Ty::Void,
    }
}

fn lower_func(sf: &SerializedFunc, def_id: DefId) -> HirFunc {
    let mut params = Vec::new();
    for p in &sf.params {
        params.push(HirParam {
            name: SmolStr::new(&p.name),
            ty: lower_ty(&p.ty),
            default: None,
            contract: None,
            variadic: false,
            span: Span::DUMMY,
        });
    }

    let mut stmts = Vec::new();
    for s in &sf.body_stmts {
        stmts.push(lower_stmt(s));
    }

    HirFunc {
        def_id,
        name: SmolStr::new(&sf.name),
        params,
        return_ty: lower_ty(&sf.return_ty),
        body: HirBlock {
            stmts,
            span: Span::DUMMY,
        },
        closure_captures: vec![],
        is_public: sf.is_public,
        is_async: false,
        is_mut: false,
        is_inline: false,
        is_no_inline: false,
        c_export_name: None,
        span: Span::DUMMY,
    }
}

fn lower_stmt(ss: &SerializedStmt) -> HirStmt {
    match ss {
        SerializedStmt::Let { name, ty, value } => HirStmt::Let {
            name: SmolStr::new(name),
            ty: lower_ty(ty),
            mutable: false,
            value: lower_expr(value),
            span: Span::DUMMY,
        },
        SerializedStmt::Return(maybe_expr) => {
            HirStmt::Return(maybe_expr.as_ref().map(lower_expr), Span::DUMMY)
        }
        SerializedStmt::Expr(expr) => HirStmt::Expr(lower_expr(expr)),
    }
}

fn lower_expr(se: &SerializedExpr) -> HirExpr {
    match se {
        SerializedExpr::IntLit(val) => HirExpr {
            kind: HirExprKind::IntLit(*val),
            ty: Ty::Int,
            span: Span::DUMMY,
        },
        SerializedExpr::StrLit(val) => HirExpr {
            kind: HirExprKind::StrLit(SmolStr::new(val)),
            ty: Ty::String,
            span: Span::DUMMY,
        },
        SerializedExpr::BoolLit(val) => HirExpr {
            kind: HirExprKind::BoolLit(*val),
            ty: Ty::Bool,
            span: Span::DUMMY,
        },
        SerializedExpr::Var(name) => HirExpr {
            kind: HirExprKind::Var(SmolStr::new(name)),
            ty: Ty::Int, // Fallback scalar
            span: Span::DUMMY,
        },
        SerializedExpr::Add(left, right) => HirExpr {
            kind: HirExprKind::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(lower_expr(left)),
                rhs: Box::new(lower_expr(right)),
            },
            ty: Ty::Int,
            span: Span::DUMMY,
        },
        SerializedExpr::Call { callee, args } => HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(HirExpr {
                    kind: HirExprKind::Var(SmolStr::new(callee)),
                    ty: Ty::Void,
                    span: Span::DUMMY,
                }),
                args: args
                    .iter()
                    .map(|a| HirArg {
                        value: lower_expr(a),
                        label: None,
                        spread: false,
                    })
                    .collect(),
            },
            ty: Ty::Void,
            span: Span::DUMMY,
        },
    }
}
