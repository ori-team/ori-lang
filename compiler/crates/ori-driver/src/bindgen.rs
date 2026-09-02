use std::fs;
use std::path::Path;

/// Generate low-level Ori `extern "c"` declarations and `@repr("C")` structs
/// from a C header file (FFI-BINDGEN-1).
pub fn generate_bindings(header_path: &Path, module_name: Option<&str>) -> Result<String, String> {
    let content = fs::read_to_string(header_path)
        .map_err(|e| format!("failed to read C header '{}': {e}", header_path.display()))?;

    let stem = header_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bindings");
    let mod_name = module_name.unwrap_or(stem);

    let mut out = String::new();
    out.push_str(&format!("module {}\n\n", mod_name));

    let mut constants = Vec::new();
    let mut typedefs = Vec::new();
    let mut structs = Vec::new();
    let mut functions = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;

        if line.is_empty() || line.starts_with("//") || line.starts_with("/*") {
            continue;
        }

        // #define NAME VALUE
        if let Some(rest) = line.strip_prefix("#define ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0];
                let val = parts[1];
                if let Ok(num) = val.parse::<i64>() {
                    constants.push(format!("public const {name}: int = {num}"));
                }
            }
            continue;
        }

        // typedef struct Name { ... } Name; or struct Name { ... };
        if (line.starts_with("typedef struct") || line.starts_with("struct ")) && line.contains('{')
        {
            let mut struct_name = "";
            let header_parts: Vec<&str> = line.split('{').collect();
            let prefix = header_parts[0].trim();
            for word in prefix.split_whitespace() {
                if word != "typedef" && word != "struct" {
                    struct_name = word;
                }
            }

            let mut fields = Vec::new();
            while i < lines.len() {
                let s_line = lines[i].trim();
                i += 1;
                if let Some(rest) = s_line.strip_prefix('}') {
                    let name = rest.trim_end_matches(';').trim();
                    if !name.is_empty() {
                        struct_name = name;
                    }
                    break;
                }
                let clean = s_line.trim_end_matches(';').trim();
                if clean.is_empty() || clean.starts_with("//") {
                    continue;
                }
                let f_parts: Vec<&str> = clean.split_whitespace().collect();
                if f_parts.len() >= 2 {
                    let f_type = map_c_type(f_parts[0]);
                    let f_name = f_parts[1].trim_start_matches('*');
                    fields.push(format!("    {f_name}: {f_type}"));
                }
            }

            if !struct_name.is_empty() {
                let mut struct_decl = String::new();
                struct_decl.push_str("@repr(\"C\")\n");
                struct_decl.push_str(&format!("public struct {struct_name}\n"));
                for field in fields {
                    struct_decl.push_str(&format!("{field}\n"));
                }
                struct_decl.push_str("end\n");
                structs.push(struct_decl);
            }
            continue;
        }

        // typedef alias
        if let Some(rest) = line.strip_prefix("typedef ") {
            if line.ends_with(';') && !line.contains('(') {
                let clean = rest.trim_end_matches(';').trim();
                let parts: Vec<&str> = clean.split_whitespace().collect();
                if parts.len() >= 2 {
                    let c_type = map_c_type(parts[0]);
                    let alias_name = parts[1];
                    typedefs.push(format!("public alias {alias_name} = {c_type}"));
                }
                continue;
            }
        }

        // Function declaration: ret_type func_name(args);
        if line.ends_with(';') && line.contains('(') && line.contains(')') {
            let clean = line.trim_end_matches(';').trim();
            if let Some(paren_idx) = clean.find('(') {
                let head = clean[..paren_idx].trim();
                let params_str = &clean[paren_idx + 1..clean.len() - 1];

                let head_parts: Vec<&str> = head.split_whitespace().collect();
                if head_parts.len() >= 2 {
                    let ret_c = head_parts[0];
                    let ret_ty = map_c_type(ret_c);
                    let fn_name = head_parts[1].trim_start_matches('*');

                    let mut param_decls = Vec::new();
                    if !params_str.trim().is_empty() && params_str.trim() != "void" {
                        for p in params_str.split(',') {
                            let p = p.trim();
                            let p_parts: Vec<&str> = p.split_whitespace().collect();
                            if p_parts.len() >= 2 {
                                let p_type = map_c_type(p_parts[0]);
                                let p_name = p_parts[1].trim_start_matches('*');
                                param_decls.push(format!("{p_name}: {p_type}"));
                            }
                        }
                    }

                    let params_joined = param_decls.join(", ");
                    functions.push(format!("    public {fn_name}({params_joined}) -> {ret_ty}"));
                }
            }
        }
    }

    // Emit constants
    for c in constants {
        out.push_str(&format!("{c}\n"));
    }
    if !typedefs.is_empty() {
        out.push('\n');
        for t in typedefs {
            out.push_str(&format!("{t}\n"));
        }
    }
    if !structs.is_empty() {
        out.push('\n');
        for s in structs {
            out.push_str(&format!("{s}\n"));
        }
    }
    if !functions.is_empty() {
        out.push_str("extern \"c\"\n");
        for f in functions {
            out.push_str(&format!("{f}\n"));
        }
        out.push_str("end\n");
    }

    Ok(out)
}

fn map_c_type(c_type: &str) -> &'static str {
    let t = c_type.trim_start_matches("const ").trim();
    if t.ends_with('*') {
        return "int";
    }
    match t {
        "int" | "int32_t" | "int64_t" | "long" | "size_t" | "uint32_t" | "uint64_t"
        | "uintptr_t" => "int",
        "float" | "double" => "float",
        "bool" | "_Bool" => "bool",
        "void" => "void",
        _ => "int",
    }
}
