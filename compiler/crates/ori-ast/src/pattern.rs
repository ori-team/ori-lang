use crate::common::Name;
use crate::expr::Expr;
use ori_diagnostics::Span;

/// A pattern in a `match` arm.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` — matches anything, binds nothing.
    Wildcard(Span),

    /// A literal value: `42`, `"hello"`, `true`.
    Literal(Box<Expr>),

    /// A plain identifier — creates a binding: `n`, `user`.
    Binding(Name),

    /// `Direction.North` or `.North` — unit enum variant.
    VariantUnit {
        name: Name,
        shorthand: bool,
        span: Span,
    },

    /// `Shape.Circle(radius: r)` or `.Circle(radius: r)`.
    VariantNamed {
        name: Name,
        fields: Vec<NamedPattern>,
        shorthand: bool,
        span: Span,
    },

    /// `some(inner)` — optional presence.
    Some(Box<Pattern>, Span),

    /// `none` — optional absence.
    None(Span),

    /// `ok(inner)` — result success.
    Ok(Box<Pattern>, Span),

    /// `err(inner)` — result failure.
    Err(Box<Pattern>, Span),

    /// `tuple(a, b, c)` — tuple destructuring.
    Tuple(Vec<Pattern>, Span),

    /// `case North or South:` — one arm, several alternatives.
    ///
    /// Alternatives may not bind anything: every branch would have to bind the
    /// same names to the same types, which is a rule readers must carry in
    /// their head. Keeping it binding-free means an or-pattern is a pure
    /// "is it one of these?" test.
    Or(Vec<Pattern>, Span),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(s) | Pattern::None(s) => *s,
            Pattern::Literal(e) => e.span(),
            Pattern::Binding(n) => n.span,
            Pattern::VariantUnit { span, .. } | Pattern::VariantNamed { span, .. } => *span,
            Pattern::Some(_, s) | Pattern::Ok(_, s) | Pattern::Err(_, s) => *s,
            Pattern::Tuple(_, s) | Pattern::Or(_, s) => *s,
        }
    }
}

/// A named field inside a variant pattern: `radius: r` or bare `radius`.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedPattern {
    /// Field name.
    pub name: Name,
    /// Sub-pattern. When the source uses the bare shorthand `field`,
    /// this is `Pattern::Binding(field)`.
    pub pattern: Pattern,
    pub span: Span,
}
