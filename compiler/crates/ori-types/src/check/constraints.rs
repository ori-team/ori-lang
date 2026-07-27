use super::*;

impl<'a> Checker<'a> {
    pub(super) fn check_where_constraints(
        &mut self,
        constraints: &[WhereConstraintSig],
        subst: &HashMap<u32, Ty>,
        span: ori_diagnostics::Span,
    ) {
        for constraint in constraints {
            let Some(actual) = subst.get(&constraint.param_index) else {
                continue;
            };
            if actual.is_error() || actual.contains_infer() || contains_generic_param(actual) {
                continue;
            }

            let satisfied = self.type_satisfies_trait(actual, constraint.trait_def_id);
            let failed = if constraint.negative {
                satisfied
            } else {
                !satisfied
            };
            if !failed {
                continue;
            }

            let trait_name = self.def_map.get(constraint.trait_def_id).name.clone();
            let relation = if constraint.negative {
                "must not implement"
            } else {
                "must implement"
            };
            self.sink.emit(
                Diagnostic::error(
                    if constraint.negative {
                        "generic.negative_constraint_violated"
                    } else {
                        "generic.constraint_not_satisfied"
                    },
                    format!(
                        "`{}` {} `{}`, but call uses `{}`",
                        constraint.param_name,
                        relation,
                        trait_name,
                        actual.display_in(self.def_map)
                    ),
                )
                .with_label(Label::primary(self.file_id, span, "generic call here"))
                .with_action("pass a value whose type satisfies the function `where` clause"),
            );
        }
    }

    fn type_satisfies_trait(&self, ty: &Ty, trait_def_id: DefId) -> bool {
        if let Some(equatable_def_id) = self.def_map.lookup("ori.core.Equatable") {
            if trait_def_id == equatable_def_id && self.supports_generic_equality(ty) {
                return true;
            }
        }
        match ty {
            Ty::Named(type_def_id, _) => {
                self.named_type_implements_trait(*type_def_id, trait_def_id)
            }
            Ty::Any(id) => *id == trait_def_id,
            _ => false,
        }
    }
}
