use ori_diagnostics::{Diagnostic as OriDiagnostic, Label, Severity, SourceCache};
use std::path::Path;
use tower_lsp::lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Range,
};

use crate::utils::position;
use crate::utils::uri;

/// Convert Ori diagnostics for a specific file into LSP diagnostics.
pub fn diagnostics_for_path(
    cache: &SourceCache,
    diagnostics: &[OriDiagnostic],
    target: &Path,
) -> Vec<LspDiagnostic> {
    let target = uri::canonical_path(target);
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let label = diagnostic
                .labels
                .iter()
                .find(|label| label_points_to_path(cache, label, &target));

            if !diagnostic.labels.is_empty() && label.is_none() {
                return None;
            }

            Some(LspDiagnostic {
                range: label
                    .map(|label| range_for_label(cache, label))
                    .unwrap_or_else(position::default_range),
                severity: Some(match diagnostic.severity {
                    Severity::Error => DiagnosticSeverity::ERROR,
                    Severity::Warning => DiagnosticSeverity::WARNING,
                }),
                code: Some(NumberOrString::String(diagnostic.code.to_string())),
                code_description: None,
                source: Some("ori".to_string()),
                message: diagnostic_message(diagnostic, label),
                related_information: None,
                tags: None,
                data: None,
            })
        })
        .collect()
}

/// Construct a file-level error diagnostic (e.g. when the file can't be read).
pub fn file_error_diagnostic(message: String) -> LspDiagnostic {
    LspDiagnostic {
        range: position::default_range(),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("lsp.file".to_string())),
        code_description: None,
        source: Some("ori".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Map a `run_check` / `run_check_source` error message to a structured
/// project-level diagnostic when it corresponds to a known project
/// configuration failure (Etapa 6.5).
///
/// The driver's `resolve_entry_path` returns plain `String` errors for these
/// cases (it predates the `project.*` diagnostic namespace). Rather than
/// refactor the error plumbing through every `run_*` entry point, the LSP
/// layer recognises the canonical messages and surfaces them under the
/// catalog's `project.*` codes. Returns `None` for unrecognized messages so
/// the caller can fall back to `file_error_diagnostic`.
pub fn project_error_diagnostic(message: &str) -> Option<LspDiagnostic> {
    let (code, severity) = if message.contains("project manifest") && message.contains("not found")
    {
        ("project.no_proj_file", DiagnosticSeverity::ERROR)
    } else if message.contains("project manifest") && message.contains("missing `entry`") {
        // Manifest exists but declares no `entry=...` key: the entrypoint
        // cannot be located, so report it as `entry_not_found`.
        ("project.entry_not_found", DiagnosticSeverity::ERROR)
    } else if message.contains("project entry") && message.contains("does not exist") {
        ("project.entry_not_found", DiagnosticSeverity::ERROR)
    } else {
        return None;
    };
    Some(LspDiagnostic {
        range: position::default_range(),
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some("ori".to_string()),
        message: message.to_string(),
        related_information: None,
        tags: None,
        data: None,
    })
}

/// Collect `project.*` diagnostics whose primary label is NOT on `target`.
///
/// Project-level diagnostics such as `project.circular_import` are emitted
/// with a label on the "back-edge" import (the file that closes the cycle),
/// which may differ from the file the user has open. `diagnostics_for_path`
/// routes by label, so those diagnostics would never reach the opened file.
/// This function surfaces them on the opened file using a default range, so
/// the user sees project-wide errors regardless of which file they edit
/// (Etapa 6.5).
pub fn project_diagnostics_for_path(
    cache: &SourceCache,
    diagnostics: &[OriDiagnostic],
    target: &Path,
) -> Vec<LspDiagnostic> {
    let target = uri::canonical_path(target);
    diagnostics
        .iter()
        .filter(|d| d.code.starts_with("project."))
        .filter(|d| {
            // Only include project diagnostics whose labels do NOT already
            // point at the target file (those are already returned by
            // `diagnostics_for_path`).
            d.labels
                .iter()
                .all(|label| !label_points_to_path(cache, label, &target))
        })
        .map(|d| LspDiagnostic {
            range: position::default_range(),
            severity: Some(match d.severity {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
            }),
            code: Some(NumberOrString::String(d.code.to_string())),
            code_description: None,
            source: Some("ori".to_string()),
            message: diagnostic_message(d, None),
            related_information: None,
            tags: None,
            data: None,
        })
        .collect()
}

fn label_points_to_path(cache: &SourceCache, label: &Label, target: &Path) -> bool {
    cache
        .get(label.file_id)
        .map(|file| uri::canonical_path(&file.path) == target)
        .unwrap_or(false)
}

fn range_for_label(cache: &SourceCache, label: &Label) -> Range {
    let Some(file) = cache.get(label.file_id) else {
        return position::default_range();
    };
    let content_len = file.content.len() as u32;
    let start = label.span.start.min(content_len);
    let mut end = label.span.end.min(content_len);
    if end <= start && start < content_len {
        end = start + 1;
    }

    Range::new(
        position::position_for_byte_offset(&file.content, start as usize),
        position::position_for_byte_offset(&file.content, end as usize),
    )
}

fn diagnostic_message(diagnostic: &OriDiagnostic, label: Option<&Label>) -> String {
    let mut message = diagnostic.message.clone();
    if let Some(label) = label {
        if !label.message.is_empty() {
            message.push('\n');
            message.push_str(&label.message);
        }
    }
    if let Some(why) = &diagnostic.why {
        message.push_str("\nwhy: ");
        message.push_str(why);
    }
    if let Some(action) = &diagnostic.action {
        message.push_str("\naction: ");
        message.push_str(action);
    }
    for note in &diagnostic.notes {
        message.push_str("\nnote: ");
        message.push_str(note);
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_error_diagnostic_maps_known_messages() {
        // Missing manifest → project.no_proj_file
        let d = project_error_diagnostic("project manifest `/foo/ori.proj` not found")
            .expect("missing manifest maps to a project diagnostic");
        assert_eq!(
            d.code.as_ref().and_then(|c| match c {
                NumberOrString::String(s) => Some(s.clone()),
                _ => None,
            }),
            Some("project.no_proj_file".to_string())
        );

        // Manifest without an entry key → project.entry_not_found
        let d = project_error_diagnostic("project manifest `/foo/ori.proj` is missing `entry`")
            .expect("manifest missing entry maps to a project diagnostic");
        let code = d.code.and_then(|c| match c {
            NumberOrString::String(s) => Some(s),
            _ => None,
        });
        assert_eq!(code.as_deref(), Some("project.entry_not_found"));

        // Entry points to a non-existent file → project.entry_not_found
        let d = project_error_diagnostic("project entry `/foo/main.orl` does not exist")
            .expect("missing entry maps to a project diagnostic");
        let code = d.code.and_then(|c| match c {
            NumberOrString::String(s) => Some(s),
            _ => None,
        });
        assert_eq!(code.as_deref(), Some("project.entry_not_found"));
    }

    #[test]
    fn project_error_diagnostic_returns_none_for_unknown_messages() {
        assert!(project_error_diagnostic("some unrelated io error").is_none());
        assert!(project_error_diagnostic("file not found: foo.orl").is_none());
    }
}
