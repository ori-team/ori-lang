//! Regression: `ori doc export` emits JSON for the documentation website.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn ori_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ori"))
}

#[test]
fn doc_export_cli_writes_valid_json() {
    let out = std::env::temp_dir().join(format!("ori_doc_export_test_{}.json", std::process::id()));
    let _ = std::fs::remove_file(&out);

    let output = Command::new(ori_exe())
        .args(["doc", "export", "--out"])
        .arg(&out)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn `ori doc export`");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ori doc export failed: status={:?} stderr={stderr}",
        output.status
    );
    assert!(out.is_file(), "expected export file at {}", out.display());

    let json = std::fs::read_to_string(&out).expect("read export json");
    assert!(json.contains("\"symbols\""));
    assert!(json.contains("\"errors\""));
    assert!(json.contains("\"keywords\""));
    assert!(json.contains("\"ori.io.print\""));

    let _ = std::fs::remove_file(&out);
}
