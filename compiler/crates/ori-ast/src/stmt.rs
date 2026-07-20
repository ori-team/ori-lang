use crate::common::{Name, QualifiedName};
use crate::expr::Expr;
use crate::pattern::Pattern;
use crate::ty::Type;
use ori_diagnostics::Span;
use smol_str::SmolStr;

/// A sequence of statements closed by `end`.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// Every statement that can appear inside a function body.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Const(LocalConst),
    Var(LocalVar),
    /// `const Point { x, y } = expr` / `var { x, y } = expr`
    Destructure(LocalDestructure),
    Assign(AssignStmt),
    CompoundAssign(CompoundAssignStmt),
    Return(ReturnStmt),
    Break(Span),
    Continue(Span),
    If(IfStmt),
    IfSome(IfSomeStmt),
    While(WhileStmt),
    WhileSome(WhileSomeStmt),
    For(ForStmt),
    Repeat(RepeatStmt),
    Loop(LoopStmt),
    Match(MatchStmt),
    Using(UsingStmt),
    Check(CheckStmt),
    /// An expression used as a statement (return value discarded).
    Expr(Box<Expr>),
}

// ── Bindings ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct LocalConst {
    pub name: Name,
    /// Explicit type, or `None` when omitted for local Nim-style inference (`0.3.1`).
    pub ty: Option<Type>,
    pub value: Box<Expr>,
    pub span: Span,
}

/// `const Point { x, y } = get_pos()` — bind several struct fields at once.
///
/// Only struct fields: Ori has tuples, but binding them positionally
/// (`.0`, `.1`) would make the reader carry "what was field 2 again?", which
/// is exactly the cost this form is meant to remove.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalDestructure {
    /// `false` for `const`, `true` for `var`.
    pub is_mutable: bool,
    /// `Point` in `const Point { … }`; `None` when the type is left to
    /// inference (allowed under the same rule as option-B local inference).
    pub type_name: Option<QualifiedName>,
    /// `(field, bound name)` — equal when written in shorthand (`x`), and
    /// different when renamed (`x: px`).
    pub fields: Vec<(Name, Name)>,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalVar {
    pub name: Name,
    /// Explicit type, or `None` when omitted for local Nim-style inference (`0.3.1`).
    pub ty: Option<Type>,
    pub value: Box<Expr>,
    pub span: Span,
}

// ── Assignment ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub lvalue: LValue,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompoundAssignStmt {
    pub lvalue: LValue,
    pub op: CompoundOp,
    pub value: Box<Expr>,
    pub span: Span,
}

/// An assignable location: a variable, field, or index.
#[derive(Debug, Clone, PartialEq)]
pub enum LValue {
    Ident(Name),
    Field {
        base: Box<LValue>,
        field: Name,
        span: Span,
    },
    Index {
        base: Box<LValue>,
        index: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundOp {
    Add,
    Sub,
    Mul,
    Div,
}

// ── Control flow ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub condition: Box<Expr>,
    pub then_block: Block,
    /// `elif cond …` chains (historical field name; surface form is `elif`).
    pub else_ifs: Vec<(Box<Expr>, Block)>,
    pub else_block: Option<Block>,
    pub span: Span,
}

/// Which wrapper a conditional binding unwraps, and which side it binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwrapKind {
    /// `if some(x) = optional_expr` — binds the payload when present.
    Some,
    /// `if ok(v) = result_expr` — binds the success value.
    Ok,
    /// `if err(e) = result_expr` — binds the error value (branch taken when
    /// the result is **not** ok).
    Err,
}

/// Conditional unwrap binding: `if some(x) = …`, `if ok(v) = …`,
/// `if err(e) = …`, each with an optional `else`.
///
/// One node covers all three because they differ only in which wrapper is
/// inspected and which side is bound; `kind` carries that difference.
#[derive(Debug, Clone, PartialEq)]
pub struct IfSomeStmt {
    pub kind: UnwrapKind,
    pub binding: Name,
    pub value: Box<Expr>,
    pub then_block: Block,
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub condition: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

/// `while some(binding) = expr … end`
#[derive(Debug, Clone, PartialEq)]
pub struct WhileSomeStmt {
    pub binding: Name,
    pub value: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub binding: Name,
    pub second_binding: Option<Name>,
    pub iterable: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepeatStmt {
    pub count: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopStmt {
    pub body: Block,
    pub span: Span,
}

// ── Match ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct MatchStmt {
    pub scrutinee: Box<Expr>,
    pub cases: Vec<MatchCase>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchCase {
    /// `case pattern [if guard]: stmts`
    Pattern {
        pattern: Pattern,
        guard: Option<Box<Expr>>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// `case else: stmts`
    Else { body: Vec<Stmt>, span: Span },
}

// ── Resource cleanup ──────────────────────────────────────────────────────────

/// `using name: Type = expr`
#[derive(Debug, Clone, PartialEq)]
pub struct UsingStmt {
    pub name: Name,
    pub ty: Type,
    pub value: Box<Expr>,
    pub span: Span,
}

// ── Assertion ─────────────────────────────────────────────────────────────────

/// `check expr` or `check expr, "message"`
#[derive(Debug, Clone, PartialEq)]
pub struct CheckStmt {
    pub condition: Box<Expr>,
    pub message: Option<SmolStr>,
    pub span: Span,
}
