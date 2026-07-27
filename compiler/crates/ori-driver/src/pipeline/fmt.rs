//! Source text formatter (`ori fmt`).
//!
//! Extracted from `pipeline.rs` as part of Etapa 8.3 monolith reduction.
//! The public entry points are re-exported from `pipeline.rs` to preserve the
//! driver API while keeping formatting concerns out of the pipeline facade.

use ori_diagnostics::{Diagnostic, DiagnosticSink, SourceCache};
use std::path::Path;

pub struct FmtOutput {
    pub cache: SourceCache,
    pub formatted: String,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

pub fn run_fmt(path: &Path) -> Result<FmtOutput, String> {
    let source = super::project::read_file(path)?;
    let mut cache = SourceCache::default();
    let mut sink = DiagnosticSink::default();
    let file_id = cache.add(path, source.clone());
    let tokens = ori_lexer::lex(&source, file_id, &mut sink);
    let _ast = ori_parser::parse(&tokens, &source, file_id, &mut sink);
    let formatted = if !sink.has_errors() {
        format_source_text(&source)
    } else {
        String::new()
    };
    let has_errors = sink.has_errors();
    let diagnostics = sink.into_diagnostics();
    Ok(FmtOutput {
        cache,
        formatted,
        diagnostics,
        has_errors,
    })
}

pub fn format_source_text(source: &str) -> String {
    let mut indent = 0usize;
    let mut out = String::new();
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut block_stack = Vec::new();

    for (index, raw_line) in lines.iter().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            out.push('\n');
            continue;
        }

        if should_dedent_before(line) {
            indent = indent.saturating_sub(1);
            if closes_current_block_before_opening(line) {
                block_stack.pop();
            }
        }

        out.push_str(&"    ".repeat(indent));
        out.push_str(line);
        out.push('\n');

        let next_line = next_significant_line(&lines, index + 1);
        if opens_block_after(line, next_line, &block_stack) {
            block_stack.push(block_kind_for(line));
            indent += 1;
        }
    }

    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockKind {
    Trait,
    Other,
}

fn should_dedent_before(line: &str) -> bool {
    is_end_line(line) || line == "else" || line.starts_with("elif ") || line.starts_with("case ")
}

fn closes_current_block_before_opening(line: &str) -> bool {
    is_end_line(line) || line == "else" || line.starts_with("elif ") || line.starts_with("case ")
}

fn is_end_line(line: &str) -> bool {
    line == "end" || line.starts_with("end ")
}

fn opens_block_after(line: &str, next_line: Option<&str>, block_stack: &[BlockKind]) -> bool {
    if is_comment_line(line) || is_end_line(line) || line.starts_with("@") {
        return false;
    }

    line == "else"
        || line.starts_with("elif ")
        || line.starts_with("case ")
        || line == "loop"
        || line.starts_with("if ")
        || line.starts_with("while ")
        || line.starts_with("for ")
        || line.starts_with("repeat ")
        || line.starts_with("match ")
        || declaration_opens_block(line, next_line, block_stack)
}

fn declaration_opens_block(line: &str, next_line: Option<&str>, block_stack: &[BlockKind]) -> bool {
    let bare = declaration_line_without_modifiers(line);
    let is_function = is_function_decl_line(bare);
    if is_function && inside_trait(block_stack) {
        return trait_function_has_body(next_line);
    }
    is_function
        || bare.starts_with("struct ")
        || bare.starts_with("enum ")
        || bare.starts_with("trait ")
        || bare.starts_with("apply ")
        || bare.starts_with("use ")
        || bare.starts_with("implement ")
        || bare.starts_with("extern ")
}

fn inside_trait(block_stack: &[BlockKind]) -> bool {
    block_stack.contains(&BlockKind::Trait)
}

fn trait_function_has_body(next_line: Option<&str>) -> bool {
    let Some(next_line) = next_line else {
        return false;
    };
    let next_member = declaration_line_without_modifiers(next_line);
    !(next_line == "end"
        || next_line == "mut"
        || next_line == "public"
        || next_line == "async"
        || is_function_decl_line(next_member)
        || next_member.starts_with("type "))
}

fn block_kind_for(line: &str) -> BlockKind {
    if declaration_line_without_modifiers(line).starts_with("trait ") {
        BlockKind::Trait
    } else {
        BlockKind::Other
    }
}

fn declaration_line_without_modifiers(mut line: &str) -> &str {
    loop {
        let next = line
            .strip_prefix("public ")
            .or_else(|| line.strip_prefix("async "))
            .or_else(|| line.strip_prefix("iter "))
            .or_else(|| line.strip_prefix("mut "));
        let Some(next) = next else {
            return line;
        };
        line = next;
    }
}

/// S3 function form: `name(...)` / `name[T](...)` / `name for T: Trait (...)`.
///
/// Expression bodies (`=> expr`) do not open an indent block.
///
/// Legacy `func name(...)` is still recognized so `ori fmt` can indent
/// unmigrated buffers; prefer `ori migrate-syntax` then re-format.
fn is_function_decl_line(line: &str) -> bool {
    // `name(params) -> T => expr` is a single-line body — no indent increase.
    if line.contains("=>") {
        return false;
    }
    if let Some(rest) = line.strip_prefix("func ") {
        return is_named_function_head(rest);
    }
    is_named_function_head(line)
}

fn is_named_function_head(line: &str) -> bool {
    let name_end = line
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(line.len());
    if name_end == 0 {
        return false;
    }
    let rest = &line[name_end..];
    rest.starts_with('(')
        || rest.starts_with('[')
        || rest.starts_with(" for ")
        || rest.starts_with('<') // legacy angle params (error surface)
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("--") || line.starts_with("|--")
}

fn next_significant_line<'a>(lines: &'a [&str], start: usize) -> Option<&'a str> {
    lines
        .iter()
        .skip(start)
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !is_comment_line(line) && !line.starts_with("@"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_idempotent_on_simple_module() {
        let src = "module App\n\nmain()\n    io.println(\"hi\")\nend\n";
        let once = format_source_text(src);
        let twice = format_source_text(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn fmt_dedents_end_and_else() {
        let src = "module App\nf()\nif x\nreturn 1\nelse\nreturn 2\nend\nend\n";
        let out = format_source_text(src);
        assert!(out.contains("    if x\n"));
        assert!(out.contains("    return 1\n"));
        assert!(out.contains("    else\n"));
        assert!(out.contains("    return 2\n"));
        assert!(out.contains("    end\n"));
        assert!(out.contains("f()\n"));
    }

    #[test]
    fn fmt_dedents_labeled_end() {
        let src = "module App\nf()\nif x\nreturn 1\nend if\nend\n";
        let out = format_source_text(src);
        assert_eq!(
            out,
            "module App\nf()\n    if x\n        return 1\n    end if\nend\n"
        );
        assert_eq!(format_source_text(&out), out);
    }

    #[test]
    fn fmt_keeps_fat_arrow_func_flat() {
        let src = "module App\ndouble(x: int) -> int => x * 2\nmain()\nreturn double(21)\nend\n";
        let out = format_source_text(src);
        assert_eq!(
            out,
            "module App\ndouble(x: int) -> int => x * 2\nmain()\n    return double(21)\nend\n"
        );
        assert_eq!(format_source_text(&out), out);
    }

    #[test]
    fn fmt_keeps_required_trait_methods_unopened() {
        let src = "module app.main\ntrait Drawable\ndraw()\narea() -> int\nend\n";
        let out = format_source_text(src);
        assert_eq!(
            out,
            "module app.main\ntrait Drawable\n    draw()\n    area() -> int\nend\n"
        );
        assert_eq!(format_source_text(&out), out);
    }

    #[test]
    fn fmt_indents_default_trait_methods() {
        let src = "module app.main\ntrait Displayable\ndisplay() -> string\nprint()\nio.print(display())\nend\nend\n";
        let out = format_source_text(src);
        assert_eq!(
            out,
            "module app.main\ntrait Displayable\n    display() -> string\n    print()\n        io.print(display())\n    end\nend\n"
        );
        assert_eq!(format_source_text(&out), out);
    }

    #[test]
    fn fmt_keeps_stack_aligned_after_branch_blocks() {
        let src = "module app.main\ntrait Displayable\ndisplay() -> string\nprint(value: int)\nif value > 0\nio.print(display())\nelse\nio.print(\"empty\")\nend\nend\nend\noutside()\nio.print(\"done\")\nend\n";
        let out = format_source_text(src);
        assert_eq!(
            out,
            "module app.main\ntrait Displayable\n    display() -> string\n    print(value: int)\n        if value > 0\n            io.print(display())\n        else\n            io.print(\"empty\")\n        end\n    end\nend\noutside()\n    io.print(\"done\")\nend\n"
        );
        assert_eq!(format_source_text(&out), out);
    }

    #[test]
    fn fmt_is_idempotent_for_real_use_constructs() {
        let src = "module app.main\nimport ori.string (trim = trim_text)\nimport ori.task = task\n\nstruct Book\nid: int\ntitle: string\nend\n\ntrait Displayable\ndisplay() -> string\ndebug()\nio.print(display())\nend\nend\n\nasync load[T](value: T) -> T\nawait task.sleep(1)\nreturn value\nend\n\nmain()\nconst book: Book = Book(id: 1, title: trim_text(\" Ori \"))\nmatch book.id\ncase 0:\nio.print(\"zero\")\ncase 1:\nio.print(book.title)\nelse\nio.print(\"many\")\nend\nend\n";
        let once = format_source_text(src);
        let twice = format_source_text(&once);
        assert_eq!(once, twice);
        assert!(once.contains("import ori.string (trim = trim_text)\n"));
        assert!(once.contains("async load[T](value: T) -> T\n"));
        assert!(once.contains("    match book.id\n"));
        assert!(once.contains("    case 1:\n"));
        assert!(once.contains("const book: Book = Book(id: 1, title: trim_text(\" Ori \"))"));
    }
}
