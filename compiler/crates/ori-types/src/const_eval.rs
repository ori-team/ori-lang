use crate::def::{
    CompileTimeValue, ConstEvalFailure, ConstEvalFailureKind, ConstEvaluation, DefId, DefKind,
    DefMap,
};
use crate::literal::parse_int_literal;
use crate::resolve::{import_aliases, ReExport};
use ori_ast::common::QualifiedName;
use ori_ast::expr::{BinaryOp, UnaryOp};
use ori_ast::item::{Item, SourceFile};
use ori_ast::ty::{ConstExpr, Type};
use ori_diagnostics::{FileId, Span};
use smol_str::SmolStr;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclaredScalar {
    Int,
    Bool,
    Unsupported,
}

#[derive(Debug, Clone)]
struct ConstContext {
    namespace: SmolStr,
    aliases: HashMap<SmolStr, SmolStr>,
    file_id: FileId,
}

#[derive(Debug, Clone)]
struct ConstDeclaration {
    expression: Result<ConstExpr, Span>,
    declared_scalar: DeclaredScalar,
    context: ConstContext,
}

/// Evaluate module constants without emitting diagnostics.
///
/// Runtime-only `const` initializers remain legal. Their failure is recorded
/// and becomes a diagnostic only if a type-level constant expression refers
/// to them.
pub fn collect_module_const_evaluations(
    files: &[(&SourceFile, FileId)],
    reexports: &[ReExport],
    def_map: &DefMap,
) -> HashMap<DefId, ConstEvaluation> {
    let mut declarations = HashMap::new();
    for (file, file_id) in files {
        let namespace = SmolStr::new(file.namespace.name.to_string());
        let context = ConstContext {
            namespace: namespace.clone(),
            aliases: import_aliases(file, reexports),
            file_id: *file_id,
        };
        for item in &file.items {
            let Item::Const(declaration) = &item.item else {
                continue;
            };
            let path = format!("{}.{}", namespace, declaration.name.text);
            let Some(def_id) = def_map.lookup(&path) else {
                continue;
            };
            declarations.insert(
                def_id,
                ConstDeclaration {
                    expression: ConstExpr::from_expr((*declaration.value).clone()),
                    declared_scalar: declared_scalar(&declaration.ty),
                    context: context.clone(),
                },
            );
        }
    }

    let mut evaluator = ModuleConstEvaluator {
        def_map,
        declarations,
        evaluations: HashMap::new(),
        stack: Vec::new(),
    };
    let ids: Vec<DefId> = evaluator.declarations.keys().copied().collect();
    for id in ids {
        let Some(file_id) = evaluator
            .declarations
            .get(&id)
            .map(|declaration| declaration.context.file_id)
        else {
            continue;
        };
        let _ = evaluator.evaluate_definition(id, file_id);
    }
    evaluator.evaluations
}

/// Evaluate a type-level CT-0 expression after module constants are available.
pub fn evaluate_type_const_expr(
    expression: &ConstExpr,
    namespace: &str,
    aliases: &HashMap<SmolStr, SmolStr>,
    def_map: &DefMap,
    file_id: FileId,
) -> ConstEvaluation {
    evaluate_expression(expression, file_id, &mut |name| {
        let Some(def_id) = resolve_name(name, namespace, aliases, def_map) else {
            return Err(failure(
                ConstEvalFailureKind::UndefinedName,
                file_id,
                name.span,
                format!("compile-time name `{name}` was not found"),
            ));
        };
        let definition = def_map.get(def_id);
        if definition.kind != DefKind::Const {
            return Err(failure(
                ConstEvalFailureKind::NonConstName,
                file_id,
                name.span,
                format!("`{name}` is not a module `const`"),
            ));
        }
        if !definition.is_public && !definition.path.starts_with(&format!("{namespace}.")) {
            return Err(failure(
                ConstEvalFailureKind::PrivateName,
                file_id,
                name.span,
                format!("compile-time constant `{name}` is private"),
            ));
        }
        def_map
            .const_evaluation(def_id)
            .cloned()
            .unwrap_or_else(|| {
                Err(failure(
                    ConstEvalFailureKind::UnsupportedExpression,
                    file_id,
                    name.span,
                    format!("`{name}` has no compile-time scalar value"),
                ))
            })
    })
}

struct ModuleConstEvaluator<'a> {
    def_map: &'a DefMap,
    declarations: HashMap<DefId, ConstDeclaration>,
    evaluations: HashMap<DefId, ConstEvaluation>,
    stack: Vec<DefId>,
}

impl ModuleConstEvaluator<'_> {
    fn evaluate_definition(&mut self, def_id: DefId, fallback_file_id: FileId) -> ConstEvaluation {
        if let Some(value) = self.evaluations.get(&def_id) {
            return value.clone();
        }
        if let Some(cycle_start) = self.stack.iter().position(|current| *current == def_id) {
            let cycle = self.stack[cycle_start..]
                .iter()
                .chain(std::iter::once(&def_id))
                .map(|id| self.def_map.get(*id).name.as_str())
                .collect::<Vec<_>>()
                .join(" -> ");
            let (file_id, span) = self
                .declarations
                .get(&def_id)
                .map(|declaration| (declaration.context.file_id, self.def_map.get(def_id).span))
                .unwrap_or((fallback_file_id, self.def_map.get(def_id).span));
            return Err(failure(
                ConstEvalFailureKind::Cycle,
                file_id,
                span,
                format!("compile-time constant cycle: {cycle}"),
            ));
        }

        let Some(declaration) = self.declarations.get(&def_id).cloned() else {
            return Err(failure(
                ConstEvalFailureKind::UnsupportedExpression,
                fallback_file_id,
                self.def_map.get(def_id).span,
                "compile-time constant has no source declaration",
            ));
        };
        let expression = match declaration.expression {
            Ok(expression) => expression,
            Err(span) => {
                let result = Err(failure(
                    ConstEvalFailureKind::UnsupportedExpression,
                    declaration.context.file_id,
                    span,
                    "module constant initializer uses a runtime-only expression",
                ));
                self.evaluations.insert(def_id, result.clone());
                return result;
            }
        };
        if declaration.declared_scalar == DeclaredScalar::Unsupported {
            let result = Err(failure(
                ConstEvalFailureKind::TypeMismatch,
                declaration.context.file_id,
                self.def_map.get(def_id).span,
                "only integer and boolean module constants have CT-0 values",
            ));
            self.evaluations.insert(def_id, result.clone());
            return result;
        }

        self.stack.push(def_id);
        let mut result =
            evaluate_expression(&expression, declaration.context.file_id, &mut |name| {
                self.evaluate_reference(name, &declaration.context)
            });
        self.stack.pop();

        if let Ok(value) = result {
            let matches_declaration = matches!(
                (declaration.declared_scalar, value),
                (DeclaredScalar::Int, CompileTimeValue::Int(_))
                    | (DeclaredScalar::Bool, CompileTimeValue::Bool(_))
            );
            if !matches_declaration {
                result = Err(failure(
                    ConstEvalFailureKind::TypeMismatch,
                    declaration.context.file_id,
                    expression.span(),
                    "compile-time value does not match the constant's declared type",
                ));
            }
        }
        self.evaluations.insert(def_id, result.clone());
        result
    }

    fn evaluate_reference(
        &mut self,
        name: &QualifiedName,
        context: &ConstContext,
    ) -> ConstEvaluation {
        let Some(def_id) = resolve_name(name, &context.namespace, &context.aliases, self.def_map)
        else {
            return Err(failure(
                ConstEvalFailureKind::UndefinedName,
                context.file_id,
                name.span,
                format!("compile-time name `{name}` was not found"),
            ));
        };
        let definition = self.def_map.get(def_id);
        if definition.kind != DefKind::Const {
            return Err(failure(
                ConstEvalFailureKind::NonConstName,
                context.file_id,
                name.span,
                format!("`{name}` is not a module `const`"),
            ));
        }
        if !definition.is_public
            && !definition
                .path
                .starts_with(&format!("{}.", context.namespace))
        {
            return Err(failure(
                ConstEvalFailureKind::PrivateName,
                context.file_id,
                name.span,
                format!("compile-time constant `{name}` is private"),
            ));
        }
        self.evaluate_definition(def_id, context.file_id)
    }
}

fn evaluate_expression(
    expression: &ConstExpr,
    file_id: FileId,
    resolve: &mut impl FnMut(&QualifiedName) -> ConstEvaluation,
) -> ConstEvaluation {
    match expression {
        ConstExpr::Int { raw, span } => parse_int_literal(raw)
            .and_then(|literal| {
                if matches!(literal.ty, crate::Ty::U64) && literal.value < 0 {
                    return Err(crate::literal::NumericLiteralError {
                        kind: crate::literal::NumericLiteralErrorKind::OutOfRange,
                        message: format!(
                            "integer literal `{raw}` is outside the CT-0 signed integer range"
                        ),
                    });
                }
                Ok(literal)
            })
            .map(|literal| CompileTimeValue::Int(literal.value))
            .map_err(|error| {
                failure(
                    ConstEvalFailureKind::InvalidLiteral,
                    file_id,
                    *span,
                    error.message,
                )
            }),
        ConstExpr::Bool(value, _) => Ok(CompileTimeValue::Bool(*value)),
        ConstExpr::Name(name) => resolve(name),
        ConstExpr::Unary { op, operand, span } => {
            let value = evaluate_expression(operand, file_id, resolve)?;
            match (op, value) {
                (UnaryOp::Neg, CompileTimeValue::Int(value)) => value
                    .checked_neg()
                    .map(CompileTimeValue::Int)
                    .ok_or_else(|| overflow(file_id, *span)),
                (UnaryOp::Not, CompileTimeValue::Bool(value)) => Ok(CompileTimeValue::Bool(!value)),
                (UnaryOp::BitNot, CompileTimeValue::Int(value)) => {
                    Ok(CompileTimeValue::Int(!value))
                }
                _ => Err(type_mismatch(
                    file_id,
                    *span,
                    "invalid operand type for unary operator",
                )),
            }
        }
        ConstExpr::Binary { op, lhs, rhs, span } => {
            evaluate_binary(*op, lhs, rhs, file_id, *span, resolve)
        }
        ConstExpr::If {
            condition,
            then_expr,
            else_expr,
            span,
        } => match evaluate_expression(condition, file_id, resolve)? {
            CompileTimeValue::Bool(true) => evaluate_expression(then_expr, file_id, resolve),
            CompileTimeValue::Bool(false) => evaluate_expression(else_expr, file_id, resolve),
            CompileTimeValue::Int(_) => Err(type_mismatch(
                file_id,
                *span,
                "compile-time `if` condition must be boolean",
            )),
        },
    }
}

fn evaluate_binary(
    op: BinaryOp,
    lhs: &ConstExpr,
    rhs: &ConstExpr,
    file_id: FileId,
    span: Span,
    resolve: &mut impl FnMut(&QualifiedName) -> ConstEvaluation,
) -> ConstEvaluation {
    let left = evaluate_expression(lhs, file_id, resolve)?;
    if op == BinaryOp::And {
        return match left {
            CompileTimeValue::Bool(false) => Ok(CompileTimeValue::Bool(false)),
            CompileTimeValue::Bool(true) => match evaluate_expression(rhs, file_id, resolve)? {
                CompileTimeValue::Bool(value) => Ok(CompileTimeValue::Bool(value)),
                _ => Err(type_mismatch(
                    file_id,
                    span,
                    "`and` requires boolean operands",
                )),
            },
            _ => Err(type_mismatch(
                file_id,
                span,
                "`and` requires boolean operands",
            )),
        };
    }
    if op == BinaryOp::Or {
        return match left {
            CompileTimeValue::Bool(true) => Ok(CompileTimeValue::Bool(true)),
            CompileTimeValue::Bool(false) => match evaluate_expression(rhs, file_id, resolve)? {
                CompileTimeValue::Bool(value) => Ok(CompileTimeValue::Bool(value)),
                _ => Err(type_mismatch(
                    file_id,
                    span,
                    "`or` requires boolean operands",
                )),
            },
            _ => Err(type_mismatch(
                file_id,
                span,
                "`or` requires boolean operands",
            )),
        };
    }
    let right = evaluate_expression(rhs, file_id, resolve)?;
    match (left, right) {
        (CompileTimeValue::Int(left), CompileTimeValue::Int(right)) => {
            evaluate_integer_binary(op, left, right, file_id, span)
        }
        (CompileTimeValue::Bool(left), CompileTimeValue::Bool(right)) => match op {
            BinaryOp::Eq => Ok(CompileTimeValue::Bool(left == right)),
            BinaryOp::Ne => Ok(CompileTimeValue::Bool(left != right)),
            _ => Err(type_mismatch(
                file_id,
                span,
                "boolean compile-time values support only `==`, `!=`, `and`, and `or`",
            )),
        },
        _ => Err(type_mismatch(
            file_id,
            span,
            "compile-time binary operands must have the same scalar type",
        )),
    }
}

fn evaluate_integer_binary(
    op: BinaryOp,
    left: i64,
    right: i64,
    file_id: FileId,
    span: Span,
) -> ConstEvaluation {
    let integer = match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Sub => left.checked_sub(right),
        BinaryOp::Mul => left.checked_mul(right),
        BinaryOp::Div if right == 0 => return Err(division_by_zero(file_id, span)),
        BinaryOp::Div => left.checked_div(right),
        BinaryOp::Rem if right == 0 => return Err(division_by_zero(file_id, span)),
        BinaryOp::Rem => left.checked_rem(right),
        BinaryOp::Eq => return Ok(CompileTimeValue::Bool(left == right)),
        BinaryOp::Ne => return Ok(CompileTimeValue::Bool(left != right)),
        BinaryOp::Lt => return Ok(CompileTimeValue::Bool(left < right)),
        BinaryOp::Le => return Ok(CompileTimeValue::Bool(left <= right)),
        BinaryOp::Gt => return Ok(CompileTimeValue::Bool(left > right)),
        BinaryOp::Ge => return Ok(CompileTimeValue::Bool(left >= right)),
        BinaryOp::And | BinaryOp::Or => unreachable!("boolean operators handled before integers"),
        BinaryOp::Band => Some(left & right),
        BinaryOp::Bor => Some(left | right),
        BinaryOp::Bxor => Some(left ^ right),
        // Shifts are well-defined in two's-complement i64; the width rules are
        // enforced at runtime. Negative shifts are rejected here (UB in most
        // languages; Ori keeps CT-0 deterministic).
        BinaryOp::Shl if right < 0 || right >= 64 => {
            return Err(type_mismatch(
                file_id,
                span,
                "compile-time shift count must be in 0..64",
            ))
        }
        BinaryOp::Shl => Some(left.wrapping_shl(right as u32)),
        BinaryOp::Shr if right < 0 || right >= 64 => {
            return Err(type_mismatch(
                file_id,
                span,
                "compile-time shift count must be in 0..64",
            ))
        }
        BinaryOp::Shr => Some(left >> right),
    };
    integer
        .map(CompileTimeValue::Int)
        .ok_or_else(|| overflow(file_id, span))
}

fn declared_scalar(ty: &Type) -> DeclaredScalar {
    match ty {
        Type::Bool(_) => DeclaredScalar::Bool,
        Type::Int(_)
        | Type::Int8(_)
        | Type::Int16(_)
        | Type::Int32(_)
        | Type::Int64(_)
        | Type::U8(_)
        | Type::U16(_)
        | Type::U32(_)
        | Type::U64(_) => DeclaredScalar::Int,
        _ => DeclaredScalar::Unsupported,
    }
}

fn resolve_name(
    name: &QualifiedName,
    namespace: &str,
    aliases: &HashMap<SmolStr, SmolStr>,
    def_map: &DefMap,
) -> Option<DefId> {
    let raw = name.to_string();
    let expanded = expand_alias(&raw, aliases);
    def_map
        .lookup(&expanded)
        .or_else(|| def_map.lookup(&format!("{namespace}.{expanded}")))
}

fn expand_alias(name: &str, aliases: &HashMap<SmolStr, SmolStr>) -> String {
    let mut prefix_end = name.len();
    loop {
        let prefix = &name[..prefix_end];
        if let Some(target) = aliases.get(prefix) {
            return format!("{}{}", target, &name[prefix_end..]);
        }
        let Some(dot) = name[..prefix_end].rfind('.') else {
            return name.to_owned();
        };
        prefix_end = dot;
    }
}

fn failure(
    kind: ConstEvalFailureKind,
    file_id: FileId,
    span: Span,
    detail: impl Into<String>,
) -> ConstEvalFailure {
    ConstEvalFailure {
        kind,
        file_id,
        span,
        detail: detail.into(),
    }
}

fn overflow(file_id: FileId, span: Span) -> ConstEvalFailure {
    failure(
        ConstEvalFailureKind::Overflow,
        file_id,
        span,
        "integer arithmetic overflowed the CT-0 `int` range",
    )
}

fn division_by_zero(file_id: FileId, span: Span) -> ConstEvalFailure {
    failure(
        ConstEvalFailureKind::DivisionByZero,
        file_id,
        span,
        "division or remainder by zero in a compile-time expression",
    )
}

fn type_mismatch(file_id: FileId, span: Span, detail: impl Into<String>) -> ConstEvalFailure {
    failure(ConstEvalFailureKind::TypeMismatch, file_id, span, detail)
}
