use crate::parser::Parser;
use ori_ast::common::{Attr, AttrArg, Visibility};
use ori_ast::item::{
    AbiLabel, AliasDecl, ApplyDecl, ApplyMember, ApplyUseSection, EnumDecl, EnumVariant,
    ExternBlock, ExternMember, FuncDecl, FuncSignature, ImportDecl, ImportItem, Item,
    ItemWithAttrs, NamedField, NamespaceDecl, NewtypeDecl, Param, ParamKind, SourceFile,
    StructDecl, StructField, TopConst, TopVar, TraitDecl, TraitMember,
};
use ori_diagnostics::Span;
use ori_lexer::TokenKind;
use std::collections::HashSet;

impl<'src> Parser<'src> {
    /// Entry point: parse a full source file.
    pub fn parse_source_file(&mut self) -> SourceFile {
        let start = self.current_span();
        let namespace = if self.at(&TokenKind::Module) {
            self.parse_module_header()
        } else if self.at(&TokenKind::Namespace) {
            // S3 cutover: `namespace` is a hard error; recover by parsing the path.
            let span = self.current_span();
            self.error(
                "parse.namespace_removed",
                "`namespace` was removed; use `module` for the file header",
                span,
            );
            self.parse_module_header_after_keyword()
        } else {
            self.error(
                "parse.module_missing",
                "source file must start with a `module` declaration",
                start,
            );
            None
        }
        .unwrap_or_else(|| NamespaceDecl {
            name: ori_ast::common::QualifiedName {
                parts: Vec::new(),
                span: Span::DUMMY,
            },
            span: Span::DUMMY,
        });
        let mut imports = Vec::new();
        while self.at(&TokenKind::Import)
            || (self.at(&TokenKind::Public) && self.peek_nth_kind(1) == Some(&TokenKind::Import))
            || self.at_contextual("imports")
        {
            if self.at_contextual("imports") {
                if let Some(batch) = self.parse_imports_block() {
                    imports.extend(batch);
                }
            } else if let Some(i) = self.parse_import() {
                imports.push(i);
            }
        }
        let mut items = Vec::new();
        while !self.at_eof() {
            let before = self.pos;
            if let Some(item) = self.parse_item_with_attrs() {
                items.push(item);
            } else {
                if self.pos == before {
                    self.advance();
                }
                self.synchronize(&[
                    TokenKind::Func,
                    TokenKind::Public,
                    TokenKind::Struct,
                    TokenKind::Enum,
                    TokenKind::Trait,
                    TokenKind::Apply,
                    TokenKind::Implement,
                    TokenKind::Alias,
                    TokenKind::Newtype,
                    TokenKind::Const,
                    TokenKind::Var,
                    TokenKind::Extern,
                    TokenKind::Mut,
                    TokenKind::Ident,
                    TokenKind::At,
                ]);
                if self.pos == before {
                    self.advance();
                }
            }
        }
        let end = self.tokens.last().map(|t| t.span).unwrap_or(start);
        SourceFile {
            namespace,
            imports,
            items,
            span: start.cover(end),
        }
    }

    /// `module path.to.name` — canonical S3 file header.
    fn parse_module_header(&mut self) -> Option<NamespaceDecl> {
        let start = self.expect(&TokenKind::Module)?;
        let name = self.parse_qualified_name()?;
        Some(NamespaceDecl {
            span: start.cover(name.span),
            name,
        })
    }

    /// Parse the path after a leading header keyword already diagnosed (or to be consumed).
    fn parse_module_header_after_keyword(&mut self) -> Option<NamespaceDecl> {
        let start = self.advance().unwrap().span; // `module` or removed `namespace`
        let name = self.parse_qualified_name()?;
        Some(NamespaceDecl {
            span: start.cover(name.span),
            name,
        })
    }

    /// Standalone `import` / `public import` statement (one form per line).
    fn parse_import(&mut self) -> Option<ImportDecl> {
        let start = self.current_span();
        let visibility = if self.eat(&TokenKind::Public) {
            Visibility::Public
        } else {
            Visibility::Private
        };
        self.expect(&TokenKind::Import)?;
        self.parse_import_body(start, visibility)
    }

    /// S3 block form:
    /// ```text
    /// imports
    ///   path (A, B), path = alias, path
    /// end
    /// ```
    /// Comma-separated multi-import is allowed only inside this block.
    fn parse_imports_block(&mut self) -> Option<Vec<ImportDecl>> {
        let start = self.current_span();
        if !self.at_contextual("imports") {
            return None;
        }
        self.advance(); // contextual `imports`
        let mut decls = Vec::new();
        while !self.at(&TokenKind::End) && !self.at_eof() {
            // Optional commas between entries (and trailing commas).
            while self.eat(&TokenKind::Comma) {}
            if self.at(&TokenKind::End) || self.at_eof() {
                break;
            }
            let item_start = self.current_span();
            let visibility = if self.eat(&TokenKind::Public) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            // Optional `import` keyword inside the block for familiarity.
            let _ = self.eat(&TokenKind::Import);
            let before = self.pos;
            if let Some(decl) = self.parse_import_body(item_start, visibility) {
                decls.push(decl);
            } else if self.pos == before {
                // Avoid infinite loop on unparseable junk.
                self.advance();
            }
        }
        let _ = self.expect_block_end(start, "imports");
        Some(decls)
    }

    /// Shared body after optional visibility / `import` keyword:
    /// `path`, `path = alias`, or `path (A, B [= alias])`.
    fn parse_import_body(&mut self, start: Span, visibility: Visibility) -> Option<ImportDecl> {
        let path = self.parse_qualified_name()?;
        let (alias, selected) = if self.at(&TokenKind::LParen) {
            (None, self.parse_import_selection()?)
        } else if self.eat(&TokenKind::Eq) {
            match self.parse_name() {
                Some(alias) => (Some(alias), Vec::new()),
                None => {
                    if !self.at_eof() && !is_import_alias_recovery_boundary(self.peek_kind()) {
                        self.advance();
                    }
                    return None;
                }
            }
        } else if self.at(&TokenKind::As) {
            // S3 cutover: `as` removed in favour of `path = alias`.
            let as_span = self.current_span();
            self.advance();
            self.error(
                "parse.import_as_removed",
                "`import path as alias` was removed; use `import path = alias`",
                as_span,
            );
            match self.parse_name() {
                Some(alias) => (Some(alias), Vec::new()),
                None => (None, Vec::new()),
            }
        } else if self.at_contextual("only") {
            // S3 cutover: `only` removed; selective form is `path (A, B)`.
            let only_span = self.current_span();
            self.advance();
            self.error(
                "parse.import_only_removed",
                "`import path only (…)` was removed; use `import path (…)`",
                only_span,
            );
            (None, self.parse_import_selection().unwrap_or_default())
        } else {
            // Whole-module import: full path only (no implicit last-segment alias).
            (None, Vec::new())
        };
        let end = selected
            .last()
            .map(|item| item.span)
            .or_else(|| alias.as_ref().map(|a| a.span))
            .unwrap_or(path.span);
        Some(ImportDecl {
            visibility,
            path,
            alias,
            selected,
            span: start.cover(end),
        })
    }

    fn parse_import_selection(&mut self) -> Option<Vec<ImportItem>> {
        self.expect(&TokenKind::LParen)?;
        let mut selected = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let name = self.parse_name()?;
            let alias = if self.eat(&TokenKind::Eq) {
                Some(self.parse_name()?)
            } else if self.at(&TokenKind::As) {
                let as_span = self.current_span();
                self.advance();
                self.error(
                    "parse.import_as_removed",
                    "selected import rename uses `name = alias`, not `as`",
                    as_span,
                );
                self.parse_name()
            } else {
                None
            };
            let end = alias.as_ref().map(|a| a.span).unwrap_or(name.span);
            selected.push(ImportItem {
                span: name.span.cover(end),
                name,
                alias,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen)?;
        Some(selected)
    }

    fn parse_item_with_attrs(&mut self) -> Option<ItemWithAttrs> {
        let mut attrs = Vec::new();
        while self.at(&TokenKind::At) {
            if let Some(attr) = self.parse_attr() {
                attrs.push(attr);
            }
        }
        let item = self.parse_item()?;
        Some(ItemWithAttrs { attrs, item })
    }

    fn parse_attr(&mut self) -> Option<Attr> {
        let start = self.advance().unwrap().span; // @
        let name = self.parse_name()?;
        let mut end = name.span;
        let args = if self.at(&TokenKind::LParen) {
            self.advance();
            let mut args = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                let span = self.current_span();
                if self.at(&TokenKind::Ident) && self.peek_nth_kind(1) == Some(&TokenKind::Colon) {
                    let key = self.parse_name()?;
                    self.expect(&TokenKind::Colon)?;
                    let value = self.parse_name()?;
                    args.push(AttrArg::Named { key, value });
                } else if self.at(&TokenKind::StrLit) {
                    let tok = self.advance().unwrap();
                    let raw = self.slice(tok.span);
                    args.push(AttrArg::String(raw[1..raw.len() - 1].into(), span));
                } else {
                    break;
                }
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            let rparen = self.expect(&TokenKind::RParen)?;
            end = rparen;
            args
        } else {
            Vec::new()
        };
        Some(Attr {
            name,
            args,
            span: start.cover(end),
        })
    }

    fn parse_visibility(&mut self) -> Visibility {
        if self.eat(&TokenKind::Public) {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn parse_item(&mut self) -> Option<Item> {
        if self.at(&TokenKind::Module) {
            let span = self.current_span();
            self.error(
                "parse.module_not_first",
                "`module` must be the first declaration in a file",
                span,
            );
            let _ = self.parse_module_header();
            return None;
        }
        if self.at(&TokenKind::Namespace) {
            let span = self.current_span();
            self.error(
                "parse.namespace_removed",
                "`namespace` was removed; use `module` for the file header",
                span,
            );
            let _ = self.parse_module_header_after_keyword();
            return None;
        }

        if self.at(&TokenKind::Import)
            || (self.at(&TokenKind::Public) && self.peek_nth_kind(1) == Some(&TokenKind::Import))
            || self.at_contextual("imports")
        {
            let span = self.current_span();
            self.error(
                "parse.import_after_declaration",
                "imports must appear before declarations",
                span,
            );
            if self.at_contextual("imports") {
                let _ = self.parse_imports_block();
            } else {
                let _ = self.parse_import();
            }
            return None;
        }

        let vis = self.parse_visibility();
        if self.at_func_decl_start() {
            return Some(Item::Func(self.parse_func_decl(vis)?));
        }
        match self.peek_kind()? {
            TokenKind::Struct => Some(Item::Struct(self.parse_struct_decl(vis)?)),
            TokenKind::Enum => Some(Item::Enum(self.parse_enum_decl(vis)?)),
            TokenKind::Trait => Some(Item::Trait(self.parse_trait_decl(vis)?)),
            TokenKind::Apply => Some(Item::Apply(self.parse_apply_decl()?)),
            TokenKind::Implement => {
                // S3 cutover: `implement Trait for Type` is a hard error.
                let span = self.current_span();
                self.error(
                    "parse.implement_removed",
                    "`implement Trait for Type` was removed; use `apply Type` with `use Trait`",
                    span,
                );
                // Recover by consuming the old form into an apply AST when possible.
                Some(Item::Apply(self.parse_legacy_implement_as_apply()?))
            }
            TokenKind::Alias => Some(Item::Alias(self.parse_alias_decl(vis)?)),
            TokenKind::Newtype => Some(Item::Newtype(self.parse_newtype_decl(vis)?)),
            TokenKind::Const => Some(Item::Const(self.parse_top_const(vis)?)),
            TokenKind::Var => Some(Item::Var(self.parse_top_var(vis)?)),
            TokenKind::Extern => Some(Item::Extern(self.parse_extern_block()?)),
            _ => {
                let span = self.current_span();
                self.error("parse.expected_declaration", "expected a declaration", span);
                None
            }
        }
    }

    /// Whether the next tokens start a function declaration (S3: no `func` keyword).
    ///
    /// Forms: `[async] [mut] name(…)` / `name[T](…)` / `name for T: Trait (…)` /
    /// legacy `name<T>(…)` (errors) / legacy `func …`.
    fn at_func_decl_start(&self) -> bool {
        if self.at(&TokenKind::Func) || self.at(&TokenKind::Mut) || self.at_contextual("async") {
            return true;
        }
        self.at_named_func_head()
    }

    /// `name(` / `name[` / `name for` / legacy `name<` — function/method head without modifiers.
    fn at_named_func_head(&self) -> bool {
        if !self.at(&TokenKind::Ident) {
            return false;
        }
        matches!(
            self.peek_nth_kind(1),
            Some(TokenKind::LParen)
                | Some(TokenKind::LBracket)
                | Some(TokenKind::For)
                | Some(TokenKind::Lt)
        )
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    pub fn parse_func_decl(&mut self, vis: Visibility) -> Option<FuncDecl> {
        let start = self.current_span();
        let (is_async, is_mut) = self.parse_func_modifiers();
        self.reject_func_keyword_on_decl();
        let name = self.parse_name()?;
        let (type_params, where_clause) = self.parse_generic_header();
        let params = self.parse_param_list()?;
        let return_ty = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        // Legacy trailing `where` (after return type) is rejected but recovered.
        let where_clause = where_clause.or_else(|| self.parse_where_clause_opt());
        // S3: single-expression body `name(params) -> T => expr` (no `end`).
        if self.eat(&TokenKind::FatArrow) {
            let expr = self.parse_expr()?;
            let expr_span = expr.span();
            let body = ori_ast::stmt::Block {
                stmts: vec![ori_ast::stmt::Stmt::Return(ori_ast::stmt::ReturnStmt {
                    value: Some(Box::new(expr)),
                    span: expr_span,
                })],
                span: expr_span,
            };
            return Some(FuncDecl {
                visibility: vis,
                is_async,
                is_mut,
                name,
                type_params,
                params,
                return_ty,
                where_clause,
                body,
                span: start.cover(expr_span),
            });
        }
        let body = self.parse_block()?;
        let end = self.expect_block_end(start, "function")?;
        Some(FuncDecl {
            visibility: vis,
            is_async,
            is_mut,
            name,
            type_params,
            params,
            return_ty,
            where_clause,
            body,
            span: start.cover(end),
        })
    }

    fn parse_func_signature(&mut self, vis: Visibility) -> Option<FuncSignature> {
        let start = self.current_span();
        let (is_async, is_mut) = self.parse_func_modifiers();
        self.reject_func_keyword_on_decl();
        let name = self.parse_name()?;
        let (type_params, where_clause) = self.parse_generic_header();
        let params = self.parse_param_list()?;
        let return_ty = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let where_clause = where_clause.or_else(|| self.parse_where_clause_opt());
        let end = return_ty.as_ref().map(|t| t.span()).unwrap_or(name.span);
        Some(FuncSignature {
            visibility: vis,
            is_async,
            is_mut,
            name,
            type_params,
            params,
            return_ty,
            where_clause,
            span: start.cover(end),
        })
    }

    /// `[T]` type params + optional `for T: Trait` bounds (S3 / Auk9-style).
    fn parse_generic_header(
        &mut self,
    ) -> (
        Vec<ori_ast::common::TypeParam>,
        Option<ori_ast::common::WhereClause>,
    ) {
        let mut type_params = self.parse_type_params_opt();
        let where_clause = self.parse_for_bounds_opt(&mut type_params);
        (type_params, where_clause)
    }

    /// S3: declaration form is `name(...)` — `func` on a declaration is a hard error.
    /// Still consumed so recovery can parse the remainder of the signature.
    fn reject_func_keyword_on_decl(&mut self) {
        if self.at(&TokenKind::Func) {
            let span = self.advance().unwrap().span;
            self.error(
                "parse.func_removed",
                "`func` was removed from declarations; write `name(...)` directly",
                span,
            );
        }
    }

    fn parse_param_list(&mut self) -> Option<Vec<Param>> {
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            params.push(self.parse_param()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen)?;
        self.validate_param_list(&params);
        Some(params)
    }

    fn parse_func_modifiers(&mut self) -> (bool, bool) {
        let mut is_async = false;
        let mut is_mut = false;
        for _ in 0..2 {
            if !is_async && self.eat_contextual("async") {
                is_async = true;
                continue;
            }
            if !is_mut && self.eat(&TokenKind::Mut) {
                is_mut = true;
                continue;
            }
            break;
        }
        (is_async, is_mut)
    }

    fn parse_param(&mut self) -> Option<Param> {
        let start = self.current_span();
        // `self` parameter
        if self.at(&TokenKind::SelfKw) {
            let tok = self.advance().unwrap();
            let name = ori_ast::common::Name::new("self", tok.span);
            return Some(Param {
                name,
                ty: ori_ast::ty::Type::Named(ori_ast::common::QualifiedName::single(
                    ori_ast::common::Name::new("Self", tok.span),
                )),
                kind: ParamKind::Required,
                span: tok.span,
            });
        }
        let name = self.parse_name()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        // Variadic: `name: Type...`.
        //
        // `Type..` is accepted for compatibility with early fixtures, but new
        // code should use the documented `Type...` form.
        if self.eat(&TokenKind::Ellipsis) || self.eat(&TokenKind::DotDot) {
            let span = start.cover(self.current_span());
            return Some(Param {
                name,
                ty,
                kind: ParamKind::Variadic,
                span,
            });
        }
        // Default: `name: Type = expr`
        let default = if self.eat(&TokenKind::Eq) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        // Contract: `name: Type if it > 0`
        let contract = if self.eat(&TokenKind::If) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let kind = match (default, contract) {
            (None, None) => ParamKind::Required,
            (Some(d), None) => ParamKind::Default(d),
            (None, Some(c)) => ParamKind::Contract(c),
            (Some(d), Some(c)) => ParamKind::DefaultAndContract(d, c),
        };
        let span = start.cover(self.current_span());
        Some(Param {
            name,
            ty,
            kind,
            span,
        })
    }

    fn validate_param_list(&mut self, params: &[Param]) {
        let mut seen_default = false;
        let mut seen_names = HashSet::new();
        for (index, param) in params.iter().enumerate() {
            if !seen_names.insert(param.name.text.clone()) {
                self.error(
                    "bind.duplicate_param",
                    format!("duplicate parameter `{}`", param.name.text),
                    param.name.span,
                );
            }

            if matches!(param.kind, ParamKind::Variadic) && index + 1 != params.len() {
                self.error(
                    "parse.variadic_not_last",
                    "variadic parameter must be the last parameter",
                    param.span,
                );
            }

            if seen_default && param_is_required(&param.kind) {
                self.error(
                    "parse.default_before_required",
                    "required parameter cannot follow a default parameter",
                    param.span,
                );
            }

            if param_has_default(&param.kind) {
                seen_default = true;
            }
        }
    }

    // ── Structs ───────────────────────────────────────────────────────────────

    fn parse_struct_decl(&mut self, vis: Visibility) -> Option<StructDecl> {
        let start = self.advance().unwrap().span; // struct
        let name = self.parse_name()?;
        let (type_params, where_clause) = self.parse_generic_header();
        // Legacy trailing `where` still recovered (with error).
        let where_clause = where_clause.or_else(|| self.parse_where_clause_opt());
        let mut fields = Vec::new();
        let mut field_names = HashSet::new();
        let mut methods = Vec::new();
        // Fields: ident : type [if expr]  (not `ident(` / `ident[` methods)
        while self.at_struct_field_start() {
            let field = self.parse_struct_field()?;
            if !field_names.insert(field.name.text.clone()) {
                self.error(
                    "bind.duplicate_field",
                    format!("duplicate struct field `{}`", field.name.text),
                    field.name.span,
                );
            }
            fields.push(field);
        }
        // Methods: [public] [async] [mut] name(…) …
        while !self.at(&TokenKind::End) && !self.at_eof() {
            let mvis = self.parse_visibility();
            methods.push(self.parse_func_decl(mvis)?);
        }
        let end = self.expect_block_end(start, "struct")?;
        Some(StructDecl {
            visibility: vis,
            name,
            type_params,
            where_clause,
            fields,
            methods,
            span: start.cover(end),
        })
    }

    fn parse_struct_field(&mut self) -> Option<StructField> {
        let start = self.current_span();
        let name = self.parse_name()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        let contract = if self.eat(&TokenKind::If) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let span = start.cover(self.current_span());
        Some(StructField {
            name,
            ty,
            contract,
            span,
        })
    }

    /// Field heads are `name:`; method heads are `name(` / `name<` (S3, no `func`).
    fn at_struct_field_start(&self) -> bool {
        self.at(&TokenKind::Ident) && self.peek_nth_kind(1) == Some(&TokenKind::Colon)
    }

    // ── Enums ─────────────────────────────────────────────────────────────────

    fn parse_enum_decl(&mut self, vis: Visibility) -> Option<EnumDecl> {
        let start = self.advance().unwrap().span; // enum
        let name = self.parse_name()?;
        let type_params = self.parse_type_params_opt();
        let mut variants = Vec::new();
        let mut variant_names = HashSet::new();
        while !self.at(&TokenKind::End) && !self.at_eof() {
            let variant = self.parse_enum_variant()?;
            if !variant_names.insert(variant.name.text.clone()) {
                self.error(
                    "bind.duplicate_variant",
                    format!("duplicate enum variant `{}`", variant.name.text),
                    variant.name.span,
                );
            }
            variants.push(variant);
        }
        let end = self.expect_block_end(start, "enum")?;
        Some(EnumDecl {
            visibility: vis,
            name,
            type_params,
            variants,
            span: start.cover(end),
        })
    }

    fn parse_enum_variant(&mut self) -> Option<EnumVariant> {
        let start = self.current_span();
        let name = self.parse_name()?;
        let fields = if self.at(&TokenKind::LParen) {
            self.advance();
            let mut fs = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                let n = self.parse_name()?;
                self.expect(&TokenKind::Colon)?;
                let ty = self.parse_type()?;
                let sp = n.span.cover(ty.span());
                fs.push(NamedField {
                    name: n,
                    ty,
                    span: sp,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RParen)?;
            fs
        } else {
            Vec::new()
        };
        let end = self.current_span();
        Some(EnumVariant {
            name,
            fields,
            span: start.cover(end),
        })
    }

    // ── Traits ────────────────────────────────────────────────────────────────

    fn parse_trait_decl(&mut self, vis: Visibility) -> Option<TraitDecl> {
        let start = self.advance().unwrap().span; // trait
        let name = self.parse_name()?;
        let (type_params, where_clause) = self.parse_generic_header();
        let where_clause = where_clause.or_else(|| self.parse_where_clause_opt());
        let mut members = Vec::new();
        while !self.at(&TokenKind::End) && !self.at_eof() {
            let associated_type = self
                .peek()
                .is_some_and(|tok| tok.kind == TokenKind::Ident && self.slice(tok.span) == "type");
            if associated_type {
                self.advance(); // consume "type"
                let name = self.parse_name()?;
                members.push(TraitMember::Type(name));
                continue;
            }
            let mvis = self.parse_visibility();
            let is_mut = self.at(&TokenKind::Mut);
            let sig = self.parse_func_signature(mvis)?;
            // S3: single-expression default `name(params) -> T => expr` (no method `end`).
            if self.eat(&TokenKind::FatArrow) && !is_mut {
                let expr = self.parse_expr()?;
                let expr_span = expr.span();
                let body = ori_ast::stmt::Block {
                    stmts: vec![ori_ast::stmt::Stmt::Return(ori_ast::stmt::ReturnStmt {
                        value: Some(Box::new(expr)),
                        span: expr_span,
                    })],
                    span: expr_span,
                };
                let decl = FuncDecl {
                    visibility: sig.visibility,
                    is_async: sig.is_async,
                    is_mut: sig.is_mut,
                    name: sig.name.clone(),
                    type_params: sig.type_params.clone(),
                    params: sig.params.clone(),
                    return_ty: sig.return_ty.clone(),
                    where_clause: sig.where_clause.clone(),
                    body,
                    span: sig.span.cover(expr_span),
                };
                members.push(TraitMember::Default(decl));
                continue;
            }
            // Required methods are followed by another member or `end`.
            // Default methods have a body (statement / expression) before `end`.
            // Bare call statements like `print("hi")` must not look like the next method.
            let has_body = !self.at_trait_member_start() && !self.at(&TokenKind::End);
            if has_body && !is_mut {
                let body = self.parse_block()?;
                // Label matches free/struct methods: `end function`.
                let end = self.expect_block_end(sig.span, "function")?;
                let decl = FuncDecl {
                    visibility: sig.visibility,
                    is_async: sig.is_async,
                    is_mut: sig.is_mut,
                    name: sig.name.clone(),
                    type_params: sig.type_params.clone(),
                    params: sig.params.clone(),
                    return_ty: sig.return_ty.clone(),
                    where_clause: sig.where_clause.clone(),
                    body,
                    span: sig.span.cover(end),
                };
                members.push(TraitMember::Default(decl));
            } else {
                members.push(TraitMember::Required(sig));
            }
        }
        let end = self.expect_block_end(start, "trait")?;
        Some(TraitDecl {
            visibility: vis,
            name,
            type_params,
            where_clause,
            members,
            span: start.cover(end),
        })
    }

    /// Start of the next trait member after a method signature (S3, no `func`).
    fn at_trait_member_start(&self) -> bool {
        if self.at_any(&[
            TokenKind::Func,
            TokenKind::Public,
            TokenKind::Mut,
            TokenKind::End,
        ]) || self.at_contextual("async")
        {
            return true;
        }
        // associated type: `type Name`
        if self
            .peek()
            .is_some_and(|tok| tok.kind == TokenKind::Ident && self.slice(tok.span) == "type")
        {
            return true;
        }
        // Next method head only when the form is a real signature, not a bare call.
        self.looks_like_trait_method_head_at(self.pos)
    }

    /// Non-mutating lookahead: is the token sequence at `start` a trait method head?
    ///
    /// Distinguishes method signatures (`area(self)`, `draw()`, `paint() -> void`)
    /// from body statements that begin with a bare *call* `say("hi")`.
    ///
    /// Param list must be signature-shaped: empty, `self`, or `name: Type`.
    /// Call arguments (literals, bare idents without `:`) → not a method head.
    ///
    /// Empty `foo()` is treated as a method head (required or default). An empty
    /// call used as the first body statement is an accepted residual edge case;
    /// calls with arguments are the important case (Issue 1).
    fn looks_like_trait_method_head_at(&self, start: usize) -> bool {
        let mut i = self.skip_trivia_after(start);
        if self.kind_at(i) == Some(&TokenKind::Func) {
            i = self.skip_trivia_after(i + 1);
        }
        if self.kind_at(i) != Some(&TokenKind::Ident) {
            return false;
        }
        i = self.skip_trivia_after(i + 1);
        // Optional type params: `[T]` (S3) or legacy `<T>` (error path still present).
        if self.kind_at(i) == Some(&TokenKind::LBracket) {
            match self.skip_balanced(i, TokenKind::LBracket, TokenKind::RBracket) {
                Some(after) => i = after,
                None => return false,
            }
        } else if self.kind_at(i) == Some(&TokenKind::Lt) {
            match self.skip_balanced(i, TokenKind::Lt, TokenKind::Gt) {
                Some(after) => i = after,
                None => return false,
            }
        }
        // Optional `for T: Trait, …` bounds.
        if self.kind_at(i) == Some(&TokenKind::For) {
            match self.skip_for_bounds_at(i) {
                Some(after) => i = after,
                None => return false,
            }
        }
        if self.kind_at(i) != Some(&TokenKind::LParen) {
            return false;
        }
        self.scan_trait_param_list_shape(i).is_some()
    }

    /// Skip `for T: Trait, U: Other` starting at `for`. Returns index after the bounds.
    fn skip_for_bounds_at(&self, start: usize) -> Option<usize> {
        let mut i = self.skip_trivia_after(start);
        if self.kind_at(i) != Some(&TokenKind::For) {
            return None;
        }
        i = self.skip_trivia_after(i + 1);
        loop {
            if self.kind_at(i) != Some(&TokenKind::Ident) {
                return None;
            }
            i = self.skip_trivia_after(i + 1);
            if self.kind_at(i) != Some(&TokenKind::Colon) {
                return None;
            }
            i = self.skip_trivia_after(i + 1);
            if self.kind_at(i) == Some(&TokenKind::Not) {
                i = self.skip_trivia_after(i + 1);
            }
            // Trait name (possibly qualified).
            if self.kind_at(i) != Some(&TokenKind::Ident) {
                return None;
            }
            i = self.skip_trivia_after(i + 1);
            while self.kind_at(i) == Some(&TokenKind::Dot) {
                i = self.skip_trivia_after(i + 1);
                if self.kind_at(i) != Some(&TokenKind::Ident) {
                    return None;
                }
                i = self.skip_trivia_after(i + 1);
            }
            if self.kind_at(i) == Some(&TokenKind::Comma) {
                i = self.skip_trivia_after(i + 1);
                continue;
            }
            return Some(i);
        }
    }

    fn kind_at(&self, index: usize) -> Option<&TokenKind> {
        self.tokens.get(index).map(|t| &t.kind)
    }

    fn skip_trivia_after(&self, mut index: usize) -> usize {
        while index < self.tokens.len() && self.tokens[index].is_trivia() {
            index += 1;
        }
        index
    }

    fn skip_balanced(&self, open_index: usize, open: TokenKind, close: TokenKind) -> Option<usize> {
        if self.kind_at(open_index) != Some(&open) {
            return None;
        }
        let mut depth = 0usize;
        let mut i = open_index;
        while i < self.tokens.len() {
            let kind = &self.tokens[i].kind;
            if *kind == open {
                depth += 1;
            } else if *kind == close {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(self.skip_trivia_after(i + 1));
                }
            }
            i += 1;
        }
        None
    }

    /// Returns `(index_after_rparen, empty_params)` when params look like a signature.
    /// Call-shaped args yield `None`.
    fn scan_trait_param_list_shape(&self, lparen_index: usize) -> Option<(usize, bool)> {
        if self.kind_at(lparen_index) != Some(&TokenKind::LParen) {
            return None;
        }
        let mut i = self.skip_trivia_after(lparen_index + 1);
        if self.kind_at(i) == Some(&TokenKind::RParen) {
            return Some((self.skip_trivia_after(i + 1), true));
        }
        loop {
            if self.kind_at(i) == Some(&TokenKind::SelfKw) {
                i = self.skip_trivia_after(i + 1);
            } else if self.kind_at(i) == Some(&TokenKind::Ident) {
                i = self.skip_trivia_after(i + 1);
                if self.kind_at(i) != Some(&TokenKind::Colon) {
                    return None;
                }
                i = self.skip_trivia_after(i + 1);
                i = self.skip_rough_type(i);
                if matches!(
                    self.kind_at(i),
                    Some(TokenKind::Ellipsis) | Some(TokenKind::DotDot)
                ) {
                    i = self.skip_trivia_after(i + 1);
                }
                if self.kind_at(i) == Some(&TokenKind::Eq) {
                    i = self.skip_trivia_after(i + 1);
                    i = self.skip_until_param_separator(i);
                }
                if self.kind_at(i) == Some(&TokenKind::If) {
                    i = self.skip_trivia_after(i + 1);
                    i = self.skip_until_param_separator(i);
                }
            } else {
                return None;
            }

            if self.kind_at(i) == Some(&TokenKind::Comma) {
                i = self.skip_trivia_after(i + 1);
                continue;
            }
            if self.kind_at(i) == Some(&TokenKind::RParen) {
                return Some((self.skip_trivia_after(i + 1), false));
            }
            return None;
        }
    }

    fn skip_rough_type(&self, mut i: usize) -> usize {
        if self.kind_at(i) == Some(&TokenKind::Func) {
            i = self.skip_trivia_after(i + 1);
            if self.kind_at(i) == Some(&TokenKind::LParen) {
                if let Some(after) = self.skip_balanced(i, TokenKind::LParen, TokenKind::RParen) {
                    i = after;
                }
            }
            if self.kind_at(i) == Some(&TokenKind::Arrow) {
                i = self.skip_trivia_after(i + 1);
                i = self.skip_rough_type(i);
            }
            return i;
        }
        while matches!(
            self.kind_at(i),
            Some(TokenKind::Ident)
                | Some(TokenKind::Optional)
                | Some(TokenKind::ResultKw)
                | Some(TokenKind::List)
                | Some(TokenKind::Map)
                | Some(TokenKind::Set)
                | Some(TokenKind::Range)
                | Some(TokenKind::Void)
                | Some(TokenKind::Tuple)
                | Some(TokenKind::Lazy)
                | Some(TokenKind::Any)
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
                | Some(TokenKind::SelfKw)
        ) {
            i = self.skip_trivia_after(i + 1);
            if self.kind_at(i) == Some(&TokenKind::Dot) {
                i = self.skip_trivia_after(i + 1);
                continue;
            }
            break;
        }
        if self.kind_at(i) == Some(&TokenKind::LBracket) {
            if let Some(after) = self.skip_balanced(i, TokenKind::LBracket, TokenKind::RBracket) {
                i = after;
            }
        } else if self.kind_at(i) == Some(&TokenKind::Lt) {
            if let Some(after) = self.skip_balanced(i, TokenKind::Lt, TokenKind::Gt) {
                i = after;
            }
        } else if self.kind_at(i) == Some(&TokenKind::LParen) {
            if let Some(after) = self.skip_balanced(i, TokenKind::LParen, TokenKind::RParen) {
                i = after;
            }
        }
        while self.kind_at(i) == Some(&TokenKind::Ident)
            && self.tokens.get(i).is_some_and(|tok| {
                let s = self.slice(tok.span);
                s == "of" || s == "to"
            })
        {
            i = self.skip_trivia_after(i + 1);
            i = self.skip_rough_type(i);
        }
        i
    }

    fn skip_until_param_separator(&self, mut i: usize) -> usize {
        let mut depth = 0i32;
        while i < self.tokens.len() {
            match self.kind_at(i) {
                Some(TokenKind::LParen) | Some(TokenKind::Lt) | Some(TokenKind::LBracket) => {
                    depth += 1;
                }
                Some(TokenKind::RParen) | Some(TokenKind::Gt) | Some(TokenKind::RBracket) => {
                    if depth == 0 {
                        return i;
                    }
                    depth -= 1;
                }
                Some(TokenKind::Comma) if depth == 0 => return i,
                _ => {}
            }
            i += 1;
            // Do not skip trivia via skip_trivia_after mid-loop (would skip commas); only advance one.
            while i < self.tokens.len() && self.tokens[i].is_trivia() {
                i += 1;
            }
        }
        i
    }

    // ── Apply (S3: `apply Type` + `use Trait` + bind) ─────────────────────────

    /// Canonical S3 form: `apply [T] Type … end`.
    ///
    /// Old form `apply Trait to Type` is a hard error (recovered into a single
    /// `use` section so later stages still see a usable AST).
    fn parse_apply_decl(&mut self) -> Option<ApplyDecl> {
        let start = self.advance().unwrap().span; // apply
        let type_params = self.parse_type_params_opt();
        let first_name = self.parse_qualified_name()?;

        // Removed form: `apply Trait to Type` (also `apply Trait for Type`).
        if self.at_contextual("to") || self.at(&TokenKind::For) {
            let connector_span = self.current_span();
            let connector = if self.at(&TokenKind::For) {
                "for"
            } else {
                "to"
            };
            self.advance(); // consume `to` / `for`
            self.error(
                "parse.apply_trait_to_removed",
                format!(
                    "`apply Trait {connector} Type` was removed; use `apply Type` with `use Trait`"
                ),
                start.cover(connector_span),
            );
            let for_type = self.parse_qualified_name()?;
            let where_clause = self.parse_where_clause_opt();
            let (members, associated_types) = self.parse_apply_use_body_members()?;
            let end = self.expect_block_end(start, "apply")?;
            let use_span = first_name.span.cover(end);
            return Some(ApplyDecl {
                type_params,
                for_type,
                where_clause,
                free_members: Vec::new(),
                uses: vec![ApplyUseSection {
                    trait_name: first_name,
                    members,
                    associated_types,
                    span: use_span,
                }],
                span: start.cover(end),
            });
        }

        let for_type = first_name;
        let where_clause = self.parse_where_clause_opt();

        // Compact header: `apply Type use Trait … end`.
        //
        // One trait and nothing else is the common case, and the nested form
        // costs a whole indentation level to say nothing extra. Which form
        // applies is decided by the content, never by the writer — see the
        // `apply.redundant_use_block` check for the other half of the rule.
        //
        // Newlines are not tokens, so "same line" has to be read off the
        // source text: `use` on the next line is the nested form.
        if self.at(&TokenKind::Use) && self.same_line_as_span_end(for_type.span) {
            let use_start = self.current_span();
            self.advance(); // use
            let trait_name = self.parse_qualified_name()?;
            let (members, associated_types) = self.parse_apply_use_body_members()?;
            let end = self.expect_block_end(start, "apply")?;
            return Some(ApplyDecl {
                type_params,
                for_type,
                where_clause,
                free_members: Vec::new(),
                uses: vec![ApplyUseSection {
                    trait_name,
                    members,
                    associated_types,
                    span: use_start.cover(end),
                }],
                span: start.cover(end),
            });
        }

        let mut free_members = Vec::new();
        let mut uses = Vec::new();
        let mut seen_use = false;

        while !self.at(&TokenKind::End) && !self.at_eof() {
            if self.at(&TokenKind::Use) {
                seen_use = true;
                uses.push(self.parse_apply_use_section()?);
                continue;
            }

            // Free member (method or bind). After the first `use`, free members
            // are illegal (fixed Auk9 order: free → use sections).
            if seen_use {
                let span = self.current_span();
                self.error(
                    "parse.apply_member_after_use",
                    "free methods and binds must appear before `use Trait` sections",
                    span,
                );
            }

            if self.at_apply_bind_start() {
                free_members.push(self.parse_apply_bind()?);
            } else {
                // Associated `type Name = Ty` only valid inside `use`.
                let is_assoc = self.peek().is_some_and(|tok| {
                    tok.kind == TokenKind::Ident && self.slice(tok.span) == "type"
                });
                if is_assoc {
                    let span = self.current_span();
                    self.error(
                        "parse.unexpected_token",
                        "associated types belong inside a `use Trait` section",
                        span,
                    );
                    self.advance(); // type
                    let _ = self.parse_name();
                    let _ = self.eat(&TokenKind::Eq);
                    let _ = self.parse_type();
                    continue;
                }
                let mvis = self.parse_visibility();
                free_members.push(ApplyMember::Method(self.parse_func_decl(mvis)?));
            }
        }

        let end = self.expect_block_end(start, "apply")?;

        // The other half of the "one canonical form" rule: a nested block that
        // holds a single `use` and nothing else says exactly what the compact
        // header says, one level deeper. Reject it so both spellings never
        // coexist for the same content.
        if uses.len() == 1 && free_members.is_empty() {
            let section = &uses[0];
            self.error(
                "apply.redundant_use_block",
                format!(
                    "`apply {}` with only `use {}` should use the compact header: \
                     write `apply {} use {}`",
                    for_type, section.trait_name, for_type, section.trait_name
                ),
                start.cover(section.trait_name.span),
            );
        }

        Some(ApplyDecl {
            type_params,
            for_type,
            where_clause,
            free_members,
            uses,
            span: start.cover(end),
        })
    }

    /// Recovery path for removed `implement Trait for Type`.
    fn parse_legacy_implement_as_apply(&mut self) -> Option<ApplyDecl> {
        let start = self.advance().unwrap().span; // implement
        let type_params = self.parse_type_params_opt();
        let trait_name = self.parse_qualified_name()?;
        if self.at(&TokenKind::For) {
            self.advance();
        } else if self.at_contextual("to") {
            self.advance();
        } else {
            self.error(
                "parse.unexpected_token",
                "expected `for` after the trait name in a legacy `implement` declaration",
                self.current_span(),
            );
        }
        let for_type = self.parse_qualified_name()?;
        let where_clause = self.parse_where_clause_opt();
        let (members, associated_types) = self.parse_apply_use_body_members()?;
        let end = self.expect_block_end(start, "implement")?;
        let use_span = trait_name.span.cover(end);
        Some(ApplyDecl {
            type_params,
            for_type,
            where_clause,
            free_members: Vec::new(),
            uses: vec![ApplyUseSection {
                trait_name,
                members,
                associated_types,
                span: use_span,
            }],
            span: start.cover(end),
        })
    }

    fn parse_apply_use_section(&mut self) -> Option<ApplyUseSection> {
        let start = self.advance().unwrap().span; // use
        let trait_name = self.parse_qualified_name()?;
        let (members, associated_types) = self.parse_apply_use_body_members()?;
        let end = self.expect_block_end(start, "use")?;
        Some(ApplyUseSection {
            trait_name,
            members,
            associated_types,
            span: start.cover(end),
        })
    }

    /// Body of a `use Trait` (or recovered legacy implement): methods, binds, assoc types.
    fn parse_apply_use_body_members(
        &mut self,
    ) -> Option<(
        Vec<ApplyMember>,
        Vec<(ori_ast::common::Name, ori_ast::ty::Type)>,
    )> {
        let mut members = Vec::new();
        let mut associated_types = Vec::new();
        while !self.at(&TokenKind::End) && !self.at(&TokenKind::Use) && !self.at_eof() {
            let is_assoc = self
                .peek()
                .is_some_and(|tok| tok.kind == TokenKind::Ident && self.slice(tok.span) == "type");
            if is_assoc {
                self.advance(); // type
                let name = self.parse_name()?;
                self.expect(&TokenKind::Eq)?;
                let ty = self.parse_type()?;
                associated_types.push((name, ty));
                continue;
            }
            if self.at_apply_bind_start() {
                members.push(self.parse_apply_bind()?);
            } else {
                let mvis = self.parse_visibility();
                members.push(ApplyMember::Method(self.parse_func_decl(mvis)?));
            }
        }
        Some((members, associated_types))
    }

    /// `slot = freeFunction` (both identifiers, no type params / parens).
    fn at_apply_bind_start(&self) -> bool {
        let mut i = self.skip_trivia_after(self.pos);
        if self.kind_at(i) != Some(&TokenKind::Ident) {
            return false;
        }
        i = self.skip_trivia_after(i + 1);
        self.kind_at(i) == Some(&TokenKind::Eq)
    }

    fn parse_apply_bind(&mut self) -> Option<ApplyMember> {
        let slot = self.parse_name()?;
        self.expect(&TokenKind::Eq)?;
        let target = self.parse_name()?;
        let span = slot.span.cover(target.span);
        Some(ApplyMember::Bind { slot, target, span })
    }

    // ── Alias / Const / Var ───────────────────────────────────────────────────

    fn parse_alias_decl(&mut self, vis: Visibility) -> Option<AliasDecl> {
        let start = self.advance().unwrap().span; // alias
        let name = self.parse_name()?;
        let type_params = self.parse_type_params_opt();
        self.expect(&TokenKind::Eq)?;
        let ty = self.parse_type()?;
        let span = start.cover(ty.span());
        Some(AliasDecl {
            visibility: vis,
            name,
            type_params,
            ty,
            span,
        })
    }

    /// `newtype Name = Repr` — a distinct type over an existing representation.
    ///
    /// No type parameters for now: the motivating use is naming scalar domain
    /// values (`newtype UserId = int`), and generics can be added later
    /// without changing this form.
    fn parse_newtype_decl(&mut self, vis: Visibility) -> Option<NewtypeDecl> {
        let start = self.advance().unwrap().span; // newtype
        let name = self.parse_name()?;
        self.expect(&TokenKind::Eq)?;
        let repr = self.parse_type()?;
        let span = start.cover(repr.span());
        Some(NewtypeDecl {
            visibility: vis,
            name,
            repr,
            span,
        })
    }

    fn parse_top_const(&mut self, vis: Visibility) -> Option<TopConst> {
        let start = self.advance().unwrap().span; // const
        let name = self.parse_name()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        let span = start.cover(value.span());
        Some(TopConst {
            visibility: vis,
            name,
            ty,
            value: Box::new(value),
            span,
        })
    }

    fn parse_top_var(&mut self, vis: Visibility) -> Option<TopVar> {
        let start = self.advance().unwrap().span; // var
        let name = self.parse_name()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        let span = start.cover(value.span());
        Some(TopVar {
            visibility: vis,
            name,
            ty,
            value: Box::new(value),
            span,
        })
    }

    // ── Extern ────────────────────────────────────────────────────────────────

    fn parse_extern_block(&mut self) -> Option<ExternBlock> {
        let start = self.advance().unwrap().span; // extern
        let abi = if self.at(&TokenKind::Ident) {
            let tok = self.peek().unwrap();
            match self.slice(tok.span) {
                "c" | "C" => {
                    self.advance();
                    AbiLabel::C
                }
                "host" => {
                    self.advance();
                    AbiLabel::Host
                }
                abi => {
                    self.error(
                        "extern.unknown_abi",
                        format!("unknown extern ABI `{abi}`"),
                        tok.span,
                    );
                    self.advance();
                    AbiLabel::C
                }
            }
        } else {
            AbiLabel::C
        };
        let mut members = Vec::new();
        while !self.at(&TokenKind::End) && !self.at_eof() {
            let mvis = self.parse_visibility();
            members.push(self.parse_extern_member(mvis)?);
        }
        let end = self.expect_block_end(start, "extern")?;
        Some(ExternBlock {
            abi,
            members,
            span: start.cover(end),
        })
    }

    fn parse_extern_member(&mut self, vis: Visibility) -> Option<ExternMember> {
        let start = self.current_span();
        if self.at(&TokenKind::Var) {
            self.advance();
            let name = self.parse_name()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let span = start.cover(ty.span());
            Some(ExternMember::Var {
                visibility: vis,
                name,
                ty,
                span,
            })
        } else if self.at(&TokenKind::Func) || self.at_named_func_head() {
            // S3: `name(...)` — legacy `func name(...)` is accepted with an error.
            self.reject_func_keyword_on_decl();
            let name = self.parse_name()?;
            let params = self.parse_param_list()?;
            let return_ty = if self.eat(&TokenKind::Arrow) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let end = return_ty.as_ref().map(|t| t.span()).unwrap_or(name.span);
            Some(ExternMember::Func {
                visibility: vis,
                name,
                params,
                return_ty,
                span: start.cover(end),
            })
        } else {
            let span = self.current_span();
            self.error(
                "parse.expected_extern_member",
                "expected a function signature or `var` in extern block",
                span,
            );
            None
        }
    }
}

fn is_import_alias_recovery_boundary(kind: Option<&TokenKind>) -> bool {
    matches!(
        kind,
        Some(TokenKind::Module)
            | Some(TokenKind::Namespace)
            | Some(TokenKind::Import)
            | Some(TokenKind::Public)
            | Some(TokenKind::Func)
            | Some(TokenKind::Struct)
            | Some(TokenKind::Enum)
            | Some(TokenKind::Trait)
            | Some(TokenKind::Apply)
            | Some(TokenKind::Implement)
            | Some(TokenKind::Alias)
            | Some(TokenKind::Newtype)
            | Some(TokenKind::Const)
            | Some(TokenKind::Var)
            | Some(TokenKind::Extern)
            | Some(TokenKind::End)
            | Some(TokenKind::Comma)
    )
}

fn param_has_default(kind: &ParamKind) -> bool {
    matches!(
        kind,
        ParamKind::Default(_) | ParamKind::DefaultAndContract(_, _)
    )
}

fn param_is_required(kind: &ParamKind) -> bool {
    matches!(kind, ParamKind::Required | ParamKind::Contract(_))
}
