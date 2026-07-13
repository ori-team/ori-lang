//! Cooperative line-level debug agent for DAP (Ori IDE).
//!
//! Enabled when `ORI_DEBUG_PORT` is set. Instrumented code calls
//! [`ori_debug_line`] at statement boundaries. The agent connects to the
//! DAP adapter (ori-dap) and pauses on breakpoints / step requests.
//!
//! Protocol (newline-delimited JSON over TCP to 127.0.0.1:PORT):
//! - runtime → adapter: `{"type":"hello"}`, `{"type":"stopped","reason":"…","line":N,"file":"…"}`
//! - adapter → runtime: `{"type":"setBreakpoints","file":"…","lines":[…]}`,
//!   `{"type":"continue"}`, `{"type":"step"}`, `{"type":"terminate"}`

use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::Duration,
};

use serde_json::{Value, json};

struct DebugState {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
    /// path (normalized) → lines (1-based)
    breakpoints: HashMap<String, HashSet<u32>>,
    step_mode: bool,
    terminated: bool,
    current_file: String,
    current_line: u32,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<DebugState>> = Mutex::new(None);
static LAST_STOP_LINE: AtomicU32 = AtomicU32::new(0);

fn normalize_path(path: &str) -> String {
    // Compare by file name + suffix if absolute paths differ (cwd relative).
    std::path::Path::new(path)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.replace('\\', "/"))
}

fn try_connect() -> Option<(TcpStream, BufReader<TcpStream>)> {
    let port = std::env::var("ORI_DEBUG_PORT").ok()?;
    let port: u16 = port.parse().ok()?;
    for _ in 0..50 {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
            let reader = BufReader::new(stream.try_clone().ok()?);
            return Some((stream, reader));
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

fn ensure_connected(state: &mut DebugState) {
    if state.stream.is_some() {
        return;
    }
    if let Some((mut stream, reader)) = try_connect() {
        let hello = json!({"type": "hello"});
        let _ = writeln!(stream, "{hello}");
        let _ = stream.flush();
        state.stream = Some(stream);
        state.reader = Some(reader);
        ENABLED.store(true, Ordering::SeqCst);
    }
}

fn send(state: &mut DebugState, msg: &Value) {
    if let Some(stream) = state.stream.as_mut() {
        let _ = writeln!(stream, "{msg}");
        let _ = stream.flush();
    }
}

fn poll_commands(state: &mut DebugState) {
    // Collect first, then apply — avoids holding `reader` mutably across `apply_command`.
    let mut pending: Vec<Value> = Vec::new();
    {
        let Some(reader) = state.reader.as_mut() else {
            return;
        };
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    state.terminated = true;
                    break;
                }
                Ok(_) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        pending.push(v);
                    }
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(_) => {
                    state.terminated = true;
                    break;
                }
            }
            // only one command per poll when not stepping
            if !state.step_mode {
                break;
            }
        }
    }
    for v in pending {
        apply_command(state, &v);
    }
}

fn apply_command(state: &mut DebugState, v: &Value) {
    match v.get("type").and_then(|t| t.as_str()) {
        Some("setBreakpoints") => {
            let file = v
                .get("file")
                .and_then(|f| f.as_str())
                .unwrap_or("")
                .to_string();
            let file = normalize_path(&file);
            let lines: HashSet<u32> = v
                .get("lines")
                .and_then(|l| l.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_u64().map(|n| n as u32))
                        .collect()
                })
                .unwrap_or_default();
            state.breakpoints.insert(file, lines);
        }
        Some("continue") => {
            state.step_mode = false;
        }
        Some("step") => {
            state.step_mode = true;
        }
        Some("terminate") => {
            state.terminated = true;
        }
        _ => {}
    }
}

fn should_stop(state: &DebugState, file: &str, line: u32) -> Option<&'static str> {
    if state.terminated {
        return None;
    }
    if state.step_mode {
        return Some("step");
    }
    let key = normalize_path(file);
    // Also try bare file name match
    let file_name = std::path::Path::new(file)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file);
    for (bp_file, lines) in &state.breakpoints {
        let bp_name = std::path::Path::new(bp_file)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(bp_file);
        if (bp_file == &key || bp_name == file_name || bp_file.ends_with(file))
            && lines.contains(&line)
        {
            // avoid re-stopping same line repeatedly without continue
            if LAST_STOP_LINE.load(Ordering::SeqCst) == line && !state.step_mode {
                return None;
            }
            return Some("breakpoint");
        }
    }
    None
}

fn wait_while_paused(state: &mut DebugState) {
    // Block until continue/step/terminate.
    // Use longer read timeout while paused.
    if let Some(stream) = state.stream.as_ref() {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3600)));
    }
    loop {
        if state.terminated {
            break;
        }
        // After stop we wait for continue or step.
        // step_mode true means "stop at next line" after continue from step request.
        // Protocol: on stop we clear step_mode; "step" sets it for next line.
        let mut line = String::new();
        let read_result = {
            let Some(reader) = state.reader.as_mut() else {
                break;
            };
            reader.read_line(&mut line)
        };
        match read_result {
            Ok(0) => {
                state.terminated = true;
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    let t = v
                        .get("type")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    apply_command(state, &v);
                    match t.as_deref() {
                        Some("continue") | Some("step") | Some("terminate") => break,
                        _ => {}
                    }
                }
            }
            Err(_) => {
                state.terminated = true;
                break;
            }
        }
    }
    if let Some(stream) = state.stream.as_ref() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    }
}

/// Called at the start of each instrumented statement.
///
/// # Safety
/// `file_ptr` must point to `file_len` valid UTF-8 bytes (or null with len 0).
#[no_mangle]
pub unsafe extern "C" fn ori_debug_line(file_ptr: *const u8, file_len: u32, line: u32) {
    if file_len == 0 || line == 0 {
        return;
    }
    // Fast path: no port configured → no-op
    if std::env::var_os("ORI_DEBUG_PORT").is_none() {
        return;
    }

    let file = if file_ptr.is_null() {
        String::new()
    } else {
        let slice = std::slice::from_raw_parts(file_ptr, file_len as usize);
        String::from_utf8_lossy(slice).into_owned()
    };

    let mut guard = match STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_none() {
        *guard = Some(DebugState {
            stream: None,
            reader: None,
            breakpoints: HashMap::new(),
            step_mode: false,
            terminated: false,
            current_file: String::new(),
            current_line: 0,
        });
    }
    let state = guard.as_mut().unwrap();
    if state.terminated {
        std::process::exit(1);
    }
    ensure_connected(state);
    if state.stream.is_none() {
        return;
    }

    // Drain any pending setBreakpoints while running
    poll_commands(state);

    if let Some(reason) = should_stop(state, &file, line) {
        state.current_file = file.clone();
        state.current_line = line;
        LAST_STOP_LINE.store(line, Ordering::SeqCst);
        // Clear step mode after stopping on a step
        if reason == "step" {
            state.step_mode = false;
        }
        send(
            state,
            &json!({
                "type": "stopped",
                "reason": reason,
                "line": line,
                "file": file,
            }),
        );
        wait_while_paused(state);
        if state.terminated {
            std::process::exit(0);
        }
        // After continue from breakpoint, ignore same line until we leave it
        if reason == "breakpoint" {
            // keep LAST_STOP_LINE so we don't re-hit until line changes
        }
    } else if LAST_STOP_LINE.load(Ordering::SeqCst) != line {
        // moved off previous stop line
        LAST_STOP_LINE.store(0, Ordering::SeqCst);
    }
}

/// Optional: force connect early (called from main wrapper if desired).
#[no_mangle]
pub extern "C" fn ori_debug_init() {
    if std::env::var_os("ORI_DEBUG_PORT").is_none() {
        return;
    }
    let mut guard = match STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_none() {
        *guard = Some(DebugState {
            stream: None,
            reader: None,
            breakpoints: HashMap::new(),
            step_mode: false,
            terminated: false,
            current_file: String::new(),
            current_line: 0,
        });
    }
    if let Some(state) = guard.as_mut() {
        ensure_connected(state);
        poll_commands(state);
    }
}
