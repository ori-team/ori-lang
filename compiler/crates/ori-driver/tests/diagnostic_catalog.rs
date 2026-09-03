use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ori_diagnostics::Severity;
use ori_driver::pipeline::run_check_source;

const DIAGNOSTIC_CATEGORIES: &[&str] = &[
    "async",
    "attr",
    "backend",
    "bind",
    "concurrency",
    "contract",
    "control",
    "doc",
    "extern",
    "generic",
    "impl",
    "lex",
    "lint",
    "match",
    "mut",
    "name",
    "native",
    "parse",
    "project",
    "type",
    "using",
];

#[test]
fn diagnostic_catalog_matches_emitted_codes() {
    let root = repo_root();
    let emitted = emitted_codes(&root);
    let (catalog_emitted, catalog_planned) =
        catalog_codes(&root.join("docs/spec/13-error-catalog.md"));

    let missing_from_catalog: Vec<_> = emitted.difference(&catalog_emitted).cloned().collect();
    assert!(
        missing_from_catalog.is_empty(),
        "diagnostic codes emitted by compiler but missing from emitted catalog: {missing_from_catalog:#?}"
    );

    let stale_emitted_catalog: Vec<_> = catalog_emitted.difference(&emitted).cloned().collect();
    assert!(
        stale_emitted_catalog.is_empty(),
        "diagnostic codes listed as emitted but not found in compiler source: {stale_emitted_catalog:#?}"
    );

    let emitted_as_planned: Vec<_> = emitted.intersection(&catalog_planned).cloned().collect();
    assert!(
        emitted_as_planned.is_empty(),
        "diagnostic codes are emitted but still documented as planned/reserved: {emitted_as_planned:#?}"
    );

    let planned_unused: Vec<_> = catalog_planned.difference(&emitted).cloned().collect();
    if !planned_unused.is_empty() {
        eprintln!("planned/reserved diagnostic codes not emitted today: {planned_unused:#?}");
    }

    // Etapa 7 nomenclature audit: codes explicitly removed from the v1
    // catalog must not reappear as planned. Each removed code is redundant,
    // not applicable, or deferred to v2 with documented justification in
    // `docs/spec/13-error-catalog.md`.
    let removed_in_audit: BTreeSet<String> = [
        "contract.check_failure",
        "contract.field_violation",
        "contract.param_violation",
        "doc.unclosed_block",
        "generic.ambiguous_type_arg",
        "match.guard_not_exhaustive",
        "type.ambiguous_generic",
        "type.annotation_required",
        "using.non_result_init",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let reintroduced: Vec<_> = catalog_planned
        .intersection(&removed_in_audit)
        .cloned()
        .collect();
    assert!(
        reintroduced.is_empty(),
        "diagnostic codes removed in Etapa 7 audit reappeared as planned: {reintroduced:#?}"
    );
}

#[test]
fn representative_diagnostics_have_catalog_shapes_across_phases() {
    use ori_driver::pipeline::run_lint_source;

    // 1. Parser diagnostic: parse.module_missing
    let parse_out = run_check_source(
        Path::new("missing_module.orl"),
        "main()\nend\n".to_owned(),
    )
    .expect("in-memory fixture should be checkable");
    let parse_diag = parse_out
        .diagnostics
        .iter()
        .find(|d| d.code == "parse.module_missing")
        .expect("missing module must emit parse.module_missing");
    assert_eq!(parse_diag.severity, Severity::Error);
    assert!(!parse_diag.message.trim().is_empty());
    let primary = parse_diag.labels.first().expect("primary label");
    assert!(primary.span.start <= primary.span.end);

    // 2. Resolver diagnostic: name.undefined
    let resolve_out = run_check_source(
        Path::new("unresolved.orl"),
        "module app.main\n\nmain()\n    unknown_function_call()\nend\n".to_owned(),
    )
    .expect("in-memory fixture should be checkable");
    let resolve_diag = resolve_out
        .diagnostics
        .iter()
        .find(|d| d.code == "name.undefined")
        .expect("undefined symbol must emit name.undefined");
    assert_eq!(resolve_diag.severity, Severity::Error);
    assert!(!resolve_diag.message.trim().is_empty());
    assert!(!resolve_diag.labels.is_empty());

    // 3. Type checker diagnostic: type.type_mismatch
    let type_out = run_check_source(
        Path::new("type_mismatch.orl"),
        "module app.main\n\nmain()\n    const x: int = \"hello\"\nend\n".to_owned(),
    )
    .expect("in-memory fixture should be checkable");
    let type_diag = type_out
        .diagnostics
        .iter()
        .find(|d| d.code == "type.type_mismatch")
        .expect("mismatched type must emit type.type_mismatch");
    assert_eq!(type_diag.severity, Severity::Error);
    assert!(!type_diag.message.trim().is_empty());

    // 4. Attribute diagnostic: attr.c_export_not_public
    let attr_out = run_check_source(
        Path::new("attr_check.orl"),
        "module app.main\n\n@c_export\ninternal_func() -> void\nend\n\nmain()\nend\n".to_owned(),
    )
    .expect("in-memory fixture should be checkable");
    let attr_diag = attr_out
        .diagnostics
        .iter()
        .find(|d| d.code == "attr.c_export_not_public")
        .expect("non-public c_export must emit attr.c_export_not_public");
    assert_eq!(attr_diag.severity, Severity::Error);
    assert!(!attr_diag.message.trim().is_empty());

    // 5. Semantic linter diagnostic: lint.unused_variable
    let lint_out = run_lint_source(
        Path::new("unused_var.orl"),
        "module app.main\n\nmain()\n    const unused_value: int = 100\nend\n".to_owned(),
    )
    .expect("in-memory fixture should be lintable");
    let lint_diag = lint_out
        .diagnostics
        .iter()
        .find(|d| d.code == "lint.unused_variable")
        .expect("unused variable must emit lint.unused_variable");
    assert_eq!(lint_diag.severity, Severity::Warning);
    assert!(!lint_diag.message.trim().is_empty());
    assert!(lint_diag.action.is_some(), "linter diagnostics must provide actionable suggestions");
}

#[test]
fn emitted_catalog_rows_have_valid_severity_and_description() {
    let text = fs::read_to_string(repo_root().join("docs/spec/13-error-catalog.md"))
        .expect("diagnostic catalog should be readable");
    let mut in_emitted = false;
    let mut rows = 0;
    for line in text.lines() {
        if line.starts_with("## Emitted Diagnostics") {
            in_emitted = true;
            continue;
        }
        if in_emitted && line.starts_with("## ") {
            break;
        }
        if !in_emitted || table_code(line).is_none() {
            continue;
        }
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        assert!(
            matches!(
                cells.get(2),
                Some(&"error") | Some(&"warning") | Some(&"warning/error") | Some(&"runtime abort")
            ),
            "invalid severity row: {line}"
        );
        assert!(
            cells
                .get(3)
                .is_some_and(|description| !description.is_empty()),
            "emitted catalog rows need a description: {line}"
        );
        rows += 1;
    }
    assert!(rows > 0, "emitted diagnostic catalog must contain rows");
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("ori-driver crate should be under compiler/crates/ori-driver")
        .to_path_buf()
}

fn emitted_codes(root: &Path) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    collect_source_codes(&root.join("compiler/crates"), &mut codes);
    codes
}

fn collect_source_codes(path: &Path, codes: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_source_codes(&path, codes);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        if !path
            .components()
            .any(|component| component.as_os_str() == "src")
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        collect_string_literal_codes(&text, codes);
    }
}

fn collect_string_literal_codes(text: &str, codes: &mut BTreeSet<String>) {
    for category in DIAGNOSTIC_CATEGORIES {
        let prefix = format!("\"{category}.");
        let mut search_from = 0;
        while let Some(relative_start) = text[search_from..].find(&prefix) {
            let start = search_from + relative_start + 1;
            let end = text[start..]
                .bytes()
                .take_while(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || *byte == b'_'
                        || *byte == b'.'
                })
                .count()
                + start;
            let value = &text[start..end];
            if text.as_bytes().get(end) == Some(&b'"') && is_diagnostic_code(value) {
                codes.insert(value.to_string());
            }
            search_from = end;
        }
    }
}

fn catalog_codes(path: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let text = fs::read_to_string(path).expect("diagnostic catalog should be readable");
    let mut section = CatalogSection::Other;
    let mut emitted = BTreeSet::new();
    let mut planned = BTreeSet::new();

    for line in text.lines() {
        if line.starts_with("## Emitted Diagnostics") {
            section = CatalogSection::Emitted;
            continue;
        }
        if line.starts_with("## Planned Or Reserved Diagnostics") {
            section = CatalogSection::Planned;
            continue;
        }
        if line.starts_with("## ") {
            section = CatalogSection::Other;
            continue;
        }
        let Some(code) = table_code(line) else {
            continue;
        };
        match section {
            CatalogSection::Emitted => {
                emitted.insert(code);
            }
            CatalogSection::Planned => {
                planned.insert(code);
            }
            CatalogSection::Other => {}
        }
    }

    (emitted, planned)
}

fn table_code(line: &str) -> Option<String> {
    if !line.starts_with('|') {
        return None;
    }
    let start = line.find('`')? + 1;
    let end = line[start..].find('`')? + start;
    let code = &line[start..end];
    is_diagnostic_code(code).then(|| code.to_string())
}

fn is_diagnostic_code(value: &str) -> bool {
    let Some((category, rest)) = value.split_once('.') else {
        return false;
    };
    DIAGNOSTIC_CATEGORIES.contains(&category)
        && !rest.is_empty()
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[derive(Clone, Copy)]
enum CatalogSection {
    Emitted,
    Planned,
    Other,
}
