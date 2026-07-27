use crate::parser::Parser;
use ori_ast::expr::{
    Arg, ArgValue, BinaryOp, ClosureBody, ClosureExpr, ClosureParam, Expr, FStrPart, FieldInit,
    IndexExpr, MatchExprArm, UnaryOp,
};
use ori_diagnostics::{DiagnosticSink, Span};
use ori_lexer::{lex, TokenKind};
use smol_str::SmolStr;

struct NormalizedTripleString {
    text: String,
    offsets: Vec<usize>,
}

// ── Pratt precedence ──────────────────────────────────────────────────────────

fn infix_prec(kind: &TokenKind) -> Option<(u8, u8)> {
    // Returns (left_prec, right_prec). right > left → right-associative.
    match kind {
        TokenKind::Pipe => Some((1, 2)), // |>  left-assoc
        TokenKind::Or => Some((3, 4)),   // or
        TokenKind::And => Some((5, 6)),  // and
        TokenKind::EqEq
        | TokenKind::BangEq
        | TokenKind::Lt
        | TokenKind::LtEq
        | TokenKind::Gt
        | TokenKind::GtEq => Some((7, 8)), // comparisons
        TokenKind::Plus | TokenKind::Minus => Some((9, 10)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some((11, 12)),
        _ => None,
    }
}

fn token_to_binop(kind: &TokenKind) -> Option<BinaryOp> {
    match kind {
        TokenKind::Plus => Some(BinaryOp::Add),
        TokenKind::Minus => Some(BinaryOp::Sub),
        TokenKind::Star => Some(BinaryOp::Mul),
        TokenKind::Slash => Some(BinaryOp::Div),
        TokenKind::Percent => Some(BinaryOp::Rem),
        TokenKind::EqEq => Some(BinaryOp::Eq),
        TokenKind::BangEq => Some(BinaryOp::Ne),
        TokenKind::Lt => Some(BinaryOp::Lt),
        TokenKind::LtEq => Some(BinaryOp::Le),
        TokenKind::Gt => Some(BinaryOp::Gt),
        TokenKind::GtEq => Some(BinaryOp::Ge),
        TokenKind::And => Some(BinaryOp::And),
        TokenKind::Or => Some(BinaryOp::Or),
        _ => None,
    }
}

// ── Public entry ──────────────────────────────────────────────────────────────

fn is_comparison_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    )
}

fn is_comparison_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Binary { op, .. } if is_comparison_op(*op))
}

fn starts_numeric_suffix(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_alphabetic())
}

impl<'src> Parser<'src> {
    /// Parse an expression (top-level precedence).
    pub fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_expr_prec(0)
    }

    fn parse_expr_prec(&mut self, min_prec: u8) -> Option<Expr> {
        let mut lhs = self.parse_unary()?;

        loop {
            // `is` type check — treated as a postfix binary at prec 7
            if self.at(&TokenKind::Is) && min_prec <= 7 {
                self.advance();
                let ty = self.parse_qualified_name()?;
                let span = lhs.span().cover(ty.span);
                lhs = Expr::IsCheck {
                    value: Box::new(lhs),
                    ty,
                    span,
                };
                continue;
            }

            // `|>` pipe — special: rhs is the function, result is Call
            if self.at(&TokenKind::Pipe) && min_prec <= 1 {
                self.advance();
                let func = self.parse_expr_prec(2)?;
                let span = lhs.span().cover(func.span());
                lhs = Expr::Pipe {
                    value: Box::new(lhs),
                    func: Box::new(func),
                    span,
                };
                continue;
            }

            if self.at(&TokenKind::DotDot) && min_prec == 0 {
                self.advance();
                let rhs = self.parse_expr_prec(0)?;
                let span = lhs.span().cover(rhs.span());
                lhs = Expr::Range {
                    start: Box::new(lhs),
                    end: Box::new(rhs),
                    span,
                };
                continue;
            }

            // `base with { field: value } end` struct update.
            if self.at(&TokenKind::With) && min_prec <= 1 {
                self.advance();
                if !self.at(&TokenKind::LBrace) {
                    self.error(
                        "parse.unexpected_token",
                        "struct update expects `{` after `with`; write `base with { field: value } end`",
                        self.current_span(),
                    );
                    return None;
                }
                let block_start = lhs.span();
                let updates = self.parse_braced_field_inits()?;
                // Labeled form: `end struct` (same label as struct declarations).
                let end = self.expect_block_end(block_start, "struct")?;
                let span = lhs.span().cover(end);
                lhs = Expr::StructUpdate {
                    base: Box::new(lhs),
                    updates,
                    span,
                };
                continue;
            }

            // Standard binary operators
            if let Some(&(left_prec, right_prec)) = self.peek_kind().and_then(infix_prec).as_ref() {
                if left_prec < min_prec {
                    break;
                }
                let op_tok = self.advance().unwrap();
                let op = token_to_binop(&op_tok.kind).unwrap();
                if is_comparison_op(op) && is_comparison_expr(&lhs) {
                    self.error(
                        "parse.chained_comparison",
                        "chained comparison is not allowed",
                        lhs.span().cover(op_tok.span),
                    );
                }
                let rhs = self.parse_expr_prec(right_prec)?;
                let span = lhs.span().cover(rhs.span());
                lhs = Expr::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    span,
                };
                continue;
            }

            break;
        }
        Some(lhs)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let span = self.current_span();
        if self.eat(&TokenKind::Minus) {
            let operand = self.parse_unary()?;
            let s = span.cover(operand.span());
            return Some(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
                span: s,
            });
        }
        if self.eat(&TokenKind::Not) {
            let operand = self.parse_unary()?;
            let s = span.cover(operand.span());
            return Some(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                span: s,
            });
        }
        if self.eat_contextual("await") {
            let inner = self.parse_unary()?;
            let s = span.cover(inner.span());
            return Some(Expr::Await {
                expr: Box::new(inner),
                span: s,
            });
        }
        if self.eat_contextual("try") {
            let inner = self.parse_unary()?;
            let s = span.cover(inner.span());
            return Some(Expr::Try {
                expr: Box::new(inner),
                span: s,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary_expr()?;
        loop {
            let span_start = expr.span();
            match self.peek_kind() {
                // `.field` or `.integer` (tuple index)
                Some(TokenKind::Dot) => {
                    self.advance();
                    match self.peek_kind() {
                        Some(TokenKind::IntLit) => {
                            let tok = self.advance().unwrap();
                            let idx: u32 = self.slice(tok.span).parse().unwrap_or(0);
                            let span = span_start.cover(tok.span);
                            expr = Expr::TupleIndex {
                                object: Box::new(expr),
                                index: idx,
                                span,
                            };
                        }
                        _ => {
                            let field = self.parse_member_name()?;
                            let span = span_start.cover(field.span);
                            expr = Expr::Field {
                                object: Box::new(expr),
                                field,
                                span,
                            };
                        }
                    }
                }
                // `(args)` — call
                Some(TokenKind::LParen) => {
                    let (args, end_span) = self.parse_call_args()?;
                    let span = span_start.cover(end_span);
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span,
                    };
                }
                // `Type { field: value }` — explicit struct literal (S3)
                Some(TokenKind::LBrace) => {
                    let Expr::QualifiedIdent(ty) = &expr else {
                        break;
                    };
                    let ty = ty.clone();
                    let (fields, end) = self.parse_braced_field_inits_with_end()?;
                    let span = span_start.cover(end);
                    expr = Expr::StructLit { ty, fields, span };
                }
                // `[index]`
                Some(TokenKind::LBracket) => {
                    self.advance();
                    let index = self.parse_index_expr()?;
                    let end = self.expect(&TokenKind::RBracket)?;
                    let span = span_start.cover(end);
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index,
                        span,
                    };
                }
                // S3: postfix `?` propagation removed — only `try expr` remains.
                Some(TokenKind::Question) => {
                    let q_span = self.advance().unwrap().span;
                    self.error(
                        "parse.question_propagate_removed",
                        "postfix `?` was removed; use `try expr` for error/optional propagation",
                        q_span,
                    );
                    // Leave the left-hand expression unwrapped so recovery does not
                    // silently restore try semantics.
                }
                _ => break,
            }
        }
        // Poetic call: `callee arg` on the same line (one argument, no nesting).
        // Only apply when the left-hand side looks callable — never for literals
        // (so `repeat 3 times` keeps the contextual `times` keyword).
        if self.allow_poetic
            && is_poetic_callee(&expr)
            && self.same_line_as_span_end(expr.span())
            && self.can_start_poetic_arg()
        {
            expr = self.parse_poetic_call(expr)?;
        }
        Some(expr)
    }

    /// Parse a single poetic argument and wrap as `Call`. Nested poetic is an error.
    fn parse_poetic_call(&mut self, callee: Expr) -> Option<Expr> {
        let span_start = callee.span();
        let prev = self.allow_poetic;
        self.allow_poetic = false;
        let arg = self.parse_expr();
        self.allow_poetic = prev;
        let arg = arg?;
        // Leftover same-line expression start ⇒ nested poetic (`print greet name`).
        if self.same_line_as_span_end(arg.span()) && self.can_start_poetic_arg() {
            let bad = self.current_span();
            self.error(
                "parse.poetic_call_nested",
                "nested poetic call is not allowed; use parentheses for the inner call",
                bad,
            );
            // Recovery: consume one more expression so later tokens stay aligned.
            let _ = {
                let prev = self.allow_poetic;
                self.allow_poetic = false;
                let e = self.parse_expr();
                self.allow_poetic = prev;
                e
            };
        }
        let arg_span = arg.span();
        Some(Expr::Call {
            callee: Box::new(callee),
            args: vec![Arg {
                label: None,
                value: ArgValue::Expr(Box::new(arg)),
                span: arg_span,
            }],
            span: span_start.cover(arg_span),
        })
    }

    fn can_start_poetic_arg(&self) -> bool {
        // Note: deliberately exclude `Minus` — it is also binary subtraction
        // (`width - s_len` must not become a poetic call). Use `f(-1)` / `f (-1)`
        // when the poetic argument is a negated literal/expression.
        matches!(
            self.peek_kind(),
            Some(
                TokenKind::Ident
                    | TokenKind::Lazy
                    | TokenKind::IntLit
                    | TokenKind::FloatLit
                    | TokenKind::StrLit
                    | TokenKind::TripleStrLit
                    | TokenKind::FStrLit
                    | TokenKind::TripleFStrLit
                    | TokenKind::BytesLit
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::None
                    | TokenKind::Some
                    | TokenKind::SuccessRemoved
                    | TokenKind::ErrorRemoved
                    | TokenKind::SelfKw
                    | TokenKind::LParen
                    | TokenKind::LBracket
                    | TokenKind::LBrace
                    | TokenKind::Not
                    | TokenKind::If
                    | TokenKind::Do
                    | TokenKind::Set
                    | TokenKind::Tuple
                    | TokenKind::Dot
                    | TokenKind::StringTy
                    | TokenKind::IntTy
                    | TokenKind::Int8Ty
                    | TokenKind::Int16Ty
                    | TokenKind::Int32Ty
                    | TokenKind::Int64Ty
                    | TokenKind::U8Ty
                    | TokenKind::U16Ty
                    | TokenKind::U32Ty
                    | TokenKind::U64Ty
                    | TokenKind::FloatTy
                    | TokenKind::Float32Ty
                    | TokenKind::Float64Ty
                    | TokenKind::BoolTy
                    | TokenKind::BytesTy
            )
        )
    }

    fn numeric_literal_raw_with_adjacent_suffix(&mut self, initial_span: Span) -> (SmolStr, Span) {
        let mut span = initial_span;
        if let Some(next) = self.peek() {
            if next.span.start == span.end && starts_numeric_suffix(self.slice(next.span)) {
                span = span.cover(next.span);
                self.advance();
            }
        }
        (SmolStr::new(self.slice(span)), span)
    }

    /// `match scr case p [if g]: expr … case else: expr end` — expression form.
    ///
    /// Mirrors `parse_match_stmt`, except each arm's body is a single
    /// expression instead of a statement list. `start` is the `match` token's
    /// span (already read by the caller's lookahead, not yet consumed).
    fn parse_match_expr(&mut self, start: Span) -> Option<Expr> {
        self.advance(); // match
        let scrutinee = self.parse_expr()?;
        let mut arms = Vec::new();
        while self.at(&TokenKind::Case) {
            let case_span = self.advance().unwrap().span; // case
            if self.at(&TokenKind::Else) {
                self.advance(); // else
                self.expect(&TokenKind::Colon)?;
                let body = self.parse_expr()?;
                let end = body.span();
                arms.push(MatchExprArm {
                    pattern: None,
                    guard: None,
                    body: Box::new(body),
                    span: case_span.cover(end),
                });
                break; // else must be last
            }
            let pattern = self.parse_pattern()?;
            let guard = if self.eat(&TokenKind::If) {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            self.expect(&TokenKind::Colon)?;
            let body = self.parse_expr()?;
            let end = body.span();
            arms.push(MatchExprArm {
                pattern: Some(pattern),
                guard,
                body: Box::new(body),
                span: case_span.cover(end),
            });
        }
        let end = self.expect_block_end(start, "match")?;
        Some(Expr::MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            span: start.cover(end),
        })
    }

    pub fn parse_primary_expr(&mut self) -> Option<Expr> {
        let span = self.current_span();
        match self.peek_kind()? {
            // Literals
            TokenKind::True => {
                self.advance();
                Some(Expr::BoolLit(true, span))
            }
            TokenKind::False => {
                self.advance();
                Some(Expr::BoolLit(false, span))
            }
            TokenKind::None => {
                self.advance();
                Some(Expr::None(span))
            }

            // Builtin wrapper: some(x)
            TokenKind::Some => {
                let tok = self.advance().unwrap();
                let name = ori_ast::common::Name::new(
                    smol_str::SmolStr::new(self.slice(tok.span)),
                    tok.span,
                );
                Some(Expr::QualifiedIdent(
                    ori_ast::common::QualifiedName::single(name),
                ))
            }

            // Removed: success(...) / error(...) — recover as ok/err names after error
            TokenKind::SuccessRemoved | TokenKind::ErrorRemoved => {
                let tok = self.advance().unwrap();
                let old = self.slice(tok.span);
                let new = if old == "success" { "ok" } else { "err" };
                self.error(
                    "parse.result_ctor_renamed",
                    format!("`{old}` was renamed to `{new}`; write `{new}(...)`"),
                    tok.span,
                );
                let name = ori_ast::common::Name::new(smol_str::SmolStr::new(new), tok.span);
                Some(Expr::QualifiedIdent(
                    ori_ast::common::QualifiedName::single(name),
                ))
            }

            TokenKind::IntLit => {
                let tok = self.advance().unwrap();
                let (raw, span) = self.numeric_literal_raw_with_adjacent_suffix(tok.span);
                Some(Expr::IntLit { raw, span })
            }
            TokenKind::FloatLit => {
                let tok = self.advance().unwrap();
                let (raw, span) = self.numeric_literal_raw_with_adjacent_suffix(tok.span);
                Some(Expr::FloatLit { raw, span })
            }
            TokenKind::StrLit => {
                let tok = self.advance().unwrap();
                let raw = self.slice(tok.span).to_string();
                let value = self
                    .unescape_string_content(&raw[1..raw.len() - 1], tok.span.start as usize + 1);
                Some(Expr::StrLit { value, span })
            }
            TokenKind::TripleStrLit => {
                let tok = self.advance().unwrap();
                let raw = self.slice(tok.span).to_string();
                let content = self.normalize_triple_string_content(&raw[3..raw.len() - 3]);
                let value = self.unescape_string_content(&content, tok.span.start as usize + 3);
                Some(Expr::StrLit { value, span })
            }
            TokenKind::FStrLit => {
                let tok = self.advance().unwrap();
                let raw = self.slice(tok.span).to_string();
                let content = &raw[2..raw.len() - 1]; // strip f" and "
                Some(Expr::FStrLit {
                    parts: self.parse_fstr_parts(content, tok.span.start as usize + 2, None),
                    span,
                })
            }
            TokenKind::TripleFStrLit => {
                let tok = self.advance().unwrap();
                let raw = self.slice(tok.span).to_string();
                let content =
                    self.normalize_triple_string_content_with_offsets(&raw[4..raw.len() - 3]);
                Some(Expr::FStrLit {
                    parts: self.parse_fstr_parts(
                        &content.text,
                        tok.span.start as usize + 4,
                        Some(&content.offsets),
                    ),
                    span,
                })
            }
            TokenKind::BytesLit => {
                let tok = self.advance().unwrap();
                let raw = self.slice(tok.span).to_string();
                let content = self
                    .unescape_bytes_content(&raw[2..raw.len() - 1], tok.span.start as usize + 2);
                Some(Expr::BytesLit {
                    bytes: content,
                    span,
                })
            }

            // `self`
            TokenKind::SelfKw => {
                self.advance();
                Some(Expr::SelfExpr(span))
            }

            // `tuple(a, b)` - explicit tuple constructor form from the grammar.
            TokenKind::Tuple => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let mut elements = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    elements.push(self.parse_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RParen)?;
                if elements.len() < 2 {
                    self.error(
                        "parse.tuple_arity",
                        "`tuple(...)` requires at least two elements",
                        span.cover(end),
                    );
                }
                Some(Expr::Tuple {
                    elements,
                    span: span.cover(end),
                })
            }

            // Closure `(params) => expr` / `(params) … end`, grouped/tuple, or removed guided struct `(field: v)`.
            TokenKind::LParen => {
                if self.looks_like_closure() {
                    return self.parse_closure_expr(span);
                }
                self.advance();
                if self.at(&TokenKind::RParen) {
                    // `()` — empty tuple
                    let end = self.advance().unwrap().span;
                    return Some(Expr::Tuple {
                        elements: Vec::new(),
                        span: span.cover(end),
                    });
                }
                // Removed guided struct: `(field: value, …)` — recover as anon struct lit.
                if self.at_struct_field_label() {
                    self.error(
                        "parse.removed_struct_call_literal",
                        "guided struct construction `(field: value)` is removed; write `{ field: value }` or `Type { field: value }`",
                        span,
                    );
                    let (fields, end) = self.parse_paren_field_inits_rest()?;
                    return Some(Expr::AnonStructLit {
                        fields,
                        span: span.cover(end),
                    });
                }
                let first = self.parse_expr()?;
                if self.eat(&TokenKind::Comma) {
                    let mut elements = vec![first];
                    while !self.at(&TokenKind::RParen) && !self.at_eof() {
                        elements.push(self.parse_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.expect(&TokenKind::RParen)?;
                    Some(Expr::Tuple {
                        elements,
                        span: span.cover(end),
                    })
                } else {
                    self.expect(&TokenKind::RParen)?;
                    Some(first) // just a grouped expr
                }
            }

            // List literal `[a, b, c]`
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                while !self.at(&TokenKind::RBracket) && !self.at_eof() {
                    elements.push(self.parse_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RBracket)?;
                Some(Expr::List {
                    elements,
                    span: span.cover(end),
                })
            }

            // Brace literal: struct `{ field: v }` vs map `{ "k": v }` / `{ 1: v }`
            TokenKind::LBrace => self.parse_brace_literal(span),

            // Set literal `set { a, b, c }`
            TokenKind::Set if self.peek_nth_kind(1) == Some(&TokenKind::LBrace) => {
                self.advance();
                self.expect(&TokenKind::LBrace)?;
                let mut elements = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                    elements.push(self.parse_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RBrace)?;
                Some(Expr::Set {
                    elements,
                    span: span.cover(end),
                })
            }

            // S3: `do(...)` closures removed — canonical form is `(params) => …`.
            TokenKind::Do => {
                let do_span = self.advance().unwrap().span;
                self.error(
                    "parse.do_removed",
                    "`do` closures were removed; write `(params) => expr` or `(params) … end`",
                    do_span,
                );
                // Recovery: still parse the parameter list/body so later code typechecks.
                self.parse_closure_expr(span)
            }

            // `.Variant` — shorthand enum variant; `.{…}` is removed (S3)
            TokenKind::Dot => {
                self.advance();
                if self.at(&TokenKind::LBrace) {
                    self.error(
                        "parse.removed_struct_call_literal",
                        "`.{…}` anonymous struct literal is removed; write `{ field: value }` or `Type { field: value }`",
                        span,
                    );
                    // Recover as anonymous struct so the rest of the file still typechecks.
                    let (fields, end) = self.parse_braced_field_inits_with_end()?;
                    return Some(Expr::AnonStructLit {
                        fields,
                        span: span.cover(end),
                    });
                }
                let variant = self.parse_name()?;
                if self.at(&TokenKind::LParen) {
                    let (fields, end) = self.parse_field_inits_with_end()?;
                    Some(Expr::EnumVariantNamed {
                        ty: None,
                        variant,
                        fields,
                        span: span.cover(end),
                    })
                } else {
                    let end = variant.span;
                    Some(Expr::EnumVariantUnit {
                        ty: None,
                        variant,
                        span: span.cover(end),
                    })
                }
            }

            // `if cond then a else b` — inline if expression
            TokenKind::If => {
                self.advance();
                let condition = self.parse_expr()?;
                self.expect(&TokenKind::Then)?;
                let then_expr = self.parse_expr()?;
                if !self.at(&TokenKind::Else) {
                    self.error(
                        "parse.missing_else_in_if_expr",
                        "inline `if` expressions require an `else` branch",
                        self.current_span(),
                    );
                    return None;
                }
                self.advance(); // else
                let else_expr = self.parse_expr()?;
                let end = else_expr.span();
                Some(Expr::IfExpr {
                    condition: Box::new(condition),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                    span: span.cover(end),
                })
            }

            // `match scr case p: expr … end` — match in expression position.
            // Statement position never reaches here: parse_stmt dispatches
            // `match` to parse_match_stmt before trying an expression.
            TokenKind::Match => self.parse_match_expr(span),

            // Identifier or qualified name — also handles `Name(fields)` struct/enum lit
            TokenKind::Ident | TokenKind::Lazy => {
                let name = self.parse_qualified_name()?;
                Some(Expr::QualifiedIdent(name))
            }

            // Primitive type keywords used as conversion functions: string(x), int(x), etc.
            TokenKind::StringTy
            | TokenKind::IntTy
            | TokenKind::Int8Ty
            | TokenKind::Int16Ty
            | TokenKind::Int32Ty
            | TokenKind::Int64Ty
            | TokenKind::U8Ty
            | TokenKind::U16Ty
            | TokenKind::U32Ty
            | TokenKind::U64Ty
            | TokenKind::FloatTy
            | TokenKind::Float32Ty
            | TokenKind::Float64Ty
            | TokenKind::BytesTy
            | TokenKind::BoolTy => {
                let tok = self.advance().unwrap();
                let name = ori_ast::common::Name::new(
                    smol_str::SmolStr::new(self.slice(tok.span)),
                    tok.span,
                );
                Some(Expr::QualifiedIdent(
                    ori_ast::common::QualifiedName::single(name),
                ))
            }

            _ => {
                let span = self.current_span();
                self.error("parse.expected_expression", "expected an expression", span);
                None
            }
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Parse `(args)` including the closing `)`. Returns args and the `)` span
    /// (must not use the following token — poetic call relies on accurate end spans).
    fn parse_call_args(&mut self) -> Option<(Vec<Arg>, ori_diagnostics::Span)> {
        self.expect(&TokenKind::LParen)?;
        let mut args = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let arg_span = self.current_span();
            // Named arg: `name: expr`
            if self.at(&TokenKind::Ident) && self.peek_nth_kind(1) == Some(&TokenKind::Colon) {
                let label = self.parse_name()?;
                self.expect(&TokenKind::Colon)?;
                let (value, span) = self.parse_call_arg_value(arg_span)?;
                args.push(Arg {
                    label: Some(label),
                    value,
                    span,
                });
            } else {
                let (value, span) = self.parse_call_arg_value(arg_span)?;
                args.push(Arg {
                    label: None,
                    value,
                    span,
                });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RParen)?;
        Some((args, end))
    }

    fn parse_call_arg_value(
        &mut self,
        start: ori_diagnostics::Span,
    ) -> Option<(ArgValue, ori_diagnostics::Span)> {
        if self.eat(&TokenKind::DotDot) {
            let value = self.parse_expr()?;
            let span = start.cover(value.span());
            return Some((ArgValue::Spread(Box::new(value)), span));
        }
        let value = self.parse_expr()?;
        let span = start.cover(value.span());
        Some((ArgValue::Expr(Box::new(value)), span))
    }

    fn parse_index_expr(&mut self) -> Option<IndexExpr> {
        // `a..b`, `a..`, `..b`, `..` — range index
        if self.at(&TokenKind::DotDot) {
            self.advance();
            let end = if !self.at(&TokenKind::RBracket) {
                Some(Box::new(self.parse_expr_prec(1)?))
            } else {
                None
            };
            return Some(IndexExpr::Range { start: None, end });
        }
        let expr = self.parse_expr_prec(1)?;
        if self.eat(&TokenKind::DotDot) {
            let end = if !self.at(&TokenKind::RBracket) {
                Some(Box::new(self.parse_expr_prec(1)?))
            } else {
                None
            };
            Some(IndexExpr::Range {
                start: Some(Box::new(expr)),
                end,
            })
        } else {
            Some(IndexExpr::Single(Box::new(expr)))
        }
    }

    fn parse_field_inits_with_end(&mut self) -> Option<(Vec<FieldInit>, Span)> {
        self.expect(&TokenKind::LParen)?;
        self.parse_paren_field_inits_rest()
    }

    fn parse_braced_field_inits_with_end(&mut self) -> Option<(Vec<FieldInit>, Span)> {
        self.expect(&TokenKind::LBrace)?;
        self.parse_braced_field_inits_rest()
    }

    /// Used by struct update `with { … } end`.
    fn parse_braced_field_inits(&mut self) -> Option<Vec<FieldInit>> {
        let (fields, _) = self.parse_braced_field_inits_with_end()?;
        Some(fields)
    }

    /// `{ field: v }` vs `{ "k": v }` / `{ 1: v }` / `{}`.
    ///
    /// Disambiguation (S3): identifier before `:` → struct field; any other
    /// key expression (literals, calls, …) → map entry.
    fn parse_brace_literal(&mut self, span: Span) -> Option<Expr> {
        self.advance(); // `{`
        if self.at(&TokenKind::RBrace) {
            let end = self.advance().unwrap().span;
            return Some(Expr::Map {
                entries: Vec::new(),
                span: span.cover(end),
            });
        }
        if self.at_struct_field_label() {
            let (fields, end) = self.parse_braced_field_inits_rest()?;
            return Some(Expr::AnonStructLit {
                fields,
                span: span.cover(end),
            });
        }
        let mut entries = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let key = self.parse_expr()?;
            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            entries.push((key, value));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RBrace)?;
        Some(Expr::Map {
            entries,
            span: span.cover(end),
        })
    }

    /// `true` when the next tokens are `ident :` (struct field label).
    fn at_struct_field_label(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Ident | TokenKind::Lazy))
            && self.peek_nth_kind(1) == Some(&TokenKind::Colon)
    }

    /// Parse `field: expr, …` until `)`, opening `(` already consumed.
    fn parse_paren_field_inits_rest(&mut self) -> Option<(Vec<FieldInit>, Span)> {
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let name = self.parse_name()?;
            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            let span = name.span.cover(value.span());
            fields.push(FieldInit {
                name,
                value: Box::new(value),
                span,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RParen)?;
        Some((fields, end))
    }

    /// Parse `field: expr, …` until `}`, opening `{` already consumed.
    fn parse_braced_field_inits_rest(&mut self) -> Option<(Vec<FieldInit>, Span)> {
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let name = self.parse_name()?;
            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr()?;
            let span = name.span.cover(value.span());
            fields.push(FieldInit {
                name,
                value: Box::new(value),
                span,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RBrace)?;
        Some((fields, end))
    }

    fn unescape_string_content(&mut self, content: &str, base: usize) -> SmolStr {
        let mut out = String::new();
        let mut iter = content.char_indices().peekable();
        while let Some((i, ch)) = iter.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }

            let Some((j, esc)) = iter.next() else {
                self.error(
                    "parse.invalid_escape",
                    "unfinished escape sequence",
                    Span::new(base + i, base + i + 1),
                );
                break;
            };

            match esc {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '0' => out.push('\0'),
                'u' => {
                    if iter.peek().map(|(_, c)| *c) != Some('{') {
                        self.error(
                            "parse.invalid_escape",
                            "expected `{` after `\\u`",
                            Span::new(base + i, base + j + esc.len_utf8()),
                        );
                        out.push('u');
                        continue;
                    }
                    iter.next(); // {
                    let mut hex = String::new();
                    let mut end = None;
                    for (k, c) in iter.by_ref() {
                        if c == '}' {
                            end = Some(k + 1);
                            break;
                        }
                        hex.push(c);
                    }
                    match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        Some(c) => out.push(c),
                        None => self.error(
                            "parse.invalid_escape",
                            "invalid unicode escape",
                            Span::new(base + i, base + end.unwrap_or(content.len())),
                        ),
                    }
                }
                other => {
                    self.error(
                        "parse.invalid_escape",
                        format!("unknown escape sequence `\\{other}`"),
                        Span::new(base + i, base + j + other.len_utf8()),
                    );
                    out.push(other);
                }
            }
        }
        SmolStr::new(out)
    }

    fn unescape_bytes_content(&mut self, content: &str, base: usize) -> Vec<u8> {
        let mut out = Vec::new();
        let mut iter = content.char_indices().peekable();
        while let Some((i, ch)) = iter.next() {
            if ch != '\\' {
                let mut buf = [0; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                continue;
            }

            let Some((j, esc)) = iter.next() else {
                self.error(
                    "parse.invalid_escape",
                    "unfinished byte escape sequence",
                    Span::new(base + i, base + i + 1),
                );
                break;
            };

            match esc {
                'n' => out.push(b'\n'),
                'r' => out.push(b'\r'),
                't' => out.push(b'\t'),
                '"' => out.push(b'"'),
                '\\' => out.push(b'\\'),
                '0' => out.push(0),
                'x' => {
                    let Some((h1_i, h1)) = iter.next() else {
                        self.error(
                            "parse.invalid_escape",
                            "expected two hex digits after `\\x`",
                            Span::new(base + i, base + j + 1),
                        );
                        continue;
                    };
                    let Some((h2_i, h2)) = iter.next() else {
                        self.error(
                            "parse.invalid_escape",
                            "expected two hex digits after `\\x`",
                            Span::new(base + i, base + h1_i + h1.len_utf8()),
                        );
                        continue;
                    };
                    if let (Some(a), Some(b)) = (hex_value(h1), hex_value(h2)) {
                        out.push((a << 4) | b);
                    } else {
                        self.error(
                            "parse.invalid_escape",
                            "invalid hex byte escape",
                            Span::new(base + i, base + h2_i + h2.len_utf8()),
                        );
                    }
                }
                'u' => {
                    let mut end = j + esc.len_utf8();
                    if iter.peek().map(|(_, c)| *c) == Some('{') {
                        iter.next();
                        end += 1;
                        for (k, c) in iter.by_ref() {
                            end = k + c.len_utf8();
                            if c == '}' {
                                break;
                            }
                        }
                    }
                    self.error(
                        "parse.byte_unicode_escape",
                        "byte strings do not support unicode escapes; use `\\xNN` bytes",
                        Span::new(base + i, base + end),
                    );
                }
                other => {
                    self.error(
                        "parse.invalid_escape",
                        format!("unknown byte escape sequence `\\{other}`"),
                        Span::new(base + i, base + j + other.len_utf8()),
                    );
                    let mut buf = [0; 4];
                    out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
                }
            }
        }
        out
    }

    fn normalize_triple_string_content(&self, content: &str) -> String {
        self.normalize_triple_string_content_with_offsets(content)
            .text
    }

    fn normalize_triple_string_content_with_offsets(
        &self,
        content: &str,
    ) -> NormalizedTripleString {
        let mut normalized = String::new();
        let mut source_offsets = Vec::new();
        let bytes = content.as_bytes();
        let mut i = 0usize;
        while i < content.len() {
            if bytes[i..].starts_with(b"\r\n") {
                source_offsets.push(i);
                normalized.push('\n');
                i += 2;
                continue;
            }
            let ch = content[i..].chars().next().unwrap();
            for byte_offset in 0..ch.len_utf8() {
                source_offsets.push(i + byte_offset);
            }
            normalized.push(ch);
            i += ch.len_utf8();
        }
        source_offsets.push(content.len());

        let mut start = 0usize;
        let mut end = normalized.len();
        if normalized.starts_with('\n') {
            start = 1;
        }

        let mut baseline = "";
        if let Some(last_newline) = normalized[start..end].rfind('\n') {
            let last_newline = start + last_newline;
            let tail = &normalized[last_newline + 1..end];
            if tail.chars().all(|ch| ch == ' ' || ch == '\t') {
                baseline = tail;
                end = last_newline;
            }
        }

        let mut text = String::new();
        let mut offsets = Vec::new();
        if baseline.is_empty() {
            append_mapped_range(
                &mut text,
                &mut offsets,
                &normalized,
                &source_offsets,
                start,
                end,
            );
        } else {
            let mut line_start = start;
            while line_start <= end {
                let line_end = normalized[line_start..end]
                    .find('\n')
                    .map(|idx| line_start + idx)
                    .unwrap_or(end);
                let line = &normalized[line_start..line_end];
                let content_start = if line.starts_with(baseline) {
                    line_start + baseline.len()
                } else {
                    line_start
                };
                append_mapped_range(
                    &mut text,
                    &mut offsets,
                    &normalized,
                    &source_offsets,
                    content_start,
                    line_end,
                );
                if line_end >= end {
                    break;
                }
                offsets.push(source_offsets[line_end]);
                text.push('\n');
                line_start = line_end + 1;
            }
        }
        offsets.push(source_offsets[end]);
        NormalizedTripleString { text, offsets }
    }

    fn parse_fstr_parts(
        &mut self,
        content: &str,
        base: usize,
        offsets: Option<&[usize]>,
    ) -> Vec<FStrPart> {
        let mut parts = Vec::new();
        let mut literal_start = 0usize;
        let mut i = 0usize;

        while i < content.len() {
            let ch = content[i..].chars().next().unwrap();
            if ch == '{' {
                if content[i + 1..].starts_with('{') {
                    if literal_start < i {
                        let literal = self.unescape_string_content(
                            &content[literal_start..i],
                            fstr_source_offset(base, offsets, literal_start),
                        );
                        if !literal.is_empty() {
                            parts.push(FStrPart::Literal(literal));
                        }
                    }
                    parts.push(FStrPart::Literal(SmolStr::new("{")));
                    i += 2;
                    literal_start = i;
                    continue;
                }
                if literal_start < i {
                    let literal = self.unescape_string_content(
                        &content[literal_start..i],
                        fstr_source_offset(base, offsets, literal_start),
                    );
                    if !literal.is_empty() {
                        parts.push(FStrPart::Literal(literal));
                    }
                }
                let expr_start = i + 1;
                let Some(expr_end) = find_fstr_expr_end(content, expr_start) else {
                    self.error(
                        "parse.fstring_unclosed_expr",
                        "unterminated f-string interpolation",
                        Span::new(
                            fstr_source_offset(base, offsets, i),
                            fstr_source_offset(base, offsets, content.len()),
                        ),
                    );
                    literal_start = i;
                    break;
                };
                let raw_expr = &content[expr_start..expr_end];
                let leading_trim = raw_expr.len() - raw_expr.trim_start().len();
                let expr_src = raw_expr.trim();
                if expr_src.is_empty() {
                    self.error(
                        "parse.fstring_empty_expr",
                        "empty f-string interpolation",
                        Span::new(
                            fstr_source_offset(base, offsets, expr_start),
                            fstr_source_offset(base, offsets, expr_end),
                        ),
                    );
                } else if let Some(expr) = self.parse_fstr_interpolated_expr(
                    expr_src,
                    Span::new(
                        fstr_source_offset(base, offsets, expr_start + leading_trim),
                        fstr_source_offset(
                            base,
                            offsets,
                            expr_start + leading_trim + expr_src.len(),
                        ),
                    ),
                ) {
                    parts.push(FStrPart::Interpolated(Box::new(expr)));
                }
                i = expr_end + 1;
                literal_start = i;
                continue;
            }

            if ch == '}' {
                if content[i + 1..].starts_with('}') {
                    if literal_start < i {
                        let literal = self.unescape_string_content(
                            &content[literal_start..i],
                            fstr_source_offset(base, offsets, literal_start),
                        );
                        if !literal.is_empty() {
                            parts.push(FStrPart::Literal(literal));
                        }
                    }
                    parts.push(FStrPart::Literal(SmolStr::new("}")));
                    i += 2;
                    literal_start = i;
                    continue;
                }
                self.error(
                    "parse.fstring_unmatched_brace",
                    "unmatched `}` in f-string",
                    Span::new(
                        fstr_source_offset(base, offsets, i),
                        fstr_source_offset(base, offsets, i + 1),
                    ),
                );
            }
            i += ch.len_utf8();
        }

        if literal_start < content.len() {
            let literal = self.unescape_string_content(
                &content[literal_start..],
                fstr_source_offset(base, offsets, literal_start),
            );
            if !literal.is_empty() {
                parts.push(FStrPart::Literal(literal));
            }
        }

        if parts.is_empty() {
            parts.push(FStrPart::Literal(SmolStr::new("")));
        }
        parts
    }

    fn parse_fstr_interpolated_expr(&mut self, source: &str, span: Span) -> Option<Expr> {
        let mut nested_sink = DiagnosticSink::default();
        let mut tokens = lex(source, self.file_id, &mut nested_sink);
        let offset = span.start as u32;
        for token in &mut tokens {
            token.span = offset_span(token.span, offset);
        }
        for mut diagnostic in nested_sink.into_diagnostics() {
            offset_diagnostic_spans(&mut diagnostic, offset);
            self.sink.emit(diagnostic);
        }

        let expr = {
            let mut parser = Parser::new(&tokens, self.source, self.file_id, self.sink);
            let expr = parser.parse_expr();
            if expr.is_some() && !parser.at_eof() {
                parser.error(
                    "parse.fstring_expr_trailing_tokens",
                    "unexpected tokens after f-string expression",
                    parser.current_span(),
                );
            }
            expr
        };
        expr
    }

    fn parse_closure_expr(&mut self, start: ori_diagnostics::Span) -> Option<Expr> {
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let pspan = self.current_span();
            let name = self.parse_name()?;
            let ty = if self.eat(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let end = ty.as_ref().map(|t| t.span()).unwrap_or(name.span);
            params.push(ClosureParam {
                name,
                ty,
                span: pspan.cover(end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen)?;
        let return_ty = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        // `=> expr` for single-expression closures; otherwise statement block + `end`.
        let (body, end) = if self.eat(&TokenKind::FatArrow) {
            let e = self.parse_expr()?;
            let s = e.span();
            (ClosureBody::Expr(Box::new(e)), s)
        } else {
            let block = self.parse_block()?;
            let end_span = self.expect_block_end(start, "closure")?;
            (ClosureBody::Block(block), end_span)
        };
        Some(Expr::Closure(Box::new(ClosureExpr {
            params,
            return_ty,
            body,
            span: start.cover(end),
        })))
    }

    /// True when `(` starts a S3 closure `(params) => …` / `(params) … end`.
    fn looks_like_closure(&self) -> bool {
        if self.peek_kind() != Some(&TokenKind::LParen) {
            return false;
        }
        let mut i = self.next_non_trivia(self.pos + 1);
        // Empty parameter list: `() =>` / `() ->` / `() … end`
        if self.token_kind_at(i) == Some(&TokenKind::RParen) {
            i = self.next_non_trivia(i + 1);
            return self.closure_suffix_at(i);
        }
        // Params: IDENT [":" type] { "," IDENT [":" type] }
        loop {
            if self.token_kind_at(i) != Some(&TokenKind::Ident) {
                return false;
            }
            i = self.next_non_trivia(i + 1);
            if self.token_kind_at(i) == Some(&TokenKind::Colon) {
                i = self.next_non_trivia(i + 1);
                i = self.skip_type_like_tokens(i);
            }
            match self.token_kind_at(i) {
                Some(TokenKind::Comma) => {
                    i = self.next_non_trivia(i + 1);
                    continue;
                }
                Some(TokenKind::RParen) => {
                    i = self.next_non_trivia(i + 1);
                    return self.closure_suffix_at(i);
                }
                _ => return false,
            }
        }
    }

    fn closure_suffix_at(&self, i: usize) -> bool {
        match self.token_kind_at(i) {
            Some(TokenKind::FatArrow) | Some(TokenKind::Arrow) => true,
            // Long form: only commit when the body clearly starts with a statement
            // keyword. Bare `Ident` after `(a)` is the next statement, not a closure.
            Some(
                TokenKind::Const
                | TokenKind::Var
                | TokenKind::Return
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Repeat
                | TokenKind::Loop
                | TokenKind::Match
                | TokenKind::Using
                | TokenKind::Check
                | TokenKind::Break
                | TokenKind::Continue,
            ) => true,
            _ => false,
        }
    }

    fn token_kind_at(&self, index: usize) -> Option<&TokenKind> {
        self.tokens.get(index).map(|t| &t.kind)
    }

    fn next_non_trivia(&self, mut index: usize) -> usize {
        while index < self.tokens.len() && self.tokens[index].is_trivia() {
            index += 1;
        }
        index
    }

    /// Advance index past a type-shaped token sequence (paren/bracket depth aware).
    fn skip_type_like_tokens(&self, mut i: usize) -> usize {
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut saw_any = false;
        while i < self.tokens.len() {
            if self.tokens[i].is_trivia() {
                i += 1;
                continue;
            }
            match self.tokens[i].kind {
                TokenKind::Comma | TokenKind::RParen if paren == 0 && bracket == 0 && saw_any => {
                    return i;
                }
                TokenKind::LParen => {
                    paren += 1;
                    saw_any = true;
                    i += 1;
                }
                TokenKind::RParen => {
                    if paren == 0 {
                        return i;
                    }
                    paren -= 1;
                    i += 1;
                }
                TokenKind::LBracket => {
                    bracket += 1;
                    saw_any = true;
                    i += 1;
                }
                TokenKind::RBracket => {
                    bracket = bracket.saturating_sub(1);
                    i += 1;
                }
                TokenKind::FatArrow if paren == 0 && bracket == 0 => return i,
                _ => {
                    saw_any = true;
                    i += 1;
                }
            }
        }
        i
    }
}

/// Callees eligible for poetic juxtaposition (`print name`, `io.print x`).
fn is_poetic_callee(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Ident(_)
            | Expr::QualifiedIdent(_)
            | Expr::Field { .. }
            | Expr::Call { .. }
            | Expr::Index { .. }
            | Expr::TupleIndex { .. }
            | Expr::SelfExpr(_)
    )
}

fn hex_value(ch: char) -> Option<u8> {
    match ch {
        '0'..='9' => Some(ch as u8 - b'0'),
        'a'..='f' => Some(ch as u8 - b'a' + 10),
        'A'..='F' => Some(ch as u8 - b'A' + 10),
        _ => None,
    }
}

fn append_mapped_range(
    text: &mut String,
    offsets: &mut Vec<usize>,
    source: &str,
    source_offsets: &[usize],
    start: usize,
    end: usize,
) {
    offsets.extend((start..end).map(|idx| source_offsets[idx]));
    text.push_str(&source[start..end]);
}

fn fstr_source_offset(base: usize, offsets: Option<&[usize]>, idx: usize) -> usize {
    base + offsets
        .and_then(|items| items.get(idx).copied())
        .unwrap_or(idx)
}

fn offset_span(span: Span, offset: u32) -> Span {
    Span {
        start: span.start + offset,
        end: span.end + offset,
    }
}

fn offset_diagnostic_spans(diagnostic: &mut ori_diagnostics::Diagnostic, offset: u32) {
    for label in &mut diagnostic.labels {
        label.span = offset_span(label.span, offset);
    }
}

fn find_fstr_expr_end(content: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while i < content.len() {
        let ch = content[i..].chars().next().unwrap();
        match ch {
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' if depth == 0 => return Some(i),
            '}' => {
                depth -= 1;
                i += 1;
            }
            '"' | '\'' => {
                i = skip_quoted_in_fstr_expr(content, i, ch)?;
            }
            _ => i += ch.len_utf8(),
        }
    }
    None
}

fn skip_quoted_in_fstr_expr(content: &str, start: usize, quote: char) -> Option<usize> {
    let mut escaped = false;
    let mut i = start + quote.len_utf8();
    while i < content.len() {
        let ch = content[i..].chars().next().unwrap();
        if escaped {
            escaped = false;
            i += ch.len_utf8();
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        i += ch.len_utf8();
        if ch == quote {
            return Some(i);
        }
    }
    None
}
