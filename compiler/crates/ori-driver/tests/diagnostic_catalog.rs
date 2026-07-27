use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
