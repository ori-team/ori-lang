use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use ori_diagnostics::{Diagnostic, SourceCache};
use ori_driver::pipeline::CheckOutput;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_quality_test_{}_{}_{}",
            std::process::id(),
            id,
            name,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    pub fn write(&self, name: &str, source: &str) {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, source).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[allow(dead_code)]
pub fn diagnostic_codes(out: &CheckOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code).collect()
}

pub fn assert_check_output_is_well_formed(out: &CheckOutput) {
    assert_diagnostic_spans_within_sources(&out.cache, &out.diagnostics);
}

pub fn assert_diagnostic_spans_within_sources(cache: &SourceCache, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        for label in &diagnostic.labels {
            let file = cache.get(label.file_id).unwrap_or_else(|| {
                panic!(
                    "diagnostic `{}` references missing file id {:?}",
                    diagnostic.code, label.file_id
                )
            });
            let len = file.content.len() as u32;
            assert!(
                label.span.start <= label.span.end,
                "diagnostic `{}` has inverted span {} in {}",
                diagnostic.code,
                label.span,
                file.path.display()
            );
            assert!(
                label.span.end <= len,
                "diagnostic `{}` span {} is outside {} bytes in {}",
                diagnostic.code,
                label.span,
                len,
                file.path.display()
            );
            assert!(
                file.content.is_char_boundary(label.span.start as usize)
                    && file.content.is_char_boundary(label.span.end as usize),
                "diagnostic `{}` span {} splits a UTF-8 character in {}",
                diagnostic.code,
                label.span,
                file.path.display()
            );
        }
    }
}

#[allow(dead_code)]
pub fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

#[allow(dead_code)]
pub fn ori_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ori"))
}

#[allow(dead_code)]
pub fn run_ori(args: &[&str]) -> std::process::Output {
    Command::new(ori_exe())
        .args(args)
        .output()
        .expect("failed to spawn `ori` subprocess")
}

pub fn normalize_stdout(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap().replace("\r\n", "\n")
}
