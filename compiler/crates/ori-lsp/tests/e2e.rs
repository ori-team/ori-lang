//! Etapa 6.3 — E2E LSP test harness.
//!
//! Drives the `ori-lsp` binary as a subprocess over stdio using the LSP base
//! protocol (Content-Length framing). A reader thread parses frames and feeds
//! them through an `mpsc` channel so the test can wait for responses/notifications
//! with a timeout instead of blocking forever.
//!
//! Gate: minimum 8 E2E scenarios passing — initialize, didOpen + diagnostics,
//! hover, definition, completion, formatting, rename, shutdown.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_MS: u64 = 8000;

struct LspClient {
    child: Child,
    stdin: Option<Box<dyn Write + Send>>,
    rx: mpsc::Receiver<Value>,
    next_id: i64,
}

impl LspClient {
    fn new() -> std::io::Result<Self> {
        let exe = lsp_exe();
        let mut child = Command::new(exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin: Box<dyn Write + Send> = Box::new(child.stdin.take().unwrap());
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel::<Value>();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut content_length: Option<usize> = None;
                let mut saw_header = false;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let trimmed = line.trim_end_matches(['\r', '\n']);
                    if trimmed.is_empty() {
                        if content_length.is_some() {
                            saw_header = true;
                        }
                        break;
                    }
                    if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
                        content_length = rest.parse().ok();
                    }
                }
                if !saw_header {
                    continue;
                }
                let len = match content_length {
                    Some(l) => l,
                    None => continue,
                };
                let mut buf = vec![0u8; len];
                if reader.read_exact(&mut buf).is_err() {
                    return;
                }
                if let Ok(v) = serde_json::from_slice::<Value>(&buf) {
                    if tx.send(v).is_err() {
                        return;
                    }
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            rx,
            next_id: 1,
        })
    }

    fn send(&mut self, msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        if let Some(stdin) = self.stdin.as_mut() {
            write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
            stdin.flush().unwrap();
        }
    }

    fn request(&mut self, method: &str, params: Value) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        id
    }

    /// Send a request with no `params` field (required by methods like
    /// `shutdown` whose params type is `()` and reject explicit `null`).
    fn request_no_params(&mut self, method: &str) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        }));
        id
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }));
    }

    fn read_response(&mut self, expected_id: i64, timeout_ms: u64) -> Value {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while Instant::now() < deadline {
            match self.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => {
                    if frame.get("id") == Some(&Value::from(expected_id))
                        && frame.get("method").is_none()
                    {
                        return frame;
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    panic!("server closed before responding to id {expected_id}")
                }
            }
        }
        panic!("timeout waiting for response id {expected_id}");
    }

    fn read_notification(&mut self, method: &str, timeout_ms: u64) -> Option<Value> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while Instant::now() < deadline {
            match self.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) if frame.get("method") == Some(&Value::from(method)) => {
                    return Some(frame)
                }
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
        None
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn uri_for(dir: &std::path::Path, name: &str) -> String {
    let path = dir.join(name);
    let path_str = path.to_string_lossy().replace('\\', "/");
    if path_str.starts_with('/') {
        format!("file://{}", path_str)
    } else {
        format!("file:///{}", path_str)
    }
}

/// Etapa 6.3 gate: a single LSP session exercising 8 scenarios in sequence.
#[test]
fn e2e_lsp_session_covers_8_scenarios() {
    let dir = tempfile_dir();
    let doc_uri = uri_for(&dir, "e2e_main.orl");
    let root_uri = uri_for(&dir, "");

    let source = "module app.main\n\
import ori.io = io\n\
main()\n\
    io.print(string(42))\n\
end\n";

    let mut lsp = LspClient::new().expect("spawn ori-lsp");

    // 1. initialize
    let init_id = lsp.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": root_uri,
            "capabilities": {},
            "clientInfo": {"name": "e2e-test", "version": "0.1"},
        }),
    );
    let init_resp = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    assert!(
        init_resp["result"]["capabilities"].is_object(),
        "initialize must return server capabilities: {init_resp}"
    );

    lsp.notify("initialized", json!({}));

    // 2. didOpen + diagnostics (clean file: diagnostics empty or absent)
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": doc_uri,
                "languageId": "ori",
                "version": 1,
                "text": source,
            },
        }),
    );
    // The server debounces diagnostics (~300ms). Wait up to 4s; a clean file
    // may publish an empty diagnostics list — either is acceptable.
    if let Some(diag) = lsp.read_notification("textDocument/publishDiagnostics", 4000) {
        assert_eq!(diag["params"]["uri"], doc_uri, "diagnostics uri matches");
    }

    // 3. hover over `io` (line 3, char 4)
    let hover_id = lsp.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": doc_uri},
            "position": {"line": 3, "character": 4},
        }),
    );
    let hover_resp = lsp.read_response(hover_id, DEFAULT_TIMEOUT_MS);
    assert!(
        hover_resp.get("result").is_some(),
        "hover must respond: {hover_resp}"
    );

    // 4. definition at `io`
    let def_id = lsp.request(
        "textDocument/definition",
        json!({
            "textDocument": {"uri": doc_uri},
            "position": {"line": 3, "character": 4},
        }),
    );
    let def_resp = lsp.read_response(def_id, DEFAULT_TIMEOUT_MS);
    assert!(
        def_resp.get("result").is_some(),
        "definition must respond: {def_resp}"
    );

    // 5. completion inside the body (Default context)
    let comp_id = lsp.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": doc_uri},
            "position": {"line": 3, "character": 8},
            "context": {"triggerKind": 1},
        }),
    );
    let comp_resp = lsp.read_response(comp_id, DEFAULT_TIMEOUT_MS);
    assert!(
        comp_resp.get("result").is_some(),
        "completion must respond: {comp_resp}"
    );

    // 6. formatting
    let fmt_id = lsp.request(
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": doc_uri},
            "options": {"tabSize": 4, "insertSpaces": true},
        }),
    );
    let fmt_resp = lsp.read_response(fmt_id, DEFAULT_TIMEOUT_MS);
    assert!(
        fmt_resp.get("result").is_some(),
        "formatting must respond: {fmt_resp}"
    );

    // 7. rename `main` (line 2, char 6)
    let rename_id = lsp.request(
        "textDocument/rename",
        json!({
            "textDocument": {"uri": doc_uri},
            "position": {"line": 2, "character": 6},
            "newName": "renamed_main",
        }),
    );
    let rename_resp = lsp.read_response(rename_id, DEFAULT_TIMEOUT_MS);
    assert!(
        rename_resp.get("result").is_some(),
        "rename must respond: {rename_resp}"
    );

    // 8. shutdown
    let shutdown_id = lsp.request_no_params("shutdown");
    let shutdown_resp = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    assert!(
        shutdown_resp.get("result").is_some(),
        "shutdown must respond: {shutdown_resp}"
    );

    lsp.notify("exit", json!(null));
    // Close stdin so the server sees EOF and terminates its read loop.
    lsp.stdin.take();
    let status = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
    assert!(
        status.is_some_and(|s| s.success()),
        "server should exit cleanly after shutdown+exit, got {status:?}"
    );
}

/// Etapa 6.3 scenario: diagnostics fire for a file with a type error.
#[test]
fn e2e_lsp_publishes_diagnostics_for_type_error() {
    let dir = tempfile_dir();
    let doc_uri = uri_for(&dir, "e2e_err.orl");
    let root_uri = uri_for(&dir, "");
    // `x` is int; assigning a string is a type mismatch.
    let source = "module app.main\n\
main()\n\
    var x: int = 0\n\
    x = \"oops\"\n\
end\n";

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": doc_uri,
                "languageId": "ori",
                "version": 1,
                "text": source,
            },
        }),
    );

    let diag = lsp.read_notification("textDocument/publishDiagnostics", 6000);
    assert!(
        diag.is_some(),
        "server must publish diagnostics for a type error"
    );
    let diag = diag.unwrap();
    assert_eq!(diag["params"]["uri"], doc_uri);
    let diagnostics = diag["params"]["diagnostics"].as_array();
    assert!(
        diagnostics.is_some_and(|d| !d.is_empty()),
        "diagnostics list must be non-empty for a type error: {}",
        diag["params"]["diagnostics"]
    );
}

/// Etapa 6.3 scenario: document symbols for a file with a function.
#[test]
fn e2e_lsp_returns_document_symbols() {
    let dir = tempfile_dir();
    let doc_uri = uri_for(&dir, "e2e_sym.orl");
    let root_uri = uri_for(&dir, "");
    let source = "module app.main\n\
import ori.io = io\n\
greet(name: string) -> string\n\
    return \"hi \" + name\n\
end\n\
main()\n\
    io.print(greet(\"ada\"))\n\
end\n";

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": doc_uri,
                "languageId": "ori",
                "version": 1,
                "text": source,
            },
        }),
    );

    let sym_id = lsp.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": doc_uri}}),
    );
    let sym_resp = lsp.read_response(sym_id, DEFAULT_TIMEOUT_MS);
    assert!(
        sym_resp.get("result").is_some(),
        "documentSymbol must respond: {sym_resp}"
    );
    let symbols = sym_resp["result"].as_array();
    assert!(
        symbols.is_some_and(|s| !s.is_empty()),
        "document symbols must be non-empty for a file with functions"
    );
}

fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ori-lsp-e2e-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Etapa 6.4 E2E: `textDocument/formatting` is idempotent — applying the
/// formatting edits once and formatting again yields no further edits (the
/// formatted text is a fixed point of the formatter).
#[test]
fn e2e_lsp_formatting_is_idempotent() {
    let dir = tempfile_dir();
    let doc_uri = uri_for(&dir, "e2e_fmt.orl");
    let root_uri = uri_for(&dir, "");
    // Unformatted: body statements at column 0 instead of indented.
    let unformatted = "module app.main\n\
import ori.io = io\n\
import ori.task = task\n\
\n\
async work(n: int) -> int\n\
await task.sleep(1)\n\
return n * 2\n\
end\n\
\n\
main()\n\
const r: int = task.block_on(work(21))\n\
io.print(string(r))\n\
end\n";

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": doc_uri,
                "languageId": "ori",
                "version": 1,
                "text": unformatted,
            },
        }),
    );

    // First formatting pass: must produce edits (the source is unformatted).
    let fmt1_id = lsp.request(
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": doc_uri},
            "options": {"tabSize": 4, "insertSpaces": true},
        }),
    );
    let fmt1 = lsp.read_response(fmt1_id, DEFAULT_TIMEOUT_MS);
    let edits1 = fmt1["result"].as_array();
    assert!(
        edits1.is_some(),
        "first formatting result must be an array: {fmt1}"
    );
    let edits1 = edits1.unwrap();
    assert!(
        !edits1.is_empty(),
        "first formatting of unformatted source must produce edits: {fmt1}"
    );
    let formatted = apply_edits(unformatted, edits1);

    // Push the formatted text back to the server via didChange.
    lsp.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": doc_uri, "version": 2},
            "contentChanges": [{"text": formatted}],
        }),
    );

    // Second formatting pass: must produce no edits (fixed point).
    let fmt2_id = lsp.request(
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": doc_uri},
            "options": {"tabSize": 4, "insertSpaces": true},
        }),
    );
    let fmt2 = lsp.read_response(fmt2_id, DEFAULT_TIMEOUT_MS);
    let result2 = &fmt2["result"];
    let idempotent = result2.is_null() || result2.as_array().is_some_and(|e| e.is_empty());
    assert!(
        idempotent,
        "second formatting of already-formatted source must produce no edits (idempotent), got: {fmt2}"
    );
}

/// Etapa 6.4 E2E: `textDocument/formatting` on an unformatted document returns
/// non-empty edits (the formatter actually does work).
#[test]
fn e2e_lsp_formatting_emits_edits_for_unformatted() {
    let dir = tempfile_dir();
    let doc_uri = uri_for(&dir, "e2e_fmt_un.orl");
    let root_uri = uri_for(&dir, "");
    // Unformatted: body statements at column 0 instead of indented.
    let unformatted = "module app.main\n\
import ori.io = io\n\
\n\
main()\n\
io.print(string(42))\n\
end\n";

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": doc_uri,
                "languageId": "ori",
                "version": 1,
                "text": unformatted,
            },
        }),
    );

    let fmt_id = lsp.request(
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": doc_uri},
            "options": {"tabSize": 4, "insertSpaces": true},
        }),
    );
    let fmt_resp = lsp.read_response(fmt_id, DEFAULT_TIMEOUT_MS);
    let edits = fmt_resp["result"].as_array();
    assert!(
        edits.is_some_and(|e| !e.is_empty()),
        "formatting an unformatted document must return non-empty edits: {fmt_resp}"
    );
}

/// Resolve the `ori-lsp` binary path. Cargo exposes the freshly built binary
/// to integration tests at compile time. The target name keeps its hyphen in
/// the environment key (`CARGO_BIN_EXE_ori-lsp`).
fn lsp_exe() -> std::path::PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_ori-lsp") {
        return std::path::PathBuf::from(path);
    }
    let manifest = env!("CARGO_MANIFEST_DIR");
    let exe_name = if cfg!(windows) {
        "ori-lsp.exe"
    } else {
        "ori-lsp"
    };
    std::path::Path::new(manifest)
        .join("../../target/debug")
        .join(exe_name)
}

/// Poll `child.try_wait()` until it exits or the deadline passes. Returns
/// `None` on timeout (the `Drop` impl of `LspClient` kills the child).
fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Apply a list of LSP `TextEdit`s to a document string. Edits are applied in
/// reverse order of start position so earlier offsets stay valid. Each edit's
/// range is `{start:{line,character}, end:{line,character}}` and replaces with
/// `newText`. Whole-document edits (`range` null/absent or full range) replace
/// the entire text.
fn apply_edits(text: &str, edits: &[Value]) -> String {
    // Whole-document replacement: a single edit whose range covers the whole
    // document, or whose `range` is absent.
    if edits.len() == 1 && edits[0].get("range").is_none() {
        return edits[0]["newText"].as_str().unwrap_or("").to_string();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    // Build a flat list of (byte_offset_start, byte_offset_end, new_text).
    let mut flat: Vec<(usize, usize, String)> = Vec::new();
    for edit in edits {
        let new_text = edit["newText"].as_str().unwrap_or("").to_string();
        let range = &edit["range"];
        let start_line = range["start"]["line"].as_i64().unwrap_or(0) as usize;
        let start_char = range["start"]["character"].as_i64().unwrap_or(0) as usize;
        let end_line = range["end"]["line"].as_i64().unwrap_or(0) as usize;
        let end_char = range["end"]["character"].as_i64().unwrap_or(0) as usize;
        let start_off = line_char_to_offset(&lines, start_line, start_char);
        let end_off = line_char_to_offset(&lines, end_line, end_char);
        flat.push((start_off, end_off, new_text));
    }
    flat.sort_by(|a, b| a.0.cmp(&b.0));
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start, end, new_text) in &flat {
        if *start < cursor {
            // Overlapping edits are not expected from the formatter; bail.
            return text.to_string();
        }
        result.push_str(&text[cursor..*start]);
        result.push_str(new_text);
        cursor = *end;
    }
    result.push_str(&text[cursor..]);
    result
}

fn line_char_to_offset(lines: &[&str], line: usize, ch: usize) -> usize {
    let mut off = 0usize;
    for (i, l) in lines.iter().enumerate() {
        if i == line {
            return off + ch.min(l.len());
        }
        off += l.len() + 1; // +1 for the '\n'
    }
    off
}

// ── Etapa 6.1 / 6.2 / 6.5 E2E scenarios ─────────────────────────────────────

/// Write a file into the test workspace dir.
fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap_or(dir)).ok();
    std::fs::write(&path, content).expect("write workspace file");
    path
}

/// Drain any pending `publishDiagnostics` notifications for `uri` so they do
/// not interfere with later reads. Returns the last seen diagnostics array.
fn drain_diagnostics(lsp: &mut LspClient, uri: &str, timeout_ms: u64) -> Vec<Value> {
    let mut last = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if let Some(frame) = lsp.read_notification("textDocument/publishDiagnostics", 300) {
            if frame["params"]["uri"] == Value::from(uri) {
                if let Some(arr) = frame["params"]["diagnostics"].as_array() {
                    last = arr.clone();
                }
            }
        } else {
            break;
        }
    }
    last
}

/// Etapa 6.5: opening a file that participates in an import cycle surfaces
/// `project.circular_import` on the opened file (even though the diagnostic
/// label sits on the back-edge import in the other file).
#[test]
fn e2e_lsp_circular_import_diagnostic() {
    let dir = tempfile_dir();
    let root_uri = uri_for(&dir, "");
    write_file(
        &dir,
        "cyc_a.orl",
        "module app.cyc_a\nimport app.cyc_b\nf()\nend\n",
    );
    write_file(
        &dir,
        "cyc_b.orl",
        "module app.cyc_b\nimport app.cyc_a\ng()\nend\n",
    );
    let a_uri = uri_for(&dir, "cyc_a.orl");

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));

    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": a_uri,
                "languageId": "ori",
                "version": 1,
                "text": "module app.cyc_a\nimport app.cyc_b\nf()\nend\n",
            },
        }),
    );

    let diags = drain_diagnostics(&mut lsp, &a_uri, 6000);
    let codes: Vec<String> = diags
        .iter()
        .filter_map(|d| d["code"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        codes.iter().any(|c| c == "project.circular_import"),
        "expected project.circular_import in {codes:?} (full diagnostics: {diags:?})"
    );

    let shutdown_id = lsp.request_no_params("shutdown");
    let _ = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("exit", json!(null));
    lsp.stdin.take();
    let _ = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
}

/// Etapa 6.1: go-to-definition on an imported struct resolves cross-file to
/// the imported module's URI.
#[test]
fn e2e_lsp_cross_file_goto_definition() {
    let dir = tempfile_dir();
    let root_uri = uri_for(&dir, "");
    write_file(
        &dir,
        "crossdef_lib.orl",
        "module app.crossdef_lib\nstruct Point\n    x: int\n    y: int\nend\n",
    );
    let main_src = "module app.crossdef_main\nimport app.crossdef_lib = lib\nmain()\n    var p: lib.Point = Point { x: 1, y: 2 }\nend\n";
    let main_uri = uri_for(&dir, "crossdef_main.orl");
    let lib_uri = uri_for(&dir, "crossdef_lib.orl");

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));

    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "ori",
                "version": 1,
                "text": main_src,
            },
        }),
    );
    // Wait for the validate pass to store the project semantic index.
    let _ = drain_diagnostics(&mut lsp, &main_uri, 6000);

    // Cursor on `Point` in `var p: lib.Point` (line 3).
    // Line 3 = "    var p: lib.Point = Point { x: 1, y: 2 }". `lib.` ends at
    // char 14, so `Point` starts at char 15; char 18 lands on "i" of "Point".
    let def_id = lsp.request(
        "textDocument/definition",
        json!({
            "textDocument": {"uri": main_uri},
            "position": {"line": 3, "character": 18},
        }),
    );
    let def_resp = lsp.read_response(def_id, DEFAULT_TIMEOUT_MS);
    let result = &def_resp["result"];
    let target_uri = result
        .get("uri")
        .and_then(|u| u.as_str())
        .or_else(|| {
            // Scalar form is {uri, range}; array form is a list.
            result.as_array().and_then(|arr| {
                arr.first()
                    .and_then(|loc| loc.get("uri"))
                    .and_then(|u| u.as_str())
            })
        })
        .unwrap_or("");
    assert!(
        target_uri.ends_with("crossdef_lib.orl"),
        "cross-file goto-definition should resolve to crossdef_lib.orl, got uri `{target_uri}`: {def_resp}"
    );

    let shutdown_id = lsp.request_no_params("shutdown");
    let _ = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("exit", json!(null));
    lsp.stdin.take();
    let _ = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
    let _ = lib_uri;
}

/// Etapa 6.2: completion after `receiver.` lists the fields of the receiver's
/// declared struct type (type-aware dot completion).
#[test]
fn e2e_lsp_type_aware_dot_completion() {
    let dir = tempfile_dir();
    let root_uri = uri_for(&dir, "");
    let src = "module app.dotcomp\nstruct Point\n    x: int\n    y: int\nend\nmain()\n    var p: Point = Point { x: 1, y: 2 }\n    p.\nend\n";
    let main_uri = uri_for(&dir, "dotcomp_main.orl");

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));

    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "ori",
                "version": 1,
                "text": src,
            },
        }),
    );
    let _ = drain_diagnostics(&mut lsp, &main_uri, 6000);

    // Cursor right after `p.` on the last body line.
    // Line 7 = "    p." — 4 spaces + "p." = 6 chars; cursor at character 6.
    let comp_id = lsp.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": main_uri},
            "position": {"line": 7, "character": 6},
            "context": {"triggerKind": 1},
        }),
    );
    let comp_resp = lsp.read_response(comp_id, DEFAULT_TIMEOUT_MS);
    let items = comp_resp["result"]
        .as_array()
        .or_else(|| comp_resp["result"]["items"].as_array())
        .unwrap_or_else(|| panic!("completion must return an array: {comp_resp}"));
    let labels: Vec<String> = items
        .iter()
        .filter_map(|i| i["label"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        labels.iter().any(|l| l == "x"),
        "type-aware dot completion must list field `x` of Point, got {labels:?}: {comp_resp}"
    );
    assert!(
        labels.iter().any(|l| l == "y"),
        "type-aware dot completion must list field `y` of Point, got {labels:?}: {comp_resp}"
    );

    let shutdown_id = lsp.request_no_params("shutdown");
    let _ = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("exit", json!(null));
    lsp.stdin.take();
    let _ = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
}

/// Etapa 6.2: find-references on an imported symbol returns locations in the
/// importing file (cross-file scan via the project semantic index).
#[test]
fn e2e_lsp_cross_file_find_references() {
    let dir = tempfile_dir();
    let root_uri = uri_for(&dir, "");
    write_file(
        &dir,
        "findref_lib.orl",
        "module app.findref_lib\nstruct Point\n    x: int\n    y: int\nend\n",
    );
    let main_src = "module app.findref_main\nimport app.findref_lib = lib\nmain()\n    var p: lib.Point = Point { x: 1, y: 2 }\nend\n";
    let main_uri = uri_for(&dir, "findref_main.orl");

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));

    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "ori",
                "version": 1,
                "text": main_src,
            },
        }),
    );
    let _ = drain_diagnostics(&mut lsp, &main_uri, 6000);

    // Cursor on the second `Point` (the constructor), line 3 char 25.
    // Line 3 = "    var p: lib.Point = Point { x: 1, y: 2 }"
    // "    var p: lib.Point = " = 23 chars, "Point" starts at index 23.
    let ref_id = lsp.request(
        "textDocument/references",
        json!({
            "textDocument": {"uri": main_uri},
            "position": {"line": 3, "character": 25},
            "context": {"includeDeclaration": true},
        }),
    );
    let ref_resp = lsp.read_response(ref_id, DEFAULT_TIMEOUT_MS);
    let locations = ref_resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("references must return an array: {ref_resp}"));
    let in_main: Vec<&Value> = locations
        .iter()
        .filter(|loc| {
            loc["uri"]
                .as_str()
                .is_some_and(|u| u.ends_with("findref_main.orl"))
        })
        .collect();
    assert!(
        !in_main.is_empty(),
        "cross-file find-references must include occurrences in findref_main.orl: {ref_resp}"
    );

    let shutdown_id = lsp.request_no_params("shutdown");
    let _ = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("exit", json!(null));
    lsp.stdin.take();
    let _ = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
}

fn stdlib_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../stdlib")
}

/// Stdlib Layer 2: hover on `su.is_empty` includes signature text.
#[test]
fn e2e_lsp_stdlib_layer2_hover() {
    std::env::set_var("ORI_STDLIB_ROOT", stdlib_root());
    let dir = tempfile_dir();
    let root_uri = uri_for(&dir, "");
    let src =
        "module app.main\nimport ori.string = su\nmain() -> void\n    su.is_empty(\"x\")\nend\n";
    let main_uri = uri_for(&dir, "stdlib_hover.orl");

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "ori",
                "version": 1,
                "text": src,
            },
        }),
    );
    let _ = drain_diagnostics(&mut lsp, &main_uri, 8000);

    // Cursor on `is_empty` in `su.is_empty("x")` — line 3, inside identifier.
    let hover_id = lsp.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": main_uri},
            "position": {"line": 3, "character": 7},
        }),
    );
    let hover_resp = lsp.read_response(hover_id, DEFAULT_TIMEOUT_MS);
    let hover_text = hover_resp["result"]["contents"]["value"]
        .as_str()
        .or_else(|| hover_resp["result"]["contents"].as_str())
        .unwrap_or("");
    assert!(
        hover_text.contains("is_empty") || hover_text.contains("string"),
        "stdlib hover must describe is_empty: {hover_resp}"
    );

    let shutdown_id = lsp.request_no_params("shutdown");
    let _ = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("exit", json!(null));
    lsp.stdin.take();
    let _ = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
}

/// Incremental sync: a ranged edit keeps the document parseable for completion.
#[test]
fn e2e_lsp_incremental_edit_completion() {
    let dir = tempfile_dir();
    let root_uri = uri_for(&dir, "");
    let initial = "module app.main\nimport ori.io = io\nmain() -> void\n    io.\nend\n";
    let main_uri = uri_for(&dir, "incr.orl");

    let mut lsp = LspClient::new().expect("spawn ori-lsp");
    let init_id = lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
    );
    let _ = lsp.read_response(init_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("initialized", json!({}));

    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "ori",
                "version": 1,
                "text": initial,
            },
        }),
    );
    let _ = drain_diagnostics(&mut lsp, &main_uri, 6000);

    // Replace `io.` line body with `io.print("")` via incremental edit on line 3.
    lsp.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": main_uri, "version": 2},
            "contentChanges": [{
                "range": {
                    "start": {"line": 3, "character": 4},
                    "end": {"line": 3, "character": 5},
                },
                "text": "print(\"\")",
            }],
        }),
    );
    let _ = drain_diagnostics(&mut lsp, &main_uri, 6000);

    let comp_id = lsp.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": main_uri},
            "position": {"line": 3, "character": 6},
        }),
    );
    let comp_resp = lsp.read_response(comp_id, DEFAULT_TIMEOUT_MS);
    let items = comp_resp["result"]
        .as_array()
        .or_else(|| comp_resp["result"]["items"].as_array());
    assert!(
        items.is_some_and(|a| !a.is_empty()),
        "completion after incremental edit must return items: {comp_resp}"
    );

    let shutdown_id = lsp.request_no_params("shutdown");
    let _ = lsp.read_response(shutdown_id, DEFAULT_TIMEOUT_MS);
    lsp.notify("exit", json!(null));
    lsp.stdin.take();
    let _ = wait_with_timeout(&mut lsp.child, Duration::from_secs(5));
}
