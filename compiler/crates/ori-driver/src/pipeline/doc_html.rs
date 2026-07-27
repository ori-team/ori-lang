//! Static HTML renderer for `ori doc --format html`.

/// Convert the Markdown subset emitted by `render_documentation_markdown` into
/// a self-contained HTML document with semantic structure and minimal styling.
pub fn render_static_html(markdown: &str) -> String {
    let mut body = String::new();
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_lines: Vec<String> = Vec::new();
    let mut list_open = false;

    let flush_list = |body: &mut String, list_open: &mut bool| {
        if *list_open {
            body.push_str("</ul>\n");
            *list_open = false;
        }
    };

    for line in markdown.lines() {
        if line.starts_with("```") {
            flush_list(&mut body, &mut list_open);
            if in_code {
                body.push_str("<pre><code");
                if !code_lang.is_empty() {
                    let _ = write!(body, " class=\"language-{}\"", html_escape_attr(&code_lang));
                }
                body.push('>');
                body.push_str(&html_escape_text(&code_lines.join("\n")));
                body.push_str("</code></pre>\n");
                code_lines.clear();
                code_lang.clear();
                in_code = false;
            } else {
                in_code = true;
                code_lang = line.trim_start_matches('`').trim().to_string();
            }
            continue;
        }

        if in_code {
            code_lines.push(line.to_string());
            continue;
        }

        if line.starts_with("# ") {
            flush_list(&mut body, &mut list_open);
            body.push_str("<h1>");
            body.push_str(&html_escape_text(line.trim_start_matches("# ").trim()));
            body.push_str("</h1>\n");
            continue;
        }
        if line.starts_with("## ") {
            flush_list(&mut body, &mut list_open);
            body.push_str("<h2 id=\"");
            body.push_str(&html_anchor(line.trim_start_matches("## ").trim()));
            body.push_str("\">");
            body.push_str(&html_escape_text(line.trim_start_matches("## ").trim()));
            body.push_str("</h2>\n");
            continue;
        }
        if line.starts_with("### ") {
            flush_list(&mut body, &mut list_open);
            body.push_str("<h3>");
            body.push_str(&html_escape_text(line.trim_start_matches("### ").trim()));
            body.push_str("</h3>\n");
            continue;
        }
        if line.starts_with("#### ") {
            flush_list(&mut body, &mut list_open);
            body.push_str("<h4>");
            body.push_str(&html_escape_text(line.trim_start_matches("#### ").trim()));
            body.push_str("</h4>\n");
            continue;
        }

        if line.starts_with("- ") {
            if !list_open {
                body.push_str("<ul>\n");
                list_open = true;
            }
            body.push_str("<li>");
            body.push_str(&render_inline_markdown(
                line.trim_start_matches("- ").trim(),
            ));
            body.push_str("</li>\n");
            continue;
        }

        if line.trim().is_empty() {
            flush_list(&mut body, &mut list_open);
            continue;
        }

        flush_list(&mut body, &mut list_open);
        body.push_str("<p>");
        body.push_str(&render_inline_markdown(line.trim()));
        body.push_str("</p>\n");
    }

    flush_list(&mut body, &mut list_open);

    if in_code && !code_lines.is_empty() {
        body.push_str("<pre><code>");
        body.push_str(&html_escape_text(&code_lines.join("\n")));
        body.push_str("</code></pre>\n");
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Ori API Documentation</title>
<style>
:root {{ color-scheme: light dark; font-family: system-ui, sans-serif; line-height: 1.5; }}
body {{ max-width: 52rem; margin: 2rem auto; padding: 0 1rem; }}
h1, h2, h3, h4 {{ line-height: 1.2; }}
pre {{ overflow-x: auto; padding: 1rem; border-radius: 0.5rem; background: #f4f4f5; }}
code {{ font-family: ui-monospace, monospace; }}
@media (prefers-color-scheme: dark) {{
  pre {{ background: #27272a; }}
}}
</style>
</head>
<body>
<main>
{body}
</main>
</body>
</html>
"#
    )
}

fn render_inline_markdown(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        out.push_str(&html_escape_text(&rest[..start]));
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            out.push_str("<code>");
            out.push_str(&html_escape_text(&rest[..end]));
            out.push_str("</code>");
            rest = &rest[end + 1..];
        } else {
            out.push('`');
            break;
        }
    }
    out.push_str(&html_escape_text(rest));
    out
}

fn html_escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_attr(text: &str) -> String {
    html_escape_text(text).replace('"', "&quot;")
}

fn html_anchor(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

use std::fmt::Write as _;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_heading_and_code_block() {
        let md = "# Title\n\n```ori\nfunc main()\nend\n```\n";
        let html = render_static_html(md);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<pre><code class=\"language-ori\">"));
        assert!(html.contains("func main()"));
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn escapes_html_in_body_text() {
        let md = "Use `x < y` carefully.\n";
        let html = render_static_html(md);
        assert!(html.contains("&lt;"));
        assert!(!html.contains("x < y"));
    }
}
