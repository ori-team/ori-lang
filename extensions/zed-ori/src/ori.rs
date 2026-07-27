use zed_extension_api::{self as zed, LanguageServerId, Result};

struct OriExtension;

impl zed::Extension for OriExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // Prefer PATH; monorepo users should put compiler/target/debug on PATH
        // or install a release `ori-lsp` next to `ori`.
        let command = worktree.which("ori-lsp").ok_or_else(|| {
            "ori-lsp not found on PATH. From the Ori monorepo: \
             `cd compiler && cargo build -p ori-lsp` and add `compiler/target/debug` to PATH. \
             Or install Ori so `ori-lsp` is available."
                .to_string()
        })?;

        let mut env = Vec::new();
        // If the worktree looks like the Ori monorepo, point stdlib at it.
        if let Some(stdlib) = worktree_stdlib_root(worktree) {
            env.push(("ORI_STDLIB_ROOT".to_string(), stdlib));
        }

        Ok(zed::Command {
            command,
            args: Vec::new(),
            env,
        })
    }
}

/// Detect `stdlib/` at the worktree root or `../stdlib` when opened on `compiler/`.
fn worktree_stdlib_root(worktree: &zed::Worktree) -> Option<String> {
    // worktree.read_text_file fails if missing; we only probe known relative paths
    // via `which` is not applicable — try reading a marker file.
    const CANDIDATES: &[&str] = &[
        "stdlib/io.orl",
        "stdlib/README.md",
        "../stdlib/io.orl",
        "compiler/../stdlib/io.orl",
    ];
    for rel in CANDIDATES {
        if worktree.read_text_file(rel).is_ok() {
            // Derive directory of the marker.
            let dir = rel
                .trim_end_matches("io.orl")
                .trim_end_matches("README.md")
                .trim_end_matches('/')
                .to_string();
            if !dir.is_empty() {
                return Some(dir);
            }
        }
    }
    None
}

zed::register_extension!(OriExtension);
