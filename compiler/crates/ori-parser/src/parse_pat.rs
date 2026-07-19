use crate::parser::Parser;
use ori_ast::common::Name;
use ori_ast::expr::{Expr, UnaryOp};
use ori_ast::pattern::{NamedPattern, Pattern};
use ori_lexer::TokenKind;

impl<'src> Parser<'src> {
    /// A full arm pattern, including `a or b or c` alternatives.
    ///
    /// Safe to look for `or` here: literal patterns are parsed with
    /// `parse_primary_expr`, which never consumes binary operators, so the
    /// `or` can only belong to the pattern.
    pub fn parse_pattern(&mut self) -> Option<Pattern> {
        let first = self.parse_pattern_alternative()?;
        if !self.at(&TokenKind::Or) {
            return Some(first);
        }
        let start = first.span();
        let mut alternatives = vec![first];
        while self.eat(&TokenKind::Or) {
            alternatives.push(self.parse_pattern_alternative()?);
        }
        let end = alternatives.last().map(|p| p.span()).unwrap_or(start);
        Some(Pattern::Or(alternatives, start.cover(end)))
    }

    fn parse_pattern_alternative(&mut self) -> Option<Pattern> {
        let span = self.current_span();
        match self.peek_kind()? {
            // `_` wildcard
            TokenKind::Ident if self.slice(span) == "_" => {
                self.advance();
                Some(Pattern::Wildcard(span))
            }

            // `none`
            TokenKind::None => {
                self.advance();
                Some(Pattern::None(span))
            }

            // `some(pat)`
            TokenKind::Some => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_pattern()?;
                let end = self.expect(&TokenKind::RParen)?;
                Some(Pattern::Some(Box::new(inner), span.cover(end)))
            }

            // Soft keywords: `ok(pat)` / `err(pat)` when Ident is followed by `(`.
            TokenKind::Ident
                if {
                    let s = self.slice(span);
                    (s == "ok" || s == "err") && self.peek_nth_kind(1) == Some(&TokenKind::LParen)
                } =>
            {
                let is_ok = self.slice(span) == "ok";
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_pattern()?;
                let end = self.expect(&TokenKind::RParen)?;
                if is_ok {
                    Some(Pattern::Ok(Box::new(inner), span.cover(end)))
                } else {
                    Some(Pattern::Err(Box::new(inner), span.cover(end)))
                }
            }

            // Removed: `success(...)` / `error(...)` patterns
            TokenKind::SuccessRemoved | TokenKind::ErrorRemoved => {
                let removed = self.advance().unwrap();
                let old = self.slice(removed.span);
                let new = if old == "success" { "ok" } else { "err" };
                self.error(
                    "parse.result_ctor_renamed",
                    format!("`{old}` was renamed to `{new}`; write `case {new}(...)`"),
                    removed.span,
                );
                self.expect(&TokenKind::LParen)?;
                let inner = self.parse_pattern()?;
                let end = self.expect(&TokenKind::RParen)?;
                if new == "ok" {
                    Some(Pattern::Ok(Box::new(inner), span.cover(end)))
                } else {
                    Some(Pattern::Err(Box::new(inner), span.cover(end)))
                }
            }

            // `tuple(pat, pat, ...)`
            TokenKind::Tuple => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let mut pats = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    pats.push(self.parse_pattern()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RParen)?;
                Some(Pattern::Tuple(pats, span.cover(end)))
            }

            // S3: match cases use bare `Variant` / `Variant(...)` — leading dot is an error.
            TokenKind::Dot => {
                let dot_span = self.advance().unwrap().span;
                self.error(
                    "parse.case_dot_variant_removed",
                    "leading `.` on match enum variants was removed; write `case Variant` or `case Variant(...)`",
                    dot_span,
                );
                // Recover by parsing the remainder as a non-shorthand variant pattern.
                let name = self.parse_name()?;
                self.parse_variant_pattern(name, false)
            }

            // Literal: `true`, `false`, integer, float, string
            TokenKind::True
            | TokenKind::False
            | TokenKind::IntLit
            | TokenKind::FloatLit
            | TokenKind::StrLit => {
                let expr = self.parse_primary_expr()?;
                Some(Pattern::Literal(Box::new(expr)))
            }
            TokenKind::Minus => {
                self.advance(); // consume `-`
                let inner = self.parse_primary_expr()?;
                let s = span.cover(inner.span());
                Some(Pattern::Literal(Box::new(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(inner),
                    span: s,
                })))
            }

            // Identifier: binding or `Variant` / `Variant(fields)`
            TokenKind::Ident => {
                let name = self.parse_name()?;
                // If followed by `(`, it's a named variant pattern
                if self.at(&TokenKind::LParen) {
                    self.parse_variant_pattern(name, false)
                } else {
                    // Plain binding
                    Some(Pattern::Binding(name))
                }
            }

            _ => {
                let span = self.current_span();
                self.error("parse.expected_pattern", "expected a pattern", span);
                None
            }
        }
    }

    fn parse_variant_pattern(&mut self, name: Name, shorthand: bool) -> Option<Pattern> {
        let start = name.span;
        if self.at(&TokenKind::LParen) {
            self.advance(); // (
            let mut fields = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                if !matches!(self.peek_kind(), Some(TokenKind::Ident)) {
                    let span = self.current_span();
                    self.error(
                        "parse.expected_identifier",
                        "expected variant payload field name",
                        span,
                    );
                    self.recover_variant_pattern_field();
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    continue;
                }
                let field_name = self.parse_name()?;
                let pattern = if self.eat(&TokenKind::Colon) {
                    self.parse_pattern()?
                } else {
                    // Shorthand: `field` means `field: field`
                    Pattern::Binding(field_name.clone())
                };
                let span = field_name.span.cover(pattern.span());
                fields.push(NamedPattern {
                    name: field_name,
                    pattern,
                    span,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            let end = self.expect(&TokenKind::RParen)?;
            Some(Pattern::VariantNamed {
                name,
                fields,
                shorthand,
                span: start.cover(end),
            })
        } else {
            Some(Pattern::VariantUnit {
                name,
                shorthand,
                span: start,
            })
        }
    }

    fn recover_variant_pattern_field(&mut self) {
        let mut paren_depth = 0usize;
        while !self.at_eof() {
            match self.peek_kind() {
                Some(TokenKind::LParen) => {
                    paren_depth += 1;
                    self.advance();
                }
                Some(TokenKind::RParen) if paren_depth == 0 => break,
                Some(TokenKind::RParen) => {
                    paren_depth -= 1;
                    self.advance();
                }
                Some(TokenKind::Comma) if paren_depth == 0 => break,
                Some(_) => {
                    self.advance();
                }
                None => break,
            }
        }
    }
}
