use std::collections::BTreeSet;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

use crate::stdlib_catalog::{import_alias_map, stdlib_catalog};

/// Generate completion items for the Ori standard library (Layer 1 + Layer 2).
pub fn stdlib_completion_items() -> Vec<CompletionItem> {
    stdlib_catalog().completion_items()
}

/// Completion items when the cursor is inside an `import` statement.
///
/// `prefix` is the path typed after `import ` (e.g. `ori.` or `ori.st`).
/// `insert_text` is only the remaining suffix so `import ori.` + accept `io`
/// becomes `import ori.io`, not `import ori.ori.io`.
///
/// **Product surface (M2 / STDLIB-1):** only canonical `ori.X` modules.
/// Nested `ori.X.utils` / `ori.X.algorithms` stay importable as silent compat
/// but are **not** offered in autocomplete (`stdlib-merge-policy.md`).
pub fn stdlib_import_completion_items(prefix: &str) -> Vec<CompletionItem> {
    stdlib_catalog()
        .modules()
        .filter(|m| !is_compat_nested_module(m))
        .filter(|m| prefix.is_empty() || m.starts_with(prefix) || m.contains(prefix))
        .map(|m| {
            let insert = import_insert_suffix(prefix, m);
            CompletionItem {
                label: m.clone(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some("Ori stdlib module".into()),
                filter_text: Some(m.clone()),
                insert_text: Some(insert),
                ..CompletionItem::default()
            }
        })
        .collect()
}

/// Legacy nested paths kept for compile compat — hide from teachable UI.
fn is_compat_nested_module(module: &str) -> bool {
    module.ends_with(".utils") || module.ends_with(".algorithms")
}

/// Keywords valid on an import line after the path (`= alias` S3).
pub fn import_keyword_completion_items() -> Vec<CompletionItem> {
    // S3: `import ori.io = io` — only `=` is punctuation; no `as`/`only` clause keywords.
    // Keep empty for path position; alias names come from the user.
    Vec::new()
}

fn import_insert_suffix(prefix: &str, module: &str) -> String {
    if prefix.is_empty() {
        return module.to_string();
    }
    if let Some(suffix) = module.strip_prefix(prefix) {
        return suffix.to_string();
    }
    module.to_string()
}

/// Dot-completion items for a stdlib import alias or module prefix.
pub fn stdlib_dot_completion_items(receiver: &str, source: &str) -> Vec<CompletionItem> {
    let import_map = import_alias_map(source);
    stdlib_catalog().dot_completion_items(receiver, &import_map)
}

/// Keyword completions for the Ori language (S3 surface).
pub fn keyword_completion_items() -> Vec<CompletionItem> {
    let keywords = [
        "module",
        "import",
        "imports",
        // Removed S3 forms still highlighted; kept as completions only for migration search.
        "as",
        "only",
        "public",
        // `func` remains a keyword for callable types `func(T) -> R`, not declarations.
        "func",
        "return",
        "end",
        "const",
        "var",
        "if",
        "else",
        "elif",
        "try",
        "while",
        "for",
        "in",
        "repeat",
        "loop",
        "break",
        "continue",
        "match",
        "case",
        "struct",
        "trait",
        "apply",
        "use",
        "implement",
        "enum",
        "where",
        "is",
        "alias",
        "do",
        "and",
        "or",
        "not",
        "true",
        "false",
        "none",
        "ok",
        "err",
        "some",
        "mut",
        "self",
        "extern",
        "any",
        "optional",
        "result",
        "list",
        "map",
        "set",
        "range",
        "void",
        "using",
        "try",
        "check",
        "with",
        "then",
        "tuple",
        "lazy",
        "async",
        "await",
    ];

    keywords
        .iter()
        .map(|kw| CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Ori keyword".to_string()),
            ..CompletionItem::default()
        })
        .collect()
}

/// Snippet completions for common Ori constructs (S3: no declaration `func`).
pub fn snippet_completion_items() -> Vec<CompletionItem> {
    vec![
        snippet(
            "fn",
            "${1:name}(${2:params}) -> ${3:ret}\n    ${0}\nend",
        ),
        snippet(
            "main",
            "module ${1:app.main}\n\nimport ori.io = io\n\nmain() -> void\n    ${0}\nend",
        ),
        snippet(
            "async fn",
            "async ${1:name}(${2:params}) -> ${3:ret}\n    ${0}\nend",
        ),
        snippet("struct", "struct ${1:Name}\n    ${0}\nend"),
        snippet("enum", "enum ${1:Name}\n    ${0}\nend"),
        snippet(
            "trait",
            "trait ${1:Name}\n    ${2:method}(self) -> ${3:ret}\nend",
        ),
        snippet(
            "apply",
            "apply ${1:Type}\n    use ${2:Trait}\n        ${3:method}(self) -> ${4:ret}\n            ${0}\n        end\n    end\nend",
        ),
        snippet("if", "if ${1:condition}\n    ${0}\nend"),
        snippet("ifelse", "if ${1:condition}\n    ${2}\nelse\n    ${0}\nend"),
        snippet("while", "while ${1:condition}\n    ${0}\nend"),
        snippet("for", "for ${1:item} in ${2:collection}\n    ${0}\nend"),
        snippet("loop", "loop\n    ${0}\nend"),
        snippet("match", "match ${1:value}\ncase ${2:pattern}:\n    ${0}\nend"),
        snippet(
            "using",
            "using ${1:name}: ${2:Type} = ${3:expr}\n    ${0}\nend",
        ),
        snippet("check", "check ${1:condition}, \"${2:message}\""),
        snippet("import", "import ${1:ori.module} = ${2:alias}"),
    ]
}

fn snippet(label: &str, body: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Ori snippet".to_string()),
        insert_text: Some(body.to_string()),
        insert_text_format: Some(tower_lsp::lsp_types::InsertTextFormat::SNIPPET),
        ..CompletionItem::default()
    }
}

/// Deduplicate completion items by label while preserving order.
pub fn dedupe_completion_items(items: &mut Vec<CompletionItem>) {
    let mut seen = BTreeSet::new();
    items.retain(|item| seen.insert(item.label.clone()));
}
