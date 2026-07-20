use crate::parser::Parser;
use ori_ast::ty::Type;
use ori_lexer::TokenKind;

macro_rules! primitive_type {
    ($self:expr, $span:expr, $variant:ident) => {{
        $self.advance();
        Some(Type::$variant($span))
    }};
}

/// Single-arg built-ins: `list[T]` (canonical) or recover `list of T` as AST + error.
macro_rules! single_bracket_type {
    ($self:expr, $span:expr, $variant:ident) => {{
        $self.advance();
        if $self.at_contextual("of") {
            let of_span = $self.current_span();
            $self.error(
                "parse.removed_of_type",
                "`of` type forms are removed; write `list[T]`, `optional[T]`, `result[T, E]`, …",
                of_span,
            );
            $self.advance(); // of
            // Recovery: build the type so the surrounding statement/block continues
            // and later diagnostics (e.g. angle forms) are still reported.
            let inner = $self.parse_type()?;
            let end = inner.span();
            Some(Type::$variant(Box::new(inner), $span.cover(end)))
        } else {
            let (inner, end) = $self.parse_single_type_arg()?;
            Some(Type::$variant(Box::new(inner), $span.cover(end)))
        }
    }};
}

impl<'src> Parser<'src> {
    pub fn parse_type(&mut self) -> Option<Type> {
        let span = self.current_span();
        match self.peek_kind()? {
            // ── Primitive types ───────────────────────────────────────────────
            TokenKind::BoolTy => primitive_type!(self, span, Bool),
            TokenKind::IntTy => primitive_type!(self, span, Int),
            TokenKind::Int8Ty => primitive_type!(self, span, Int8),
            TokenKind::Int16Ty => primitive_type!(self, span, Int16),
            TokenKind::Int32Ty => primitive_type!(self, span, Int32),
            TokenKind::Int64Ty => primitive_type!(self, span, Int64),
            TokenKind::U8Ty => primitive_type!(self, span, U8),
            TokenKind::U16Ty => primitive_type!(self, span, U16),
            TokenKind::U32Ty => primitive_type!(self, span, U32),
            TokenKind::U64Ty => primitive_type!(self, span, U64),
            TokenKind::FloatTy => primitive_type!(self, span, Float),
            TokenKind::Float32Ty => primitive_type!(self, span, Float32),
            TokenKind::Float64Ty => primitive_type!(self, span, Float64),
            TokenKind::StringTy => primitive_type!(self, span, String),
            TokenKind::BytesTy => primitive_type!(self, span, Bytes),
            TokenKind::Void => primitive_type!(self, span, Void),

            // ── Single-arg generic built-in types ─────────────────────────────
            TokenKind::Optional => single_bracket_type!(self, span, Optional),
            TokenKind::List => single_bracket_type!(self, span, List),
            TokenKind::Set => single_bracket_type!(self, span, Set),
            TokenKind::Range => single_bracket_type!(self, span, Range),
            TokenKind::Lazy => single_bracket_type!(self, span, Lazy),
            TokenKind::Handle => single_bracket_type!(self, span, Handle),

            // ── Multi-arg generic built-in types ──────────────────────────────
            TokenKind::ResultKw => {
                self.advance();
                if self.at_contextual("of") {
                    let of_span = self.current_span();
                    self.error(
                        "parse.removed_of_type",
                        "`of` type forms are removed; write `result[T, E]`",
                        of_span,
                    );
                    self.advance(); // of
                    let ok = self.parse_type()?;
                    if self.at_contextual("to") || self.at_contextual("of") {
                        self.advance();
                    }
                    // Recovery: second type if present; else reuse ok for a complete AST.
                    let err = if self
                        .peek_kind()
                        .is_some_and(|k| self.kind_can_start_type(k))
                    {
                        self.parse_type()?
                    } else {
                        ok.clone()
                    };
                    let end = err.span();
                    return Some(Type::Result(Box::new(ok), Box::new(err), span.cover(end)));
                }
                let (args, end) = self.parse_type_arg_list(2)?;
                Some(Type::Result(
                    Box::new(args[0].clone()),
                    Box::new(args[1].clone()),
                    span.cover(end),
                ))
            }
            TokenKind::Map => {
                self.advance();
                // Removed: `map of K to V`
                if self.at_contextual("of") {
                    let of_span = self.current_span();
                    self.error(
                        "parse.removed_of_type",
                        "`of` type forms are removed; write `map[K, V]`",
                        of_span,
                    );
                    self.advance(); // of
                    let key = self.parse_type()?;
                    if self.at_contextual("to") {
                        self.advance();
                    }
                    let val = self.parse_type()?;
                    let end = val.span();
                    return Some(Type::Map(Box::new(key), Box::new(val), span.cover(end)));
                }
                let (args, end) = self.parse_type_arg_list(2)?;
                Some(Type::Map(
                    Box::new(args[0].clone()),
                    Box::new(args[1].clone()),
                    span.cover(end),
                ))
            }
            TokenKind::Any => {
                self.advance();
                if self.at_contextual("of") {
                    let of_span = self.current_span();
                    self.error(
                        "parse.removed_of_type",
                        "`of` type forms are removed; write `any[Trait]`",
                        of_span,
                    );
                    self.advance(); // of
                    let trait_name = self.parse_qualified_name()?;
                    let end = trait_name.span;
                    return Some(Type::Any(trait_name, span.cover(end)));
                }
                let open = match self.peek_kind() {
                    Some(TokenKind::LBracket) => TokenKind::LBracket,
                    Some(TokenKind::Lt) => {
                        self.error_removed_angle_type(self.current_span());
                        TokenKind::Lt
                    }
                    _ => {
                        let span = self.current_span();
                        self.error(
                            "parse.expected_type",
                            "expected type arguments in `[...]`",
                            span,
                        );
                        return None;
                    }
                };
                self.advance(); // [ or <
                let trait_name = self.parse_qualified_name()?;
                let end = if open == TokenKind::LBracket {
                    self.expect(&TokenKind::RBracket)?
                } else {
                    self.expect(&TokenKind::Gt)?
                };
                Some(Type::Any(trait_name, span.cover(end)))
            }
            TokenKind::Tuple => {
                self.advance();
                if self.at_contextual("of") {
                    let of_span = self.current_span();
                    self.error(
                        "parse.removed_of_type",
                        "`of` type forms are removed; write `tuple[T, U, …]`",
                        of_span,
                    );
                    self.advance(); // of
                                    // Recovery: one or more types until a non-type token.
                    let mut args = Vec::new();
                    if self
                        .peek_kind()
                        .is_some_and(|k| self.kind_can_start_type(k))
                    {
                        args.push(self.parse_type()?);
                        while self.eat(&TokenKind::Comma)
                            && self
                                .peek_kind()
                                .is_some_and(|k| self.kind_can_start_type(k))
                        {
                            args.push(self.parse_type()?);
                        }
                    }
                    let end = args.last().map(|t| t.span()).unwrap_or(of_span);
                    return Some(Type::Tuple(args, span.cover(end)));
                }
                let (args, end) = self.parse_type_arg_list_free()?;
                Some(Type::Tuple(args, span.cover(end)))
            }

            // ── Callable type: `func(T, U) -> R` ─────────────────────────────
            TokenKind::Func => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let params = if self.at(&TokenKind::RParen) {
                    Vec::new()
                } else {
                    self.parse_type_list(&TokenKind::RParen)?
                };
                let end_paren = self.expect(&TokenKind::RParen)?;
                let return_ty = if self.eat(&TokenKind::Arrow) {
                    let ty = self.parse_type()?;
                    Some(Box::new(ty))
                } else {
                    None
                };
                let end = return_ty.as_ref().map(|t| t.span()).unwrap_or(end_paren);
                Some(Type::Func {
                    params,
                    return_ty,
                    span: span.cover(end),
                })
            }

            // ── User-defined type: `Name` or `Name[T, U]` ────────────────────
            TokenKind::Ident => {
                let name = self.parse_qualified_name()?;
                if self.at(&TokenKind::LBracket) {
                    self.advance(); // [
                    let args = self.parse_type_list(&TokenKind::RBracket)?;
                    let end = self.expect(&TokenKind::RBracket)?;
                    Some(Type::Generic {
                        name,
                        args,
                        span: span.cover(end),
                    })
                } else if self.at(&TokenKind::Lt) && self.peek_nth_kind(1) != Some(&TokenKind::Eq) {
                    // Removed angle-bracket type args: `Name<T>`
                    self.error_removed_angle_type(self.current_span());
                    self.advance(); // <
                    let args = self.parse_type_list(&TokenKind::Gt)?;
                    let end = self.expect(&TokenKind::Gt)?;
                    Some(Type::Generic {
                        name,
                        args,
                        span: span.cover(end),
                    })
                } else {
                    Some(Type::Named(name))
                }
            }

            TokenKind::IntLit | TokenKind::True | TokenKind::False => {
                let tok = self.advance().unwrap();
                let text = smol_str::SmolStr::new(self.slice(tok.span));
                let name = ori_ast::common::Name::new(text, tok.span);
                Some(Type::Named(ori_ast::common::QualifiedName {
                    parts: vec![name],
                    span: tok.span,
                }))
            }

            _ => {
                let span = self.current_span();
                self.error("parse.expected_type", "expected a type", span);
                None
            }
        }
    }

    /// Parse a comma-separated list of types, stopping before `stop`.
    pub fn parse_type_list(&mut self, stop: &TokenKind) -> Option<Vec<Type>> {
        let mut types = Vec::new();
        while !self.at(stop) && !self.at_eof() {
            if let Some(const_arg) = self.parse_named_const_arg() {
                types.push(const_arg);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                continue;
            }
            types.push(self.parse_type()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Some(types)
    }

    /// `size: 8` inside type arguments — a named compile-time constant.
    ///
    /// Named rather than positional so it cannot be misread as indexing: a
    /// bare `Buffer[8]` looks exactly like `frutas[8]`, while `Buffer[size: 8]`
    /// reads as an argument, matching how calls and struct literals already
    /// name their values.
    fn parse_named_const_arg(&mut self) -> Option<Type> {
        if !matches!(self.peek_kind(), Some(TokenKind::Ident))
            || self.peek_nth_kind(1) != Some(&TokenKind::Colon)
        {
            return None;
        }
        let name = self.parse_name()?;
        self.expect(&TokenKind::Colon)?;
        let tok = self.peek()?.clone();
        let negative = tok.kind == TokenKind::Minus;
        if negative {
            self.advance();
        }
        if !matches!(self.peek_kind(), Some(TokenKind::IntLit)) {
            let span = self.current_span();
            self.error(
                "parse.expected_const_arg_value",
                "a named type argument takes an integer constant, e.g. `size: 8`",
                span,
            );
            return None;
        }
        let value_tok = self.advance().unwrap();
        let text = self.slice(value_tok.span).replace('_', "");
        let value: i64 = match text.parse() {
            Ok(v) => v,
            Err(_) => {
                self.error(
                    "parse.expected_const_arg_value",
                    "integer constant is out of range for a type argument",
                    value_tok.span,
                );
                return None;
            }
        };
        let span = name.span.cover(value_tok.span);
        Some(Type::ConstArg {
            name,
            value: if negative { -value } else { value },
            span,
        })
    }

    fn error_removed_angle_type(&mut self, span: ori_diagnostics::Span) {
        self.error(
            "parse.removed_angle_type",
            "angle-bracket type arguments are removed; write `Type[...]` (e.g. `list[T]`)",
            span,
        );
    }

    /// True when the current token can start a type (for of-form recovery).
    fn kind_can_start_type(&self, kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::BoolTy
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
                | TokenKind::StringTy
                | TokenKind::BytesTy
                | TokenKind::Void
                | TokenKind::Optional
                | TokenKind::List
                | TokenKind::Set
                | TokenKind::Range
                | TokenKind::Lazy
                | TokenKind::Handle
                | TokenKind::ResultKw
                | TokenKind::Map
                | TokenKind::Any
                | TokenKind::Tuple
                | TokenKind::Func
                | TokenKind::Ident
                | TokenKind::SelfKw
        )
    }

    /// Parse exactly one type argument in `[T]` (or recover from `<T>`).
    fn parse_single_type_arg(&mut self) -> Option<(Type, ori_diagnostics::Span)> {
        let open = match self.peek_kind() {
            Some(TokenKind::LBracket) => TokenKind::LBracket,
            Some(TokenKind::Lt) => {
                self.error_removed_angle_type(self.current_span());
                TokenKind::Lt
            }
            _ => {
                let span = self.current_span();
                self.error(
                    "parse.expected_type",
                    "expected type arguments in `[...]`",
                    span,
                );
                return None;
            }
        };
        self.advance();
        let inner = self.parse_type()?;
        let close = if open == TokenKind::LBracket {
            TokenKind::RBracket
        } else {
            TokenKind::Gt
        };
        let end = self.expect(&close)?;
        Some((inner, end))
    }

    /// Parse exactly `expected` type arguments in brackets (or recover from angles).
    fn parse_type_arg_list(
        &mut self,
        expected: usize,
    ) -> Option<(Vec<Type>, ori_diagnostics::Span)> {
        let (args, end) = self.parse_type_arg_list_free()?;
        if args.len() != expected {
            self.error(
                "parse.expected_type",
                format!("expected {expected} type argument(s)"),
                end,
            );
        }
        Some((args, end))
    }

    fn parse_type_arg_list_free(&mut self) -> Option<(Vec<Type>, ori_diagnostics::Span)> {
        let open = match self.peek_kind() {
            Some(TokenKind::LBracket) => TokenKind::LBracket,
            Some(TokenKind::Lt) => {
                self.error_removed_angle_type(self.current_span());
                TokenKind::Lt
            }
            _ => {
                let span = self.current_span();
                self.error(
                    "parse.expected_type",
                    "expected type arguments in `[...]`",
                    span,
                );
                return None;
            }
        };
        self.advance();
        let close = if open == TokenKind::LBracket {
            TokenKind::RBracket
        } else {
            TokenKind::Gt
        };
        let args = self.parse_type_list(&close)?;
        let end = self.expect(&close)?;
        Some((args, end))
    }
}
