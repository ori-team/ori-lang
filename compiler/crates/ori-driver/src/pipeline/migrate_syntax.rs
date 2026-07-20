//! Best-effort S3 syntax migration for Ori source (`.orl`).
//!
//! Mechanical rewrites only. Complex forms (`implement` bodies, ambiguous `?`,
//! misplaced `where` bounds) are flagged as notes for manual review.
//! Does **not** touch `.oridoc` sidecars (their `namespace` / `doc func` are a
//! separate DSL).
//!
//! All rewrites operate on UTF-8 `str`/`char` boundaries (never raw bytes as
//! Latin-1) so comments with box-drawing / accented text stay intact.

use std::fs;
use std::path::{Path, PathBuf};

/// CLI / library options for `ori migrate-syntax`.
#[derive(Clone, Debug)]
pub struct MigrateSyntaxOptions {
    /// When true, do not write files; only report planned changes.
    pub dry_run: bool,
    /// When true, list every scanned file in the summary (`[ok]` for unchanged).
    pub verbose: bool,
}

impl Default for MigrateSyntaxOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            verbose: false,
        }
    }
}

/// One file considered by the migrator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigratedFile {
    pub path: PathBuf,
    pub changed: bool,
    pub rewrites: Vec<String>,
    pub notes: Vec<String>,
}

/// Aggregate report for a migrate-syntax invocation.
#[derive(Clone, Debug, Default)]
pub struct MigrateSyntaxReport {
    pub files: Vec<MigratedFile>,
    pub skipped: Vec<PathBuf>,
}

impl MigrateSyntaxReport {
    pub fn changed_count(&self) -> usize {
        self.files.iter().filter(|f| f.changed).count()
    }

    /// Format a human-readable summary.
    ///
    /// When `verbose` is true, every scanned file is listed (`[ok]` for
    /// unchanged sources). When false, only changed files, files with notes,
    /// and skipped paths are shown.
    pub fn format_summary(&self, verbose: bool) -> String {
        let mut out = String::new();
        let scanned = self.files.len();
        let changed = self.changed_count();
        out.push_str(&format!(
            "migrate-syntax: scanned {scanned} file(s), changed {changed}\n"
        ));
        for file in &self.files {
            let quiet_unchanged = !file.changed && file.notes.is_empty();
            if quiet_unchanged && !verbose {
                continue;
            }
            let status = if file.changed {
                "changed"
            } else if !file.notes.is_empty() {
                "notes"
            } else {
                "ok"
            };
            out.push_str(&format!("  [{status}] {}\n", file.path.display()));
            if verbose || file.changed || !file.notes.is_empty() {
                for tag in &file.rewrites {
                    out.push_str(&format!("    - {tag}\n"));
                }
                for note in &file.notes {
                    out.push_str(&format!("    ! {note}\n"));
                }
            }
        }
        for path in &self.skipped {
            out.push_str(&format!("  [skipped] {}\n", path.display()));
        }
        out
    }
}

/// Result of rewriting a single source buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrateTextResult {
    pub source: String,
    pub rewrites: Vec<String>,
    pub notes: Vec<String>,
}

impl MigrateTextResult {
    pub fn changed(&self, original: &str) -> bool {
        self.source != original
    }
}

/// Migrate a single Ori source string (best-effort).
pub fn migrate_source(source: &str) -> MigrateTextResult {
    let mut rewrites = Vec::new();
    let mut notes = Vec::new();
    let mut text = source.to_string();

    text = apply_line_rewrites(&text, &mut rewrites, &mut notes);
    text = map_code_regions(&text, |code| {
        let mut c = code.to_string();
        if replace_angle_type_args_in_place(&mut c) {
            push_tag(&mut rewrites, "<>→[]");
        }
        if rewrite_of_type_forms_in_place(&mut c) {
            push_tag(&mut rewrites, "of-type→[]");
        }
        if rewrite_do_closures_in_place(&mut c) {
            push_tag(&mut rewrites, "do(→(");
        }
        c
    });
    text = rewrite_postfix_question(&text, &mut rewrites, &mut notes);
    text = rewrite_redundant_apply_use(&text, &mut rewrites, &mut notes);
    text = rewrite_associated_type_keyword(&text, &mut rewrites);

    let mut seen = std::collections::HashSet::new();
    rewrites.retain(|tag| seen.insert(tag.clone()));

    MigrateTextResult {
        source: text,
        rewrites,
        notes,
    }
}

/// Walk paths (files or directories) and migrate `.orl` sources.
pub fn run_migrate_syntax(
    paths: &[PathBuf],
    options: MigrateSyntaxOptions,
) -> Result<MigrateSyntaxReport, String> {
    if paths.is_empty() {
        return Err("migrate-syntax: provide at least one file or directory path".into());
    }

    let mut report = MigrateSyntaxReport::default();
    let mut files = Vec::new();
    for path in paths {
        collect_orl_files(path, &mut files, &mut report.skipped)?;
    }
    files.sort();
    files.dedup();

    for path in files {
        if should_skip_path(&path) {
            report.skipped.push(path);
            continue;
        }
        let original = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
        let result = migrate_source(&original);
        let changed = result.changed(&original);
        if changed && !options.dry_run {
            fs::write(&path, &result.source)
                .map_err(|e| format!("cannot write `{}`: {e}", path.display()))?;
        }
        report.files.push(MigratedFile {
            path,
            changed,
            rewrites: result.rewrites,
            notes: result.notes,
        });
    }

    Ok(report)
}

fn collect_orl_files(
    path: &Path,
    out: &mut Vec<PathBuf>,
    skipped: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if should_skip_path(path) {
        skipped.push(path.to_path_buf());
        return Ok(());
    }
    if path.is_file() {
        if path.extension().and_then(|e| e.to_str()) == Some("orl") {
            out.push(path.to_path_buf());
        } else {
            skipped.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !path.is_dir() {
        return Err(format!("path not found: `{}`", path.display()));
    }
    let entries = fs::read_dir(path)
        .map_err(|e| format!("cannot read directory `{}`: {e}", path.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("cannot read entry under `{}`: {e}", path.display()))?;
        let child = entry.path();
        if should_skip_path(&child) {
            skipped.push(child);
            continue;
        }
        if child.is_dir() {
            collect_orl_files(&child, out, skipped)?;
        } else if child.extension().and_then(|e| e.to_str()) == Some("orl") {
            out.push(child);
        }
    }
    Ok(())
}

fn should_skip_path(_path: &Path) -> bool {
    // Reserved for future path filters (e.g. vendor trees). No packages skipped today.
    false
}

fn push_tag(tags: &mut Vec<String>, tag: &str) {
    tags.push(tag.to_string());
}

/// Apply `f` to non-string, non-line-comment regions of `source` (UTF-8 safe).
fn map_code_regions(source: &str, mut f: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(source.len());
    let mut code = String::new();
    let mut chars = source.chars().peekable();
    let mut in_string = false;

    let flush_code = |code: &mut String, out: &mut String, f: &mut dyn FnMut(&str) -> String| {
        if !code.is_empty() {
            out.push_str(&f(code));
            code.clear();
        }
    };

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    out.push(next);
                    chars.next();
                }
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            flush_code(&mut code, &mut out, &mut f);
            out.push('"');
            in_string = true;
            continue;
        }
        if c == '-' && chars.peek() == Some(&'-') {
            flush_code(&mut code, &mut out, &mut f);
            out.push('-');
            out.push('-');
            chars.next();
            // rest of line comment
            for c2 in chars.by_ref() {
                out.push(c2);
                if c2 == '\n' {
                    break;
                }
            }
            continue;
        }
        code.push(c);
    }
    flush_code(&mut code, &mut out, &mut f);
    out
}

fn apply_line_rewrites(
    source: &str,
    rewrites: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> String {
    let mut out = String::with_capacity(source.len());
    for (idx, line) in source.lines().enumerate() {
        let (code_part, comment) = split_line_comment(line);
        let mut code = code_part.to_string();

        if replace_word(&mut code, "namespace", "module") {
            push_tag(rewrites, "namespace→module");
        }
        if code.contains("else if") {
            let next = code.replace("else if", "elif");
            if next != code {
                code = next;
                push_tag(rewrites, "else if→elif");
            }
        }
        // Result constructors: success/error → ok/err (M2.result-ctors)
        if rewrite_result_ctors(&mut code) {
            push_tag(rewrites, "success/error→ok/err");
        }
        if rewrite_import_as(&mut code) {
            push_tag(rewrites, "import as→=");
        }
        if rewrite_import_only(&mut code) {
            push_tag(rewrites, "import only→(…)");
        }
        if rewrite_func_decl_keyword(&mut code) {
            push_tag(rewrites, "strip func decl");
        }
        if rewrite_case_dot_variant(&mut code) {
            push_tag(rewrites, "case .Variant→Variant");
        }
        if rewrite_implement_header(&mut code) {
            push_tag(rewrites, "implement→apply/use header");
            notes.push(format!(
                "line {}: implement header rewritten — review body as `use Trait` section",
                idx + 1
            ));
        }
        if rewrite_apply_trait_to_header(&mut code) {
            push_tag(rewrites, "apply Trait to Type→apply Type / use Trait");
            notes.push(format!(
                "line {}: apply Trait to/for Type rewritten — ensure free members precede `use`",
                idx + 1
            ));
        }
        if rewrite_where_is_bounds(&mut code) {
            push_tag(rewrites, "where T is→for T:");
            notes.push(format!(
                "line {}: `where` bounds rewritten in place — prefer `name for T: Trait (...)` position",
                idx + 1
            ));
        }

        out.push_str(&code);
        out.push_str(comment);
        out.push('\n');
    }
    if !source.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn split_line_comment(line: &str) -> (&str, &str) {
    let mut in_string = false;
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if in_string {
            if c == '\\' {
                chars.next();
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            continue;
        }
        if c == '-' && chars.peek().map(|(_, ch)| *ch) == Some('-') {
            return (&line[..i], &line[i..]);
        }
    }
    (line, "")
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_word_boundary_before_str(s: &str, byte_idx: usize) -> bool {
    if byte_idx == 0 {
        return true;
    }
    match s[..byte_idx].chars().next_back() {
        Some(c) => !is_ident_char(c),
        None => true,
    }
}

fn is_word_boundary_after_str(s: &str, byte_idx: usize) -> bool {
    match s[byte_idx..].chars().next() {
        Some(c) => !is_ident_char(c),
        None => true,
    }
}

fn replace_word(code: &mut String, from: &str, to: &str) -> bool {
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    let mut changed = false;
    while i < code.len() {
        if code[i..].starts_with(from)
            && is_word_boundary_before_str(code, i)
            && is_word_boundary_after_str(code, i + from.len())
        {
            out.push_str(to);
            i += from.len();
            changed = true;
            continue;
        }
        let ch = code[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    if changed {
        *code = out;
    }
    changed
}

/// Rewrite `success`/`error` result constructors and match cases to `ok`/`err`.
fn rewrite_result_ctors(code: &mut String) -> bool {
    let mut changed = false;
    let mut next = code.replace("case success", "case ok");
    if next != *code {
        changed = true;
    }
    let n2 = next.replace("case error", "case err");
    if n2 != next {
        changed = true;
        next = n2;
    }
    // Word-boundary replacements for constructors (and capitalized forms).
    if replace_word(&mut next, "success", "ok") {
        changed = true;
    }
    if replace_word(&mut next, "Success", "ok") {
        changed = true;
    }
    // Only rewrite `error` when used as result ctor-ish keyword at word boundary.
    // `replace_word` turns every bare `error` into `err` on a code line — acceptable
    // for migrate of Ori sources where `error` was the result ctor keyword.
    if replace_word(&mut next, "error", "err") {
        changed = true;
    }
    if replace_word(&mut next, "Error", "err") {
        changed = true;
    }
    if changed {
        *code = next;
    }
    changed
}

fn rewrite_import_as(code: &mut String) -> bool {
    let trimmed = code.trim_start();
    let is_import = trimmed.starts_with("import ")
        || trimmed.starts_with("public import ")
        || trimmed.starts_with("pub import ");
    if !is_import || !code.contains(" as ") {
        return false;
    }
    if let Some(idx) = code.find(" as ") {
        code.replace_range(idx..idx + 4, " = ");
        return true;
    }
    false
}

fn rewrite_import_only(code: &mut String) -> bool {
    if !code.contains(" only ") {
        return false;
    }
    let trimmed = code.trim_start();
    if !(trimmed.starts_with("import ")
        || trimmed.starts_with("public import ")
        || trimmed.starts_with("pub import "))
    {
        return false;
    }
    let next = code.replacen(" only ", " ", 1);
    if next != *code {
        *code = next;
        true
    } else {
        false
    }
}

fn rewrite_func_decl_keyword(code: &mut String) -> bool {
    // Strip declaration `func` (not callable type `func(`).
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    let mut changed = false;
    while i < code.len() {
        if code[i..].starts_with("func")
            && is_word_boundary_before_str(code, i)
            && is_word_boundary_after_str(code, i + 4)
        {
            let after = i + 4;
            let rest = &code[after..];
            // callable type: `func(`
            if rest.starts_with('(') {
                out.push_str("func");
                i = after;
                continue;
            }
            // declaration: `func name` / `func\tname`
            if rest.starts_with(|c: char| c.is_whitespace()) {
                let name_start = rest
                    .find(|c: char| !c.is_whitespace())
                    .unwrap_or(rest.len());
                let after_ws = &rest[name_start..];
                if after_ws
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                {
                    // skip `func` + whitespace; keep single space if previous was non-space
                    if out
                        .chars()
                        .next_back()
                        .is_some_and(|c| !c.is_whitespace() && c != '\n')
                    {
                        out.push(' ');
                    }
                    i = after + name_start;
                    changed = true;
                    continue;
                }
            }
        }
        let ch = code[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    if changed {
        *code = out;
    }
    changed
}

fn rewrite_case_dot_variant(code: &mut String) -> bool {
    let trimmed = code.trim_start();
    if !trimmed.starts_with("case .") {
        return false;
    }
    let indent_len = code.len() - trimmed.len();
    let rest = &trimmed["case .".len()..];
    *code = format!("{}case {rest}", &code[..indent_len]);
    true
}

fn rewrite_implement_header(code: &mut String) -> bool {
    let trimmed = code.trim_start();
    if !trimmed.starts_with("implement ") {
        return false;
    }
    let indent_len = code.len() - trimmed.len();
    let body = trimmed.trim_start_matches("implement ").trim();
    if let Some((trait_name, type_part)) = body.split_once(" for ") {
        let trait_name = trait_name.trim();
        let type_name = type_part.trim();
        if trait_name.is_empty() || type_name.is_empty() {
            return false;
        }
        let indent = &code[..indent_len];
        *code = format!("{indent}apply {type_name}\n{indent}  use {trait_name}");
        return true;
    }
    false
}

fn rewrite_apply_trait_to_header(code: &mut String) -> bool {
    let trimmed = code.trim_start();
    if !trimmed.starts_with("apply ") {
        return false;
    }
    let indent_len = code.len() - trimmed.len();
    let body = trimmed.trim_start_matches("apply ").trim();
    let (trait_name, type_name) = if let Some((t, ty)) = body.split_once(" to ") {
        (t.trim(), ty.trim())
    } else if let Some((t, ty)) = body.split_once(" for ") {
        (t.trim(), ty.trim())
    } else {
        return false;
    };
    if trait_name.is_empty() || type_name.is_empty() || trait_name.contains('(') {
        return false;
    }
    let indent = &code[..indent_len];
    *code = format!("{indent}apply {type_name}\n{indent}  use {trait_name}");
    true
}

fn rewrite_where_is_bounds(code: &mut String) -> bool {
    if !code.contains("where ") {
        return false;
    }
    let original = code.clone();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    while i < code.len() {
        if code[i..].starts_with("where ") && is_word_boundary_before_str(code, i) {
            if let Some((consumed, replacement)) = parse_where_clause_at(code, i) {
                out.push_str(&replacement);
                i += consumed;
                continue;
            }
        }
        let ch = code[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    if out != original {
        *code = out;
        true
    } else {
        false
    }
}

fn parse_where_clause_at(s: &str, start: usize) -> Option<(usize, String)> {
    let rest = &s[start..];
    if !rest.starts_with("where ") {
        return None;
    }
    let after_where = &rest["where ".len()..];
    let mut parts = Vec::new();
    let mut cursor = 0;
    let chars: Vec<char> = after_where.chars().collect();
    while cursor < chars.len() {
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        let name_start = cursor;
        while cursor < chars.len() && is_ident_char(chars[cursor]) {
            cursor += 1;
        }
        if cursor == name_start {
            return None;
        }
        let name: String = chars[name_start..cursor].iter().collect();
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        if cursor + 1 >= chars.len() || chars[cursor] != 'i' || chars[cursor + 1] != 's' {
            return None;
        }
        // word boundary after `is`
        if cursor + 2 < chars.len() && is_ident_char(chars[cursor + 2]) {
            return None;
        }
        cursor += 2;
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        let mut negated = false;
        if cursor + 2 < chars.len()
            && chars[cursor] == 'n'
            && chars[cursor + 1] == 'o'
            && chars[cursor + 2] == 't'
            && (cursor + 3 >= chars.len() || !is_ident_char(chars[cursor + 3]))
        {
            negated = true;
            cursor += 3;
            while cursor < chars.len() && chars[cursor].is_whitespace() {
                cursor += 1;
            }
        }
        let trait_start = cursor;
        while cursor < chars.len() && is_ident_char(chars[cursor]) {
            cursor += 1;
        }
        if cursor == trait_start {
            return None;
        }
        let trait_name: String = chars[trait_start..cursor].iter().collect();
        if negated {
            parts.push(format!("{name}: not {trait_name}"));
        } else {
            parts.push(format!("{name}: {trait_name}"));
        }
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        if cursor < chars.len() && chars[cursor] == ',' {
            cursor += 1;
            continue;
        }
        break;
    }
    if parts.is_empty() {
        return None;
    }
    let replacement = format!("for {}", parts.join(", "));
    let consumed_chars = "where ".chars().count() + cursor;
    let consumed_bytes: usize = s[start..]
        .chars()
        .take(consumed_chars)
        .map(|c| c.len_utf8())
        .sum();
    Some((consumed_bytes, replacement))
}

fn replace_angle_type_args_in_place(code: &mut String) -> bool {
    let original = code.clone();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    let mut changed = false;
    while i < code.len() {
        let ch = code[i..].chars().next().unwrap();
        if (ch.is_ascii_alphabetic() || ch == '_') && looks_like_angle_type_start(code, i) {
            if let Some((end, name, inner)) = take_angle_type(code, i) {
                if looks_like_type_args(&inner) {
                    out.push_str(&name);
                    out.push('[');
                    let mut nested = inner;
                    let _ = replace_angle_type_args_in_place(&mut nested);
                    out.push_str(&nested);
                    out.push(']');
                    i = end;
                    changed = true;
                    continue;
                }
            }
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    if changed {
        *code = out;
    } else {
        // keep original if algorithm walked without rewrite
        let _ = original;
    }
    changed
}

fn looks_like_angle_type_start(s: &str, name_start: usize) -> bool {
    let rest = &s[name_start..];
    let mut chars = rest.chars();
    // consume ident
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for c in chars.by_ref() {
        if is_ident_char(c) {
            continue;
        }
        // optional space then <
        if c.is_whitespace() {
            for c2 in chars {
                if c2.is_whitespace() {
                    continue;
                }
                return c2 == '<';
            }
            return false;
        }
        return c == '<';
    }
    false
}

fn take_angle_type(s: &str, name_start: usize) -> Option<(usize, String, String)> {
    let rest = &s[name_start..];
    let mut chars = rest.char_indices();
    let mut name_end = 0;
    while let Some((off, c)) = chars.next() {
        if is_ident_char(c) {
            name_end = off + c.len_utf8();
            continue;
        }
        break;
    }
    if name_end == 0 {
        return None;
    }
    let name = rest[..name_end].to_string();
    let mut i = name_end;
    while i < rest.len() {
        let c = rest[i..].chars().next().unwrap();
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        break;
    }
    if !rest[i..].starts_with('<') {
        return None;
    }
    i += 1; // <
    let inner_start = i;
    let mut depth = 1;
    while i < rest.len() {
        let c = rest[i..].chars().next().unwrap();
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    let inner = rest[inner_start..i].to_string();
                    let abs_end = name_start + i + 1;
                    return Some((abs_end, name, inner));
                }
            }
            '"' => {
                i += 1;
                while i < rest.len() {
                    let c2 = rest[i..].chars().next().unwrap();
                    if c2 == '\\' {
                        i += c2.len_utf8();
                        if i < rest.len() {
                            let esc = rest[i..].chars().next().unwrap();
                            i += esc.len_utf8();
                        }
                        continue;
                    }
                    i += c2.len_utf8();
                    if c2 == '"' {
                        break;
                    }
                }
                continue;
            }
            _ => {}
        }
        i += c.len_utf8();
    }
    None
}

fn looks_like_type_args(inner: &str) -> bool {
    let t = inner.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains("&&") || t.contains("||") || t.contains("==") || t.contains("!=") {
        return false;
    }
    t.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || c == '_'
            || c == ','
            || c == '.'
            || c == ' '
            || c == '\t'
            || c == '<'
            || c == '>'
            || c == '['
            || c == ']'
            || c == '('
            || c == ')'
            || c == ':'
    })
}

fn rewrite_of_type_forms_in_place(code: &mut String) -> bool {
    let mut s = code.clone();
    let mut changed = false;

    if s.contains("map of ") && s.contains(" to ") {
        let next = rewrite_map_of(&s);
        if next != s {
            s = next;
            changed = true;
        }
    }
    for (prefix, open) in [
        ("list of ", "list["),
        ("set of ", "set["),
        ("optional of ", "optional["),
        ("result of ", "result["),
        ("channel of ", "channel["),
        ("lazy of ", "lazy["),
        ("handle of ", "handle["),
    ] {
        if s.contains(prefix) {
            let next = rewrite_simple_of(&s, prefix, open);
            if next != s {
                s = next;
                changed = true;
            }
        }
    }
    if changed {
        *code = s;
    }
    changed
}

fn rewrite_map_of(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < source.len() {
        if source[i..].starts_with("map of ") {
            let after = i + "map of ".len();
            if let Some((end, k, v)) = take_map_of_pair(source, after) {
                out.push_str("map[");
                out.push_str(k.trim());
                out.push_str(", ");
                out.push_str(v.trim());
                out.push(']');
                i = end;
                continue;
            }
        }
        let ch = source[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn take_map_of_pair(source: &str, start: usize) -> Option<(usize, String, String)> {
    let rest = &source[start..];
    let to_idx = rest.find(" to ")?;
    let key = rest[..to_idx].trim();
    if key.is_empty() || key.contains('\n') {
        return None;
    }
    let after_to = to_idx + 4;
    let value_src = &rest[after_to..];
    let value_len = type_token_len(value_src)?;
    let value = value_src[..value_len].trim();
    if value.is_empty() {
        return None;
    }
    Some((
        start + after_to + value_len,
        key.to_string(),
        value.to_string(),
    ))
}

fn rewrite_simple_of(source: &str, prefix: &str, open: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < source.len() {
        if source[i..].starts_with(prefix) {
            let after = i + prefix.len();
            let rest = &source[after..];
            if let Some(len) = type_token_len(rest) {
                let ty = rest[..len].trim();
                out.push_str(open);
                out.push_str(ty);
                out.push(']');
                i = after + len;
                continue;
            }
        }
        let ch = source[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn type_token_len(src: &str) -> Option<usize> {
    let mut i = 0;
    while i < src.len() {
        let c = src[i..].chars().next().unwrap();
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        break;
    }
    if i >= src.len() {
        return None;
    }
    if src[i..].starts_with("func") {
        let after_func = i + 4;
        if src[after_func..].starts_with('(') {
            i = skip_balanced_str(src, after_func, '(', ')')?;
            while i < src.len() {
                let c = src[i..].chars().next().unwrap();
                if c.is_whitespace() {
                    i += c.len_utf8();
                    continue;
                }
                break;
            }
            if src[i..].starts_with("->") {
                i += 2;
                while i < src.len() {
                    let c = src[i..].chars().next().unwrap();
                    if c.is_whitespace() {
                        i += c.len_utf8();
                        continue;
                    }
                    break;
                }
                let rest_len = type_token_len(&src[i..])?;
                return Some(i + rest_len);
            }
            return Some(i);
        }
    }
    let start_c = src[i..].chars().next().unwrap();
    if !(start_c.is_ascii_alphabetic() || start_c == '_') {
        return None;
    }
    while i < src.len() {
        let c = src[i..].chars().next().unwrap();
        if is_ident_char(c) || c == '.' {
            i += c.len_utf8();
            continue;
        }
        if c == '[' {
            i = skip_balanced_str(src, i, '[', ']')?;
            continue;
        }
        if c == '<' {
            i = skip_balanced_str(src, i, '<', '>')?;
            continue;
        }
        break;
    }
    Some(i)
}

fn skip_balanced_str(s: &str, open_at: usize, open: char, close: char) -> Option<usize> {
    if !s[open_at..].starts_with(open) {
        return None;
    }
    let mut depth = 0;
    let mut i = open_at;
    while i < s.len() {
        let c = s[i..].chars().next().unwrap();
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i + c.len_utf8());
            }
        }
        i += c.len_utf8();
    }
    None
}

fn rewrite_do_closures_in_place(code: &mut String) -> bool {
    let mut out = String::with_capacity(code.len());
    let mut i = 0;
    let mut changed = false;
    while i < code.len() {
        if code[i..].starts_with("do")
            && is_word_boundary_before_str(code, i)
            && code[i + 2..].starts_with('(')
        {
            out.push('(');
            i += 3; // skip do(
            changed = true;
            continue;
        }
        let ch = code[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    if changed {
        *code = out;
    }
    changed
}

fn rewrite_postfix_question(
    source: &str,
    rewrites: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> String {
    let mut out_lines = Vec::new();
    let mut changed = false;
    for (idx, line) in source.lines().enumerate() {
        let (code, comment) = split_line_comment(line);
        if let Some(migrated) = try_rewrite_line_question(code) {
            out_lines.push(format!("{migrated}{comment}"));
            changed = true;
        } else if code_has_postfix_question(code) {
            notes.push(format!(
                "line {}: postfix `?` not auto-migrated — rewrite to `try expr` manually",
                idx + 1
            ));
            out_lines.push(line.to_string());
        } else {
            out_lines.push(line.to_string());
        }
    }
    let mut out = out_lines.join("\n");
    if source.ends_with('\n') {
        out.push('\n');
    }
    if changed {
        push_tag(rewrites, "?→try");
    }
    out
}

fn code_has_postfix_question(code: &str) -> bool {
    code.trim_end().ends_with('?')
}

fn try_rewrite_line_question(code: &str) -> Option<String> {
    let trimmed_end = code.trim_end();
    if !trimmed_end.ends_with('?') {
        return None;
    }
    let without_q = &trimmed_end[..trimmed_end.len() - 1];
    let trailing_ws = &code[trimmed_end.len()..];
    if without_q.trim_start().starts_with("try ") {
        return None;
    }
    if let Some(pos) = without_q.rfind('=') {
        let (lhs, rhs) = without_q.split_at(pos + 1);
        let rhs = rhs.trim_start();
        if rhs.is_empty() {
            return None;
        }
        return Some(format!("{lhs} try {rhs}{trailing_ws}"));
    }
    let trim_start = without_q.trim_start();
    let indent = &without_q[..without_q.len() - trim_start.len()];
    for prefix in ["return ", "const ", "var ", "let "] {
        if let Some(rest) = trim_start.strip_prefix(prefix) {
            return Some(format!("{indent}{prefix}try {rest}{trailing_ws}"));
        }
    }
    if is_simple_expr_token(trim_start) {
        return Some(format!("{indent}try {trim_start}{trailing_ws}"));
    }
    None
}

fn is_simple_expr_token(s: &str) -> bool {
    !s.is_empty() && !s.contains(" if ") && !s.contains(" else ") && !s.contains(" match ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_namespace_func_import_elif() {
        let src = r#"namespace app.main
import ori.io as io
import ori.list only (len)
public func main()
    if true
    else if false
    end
end
"#;
        let result = migrate_source(src);
        assert!(
            result.source.contains("module app.main"),
            "{}",
            result.source
        );
        assert!(result.source.contains("import ori.io = io"));
        assert!(result.source.contains("import ori.list (len)"));
        assert!(result.source.contains("public main()"), "{}", result.source);
        assert!(!result.source.contains("func main"));
        assert!(result.source.contains("elif false"));
    }

    #[test]
    fn keeps_func_callable_type() {
        let src = "module app.main\nconst f: func(int) -> int = (x: int) => x\n";
        let result = migrate_source(src);
        assert!(result.source.contains("func(int) -> int"));
        assert_eq!(result.source, src);
    }

    #[test]
    fn migrates_angle_and_of_types() {
        let src = "const xs: list<int> = []\nconst m: map of string to int = {}\n";
        let result = migrate_source(src);
        assert!(result.source.contains("list[int]"), "{}", result.source);
        assert!(
            result.source.contains("map[string, int]"),
            "{}",
            result.source
        );
    }

    #[test]
    fn migrates_do_closure_and_case_dot() {
        let src = r#"const f = do(x: int) => x * 2
match v
    case .Ok(x):
        return x
end
"#;
        let result = migrate_source(src);
        assert!(
            result.source.contains("= (x: int) => x * 2"),
            "{}",
            result.source
        );
        assert!(result.source.contains("case Ok(x):"));
        assert!(!result.source.contains("do("));
    }

    #[test]
    fn migrates_simple_question_propagate() {
        let src = "    const x: int = foo()?\n";
        let result = migrate_source(src);
        assert!(result.source.contains("try foo()"), "{}", result.source);
        assert!(!result.source.contains(")?"));
    }

    #[test]
    fn migrates_implement_header() {
        let src = "implement Displayable for Point\n  display(self) -> string\n  end\nend\n";
        let result = migrate_source(src);
        assert!(result.source.contains("apply Point"));
        assert!(result.source.contains("use Displayable"));
        assert!(!result.notes.is_empty());
    }

    #[test]
    fn does_not_touch_todo() {
        let src = "todo()\n";
        let result = migrate_source(src);
        assert_eq!(result.source, src);
    }

    #[test]
    fn preserves_utf8_box_drawing_in_comments() {
        let src = "-- ─────────────────── Structs ────────────────────\nconst xs: list<int> = []\n";
        let result = migrate_source(src);
        assert!(
            result.source.contains("─────────────────── Structs"),
            "UTF-8 corrupted: {}",
            result.source
        );
        assert!(result.source.contains("list[int]"));
    }

    #[test]
    fn should_skip_path_is_permissive_by_default() {
        assert!(!should_skip_path(Path::new("stdlib/list.orl")));
        assert!(!should_skip_path(Path::new(
            "examples/hello_world/main.orl"
        )));
        assert!(!should_skip_path(Path::new(
            "packages/other-lib/src/main.orl"
        )));
    }

    #[test]
    fn format_summary_verbose_lists_unchanged_files() {
        let report = MigrateSyntaxReport {
            files: vec![
                MigratedFile {
                    path: PathBuf::from("a.orl"),
                    changed: false,
                    rewrites: Vec::new(),
                    notes: Vec::new(),
                },
                MigratedFile {
                    path: PathBuf::from("b.orl"),
                    changed: true,
                    rewrites: vec!["namespace→module".into()],
                    notes: Vec::new(),
                },
            ],
            skipped: vec![PathBuf::from("vendor/legacy")],
        };
        let quiet = report.format_summary(false);
        assert!(quiet.contains("[changed] b.orl"));
        assert!(!quiet.contains("[ok] a.orl"));
        assert!(quiet.contains("[skipped] vendor/legacy"));

        let verbose = report.format_summary(true);
        assert!(verbose.contains("[ok] a.orl"), "{verbose}");
        assert!(verbose.contains("[changed] b.orl"));
        assert!(verbose.contains("namespace→module"));
    }
}

/// Collapse `apply T` + a lone `use Trait` section into the compact header
/// `apply T use Trait` (0.4).
///
/// Only the unambiguous shape is rewritten: an `apply` block whose entire body
/// is one `use` section. Anything else (inherent members, two or more traits)
/// is already correct as a nested block and is left alone. When the shape does
/// not match exactly, the file is left untouched and the compiler's
/// `apply.redundant_use_block` diagnostic — which prints the exact line to
/// write — guides the manual fix.
fn rewrite_redundant_apply_use(
    source: &str,
    rewrites: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut index = 0usize;
    let mut changed = false;

    while index < lines.len() {
        let Some(apply_rest) = apply_header_only(lines[index]) else {
            out.push(lines[index].to_string());
            index += 1;
            continue;
        };
        let apply_indent = leading_indent(lines[index]);

        // The body must open with the `use` line.
        let Some(use_line) = lines.get(index + 1) else {
            out.push(lines[index].to_string());
            index += 1;
            continue;
        };
        let Some(trait_name) = use_header_only(use_line) else {
            out.push(lines[index].to_string());
            index += 1;
            continue;
        };
        let use_indent = leading_indent(use_line);

        // Find the `end` closing the whole apply block: the first line at the
        // apply's own indentation that is exactly `end`.
        let mut apply_end = None;
        let mut scan = index + 2;
        while scan < lines.len() {
            let trimmed = lines[scan].trim();
            if trimmed == "end" && leading_indent(lines[scan]) == apply_indent {
                apply_end = Some(scan);
                break;
            }
            scan += 1;
        }
        let Some(apply_end) = apply_end else {
            out.push(lines[index].to_string());
            index += 1;
            continue;
        };

        // The line before it must close the `use` section at its indentation.
        let use_end = apply_end.saturating_sub(1);
        let closes_use =
            lines[use_end].trim() == "end" && leading_indent(lines[use_end]) == use_indent;
        // Everything between must belong to the `use` section — i.e. be
        // indented deeper than it. A line back at the section's own
        // indentation means a sibling member (a second trait, or a method
        // after the section), and the nested form has to stay.
        let body_is_only_the_use = lines[index + 2..use_end]
            .iter()
            .all(|line| line.trim().is_empty() || leading_indent(line).len() > use_indent.len());
        if !closes_use || !body_is_only_the_use {
            out.push(lines[index].to_string());
            index += 1;
            continue;
        }

        // Rewrite: merged header, body dedented by one level, one `end` less.
        let dedent = use_indent.len().saturating_sub(apply_indent.len());
        out.push(format!("{apply_indent}apply {apply_rest} use {trait_name}"));
        for line in &lines[index + 2..use_end] {
            out.push(dedent_line(line, dedent));
        }
        out.push(format!("{apply_indent}end"));
        changed = true;
        index = apply_end + 1;
    }

    if !changed {
        return source.to_string();
    }
    push_tag(rewrites, "apply/use→compact header");
    notes.push(
        "collapsed single-trait `apply` blocks into the compact `apply T use Trait` header"
            .to_string(),
    );
    let mut text = out.join("\n");
    if source.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// `apply Foo` alone on a line → `Some("Foo")`.
fn apply_header_only(line: &str) -> Option<String> {
    let (code, _) = split_line_comment(line);
    let rest = code.trim().strip_prefix("apply ")?;
    let rest = rest.trim();
    if rest.is_empty() || rest.contains(' ') {
        return None;
    }
    Some(rest.to_string())
}

/// `use Trait` alone on a line → `Some("Trait")`.
fn use_header_only(line: &str) -> Option<String> {
    let (code, _) = split_line_comment(line);
    let rest = code.trim().strip_prefix("use ")?;
    let rest = rest.trim();
    if rest.is_empty() || rest.contains(' ') {
        return None;
    }
    Some(rest.to_string())
}

fn leading_indent(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn dedent_line(line: &str, amount: usize) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    let indent = leading_indent(line);
    let keep = indent.len().saturating_sub(amount);
    format!("{}{}", &indent[..keep], line.trim_start())
}

/// `type Item = …` → `alias Item = …` for associated types (0.4).
///
/// Only rewrites lines inside an `apply` block, where `type` was the old
/// keyword; a top-level `type` was never valid Ori, so there is nothing else
/// this could hit.
fn rewrite_associated_type_keyword(source: &str, rewrites: &mut Vec<String>) -> String {
    let mut out = Vec::new();
    let mut in_apply = false;
    let mut changed = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("apply ") {
            in_apply = true;
        } else if in_apply && trimmed == "end" && leading_indent(line).is_empty() {
            in_apply = false;
        }
        if in_apply {
            if let Some(rest) = trimmed.strip_prefix("type ") {
                if rest.contains('=') {
                    let indent = leading_indent(line);
                    out.push(format!("{indent}alias {rest}"));
                    changed = true;
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    if !changed {
        return source.to_string();
    }
    push_tag(rewrites, "associated `type`→`alias`");
    let mut text = out.join("\n");
    if source.ends_with('\n') {
        text.push('\n');
    }
    text
}
