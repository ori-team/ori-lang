//! Native debug metadata emitted after linking.
//!
//! Cranelift 0.131 deliberately leaves DWARF generation to embedders.  Ori
//! therefore emits a compact DWARF v4 compilation unit from the final native
//! symbol table.  The addresses are absolute at this point, which keeps the
//! generated line table valid for the linked executable on ELF platforms.

use gimli::write::{
    Address, AttributeValue, Dwarf, EndianVec, LineProgram, LineString, Sections, Unit,
};
use gimli::{constants, Encoding, Format, LineEncoding, LittleEndian};
use object::{Object, ObjectSymbol, SymbolKind};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFunction {
    pub name: String,
    pub source: PathBuf,
    pub line: u64,
    pub variables: Vec<DebugVariable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariable {
    pub name: String,
    pub ty: String,
    pub line: u64,
}

/// Emit native debug metadata and return the produced artifact, if the host
/// has a compatible object-copy utility.  ELF receives real `.debug_*`
/// sections; other targets still receive a JSON source map beside the binary
/// so editors and the Ori debugger can inspect the same function/line data.
pub fn emit_native_debug_symbols(
    executable: &Path,
    functions: &[DebugFunction],
) -> Result<Option<PathBuf>, String> {
    let map_path = executable.with_extension("debug.json");
    write_debug_map(&map_path, functions)?;
    if !cfg!(target_os = "linux") {
        // The linker owns PDB generation on Windows. Returning the concrete
        // path when it exists lets the CLI and editor surface the artifact;
        // the JSON map remains the portable fallback when the linker cannot
        // emit CodeView/PDB metadata.
        if cfg!(windows) {
            let pdb_path = executable.with_extension("pdb");
            if pdb_path.is_file() {
                return Ok(Some(pdb_path));
            }
        }
        return Ok(Some(map_path));
    }
    let dwarf = build_dwarf_sections(executable, functions)?;
    let Some(objcopy) = find_objcopy() else {
        return Ok(Some(map_path));
    };
    let mut section_files = Vec::new();
    for (name, bytes) in dwarf {
        if bytes.is_empty() {
            continue;
        }
        let path = std::env::temp_dir().join(format!(
            "ori-debug-{}-{}",
            std::process::id(),
            name.trim_start_matches('.')
        ));
        fs::write(&path, bytes)
            .map_err(|err| format!("debug_symbols.write_failed: {}: {err}", path.display()))?;
        section_files.push((name, path));
    }
    if section_files.is_empty() {
        return Ok(Some(map_path));
    }
    let output = executable.with_extension("debug.tmp");
    let mut command = Command::new(&objcopy);
    // The runtime staticlib may already contribute Rust `.debug_*` sections.
    // Remove those sections first so the Ori compilation unit can occupy the
    // canonical DWARF names consumed by GDB/LLDB.
    for name in [
        ".debug_abbrev",
        ".debug_aranges",
        ".debug_gdb_scripts",
        ".debug_info",
        ".debug_line",
        ".debug_line_str",
        ".debug_loc",
        ".debug_macro",
        ".debug_ranges",
        ".debug_rnglists",
        ".debug_str",
        ".debug_str_offsets",
    ] {
        command.arg("--remove-section").arg(name);
    }
    for (name, path) in &section_files {
        command
            .arg("--add-section")
            .arg(format!("{name}={}", path.display()))
            .arg("--set-section-flags")
            .arg(format!("{name}=readonly,debug"));
    }
    command.arg(executable).arg(&output);
    let result = command.output().map_err(|err| {
        format!(
            "debug_symbols.objcopy_failed: cannot invoke `{}`: {err}",
            objcopy.display()
        )
    });
    for (_, path) in section_files {
        let _ = fs::remove_file(path);
    }
    let result = result?;
    if !result.status.success() {
        let detail = String::from_utf8_lossy(&result.stderr);
        let _ = fs::remove_file(&output);
        return Err(format!(
            "debug_symbols.objcopy_failed: `{}` exited with {}: {}",
            objcopy.display(),
            result.status,
            detail.trim()
        ));
    }
    fs::rename(&output, executable).map_err(|err| {
        format!(
            "debug_symbols.replace_failed: cannot replace `{}`: {err}",
            executable.display()
        )
    })?;
    Ok(Some(executable.to_path_buf()))
}

fn write_debug_map(path: &Path, functions: &[DebugFunction]) -> Result<(), String> {
    let mut entries = Vec::with_capacity(functions.len());
    for function in functions {
        entries.push(serde_json::json!({
            "name": function.name,
            "source": function.source,
            "line": function.line,
            "variables": function.variables.iter().map(|variable| serde_json::json!({
                "name": variable.name,
                "type": variable.ty,
                "line": variable.line,
            })).collect::<Vec<_>>(),
        }));
    }
    let body = serde_json::to_vec_pretty(&serde_json::json!({
        "format": 1,
        "functions": entries,
    }))
    .map_err(|err| format!("debug_symbols.serialize_failed: {err}"))?;
    fs::write(path, body).map_err(|err| {
        format!(
            "debug_symbols.write_failed: cannot write `{}`: {err}",
            path.display()
        )
    })
}

fn build_dwarf_sections(
    executable: &Path,
    functions: &[DebugFunction],
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let bytes = fs::read(executable).map_err(|err| {
        format!(
            "debug_symbols.read_failed: cannot read `{}`: {err}",
            executable.display()
        )
    })?;
    let object = object::File::parse(&*bytes)
        .map_err(|err| format!("debug_symbols.object_parse_failed: {err}"))?;
    let mut symbols = HashMap::new();
    for symbol in object.symbols() {
        if symbol.kind() != SymbolKind::Text {
            continue;
        }
        let Ok(name) = symbol.name() else { continue };
        if name.starts_with("ORI__") || name == "main" {
            symbols.insert(name.to_string(), (symbol.address(), symbol.size()));
        }
    }
    let mut located = Vec::new();
    for function in functions {
        let symbol_name = format!("ORI__{}", mangle_symbol(&function.name));
        let Some(&(address, size)) = symbols.get(&symbol_name) else {
            continue;
        };
        if size == 0 {
            continue;
        }
        located.push((address, size, function.clone()));
    }
    // DWARF line sequences must be monotonic in address order. HIR order is
    // source order, while the linker is free to reorder functions.
    located.sort_by_key(|(address, _, _)| *address);
    let Some((first_address, _, first_function)) = located.first() else {
        return Ok(Vec::new());
    };

    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 8,
    };
    let line_encoding = LineEncoding::default();
    let mut line_program = LineProgram::new(
        encoding,
        line_encoding,
        LineString::String(b".".to_vec()),
        None,
        LineString::String(first_function.source.to_string_lossy().as_bytes().to_vec()),
        None,
    );
    let working_dir = line_program.add_directory(LineString::String(b".".to_vec()));
    let mut file_ids = HashMap::new();
    for (_, _, function) in &located {
        if file_ids.contains_key(&function.source) {
            continue;
        }
        let file_id = line_program.add_file(
            LineString::String(function.source.to_string_lossy().as_bytes().to_vec()),
            working_dir,
            None,
        );
        file_ids.insert(function.source.clone(), file_id);
    }
    for (address, size, function) in &located {
        line_program.begin_sequence(Some(Address::Constant(*address)));
        let row = line_program.row();
        row.address_offset = 0;
        row.file = file_ids[&function.source];
        row.line = function.line.max(1);
        row.is_statement = true;
        row.prologue_end = true;
        line_program.generate_row();
        line_program.end_sequence(*size);
    }

    let mut dwarf = Dwarf::new();
    let unit_id = dwarf.units.add(Unit::new(encoding, line_program));
    let root = dwarf.units.get_mut(unit_id).root();
    let unit = dwarf.units.get_mut(unit_id);
    unit.get_mut(root).set(
        constants::DW_AT_name,
        AttributeValue::String(executable.to_string_lossy().as_bytes().to_vec()),
    );
    unit.get_mut(root).set(
        constants::DW_AT_producer,
        AttributeValue::String(b"Ori native backend".to_vec()),
    );
    unit.get_mut(root).set(
        constants::DW_AT_language,
        AttributeValue::Language(constants::DW_LANG_C),
    );
    unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(*first_address)),
    );
    let last = located
        .iter()
        .map(|(address, size, _)| address.saturating_add(*size))
        .max()
        .unwrap_or(*first_address);
    unit.get_mut(root).set(
        constants::DW_AT_high_pc,
        AttributeValue::Address(Address::Constant(last)),
    );

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf
        .write(&mut sections)
        .map_err(|err| format!("debug_symbols.dwarf_write_failed: {err:?}"))?;
    Ok(vec![
        (".debug_abbrev", sections.debug_abbrev.0.into_vec()),
        (".debug_info", sections.debug_info.0.into_vec()),
        (".debug_line", sections.debug_line.0.into_vec()),
        (".debug_str", sections.debug_str.0.into_vec()),
    ])
}

fn find_objcopy() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ORI_OBJCOPY") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    ["objcopy", "llvm-objcopy"].iter().find_map(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|_| PathBuf::from(candidate))
    })
}

fn mangle_symbol(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 2);
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if ch == '.' {
            out.push_str("_dot_");
        } else {
            use std::fmt::Write;
            let _ = write!(out, "_x{:02x}_", ch as u32);
        }
    }
    out
}
