//! Regression: `ori summary` lists project modules and imports.

#[test]
fn summary_lists_transitive_imports() {
    let dir = std::env::temp_dir().join(format!("ori-summary-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let lib = dir.join("lib.orl");
    std::fs::write(&lib, "module app.lib\nstruct Point\n    x: int\nend\n").unwrap();
    let main = dir.join("main.orl");
    std::fs::write(
        &main,
        "module app.main\nimport app.lib = lib\nmain() -> void\nend\n",
    )
    .unwrap();

    let summary = ori_driver::pipeline::run_summary(&main).expect("summary");
    assert!(summary.modules.len() >= 2, "expected entry + import");
    let text = ori_driver::pipeline::format_summary_text(&summary);
    assert!(text.contains("app.main"));
    assert!(text.contains("app.lib"));
    assert!(text.contains("import app.lib"));
}
