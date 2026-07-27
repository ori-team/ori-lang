/// Lint checks that augment compiler diagnostics.
///
/// The compiler already reports parse errors, type errors, etc. This module
/// adds best-effort lint warnings: style and correctness hints that are not
/// compilation errors but are useful while editing Ori code.
use std::collections::HashMap;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

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

/// Run all enabled lint checks on source code and return LSP diagnostics.
pub fn lint(source: &str, config: &LintConfig) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut seen_bindings: HashMap<String, usize> = HashMap::new();

    for (line_index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let binding_name = extract_binding_name(trimmed).or_else(|| extract_var_binding(trimmed));

        if config.shadowed_variable {
            if let Some(name) = binding_name.as_deref() {
                if !name.starts_with('_') {
                    if let Some(previous_line) = seen_bindings.get(name) {
                        diagnostics.push(lint_diagnostic(
                            range_for_word_on_line(line, name, line_index),
                            DiagnosticSeverity::WARNING,
                            "lint.shadowed_variable",
                            format!(
                                "Binding `{name}` shadows a previous binding on line {}.",
                                previous_line + 1
                            ),
                        ));
                    }
                }
            }
        }

        if let Some(name) = binding_name.as_deref() {
            seen_bindings.entry(name.to_string()).or_insert(line_index);
        }

        if config.unused_variable {
            if let Some(name) = binding_name.as_deref() {
                let mut ref_count = 0;
                for (other_index, other) in lines.iter().enumerate() {
                    if line_index == other_index {
                        continue;
                    }
                    ref_count += other.matches(name).count();
                }
                if ref_count == 0 && !name.starts_with('_') {
                    diagnostics.push(lint_diagnostic(
                        range_for_word_on_line(line, name, line_index),
                        DiagnosticSeverity::WARNING,
                        "lint.unused_variable",
                        format!("Variable `{name}` is never used. Prefix with `_` to suppress."),
                    ));
                }
            }
        }

        if config.prefer_const {
            if let Some(name) = extract_var_binding(trimmed) {
                if !name.starts_with('_') {
                    let is_mutated = lines.iter().any(|line| {
                        line.contains(&format!("{name} :=")) || line.contains(&format!("{name} ="))
                    });
                    if !is_mutated {
                        diagnostics.push(lint_diagnostic(
                            range_for_word_on_line(line, &name, line_index),
                            DiagnosticSeverity::HINT,
                            "lint.prefer_const",
                            format!("`{name}` is never mutated; consider using `const`."),
                        ));
                    }
                }
            }
        }
    }

    diagnostics
}

fn lint_diagnostic(
    range: Range,
    severity: DiagnosticSeverity,
    code: &str,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(code.into())),
        source: Some("ori-lint".into()),
        message,
        ..Default::default()
    }
}

fn extract_binding_name(line: &str) -> Option<String> {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix("const ") {
        let name = rest
            .split(|c: char| c.is_whitespace() || c == ':' || c == '=')
            .next()?
            .to_string();
        if is_valid_binding_name(&name) {
            return Some(name);
        }
    }
    if let Some((name, _)) = line.split_once(" :=") {
        let name = name.trim().to_string();
        if is_valid_binding_name(&name) && !name.contains(' ') {
            return Some(name);
        }
    }
    None
}

fn extract_var_binding(line: &str) -> Option<String> {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix("var ") {
        let name = rest
            .split(|c: char| c.is_whitespace() || c == ':' || c == '=')
            .next()?
            .to_string();
        if is_valid_binding_name(&name) {
            return Some(name);
        }
    }
    None
}

fn is_valid_binding_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .map(|c| c.is_alphabetic() || c == '_')
            .unwrap_or(false)
}

fn range_for_word_on_line(line: &str, word: &str, line_index: usize) -> Range {
    let column = line.find(word).unwrap_or(0);
    let line = line_index as u32;
    Range {
        start: Position {
            line,
            character: column as u32,
        },
        end: Position {
            line,
            character: (column + word.len()) as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::NumberOrString;

    fn diagnostic_codes(source: &str, config: &LintConfig) -> Vec<String> {
        lint(source, config)
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
        let codes = diagnostic_codes("main()\n    const value = 1\nend\n", &config);
        assert!(!codes.iter().any(|code| code == "lint.unused_variable"));
    }

    #[test]
    fn config_can_disable_prefer_const() {
        let config = LintConfig {
            unused_variable: false,
            shadowed_variable: false,
            prefer_const: false,
        };
        let codes = diagnostic_codes("main()\n    var value: int = 1\nend\n", &config);
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
            "main()\n    const value = 1\n    const value = 2\nend\n",
            &config,
        );
        assert!(codes.iter().any(|code| code == "lint.shadowed_variable"));
    }
}
