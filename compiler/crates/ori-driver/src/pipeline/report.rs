//! Environment diagnostics and lightweight project summaries.

use std::path::{Path, PathBuf};

use ori_diagnostics::DiagnosticSink;

use super::frontend::run_check;
use super::project::find_stdlib_root;
use super::runtime::{
    env_flag, find_native_runtime_cdylib, find_native_runtime_link, native_target_triple,
    should_use_jit_for_run,
};
// ── Doctor ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == DoctorStatus::Fail)
    }
}

/// Environment and toolchain sanity checks for the Ori native pipeline.
pub fn run_doctor() -> DoctorReport {
    let mut checks = Vec::new();

    match find_stdlib_root() {
        Some(path) => {
            let orl_count = count_orl_files(&path);
            checks.push(DoctorCheck {
                name: "stdlib root",
                status: DoctorStatus::Ok,
                detail: format!("{} ({} `.orl` modules)", path.display(), orl_count),
            });
        }
        None => checks.push(DoctorCheck {
            name: "stdlib root",
            status: DoctorStatus::Fail,
            detail: "not found — set ORI_STDLIB_ROOT or install the packaged stdlib/ layout".into(),
        }),
    }

    let target = native_target_triple();
    checks.push(DoctorCheck {
        name: "target triple",
        status: DoctorStatus::Ok,
        detail: target,
    });

    match find_native_runtime_link() {
        Ok(link) => checks.push(DoctorCheck {
            name: "native runtime (AOT)",
            status: DoctorStatus::Ok,
            detail: link.runtime_lib.display().to_string(),
        }),
        Err(err) => checks.push(DoctorCheck {
            name: "native runtime (AOT)",
            status: DoctorStatus::Fail,
            detail: err,
        }),
    }

    match find_native_runtime_cdylib() {
        Ok(path) => checks.push(DoctorCheck {
            name: "native runtime (JIT cdylib)",
            status: DoctorStatus::Ok,
            detail: path.display().to_string(),
        }),
        Err(err) => checks.push(DoctorCheck {
            name: "native runtime (JIT cdylib)",
            status: DoctorStatus::Warn,
            detail: format!("{err} (ori run falls back to AOT when unset)"),
        }),
    }

    let linker_detail = match ori_codegen::NativeLinker::discover() {
        Ok(linker) => {
            let name = linker.strategy_name();
            let suffix = if env_flag("ORI_NATIVE_LINKER") {
                " (ORI_NATIVE_LINKER)"
            } else if env_flag("ORI_USE_BUNDLED_RUST_LLD") {
                " (ORI_USE_BUNDLED_RUST_LLD=1)"
            } else if env_flag("ORI_USE_SYSTEM_LINKER") {
                " (ORI_USE_SYSTEM_LINKER=1)"
            } else if env_flag("ORI_USE_RUSTC_DRIVER") {
                " (ORI_USE_RUSTC_DRIVER=1)"
            } else {
                " (default)"
            };
            (DoctorStatus::Ok, format!("{name}{suffix}"))
        }
        Err(err) => (
            DoctorStatus::Warn,
            format!("{err} (AOT compile will fail until resolved)"),
        ),
    };
    checks.push(DoctorCheck {
        name: "linker strategy",
        status: linker_detail.0,
        detail: linker_detail.1,
    });

    let run_mode = if should_use_jit_for_run() {
        "JIT (in-process Cranelift)"
    } else {
        "AOT compile + link"
    };
    checks.push(DoctorCheck {
        name: "ori run mode",
        status: DoctorStatus::Ok,
        detail: run_mode.into(),
    });

    if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") {
        checks.push(DoctorCheck {
            name: "packaged runtime gate",
            status: DoctorStatus::Ok,
            detail: "ORI_REQUIRE_PACKAGED_RUNTIME=1 (release smoke mode)".into(),
        });
    }

    DoctorReport { checks }
}

// ── Project summary ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SummaryImport {
    pub path: String,
    pub alias: Option<String>,
    pub selected: Vec<SummaryImportItem>,
}

#[derive(Debug, Clone)]
pub struct SummaryImportItem {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SummaryModule {
    pub path: PathBuf,
    pub namespace: String,
    pub imports: Vec<SummaryImport>,
}

#[derive(Debug, Clone)]
pub struct SummaryOutput {
    pub entry: PathBuf,
    pub modules: Vec<SummaryModule>,
    pub diagnostic_count: usize,
}

/// Build a lightweight project overview: entry file, namespaces, import graph.
pub fn run_summary(path: &Path) -> Result<SummaryOutput, String> {
    let output = run_check(path)?;
    let entry = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut modules = Vec::new();

    for file in output.cache.all_files() {
        let mut sink = DiagnosticSink::default();
        let tokens = ori_lexer::lex(&file.content, file.id, &mut sink);
        let ast = ori_parser::parse(&tokens, &file.content, file.id, &mut sink);
        let imports = ast
            .imports
            .iter()
            .map(|import| SummaryImport {
                path: import.path.to_string(),
                alias: import.alias.as_ref().map(|alias| alias.text.to_string()),
                selected: import
                    .selected
                    .iter()
                    .map(|item| SummaryImportItem {
                        name: item.name.text.to_string(),
                        alias: item.alias.as_ref().map(|alias| alias.text.to_string()),
                    })
                    .collect(),
            })
            .collect();
        modules.push(SummaryModule {
            path: file.path.clone(),
            namespace: ast.namespace.name.to_string(),
            imports,
        });
    }

    modules.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(SummaryOutput {
        entry,
        modules,
        diagnostic_count: output.diagnostics.len(),
    })
}

pub fn format_summary_text(summary: &SummaryOutput) -> String {
    let mut out = String::new();
    out.push_str(&format!("entry: {}\n", summary.entry.display()));
    out.push_str(&format!(
        "modules: {} ({} diagnostic(s) from last check)\n\n",
        summary.modules.len(),
        summary.diagnostic_count
    ));
    for module in &summary.modules {
        out.push_str(&format!(
            "- {} → namespace {}\n",
            module.path.display(),
            module.namespace
        ));
        for import in &module.imports {
            if !import.selected.is_empty() {
                let selected = import
                    .selected
                    .iter()
                    .map(|item| match &item.alias {
                        Some(alias) => format!("{} = {}", item.name, alias),
                        None => item.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("    import {} ({})\n", import.path, selected));
            } else if let Some(alias) = &import.alias {
                out.push_str(&format!("    import {} = {}\n", import.path, alias));
            } else {
                out.push_str(&format!("    import {}\n", import.path));
            }
        }
    }
    out
}

fn count_orl_files(root: &Path) -> usize {
    fn walk(dir: &Path, count: &mut usize) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, count);
            } else if path.extension().is_some_and(|extension| extension == "orl") {
                *count += 1;
            }
        }
    }
    let mut count = 0;
    walk(root, &mut count);
    count
}
