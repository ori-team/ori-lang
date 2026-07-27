use ori_ast::common::{Name, QualifiedName, TypeParam, WhereClause, WhereConstraint};
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, Span};
use ori_lexer::{Token, TokenKind};
use smol_str::SmolStr;

// ── Parser core ───────────────────────────────────────────────────────────────

pub(crate) struct Parser<'src> {
    pub tokens: &'src [Token],
    pub pos: usize,
    pub source: &'src str,
    pub file_id: FileId,
    pub sink: &'src mut DiagnosticSink,
    /// When false, juxtaposition is not parsed as a poetic call (used inside
    /// poetic arguments so `print greet name` can be rejected as nested).
    pub allow_poetic: bool,
}

impl<'src> Parser<'src> {
    pub fn new(
        tokens: &'src [Token],
        source: &'src str,
        file_id: FileId,
        sink: &'src mut DiagnosticSink,
    ) -> Self {
        let mut p = Self {
            tokens,
            pos: 0,
            source,
            file_id,
            sink,
            allow_poetic: true,
        };
        p.skip_trivia();
        p
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.tokens.len() && self.tokens[self.pos].is_trivia() {
            self.pos += 1;
        }
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    pub fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    pub fn at(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    pub fn at_any(&self, kinds: &[TokenKind]) -> bool {
        self.peek_kind().map_or(false, |k| kinds.contains(k))
    }

    pub fn at_contextual(&self, keyword: &str) -> bool {
        self.peek_kind() == Some(&TokenKind::Ident)
            && self
                .peek()
                .is_some_and(|tok| self.slice(tok.span) == keyword)
    }

    pub fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Peek at the nth non-trivia token ahead (0 = current).
    pub fn peek_nth_kind(&self, n: usize) -> Option<&TokenKind> {
        let mut count = 0usize;
        let mut i = self.pos;
        while i < self.tokens.len() {
            if !self.tokens[i].is_trivia() {
                if count == n {
                    return Some(&self.tokens[i].kind);
                }
                count += 1;
            }
            i += 1;
        }
        None
    }

    pub fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            self.skip_trivia();
            Some(tok)
        } else {
            None
        }
    }

    /// Consume the current token if it matches `kind`. Returns its span.
    pub fn expect(&mut self, kind: &TokenKind) -> Option<Span> {
        if self.peek_kind() == Some(kind) {
            Some(self.advance().unwrap().span)
        } else {
            let span = self.current_span();
            let found = self
                .peek_kind()
                .map(|k| k.display_name())
                .unwrap_or("end of file");
            self.error(
                "parse.unexpected_token",
                format!("expected {}, found {}", kind.display_name(), found),
                span,
            );
            None
        }
    }

    pub fn expect_block_end(&mut self, start: Span, block_name: &'static str) -> Option<Span> {
        if self.peek_kind() != Some(&TokenKind::End) {
            if self.at_eof() {
                let diag = Diagnostic::error(
                    "parse.unterminated_block",
                    format!("{block_name} block is not closed"),
                )
                .with_label(Label::primary(self.file_id, start, "block starts here"))
                .with_why("Ori blocks must be closed with `end`")
                .with_action(format!("add `end` to close this {block_name} block"));
                self.sink.emit(diag);
                return None;
            }
            return self.expect(&TokenKind::End);
        }

        let end_span = self.advance().unwrap().span;
        // Optional labeled end: `end if`, `end match`, … (same line only).
        if let Some((label, label_span)) = self.peek_end_label_same_line(end_span) {
            self.advance();
            let expected = expected_end_label(block_name);
            if label != expected {
                self.error(
                    "parse.end_label_mismatch",
                    format!("labeled `end {label}` does not match opening `{expected}`",),
                    end_span.cover(label_span),
                );
            }
            return Some(end_span.cover(label_span));
        }
        Some(end_span)
    }

    /// Optional construct label after `end` on the same source line.
    fn peek_end_label_same_line(&self, end_span: Span) -> Option<(&'static str, Span)> {
        let tok = self.peek()?;
        if self.source_has_newline_between(end_span.end, tok.span.start) {
            return None;
        }
        let label = match &tok.kind {
            TokenKind::If => "if",
            TokenKind::Match => "match",
            TokenKind::While => "while",
            TokenKind::For => "for",
            TokenKind::Loop => "loop",
            TokenKind::Repeat => "repeat",
            TokenKind::Struct => "struct",
            TokenKind::Enum => "enum",
            TokenKind::Trait => "trait",
            TokenKind::Implement => "implement",
            TokenKind::Extern => "extern",
            TokenKind::Ident => match self.slice(tok.span) {
                "function" => "function",
                "closure" => "closure",
                _ => return None,
            },
            _ => return None,
        };
        Some((label, tok.span))
    }

    pub fn source_has_newline_between(&self, start: u32, end: u32) -> bool {
        if start >= end {
            return false;
        }
        self.source[start as usize..end as usize].contains('\n')
    }

    pub fn same_line_as_span_end(&self, span: Span) -> bool {
        let Some(tok) = self.peek() else {
            return false;
        };
        !self.source_has_newline_between(span.end, tok.span.start)
    }

    /// Consume and return `true` if the current token matches `kind`.
    pub fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == Some(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn eat_contextual(&mut self, keyword: &str) -> bool {
        if self.at_contextual(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn current_span(&self) -> Span {
        self.peek().map(|t| t.span).unwrap_or_else(|| {
            self.tokens
                .last()
                .map(|t| Span::new(t.span.end as usize, t.span.end as usize))
                .unwrap_or(Span::DUMMY)
        })
    }

    pub fn slice(&self, span: Span) -> &str {
        &self.source[span.as_range()]
    }

    pub fn error(&mut self, code: &'static str, msg: impl Into<String>, span: Span) {
        let diag =
            Diagnostic::error(code, msg).with_label(Label::primary(self.file_id, span, "here"));
        self.sink.emit(diag);
    }

    // ── Common grammar helpers ────────────────────────────────────────────────

    pub fn parse_name(&mut self) -> Option<Name> {
        match self.peek_kind() {
            Some(TokenKind::Ident) => {
                let tok = self.advance().unwrap();
                let text = SmolStr::new(self.slice(tok.span));
                Some(Name::new(text, tok.span))
            }
            _ => {
                let span = self.current_span();
                self.error("parse.expected_identifier", "expected identifier", span);
                None
            }
        }
    }

    pub fn parse_member_name(&mut self) -> Option<Name> {
        match self.peek_kind() {
            Some(TokenKind::Ident | TokenKind::Or) => {
                let tok = self.advance().unwrap();
                let text = SmolStr::new(self.slice(tok.span));
                Some(Name::new(text, tok.span))
            }
            _ => {
                let span = self.current_span();
                self.error("parse.expected_identifier", "expected identifier", span);
                None
            }
        }
    }

    fn parse_qualified_name_part(&mut self) -> Option<Name> {
        if !is_qualified_name_part(self.peek_kind()) {
            let span = self.current_span();
            self.error("parse.expected_identifier", "expected identifier", span);
            return None;
        }
        let tok = self.advance().unwrap();
        let text = SmolStr::new(self.slice(tok.span));
        Some(Name::new(text, tok.span))
    }

    /// Parse `ident (.ident)*` in a type or import context.
    pub fn parse_qualified_name(&mut self) -> Option<QualifiedName> {
        let first = self.parse_qualified_name_part()?;
        let mut span = first.span;
        let mut parts = vec![first];
        // Continue only if Dot is followed by Ident (not a field access)
        while self.at(&TokenKind::Dot) && is_qualified_name_part(self.peek_nth_kind(1)) {
            self.advance(); // dot
            let name = self.parse_qualified_name_part()?;
            span = span.cover(name.span);
            parts.push(name);
        }
        Some(QualifiedName { parts, span })
    }

    /// Parse optional `[T, U, …]` type parameters (S3).
    ///
    /// Legacy `<T, U>` still parses for recovery and emits `parse.removed_angle_type`.
    pub fn parse_type_params_opt(&mut self) -> Vec<TypeParam> {
        if self.at(&TokenKind::LBracket) {
            return self.parse_bracket_type_params();
        }
        if self.at(&TokenKind::Lt) {
            let span = self.current_span();
            self.error(
                "parse.removed_angle_type",
                "angle-bracket type parameters are removed; write `Name[T]` (e.g. `Pair[A, B]`)",
                span,
            );
            return self.parse_angle_type_params_recovery();
        }
        Vec::new()
    }

    fn parse_bracket_type_params(&mut self) -> Vec<TypeParam> {
        self.advance(); // [
        let mut params = Vec::new();
        loop {
            if self.at_eof() || self.at(&TokenKind::RBracket) {
                break;
            }
            let is_const = self.eat(&TokenKind::Const);
            if let Some(name) = self.parse_name() {
                if is_const && self.eat(&TokenKind::Colon) {
                    let _ = self.parse_type();
                }
                // Higher-kinded placeholder: `F[_]` after the param name.
                if self.at(&TokenKind::LBracket) {
                    self.skip_bracket_group();
                }
                params.push(TypeParam { name });
            } else {
                break;
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBracket);
        params
    }

    fn skip_bracket_group(&mut self) {
        if !self.at(&TokenKind::LBracket) {
            return;
        }
        let mut depth = 0usize;
        while !self.at_eof() {
            if self.at(&TokenKind::LBracket) {
                depth += 1;
                self.advance();
                continue;
            }
            if self.at(&TokenKind::RBracket) {
                self.advance();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                continue;
            }
            self.advance();
        }
    }

    fn parse_angle_type_params_recovery(&mut self) -> Vec<TypeParam> {
        self.advance(); // <
        let mut params = Vec::new();
        loop {
            if self.at_eof() || self.at(&TokenKind::Gt) {
                break;
            }
            let is_const = self.eat(&TokenKind::Const);
            if let Some(name) = self.parse_name() {
                if is_const && self.eat(&TokenKind::Colon) {
                    let _ = self.parse_type();
                }
                params.push(TypeParam { name });
                if self.at(&TokenKind::Lt) {
                    self.skip_angle_group();
                }
            } else {
                break;
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Gt);
        params
    }

    fn skip_angle_group(&mut self) {
        if !self.at(&TokenKind::Lt) {
            return;
        }
        let mut depth = 0usize;
        while !self.at_eof() {
            if self.at(&TokenKind::Lt) {
                depth += 1;
                self.advance();
                continue;
            }
            if self.at(&TokenKind::Gt) {
                self.advance();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                continue;
            }
            self.advance();
        }
    }

    /// Parse Auk9-style bounds: `for T: Trait, U: not Disposable`.
    ///
    /// Type parameters introduced only via bounds are appended to `type_params`.
    /// Returns a `WhereClause` so the checker/HIR keep using the existing AST.
    pub fn parse_for_bounds_opt(
        &mut self,
        type_params: &mut Vec<TypeParam>,
    ) -> Option<WhereClause> {
        if !self.at(&TokenKind::For) {
            return None;
        }
        // Disambiguate `for T:` bounds from `implement Trait for Type` / for-loops.
        // Bounds require `for Ident :`.
        if self.peek_nth_kind(1) != Some(&TokenKind::Ident) {
            return None;
        }
        // Look ahead past the name for `:`.
        if !self.peek_for_bound_colon() {
            return None;
        }

        let start = self.advance().unwrap().span; // `for`
        let mut constraints = Vec::new();
        loop {
            let param = self.parse_name()?;
            self.expect(&TokenKind::Colon)?;
            let negated = self.eat(&TokenKind::Not);
            let bound = self.parse_qualified_name()?;
            let span = param.span.cover(bound.span);
            if !type_params.iter().any(|p| p.name.text == param.text) {
                type_params.push(TypeParam {
                    name: param.clone(),
                });
            }
            if negated {
                constraints.push(WhereConstraint::IsNot { param, bound, span });
            } else {
                constraints.push(WhereConstraint::Is { param, bound, span });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = constraints
            .last()
            .map(|c| match c {
                WhereConstraint::Is { span, .. } => *span,
                WhereConstraint::IsNot { span, .. } => *span,
            })
            .unwrap_or(start);
        Some(WhereClause {
            constraints,
            span: start.cover(end),
        })
    }

    /// True when tokens look like `for Ident :` starting at the current position.
    fn peek_for_bound_colon(&self) -> bool {
        let mut count = 0usize;
        let mut i = self.pos;
        while i < self.tokens.len() {
            if self.tokens[i].is_trivia() {
                i += 1;
                continue;
            }
            let kind = &self.tokens[i].kind;
            match count {
                0 => {
                    if *kind != TokenKind::For {
                        return false;
                    }
                }
                1 => {
                    if *kind != TokenKind::Ident {
                        return false;
                    }
                }
                2 => return *kind == TokenKind::Colon,
                _ => return false,
            }
            count += 1;
            i += 1;
        }
        false
    }

    /// Reject legacy `where T is Trait` (S3). Still parses for recovery when present.
    pub fn parse_where_clause_opt(&mut self) -> Option<WhereClause> {
        if !self.at(&TokenKind::Where) {
            return None;
        }
        let start = self.current_span();
        self.error(
            "parse.removed_where_bound",
            "`where T is Trait` bounds are removed; write `for T: Trait` after the name (Auk9-style)",
            start,
        );
        self.parse_legacy_where_clause_recovery()
    }

    fn parse_legacy_where_clause_recovery(&mut self) -> Option<WhereClause> {
        let start = self.advance().unwrap().span; // `where`
        let mut constraints = Vec::new();
        let grouped = self.eat(&TokenKind::LParen);
        loop {
            if grouped && self.at(&TokenKind::RParen) {
                break;
            }
            let param = self.parse_name()?;
            let negated = if self.eat(&TokenKind::Is) {
                self.eat(&TokenKind::Not)
            } else {
                self.expect(&TokenKind::Is)?;
                false
            };
            let bound = self.parse_qualified_name()?;
            let span = param.span.cover(bound.span);
            if negated {
                constraints.push(WhereConstraint::IsNot { param, bound, span });
            } else {
                constraints.push(WhereConstraint::Is { param, bound, span });
            }
            if self.eat(&TokenKind::Comma) || self.eat(&TokenKind::And) {
                if grouped && self.at(&TokenKind::RParen) {
                    break;
                }
                continue;
            } else {
                break;
            }
        }
        if grouped {
            self.expect(&TokenKind::RParen)?;
        }
        let end = constraints
            .last()
            .map(|c| match c {
                WhereConstraint::Is { span, .. } => *span,
                WhereConstraint::IsNot { span, .. } => *span,
            })
            .unwrap_or(start);
        Some(WhereClause {
            constraints,
            span: start.cover(end),
        })
    }

    /// Advance past all tokens that are NOT in the sync set (error recovery).
    pub fn synchronize(&mut self, sync: &[TokenKind]) {
        while !self.at_eof() && !self.at_any(sync) {
            self.advance();
        }
    }
}

/// Map internal block names (`if some`, `function`, …) to the optional `end` label.
fn expected_end_label(block_name: &str) -> &str {
    match block_name {
        "if some" => "if",
        "while some" => "while",
        // Trait defaults / free methods share the `function` label.
        "trait method" | "function" => "function",
        // `base with { … } end struct` — same label as struct declarations.
        "struct update" => "struct",
        other => other,
    }
}

fn is_qualified_name_part(kind: Option<&TokenKind>) -> bool {
    matches!(
        kind,
        Some(TokenKind::Ident)
            // Allow `module` as a path segment (`pkg.module.name`) even though it is
            // a keyword for the file header.
            | Some(TokenKind::Module)
            | Some(TokenKind::Any)
            | Some(TokenKind::Optional)
            | Some(TokenKind::ResultKw)
            | Some(TokenKind::List)
            | Some(TokenKind::Map)
            | Some(TokenKind::Set)
            | Some(TokenKind::Range)
            | Some(TokenKind::Repeat)
            | Some(TokenKind::Void)
            | Some(TokenKind::Tuple)
            | Some(TokenKind::Lazy)
            | Some(TokenKind::BoolTy)
            | Some(TokenKind::IntTy)
            | Some(TokenKind::Int8Ty)
            | Some(TokenKind::Int16Ty)
            | Some(TokenKind::Int32Ty)
            | Some(TokenKind::Int64Ty)
            | Some(TokenKind::U8Ty)
            | Some(TokenKind::U16Ty)
            | Some(TokenKind::U32Ty)
            | Some(TokenKind::U64Ty)
            | Some(TokenKind::FloatTy)
            | Some(TokenKind::Float32Ty)
            | Some(TokenKind::Float64Ty)
            | Some(TokenKind::StringTy)
            | Some(TokenKind::BytesTy)
    )
}
