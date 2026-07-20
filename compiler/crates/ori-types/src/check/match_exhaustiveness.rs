use super::*;

impl<'a> Checker<'a> {
    pub(super) fn check_match_duplicate_cases(
        &mut self,
        scr_ty: &Ty,
        cases: &[ori_ast::stmt::MatchCase],
    ) {
        if scr_ty.is_error() || scr_ty.contains_infer() {
            return;
        }

        let mut seen = HashSet::new();
        for case in cases {
            let ori_ast::stmt::MatchCase::Pattern {
                pattern,
                guard: None,
                span,
                ..
            } = case
            else {
                continue;
            };

            let Some(key) = self.match_duplicate_key(pattern, false) else {
                continue;
            };

            if !seen.insert(key) {
                self.sink.emit(
                    Diagnostic::warning("match.duplicate_case", "match case is duplicated")
                        .with_label(Label::primary(
                            self.file_id,
                            *span,
                            "this case repeats an earlier case",
                        ))
                        .with_note("an earlier unguarded case has the same pattern")
                        .with_action(
                            "remove the duplicated case or add a guard if it is intentional",
                        ),
                );
            }
        }
    }

    pub(super) fn check_match_unreachable_cases(
        &mut self,
        scr_ty: &Ty,
        cases: &[ori_ast::stmt::MatchCase],
    ) {
        if scr_ty.is_error() || scr_ty.contains_infer() {
            return;
        }

        let mut catch_all_seen = false;
        for case in cases {
            if catch_all_seen {
                self.sink.emit(
                    Diagnostic::warning("match.unreachable_case", "match case is unreachable")
                        .with_label(Label::primary(
                            self.file_id,
                            match_case_span(case),
                            "this case cannot be reached",
                        ))
                        .with_note(
                            "an earlier unguarded case already matches every remaining value",
                        )
                        .with_action("move this case before the catch-all case or remove it"),
                );
                continue;
            }

            if self.case_is_unguarded_catch_all(case, scr_ty) {
                catch_all_seen = true;
            }
        }
    }

    pub(super) fn check_match_exhaustiveness(
        &mut self,
        scr_ty: &Ty,
        cases: &[ori_ast::stmt::MatchCase],
        span: ori_diagnostics::Span,
    ) {
        if scr_ty.is_error() || scr_ty.contains_infer() {
            return;
        }

        // `case a or b:` covers exactly what `case a:` and `case b:` cover, so
        // expanding first lets every per-type branch below stay unaware of
        // or-patterns.
        let expanded = expand_or_patterns(cases);
        let cases: &[ori_ast::stmt::MatchCase] = &expanded;

        if cases
            .iter()
            .any(|case| self.case_is_unguarded_catch_all(case, scr_ty))
        {
            self.exhaustive_matches.insert(span);
            return;
        }

        match scr_ty {
            Ty::Bool => {
                let mut seen_true = false;
                let mut seen_false = false;
                for case in cases {
                    let ori_ast::stmt::MatchCase::Pattern {
                        pattern,
                        guard: None,
                        ..
                    } = case
                    else {
                        continue;
                    };
                    if let Pattern::Literal(expr) = pattern {
                        match expr.as_ref() {
                            Expr::BoolLit(true, _) => seen_true = true,
                            Expr::BoolLit(false, _) => seen_false = true,
                            _ => {}
                        }
                    }
                }
                let mut missing = Vec::new();
                if !seen_true {
                    missing.push("true".to_string());
                }
                if !seen_false {
                    missing.push("false".to_string());
                }
                self.emit_match_non_exhaustive(span, missing);
            }
            Ty::Optional(_) => {
                let mut seen_some = false;
                let mut seen_none = false;
                for case in cases {
                    let ori_ast::stmt::MatchCase::Pattern {
                        pattern,
                        guard: None,
                        ..
                    } = case
                    else {
                        continue;
                    };
                    match pattern {
                        Pattern::Some(_, _) => seen_some = true,
                        Pattern::None(_) => seen_none = true,
                        _ => {}
                    }
                }
                let mut missing = Vec::new();
                if !seen_some {
                    missing.push("some(...)".to_string());
                }
                if !seen_none {
                    missing.push("none".to_string());
                }
                self.emit_match_non_exhaustive(span, missing);
            }
            Ty::Result(_, _) => {
                let mut seen_success = false;
                let mut seen_error = false;
                for case in cases {
                    let ori_ast::stmt::MatchCase::Pattern {
                        pattern,
                        guard: None,
                        ..
                    } = case
                    else {
                        continue;
                    };
                    match pattern {
                        Pattern::Ok(_, _) => seen_success = true,
                        Pattern::Err(_, _) => seen_error = true,
                        _ => {}
                    }
                }
                let mut missing = Vec::new();
                if !seen_success {
                    missing.push("success(...)".to_string());
                }
                if !seen_error {
                    missing.push("error(...)".to_string());
                }
                self.emit_match_non_exhaustive(span, missing);
            }
            Ty::Named(def_id, _) if self.def_map.get(*def_id).kind == DefKind::Enum => {
                let Some(enum_sig) = self.enum_sig(*def_id) else {
                    return;
                };
                let mut covered = HashSet::new();
                for case in cases {
                    let ori_ast::stmt::MatchCase::Pattern {
                        pattern,
                        guard: None,
                        ..
                    } = case
                    else {
                        continue;
                    };
                    if let Some(name) = self.covered_enum_variant(pattern, enum_sig) {
                        covered.insert(name);
                    }
                }
                let missing: Vec<String> = enum_sig
                    .variants
                    .iter()
                    .filter(|variant| !covered.contains(&variant.name))
                    .map(|variant| variant.name.to_string())
                    .collect();
                self.emit_match_non_exhaustive(span, missing);
            }
            _ => {}
        }
    }

    fn case_is_unguarded_catch_all(&self, case: &ori_ast::stmt::MatchCase, scr_ty: &Ty) -> bool {
        match case {
            ori_ast::stmt::MatchCase::Else { .. } => true,
            ori_ast::stmt::MatchCase::Pattern {
                pattern,
                guard: None,
                ..
            } => self.pattern_is_catch_all(pattern, scr_ty),
            ori_ast::stmt::MatchCase::Pattern { .. } => false,
        }
    }

    fn pattern_is_catch_all(&self, pattern: &Pattern, scr_ty: &Ty) -> bool {
        match pattern {
            Pattern::Wildcard(_) => true,
            Pattern::Binding(name) => {
                if let Ty::Named(def_id, _) = scr_ty {
                    if let Some(enum_sig) = self.enum_sig(*def_id) {
                        return !enum_sig
                            .variants
                            .iter()
                            .any(|variant| variant.name == name.text);
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn covered_enum_variant(&self, pattern: &Pattern, enum_sig: &EnumSig) -> Option<SmolStr> {
        match pattern {
            Pattern::VariantUnit { name, .. } | Pattern::Binding(name) => {
                let variant = enum_sig
                    .variants
                    .iter()
                    .find(|variant| variant.name == name.text)?;
                if variant.fields.is_empty() {
                    Some(name.text.clone())
                } else {
                    None
                }
            }
            Pattern::VariantNamed { name, fields, .. } => {
                let variant = enum_sig
                    .variants
                    .iter()
                    .find(|variant| variant.name == name.text)?;
                if fields.len() != variant.fields.len() {
                    return None;
                }

                let mut seen = HashSet::new();
                for field in fields {
                    if !seen.insert(field.name.text.clone()) {
                        return None;
                    }
                    if !variant
                        .fields
                        .iter()
                        .any(|(field_name, _)| field_name == &field.name.text)
                    {
                        return None;
                    }
                }
                Some(name.text.clone())
            }
            _ => None,
        }
    }

    fn match_duplicate_key(&self, pattern: &Pattern, allow_binding: bool) -> Option<String> {
        match pattern {
            // Each alternative is its own coverage key; an or-pattern as a
            // whole has none, so it never reads as a duplicate of a single
            // pattern.
            Pattern::Or(_, _) => None,
            Pattern::Wildcard(_) => Some("_".to_string()),
            Pattern::Binding(_) if allow_binding => Some("_".to_string()),
            Pattern::Binding(_) => None,
            Pattern::Literal(expr) => self.literal_duplicate_key(expr),
            Pattern::VariantUnit { name, .. } => Some(format!("variant:{}", name.text)),
            Pattern::VariantNamed { name, fields, .. } => {
                let mut field_keys = Vec::new();
                for field in fields {
                    let key = self.match_duplicate_key(&field.pattern, true)?;
                    field_keys.push((field.name.text.to_string(), key));
                }
                field_keys.sort_by(|left, right| left.0.cmp(&right.0));
                Some(format!("variant:{}:{field_keys:?}", name.text))
            }
            Pattern::Some(inner, _) => self
                .match_duplicate_key(inner, true)
                .map(|key| format!("some({key})")),
            Pattern::None(_) => Some("none".to_string()),
            Pattern::Ok(inner, _) => self
                .match_duplicate_key(inner, true)
                .map(|key| format!("success({key})")),
            Pattern::Err(inner, _) => self
                .match_duplicate_key(inner, true)
                .map(|key| format!("error({key})")),
            Pattern::Tuple(patterns, _) => {
                let mut keys = Vec::new();
                for pattern in patterns {
                    keys.push(self.match_duplicate_key(pattern, true)?);
                }
                Some(format!("tuple({})", keys.join(",")))
            }
        }
    }

    fn literal_duplicate_key(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::BoolLit(value, _) => Some(format!("bool:{value}")),
            Expr::IntLit { raw, .. } => Some(format!("int:{raw}")),
            Expr::FloatLit { raw, .. } => Some(format!("float:{raw}")),
            Expr::StrLit { value, .. } => Some(format!("string:{value}")),
            Expr::Unary {
                op: UnaryOp::Neg,
                operand,
                ..
            } => self
                .literal_duplicate_key(operand)
                .map(|key| format!("neg:{key}")),
            _ => None,
        }
    }

    fn emit_match_non_exhaustive(&mut self, span: ori_diagnostics::Span, missing: Vec<String>) {
        if missing.is_empty() {
            // Nothing missing: record it so return analysis (which runs later
            // and has no types) can treat every arm as covering a path.
            self.exhaustive_matches.insert(span);
            return;
        }
        self.sink.emit(
            Diagnostic::error(
                "match.non_exhaustive",
                format!("match is not exhaustive; missing {}", missing.join(", ")),
            )
            .with_label(Label::primary(self.file_id, span, "match checked here"))
            .with_action("add the missing cases or a `case else` arm"),
        );
    }
}

fn match_case_span(case: &ori_ast::stmt::MatchCase) -> ori_diagnostics::Span {
    match case {
        ori_ast::stmt::MatchCase::Pattern { span, .. }
        | ori_ast::stmt::MatchCase::Else { span, .. } => *span,
    }
}

/// Rewrite `case a or b:` into one case per alternative.
///
/// Coverage-wise the two are identical, so every per-type analysis in this
/// module can ignore `Pattern::Or` entirely. Bodies are dropped: only
/// pattern, guard and span are read by these checks.
fn expand_or_patterns(cases: &[ori_ast::stmt::MatchCase]) -> Vec<ori_ast::stmt::MatchCase> {
    let mut expanded = Vec::with_capacity(cases.len());
    for case in cases {
        match case {
            ori_ast::stmt::MatchCase::Pattern {
                pattern: Pattern::Or(alternatives, _),
                guard,
                span,
                ..
            } => {
                for alternative in alternatives {
                    expanded.push(ori_ast::stmt::MatchCase::Pattern {
                        pattern: alternative.clone(),
                        guard: guard.clone(),
                        body: Vec::new(),
                        span: *span,
                    });
                }
            }
            other => expanded.push(other.clone()),
        }
    }
    expanded
}
