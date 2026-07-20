use crate::common::{Name, QualifiedName};
use ori_diagnostics::Span;

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
        value: i64,
        span: Span,
    },

    // ── Built-in generic types ────────────────────────────────────────────────
    Optional(Box<Type>, Span),
    Result(Box<Type>, Box<Type>, Span),
    List(Box<Type>, Span),
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
            Type::Optional(_, s)
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
