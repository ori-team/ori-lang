//! Persistent background compilation and evaluation daemon (CLI-DAEMON-1).
//!
//! The daemon speaks one JSON-RPC 2.0 request per stdin line. Requests are
//! decoded into typed DTOs so escaped strings, numeric/string IDs, malformed
//! JSON, and missing parameters cannot corrupt the response stream.

use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, Write};
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::pipeline::{fmt::format_source_text, frontend::run_check_source, native::run_jit};

const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");
const DAEMON_PROTOCOL: &str = "ori-daemon-v1";
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 256;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    id: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CheckCacheKey {
    file_name: String,
    content_hash: [u8; 32],
}

#[derive(Debug, Clone)]
struct CheckCacheEntry {
    has_errors: bool,
    error_count: usize,
    diagnostic_count: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DaemonStats {
    pub check_hits: usize,
    pub check_misses: usize,
    pub invalidations: usize,
}

/// Persistent daemon session retaining check results across JSON-RPC requests.
#[derive(Debug, Default)]
pub struct DaemonSession {
    check_cache: HashMap<CheckCacheKey, CheckCacheEntry>,
    cache_keys: VecDeque<CheckCacheKey>,
    pub stats: DaemonStats,
}

impl DaemonSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_request(&mut self, req_str: &str) -> (String, bool) {
        if req_str.len() > MAX_REQUEST_BYTES {
            return (
                error_response(Value::Null, -32600, "Request exceeds the 1 MiB limit"),
                false,
            );
        }
        let request: JsonRpcRequest = match serde_json::from_str(req_str) {
            Ok(request) => request,
            Err(_) => {
                return (
                    error_response(Value::Null, -32600, "Invalid Request"),
                    false,
                )
            }
        };
        let id = request.id.clone().unwrap_or(Value::Null);
        let valid_id = request.id.as_ref().is_none_or(is_valid_id);
        if request.jsonrpc.as_deref() != Some("2.0") || request.method.is_none() || !valid_id {
            let error_id = if valid_id { id } else { Value::Null };
            return (error_response(error_id, -32600, "Invalid Request"), false);
        }
        let method = request.method.as_deref().unwrap_or_default();

        match method {
            "version" => (
                success_response(
                    id,
                    json!({
                        "version": DAEMON_VERSION,
                        "protocol": DAEMON_PROTOCOL,
                        "cached_entries": self.check_cache.len(),
                        "check_hits": self.stats.check_hits,
                        "check_misses": self.stats.check_misses,
                    }),
                ),
                false,
            ),
            "check" => (self.handle_check(&request, id), false),
            "fmt" => (handle_fmt(&request, id), false),
            "eval" => (handle_eval(&request, id), false),
            "invalidate" => (self.handle_invalidate(&request, id), false),
            "stats" => (
                success_response(
                    id,
                    json!({
                        "check_hits": self.stats.check_hits,
                        "check_misses": self.stats.check_misses,
                        "invalidations": self.stats.invalidations,
                        "cached_entries": self.check_cache.len(),
                    }),
                ),
                false,
            ),
            "shutdown" => (success_response(id, json!({"status": "shutdown"})), true),
            _ => (error_response(id, -32601, "Method not found"), false),
        }
    }

    fn handle_check(&mut self, request: &JsonRpcRequest, id: Value) -> String {
        let file = param_string(request, "file");
        let code = param_string(request, "code");
        let (code, file_name) = if let Some(code) = code {
            if code.len() > MAX_SOURCE_BYTES {
                return error_response(id, -32602, "Source exceeds the 8 MiB limit");
            }
            (code, file.unwrap_or_else(|| "<daemon_check>".to_string()))
        } else if let Some(file) = file {
            match read_source_file(&file) {
                Ok(code) => (code, file),
                Err(error) => {
                    return error_response(id, -32001, &format!("failed to read `{file}`: {error}"));
                }
            }
        } else {
            return error_response(id, -32602, "Missing file or code parameter");
        };

        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();

        let key = CheckCacheKey {
            file_name: file_name.clone(),
            content_hash: hash,
        };

        if let Some(entry) = self.check_cache.get(&key) {
            self.stats.check_hits += 1;
            return success_response(
                id,
                json!({
                    "has_errors": entry.has_errors,
                    "error_count": entry.error_count,
                    "diagnostic_count": entry.diagnostic_count,
                    "cached": true,
                }),
            );
        }

        self.stats.check_misses += 1;
        match run_check_source(Path::new(&file_name), code) {
            Ok(check_out) => {
                let error_count = check_out
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .count();
                let diagnostic_count = check_out.diagnostics.len();
                let has_errors = check_out.has_errors;

                let entry = CheckCacheEntry {
                    has_errors,
                    error_count,
                    diagnostic_count,
                };

                if self.check_cache.len() >= MAX_CACHE_ENTRIES {
                    if let Some(oldest) = self.cache_keys.pop_front() {
                        self.check_cache.remove(&oldest);
                    }
                }
                self.cache_keys.push_back(key.clone());
                self.check_cache.insert(key, entry);

                success_response(
                    id,
                    json!({
                        "has_errors": has_errors,
                        "error_count": error_count,
                        "diagnostic_count": diagnostic_count,
                        "cached": false,
                    }),
                )
            }
            Err(error) => error_response(id, -32003, &error),
        }
    }

    fn handle_invalidate(&mut self, request: &JsonRpcRequest, id: Value) -> String {
        let file = param_string(request, "file");
        let count = if let Some(file_name) = file {
            let before = self.check_cache.len();
            self.check_cache.retain(|k, _| k.file_name != file_name);
            self.cache_keys.retain(|k| k.file_name != file_name);
            before.saturating_sub(self.check_cache.len())
        } else {
            let total = self.check_cache.len();
            self.check_cache.clear();
            self.cache_keys.clear();
            total
        };

        self.stats.invalidations += count;
        success_response(id, json!({ "invalidated": count }))
    }
}

/// Run the daemon event loop reading JSON-RPC lines from stdin and writing
/// responses to stdout. Blank lines are ignored; every non-blank line gets
/// exactly one response, including malformed input.
pub fn run_daemon() -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();
    let mut session = DaemonSession::new();

    loop {
        let line = match read_bounded_line(&mut stdin_lock, MAX_REQUEST_BYTES)
            .map_err(|e| format!("stdin read error: {e}"))?
        {
            None => break,
            Some(BoundedLine::TooLarge) => {
                let response =
                    error_response(Value::Null, -32600, "Request exceeds the 1 MiB limit");
                writeln!(stdout_lock, "{response}")
                    .map_err(|e| format!("stdout write error: {e}"))?;
                stdout_lock
                    .flush()
                    .map_err(|e| format!("stdout flush error: {e}"))?;
                continue;
            }
            Some(BoundedLine::InvalidUtf8) => {
                let response = error_response(Value::Null, -32600, "Invalid Request");
                writeln!(stdout_lock, "{response}")
                    .map_err(|e| format!("stdout write error: {e}"))?;
                stdout_lock
                    .flush()
                    .map_err(|e| format!("stdout flush error: {e}"))?;
                continue;
            }
            Some(BoundedLine::Text(line)) if line.trim().is_empty() => continue,
            Some(BoundedLine::Text(line)) => line,
        };

        let (response, should_shutdown) = session.handle_request(&line);
        writeln!(stdout_lock, "{response}").map_err(|e| format!("stdout write error: {e}"))?;
        stdout_lock
            .flush()
            .map_err(|e| format!("stdout flush error: {e}"))?;

        if should_shutdown {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
pub fn handle_jsonrpc_request(req_str: &str) -> (String, bool) {
    let mut session = DaemonSession::new();
    session.handle_request(req_str)
}

enum BoundedLine {
    Text(String),
    TooLarge,
    InvalidUtf8,
}

/// Read one newline-delimited request without ever buffering more than the
/// configured limit plus one byte. The complete oversized line is consumed so
/// the next request remains framed correctly.
fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> io::Result<Option<BoundedLine>> {
    let mut bytes = Vec::new();
    let mut too_large = false;
    let mut saw_input = false;

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            break;
        }
        saw_input = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        if !too_large {
            let remaining = max_bytes.saturating_sub(bytes.len());
            let copy_len = content_len.min(remaining.saturating_add(1));
            bytes.extend_from_slice(&available[..copy_len]);
            too_large = copy_len < content_len;
        }
        let consumed = content_len + usize::from(newline.is_some());
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }

    if !saw_input {
        return Ok(None);
    }
    if too_large || bytes.len() > max_bytes {
        return Ok(Some(BoundedLine::TooLarge));
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Ok(Some(match String::from_utf8(bytes) {
        Ok(line) => BoundedLine::Text(line),
        Err(_) => BoundedLine::InvalidUtf8,
    }))
}

fn handle_fmt(request: &JsonRpcRequest, id: Value) -> String {
    let Some(code) = param_string(request, "code") else {
        return error_response(id, -32602, "Missing code parameter");
    };
    success_response(id, json!({"formatted": format_source_text(&code)}))
}

fn handle_eval(request: &JsonRpcRequest, id: Value) -> String {
    let Some(file) = param_string(request, "file") else {
        return error_response(id, -32602, "Missing file parameter for eval");
    };
    if let Err(error) = ensure_source_file_size(&file) {
        return error_response(id, -32001, &format!("failed to read `{file}`: {error}"));
    }
    match run_jit(Path::new(&file)) {
        Ok(output) => success_response(
            id,
            json!({"exit_code": output.exit_code, "has_errors": output.has_errors}),
        ),
        Err(error) => error_response(id, -32002, &error),
    }
}

fn param_string(request: &JsonRpcRequest, name: &str) -> Option<String> {
    request
        .params
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|params| params.get(name))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn read_source_file(path: &str) -> Result<String, String> {
    ensure_source_file_size(path)?;
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("source exceeds the 8 MiB limit".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "source is not valid UTF-8".to_string())
}

fn ensure_source_file_size(path: &str) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err("source exceeds the 8 MiB limit".to_string());
    }
    Ok(())
}

fn is_valid_id(id: &Value) -> bool {
    id.is_null() || id.is_string() || id.is_number()
}

fn success_response(id: Value, result: Value) -> String {
    json!({"jsonrpc": "2.0", "result": result, "id": id}).to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({"jsonrpc": "2.0", "error": {"code": code, "message": message}, "id": id}).to_string()
}

#[cfg(test)]
mod tests {
    use super::{handle_jsonrpc_request, read_bounded_line, BoundedLine};
    use serde_json::Value;
    use std::io::Cursor;

    fn response(line: &str) -> Value {
        let (body, shutdown) = handle_jsonrpc_request(line);
        assert!(!shutdown);
        serde_json::from_str(&body).expect("daemon response must be valid JSON")
    }

    #[test]
    fn malformed_json_returns_json_rpc_error() {
        let value = response("{not-json");
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["id"], Value::Null);
    }

    #[test]
    fn escaped_ids_and_parameters_are_decoded_without_string_scanning() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "fmt",
            "params": {"code": "module app.main\n"},
            "id": "a,}\"b"
        })
        .to_string();
        let value = response(&request);
        assert_eq!(value["id"], "a,}\"b");
        assert!(value["result"]["formatted"].is_string());
    }

    #[test]
    fn shutdown_is_structural_not_substring_based() {
        let (body, shutdown) = handle_jsonrpc_request(
            r#"{"jsonrpc":"2.0","method":"fmt","params":{"code":"shutdown"},"id":1}"#,
        );
        assert!(!shutdown);
        assert!(body.contains("formatted"));

        let (_, shutdown) =
            handle_jsonrpc_request(r#"{"jsonrpc":"2.0","method":"shutdown","id":2}"#);
        assert!(shutdown);
    }

    #[test]
    fn oversized_requests_fail_before_json_parsing() {
        let request = "x".repeat(super::MAX_REQUEST_BYTES + 1);
        let value = response(&request);
        assert_eq!(value["error"]["code"], -32600);
        assert!(value["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("1 MiB"));
    }

    #[test]
    fn object_ids_are_rejected_by_json_rpc_envelope_validation() {
        let value = response(r#"{"jsonrpc":"2.0","method":"version","id":{"bad":true}}"#);
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["id"], Value::Null);
    }

    #[test]
    fn bounded_reader_preserves_empty_lines_and_next_request_after_oversize() {
        let mut reader = Cursor::new(b"\n12345\n{}\n");
        assert!(matches!(
            read_bounded_line(&mut reader, 8).expect("read succeeds"),
            Some(BoundedLine::Text(line)) if line.is_empty()
        ));
        assert!(matches!(
            read_bounded_line(&mut reader, 3).expect("read succeeds"),
            Some(BoundedLine::TooLarge)
        ));
        assert!(matches!(
            read_bounded_line(&mut reader, 8).expect("read succeeds"),
            Some(BoundedLine::Text(line)) if line == "{}"
        ));
    }

    #[test]
    fn session_caches_identical_check_requests_and_hits() {
        let mut session = super::DaemonSession::new();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "check",
            "params": {
                "file": "test.orl",
                "code": "module app.main\nmain()\n    const x: int = 1\nend\n"
            },
            "id": 1
        })
        .to_string();

        let (resp1, _) = session.handle_request(&request);
        let val1: Value = serde_json::from_str(&resp1).unwrap();
        assert_eq!(val1["result"]["cached"], false);
        assert_eq!(session.stats.check_misses, 1);
        assert_eq!(session.stats.check_hits, 0);

        let (resp2, _) = session.handle_request(&request);
        let val2: Value = serde_json::from_str(&resp2).unwrap();
        assert_eq!(val2["result"]["cached"], true);
        assert_eq!(val2["result"]["has_errors"], false);
        assert_eq!(session.stats.check_hits, 1);
        assert_eq!(session.stats.check_misses, 1);
    }

    #[test]
    fn session_invalidates_cache_on_code_modification() {
        let mut session = super::DaemonSession::new();
        let req1 = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "check",
            "params": {"file": "test.orl", "code": "module app.main\nmain()\nend\n"},
            "id": 1
        })
        .to_string();
        let (resp1, _) = session.handle_request(&req1);
        let val1: Value = serde_json::from_str(&resp1).unwrap();
        assert_eq!(val1["result"]["cached"], false);

        let req2 = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "check",
            "params": {"file": "test.orl", "code": "module app.main\nmain()\n    const x: int = 2\nend\n"},
            "id": 2
        })
        .to_string();
        let (resp2, _) = session.handle_request(&req2);
        let val2: Value = serde_json::from_str(&resp2).unwrap();
        assert_eq!(val2["result"]["cached"], false);
        assert_eq!(session.stats.check_misses, 2);
    }

    #[test]
    fn session_explicit_invalidation_and_stats() {
        let mut session = super::DaemonSession::new();
        let check_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "check",
            "params": {"file": "test.orl", "code": "module app.main\nmain()\nend\n"},
            "id": 1
        })
        .to_string();
        let _ = session.handle_request(&check_req);
        let _ = session.handle_request(&check_req); // hit

        let stats_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "stats",
            "id": 2
        })
        .to_string();
        let (stats_body, _) = session.handle_request(&stats_req);
        let stats_val: Value = serde_json::from_str(&stats_body).unwrap();
        assert_eq!(stats_val["result"]["check_hits"], 1);
        assert_eq!(stats_val["result"]["check_misses"], 1);
        assert_eq!(stats_val["result"]["cached_entries"], 1);

        let inv_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "invalidate",
            "params": {"file": "test.orl"},
            "id": 3
        })
        .to_string();
        let (inv_body, _) = session.handle_request(&inv_req);
        let inv_val: Value = serde_json::from_str(&inv_body).unwrap();
        assert_eq!(inv_val["result"]["invalidated"], 1);

        // Next check is a miss again
        let (resp3, _) = session.handle_request(&check_req);
        let val3: Value = serde_json::from_str(&resp3).unwrap();
        assert_eq!(val3["result"]["cached"], false);
        assert_eq!(session.stats.check_misses, 2);
    }
}
