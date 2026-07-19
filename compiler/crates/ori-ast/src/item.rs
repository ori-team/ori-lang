use crate::common::{Attr, Name, QualifiedName, TypeParams, Visibility, WhereClause};
use crate::expr::Expr;
use crate::stmt::Block;
use crate::ty::Type;
use ori_diagnostics::Span;

// ── Source file ───────────────────────────────────────────────────────────────

/// The root node: one `.orl` source file.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub namespace: NamespaceDecl,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<ItemWithAttrs>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamespaceDecl {
    pub name: QualifiedName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub visibility: Visibility,
    pub path: QualifiedName,
    pub alias: Option<Name>,
    pub selected: Vec<ImportItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportItem {
    pub name: Name,
    pub alias: Option<Name>,
    pub span: Span,
}

// ── Top-level items ───────────────────────────────────────────────────────────

/// A top-level declaration optionally preceded by attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemWithAttrs {
    pub attrs: Vec<Attr>,
    pub item: Item,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Func(FuncDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    /// S3: `apply Type … end` with free members and `use Trait` sections.
    Apply(ApplyDecl),
    Alias(AliasDecl),
    /// `newtype UserId = int` — a distinct type over an existing
    /// representation (no implicit conversion in either direction).
    Newtype(NewtypeDecl),
    Const(TopConst),
    Var(TopVar),
    Extern(ExternBlock),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Func(f) => f.span,
            Item::Struct(s) => s.span,
            Item::Enum(e) => e.span,
            Item::Trait(t) => t.span,
            Item::Apply(a) => a.span,
            Item::Alias(a) => a.span,
            Item::Newtype(n) => n.span,
            Item::Const(c) => c.span,
            Item::Var(v) => v.span,
            Item::Extern(e) => e.span,
        }
    }
}

// ── Functions ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct FuncDecl {
    pub visibility: Visibility,
    pub is_async: bool,
    pub is_mut: bool,
    pub name: Name,
    pub type_params: TypeParams,
    pub params: Vec<Param>,
    pub return_ty: Option<Type>,
    pub where_clause: Option<WhereClause>,
    pub body: Block,
    pub span: Span,
}

/// A `func` declaration with no body (used inside trait declarations).
#[derive(Debug, Clone, PartialEq)]
pub struct FuncSignature {
    pub visibility: Visibility,
    pub is_async: bool,
    pub is_mut: bool,
    pub name: Name,
    pub type_params: TypeParams,
    pub params: Vec<Param>,
    pub return_ty: Option<Type>,
    pub where_clause: Option<WhereClause>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Name,
    pub ty: Type,
    pub kind: ParamKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    /// Plain required parameter.
    Required,
    /// `name: Type = default_expr`
    Default(Box<Expr>),
    /// `name: Type if it > 0`
    Contract(Box<Expr>),
    /// `name: Type = default_expr if it > 0`
    DefaultAndContract(Box<Expr>, Box<Expr>),
    /// `name: Type...`  (must be the last parameter)
    Variadic,
}

// ── Structs ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub visibility: Visibility,
    pub name: Name,
    pub type_params: TypeParams,
    pub where_clause: Option<WhereClause>,
    pub fields: Vec<StructField>,
    pub methods: Vec<FuncDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: Name,
    pub ty: Type,
    /// `if it > 0` value contract; `None` = no contract.
    pub contract: Option<Box<Expr>>,
    pub span: Span,
}

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub visibility: Visibility,
    pub name: Name,
    pub type_params: TypeParams,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Name,
    /// Empty = unit variant; non-empty = named-field variant.
    pub fields: Vec<NamedField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedField {
    pub name: Name,
    pub ty: Type,
    pub span: Span,
}

// ── Traits ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub visibility: Visibility,
    pub name: Name,
    pub type_params: TypeParams,
    pub where_clause: Option<WhereClause>,
    pub members: Vec<TraitMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraitMember {
    /// A required method — has signature but no body.
    Required(FuncSignature),
    /// A default method — has both signature and body.
    Default(FuncDecl),
    /// An associated type declaration.
    Type(Name),
}

// ── Apply (trait implementation surface, S3) ──────────────────────────────────

/// `apply [T] Type … end` — free methods/binds, then zero or more `use Trait` sections.
///
/// Order is fixed (Auk9-style): free members first, then `use` sections. Inside a
/// `use`, members may be inline methods or compile-time binds `slot = functionName`.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyDecl {
    pub type_params: TypeParams,
    pub for_type: QualifiedName,
    pub where_clause: Option<WhereClause>,
    /// Methods / binds before any `use` section (inherent-style on the type).
    pub free_members: Vec<ApplyMember>,
    pub uses: Vec<ApplyUseSection>,
    pub span: Span,
}

/// A member of an `apply` body or `use Trait` section.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplyMember {
    /// Inline method with body.
    Method(FuncDecl),
    /// Compile-time bind: `slot = freeFunction` (not a runtime assignment).
    Bind {
        slot: Name,
        target: Name,
        span: Span,
    },
}

/// `use Trait … end` inside `apply Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyUseSection {
    pub trait_name: QualifiedName,
    pub members: Vec<ApplyMember>,
    pub associated_types: Vec<(Name, Type)>,
    pub span: Span,
}

impl ApplyMember {
    pub fn span(&self) -> Span {
        match self {
            ApplyMember::Method(m) => m.span,
            ApplyMember::Bind { span, .. } => *span,
        }
    }

    pub fn slot_name(&self) -> &Name {
        match self {
            ApplyMember::Method(m) => &m.name,
            ApplyMember::Bind { slot, .. } => slot,
        }
    }
}

// ── Alias ─────────────────────────────────────────────────────────────────────

/// `alias Name<T> = Type`
#[derive(Debug, Clone, PartialEq)]
pub struct AliasDecl {
    pub visibility: Visibility,
    pub name: Name,
    pub type_params: TypeParams,
    pub ty: Type,
    pub span: Span,
}

/// `newtype UserId = int`.
///
/// The counterpart of `alias`: same representation, but a **distinct** type.
/// `alias` says "another name for this type" (values flow freely); `newtype`
/// says "a new type shaped like this one" (conversion is written out).
#[derive(Debug, Clone, PartialEq)]
pub struct NewtypeDecl {
    pub visibility: Visibility,
    pub name: Name,
    /// The representation type. Carries no runtime cost: it is erased when
    /// lowering to HIR, so a `newtype` over `int` *is* an `int` at runtime.
    pub repr: Type,
    pub span: Span,
}

// ── Top-level const / var ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TopConst {
    pub visibility: Visibility,
    pub name: Name,
    pub ty: Type,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopVar {
    pub visibility: Visibility,
    pub name: Name,
    pub ty: Type,
    pub value: Box<Expr>,
    pub span: Span,
}

// ── Extern ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ExternBlock {
    pub abi: AbiLabel,
    pub members: Vec<ExternMember>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiLabel {
    C,
    Host,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExternMember {
    Func {
        visibility: Visibility,
        name: Name,
        params: Vec<Param>,
        return_ty: Option<Type>,
        span: Span,
    },
    Var {
        visibility: Visibility,
        name: Name,
        ty: Type,
        span: Span,
    },
}
