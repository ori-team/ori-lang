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
        "module", "import", "imports", "public",
        // `func` remains a keyword for callable types `func(T) -> R`, not declarations.
        "func", "return", "end", "const", "var", "if", "else", "elif", "while", "for", "in",
        "repeat", "loop", "break", "continue", "match", "case", "struct", "trait", "apply", "use",
        "enum", "alias", "newtype", "and", "or", "not", "true", "false", "none", "ok", "err",
        "some", "mut", "self", "attr", "extern", "any", "optional", "result", "list", "map", "set",
        "range", "void", "handle", "using", "try", "check", "with", "then", "tuple", "lazy",
        "async", "await", "iter",
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
            "apply ${1:Type} use ${2:Trait}\n    ${3:method}(self) -> ${4:ret}\n        ${0}\n    end\nend",
        ),
        snippet("if", "if ${1:condition}\n    ${0}\nend"),
        snippet("ifelse", "if ${1:condition}\n    ${2}\nelse\n    ${0}\nend"),
        snippet("while", "while ${1:condition}\n    ${0}\nend"),
        snippet("for", "for ${1:item} in ${2:collection}\n    ${0}\nend"),
        snippet("loop", "loop\n    ${0}\nend"),
        snippet("match", "match ${1:value}\ncase ${2:pattern}:\n    ${0}\nend"),
        // `using` is a single statement. The surrounding function/block owns
        // the `end`; inserting one here would close user code unexpectedly.
        snippet("using", "using ${1:name}: ${2:Type} = ${3:expr}"),
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

#[cfg(test)]
mod tests {
    use super::{keyword_completion_items, snippet_completion_items};

    fn assert_parses(source: &str) {
        let file_id = ori_diagnostics::FileId(0);
        let mut sink = ori_diagnostics::DiagnosticSink::default();
        let tokens = ori_lexer::lex(source, file_id, &mut sink);
        let _ = ori_parser::parse(&tokens, source, file_id, &mut sink);
        assert!(
            !sink.has_errors(),
            "canonical snippet fixture must parse: {:?}",
            sink.diagnostics()
        );
    }

    #[test]
    fn keyword_completion_only_offers_s3_surface() {
        let labels: Vec<_> = keyword_completion_items()
            .into_iter()
            .map(|item| item.label)
            .collect();

        for removed in ["namespace", "as", "only", "implement", "where", "is", "do"] {
            assert!(
                !labels.iter().any(|label| label == removed),
                "removed S3 keyword `{removed}` must not be suggested"
            );
        }
        for canonical in [
            "module", "imports", "elif", "newtype", "async", "await", "try",
        ] {
            assert!(
                labels.iter().any(|label| label == canonical),
                "canonical S3 keyword `{canonical}` must be suggested"
            );
        }

        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            labels.len(),
            sorted.len(),
            "keyword list must not contain duplicates"
        );
    }

    #[test]
    fn snippets_do_not_insert_removed_syntax_or_extra_block_end() {
        let snippets = snippet_completion_items();
        let apply = snippets
            .iter()
            .find(|item| item.label == "apply")
            .and_then(|item| item.insert_text.as_deref())
            .expect("apply snippet");
        assert!(apply.starts_with("apply ${1:Type} use ${2:Trait}"));
        assert!(!apply.contains("implement") && !apply.contains(" to "));

        let using = snippets
            .iter()
            .find(|item| item.label == "using")
            .and_then(|item| item.insert_text.as_deref())
            .expect("using snippet");
        assert!(
            !using.contains("\nend"),
            "using is a statement, not a block"
        );

        for item in snippets {
            let body = item.insert_text.unwrap_or_default();
            assert!(!body.contains(" as "), "snippet uses removed import syntax");
            assert!(
                !body.contains(" only "),
                "snippet uses removed import syntax"
            );
            assert!(!body.contains("<"), "snippet uses removed angle syntax");
        }

        assert_parses(
            "module app.main\n\ntrait Trait\n    method(self) -> int\nend\n\nstruct Type\nend\n\napply Type use Trait\n    method(self) -> int\n        return 1\n    end\nend\n",
        );
        assert_parses("module app.main\n\nmain()\n    using value: string = \"ok\"\nend\n");
    }
}
