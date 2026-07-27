use crate::common::{Name, QualifiedName};
use crate::stmt::Block;
use crate::ty::Type;
use ori_diagnostics::Span;
use smol_str::SmolStr;

/// Every expression in Ori.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // ── Literals ─────────────────────────────────────────────────────────────
    BoolLit(bool, Span),
    /// Raw source text is kept for arbitrary-precision parsing later.
    IntLit {
        raw: SmolStr,
        span: Span,
    },
    FloatLit {
        raw: SmolStr,
        span: Span,
    },
    StrLit {
        value: SmolStr,
        span: Span,
    },
    FStrLit {
        parts: Vec<FStrPart>,
        span: Span,
    },
    BytesLit {
        bytes: Vec<u8>,
        span: Span,
    },
    None(Span),

    // ── Name references ──────────────────────────────────────────────────────
    Ident(Name),
    QualifiedIdent(QualifiedName),
    SelfExpr(Span),

    // ── Range ────────────────────────────────────────────────────────────────
    /// `start..end` — always inclusive both ends.
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        span: Span,
    },

    // ── Collection literals ──────────────────────────────────────────────────
    List {
        elements: Vec<Expr>,
        span: Span,
    },
    Map {
        entries: Vec<(Expr, Expr)>,
        span: Span,
    },
    Set {
        elements: Vec<Expr>,
        span: Span,
    },
    Tuple {
        elements: Vec<Expr>,
        span: Span,
    },

    // ── Struct / enum construction ────────────────────────────────────────────
    /// `Point { x: 0, y: 0 }` — explicit type + braced fields (S3).
    StructLit {
        ty: QualifiedName,
        fields: Vec<FieldInit>,
        span: Span,
    },
    /// `{ x: 0, y: 0 }` — anonymous form; type resolved by checker (S3).
    AnonStructLit {
        fields: Vec<FieldInit>,
        span: Span,
    },
    /// `Direction.North` or `.North` (shorthand).
    EnumVariantUnit {
        ty: Option<QualifiedName>,
        variant: Name,
        span: Span,
    },
    /// `Shape.Circle(radius: 5.0)` or `.Circle(radius: 5.0)`.
    EnumVariantNamed {
        ty: Option<QualifiedName>,
        variant: Name,
        fields: Vec<FieldInit>,
        span: Span,
    },

    // ── Operators ────────────────────────────────────────────────────────────
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },

    // ── Postfix operations ───────────────────────────────────────────────────
    Field {
        object: Box<Expr>,
        field: Name,
        span: Span,
    },
    TupleIndex {
        object: Box<Expr>,
        index: u32,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: IndexExpr,
        span: Span,
    },
    /// `try expr` — propagate error / absence (postfix `expr?` removed in S3).
    Try {
        expr: Box<Expr>,
        span: Span,
    },
    /// `await expr` waits for a `future<T>` and produces `T`.
    Await {
        expr: Box<Expr>,
        span: Span,
    },
    /// `a |> f` — pipe into function.
    Pipe {
        value: Box<Expr>,
        func: Box<Expr>,
        span: Span,
    },

    // ── Inline control flow ──────────────────────────────────────────────────
    /// `if cond then a else b`
    IfExpr {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },

    /// `match scrutinee case p: expr … case else: expr end` in expression
    /// position. Each arm's body is a single expression; all arms must agree
    /// on one type. The statement form (`Stmt::Match`, bodies are statement
    /// lists) stays separate — position decides the form, as with `if`.
    MatchExpr {
        scrutinee: Box<Expr>,
        arms: Vec<MatchExprArm>,
        span: Span,
    },

    // ── Closures ─────────────────────────────────────────────────────────────
    Closure(Box<ClosureExpr>),

    // ── Struct update ─────────────────────────────────────────────────────────
    /// `original with { field: value } end`
    StructUpdate {
        base: Box<Expr>,
        updates: Vec<FieldInit>,
        span: Span,
    },

    // ── Type check ───────────────────────────────────────────────────────────
    /// `expr is TypeName` — runtime type narrowing on `any<Trait>`.
    IsCheck {
        value: Box<Expr>,
        ty: QualifiedName,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::BoolLit(_, s) | Expr::None(s) | Expr::SelfExpr(s) => *s,
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StrLit { span, .. }
            | Expr::FStrLit { span, .. }
            | Expr::BytesLit { span, .. } => *span,
            Expr::Ident(n) => n.span,
            Expr::QualifiedIdent(q) => q.span,
            Expr::Range { span, .. }
            | Expr::List { span, .. }
            | Expr::Map { span, .. }
            | Expr::Set { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::StructLit { span, .. }
            | Expr::AnonStructLit { span, .. }
            | Expr::EnumVariantUnit { span, .. }
            | Expr::EnumVariantNamed { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Field { span, .. }
            | Expr::TupleIndex { span, .. }
            | Expr::Call { span, .. }
            | Expr::Index { span, .. }
            | Expr::Try { span, .. }
            | Expr::Await { span, .. }
            | Expr::Pipe { span, .. }
            | Expr::IfExpr { span, .. }
            | Expr::MatchExpr { span, .. }
            | Expr::StructUpdate { span, .. }
            | Expr::IsCheck { span, .. } => *span,
            Expr::Closure(c) => c.span,
        }
    }
}

// ── Supporting types ──────────────────────────────────────────────────────────

/// A part of an interpolated string: raw text or an embedded expression.
#[derive(Debug, Clone, PartialEq)]
pub enum FStrPart {
    Literal(SmolStr),
    Interpolated(Box<Expr>),
}

/// A named field initialiser: `name: expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    pub name: Name,
    pub value: Box<Expr>,
    pub span: Span,
}

/// A call argument: positional, named, or spread (`..list`).
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub label: Option<Name>,
    pub value: ArgValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    Expr(Box<Expr>),
    Spread(Box<Expr>),
}

/// Index / slice expression inside `[…]`.
#[derive(Debug, Clone, PartialEq)]
pub enum IndexExpr {
    /// `obj[i]`
    Single(Box<Expr>),
    /// `obj[a..b]`, `obj[a..]`, `obj[..b]`, `obj[..]`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// One arm of a `match` used as an expression: `case pattern [if guard]: expr`.
///
/// `pattern: None` is the `case else:` arm, which must come last.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExprArm {
    pub pattern: Option<crate::pattern::Pattern>,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
    pub span: Span,
}

/// A closure expression: `(params) => expr` or `(params) block end`.
///
/// S3 surface: no `do` / `fn` / `given` keyword; parameter types may be omitted
/// when the surrounding call provides context (e.g. `iter.map`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureExpr {
    pub params: Vec<ClosureParam>,
    pub return_ty: Option<Type>,
    pub body: ClosureBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosureParam {
    pub name: Name,
    /// Omitted when written as `(u) => …`; filled by type checking context.
    pub ty: Option<Type>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClosureBody {
    Expr(Box<Expr>),
    Block(Block),
}
