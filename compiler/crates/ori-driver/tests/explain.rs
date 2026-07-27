//! Regression: `ori explain <code>` prints actionable guidance.

use ori_driver::explain::{explain_code, format_explanation};

#[test]
fn explain_covers_gate_codes() {
    for code in [
        "name.undefined",
        "project.circular_import",
        "type.type_mismatch",
    ] {
        let entry = explain_code(code).unwrap_or_else(|| panic!("no entry for {code}"));
        let text = format_explanation(entry);
        assert!(text.contains(code));
        assert!(text.contains("cause:"));
        assert!(text.contains("fix:"));
    }
}

#[test]
fn explain_unknown_code_returns_none() {
    assert!(explain_code("not.a.real.code").is_none());
}
