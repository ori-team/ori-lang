use ori_diagnostics::{Diagnostic, Severity, SourceCache};

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Return `s` if color is enabled, else `""`. All ANSI codes are `&'static str`.
#[inline]
fn c(use_color: bool, s: &'static str) -> &'static str {
    if use_color {
        s
    } else {
        ""
    }
}

/// Render all diagnostics to stderr.
pub fn render_all(cache: &SourceCache, diagnostics: &[Diagnostic], color: bool) {
    for diag in diagnostics {
        render_one(cache, diag, color);
    }
}

fn render_one(cache: &SourceCache, diag: &Diagnostic, color: bool) {
    let (sev_color, sev_label) = match diag.severity {
        Severity::Error => (RED, "error"),
        Severity::Warning => (YELLOW, "warning"),
    };

    // Header: `error[code]: message`
    eprintln!(
        "{}{}{}[{}]{}: {}{}{}",
        c(color, BOLD),
        c(color, sev_color),
        sev_label,
        diag.code,
        c(color, RESET),
        c(color, BOLD),
        diag.message,
        c(color, RESET)
    );

    // Labels
    for label in &diag.labels {
        if let Some(file) = cache.get(label.file_id) {
            let (line, col) = file.line_col(label.span.start);
            let line_text = file.line_text(line);
            let line_num = format!("{}", line);
            let gutter = " ".repeat(line_num.len());

            eprintln!(
                "  {}{}-->{} {}:{}:{}",
                c(color, DIM),
                c(color, CYAN),
                c(color, RESET),
                file.path.display(),
                line,
                col
            );
            eprintln!("   {}{}|{}", c(color, DIM), gutter, c(color, RESET));
            eprintln!(
                "{}{}{}{}|{} {}",
                c(color, DIM),
                line_num,
                c(color, RESET),
                c(color, DIM),
                c(color, RESET),
                line_text
            );

            // Underline
            let col0 = (col as usize).saturating_sub(1);
            let (end_line, end_col) = file.line_col(label.span.end);
            let line_width = line_text.chars().count();
            let len = if end_line == line {
                end_col.saturating_sub(col) as usize
            } else {
                line_width.saturating_sub(col0)
            }
            .max(1)
            .min(line_width.saturating_sub(col0));
            let under = "^".repeat(len);
            eprintln!(
                "   {}{}|{} {}{}{}{}{}",
                c(color, DIM),
                gutter,
                c(color, RESET),
                " ".repeat(col0),
                c(color, sev_color),
                c(color, BOLD),
                under,
                c(color, RESET)
            );

            // Label message
            if !label.message.is_empty() {
                eprintln!(
                    "   {}{}|{} {}{} {}{}",
                    c(color, DIM),
                    gutter,
                    c(color, RESET),
                    " ".repeat(col0),
                    c(color, DIM),
                    label.message,
                    c(color, RESET)
                );
            }
            eprintln!("   {}{}|{}", c(color, DIM), gutter, c(color, RESET));
        }
    }

    if let Some(why) = &diag.why {
        eprintln!("   {}= why:{} {}", c(color, DIM), c(color, RESET), why);
    }
    if let Some(action) = &diag.action {
        eprintln!(
            "   {}= action:{} {}",
            c(color, DIM),
            c(color, RESET),
            action
        );
    }
    for note in &diag.notes {
        eprintln!("   {}= note:{} {}", c(color, DIM), c(color, RESET), note);
    }

    eprintln!();
}

/// Print a summary line: `N error(s), M warning(s)`.
pub fn print_summary(errors: usize, warnings: usize, color: bool) {
    if errors == 0 && warnings == 0 {
        eprintln!("{}no errors{}", c(color, BOLD), c(color, RESET));
    } else {
        if errors > 0 {
            eprint!(
                "{}{}{}{} error(s){}",
                c(color, BOLD),
                c(color, RED),
                errors,
                c(color, RESET),
                c(color, RESET)
            );
        }
        if errors > 0 && warnings > 0 {
            eprint!(", ");
        }
        if warnings > 0 {
            eprint!(
                "{}{}{}{} warning(s){}",
                c(color, BOLD),
                c(color, YELLOW),
                warnings,
                c(color, RESET),
                c(color, RESET)
            );
        }
        eprintln!();
    }
}
