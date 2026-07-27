use crate::def::{CompileTimeValue, ConstEvalFailure, ConstEvalFailureKind, DefMap};
use crate::ty::{OpaqueTy, Ty};
use ori_ast::common::QualifiedName;
use ori_ast::ty::{ConstExpr, Type as AstType};
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label};
use smol_str::SmolStr;
use std::collections::HashMap;

/// Lower an AST type into a canonical `Ty`.
///
/// `module_path` is the current namespace (e.g. `"app.user"`).
/// `type_params` are the in-scope generic type parameter names.
pub fn lower_type(
    ast_ty: &AstType,
    module_path: &str,
    type_params: &[SmolStr],
    def_map: &DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> Ty {
    let aliases = HashMap::new();
    lower_type_with_aliases(
        ast_ty,
        module_path,
        type_params,
        def_map,
        file_id,
        sink,
        &aliases,
    )
}

pub fn lower_type_with_aliases(
    ast_ty: &AstType,
    module_path: &str,
    type_params: &[SmolStr],
    def_map: &DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
    aliases: &HashMap<SmolStr, SmolStr>,
) -> Ty {
    lower_type_with_local_aliases(
        ast_ty,
        module_path,
        type_params,
        def_map,
        file_id,
        sink,
        aliases,
        &HashMap::new(),
    )
}

pub fn lower_type_with_local_aliases(
    ast_ty: &AstType,
    module_path: &str,
    type_params: &[SmolStr],
    def_map: &DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
    aliases: &HashMap<SmolStr, SmolStr>,
    local_aliases: &HashMap<SmolStr, AstType>,
) -> Ty {
    macro_rules! rec {
        ($t:expr) => {
            lower_type_with_local_aliases(
                $t,
                module_path,
                type_params,
                def_map,
                file_id,
                sink,
                aliases,
                local_aliases,
            )
        };
    }
    match ast_ty {
        AstType::Slice(inner, _) => Ty::Slice(Box::new(rec!(inner))),
        // Concrete CT-0 expressions become ConstInt. A direct `size: cap`
        // remains a parameter for substitution at the use site.
        AstType::ConstArg { name, value, span } => lower_const_expression(
            value,
            &name.text,
            *span,
            &ConstLoweringContext {
                module_path,
                type_params,
                def_map,
                file_id,
                aliases,
            },
            sink,
        ),
        AstType::Array { elem, size, span } => {
            let elem_ty = rec!(elem);
            let size_ty = lower_const_expression(
                size,
                "size",
                *span,
                &ConstLoweringContext {
                    module_path,
                    type_params,
                    def_map,
                    file_id,
                    aliases,
                },
                sink,
            );
            let size_ty = match size_ty {
                Ty::ConstInt(_, value) if value < 0 => {
                    sink.emit(
                        Diagnostic::error(
                            "type.negative_array_size",
                            format!("array length must not be negative, found `{value}`"),
                        )
                        .with_label(Label::primary(file_id, size.span(), "array length"))
                        .with_action("use a length of zero or more"),
                    );
                    Ty::Error
                }
                other => other,
            };
            // Elements live inline with no reference counting, so a managed
            // element would be stored without a retain and released by nobody.
            // Reject that instead of producing a program that leaks or
            // double-frees depending on the path.
            if elem_ty.is_runtime_managed() && !elem_ty.is_error() {
                sink.emit(
                    Diagnostic::error(
                        "type.array_element_not_inline",
                        format!(
                            "`array` elements are stored inline, so `{}` cannot be an element type",
                            elem_ty.display()
                        ),
                    )
                    .with_label(Label::primary(file_id, *span, "element type"))
                    .with_why("inline storage has no reference counting, and this type is reference counted")
                    .with_action("use a scalar element (`int`, `float`, `bool`, `u8`, …), or `list[T]` for managed values"),
                );
                return Ty::Error;
            }
            Ty::Array(Box::new(elem_ty), Box::new(size_ty))
        }
        // Check local type aliases (e.g. associated types in implement blocks)
        AstType::Named(name)
            if name.is_single() && local_aliases.contains_key(name.last().as_str()) =>
        {
            let target_ast_ty = &local_aliases[name.last().as_str()];
            rec!(target_ast_ty)
        }
        AstType::Generic { name, .. }
            if name.is_single() && local_aliases.contains_key(name.last().as_str()) =>
        {
            let target_ast_ty = &local_aliases[name.last().as_str()];
            rec!(target_ast_ty)
        }

        // ── Primitives ────────────────────────────────────────────────────────
        AstType::Bool(_) => Ty::Bool,
        AstType::Int(_) => Ty::Int,
        AstType::Int8(_) => Ty::Int8,
        AstType::Int16(_) => Ty::Int16,
        AstType::Int32(_) => Ty::Int32,
        AstType::Int64(_) => Ty::Int64,
        AstType::U8(_) => Ty::U8,
        AstType::U16(_) => Ty::U16,
        AstType::U32(_) => Ty::U32,
        AstType::U64(_) => Ty::U64,
        AstType::Float(_) => Ty::Float,
        AstType::Float32(_) => Ty::Float32,
        AstType::Float64(_) => Ty::Float64,
        AstType::String(_) => Ty::String,
        AstType::Bytes(_) => Ty::Bytes,
        AstType::Void(_) => Ty::Void,

        // ── Built-in generic types ────────────────────────────────────────────
        AstType::Optional(inner, _) => Ty::Optional(Box::new(rec!(inner))),
        AstType::Result(ok, err, _) => Ty::Result(Box::new(rec!(ok)), Box::new(rec!(err))),
        AstType::List(elem, _) => Ty::List(Box::new(rec!(elem))),
        AstType::Map(key, val, _) => Ty::Map(Box::new(rec!(key)), Box::new(rec!(val))),
        AstType::Set(elem, _) => Ty::Set(Box::new(rec!(elem))),
        AstType::Range(elem, _) => Ty::Range(Box::new(rec!(elem))),
        AstType::Lazy(inner, _) => Ty::Lazy(Box::new(rec!(inner))),
        AstType::Handle(inner, _) => Ty::Handle(Box::new(rec!(inner))),
        AstType::Tuple(elems, _) => Ty::Tuple(elems.iter().map(|t| rec!(t)).collect()),
        AstType::Any(trait_name, span) => {
            let id = resolve_name(
                trait_name,
                module_path,
                def_map,
                file_id,
                *span,
                sink,
                aliases,
            );
            Ty::Any(id.unwrap_or(crate::def::DefId(u32::MAX)))
        }

        // ── Callable type ─────────────────────────────────────────────────────
        AstType::Func {
            params, return_ty, ..
        } => {
            let ps = params.iter().map(|t| rec!(t)).collect();
            let ret = return_ty.as_ref().map_or(Ty::Void, |t| rec!(t));
            Ty::Func {
                params: ps,
                ret: Box::new(ret),
            }
        }

        // ── Named / generic types ─────────────────────────────────────────────
        AstType::Named(name) => lower_named(
            name,
            &[],
            module_path,
            type_params,
            def_map,
            file_id,
            sink,
            aliases,
        ),

        AstType::Generic { name, args, .. } => {
            let lowered_args: Vec<Ty> = args.iter().map(|t| rec!(t)).collect();
            lower_named(
                name,
                &lowered_args,
                module_path,
                type_params,
                def_map,
                file_id,
                sink,
                aliases,
            )
        }
    }
}

struct ConstLoweringContext<'a> {
    module_path: &'a str,
    type_params: &'a [SmolStr],
    def_map: &'a DefMap,
    file_id: FileId,
    aliases: &'a HashMap<SmolStr, SmolStr>,
}

fn lower_const_expression(
    expression: &ConstExpr,
    argument_name: &str,
    usage_span: ori_diagnostics::Span,
    context: &ConstLoweringContext<'_>,
    sink: &mut DiagnosticSink,
) -> Ty {
    if let ConstExpr::Name(name) = expression {
        if name.is_single() {
            if let Some(index) = context
                .type_params
                .iter()
                .position(|param| param == &name.last().text)
            {
                return Ty::Param {
                    index: index as u32,
                    name: name.last().text.clone(),
                };
            }
        }
    }

    if let Some(parameter) = referenced_type_parameter(expression, context.type_params) {
        sink.emit(
            Diagnostic::error(
                "type.const_param_expression_unsupported",
                format!(
                    "const parameter `{parameter}` cannot yet be used inside arithmetic or conditionals"
                ),
            )
            .with_label(Label::primary(
                context.file_id,
                expression.span(),
                "symbolic const expression",
            ))
            .with_why(
                "CT-0 evaluates concrete expressions; symbolic const arithmetic needs a later monomorphization phase",
            )
            .with_action("pass the const parameter directly, or compute a concrete module constant"),
        );
        return Ty::Error;
    }

    match crate::const_eval::evaluate_type_const_expr(
        expression,
        context.module_path,
        context.aliases,
        context.def_map,
        context.file_id,
    ) {
        Ok(CompileTimeValue::Int(value)) => Ty::ConstInt(SmolStr::new(argument_name), value),
        Ok(CompileTimeValue::Bool(_)) => {
            sink.emit(
                Diagnostic::error(
                    "type.const_argument_not_integer",
                    format!("const type argument `{argument_name}` must evaluate to an integer"),
                )
                .with_label(Label::primary(
                    context.file_id,
                    expression.span(),
                    "boolean value",
                ))
                .with_action("use an integer result, such as `if condition then 1 else 0`"),
            );
            Ty::Error
        }
        Err(failure) => {
            if failure.kind == ConstEvalFailureKind::UndefinedName {
                if let ConstExpr::Name(name) = expression {
                    if name.is_single() {
                        sink.emit(
                            Diagnostic::error(
                                "type.undefined_const_param",
                                format!(
                                    "`{}` is neither a const parameter nor a module constant in scope",
                                    name.last()
                                ),
                            )
                            .with_label(Label::primary(
                                context.file_id,
                                expression.span(),
                                "unknown compile-time name",
                            ))
                            .with_action(
                                "declare it as a const parameter, define a module `const`, or import a public module constant",
                            ),
                        );
                        return Ty::Error;
                    }
                }
            }
            emit_const_eval_failure(sink, failure, context.file_id, usage_span);
            Ty::Error
        }
    }
}

fn referenced_type_parameter<'a>(
    expression: &'a ConstExpr,
    type_params: &'a [SmolStr],
) -> Option<&'a str> {
    match expression {
        ConstExpr::Name(name)
            if name.is_single()
                && type_params
                    .iter()
                    .any(|parameter| parameter == &name.last().text) =>
        {
            Some(name.last().as_str())
        }
        ConstExpr::Unary { operand, .. } => referenced_type_parameter(operand, type_params),
        ConstExpr::Binary { lhs, rhs, .. } => referenced_type_parameter(lhs, type_params)
            .or_else(|| referenced_type_parameter(rhs, type_params)),
        ConstExpr::If {
            condition,
            then_expr,
            else_expr,
            ..
        } => referenced_type_parameter(condition, type_params)
            .or_else(|| referenced_type_parameter(then_expr, type_params))
            .or_else(|| referenced_type_parameter(else_expr, type_params)),
        _ => None,
    }
}

fn emit_const_eval_failure(
    sink: &mut DiagnosticSink,
    failure: ConstEvalFailure,
    usage_file_id: FileId,
    usage_span: ori_diagnostics::Span,
) {
    let (code, action) = match failure.kind {
        ConstEvalFailureKind::UnsupportedExpression => (
            "consteval.unsupported_expression",
            "use literals, integer/boolean module constants, integer arithmetic, comparisons, boolean logic, or inline `if`",
        ),
        ConstEvalFailureKind::InvalidLiteral => (
            "consteval.invalid_literal",
            "use an integer literal representable by Ori's compile-time `int`",
        ),
        ConstEvalFailureKind::UndefinedName => (
            "consteval.undefined_name",
            "declare or import the module constant before using it in a type",
        ),
        ConstEvalFailureKind::NonConstName => (
            "consteval.non_const_name",
            "use a module `const`; variables and functions are runtime values",
        ),
        ConstEvalFailureKind::PrivateName => (
            "name.private",
            "make the constant public or keep the type-level use in the declaring module",
        ),
        ConstEvalFailureKind::Cycle => (
            "consteval.cycle",
            "break the cycle so each compile-time constant has a finite value",
        ),
        ConstEvalFailureKind::Overflow => (
            "consteval.overflow",
            "reduce the operands so the result fits in the compile-time `int` range",
        ),
        ConstEvalFailureKind::DivisionByZero => (
            "consteval.division_by_zero",
            "make the divisor non-zero",
        ),
        ConstEvalFailureKind::TypeMismatch => (
            "consteval.type_mismatch",
            "keep arithmetic operands integer and conditions boolean",
        ),
    };
    let mut diagnostic = Diagnostic::error(code, failure.detail).with_label(Label::primary(
        failure.file_id,
        failure.span,
        "compile-time evaluation failed here",
    ));
    if failure.file_id != usage_file_id || failure.span != usage_span {
        diagnostic = diagnostic.with_label(Label::primary(
            usage_file_id,
            usage_span,
            "required by this const type argument",
        ));
    }
    sink.emit(diagnostic.with_action(action));
}

fn lower_named(
    name: &QualifiedName,
    args: &[Ty],
    module_path: &str,
    type_params: &[SmolStr],
    def_map: &DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
    aliases: &HashMap<SmolStr, SmolStr>,
) -> Ty {
    // Check if it's an in-scope type parameter (must be a single-segment name)
    if name.is_single() {
        let n = name.last().as_str();
        if let Some(idx) = type_params.iter().position(|p| p == n) {
            if args.is_empty() {
                return Ty::Param {
                    index: idx as u32,
                    name: SmolStr::new(n),
                };
            } else {
                return Ty::Named(crate::def::DefId(0x4000_0000 | (idx as u32)), args.to_vec());
            }
        }
    }
    let expanded = expand_alias(&name.to_string(), aliases);
    if let Some(ty) = lower_builtin_concurrency_type(&expanded, args) {
        return ty;
    }
    let span = name.span;
    match resolve_name(name, module_path, def_map, file_id, span, sink, aliases) {
        Some(id) => Ty::Named(id, args.to_vec()),
        None => Ty::Error,
    }
}

fn lower_builtin_concurrency_type(path: &str, args: &[Ty]) -> Option<Ty> {
    match path {
        "future" => Some(Ty::Future(Box::new(
            args.first().cloned().unwrap_or(Ty::Infer(0)),
        ))),
        "ori.task.Job" => Some(Ty::TaskJob(Box::new(
            args.first().cloned().unwrap_or(Ty::Infer(0)),
        ))),
        "ori.task.JoinError" => Some(Ty::TaskJoinError),
        "ori.channel.Channel" => Some(Ty::Channel(Box::new(
            args.first().cloned().unwrap_or(Ty::Infer(0)),
        ))),
        "ori.channel.SendError" => Some(Ty::ChannelSendError),
        "ori.channel.ReceiveError" => Some(Ty::ChannelReceiveError),
        "ori.atomic.AtomicInt" => Some(Ty::AtomicInt),
        "ori.deque.Deque" => Some(Ty::Opaque {
            kind: OpaqueTy::Deque,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.queue.Queue" => Some(Ty::Opaque {
            kind: OpaqueTy::Queue,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.stack.Stack" => Some(Ty::Opaque {
            kind: OpaqueTy::Stack,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.linked_list.LinkedList" => Some(Ty::Opaque {
            kind: OpaqueTy::LinkedList,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.doubly_linked_list.DoublyLinkedList" => Some(Ty::Opaque {
            kind: OpaqueTy::DoublyLinkedList,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.tree.Tree" => Some(Ty::Opaque {
            kind: OpaqueTy::Tree,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.tree.NodeId" => Some(Ty::Opaque {
            kind: OpaqueTy::NodeId,
            args: vec![],
        }),
        "ori.hash_table.HashTable" => Some(Ty::Opaque {
            kind: OpaqueTy::HashTable,
            args: vec![
                args.first().cloned().unwrap_or(Ty::Infer(0)),
                args.get(1).cloned().unwrap_or(Ty::Infer(1)),
            ],
        }),
        "ori.graph.Graph" => Some(Ty::Opaque {
            kind: OpaqueTy::Graph,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.heap.Heap" => Some(Ty::Opaque {
            kind: OpaqueTy::Heap,
            args: vec![args.first().cloned().unwrap_or(Ty::Infer(0))],
        }),
        "ori.fs.File" => Some(Ty::Opaque {
            kind: OpaqueTy::File,
            args: vec![],
        }),
        "ori.task.CancelToken" => Some(Ty::Opaque {
            kind: OpaqueTy::CancelToken,
            args: vec![],
        }),
        "ori.net.Connection" => Some(Ty::Opaque {
            kind: OpaqueTy::Connection,
            args: vec![],
        }),
        "ori.net.Listener" => Some(Ty::Opaque {
            kind: OpaqueTy::Listener,
            args: vec![],
        }),
        "ori.net.UdpSocket" => Some(Ty::Opaque {
            kind: OpaqueTy::UdpSocket,
            args: vec![],
        }),
        "ori.io.Input" => Some(Ty::Opaque {
            kind: OpaqueTy::Input,
            args: vec![],
        }),
        "ori.io.Output" => Some(Ty::Opaque {
            kind: OpaqueTy::Output,
            args: vec![],
        }),
        _ => None,
    }
}

fn resolve_name(
    name: &QualifiedName,
    module_path: &str,
    def_map: &DefMap,
    file_id: FileId,
    span: ori_diagnostics::Span,
    sink: &mut DiagnosticSink,
    aliases: &HashMap<SmolStr, SmolStr>,
) -> Option<crate::def::DefId> {
    let path_str = name.to_string();
    let expanded = expand_alias(&path_str, aliases);
    // Try fully-qualified first
    if let Some(id) = def_map.lookup(&expanded) {
        return Some(id);
    }
    // Try with module prefix
    let local = format!("{}.{}", module_path, expanded);
    if let Some(id) = def_map.lookup(&local) {
        return Some(id);
    }
    // Return a dummy DefId for numeric and boolean constants so they resolve without error
    if name.is_single() {
        let text = name.last().as_str();
        if text
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_digit() || c == '-')
            || text == "true"
            || text == "false"
        {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            let hash = hasher.finish() as u32;
            let dummy_id = 0x2000_0000 | (hash & 0x1FFF_FFFF);
            return Some(crate::def::DefId(dummy_id));
        }
    }
    sink.emit(
        Diagnostic::error(
            "type.undefined_name",
            format!("undefined type `{}`", path_str),
        )
        .with_label(Label::primary(file_id, span, "not defined in scope"))
        .with_action("ensure the type is defined in this namespace or imported with `import`"),
    );
    None
}

fn expand_alias(name: &str, aliases: &HashMap<SmolStr, SmolStr>) -> String {
    let mut prefix_end = name.len();
    loop {
        let prefix = &name[..prefix_end];
        if let Some(full_ns) = aliases.get(prefix) {
            let suffix = &name[prefix_end..];
            if suffix.is_empty() {
                return full_ns.to_string();
            }
            return format!("{}{}", full_ns, suffix);
        }
        if let Some(dot) = name[..prefix_end].rfind('.') {
            prefix_end = dot;
        } else {
            break;
        }
    }
    name.to_string()
}
