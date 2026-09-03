use crate::common::{Name, QualifiedName};
use crate::expr::{BinaryOp, Expr, UnaryOp};
use ori_diagnostics::Span;

/// The side-effect-free expression behind a named const type argument.
///
/// This deliberately mirrors only the expression forms that CT-0 can evaluate.
/// Keeping it separate from [`Expr`] prevents calls, allocation, I/O, and other
/// runtime operations from reaching type lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstExpr {
    Int {
        raw: smol_str::SmolStr,
        span: Span,
    },
    Bool(bool, Span),
    Name(QualifiedName),
    Unary {
        op: UnaryOp,
        operand: Box<ConstExpr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<ConstExpr>,
        rhs: Box<ConstExpr>,
        span: Span,
    },
    If {
        condition: Box<ConstExpr>,
        then_expr: Box<ConstExpr>,
        else_expr: Box<ConstExpr>,
        span: Span,
    },
}

impl ConstExpr {
    /// Restrict a parsed runtime expression to the CT-0 expression subset.
    pub fn from_expr(expr: Expr) -> Result<Self, Span> {
        match expr {
            Expr::IntLit { raw, span } => Ok(Self::Int { raw, span }),
            Expr::BoolLit(value, span) => Ok(Self::Bool(value, span)),
            Expr::Ident(name) => Ok(Self::Name(QualifiedName::single(name))),
            Expr::QualifiedIdent(name) => Ok(Self::Name(name)),
            Expr::Unary { op, operand, span } => Ok(Self::Unary {
                op,
                operand: Box::new(Self::from_expr(*operand)?),
                span,
            }),
            Expr::Binary { op, lhs, rhs, span } => Ok(Self::Binary {
                op,
                lhs: Box::new(Self::from_expr(*lhs)?),
                rhs: Box::new(Self::from_expr(*rhs)?),
                span,
            }),
            Expr::IfExpr {
                condition,
                then_expr,
                else_expr,
                span,
            } => Ok(Self::If {
                condition: Box::new(Self::from_expr(*condition)?),
                then_expr: Box::new(Self::from_expr(*then_expr)?),
                else_expr: Box::new(Self::from_expr(*else_expr)?),
                span,
            }),
            other => Err(other.span()),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Int { span, .. }
            | Self::Bool(_, span)
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::If { span, .. } => *span,
            Self::Name(name) => name.span,
        }
    }
}

/// Every type that can appear in an Ori program.
///
/// Primitive types are explicit variants so the type checker can recognise them
/// without a symbol-table lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    // ── Primitive types ──────────────────────────────────────────────────────
    Bool(Span),
    Int(Span),
    Int8(Span),
    Int16(Span),
    Int32(Span),
    Int64(Span),
    U8(Span),
    U16(Span),
    U32(Span),
    U64(Span),
    Float(Span),
    Float32(Span),
    Float64(Span),
    String(Span),
    Bytes(Span),
    Void(Span),

    // ── Named types ───────────────────────────────────────────────────────────
    /// A user-defined type by name: `User`, `app.config.Config`.
    Named(QualifiedName),

    /// A compile-time constant used as a type argument: `size: 8` in
    /// `Buffer[size: 8]`.
    ///
    /// Named on purpose. A bare number between brackets reads as an index
    /// everywhere else in Ori, so const arguments carry the parameter's name
    /// the same way call arguments and struct fields do.
    ConstArg {
        name: Name,
        value: ConstExpr,
        span: Span,
    },

    // ── Built-in generic types ────────────────────────────────────────────────
    Optional(Box<Type>, Span),
    Result(Box<Type>, Box<Type>, Span),
    List(Box<Type>, Span),
    /// `buffer[T]` — a mutable, contiguous, fixed-length sequence.
    Buffer(Box<Type>, Span),
    /// `slice[T]` — a read-only window over a `list[T]`.
    Slice(Box<Type>, Span),
    /// `array[T, size: N]` — fixed length, part of the type.
    ///
    /// Distinct from `list[T]`: the length is known at compile time, so the
    /// elements live inline instead of behind a heap pointer, and two arrays of
    /// different lengths are different types.
    Array {
        elem: Box<Type>,
        size: ConstExpr,
        span: Span,
    },
    /// `simd[T, lanes: N]` or `simd[T, N]` — portable fixed-width SIMD vector (LANG-SIMD-1).
    Simd {
        elem: Box<Type>,
        lanes: u16,
        span: Span,
    },
    Map(Box<Type>, Box<Type>, Span),
    Set(Box<Type>, Span),
    Range(Box<Type>, Span),
    Lazy(Box<Type>, Span),
    Handle(Box<Type>, Span),
    /// `any<Trait>` — dynamic dispatch.
    Any(QualifiedName, Span),
    /// `tuple<A, B, …>` — always at least 2 type arguments.
    Tuple(Vec<Type>, Span),

    // ── Callable types ────────────────────────────────────────────────────────
    /// `func(T, U) -> R`  or `func(T)` (void return → `None`).
    Func {
        params: Vec<Type>,
        return_ty: Option<Box<Type>>,
        span: Span,
    },

    // ── User-defined generic types ────────────────────────────────────────────
    /// `MyContainer<T>`, `Either<Left, Right>`.
    Generic {
        name: QualifiedName,
        args: Vec<Type>,
        span: Span,
    },
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Bool(s)
            | Type::Int(s)
            | Type::Int8(s)
            | Type::Int16(s)
            | Type::Int32(s)
            | Type::Int64(s)
            | Type::U8(s)
            | Type::U16(s)
            | Type::U32(s)
            | Type::U64(s)
            | Type::Float(s)
            | Type::Float32(s)
            | Type::Float64(s)
            | Type::String(s)
            | Type::Bytes(s)
            | Type::Void(s) => *s,
            Type::Named(q) => q.span,
            Type::ConstArg { span, .. } => *span,
            Type::Buffer(_, s)
            | Type::Slice(_, s)
            | Type::Array { span: s, .. }
            | Type::Simd { span: s, .. }
            | Type::Optional(_, s)
            | Type::List(_, s)
            | Type::Set(_, s)
            | Type::Range(_, s)
            | Type::Lazy(_, s)
            | Type::Handle(_, s)
            | Type::Any(_, s)
            | Type::Tuple(_, s)
            | Type::Result(_, _, s)
            | Type::Map(_, _, s) => *s,
            Type::Func { span, .. } | Type::Generic { span, .. } => *span,
        }
    }
}
