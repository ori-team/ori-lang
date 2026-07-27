//! Debugger adapters for the cooperative native runtime agent.
//!
//! The runtime speaks a small newline-delimited JSON protocol over a local
//! TCP socket. This module exposes that protocol as both a terminal debugger
//! (`ori debug file.orl`) and a minimal Debug Adapter Protocol (DAP) server
//! (`ori debug --dap`). Keeping the bridge here means IDEs do not need to
//! understand compiler-specific environment variables or TCP messages.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::pipeline;

const DEBUG_PORT_ENV: &str = "ORI_DEBUG_PORT";
const DEBUG_INSTRUMENT_ENV: &str = "ORI_DEBUG_INSTRUMENT";
const DEBUG_SOURCE_ENV: &str = "ORI_DEBUG_SOURCE";
const DAP_LOCALS_REFERENCE: u64 = 1;
const DAP_FIRST_LIST_REFERENCE: u64 = 2;
const DAP_MAX_EVALUATE_LENGTH: usize = 512;

/// Options for the interactive terminal adapter.
#[derive(Debug, Clone)]
pub struct DebugOptions {
    pub file: PathBuf,
    pub breakpoints: Vec<u32>,
    pub native_raw: bool,
}

/// Runs an instrumented target with a small line-oriented terminal adapter.
pub fn run_terminal(options: &DebugOptions) -> Result<i32, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("cannot bind debugger listener: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("cannot read debugger listener address: {error}"))?
        .port();
    let executable = temporary_debug_executable(&options.file);
    compile_instrumented(&options.file, &executable, options.native_raw)?;

    let mut child = Command::new(&executable)
        .env(DEBUG_PORT_ENV, port.to_string())
        .spawn()
        .map_err(|error| format!("cannot start debug target: {error}"))?;
    let result = terminal_session(&listener, &mut child, &options.file, &options.breakpoints);
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&executable);
    result
}

fn terminal_session(
    listener: &TcpListener,
    child: &mut Child,
    source: &Path,
    breakpoints: &[u32],
) -> Result<i32, String> {
    let (mut stream, _) = listener
        .accept()
        .map_err(|error| format!("debug target did not connect: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|error| format!("cannot configure debugger socket: {error}"))?;
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|error| format!("cannot clone debugger socket: {error}"))?,
    );
    let mut input = String::new();
    let mut hello_seen = false;

    eprintln!("ori debug: connected; breakpoints={:?}", breakpoints);
    loop {
        input.clear();
        match reader.read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => {
                let event: Value = serde_json::from_str(input.trim())
                    .map_err(|error| format!("invalid debugger event: {error}"))?;
                match event.get("type").and_then(Value::as_str) {
                    Some("hello") => {
                        hello_seen = true;
                        send_runtime(
                            &mut stream,
                            json!({
                                "type": "setBreakpoints",
                                "file": source.display().to_string(),
                                "lines": breakpoints,
                            }),
                        )?;
                        eprintln!("ori debug: target ready");
                    }
                    Some("stopped") => {
                        print_stopped(&event);
                        match read_terminal_command()? {
                            TerminalCommand::Continue => {
                                send_runtime(&mut stream, json!({"type": "continue"}))?;
                            }
                            TerminalCommand::Step => {
                                send_runtime(&mut stream, json!({"type": "step"}))?;
                            }
                            TerminalCommand::Terminate => {
                                send_runtime(&mut stream, json!({"type": "terminate"}))?;
                                return Ok(1);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|wait_error| format!("cannot poll debug target: {wait_error}"))?
                {
                    return Ok(status.code().unwrap_or(1));
                }
            }
            Err(error) => return Err(format!("cannot read debugger event: {error}")),
        }
        if !hello_seen {
            thread::sleep(Duration::from_millis(10));
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for debug target: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

#[derive(Debug, Clone, Copy)]
enum TerminalCommand {
    Continue,
    Step,
    Terminate,
}

fn read_terminal_command() -> Result<TerminalCommand, String> {
    eprint!("[c]ontinue [s]tep [q]uit > ");
    io::stderr()
        .flush()
        .map_err(|error| format!("cannot flush debugger prompt: {error}"))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("cannot read debugger command: {error}"))?;
    match line.trim().to_ascii_lowercase().as_str() {
        "s" | "step" | "n" | "next" => Ok(TerminalCommand::Step),
        "q" | "quit" | "terminate" | "exit" => Ok(TerminalCommand::Terminate),
        _ => Ok(TerminalCommand::Continue),
    }
}

fn print_stopped(event: &Value) {
    let file = event
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>");
    let line = event.get("line").and_then(Value::as_u64).unwrap_or(0);
    let reason = event
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("pause");
    eprintln!("\npaused: {file}:{line} ({reason})");

    if let Some(frames) = event.get("stackTrace").and_then(Value::as_array) {
        eprintln!("stack:");
        for frame in frames {
            let name = frame
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<frame>");
            eprintln!("  {name}");
        }
    }
    if let Some(variables) = event.get("variables").and_then(Value::as_array) {
        eprintln!("variables:");
        for variable in variables {
            let name = variable.get("name").and_then(Value::as_str).unwrap_or("?");
            let value = variable
                .get("value")
                .map(Value::to_string)
                .unwrap_or_else(|| "?".to_string());
            eprintln!("  {name} = {value}");
        }
    }
}

/// Runs a minimal DAP server on stdin/stdout.
pub fn run_dap_stdio() -> Result<i32, String> {
    let requests = spawn_dap_reader();
    let mut output = io::BufWriter::new(io::stdout().lock());
    let mut sequence = 1_u64;
    let mut session: Option<DebugSession> = None;
    let mut breakpoints: HashMap<String, Vec<u32>> = HashMap::new();
    let mut latest_stop: Option<Value> = None;
    let mut list_references: HashMap<u64, String> = HashMap::new();
    let mut next_variable_reference = DAP_FIRST_LIST_REFERENCE;
    let mut target_exited = false;

    loop {
        if let Some(active) = session.as_mut() {
            loop {
                match active.events.try_recv() {
                    Ok(RuntimeMessage::Event(event)) => {
                        handle_runtime_event(
                            &mut output,
                            &mut sequence,
                            &event,
                            &mut latest_stop,
                            &mut list_references,
                            &mut next_variable_reference,
                        )?;
                    }
                    Ok(RuntimeMessage::Closed) => {
                        if !target_exited {
                            write_dap_event(&mut output, &mut sequence, "terminated", json!({}))?;
                            target_exited = true;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
            if !target_exited {
                if let Some(status) = active
                    .child
                    .try_wait()
                    .map_err(|error| format!("cannot poll debug target: {error}"))?
                {
                    write_dap_event(
                        &mut output,
                        &mut sequence,
                        "exited",
                        json!({"exitCode": status.code().unwrap_or(1)}),
                    )?;
                    write_dap_event(&mut output, &mut sequence, "terminated", json!({}))?;
                    target_exited = true;
                }
            }
        }

        match requests.recv_timeout(Duration::from_millis(20)) {
            Ok(request) => {
                let should_exit = handle_dap_request(
                    &request,
                    &mut output,
                    DapRequestState {
                        sequence: &mut sequence,
                        session: &mut session,
                        breakpoints: &mut breakpoints,
                        latest_stop: &mut latest_stop,
                        list_references: &mut list_references,
                        target_exited: &mut target_exited,
                    },
                )?;
                if should_exit {
                    return Ok(0);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
        }
    }
}

fn handle_runtime_event<W: Write>(
    output: &mut W,
    sequence: &mut u64,
    event: &Value,
    latest_stop: &mut Option<Value>,
    list_references: &mut HashMap<u64, String>,
    next_variable_reference: &mut u64,
) -> Result<(), String> {
    match event.get("type").and_then(Value::as_str) {
        Some("hello") => {}
        Some("stopped") => {
            *latest_stop = Some(event.clone());
            list_references.clear();
            *next_variable_reference = DAP_FIRST_LIST_REFERENCE;
            if let Some(event) = latest_stop.as_ref() {
                for prefix in debug_list_prefixes(event) {
                    list_references.insert(*next_variable_reference, prefix);
                    *next_variable_reference += 1;
                }
            }
            let reason = event
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("pause");
            write_dap_event(
                output,
                sequence,
                "stopped",
                json!({"reason": reason, "threadId": 1, "allThreadsStopped": true}),
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn debug_list_prefixes(event: &Value) -> Vec<String> {
    let names: HashSet<&str> = event
        .get("variables")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|variable| variable.get("name").and_then(Value::as_str))
        .collect();
    let mut prefixes = HashSet::new();

    for name in &names {
        let mut search_from = 0;
        while let Some(relative) = name[search_from..].find('[') {
            let bracket = search_from + relative;
            let prefix = &name[..bracket];
            let marker = format!("{prefix}[");
            if names.iter().any(|candidate| candidate.starts_with(&marker)) {
                prefixes.insert(prefix.to_string());
            }
            let Some(relative_end) = name[bracket..].find(']') else {
                break;
            };
            search_from = bracket + relative_end + 1;
            if search_from >= name.len() {
                break;
            }
        }
    }

    let mut prefixes = prefixes.into_iter().collect::<Vec<_>>();
    prefixes.sort();
    prefixes
}

struct DapRequestState<'a> {
    sequence: &'a mut u64,
    session: &'a mut Option<DebugSession>,
    breakpoints: &'a mut HashMap<String, Vec<u32>>,
    latest_stop: &'a mut Option<Value>,
    list_references: &'a mut HashMap<u64, String>,
    target_exited: &'a mut bool,
}

fn handle_dap_request<W: Write>(
    request: &Value,
    output: &mut W,
    state: DapRequestState<'_>,
) -> Result<bool, String> {
    let DapRequestState {
        sequence,
        session,
        breakpoints,
        latest_stop,
        list_references,
        target_exited,
    } = state;
    let command = request
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let request_seq = request.get("seq").and_then(Value::as_u64).unwrap_or(0);
    let arguments = request.get("arguments").unwrap_or(&Value::Null);

    match command {
        "initialize" => {
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({
                    "supportsConfigurationDoneRequest": true,
                    "supportsTerminateRequest": true,
                    "supportsStepOverRequest": true,
                    "supportsStepIn": false,
                    "supportsSteppingGranularity": false,
                    "supportsEvaluateForHovers": true,
                }),
            )?;
            write_dap_event(output, sequence, "initialized", json!({}))?;
        }
        "launch" => {
            let Some(file) = launch_source_path(arguments) else {
                write_dap_response(
                    output,
                    sequence,
                    request_seq,
                    command,
                    false,
                    Some("launch requires `program` or `file`"),
                    json!({}),
                )?;
                return Ok(false);
            };
            if session.is_some() {
                write_dap_response(
                    output,
                    sequence,
                    request_seq,
                    command,
                    false,
                    Some("a debug target is already running"),
                    json!({}),
                )?;
                return Ok(false);
            }
            let native_raw = arguments
                .get("nativeRaw")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            match DebugSession::start(&file, native_raw) {
                Ok(active) => {
                    *session = Some(active);
                    *target_exited = false;
                    if let Some(active) = session.as_ref() {
                        for (source, lines) in breakpoints.iter() {
                            active.send(json!({
                                "type": "setBreakpoints",
                                "file": source,
                                "lines": lines,
                            }))?;
                        }
                    }
                    let process_id = session
                        .as_ref()
                        .map(|active| active.child.id())
                        .unwrap_or_default();
                    write_dap_response(
                        output,
                        sequence,
                        request_seq,
                        command,
                        true,
                        None,
                        json!({}),
                    )?;
                    write_dap_event(
                        output,
                        sequence,
                        "process",
                        json!({"name": file.display().to_string(), "systemProcessId": process_id}),
                    )?;
                }
                Err(error) => {
                    write_dap_response(
                        output,
                        sequence,
                        request_seq,
                        command,
                        false,
                        Some(&error),
                        json!({}),
                    )?;
                }
            }
        }
        "setBreakpoints" => {
            let source_path = arguments
                .get("source")
                .and_then(|source| source.get("path"))
                .and_then(Value::as_str)
                .map(PathBuf::from);
            let lines = arguments
                .get("breakpoints")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.get("line").and_then(Value::as_u64))
                        .filter_map(|line| u32::try_from(line).ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Some(path) = source_path {
                let key = normalize_path(&path);
                breakpoints.insert(key.clone(), lines.clone());
                if let Some(active) = session.as_ref() {
                    active.send(json!({
                        "type": "setBreakpoints",
                        "file": path.display().to_string(),
                        "lines": lines,
                    }))?;
                }
                let verified = lines
                    .iter()
                    .map(|line| json!({"verified": true, "line": line}))
                    .collect::<Vec<_>>();
                write_dap_response(
                    output,
                    sequence,
                    request_seq,
                    command,
                    true,
                    None,
                    json!({"breakpoints": verified}),
                )?;
            } else {
                write_dap_response(
                    output,
                    sequence,
                    request_seq,
                    command,
                    false,
                    Some("setBreakpoints requires source.path"),
                    json!({}),
                )?;
            }
        }
        "configurationDone" => {
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({}),
            )?;
        }
        "threads" => {
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({"threads": [{"id": 1, "name": "main"}]}),
            )?;
        }
        "stackTrace" => {
            let frames = latest_stop
                .as_ref()
                .and_then(|event| event.get("stackTrace"))
                .and_then(Value::as_array)
                .map(|frames| {
                    frames
                        .iter()
                        .map(|frame| {
                            let id = frame.get("id").and_then(Value::as_u64).unwrap_or(0);
                            let name = frame
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("<frame>");
                            let source = latest_stop
                                .as_ref()
                                .and_then(|event| event.get("file"))
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            let line = latest_stop
                                .as_ref()
                                .and_then(|event| event.get("line"))
                                .and_then(Value::as_u64)
                                .unwrap_or(0);
                            json!({
                                "id": id,
                                "name": name,
                                "source": {"path": source},
                                "line": line,
                                "column": 1,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({"stackFrames": frames, "totalFrames": frames.len()}),
            )?;
        }
        "scopes" => {
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({
                    "scopes": [{
                        "name": "Locals",
                        "presentationHint": "locals",
                        "variablesReference": DAP_LOCALS_REFERENCE,
                        "expensive": false,
                    }]
                }),
            )?;
        }
        "variables" => {
            let requested_reference = arguments
                .get("variablesReference")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let prefix = if requested_reference == DAP_LOCALS_REFERENCE {
                None
            } else {
                list_references.get(&requested_reference).cloned()
            };
            let variables = latest_stop
                .as_ref()
                .and_then(|event| event.get("variables"))
                .and_then(Value::as_array)
                .map(|variables| {
                    variables
                        .iter()
                        .filter_map(|variable| {
                            let full_name = variable.get("name").and_then(Value::as_str)?;
                            let display_name = match prefix.as_deref() {
                                None if requested_reference == 0
                                    || requested_reference == DAP_LOCALS_REFERENCE =>
                                {
                                    if full_name.contains('[') {
                                        return None;
                                    }
                                    full_name.to_string()
                                }
                                Some(prefix) => {
                                    let suffix = full_name.strip_prefix(prefix)?;
                                    if !suffix.starts_with('[') {
                                        return None;
                                    }
                                    suffix.to_string()
                                }
                                None => return None,
                            };
                            let value = variable
                                .get("value")
                                .map(Value::to_string)
                                .unwrap_or_else(|| "<unavailable>".to_string());
                            let child_reference = list_references
                                .iter()
                                .find_map(|(reference, candidate)| {
                                    (candidate == full_name).then_some(*reference)
                                })
                                .unwrap_or(0);
                            Some(json!({
                                "name": display_name,
                                "type": variable.get("type").cloned().unwrap_or(Value::Null),
                                "value": value,
                                "variablesReference": child_reference,
                            }))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({"variables": variables}),
            )?;
        }
        "evaluate" => {
            let expression = arguments
                .get("expression")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|expression| !expression.is_empty());
            let Some(expression) = expression else {
                write_dap_response(
                    output,
                    sequence,
                    request_seq,
                    command,
                    false,
                    Some("evaluate requires a non-empty expression"),
                    json!({}),
                )?;
                return Ok(false);
            };
            match latest_stop
                .as_ref()
                .ok_or_else(|| "the target is not stopped".to_string())
                .and_then(|event| evaluate_debug_expression(event, expression))
            {
                Ok(value) => {
                    write_dap_response(
                        output,
                        sequence,
                        request_seq,
                        command,
                        true,
                        None,
                        json!({
                            "result": value.display(),
                            "type": value.type_name(),
                            "variablesReference": 0,
                        }),
                    )?;
                }
                Err(error) => {
                    write_dap_response(
                        output,
                        sequence,
                        request_seq,
                        command,
                        false,
                        Some(&error),
                        json!({}),
                    )?;
                }
            }
        }
        "continue" | "next" => {
            if let Some(active) = session.as_ref() {
                active.send(json!({
                    "type": if command == "next" { "step" } else { "continue" }
                }))?;
            }
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({"allThreadsContinued": true}),
            )?;
        }
        "terminate" | "disconnect" => {
            if let Some(active) = session.as_ref() {
                let _ = active.send(json!({"type": "terminate"}));
            }
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                true,
                None,
                json!({}),
            )?;
            return Ok(true);
        }
        _ => {
            write_dap_response(
                output,
                sequence,
                request_seq,
                command,
                false,
                Some("request is not implemented by the Ori adapter"),
                json!({}),
            )?;
        }
    }

    Ok(false)
}

#[derive(Debug, Clone, PartialEq)]
enum DebugEvalValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
}

impl DebugEvalValue {
    fn type_name(&self) -> &'static str {
        match self {
            Self::Integer(_) => "int",
            Self::Float(_) => "float",
            Self::Boolean(_) => "bool",
            Self::String(_) => "string",
        }
    }

    fn display(&self) -> String {
        match self {
            Self::Integer(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::String(value) => serde_json::to_string(value)
                .unwrap_or_else(|_| format!("\"{}\"", value.replace('"', "\\\""))),
        }
    }
}

fn evaluate_debug_expression(event: &Value, expression: &str) -> Result<DebugEvalValue, String> {
    if expression.len() > DAP_MAX_EVALUATE_LENGTH {
        return Err(format!(
            "expression exceeds the {}-byte limit",
            DAP_MAX_EVALUATE_LENGTH
        ));
    }
    let variables = event
        .get("variables")
        .and_then(Value::as_array)
        .ok_or_else(|| "the stopped event has no visible variables".to_string())?;
    let tokens = tokenize_debug_expression(expression)?;
    let mut parser = DebugExpressionParser {
        tokens,
        cursor: 0,
        variables,
    };
    let value = parser.parse_or()?;
    if !matches!(parser.peek(), DebugEvalToken::End) {
        return Err("unexpected tokens after expression".to_string());
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq)]
enum DebugEvalToken {
    Identifier(String),
    Number(String),
    String(String),
    Operator(String),
    LeftParen,
    RightParen,
    End,
}

fn tokenize_debug_expression(expression: &str) -> Result<Vec<DebugEvalToken>, String> {
    let bytes = expression.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if byte.is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if byte == b'(' {
            tokens.push(DebugEvalToken::LeftParen);
            cursor += 1;
            continue;
        }
        if byte == b')' {
            tokens.push(DebugEvalToken::RightParen);
            cursor += 1;
            continue;
        }
        if byte == b'"' {
            let start = cursor;
            cursor += 1;
            let mut escaped = false;
            while cursor < bytes.len() {
                let current = bytes[cursor];
                cursor += 1;
                if escaped {
                    escaped = false;
                } else if current == b'\\' {
                    escaped = true;
                } else if current == b'"' {
                    break;
                }
            }
            let literal = expression
                .get(start..cursor)
                .ok_or_else(|| "invalid string literal".to_string())?;
            if !literal.ends_with('"') {
                return Err("unterminated string literal".to_string());
            }
            let value = serde_json::from_str::<String>(literal)
                .map_err(|_| "invalid string literal".to_string())?;
            tokens.push(DebugEvalToken::String(value));
            continue;
        }
        if byte.is_ascii_digit()
            || (byte == b'.' && bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = cursor;
            let mut has_dot = false;
            while cursor < bytes.len() {
                let current = bytes[cursor];
                if current.is_ascii_digit() {
                    cursor += 1;
                } else if current == b'.' && !has_dot {
                    has_dot = true;
                    cursor += 1;
                } else {
                    break;
                }
            }
            tokens.push(DebugEvalToken::Number(
                expression[start..cursor].to_string(),
            ));
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = cursor;
            cursor += 1;
            while cursor < bytes.len() {
                let current = bytes[cursor];
                if current.is_ascii_alphanumeric() || matches!(current, b'_' | b'.' | b'[' | b']') {
                    cursor += 1;
                } else {
                    break;
                }
            }
            let identifier = expression[start..cursor].to_string();
            if matches!(identifier.as_str(), "and" | "or" | "not") {
                tokens.push(DebugEvalToken::Operator(identifier));
            } else {
                tokens.push(DebugEvalToken::Identifier(identifier));
            }
            continue;
        }
        let operator = if cursor + 1 < bytes.len()
            && matches!(
                (byte, bytes[cursor + 1]),
                (b'=', b'=') | (b'!', b'=') | (b'<', b'=') | (b'>', b'=')
            ) {
            let value = expression[cursor..cursor + 2].to_string();
            cursor += 2;
            value
        } else if matches!(byte, b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>') {
            cursor += 1;
            char::from(byte).to_string()
        } else {
            return Err(format!("unsupported character `{}`", char::from(byte)));
        };
        tokens.push(DebugEvalToken::Operator(operator));
    }
    tokens.push(DebugEvalToken::End);
    Ok(tokens)
}

struct DebugExpressionParser<'a> {
    tokens: Vec<DebugEvalToken>,
    cursor: usize,
    variables: &'a [Value],
}

impl<'a> DebugExpressionParser<'a> {
    fn peek(&self) -> &DebugEvalToken {
        self.tokens.get(self.cursor).unwrap_or(&DebugEvalToken::End)
    }

    fn take_operator(&mut self, expected: &str) -> bool {
        if matches!(self.peek(), DebugEvalToken::Operator(operator) if operator == expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<DebugEvalValue, String> {
        let mut value = self.parse_and()?;
        while self.take_operator("or") {
            let right = self.parse_and()?;
            value = DebugEvalValue::Boolean(as_bool(&value)? || as_bool(&right)?);
        }
        Ok(value)
    }

    fn parse_and(&mut self) -> Result<DebugEvalValue, String> {
        let mut value = self.parse_comparison()?;
        while self.take_operator("and") {
            let right = self.parse_comparison()?;
            value = DebugEvalValue::Boolean(as_bool(&value)? && as_bool(&right)?);
        }
        Ok(value)
    }

    fn parse_comparison(&mut self) -> Result<DebugEvalValue, String> {
        let mut value = self.parse_additive()?;
        loop {
            let operator = match self.peek() {
                DebugEvalToken::Operator(operator)
                    if matches!(operator.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=") =>
                {
                    operator.clone()
                }
                _ => break,
            };
            self.cursor += 1;
            let right = self.parse_additive()?;
            value = compare_values(&value, &right, &operator)?;
        }
        Ok(value)
    }

    fn parse_additive(&mut self) -> Result<DebugEvalValue, String> {
        let mut value = self.parse_multiplicative()?;
        loop {
            let operator = if self.take_operator("+") {
                "+"
            } else if self.take_operator("-") {
                "-"
            } else {
                break;
            };
            let right = self.parse_multiplicative()?;
            value = arithmetic_values(&value, &right, operator)?;
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<DebugEvalValue, String> {
        let mut value = self.parse_unary()?;
        loop {
            let operator = if self.take_operator("*") {
                "*"
            } else if self.take_operator("/") {
                "/"
            } else if self.take_operator("%") {
                "%"
            } else {
                break;
            };
            let right = self.parse_unary()?;
            value = arithmetic_values(&value, &right, operator)?;
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<DebugEvalValue, String> {
        if self.take_operator("-") {
            return match self.parse_unary()? {
                DebugEvalValue::Integer(value) => Ok(DebugEvalValue::Integer(-value)),
                DebugEvalValue::Float(value) => Ok(DebugEvalValue::Float(-value)),
                _ => Err("unary `-` requires a number".to_string()),
            };
        }
        if self.take_operator("not") {
            return Ok(DebugEvalValue::Boolean(!as_bool(&self.parse_unary()?)?));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<DebugEvalValue, String> {
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .unwrap_or(DebugEvalToken::End);
        match token {
            DebugEvalToken::Number(number) => {
                self.cursor += 1;
                if number.contains('.') {
                    number
                        .parse::<f64>()
                        .map(DebugEvalValue::Float)
                        .map_err(|_| format!("invalid number `{number}`"))
                } else {
                    number
                        .parse::<i64>()
                        .map(DebugEvalValue::Integer)
                        .map_err(|_| format!("invalid integer `{number}`"))
                }
            }
            DebugEvalToken::String(value) => {
                self.cursor += 1;
                Ok(DebugEvalValue::String(value))
            }
            DebugEvalToken::Identifier(name) => {
                self.cursor += 1;
                match name.as_str() {
                    "true" => Ok(DebugEvalValue::Boolean(true)),
                    "false" => Ok(DebugEvalValue::Boolean(false)),
                    _ => self.lookup_variable(&name),
                }
            }
            DebugEvalToken::LeftParen => {
                self.cursor += 1;
                let value = self.parse_or()?;
                if !matches!(self.peek(), DebugEvalToken::RightParen) {
                    return Err("missing `)`".to_string());
                }
                self.cursor += 1;
                Ok(value)
            }
            _ => Err("expected a literal, variable, or parenthesized expression".to_string()),
        }
    }

    fn lookup_variable(&self, name: &str) -> Result<DebugEvalValue, String> {
        let variable = self
            .variables
            .iter()
            .find(|variable| variable.get("name").and_then(Value::as_str) == Some(name))
            .ok_or_else(|| format!("variable `{name}` is not visible"))?;
        let type_name = variable
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let value = variable
            .get("value")
            .ok_or_else(|| format!("variable `{name}` has no value"))?;
        match type_name {
            "int" | "uint" => value
                .as_i64()
                .map(DebugEvalValue::Integer)
                .ok_or_else(|| format!("variable `{name}` is not an integer")),
            "float" | "float32" => value
                .as_f64()
                .map(DebugEvalValue::Float)
                .ok_or_else(|| format!("variable `{name}` is not a float")),
            "bool" => value
                .as_bool()
                .map(DebugEvalValue::Boolean)
                .ok_or_else(|| format!("variable `{name}` is not a boolean")),
            "string" => {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("variable `{name}` is not a string"))?;
                let unquoted = text
                    .strip_prefix('"')
                    .filter(|_| text.ends_with('"'))
                    .and_then(|_| serde_json::from_str::<String>(text).ok())
                    .unwrap_or_else(|| text.to_string());
                Ok(DebugEvalValue::String(unquoted))
            }
            _ => Err(format!(
                "variable `{name}` has unsupported debugger type `{type_name}`"
            )),
        }
    }
}

fn as_bool(value: &DebugEvalValue) -> Result<bool, String> {
    match value {
        DebugEvalValue::Boolean(value) => Ok(*value),
        _ => Err("boolean operator requires bool operands".to_string()),
    }
}

fn compare_values(
    left: &DebugEvalValue,
    right: &DebugEvalValue,
    operator: &str,
) -> Result<DebugEvalValue, String> {
    let result = match (left, right) {
        (DebugEvalValue::Integer(left), DebugEvalValue::Integer(right)) => {
            compare_integers(*left, *right, operator)
        }
        (DebugEvalValue::Integer(left), DebugEvalValue::Float(right)) => {
            compare_ordered(*left as f64, *right, operator)
        }
        (DebugEvalValue::Float(left), DebugEvalValue::Integer(right)) => {
            compare_ordered(*left, *right as f64, operator)
        }
        (DebugEvalValue::Float(left), DebugEvalValue::Float(right)) => {
            compare_ordered(*left, *right, operator)
        }
        (DebugEvalValue::Boolean(left), DebugEvalValue::Boolean(right))
            if matches!(operator, "==" | "!=") =>
        {
            if operator == "==" {
                *left == *right
            } else {
                *left != *right
            }
        }
        (DebugEvalValue::String(left), DebugEvalValue::String(right)) => match operator {
            "==" => left == right,
            "!=" => left != right,
            "<" => left < right,
            "<=" => left <= right,
            ">" => left > right,
            ">=" => left >= right,
            _ => false,
        },
        _ => {
            return Err(format!(
                "cannot compare {} and {}",
                left.type_name(),
                right.type_name()
            ))
        }
    };
    Ok(DebugEvalValue::Boolean(result))
}

fn compare_ordered(left: f64, right: f64, operator: &str) -> bool {
    match operator {
        "==" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        _ => false,
    }
}

fn compare_integers(left: i64, right: i64, operator: &str) -> bool {
    match operator {
        "==" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        _ => false,
    }
}

fn arithmetic_values(
    left: &DebugEvalValue,
    right: &DebugEvalValue,
    operator: &str,
) -> Result<DebugEvalValue, String> {
    if operator == "+" {
        if let (DebugEvalValue::String(left), DebugEvalValue::String(right)) = (left, right) {
            return Ok(DebugEvalValue::String(format!("{left}{right}")));
        }
    }
    let (left, right) = numeric_values(left, right)?;
    match (left, right, operator) {
        (NumericValue::Integer(left), NumericValue::Integer(right), "+") => {
            Ok(DebugEvalValue::Integer(left.saturating_add(right)))
        }
        (NumericValue::Integer(left), NumericValue::Integer(right), "-") => {
            Ok(DebugEvalValue::Integer(left.saturating_sub(right)))
        }
        (NumericValue::Integer(left), NumericValue::Integer(right), "*") => {
            Ok(DebugEvalValue::Integer(left.saturating_mul(right)))
        }
        (NumericValue::Integer(left), NumericValue::Integer(right), "%") => {
            if right == 0 {
                Err("integer remainder by zero".to_string())
            } else {
                Ok(DebugEvalValue::Integer(left % right))
            }
        }
        (left, right, "/") => {
            let denominator = right.as_f64();
            if denominator == 0.0 {
                return Err("division by zero".to_string());
            }
            Ok(DebugEvalValue::Float(left.as_f64() / denominator))
        }
        (left, right, operator) => {
            if operator == "%" && right.as_f64() == 0.0 {
                return Err("remainder by zero".to_string());
            }
            let result = match operator {
                "+" => left.as_f64() + right.as_f64(),
                "-" => left.as_f64() - right.as_f64(),
                "*" => left.as_f64() * right.as_f64(),
                "%" => left.as_f64() % right.as_f64(),
                _ => return Err(format!("unsupported numeric operator `{operator}`")),
            };
            Ok(DebugEvalValue::Float(result))
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum NumericValue {
    Integer(i64),
    Float(f64),
}

impl NumericValue {
    fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

fn numeric_values(
    left: &DebugEvalValue,
    right: &DebugEvalValue,
) -> Result<(NumericValue, NumericValue), String> {
    let left = match left {
        DebugEvalValue::Integer(value) => NumericValue::Integer(*value),
        DebugEvalValue::Float(value) => NumericValue::Float(*value),
        _ => return Err("arithmetic requires numeric operands".to_string()),
    };
    let right = match right {
        DebugEvalValue::Integer(value) => NumericValue::Integer(*value),
        DebugEvalValue::Float(value) => NumericValue::Float(*value),
        _ => return Err("arithmetic requires numeric operands".to_string()),
    };
    Ok((left, right))
}

struct DebugSession {
    child: Child,
    commands: Sender<Value>,
    events: Receiver<RuntimeMessage>,
    executable: PathBuf,
}

impl DebugSession {
    fn start(file: &Path, native_raw: bool) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("cannot bind debugger listener: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("cannot read debugger listener address: {error}"))?
            .port();
        let executable = temporary_debug_executable(file);
        compile_instrumented(file, &executable, native_raw)?;
        let child = Command::new(&executable)
            .env(DEBUG_PORT_ENV, port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("cannot start debug target: {error}"))?;
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        thread::spawn(move || runtime_bridge(listener, command_rx, event_tx));
        Ok(Self {
            child,
            commands,
            events,
            executable,
        })
    }

    fn send(&self, command: Value) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|error| format!("debug target is no longer connected: {error}"))
    }
}

impl Drop for DebugSession {
    fn drop(&mut self) {
        let _ = self.commands.send(json!({"type": "terminate"}));
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.executable);
    }
}

enum RuntimeMessage {
    Event(Value),
    Closed,
}

fn runtime_bridge(
    listener: TcpListener,
    commands: Receiver<Value>,
    events: Sender<RuntimeMessage>,
) {
    let Ok((mut stream, _)) = listener.accept() else {
        let _ = events.send(RuntimeMessage::Closed);
        return;
    };
    let _ = stream.set_nonblocking(true);
    let mut pending = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        while let Ok(command) = commands.try_recv() {
            if writeln!(stream, "{command}")
                .and_then(|_| stream.flush())
                .is_err()
            {
                let _ = events.send(RuntimeMessage::Closed);
                return;
            }
        }

        match stream.read(&mut buffer) {
            Ok(0) => {
                let _ = events.send(RuntimeMessage::Closed);
                return;
            }
            Ok(read) => {
                pending.extend_from_slice(&buffer[..read]);
                while let Some(position) = pending.iter().position(|byte| *byte == b'\n') {
                    let line = pending.drain(..=position).collect::<Vec<_>>();
                    if let Ok(event) = serde_json::from_slice::<Value>(&line) {
                        let _ = events.send(RuntimeMessage::Event(event));
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => {
                let _ = events.send(RuntimeMessage::Closed);
                return;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn compile_instrumented(file: &Path, executable: &Path, native_raw: bool) -> Result<(), String> {
    let previous_instrument = std::env::var_os(DEBUG_INSTRUMENT_ENV);
    let previous_source = std::env::var_os(DEBUG_SOURCE_ENV);
    std::env::set_var(DEBUG_INSTRUMENT_ENV, "1");
    std::env::set_var(DEBUG_SOURCE_ENV, file);
    let result = pipeline::run_compile_with_options(
        file,
        executable,
        pipeline::CompileOptions {
            native_raw,
            lib: false,
        },
    )
    .map_err(|error| error.to_string())
    .and_then(|out| {
        if out.has_errors {
            Err("debug target contains compilation errors".to_string())
        } else {
            Ok(())
        }
    });
    restore_env(DEBUG_INSTRUMENT_ENV, previous_instrument);
    restore_env(DEBUG_SOURCE_ENV, previous_source);
    result
}

fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

fn temporary_debug_executable(file: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    std::env::temp_dir().join(format!(
        "ori-debug-{stem}-{}-{millis}{suffix}",
        std::process::id()
    ))
}

fn normalize_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn launch_source_path(arguments: &Value) -> Option<PathBuf> {
    arguments
        .get("program")
        .or_else(|| arguments.get("file"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

fn spawn_dap_reader() -> Receiver<Value> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        while let Ok(Some(message)) = read_dap_message(&mut reader) {
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    receiver
}

fn read_dap_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut header = String::new();
    loop {
        header.clear();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "DAP message has no Content-Length header",
        ));
    };
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_dap_response<W: Write>(
    output: &mut W,
    sequence: &mut u64,
    request_seq: u64,
    command: &str,
    success: bool,
    message: Option<&str>,
    body: Value,
) -> Result<(), String> {
    let mut response = json!({
        "seq": *sequence,
        "type": "response",
        "request_seq": request_seq,
        "success": success,
        "command": command,
        "body": body,
    });
    if let Some(message) = message {
        response["message"] = json!(message);
    }
    *sequence += 1;
    write_dap_message(output, &response)
}

fn write_dap_event<W: Write>(
    output: &mut W,
    sequence: &mut u64,
    event: &str,
    body: Value,
) -> Result<(), String> {
    let message = json!({"seq": *sequence, "type": "event", "event": event, "body": body});
    *sequence += 1;
    write_dap_message(output, &message)
}

fn write_dap_message<W: Write>(output: &mut W, message: &Value) -> Result<(), String> {
    let payload =
        serde_json::to_vec(message).map_err(|error| format!("encode DAP message: {error}"))?;
    write!(output, "Content-Length: {}\r\n\r\n", payload.len())
        .and_then(|_| output.write_all(&payload))
        .and_then(|_| output.flush())
        .map_err(|error| format!("write DAP message: {error}"))
}

fn send_runtime(stream: &mut TcpStream, message: Value) -> Result<(), String> {
    writeln!(stream, "{message}")
        .and_then(|_| stream.flush())
        .map_err(|error| format!("send debugger command: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{evaluate_debug_expression, normalize_path, read_dap_message};
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn reads_content_length_framed_dap_message() {
        let payload = br#"{"seq":1,"type":"request","command":"initialize"}"#;
        let input = format!(
            "Content-Length: {}\r\n\r\n{}",
            payload.len(),
            String::from_utf8(payload.to_vec()).expect("payload is utf8")
        );
        let message = read_dap_message(&mut Cursor::new(input)).expect("read message");
        assert_eq!(message.expect("message")["command"], "initialize");
    }

    #[test]
    fn normalizes_windows_separators_for_breakpoint_keys() {
        assert!(!normalize_path(std::path::Path::new("foo\\bar.orl")).contains('\\'));
    }

    #[test]
    fn evaluates_visible_scalars_without_running_target_code() {
        let event = json!({
            "variables": [
                {"name": "answer", "type": "int", "value": 41},
                {"name": "enabled", "type": "bool", "value": true},
                {"name": "label", "type": "string", "value": "\"ori\""}
            ]
        });
        assert_eq!(
            evaluate_debug_expression(&event, "answer + 1")
                .expect("integer expression")
                .display(),
            "42"
        );
        assert_eq!(
            evaluate_debug_expression(&event, "enabled and answer > 40")
                .expect("boolean expression")
                .display(),
            "true"
        );
        assert_eq!(
            evaluate_debug_expression(&event, "label + \"!\"")
                .expect("string expression")
                .display(),
            "\"ori!\""
        );
    }

    #[test]
    fn rejects_unsupported_or_dangerous_debug_expressions() {
        let event = json!({"variables": []});
        assert!(evaluate_debug_expression(&event, "process()").is_err());
        assert!(evaluate_debug_expression(&event, "1 / 0").is_err());
        assert!(evaluate_debug_expression(&event, &"x".repeat(513)).is_err());
    }
}
