#[allow(dead_code)]
mod common;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use common::{exe_path, TestDir};
use ori_driver::pipeline::run_compile;
use serde_json::{json, Value};

static DEBUGGER_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn native_debugger_reports_stack_trace_and_scalar_variables() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_stack_and_variables");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nhelper(value: int) -> int\n    const next: int = value + 1\n    return next\nend\n\nmain()\n    const answer: int = helper(4)\nend\n",
    );
    let executable = exe_path(&dir, "debugger_app");

    std::env::set_var("ORI_DEBUG_INSTRUMENT", "1");
    std::env::set_var("ORI_DEBUG_SOURCE", &source_path);
    let compile_result = run_compile(&source_path, &executable);
    std::env::remove_var("ORI_DEBUG_INSTRUMENT");
    std::env::remove_var("ORI_DEBUG_SOURCE");
    compile_result.expect("debug-instrumented program should compile");

    // The portable source map carries the same local-variable catalogue on
    // Linux, macOS and Windows. Runtime snapshots below provide the live
    // values; this assertion protects the cross-platform metadata fallback.
    let debug_map: Value = serde_json::from_str(
        &std::fs::read_to_string(executable.with_extension("debug.json"))
            .expect("native debug map should be written"),
    )
    .expect("native debug map should be valid JSON");
    let functions = debug_map["functions"].as_array().expect("debug functions");
    let helper = functions
        .iter()
        .find(|function| function["name"].as_str() == Some("app.main.helper"))
        .expect("helper debug function");
    let variables = helper["variables"].as_array().expect("helper variables");
    assert!(variables.iter().any(|variable| {
        variable["name"].as_str() == Some("value") && variable["type"].as_str() == Some("int")
    }));
    assert!(variables.iter().any(|variable| {
        variable["name"].as_str() == Some("next") && variable["type"].as_str() == Some("int")
    }));

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind debugger listener");
    let port = listener.local_addr().expect("read debugger port").port();
    let expected_file = source_path.to_string_lossy().replace('\\', "/");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept debugger connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set debugger timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone debugger stream"));
        let mut stopped_with_stack = None;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read debugger event");
            if bytes == 0 {
                break;
            }
            let event: Value = serde_json::from_str(line.trim()).expect("valid debugger JSON");
            match event.get("type").and_then(Value::as_str) {
                Some("hello") => {
                    writeln!(
                        stream,
                        "{}",
                        json!({
                            "type": "setBreakpoints",
                            "file": expected_file,
                            "lines": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
                        })
                    )
                    .expect("send debugger breakpoints");
                }
                Some("stopped") => {
                    let has_nested_stack = event
                        .get("stackTrace")
                        .and_then(Value::as_array)
                        .is_some_and(|frames| frames.len() >= 2);
                    let has_parameter = event
                        .get("variables")
                        .and_then(Value::as_array)
                        .is_some_and(|variables| {
                            variables.iter().any(|variable| {
                                variable.get("name").and_then(Value::as_str) == Some("value")
                                    && variable.get("value").and_then(Value::as_i64) == Some(4)
                            })
                        });
                    if has_nested_stack && has_parameter {
                        stopped_with_stack = Some(event.clone());
                    }
                    writeln!(stream, "{}", json!({"type": "continue"}))
                        .expect("continue debugger program");
                }
                _ => {}
            }
        }
        stopped_with_stack
    });

    let output = Command::new(&executable)
        .env("ORI_DEBUG_PORT", port.to_string())
        .output()
        .expect("run debug-instrumented program");
    let stopped = server.join().expect("join debugger server");

    assert!(
        output.status.success(),
        "debug program failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stopped.is_some(),
        "debugger never reported nested stack and scalar parameter"
    );
}

#[test]
fn native_debugger_expands_struct_fields_and_list_metadata() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_aggregate_values");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nimports\n    ori.io = io\nend\n\nstruct Point\n    x: int\n    y: int\n    label: string\nend\n\nmain()\n    const point: Point = Point { x: 3, y: 7, label: \"or\" + \"igin\" }\n    const values: list[int] = [10, 20, 30]\n    const first: int = values[0]\n    io.println(f\"{point.x + point.y + first}\")\nend\n",
    );
    let executable = exe_path(&dir, "debugger_aggregate_app");

    std::env::set_var("ORI_DEBUG_INSTRUMENT", "1");
    std::env::set_var("ORI_DEBUG_SOURCE", &source_path);
    let compile_result = run_compile(&source_path, &executable);
    std::env::remove_var("ORI_DEBUG_INSTRUMENT");
    std::env::remove_var("ORI_DEBUG_SOURCE");
    compile_result.expect("debug-instrumented aggregate program should compile");

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind debugger listener");
    let port = listener.local_addr().expect("read debugger port").port();
    let expected_file = source_path.to_string_lossy().replace('\\', "/");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept debugger connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set debugger timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone debugger stream"));
        let mut aggregate_snapshot = None;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read debugger event");
            if bytes == 0 {
                break;
            }
            let event: Value = serde_json::from_str(line.trim()).expect("valid debugger JSON");
            match event.get("type").and_then(Value::as_str) {
                Some("hello") => {
                    writeln!(
                        stream,
                        "{}",
                        json!({
                            "type": "setBreakpoints",
                            "file": expected_file,
                            "lines": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
                        })
                    )
                    .expect("send debugger breakpoints");
                }
                Some("stopped") => {
                    let variables = event
                        .get("variables")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let has_field = |name: &str, value: i64| {
                        variables.iter().any(|variable| {
                            variable.get("name").and_then(Value::as_str) == Some(name)
                                && variable.get("value").and_then(Value::as_i64) == Some(value)
                        })
                    };
                    if has_field("point.x", 3)
                        && has_field("point.y", 7)
                        && has_field("values.length", 3)
                        && has_field("values[0]", 10)
                        && has_field("values[2]", 30)
                        && variables.iter().any(|variable| {
                            variable.get("name").and_then(Value::as_str) == Some("point.label")
                                && variable.get("type").and_then(Value::as_str) == Some("string")
                                && variable.get("value").and_then(Value::as_str) == Some("origin")
                        })
                        && variables.iter().any(|variable| {
                            variable.get("name").and_then(Value::as_str) == Some("values.capacity")
                                && variable
                                    .get("value")
                                    .and_then(Value::as_i64)
                                    .is_some_and(|capacity| capacity >= 3)
                        })
                    {
                        aggregate_snapshot = Some(event.clone());
                    }
                    writeln!(stream, "{}", json!({"type": "continue"}))
                        .expect("continue debugger program");
                }
                _ => {}
            }
        }
        aggregate_snapshot
    });

    let output = Command::new(&executable)
        .env("ORI_DEBUG_PORT", port.to_string())
        .output()
        .expect("run aggregate debug-instrumented program");
    let snapshot = server.join().expect("join debugger server");

    assert!(
        output.status.success(),
        "aggregate debug program failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        snapshot.is_some(),
        "debugger never reported struct fields and list metadata"
    );
}

#[test]
fn native_debugger_expands_structured_payloads_and_nested_lists() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_structured_payloads");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nimports\n    ori.io = io\n    ori.map = maps\n    ori.set = sets\nend\n\nenum Status\n    Ready\n    Done(code: int)\nend\n\nmain()\n    const maybe: optional[int] = some(7)\n    const outcome: result[void, string] = err(\"bad\")\n    const matrix: list[list[int]] = [[1, 2], [3, 4]]\n    const labels: map[string, int] = {\"a\": 1, \"b\": 2}\n    const seen: set[int] = set { 3, 5 }\n    const status: Status = Status.Done(code: 9)\n    io.println(string(matrix[0][1]))\n    match maybe\n    case some(value): io.println(string(value))\n    case none: io.println(\"none\")\n    end\n    match outcome\n    case ok(_): io.println(\"ok\")\n    case err(message): io.println(message)\n    end\n    io.println(string(maps.key_count_string_int(labels)))\n    io.println(string(sets.contains(seen, 3)))\n    match status\n    case Ready: io.println(\"ready\")\n    case Done(code): io.println(string(code))\n    end\nend\n",
    );
    let executable = exe_path(&dir, "debugger_structured_app");

    std::env::set_var("ORI_DEBUG_INSTRUMENT", "1");
    std::env::set_var("ORI_DEBUG_SOURCE", &source_path);
    let compile_result = run_compile(&source_path, &executable);
    std::env::remove_var("ORI_DEBUG_INSTRUMENT");
    std::env::remove_var("ORI_DEBUG_SOURCE");
    compile_result.expect("debug-instrumented structured program should compile");

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind debugger listener");
    let port = listener.local_addr().expect("read debugger port").port();
    let expected_file = source_path.to_string_lossy().replace('\\', "/");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept debugger connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set debugger timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone debugger stream"));
        let mut snapshot = None;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read debugger event");
            if bytes == 0 {
                break;
            }
            let event: Value = serde_json::from_str(line.trim()).expect("valid debugger JSON");
            match event.get("type").and_then(Value::as_str) {
                Some("hello") => {
                    writeln!(
                        stream,
                        "{}",
                        json!({
                            "type": "setBreakpoints",
                            "file": expected_file,
                            "lines": [21]
                        })
                    )
                    .expect("send debugger breakpoints");
                }
                Some("stopped") => {
                    let variables = event
                        .get("variables")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let has = |name: &str, expected: Value| {
                        variables.iter().any(|variable| {
                            variable.get("name").and_then(Value::as_str) == Some(name)
                                && variable.get("value") == Some(&expected)
                        })
                    };
                    let has_string = |name: &str, expected: &str| {
                        variables.iter().any(|variable| {
                            variable.get("name").and_then(Value::as_str) == Some(name)
                                && variable.get("value").and_then(Value::as_str) == Some(expected)
                        })
                    };
                    if has("maybe.has_value", json!(true))
                        && has("maybe.value", json!(7))
                        && has("outcome.is_ok", json!(false))
                        && has_string("outcome.error", "bad")
                        && has("matrix[0][1]", json!(2))
                        && has("matrix[1][0]", json!(3))
                        && has("labels.length", json!(2))
                        && has("labels.values[0]", json!(1))
                        && has("labels.values[1]", json!(2))
                        && has("seen.length", json!(2))
                        && has("status.tag", json!(1))
                        && has("status.Done.code", json!(9))
                    {
                        snapshot = Some(event.clone());
                    }
                    writeln!(stream, "{}", json!({"type": "continue"}))
                        .expect("continue debugger program");
                }
                _ => {}
            }
        }
        snapshot
    });

    let output = Command::new(&executable)
        .env("ORI_DEBUG_PORT", port.to_string())
        .output()
        .expect("run structured debug-instrumented program");
    let stopped = server.join().expect("join debugger server");
    assert!(
        output.status.success(),
        "structured debug program failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stopped.is_some(),
        "debugger never reported optional/result/enum/collection payloads"
    );
}

#[test]
fn native_debugger_reports_async_frame_after_await() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_async_frame");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nimports\n    ori.io = io\n    ori.task = task\nend\n\nasync compute(base: int) -> int\n    const before: int = base + 1\n    await task.sleep(5)\n    const after: int = before + 1\n    return after\nend\n\nmain()\n    const answer: int = task.block_on(compute(40))\n    io.print(string(answer))\nend\n",
    );
    let executable = exe_path(&dir, "debugger_async_app");

    std::env::set_var("ORI_DEBUG_INSTRUMENT", "1");
    std::env::set_var("ORI_DEBUG_SOURCE", &source_path);
    let compile_result = run_compile(&source_path, &executable);
    std::env::remove_var("ORI_DEBUG_INSTRUMENT");
    std::env::remove_var("ORI_DEBUG_SOURCE");
    compile_result.expect("debug-instrumented async program should compile");

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind debugger listener");
    let port = listener.local_addr().expect("read debugger port").port();
    let expected_file = source_path.to_string_lossy().replace('\\', "/");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept debugger connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set debugger timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone debugger stream"));
        let mut async_snapshot = None;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read debugger event");
            if bytes == 0 {
                break;
            }
            let event: Value = serde_json::from_str(line.trim()).expect("valid debugger JSON");
            match event.get("type").and_then(Value::as_str) {
                Some("hello") => {
                    writeln!(
                        stream,
                        "{}",
                        json!({
                            "type": "setBreakpoints",
                            "file": expected_file,
                            "lines": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
                        })
                    )
                    .expect("send debugger breakpoints");
                }
                Some("stopped") => {
                    let has_async_frame = event
                        .get("stackTrace")
                        .and_then(Value::as_array)
                        .is_some_and(|frames| {
                            frames.iter().any(|frame| {
                                frame
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .is_some_and(|name| name.ends_with("compute"))
                            })
                        });
                    let has_before = event
                        .get("variables")
                        .and_then(Value::as_array)
                        .is_some_and(|variables| {
                            variables.iter().any(|variable| {
                                variable.get("name").and_then(Value::as_str) == Some("before")
                                    && variable.get("value").and_then(Value::as_i64) == Some(41)
                            })
                        });
                    if has_async_frame && has_before {
                        async_snapshot = Some(event.clone());
                    }
                    writeln!(stream, "{}", json!({"type": "continue"}))
                        .expect("continue debugger program");
                }
                _ => {}
            }
        }
        async_snapshot
    });

    let output = Command::new(&executable)
        .env("ORI_DEBUG_PORT", port.to_string())
        .output()
        .expect("run async debug-instrumented program");
    let snapshot = server.join().expect("join debugger server");

    assert!(
        output.status.success(),
        "async debug program failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        snapshot.is_some(),
        "debugger never reported an async frame with the pre-await local"
    );
}

#[test]
fn native_debugger_reports_closure_captures() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_closure_capture");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nimports\n    ori.io = io\nend\n\nmain()\n    const offset: int = 3\n    const add: func(int) -> int = (x: int) -> int\n        const total: int = x + offset\n        return total\n    end\n    io.print(string(add(4)))\nend\n",
    );
    let executable = exe_path(&dir, "debugger_closure_app");

    std::env::set_var("ORI_DEBUG_INSTRUMENT", "1");
    std::env::set_var("ORI_DEBUG_SOURCE", &source_path);
    let compile_result = run_compile(&source_path, &executable);
    std::env::remove_var("ORI_DEBUG_INSTRUMENT");
    std::env::remove_var("ORI_DEBUG_SOURCE");
    compile_result.expect("debug-instrumented closure program should compile");

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind debugger listener");
    let port = listener.local_addr().expect("read debugger port").port();
    let expected_file = source_path.to_string_lossy().replace('\\', "/");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept debugger connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set debugger timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone debugger stream"));
        let mut closure_snapshot = None;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read debugger event");
            if bytes == 0 {
                break;
            }
            let event: Value = serde_json::from_str(line.trim()).expect("valid debugger JSON");
            match event.get("type").and_then(Value::as_str) {
                Some("hello") => {
                    writeln!(
                        stream,
                        "{}",
                        json!({
                            "type": "setBreakpoints",
                            "file": expected_file,
                            "lines": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
                        })
                    )
                    .expect("send debugger breakpoints");
                }
                Some("stopped") => {
                    let has_closure_frame = event
                        .get("stackTrace")
                        .and_then(Value::as_array)
                        .is_some_and(|frames| {
                            frames.iter().any(|frame| {
                                frame
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .is_some_and(|name| name.contains(".__closure_"))
                            })
                        });
                    let has_capture = event
                        .get("variables")
                        .and_then(Value::as_array)
                        .is_some_and(|variables| {
                            variables.iter().any(|variable| {
                                variable.get("name").and_then(Value::as_str) == Some("offset")
                                    && variable.get("value").and_then(Value::as_i64) == Some(3)
                            })
                        });
                    if has_closure_frame && has_capture {
                        closure_snapshot = Some(event.clone());
                    }
                    writeln!(stream, "{}", json!({"type": "continue"}))
                        .expect("continue debugger program");
                }
                _ => {}
            }
        }
        closure_snapshot
    });

    let output = Command::new(&executable)
        .env("ORI_DEBUG_PORT", port.to_string())
        .output()
        .expect("run closure debug-instrumented program");
    let snapshot = server.join().expect("join debugger server");

    assert!(
        output.status.success(),
        "closure debug program failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        snapshot.is_some(),
        "debugger never reported a closure frame with its captured value"
    );
}

#[test]
fn ori_debug_dap_bridges_breakpoint_and_continue() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_dap_bridge");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nhelper(value: int) -> int\n    return value + 1\nend\n\nmain()\n    const answer: int = helper(4)\nend\n",
    );
    let mut adapter = Command::new(env!("CARGO_BIN_EXE_ori"))
        .args(["debug", "--dap"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start DAP adapter");
    let mut input = adapter.stdin.take().expect("adapter stdin");
    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut output = BufReader::new(stdout);

    send_dap_request(
        &mut input,
        1,
        "initialize",
        json!({"clientID": "ori-test", "adapterID": "ori"}),
    );
    send_dap_request(
        &mut input,
        2,
        "launch",
        json!({"program": source_path.display().to_string()}),
    );
    send_dap_request(
        &mut input,
        3,
        "setBreakpoints",
        json!({
            "source": {"path": source_path.display().to_string()},
            "breakpoints": [{"line": 4}]
        }),
    );
    send_dap_request(&mut input, 4, "configurationDone", json!({}));

    let mut saw_stopped = false;
    let mut saw_stack = false;
    let mut saw_variables = false;
    let mut saw_evaluation = false;
    let mut saw_terminated = false;
    for _ in 0..60 {
        let message = read_dap_message(&mut output);
        let message_type = message.get("type").and_then(Value::as_str);
        if message_type == Some("event")
            && message.get("event").and_then(Value::as_str) == Some("stopped")
        {
            saw_stopped = true;
            send_dap_request(
                &mut input,
                5,
                "evaluate",
                json!({"expression": "value + 1", "frameId": 0}),
            );
            send_dap_request(&mut input, 6, "stackTrace", json!({"threadId": 1}));
            send_dap_request(&mut input, 7, "scopes", json!({"frameId": 0}));
            send_dap_request(&mut input, 8, "variables", json!({"variablesReference": 1}));
            send_dap_request(&mut input, 9, "continue", json!({"threadId": 1}));
        }
        if message_type == Some("response")
            && message.get("command").and_then(Value::as_str) == Some("evaluate")
        {
            saw_evaluation = message.get("success").and_then(Value::as_bool) == Some(true)
                && message
                    .get("body")
                    .and_then(|body| body.get("result"))
                    .and_then(Value::as_str)
                    == Some("5")
                && message
                    .get("body")
                    .and_then(|body| body.get("type"))
                    .and_then(Value::as_str)
                    == Some("int");
        }
        if message_type == Some("response")
            && message.get("command").and_then(Value::as_str) == Some("stackTrace")
        {
            saw_stack = message
                .get("body")
                .and_then(|body| body.get("stackFrames"))
                .and_then(Value::as_array)
                .is_some_and(|frames| frames.len() >= 2);
        }
        if message_type == Some("response")
            && message.get("command").and_then(Value::as_str) == Some("variables")
        {
            saw_variables = message
                .get("body")
                .and_then(|body| body.get("variables"))
                .and_then(Value::as_array)
                .is_some_and(|variables| {
                    variables.iter().any(|variable| {
                        variable.get("name").and_then(Value::as_str) == Some("value")
                            && variable.get("value").and_then(Value::as_str) == Some("4")
                    })
                });
        }
        if message_type == Some("event")
            && message.get("event").and_then(Value::as_str) == Some("terminated")
        {
            saw_terminated = true;
            send_dap_request(&mut input, 6, "disconnect", json!({}));
            break;
        }
    }
    drop(input);
    let status = adapter.wait().expect("wait for DAP adapter");
    assert!(status.success(), "DAP adapter exited with {status}");
    assert!(saw_stopped, "DAP adapter never forwarded stopped event");
    assert!(saw_stack, "DAP adapter never forwarded stack frames");
    assert!(
        saw_evaluation,
        "DAP adapter did not evaluate a visible scalar"
    );
    assert!(
        saw_variables,
        "DAP adapter never forwarded scalar variables"
    );
    assert!(
        saw_terminated,
        "DAP adapter never forwarded terminated event"
    );
}

#[test]
fn ori_debug_dap_expands_list_elements() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_dap_list_elements");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nimports\n    ori.io = io\nend\n\nmain()\n    const values: list[int] = [10, 20, 30]\n    io.print(string(values[0]))\nend\n",
    );
    let mut adapter = Command::new(env!("CARGO_BIN_EXE_ori"))
        .args(["debug", "--dap"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start DAP adapter");
    let mut input = adapter.stdin.take().expect("adapter stdin");
    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut output = BufReader::new(stdout);

    send_dap_request(
        &mut input,
        1,
        "initialize",
        json!({"clientID": "ori-list-test", "adapterID": "ori"}),
    );
    send_dap_request(
        &mut input,
        2,
        "launch",
        json!({"program": source_path.display().to_string()}),
    );
    send_dap_request(
        &mut input,
        3,
        "setBreakpoints",
        json!({
            "source": {"path": source_path.display().to_string()},
            "breakpoints": [{"line": 9}]
        }),
    );
    send_dap_request(&mut input, 4, "configurationDone", json!({}));

    let mut requested_children = false;
    let mut saw_parent = false;
    let mut saw_elements = false;
    let mut saw_terminated = false;
    for _ in 0..80 {
        let message = read_dap_message(&mut output);
        let message_type = message.get("type").and_then(Value::as_str);
        if message_type == Some("event")
            && message.get("event").and_then(Value::as_str) == Some("stopped")
        {
            send_dap_request(&mut input, 5, "scopes", json!({"frameId": 0}));
            send_dap_request(&mut input, 6, "variables", json!({"variablesReference": 1}));
            send_dap_request(&mut input, 7, "continue", json!({"threadId": 1}));
        }
        if message_type == Some("response")
            && message.get("command").and_then(Value::as_str) == Some("variables")
        {
            let variables = message
                .get("body")
                .and_then(|body| body.get("variables"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if !requested_children {
                if let Some(parent) = variables
                    .iter()
                    .find(|variable| variable.get("name").and_then(Value::as_str) == Some("values"))
                {
                    let reference = parent
                        .get("variablesReference")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    saw_parent = reference > 1;
                    if saw_parent {
                        requested_children = true;
                        send_dap_request(
                            &mut input,
                            8,
                            "variables",
                            json!({"variablesReference": reference}),
                        );
                    }
                }
            } else if variables.iter().any(|variable| {
                variable.get("name").and_then(Value::as_str) == Some("[0]")
                    && variable.get("value").and_then(Value::as_str) == Some("10")
            }) && variables.iter().any(|variable| {
                variable.get("name").and_then(Value::as_str) == Some("[2]")
                    && variable.get("value").and_then(Value::as_str) == Some("30")
            }) {
                saw_elements = true;
            }
        }
        if message_type == Some("event")
            && message.get("event").and_then(Value::as_str) == Some("terminated")
        {
            saw_terminated = true;
            send_dap_request(&mut input, 9, "disconnect", json!({}));
            break;
        }
    }
    drop(input);
    let status = adapter.wait().expect("wait for DAP adapter");
    assert!(status.success(), "DAP adapter exited with {status}");
    assert!(saw_parent, "DAP did not expose a reference for the list");
    assert!(saw_elements, "DAP did not expose indexed list elements");
    assert!(
        saw_terminated,
        "DAP adapter never forwarded terminated event"
    );
}

#[test]
fn ori_debug_dap_previews_managed_string_and_bytes_payloads() {
    let _guard = DEBUGGER_TEST_LOCK.lock().expect("debugger test lock");
    let dir = TestDir::new("debugger_dap_managed_payloads");
    let source_path = dir.path("main.orl");
    dir.write(
        "main.orl",
        "module app.main\n\nimports\n    ori.io = io\nend\n\nmain()\n    const literal: string = \"static debugger\"\n    const message: string = \"hello \" + \"debugger\"\n    const payload: bytes = b\"\\x01\\x02\\xFF\"\n    io.println(literal + message + payload.to_hex())\nend\n",
    );
    let mut adapter = Command::new(env!("CARGO_BIN_EXE_ori"))
        .args(["debug", "--dap"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start DAP adapter");
    let mut input = adapter.stdin.take().expect("adapter stdin");
    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut output = BufReader::new(stdout);

    send_dap_request(
        &mut input,
        1,
        "initialize",
        json!({"clientID": "ori-payload-test", "adapterID": "ori"}),
    );
    send_dap_request(
        &mut input,
        2,
        "launch",
        json!({"program": source_path.display().to_string()}),
    );
    send_dap_request(
        &mut input,
        3,
        "setBreakpoints",
        json!({
            "source": {"path": source_path.display().to_string()},
            "breakpoints": [{"line": 11}]
        }),
    );
    send_dap_request(&mut input, 4, "configurationDone", json!({}));

    let mut saw_payloads = false;
    let mut saw_terminated = false;
    for _ in 0..80 {
        let message = read_dap_message(&mut output);
        let message_type = message.get("type").and_then(Value::as_str);
        if message_type == Some("event")
            && message.get("event").and_then(Value::as_str) == Some("stopped")
        {
            send_dap_request(&mut input, 5, "scopes", json!({"frameId": 0}));
            send_dap_request(&mut input, 6, "variables", json!({"variablesReference": 1}));
            send_dap_request(&mut input, 7, "continue", json!({"threadId": 1}));
        }
        if message_type == Some("response")
            && message.get("command").and_then(Value::as_str) == Some("variables")
        {
            let variables = message
                .get("body")
                .and_then(|body| body.get("variables"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let has_message = variables.iter().any(|variable| {
                variable.get("name").and_then(Value::as_str) == Some("message")
                    && variable.get("type").and_then(Value::as_str) == Some("string")
                    && variable.get("value").and_then(Value::as_str) == Some("\"hello debugger\"")
            });
            let has_literal = variables.iter().any(|variable| {
                variable.get("name").and_then(Value::as_str) == Some("literal")
                    && variable.get("type").and_then(Value::as_str) == Some("string")
                    && variable.get("value").and_then(Value::as_str) == Some("\"static debugger\"")
            });
            let has_payload = variables.iter().any(|variable| {
                variable.get("name").and_then(Value::as_str) == Some("payload")
                    && variable.get("type").and_then(Value::as_str) == Some("bytes")
                    && variable.get("value").and_then(Value::as_str) == Some("\"0x0102ff\"")
            });
            saw_payloads |= has_literal && has_message && has_payload;
        }
        if message_type == Some("event")
            && message.get("event").and_then(Value::as_str) == Some("terminated")
        {
            saw_terminated = true;
            send_dap_request(&mut input, 8, "disconnect", json!({}));
            break;
        }
    }
    drop(input);
    let status = adapter.wait().expect("wait for DAP adapter");
    assert!(status.success(), "DAP adapter exited with {status}");
    assert!(saw_payloads, "DAP did not preview managed string and bytes");
    assert!(
        saw_terminated,
        "DAP adapter never forwarded terminated event"
    );
}

fn send_dap_request(writer: &mut impl Write, seq: u64, command: &str, arguments: Value) {
    let payload = serde_json::to_vec(&json!({
        "seq": seq,
        "type": "request",
        "command": command,
        "arguments": arguments,
    }))
    .expect("encode DAP request");
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len()).expect("write DAP header");
    writer.write_all(&payload).expect("write DAP payload");
    writer.flush().expect("flush DAP request");
}

fn read_dap_message(reader: &mut impl BufRead) -> Value {
    let mut header = String::new();
    let mut length = None;
    loop {
        header.clear();
        reader.read_line(&mut header).expect("read DAP header");
        assert!(!header.is_empty(), "DAP adapter closed before a message");
        if header == "\r\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = Some(value.trim().parse::<usize>().expect("DAP length"));
        }
    }
    let mut payload = vec![0; length.expect("DAP Content-Length")];
    reader.read_exact(&mut payload).expect("read DAP payload");
    serde_json::from_slice(&payload).expect("decode DAP message")
}
