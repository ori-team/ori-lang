/// Lint checks that augment compiler diagnostics.
///
/// The compiler already reports parse errors, type errors, etc. This module
/// adds best-effort lint warnings: style and correctness hints that are not
/// compilation errors but are useful while editing Ori code.
use std::path::Path;
use tower_lsp::lsp_types::Diagnostic;

/// Upper bound for editor linting. Full semantic linting reparses and checks
/// the buffer; refusing pathological buffers keeps diagnostics responsive and
/// leaves compilation itself unrestricted.
pub const MAX_LINT_SOURCE_BYTES: usize = 1 << 20;

/// Configuration for which lints are enabled.
#[derive(Debug, Clone)]
pub struct LintConfig {
    pub unused_variable: bool,
    pub shadowed_variable: bool,
    pub prefer_const: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            unused_variable: true,
            shadowed_variable: true,
            prefer_const: true,
        }
    }
}

/// Run the compiler's AST-based linter on an in-memory editor buffer.
///
/// `run_lint_source` already resolves scopes and ignores comments/strings. We
/// retain the small config surface here by filtering only the optional LSP
/// warning families before converting their source spans to LSP ranges.
pub fn lint(path: Option<&Path>, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
    if source.len() > MAX_LINT_SOURCE_BYTES {
        return Vec::new();
    }
    let fallback = Path::new("<lsp-buffer>.orl");
    let target = path.unwrap_or(fallback);
    let Ok(output) = ori_driver::pipeline::run_lint_source(target, source.to_owned()) else {
        return Vec::new();
    };

    let filtered: Vec<_> = output
        .diagnostics
        .into_iter()
        .filter(|diagnostic| match diagnostic.code {
            "lint.unused_variable" => config.unused_variable,
            "lint.shadowed_variable" => config.shadowed_variable,
            "lint.prefer_const" => config.prefer_const,
            _ => false,
        })
        .collect();

    crate::handlers::diagnostics::diagnostics_for_path(&output.cache, &filtered, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::NumberOrString;

    fn diagnostic_codes(source: &str, config: &LintConfig) -> Vec<String> {
        lint(None, source, config)
            .into_iter()
            .filter_map(|diagnostic| match diagnostic.code {
                Some(NumberOrString::String(code)) => Some(code),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn config_can_disable_unused_variable() {
        let config = LintConfig {
            unused_variable: false,
            shadowed_variable: false,
            prefer_const: false,
        };
        let codes = diagnostic_codes(
            "module app.main\n\nmain()\n    const value = 1\nend\n",
            &config,
        );
        assert!(!codes.iter().any(|code| code == "lint.unused_variable"));
    }

    #[test]
    fn config_can_disable_prefer_const() {
        let config = LintConfig {
            unused_variable: false,
            shadowed_variable: false,
            prefer_const: false,
        };
        let codes = diagnostic_codes(
            "module app.main\n\nmain()\n    var value: int = 1\nend\n",
            &config,
        );
        assert!(!codes.iter().any(|code| code == "lint.prefer_const"));
    }

    #[test]
    fn detects_shadowed_variable_when_enabled() {
        let config = LintConfig {
            unused_variable: false,
            shadowed_variable: true,
            prefer_const: false,
        };
        let codes = diagnostic_codes(
            "module app.main\n\nmain()\n    const value = 1\n    if true\n        const value = 2\n    end\nend\n",
            &config,
        );
        assert!(codes.iter().any(|code| code == "lint.shadowed_variable"));
    }

    #[test]
    fn ast_lint_ignores_comments_and_string_contents() {
        let config = LintConfig {
            unused_variable: true,
            shadowed_variable: false,
            prefer_const: false,
        };
        let codes = diagnostic_codes(
            "module app.main\n\nmain()\n    const café = 1\n    check true, \"café is mentioned here\"\n    -- café is mentioned in a comment too\nend\n",
            &config,
        );
        assert!(
            codes.iter().any(|code| code == "lint.unused_variable"),
            "a binding mentioned only in comments/strings remains unused"
        );
    }

    #[test]
    fn ast_lint_tracks_nested_scope_shadowing() {
        let config = LintConfig {
            unused_variable: true,
            shadowed_variable: true,
            prefer_const: false,
        };
        let codes = diagnostic_codes(
            "module app.main\n\nmain()\n    const value = 1\n    if true\n        const value = 2\n        value\n    end\n    value\nend\n",
            &config,
        );
        assert!(
            codes.iter().any(|code| code == "lint.shadowed_variable"),
            "nested binding must be diagnosed as shadowing"
        );
        assert!(
            !codes.iter().any(|code| code == "lint.unused_variable"),
            "both bindings are read in their respective lexical scopes"
        );
    }

    #[test]
    fn ast_lint_keeps_outer_binding_unused_after_inner_shadowing() {
        let config = LintConfig {
            unused_variable: true,
            shadowed_variable: true,
            prefer_const: false,
        };
        let codes = diagnostic_codes(
            "module app.main\n\nmain()\n    const value = 1\n    if true\n        const value = 2\n        value\n    end\nend\n",
            &config,
        );
        assert_eq!(
            codes
                .iter()
                .filter(|code| code.as_str() == "lint.unused_variable")
                .count(),
            1,
            "the outer binding remains unused; the inner binding is read"
        );
        assert!(codes.iter().any(|code| code == "lint.shadowed_variable"));
    }

    #[test]
    fn large_buffers_skip_linting_within_the_editor_budget() {
        let config = LintConfig::default();
        let source = "x".repeat(MAX_LINT_SOURCE_BYTES + 1);
        assert!(lint(None, &source, &config).is_empty());
    }
}
