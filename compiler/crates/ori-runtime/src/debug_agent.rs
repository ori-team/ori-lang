//! Cooperative line-level debug agent for DAP (Ori IDE).
//!
//! Enabled when `ORI_DEBUG_PORT` is set. Instrumented code calls
//! [`ori_debug_line`] at statement boundaries. The agent connects to the
//! `ori debug --dap` adapter and pauses on breakpoints / step requests.
//!
//! Protocol (newline-delimited JSON over TCP to 127.0.0.1:PORT):
//! - runtime → adapter: `{"type":"hello"}`, `{"type":"stopped","reason":"…","line":N,"file":"…","stackTrace":[…],"variables":[…]}`
//!   Variables may use qualified names for struct fields, optional/result and
//!   enum payloads, collection metadata, and bounded indexed list elements;
//!   managed strings/bytes receive bounded content previews, and static or
//!   foreign buffers are read only after an exact length is registered. Opaque
//!   pointer-backed values are reported as a safe `<managed>` summary.
//! - adapter → runtime: `{"type":"setBreakpoints","file":"…","lines":[…]}`,
//!   `{"type":"continue"}`, `{"type":"step"}`, `{"type":"terminate"}`

use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use serde_json::{json, Value};

struct DebugState {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
    /// path (normalized) → lines (1-based)
    breakpoints: HashMap<String, HashSet<u32>>,
    step_mode: bool,
    terminated: bool,
    current_file: String,
    current_line: u32,
    stack: Vec<DebugFrame>,
}

struct DebugFrame {
    name: String,
    variables: Vec<DebugVariable>,
}

struct DebugVariable {
    name: String,
    type_tag: u32,
    raw_value: u64,
    /// Optional display text for managed payload previews. Keeping this
    /// separate from `raw_value` means the debugger never needs to expose a
    /// target pointer to the adapter.
    display_value: Option<String>,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<DebugState>> = Mutex::new(None);
static LAST_STOP_LINE: AtomicU32 = AtomicU32::new(0);
/// Addresses of compiler-emitted string literals and their known byte lengths.
/// Registration lets the debugger inspect static data without probing an
/// arbitrary foreign pointer.
static STATIC_PAYLOADS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

fn static_payloads() -> &'static Mutex<HashMap<usize, usize>> {
    STATIC_PAYLOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn normalize_path(path: &str) -> String {
    // Compare by file name + suffix if absolute paths differ (cwd relative).
    std::path::Path::new(path)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.replace('\\', "/"))
}

fn new_debug_state() -> DebugState {
    DebugState {
        stream: None,
        reader: None,
        breakpoints: HashMap::new(),
        step_mode: false,
        terminated: false,
        current_file: String::new(),
        current_line: 0,
        stack: Vec::new(),
    }
}

/// Reads a UTF-8 debug string supplied by generated native code.
///
/// # Safety
/// `ptr` must be null only when `len` is zero; otherwise it must point to
/// `len` readable bytes for the duration of this call.
unsafe fn read_debug_text(ptr: *const u8, len: u32) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let slice = std::slice::from_raw_parts(ptr, len as usize);
    String::from_utf8_lossy(slice).into_owned()
}

fn debug_type_name(type_tag: u32) -> &'static str {
    match type_tag {
        1 => "bool",
        2 => "int",
        3 => "uint",
        4 => "float",
        5 => "float32",
        6 => "managed",
        7 => "string",
        8 => "bytes",
        _ => "scalar",
    }
}

fn debug_value(type_tag: u32, raw_value: u64) -> Value {
    match type_tag {
        1 => json!(raw_value != 0),
        2 => json!(raw_value as i64),
        3 => json!(raw_value),
        4 => json!(f64::from_bits(raw_value)),
        5 => json!(f32::from_bits(raw_value as u32)),
        6 => json!(if raw_value == 0 { "null" } else { "<managed>" }),
        7 => json!(if raw_value == 0 {
            "null"
        } else {
            "<unavailable>"
        }),
        8 => json!(if raw_value == 0 {
            "null"
        } else {
            "<unavailable>"
        }),
        _ => json!(raw_value),
    }
}

fn stack_trace_json(stack: &[DebugFrame]) -> Vec<Value> {
    stack
        .iter()
        .rev()
        .enumerate()
        .map(|(id, frame)| json!({"id": id, "name": frame.name}))
        .collect()
}

fn variables_json(frame: Option<&DebugFrame>) -> Vec<Value> {
    frame
        .map(|frame| {
            frame
                .variables
                .iter()
                .map(|variable| {
                    json!({
                        "name": variable.name,
                        "type": debug_type_name(variable.type_tag),
                        "value": variable
                            .display_value
                            .as_deref()
                            .map_or_else(
                                || debug_value(variable.type_tag, variable.raw_value),
                                |display| json!(display),
                            ),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
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

    let file = read_debug_text(file_ptr, file_len);

    let mut guard = match STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_none() {
        *guard = Some(new_debug_state());
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
                "stackTrace": stack_trace_json(&state.stack),
                "variables": variables_json(state.stack.last()),
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
        *guard = Some(new_debug_state());
    }
    if let Some(state) = guard.as_mut() {
        ensure_connected(state);
        poll_commands(state);
    }
}

/// Register entry into an instrumented Ori function, including an async step.
///
/// # Safety
/// `name_ptr` must point to `name_len` valid UTF-8 bytes (or be null with len 0).
#[no_mangle]
pub unsafe extern "C" fn ori_debug_enter(name_ptr: *const u8, name_len: u32) {
    if std::env::var_os("ORI_DEBUG_PORT").is_none() {
        return;
    }
    let mut guard = match STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let state = guard.get_or_insert_with(new_debug_state);
    ensure_connected(state);
    if state.stream.is_some() {
        poll_commands(state);
        state.stack.push(DebugFrame {
            name: read_debug_text(name_ptr, name_len),
            variables: Vec::new(),
        });
    }
}

/// Pop the current instrumented Ori function.
#[no_mangle]
pub extern "C" fn ori_debug_leave() {
    if std::env::var_os("ORI_DEBUG_PORT").is_none() {
        return;
    }
    let Ok(mut guard) = STATE.lock() else {
        return;
    };
    let Some(state) = guard.as_mut() else {
        return;
    };
    state.stack.pop();
}

/// Update one scalar visible in the current debug frame.
///
/// # Safety
/// `name_ptr` must point to `name_len` valid UTF-8 bytes (or be null with len 0).
#[no_mangle]
pub unsafe extern "C" fn ori_debug_variable(
    name_ptr: *const u8,
    name_len: u32,
    type_tag: u32,
    raw_value: u64,
) {
    if std::env::var_os("ORI_DEBUG_PORT").is_none() {
        return;
    }
    let Ok(mut guard) = STATE.lock() else {
        return;
    };
    let Some(state) = guard.as_mut() else {
        return;
    };
    let Some(frame) = state.stack.last_mut() else {
        return;
    };
    let name = read_debug_text(name_ptr, name_len);
    if let Some(variable) = frame.variables.iter_mut().find(|v| v.name == name) {
        variable.type_tag = type_tag;
        variable.raw_value = raw_value;
        variable.display_value = None;
    } else {
        frame.variables.push(DebugVariable {
            name,
            type_tag,
            raw_value,
            display_value: None,
        });
    }
}

const DEBUG_TYPE_STRING: u32 = 7;
const DEBUG_TYPE_BYTES: u32 = 8;
const DEBUG_MAX_PAYLOAD_BYTES: usize = 256;

/// Copies a managed string/bytes preview only when the ARC registry confirms
/// that `ptr` is an allocation owned by this runtime. Unknown pointers are
/// deliberately not dereferenced: they may be foreign handles or static data
/// with no safely discoverable bound.
fn registered_debug_payload(ptr: *const u8) -> Option<(Vec<u8>, bool)> {
    // SAFETY: the runtime helper validates the pointer against its allocation
    // registry and copies only a bounded prefix while that registry is locked.
    if let Some((bytes, content_len)) =
        unsafe { super::registered_payload_preview(ptr, DEBUG_MAX_PAYLOAD_BYTES) }
    {
        return Some((bytes, content_len > DEBUG_MAX_PAYLOAD_BYTES));
    }

    let content_len = static_payloads()
        .lock()
        .ok()?
        .get(&(ptr as usize))
        .copied()?;
    let preview_len = content_len.min(DEBUG_MAX_PAYLOAD_BYTES);
    // SAFETY: the pointer was registered by generated code together with its
    // exact static length, and static data remains live for the process.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, preview_len).to_vec() };
    Some((bytes, content_len > DEBUG_MAX_PAYLOAD_BYTES))
}

fn render_debug_string(ptr: *const u8) -> String {
    if ptr.is_null() {
        return "null".to_string();
    }
    let Some((bytes, truncated)) = registered_debug_payload(ptr) else {
        return "<unavailable>".to_string();
    };
    let mut value = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        value.push('…');
    }
    value
}

fn render_debug_bytes(ptr: *const u8) -> String {
    if ptr.is_null() {
        return "null".to_string();
    }
    let Some((bytes, truncated)) = registered_debug_payload(ptr) else {
        return "<unavailable>".to_string();
    };
    let mut value = String::with_capacity(bytes.len() * 2 + 2 + usize::from(truncated));
    value.push_str("0x");
    for byte in &bytes {
        use std::fmt::Write as _;
        let _ = write!(value, "{byte:02x}");
    }
    if truncated {
        value.push('…');
    }
    value
}

/// Publish a safe content preview for a managed string or bytes value.
///
/// `kind` is `1` for strings and `2` for bytes. The raw pointer is consumed
/// only inside the runtime and is never serialized onto the debug wire.
///
/// # Safety
/// `name_ptr` must point to `name_len` readable bytes. `value_ptr` may be null
/// or a live managed payload pointer.
#[no_mangle]
pub unsafe extern "C" fn ori_debug_managed(
    name_ptr: *const u8,
    name_len: u32,
    kind: u32,
    value_ptr: *const u8,
) {
    if std::env::var_os("ORI_DEBUG_PORT").is_none() {
        return;
    }
    let Ok(mut guard) = STATE.lock() else {
        return;
    };
    let Some(state) = guard.as_mut() else {
        return;
    };
    let Some(frame) = state.stack.last_mut() else {
        return;
    };
    let name = read_debug_text(name_ptr, name_len);
    let (type_tag, display_value) = match kind {
        1 => (DEBUG_TYPE_STRING, render_debug_string(value_ptr)),
        2 => (DEBUG_TYPE_BYTES, render_debug_bytes(value_ptr)),
        _ => return,
    };
    if let Some(variable) = frame.variables.iter_mut().find(|v| v.name == name) {
        variable.type_tag = type_tag;
        variable.raw_value = value_ptr as usize as u64;
        variable.display_value = Some(display_value);
    } else {
        frame.variables.push(DebugVariable {
            name,
            type_tag,
            raw_value: value_ptr as usize as u64,
            display_value: Some(display_value),
        });
    }
}

/// Register a compiler-emitted static string payload for safe debugger reads.
/// Foreign hosts may use the same entry point when they can provide an exact
/// byte length; unregistered pointers remain unavailable to the debugger.
///
/// # Safety
/// `ptr` must point to `len` live bytes for the duration of the process.
#[no_mangle]
pub unsafe extern "C" fn ori_debug_register_static(ptr: *const u8, len: u32) {
    if ptr.is_null() {
        return;
    }
    if let Ok(mut payloads) = static_payloads().lock() {
        payloads.insert(ptr as usize, len as usize);
    }
}
