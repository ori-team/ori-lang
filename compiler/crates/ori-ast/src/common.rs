use ori_diagnostics::Span;
use smol_str::SmolStr;

/// An interned identifier string together with its source location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    pub text: SmolStr,
    pub span: Span,
}

impl Name {
    pub fn new(text: impl Into<SmolStr>, span: Span) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// A dot-separated qualified name: `ori.io` or `app.user`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    pub parts: Vec<Name>,
    pub span: Span,
}

impl QualifiedName {
    pub fn single(name: Name) -> Self {
        let span = name.span;
        Self {
            parts: vec![name],
            span,
        }
    }

    pub fn last(&self) -> &Name {
        self.parts
            .last()
            .expect("QualifiedName is always non-empty")
    }

    pub fn is_single(&self) -> bool {
        self.parts.len() == 1
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            f.write_str(&part.text)?;
        }
        Ok(())
    }
}

/// Whether a declaration is publicly visible outside its namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

impl Visibility {
    pub fn is_public(self) -> bool {
        self == Visibility::Public
    }
}

/// A list of generic type parameters: `[T, U]` (S3 surface).
pub type TypeParams = Vec<TypeParam>;

/// A single generic type parameter name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    pub name: Name,
}

/// A `where` clause constraining generic type parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereClause {
    pub constraints: Vec<WhereConstraint>,
    pub span: Span,
}

/// A single constraint inside a `where` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhereConstraint {
    /// `T is Trait`
    Is {
        param: Name,
        bound: QualifiedName,
        span: Span,
    },
    /// `T is not Trait`
    IsNot {
        param: Name,
        bound: QualifiedName,
        span: Span,
    },
}

/// An `@attr` or `@attr(args)` annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: Name,
    pub args: Vec<AttrArg>,
    pub span: Span,
}

/// A single argument inside an attribute: `"string"` or `key: value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrArg {
    String(SmolStr, Span),
    Named { key: Name, value: Name },
}
