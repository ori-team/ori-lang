use ori_ast::expr::{BinaryOp, UnaryOp};
use ori_diagnostics::Span;
use ori_types::{DefId, Ty};
use smol_str::SmolStr;

// ── Module ────────────────────────────────────────────────────────────────────

/// One source file lowered to HIR.
#[derive(Debug, Clone)]
pub struct HirModule {
    pub namespace: SmolStr,
    pub structs: Vec<HirStruct>,
    pub enums: Vec<HirEnum>,
    pub traits: Vec<HirTrait>,
    pub trait_impls: Vec<HirTraitImpl>,
    pub funcs: Vec<HirFunc>,
    pub consts: Vec<HirConst>,
    pub externs: Vec<HirExtern>,
}

// ── Declarations ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HirStruct {
    pub def_id: DefId,
    pub name: SmolStr,
    pub fields: Vec<HirField>,
    pub is_public: bool,
    pub repr_c: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirField {
    pub name: SmolStr,
    pub ty: Ty,
    pub contract: Option<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirEnum {
    pub def_id: DefId,
    pub name: SmolStr,
    pub variants: Vec<HirVariant>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirVariant {
    pub name: SmolStr,
    pub fields: Vec<HirField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirTrait {
    pub def_id: DefId,
    pub name: SmolStr,
    pub methods: Vec<HirTraitMethod>,
}

#[derive(Debug, Clone)]
pub struct HirTraitMethod {
    pub name: SmolStr,
    pub params: Vec<Ty>,
    pub return_ty: Ty,
    pub default_func_name: Option<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct HirTraitImpl {
    pub trait_def_id: DefId,
    pub type_def_id: DefId,
    pub methods: Vec<HirTraitImplMethod>,
}

#[derive(Debug, Clone)]
pub struct HirTraitImplMethod {
    pub name: SmolStr,
    pub func_name: SmolStr,
}

#[derive(Debug, Clone)]
pub struct HirFunc {
    pub def_id: DefId,
    pub name: SmolStr,
    pub params: Vec<HirParam>,
    pub return_ty: Ty,
    pub body: HirBlock,
    pub closure_captures: Vec<HirClosureCapture>,
    pub is_public: bool,
    pub is_async: bool,
    pub is_mut: bool,
    /// When set (`@c_export` / `@c_export("name")`), the native backend emits an
    /// additional C ABI export with this unmangled symbol name (cdylib / `--lib`).
    pub c_export_name: Option<SmolStr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: SmolStr,
    pub ty: Ty,
    pub default: Option<HirExpr>,
    pub contract: Option<HirExpr>,
    pub variadic: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirConst {
    pub def_id: DefId,
    pub name: SmolStr,
    pub ty: Ty,
    pub value: HirExpr,
    pub is_public: bool,
    /// `true` for top-level `var` declarations (mutable global).
    pub mutable: bool,
    pub span: Span,
}

/// Extern declarations from `extern "C" ... end`.
#[derive(Debug, Clone)]
pub enum HirExtern {
    Func {
        path: SmolStr,
        name: SmolStr,
        params: Vec<HirParam>,
        return_ty: Ty,
        abi: SmolStr,
        span: Span,
    },
    Var {
        path: SmolStr,
        name: SmolStr,
        ty: Ty,
        abi: SmolStr,
        span: Span,
    },
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<HirStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    /// `const name: ty = value` or `var name: ty = value`
    Let {
        name: SmolStr,
        ty: Ty,
        mutable: bool,
        value: HirExpr,
        span: Span,
    },
    /// `lvalue = value` or `lvalue += value` etc.
    Assign {
        lvalue: HirLValue,
        value: HirExpr,
        span: Span,
    },
    Return(Option<HirExpr>, Span),
    Break(Span),
    Continue(Span),
    Expr(HirExpr),
    If {
        cond: HirExpr,
        then: HirBlock,
        else_ifs: Vec<(HirExpr, HirBlock)>,
        else_: Option<HirBlock>,
        span: Span,
    },
    While {
        cond: HirExpr,
        body: HirBlock,
        span: Span,
    },
    For {
        binding: SmolStr,
        index_binding: Option<SmolStr>,
        elem_ty: Ty,
        iterable: HirExpr,
        body: HirBlock,
        span: Span,
    },
    Loop {
        body: HirBlock,
        span: Span,
    },
    Repeat {
        count: HirExpr,
        body: HirBlock,
        span: Span,
    },
    Match {
        scrutinee: HirExpr,
        arms: Vec<HirArm>,
        span: Span,
    },
    /// `if some(binding) = expr … end`
    IfSome {
        binding: SmolStr,
        inner_ty: Ty,
        value: HirExpr,
        then: HirBlock,
        else_: Option<HirBlock>,
        span: Span,
    },
    /// `while some(binding) = expr … end`
    WhileSome {
        binding: SmolStr,
        inner_ty: Ty,
        value: HirExpr,
        body: HirBlock,
        span: Span,
    },
    /// `using name: ty = expr` — resource binding (dispose called on scope exit, v1: let only)
    Using {
        name: SmolStr,
        ty: Ty,
        value: HirExpr,
        span: Span,
    },
    /// `check condition` or `check condition, "message"`
    Check {
        condition: HirExpr,
        message: Option<smol_str::SmolStr>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct HirArm {
    pub pattern: HirPattern,
    /// `case pattern if guard:` — evaluated with the pattern's bindings in
    /// scope; a false guard falls through to the next arm's test.
    pub guard: Option<HirExpr>,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirPattern {
    Wildcard,
    Binding(SmolStr, Ty),
    BoolLit(bool),
    IntLit(i64),
    StrLit(SmolStr),
    None_,
    Some_(Box<HirPattern>),
    Ok_(Box<HirPattern>),
    Err_(Box<HirPattern>),
    Variant {
        def_id: DefId,
        variant: SmolStr,
        fields: Vec<(SmolStr, HirPattern)>,
    },
    Tuple(Vec<HirPattern>),
}

#[derive(Debug, Clone)]
pub enum HirLValue {
    Var(SmolStr),
    Field {
        base: Box<HirLValue>,
        field: SmolStr,
    },
    Index {
        base: Box<HirLValue>,
        index: Box<HirExpr>,
    },
}

// ── Expressions ───────────────────────────────────────────────────────────────

/// A type-annotated expression.
#[derive(Debug, Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    // Literals
    BoolLit(bool),
    IntLit(i64),
    FloatLit(f64),
    StrLit(SmolStr),
    InterpolatedStr(Vec<HirStrPart>),
    BytesLit(Vec<u8>),
    Unit,

    // Variables and paths
    Var(SmolStr),

    // Operators
    Binary {
        op: BinaryOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<HirExpr>,
    },

    // Compound access
    Field {
        object: Box<HirExpr>,
        field: SmolStr,
    },
    Index {
        object: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    TupleIndex {
        object: Box<HirExpr>,
        index: u32,
    },

    // Calls
    Call {
        callee: Box<HirExpr>,
        args: Vec<HirArg>,
    },
    MethodCall {
        receiver: Box<HirExpr>,
        method: SmolStr,
        args: Vec<HirExpr>,
    },

    // Construction
    StructLit {
        def_id: DefId,
        fields: Vec<(SmolStr, HirExpr)>,
    },
    EnumVariant {
        def_id: DefId,
        variant: SmolStr,
        fields: Vec<(SmolStr, HirExpr)>,
    },
    ListLit {
        elem_ty: Ty,
        elements: Vec<HirExpr>,
    },
    ListSpreadLit {
        elem_ty: Ty,
        elements: Vec<HirListElement>,
    },
    TupleLit(Vec<HirExpr>),

    // Built-in wrappers
    Some_(Box<HirExpr>),
    None_,
    Ok_(Box<HirExpr>),
    Err_(Box<HirExpr>),

    // `try expr` lowered: propagate None or Err upward
    Propagate(Box<HirExpr>),

    // `await expr` desugared by the native backend through the executor.
    Await(Box<HirExpr>),

    // `if cond then a else b`
    IfExpr {
        cond: Box<HirExpr>,
        then: Box<HirExpr>,
        else_: Box<HirExpr>,
    },

    // Range `a..b`
    Range {
        start: Box<HirExpr>,
        end: Box<HirExpr>,
    },

    // Map literal `{k1: v1, k2: v2}`
    MapLit {
        key_ty: Ty,
        value_ty: Ty,
        entries: Vec<(HirExpr, HirExpr)>,
    },
    // Set literal `#{a, b, c}`
    SetLit {
        elem_ty: Ty,
        elements: Vec<HirExpr>,
    },
    // Struct update `base with { field: value } end`
    StructUpdate {
        def_id: DefId,
        base: Box<HirExpr>,
        updates: Vec<(SmolStr, HirExpr)>,
    },

    // Closure lowered to a synthetic function plus captured environment.
    Closure {
        func_name: SmolStr,
        captures: Vec<HirClosureCapture>,
    },

    // `expr is Type` type checking
    IsCheck {
        value: Box<HirExpr>,
        check_ty: Ty,
    },
}

#[derive(Debug, Clone)]
pub struct HirClosureCapture {
    pub name: SmolStr,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct HirArg {
    pub label: Option<SmolStr>,
    pub spread: bool,
    pub value: HirExpr,
}

#[derive(Debug, Clone)]
pub struct HirListElement {
    pub spread: bool,
    pub value: HirExpr,
}

#[derive(Debug, Clone)]
pub enum HirStrPart {
    Literal(SmolStr),
    Expr(HirExpr),
}
