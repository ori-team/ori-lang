use std::path::PathBuf;

#[test]
fn test_find_stdlib_root_resolves_via_cwd_fallback() {
    let original_cwd = std::env::current_dir().unwrap();
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let subdir = manifest_root.join("src");
    std::env::set_current_dir(&subdir).unwrap();

    let old_env = std::env::var("ORI_STDLIB_ROOT").ok();
    std::env::remove_var("ORI_STDLIB_ROOT");

    let root = ori_driver::pipeline::find_stdlib_root();

    if let Some(val) = old_env {
        std::env::set_var("ORI_STDLIB_ROOT", val);
    }
    std::env::set_current_dir(original_cwd).unwrap();

    assert!(
        root.is_some(),
        "Should find stdlib using cwd fallback upward search"
    );
    let root_path = root.unwrap();
    assert!(
        root_path.join("string.orl").is_file(),
        "stdlib root should contain 'string.orl' file"
    );
}
