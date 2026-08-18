//! Persistent background compilation and evaluation daemon (CLI-DAEMON-1).
//!
//! Provides a lightweight JSON-RPC 2.0 interface over stdio to serve fast
//! in-process type-checking, code evaluation, formatting, and diagnostics
//! for editors, tools, and background build systems without process spawn overhead.

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::pipeline::{
    fmt::format_source_text,
    frontend::run_check_source,
    native::run_jit,
};

/// Run the daemon event loop reading JSON-RPC lines from stdin and writing responses to stdout.
pub fn run_daemon() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("stdin read error: {}", e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let resp = handle_jsonrpc_request(trimmed);
        writeln!(stdout_lock, "{}", resp).map_err(|e| format!("stdout write error: {}", e))?;
        stdout_lock.flush().map_err(|e| format!("stdout flush error: {}", e))?;

        if trimmed.contains("\"shutdown\"") {
            break;
        }
    }

    Ok(())
}

fn handle_jsonrpc_request(req_str: &str) -> String {
    // Basic JSON-RPC 2.0 parser without heavy external dependencies
    let id = extract_json_field(req_str, "id").unwrap_or_else(|| "null".to_string());
    let method = match extract_json_string(req_str, "method") {
        Some(m) => m,
        None => {
            return format!(
                r#"{{"jsonrpc":"2.0","error":{{"code":-32600,"message":"Invalid Request"}},"id":{}}}"#,
                id
            )
        }
    };

    match method.as_str() {
        "version" => {
            format!(
                r#"{{"jsonrpc":"2.0","result":{{"version":"0.3.8","protocol":"ori-daemon-v1"}},"id":{}}}"#,
                id
            )
        }
        "check" => {
            let file_opt = extract_json_string(req_str, "file");
            let code_opt = extract_json_string(req_str, "code");

            let (code, file_name) = if let Some(code) = code_opt {
                (code, file_opt.unwrap_or_else(|| "<daemon_check>".to_string()))
            } else if let Some(file) = &file_opt {
                match std::fs::read_to_string(file) {
                    Ok(c) => (c, file.clone()),
                    Err(e) => {
                        return format!(
                            r#"{{"jsonrpc":"2.0","error":{{"code":-32001,"message":"{}"}},"id":{}}}"#,
                            e, id
                        );
                    }
                }
            } else {
                return format!(
                    r#"{{"jsonrpc":"2.0","error":{{"code":-32602,"message":"Missing file or code parameter"}},"id":{}}}"#,
                    id
                );
            };

            match run_check_source(Path::new(&file_name), code) {
                Ok(check_out) => {
                    let diag_count = check_out.diagnostics.len();
                    let error_count = check_out.diagnostics.iter().filter(|d| d.is_error()).count();
                    format!(
                        r#"{{"jsonrpc":"2.0","result":{{"has_errors":{},"error_count":{},"diagnostic_count":{}}},"id":{}}}"#,
                        check_out.has_errors, error_count, diag_count, id
                    )
                }
                Err(e) => {
                    format!(
                        r#"{{"jsonrpc":"2.0","error":{{"code":-32003,"message":"{}"}},"id":{}}}"#,
                        escape_json_string(&e), id
                    )
                }
            }
        }
        "fmt" => {
            let code_opt = extract_json_string(req_str, "code");
            if let Some(code) = code_opt {
                let formatted = format_source_text(&code);
                let escaped = escape_json_string(&formatted);
                format!(
                    r#"{{"jsonrpc":"2.0","result":{{"formatted":"{}"}},"id":{}}}"#,
                    escaped, id
                )
            } else {
                format!(
                    r#"{{"jsonrpc":"2.0","error":{{"code":-32602,"message":"Missing code parameter"}},"id":{}}}"#,
                    id
                )
            }
        }
        "eval" => {
            let file_opt = extract_json_string(req_str, "file");
            if let Some(file) = file_opt {
                match run_jit(Path::new(&file)) {
                    Ok(out) => {
                        format!(
                            r#"{{"jsonrpc":"2.0","result":{{"exit_code":{},"has_errors":{}}},"id":{}}}"#,
                            out.exit_code, out.has_errors, id
                        )
                    }
                    Err(e) => {
                        format!(
                            r#"{{"jsonrpc":"2.0","error":{{"code":-32002,"message":"{}"}},"id":{}}}"#,
                            escape_json_string(&e), id
                        )
                    }
                }
            } else {
                format!(
                    r#"{{"jsonrpc":"2.0","error":{{"code":-32602,"message":"Missing file parameter for eval"}},"id":{}}}"#,
                    id
                )
            }
        }
        "shutdown" => {
            format!(r#"{{"jsonrpc":"2.0","result":{{"status":"shutdown"}},"id":{}}}"#, id)
        }
        _ => {
            format!(
                r#"{{"jsonrpc":"2.0","error":{{"code":-32601,"message":"Method not found"}},"id":{}}}"#,
                id
            )
        }
    }
}

fn extract_json_string(src: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let key_idx = src.find(&key)?;
    let after_key = &src[key_idx + key.len()..];
    let colon_idx = after_key.find(':')?;
    let val_str = after_key[colon_idx + 1..].trim();

    if let Some(inner) = val_str.strip_prefix('"') {
        let mut res = String::new();
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                return Some(res);
            } else if c == '\\' {
                if let Some(next) = chars.next() {
                    match next {
                        'n' => res.push('\n'),
                        'r' => res.push('\r'),
                        't' => res.push('\t'),
                        '\\' => res.push('\\'),
                        '"' => res.push('"'),
                        other => res.push(other),
                    }
                }
            } else {
                res.push(c);
            }
        }
    }
    None
}

fn extract_json_field(src: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let key_idx = src.find(&key)?;
    let after_key = &src[key_idx + key.len()..];
    let colon_idx = after_key.find(':')?;
    let val_str = after_key[colon_idx + 1..].trim();

    let end_idx = val_str
        .find([',', '}', ']', '\n'])
        .unwrap_or(val_str.len());
    Some(val_str[..end_idx].trim().to_string())
}

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}
