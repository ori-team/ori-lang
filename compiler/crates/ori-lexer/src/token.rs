use logos::{Lexer, Logos};

// ── callbacks ────────────────────────────────────────────────────────────────

/// Consumes everything up to and including the closing `|--`.
/// Called after `--|` has already been consumed by logos.
fn lex_block_comment(lex: &mut Lexer<TokenKind>) -> bool {
    match lex.remainder().find("|--") {
        Some(end) => {
            lex.bump(end + 3);
            true
        }
        None => false,
    }
}

/// Consumes everything up to and including the closing `"""`.
/// Called after the opening `"""` (or `f"""`) has been consumed.
fn lex_triple_str_body(lex: &mut Lexer<TokenKind>) -> bool {
    match lex.remainder().find("\"\"\"") {
        Some(end) => {
            lex.bump(end + 3);
            true
        }
        None => false,
    }
}

// ── token kinds ──────────────────────────────────────────────────────────────

/// Every token the Ori lexer can produce.
///
/// Notes:
/// - Whitespace and line-comments are **skipped** (not emitted).
/// - Block/doc comments (`--| … |--`) are emitted as `BlockComment` so that
///   `ori doc` can process them; the main compiler can ignore them.
/// - Keywords are matched before `Ident` due to logos' token-priority rules.
/// - Primitive type names (`bool`, `int`, …) are reserved keywords.
#[derive(Logos, Debug, Clone, PartialEq, Eq, Hash)]
#[logos(skip r"[ \t\r\n]+")] // skip whitespace
pub enum TokenKind {
    // ── Comments ─────────────────────────────────────────────────────────────
    /// A line comment: `-- text`.
    /// The `--|` case is consumed by `BlockComment` first (token priority).
    #[regex(r"--[^\n]*")]
    LineComment,

    /// A block / doc comment: `--| … |--`.
    #[regex(r"--\|", lex_block_comment)]
    BlockComment,

    // ── Reserved keywords ────────────────────────────────────────────────────
    /// Canonical file header keyword (S3 surface).
    #[token("module")]
    Module,
    /// Removed in S3; kept so the parser can emit a hard error on the old form.
    #[token("namespace")]
    Namespace,
    #[token("import")]
    Import,
    #[token("as")]
    As,
    #[token("public")]
    Public,
    /// Still used for callable types `func(T) -> R`. Declaration form no longer
    /// uses this keyword (S3); the parser errors when it appears on declarations.
    #[token("func")]
    Func,
    #[token("return")]
    Return,
    #[token("end")]
    End,
    #[token("const")]
    Const,
    #[token("var")]
    Var,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    /// Canonical chained-conditional keyword (S3 surface). Replaces `else if`.
    #[token("elif")]
    Elif,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("repeat")]
    Repeat,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("match")]
    Match,
    #[token("case")]
    Case,
    #[token("loop")]
    Loop,
    #[token("struct")]
    Struct,
    #[token("trait")]
    Trait,
    /// S3 surface: `apply Type … end` with `use Trait` sections.
    #[token("apply")]
    Apply,
    /// S3 surface: `use Trait … end` inside an `apply` block.
    #[token("use")]
    Use,
    /// Removed in S3; kept so the parser can emit a hard error on the old form.
    #[token("implement")]
    Implement,
    #[token("enum")]
    Enum,
    #[token("where")]
    Where,
    #[token("is")]
    Is,
    #[token("alias")]
    Alias,
    /// `newtype Name = Repr` — a distinct type with the same representation.
    #[token("newtype")]
    Newtype,
    #[token("do")]
    Do,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("none")]
    None,
    /// Removed result ctor (`success`); emits `parse.result_ctor_renamed`.
    #[token("success")]
    SuccessRemoved,
    /// Removed result ctor (`error`); emits `parse.result_ctor_renamed`.
    #[token("error")]
    ErrorRemoved,
    // `ok` / `err` are soft: parsed as Ident; special-cased when followed by `(`.
    #[token("some")]
    Some,
    #[token("mut")]
    Mut,
    #[token("self")]
    SelfKw,
    #[token("attr")]
    Attr,
    #[token("extern")]
    Extern,
    #[token("any")]
    Any,
    #[token("optional")]
    Optional,
    #[token("result")]
    ResultKw,
    #[token("list")]
    List,
    #[token("map")]
    Map,
    #[token("set")]
    Set,
    #[token("range")]
    Range,
    #[token("void")]
    Void,

    // Keywords used by statement, expression, and type grammar.
    #[token("using")]
    Using,
    #[token("check")]
    Check,
    #[token("with")]
    With,
    #[token("then")]
    Then,
    #[token("tuple")]
    Tuple,
    #[token("lazy")]
    Lazy,
    #[token("handle")]
    Handle,

    // ── Primitive type names (also reserved) ─────────────────────────────────
    #[token("bool")]
    BoolTy,
    #[token("int")]
    IntTy,
    #[token("int8")]
    Int8Ty,
    #[token("int16")]
    Int16Ty,
    #[token("int32")]
    Int32Ty,
    #[token("int64")]
    Int64Ty,
    #[token("u8")]
    U8Ty,
    #[token("u16")]
    U16Ty,
    #[token("u32")]
    U32Ty,
    #[token("u64")]
    U64Ty,
    #[token("float")]
    FloatTy,
    #[token("float32")]
    Float32Ty,
    #[token("float64")]
    Float64Ty,
    #[token("string")]
    StringTy,
    #[token("bytes")]
    BytesTy,

    // ── Identifiers ──────────────────────────────────────────────────────────
    /// A user-defined name: Unicode letter or `_`, followed by letters,
    /// decimal digits, or `_`.
    /// Keywords above have higher priority and are matched first.
    #[regex(r"[_\p{L}][_\p{L}\p{Nd}]*")]
    Ident,

    // ── Integer literals ─────────────────────────────────────────────────────
    /// Hex: `0xFF`, `0xDEAD_u32`
    #[regex(r"0[xX][0-9a-fA-F][0-9a-fA-F_]*(i(8|16|32|64)|u(8|16|32|64))?")]
    /// Binary: `0b1010`, `0b1111_u8`
    #[regex(r"0[bB][01][01_]*(i(8|16|32|64)|u(8|16|32|64))?")]
    /// Octal: `0o755`
    #[regex(r"0[oO][0-7][0-7_]*(i(8|16|32|64)|u(8|16|32|64))?")]
    /// Decimal: `42`, `1_000_000`, `42u8`, `100i32`
    #[regex(r"[0-9][0-9_]*(i(8|16|32|64)|u(8|16|32|64))?")]
    IntLit,

    // ── Float literals ───────────────────────────────────────────────────────
    /// `3.14`, `1.0e-5`, `6.022e23f32`
    /// Must have digits on both sides of `.` to avoid conflict with `..`.
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+\-]?[0-9][0-9_]*)?(f(32|64))?")]
    FloatLit,

    // ── String literals ──────────────────────────────────────────────────────
    /// Triple-quoted f-string: `f"""…"""`
    #[regex(r#"f""""#, lex_triple_str_body)]
    TripleFStrLit,

    /// Triple-quoted string: `"""…"""`
    #[regex(r#"""""#, lex_triple_str_body)]
    TripleStrLit,

    /// Interpolated string: `f"hello {name}"`
    /// Interpolation expressions are NOT parsed at the lexer level;
    /// the parser handles `{…}` inside f-string tokens.
    #[regex(r#"f"([^"\\]|\\.)*""#)]
    FStrLit,

    /// Plain string: `"hello\n"`
    #[regex(r#""([^"\\]|\\.)*""#)]
    StrLit,

    /// Byte string: `b"\xFF\x00"`
    #[regex(r#"b"([^"\\]|\\.)*""#)]
    BytesLit,

    // ── Operators (longer tokens declared first for priority) ────────────────
    #[token("-->")]
    Uninhabited, // intentionally unused; prevents `->` + `-` ambiguity notes
    #[token("...")]
    Ellipsis,
    #[token("..")]
    DotDot,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("|>")]
    Pipe,
    #[token("==")]
    EqEq,
    #[token("!=")]
    BangEq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token(".")]
    Dot,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("=")]
    Eq,
    #[token("?")]
    Question,

    // ── Delimiters ───────────────────────────────────────────────────────────
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("@")]
    At,
}

impl TokenKind {
    /// Returns `true` for trivia tokens that most passes can ignore.
    pub fn is_trivia(&self) -> bool {
        matches!(self, TokenKind::LineComment | TokenKind::BlockComment)
    }

    /// Human-readable name used in diagnostics.
    pub fn display_name(&self) -> &'static str {
        match self {
            TokenKind::Ident => "identifier",
            TokenKind::IntLit => "integer literal",
            TokenKind::FloatLit => "float literal",
            TokenKind::StrLit
            | TokenKind::FStrLit
            | TokenKind::TripleStrLit
            | TokenKind::TripleFStrLit => "string literal",
            TokenKind::BytesLit => "bytes literal",
            TokenKind::Module => "`module`",
            TokenKind::Namespace => "`namespace`",
            TokenKind::Import => "`import`",
            TokenKind::As => "`as`",
            TokenKind::Public => "`public`",
            TokenKind::Func => "`func`",
            TokenKind::Return => "`return`",
            TokenKind::End => "`end`",
            TokenKind::Const => "`const`",
            TokenKind::Var => "`var`",
            TokenKind::If => "`if`",
            TokenKind::Else => "`else`",
            TokenKind::Elif => "`elif`",
            TokenKind::While => "`while`",
            TokenKind::For => "`for`",
            TokenKind::In => "`in`",
            TokenKind::Repeat => "`repeat`",
            TokenKind::Break => "`break`",
            TokenKind::Continue => "`continue`",
            TokenKind::Match => "`match`",
            TokenKind::Case => "`case`",
            TokenKind::Loop => "`loop`",
            TokenKind::Struct => "`struct`",
            TokenKind::Trait => "`trait`",
            TokenKind::Apply => "`apply`",
            TokenKind::Use => "`use`",
            TokenKind::Implement => "`implement`",
            TokenKind::Enum => "`enum`",
            TokenKind::Where => "`where`",
            TokenKind::Is => "`is`",
            TokenKind::Alias => "`alias`",
            TokenKind::Newtype => "`newtype`",
            TokenKind::Do => "`do`",
            TokenKind::And => "`and`",
            TokenKind::Or => "`or`",
            TokenKind::Not => "`not`",
            TokenKind::True => "`true`",
            TokenKind::False => "`false`",
            TokenKind::None => "`none`",
            TokenKind::SuccessRemoved => "`success`",
            TokenKind::ErrorRemoved => "`error`",
            TokenKind::Some => "`some`",
            TokenKind::Mut => "`mut`",
            TokenKind::SelfKw => "`self`",
            TokenKind::Attr => "`attr`",
            TokenKind::Extern => "`extern`",
            TokenKind::Any => "`any`",
            TokenKind::Optional => "`optional`",
            TokenKind::ResultKw => "`result`",
            TokenKind::List => "`list`",
            TokenKind::Map => "`map`",
            TokenKind::Set => "`set`",
            TokenKind::Range => "`range`",
            TokenKind::Void => "`void`",
            TokenKind::Using => "`using`",
            TokenKind::Check => "`check`",
            TokenKind::With => "`with`",
            TokenKind::Then => "`then`",
            TokenKind::Tuple => "`tuple`",
            TokenKind::Lazy => "`lazy`",
            TokenKind::Handle => "`handle`",
            TokenKind::BoolTy => "`bool`",
            TokenKind::IntTy => "`int`",
            TokenKind::Int8Ty => "`int8`",
            TokenKind::Int16Ty => "`int16`",
            TokenKind::Int32Ty => "`int32`",
            TokenKind::Int64Ty => "`int64`",
            TokenKind::U8Ty => "`u8`",
            TokenKind::U16Ty => "`u16`",
            TokenKind::U32Ty => "`u32`",
            TokenKind::U64Ty => "`u64`",
            TokenKind::FloatTy => "`float`",
            TokenKind::Float32Ty => "`float32`",
            TokenKind::Float64Ty => "`float64`",
            TokenKind::StringTy => "`string`",
            TokenKind::BytesTy => "`bytes`",
            TokenKind::Ellipsis => "`...`",
            TokenKind::DotDot => "`..`",
            TokenKind::Arrow => "`->`",
            TokenKind::FatArrow => "`=>`",
            TokenKind::Pipe => "`|>`",
            TokenKind::EqEq => "`==`",
            TokenKind::BangEq => "`!=`",
            TokenKind::LtEq => "`<=`",
            TokenKind::GtEq => "`>=`",
            TokenKind::PlusEq => "`+=`",
            TokenKind::MinusEq => "`-=`",
            TokenKind::StarEq => "`*=`",
            TokenKind::SlashEq => "`/=`",
            TokenKind::Dot => "`.`",
            TokenKind::Plus => "`+`",
            TokenKind::Minus => "`-`",
            TokenKind::Star => "`*`",
            TokenKind::Slash => "`/`",
            TokenKind::Percent => "`%`",
            TokenKind::Lt => "`<`",
            TokenKind::Gt => "`>`",
            TokenKind::Eq => "`=`",
            TokenKind::Question => "`?`",
            TokenKind::LParen => "`(`",
            TokenKind::RParen => "`)`",
            TokenKind::LBracket => "`[`",
            TokenKind::RBracket => "`]`",
            TokenKind::LBrace => "`{`",
            TokenKind::RBrace => "`}`",
            TokenKind::Colon => "`:`",
            TokenKind::Comma => "`,`",
            TokenKind::At => "`@`",
            TokenKind::LineComment => "line comment",
            TokenKind::BlockComment => "block comment",
            TokenKind::Uninhabited => unreachable!(),
        }
    }
}
