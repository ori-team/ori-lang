//! Incremental compilation invalidation suite.
//!
//! Validates that the native content-addressed module object cache (.ori/incremental.json)
//! correctly detects interface and body changes:
//! - Changing a shared public function interface invalidates downstream callers
//! - Rebuilding with identical sources produces a full cache hit (reused = true)
//! - `ORI_DISABLE_INCREMENTAL=1` bypasses cached objects completely

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::pipeline::run_compile;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_incr_inv_{}_{}_{}",
            std::process::id(),
            id,
            name,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write(&self, name: &str, source: &str) {
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

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

#[test]
fn incremental_cache_hit_on_unchanged_sources() {
    let dir = TestDir::new("incr_cache_hit");
    dir.write(
        "util.orl",
        r#"module app.util

public compute() -> int
    return 100
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.util as util

main()
    const v: int = util.compute()
end
"#,
    );

    let output = exe_path(&dir, "cache_hit_bin");
    let first = run_compile(&dir.path("main.orl"), &output).expect("first build");
    assert!(!first.has_errors);
    assert!(!first.reused, "first build is not reused");

    // Second build without source edits must hit full cache
    let second = run_compile(&dir.path("main.orl"), &output).expect("second build");
    assert!(!second.has_errors);
    assert!(
        second.reused,
        "second build with identical sources must be a full cache hit"
    );
}

#[test]
fn incremental_interface_change_invalidates_downstream_caller() {
    let dir = TestDir::new("incr_iface_inv");
    dir.write(
        "service.orl",
        r#"module app.service

public get_code() -> int
    return 7
end
"#,
    );
    dir.write(
        "main.orl",
        r#"module app.main

import app.service as s

main()
    const code: int = s.get_code()
end
"#,
    );

    let output = exe_path(&dir, "iface_inv_bin");
    let first = run_compile(&dir.path("main.orl"), &output).expect("first build");
    assert!(!first.has_errors);

    let record_path = dir.path(".ori/incremental.json");
    assert!(record_path.is_file(), "incremental record exists");

    // Modify the public return type interface from `int` to `string`
    // This must invalidate downstream caller `main.orl` and trigger a compile error
    // because `const code: int = s.get_code()` now receives a string!
    dir.write(
        "service.orl",
        r#"module app.service

public get_code() -> string
    return "SEVEN"
end
"#,
    );

    let second = run_compile(&dir.path("main.orl"), &output).expect("second build");
    assert!(
        second.has_errors,
        "caller should fail type-checking because get_code interface changed to string"
    );
    assert!(
        second
            .diagnostics
            .iter()
            .any(|d| d.code == "type.type_mismatch"),
        "expected type.type_mismatch diagnostic: {:?}",
        second.diagnostics
    );
}

#[test]
fn incremental_rebuild_bypassed_when_disabled() {
    let dir = TestDir::new("incr_disabled");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    const x: int = 42
end
"#,
    );

    let output = exe_path(&dir, "disabled_bin");
    std::env::set_var("ORI_DISABLE_INCREMENTAL", "1");
    let build = run_compile(&dir.path("main.orl"), &output).expect("build");
    std::env::remove_var("ORI_DISABLE_INCREMENTAL");

    assert!(!build.has_errors);
    assert!(!build.reused);

    // .ori directory should not have written cache
    let record_path = dir.path(".ori/incremental.json");
    assert!(
        !record_path.is_file(),
        "incremental cache should not be created when disabled"
    );
}
