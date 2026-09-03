//! JSON export for the Ori documentation website (`ori doc export`).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ori_ast::item::{FuncDecl, Item, ItemWithAttrs};
use ori_ast::ty::Type;
use ori_diagnostics::{DiagnosticSink, FileId};
use ori_types::stdlib::{implemented_stdlib_modules, stdlib_func_sig, stdlib_runtime_functions};
use serde_json;

use crate::explain;
use crate::pipeline::{find_stdlib_root, stdlib_doc_signature};

#[derive(Debug, serde::Serialize)]
pub struct ExportSymbol {
    id: String,
    kind: String,
    module: String,
    name: String,
    signature: String,
    layer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    aliases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExportError {
    code: String,
    severity: String,
    summary: String,
    cause: String,
    fix: String,
}

#[derive(Debug, serde::Serialize)]
pub struct DocExport {
    version: String,
    generated_at: String,
    modules: Vec<String>,
    symbols: Vec<ExportSymbol>,
    errors: Vec<ExportError>,
    keywords: Vec<String>,
}

fn format_ty_sig(path: &str) -> Option<String> {
    let (params, ret) = stdlib_func_sig(path)?;
    let params_str = if params.is_empty() {
        String::new()
    } else {
        params
            .iter()
            .enumerate()
            .map(|(i, ty)| format!("arg{i}: {}", ty.display()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Some(format!("({params_str}) -> {}", ret.display()))
}

fn build_runtime_symbols() -> (Vec<ExportSymbol>, BTreeSet<String>) {
    let mut symbols = Vec::new();
    let mut modules = BTreeSet::new();

    for entry in stdlib_runtime_functions() {
        let canonical = entry.canonical_path.to_string();
        let (module, name) = canonical
            .rsplit_once('.')
            .map(|(m, n)| (m.to_string(), n.to_string()))
            .unwrap_or_else(|| (canonical.clone(), canonical.clone()));

        modules.insert(module.clone());

        let signature = stdlib_doc_signature(&canonical)
            .map(str::to_string)
            .or_else(|| format_ty_sig(&canonical))
            .unwrap_or_else(|| format!("{name}(...)"));

        let aliases: Vec<String> = entry.aliases.iter().map(|s| (*s).to_string()).collect();

        symbols.push(ExportSymbol {
            id: canonical.clone(),
            kind: "function".into(),
            module: module.clone(),
            name,
            signature,
            layer: "runtime".into(),
            aliases: if aliases.is_empty() {
                None
            } else {
                Some(aliases)
            },
            source: None,
        });
    }

    for module in implemented_stdlib_modules() {
        modules.insert(module.to_string());
    }

    (symbols, modules)
}

fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Named(q) => q.to_string(),
        Type::Optional(t, _) => format!("optional[{}]", type_to_string(t)),
        Type::Result(ok, err, _) => {
            format!("result[{}, {}]", type_to_string(ok), type_to_string(err))
        }
        Type::List(t, _) => format!("list[{}]", type_to_string(t)),
        Type::Map(k, v, _) => format!("map[{}, {}]", type_to_string(k), type_to_string(v)),
        Type::Set(t, _) => format!("set[{}]", type_to_string(t)),
        Type::Bool(_) => "bool".into(),
        Type::Int(_) => "int".into(),
        Type::Float(_) => "float".into(),
        Type::String(_) => "string".into(),
        Type::Bytes(_) => "bytes".into(),
        Type::Void(_) => "void".into(),
        other => format!("{other:?}"),
    }
}

fn func_signature_text(func: &FuncDecl) -> String {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.text, type_to_string(&p.ty)))
        .collect();
    let ret = func
        .return_ty
        .as_ref()
        .map(|t| format!(" -> {}", type_to_string(t)))
        .unwrap_or_default();
    let mut prefix = String::new();
    if func.is_async {
        prefix.push_str("async ");
    }
    if func.is_mut {
        prefix.push_str("mut ");
    }
    format!("{}{}({}){}", prefix, func.name.text, params.join(", "), ret)
}

fn scan_orl_file(
    path: &Path,
    stdlib_root: &Path,
    catalog: &mut Vec<ExportSymbol>,
    modules: &mut BTreeSet<String>,
) -> Result<(), String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let file_id = FileId(0);
    let mut sink = DiagnosticSink::default();
    let tokens = ori_lexer::lex(&content, file_id, &mut sink);
    let mut source_file = ori_parser::parse(&tokens, &content, file_id, &mut sink);
    crate::pipeline::filter_intrinsic_source_for_current_configuration(
        &mut source_file,
        file_id,
        &mut sink,
    )?;
    let namespace = source_file.namespace.name.to_string();
    modules.insert(namespace.clone());

    let rel_source = path
        .strip_prefix(stdlib_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"));

    for ItemWithAttrs { item, .. } in &source_file.items {
        let Item::Func(func) = item else { continue };
        if !func.visibility.is_public() {
            continue;
        }
        let qualified = format!("{}.{}", namespace, func.name.text);
        catalog.push(ExportSymbol {
            id: qualified,
            kind: "function".into(),
            module: namespace.clone(),
            name: func.name.text.to_string(),
            signature: func_signature_text(func),
            layer: "orl".into(),
            aliases: None,
            source: rel_source.clone(),
        });
    }
    Ok(())
}

fn scan_orl_layer(
    root: &Path,
    catalog: &mut Vec<ExportSymbol>,
    modules: &mut BTreeSet<String>,
) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "orl") {
                scan_orl_file(&path, root, catalog, modules)?;
            }
        }
    }
    Ok(())
}

const KEYWORDS: &[&str] = &[
    "module", "import",
    // Callable-type keyword only; declarations use bare `name(...)`.
    "func", "struct", "enum", "trait", "const", "var", "public", "async", "if", "else", "then",
    "end", "match", "case", "while", "for", "loop", "break", "continue", "return", "async",
    "await", "try", "using", "some", "none", "ok", "err", "true", "false", "is", "as", "only",
    "where", "type", "lazy", "spawn", "defer",
];

/// Build the full documentation export payload.
pub fn build_doc_export() -> Result<DocExport, String> {
    let (mut symbols, mut modules) = build_runtime_symbols();

    if let Some(root) = find_stdlib_root() {
        scan_orl_layer(&root, &mut symbols, &mut modules)?;
    }

    symbols.sort_by(|a, b| a.id.cmp(&b.id));

    let errors = explain::explained_codes()
        .map(|e| ExportError {
            code: e.code.to_string(),
            severity: e.severity.to_string(),
            summary: e.summary.to_string(),
            cause: e.cause.to_string(),
            fix: e.fix.to_string(),
        })
        .collect();

    Ok(DocExport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at: chrono_lite_timestamp(),
        modules: modules.into_iter().collect(),
        symbols,
        errors,
        keywords: KEYWORDS.iter().map(|k| (*k).to_string()).collect(),
    })
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Serialize documentation export to pretty JSON.
pub fn export_doc_json() -> Result<String, String> {
    let export = build_doc_export()?;
    serde_json::to_string_pretty(&export).map_err(|e| format!("json encode failed: {e}"))
}

/// Write documentation export to a file.
pub fn write_doc_export(path: &Path) -> Result<(), String> {
    let json = export_doc_json()?;
    // A bare file name yields an empty parent, which `create_dir_all` rejects.
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_contains_runtime_and_errors() {
        let export = build_doc_export().expect("build documentation export");
        assert!(export.symbols.iter().any(|s| s.id == "ori.io.print"));
        assert!(export.errors.iter().any(|e| e.code == "name.undefined"));
        assert!(export.modules.iter().any(|m| m == "ori.fs"));
    }
}
