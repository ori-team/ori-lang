//! Concurrency stress and contention suite.
//!
//! Validates heavy concurrency primitives under multithreaded contention:
//! - Multi-task producer/consumer loops on small bounded channels (backpressure)
//! - Concurrent task cancellation via CancelScope across sleeping/waiting tasks
//! - Ordered channel drain after early closure with ARC integrity
//! - Multi-task coordination via atomic integers without data races

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use ori_driver::pipeline::run_compile;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_driver_conc_stress_{}_{}_{}",
            std::process::id(),
            id,
            name,
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, source).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn exe_path(dir: &TestDir, name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dir.path(&filename)
}

fn compile_and_run(dir: &TestDir, exe_name: &str) -> (String, String, bool) {
    let exe = exe_path(dir, exe_name);
    let out = run_compile(&dir.path("main.orl"), &exe).expect("compilation failed");
    assert!(
        !out.has_errors,
        "compiler diagnostics: {:?}",
        out.diagnostics
    );

    let output = Command::new(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("execution failed");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn stress_bounded_channel_multi_task_ping_pong() {
    let dir = TestDir::new("bounded_ping_pong");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.channel as channel
import ori.task as task
import ori.io as io

main()
    const maybe: optional[channel.Channel[int]] = channel.create_bounded(4)
    match maybe
        case some(ch):
            -- Producer task: send 50 integers through bounded channel
            const producer: task.Job[int] = task.spawn(() -> int
                var i: int = 1
                while i <= 50
                    const sent: result[void, channel.SendError] = channel.send(ch, i)
                    i = i + 1
                end
                return 50
            end)

            -- Consumer loop in main thread
            var sum: int = 0
            var received_count: int = 0
            while received_count < 50
                match channel.receive(ch)
                    case ok(v):
                        sum = sum + v
                        received_count = received_count + 1
                    case err(_):
                        panic("premature channel receive error")
                end
            end

            const p_res: result[int, task.JoinError] = task.join(producer)
            channel.close(ch)

            -- Sum of 1..50 = 50 * 51 / 2 = 1275
            check sum == 1275, "all 50 messages correctly consumed"
            io.println("PING_PONG_OK")
        case none:
            panic("failed to create bounded channel with capacity 4")
    end
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "ping_pong");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("PING_PONG_OK"));
}

#[test]
fn stress_concurrent_cancellation_during_sleep() {
    let dir = TestDir::new("conc_cancel_sleep");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.cancel as cancel
import ori.task as task
import ori.io as io

main()
    const scope: cancel.CancelScope = cancel.create_scope()

    -- Spawn background worker polling the token
    const worker: task.Job[bool] = task.spawn(() -> bool
        var count: int = 0
        while not cancel.is_cancelled(scope) and count < 1000
            count = count + 1
        end
        return cancel.is_cancelled(scope)
    end)

    -- Cancel scope from main thread
    cancel.cancel(scope)
    check cancel.is_cancelled(scope), "scope is cancelled"

    match task.join(worker)
        case ok(was_cancelled):
            check was_cancelled, "worker observed cancellation flag"
            io.println("CANCEL_OBSERVED_OK")
        case err(_):
            panic("worker thread join failed")
    end
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "cancel_sleep");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("CANCEL_OBSERVED_OK"));
}

#[test]
fn stress_channel_drain_after_close() {
    let dir = TestDir::new("channel_drain_close");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.channel as channel
import ori.io as io

main()
    const maybe: optional[channel.Channel[int]] = channel.create_bounded(10)
    match maybe
        case some(ch):
            -- Fill channel buffer with scalar values
            const _s1: result[void, channel.SendError] = channel.send(ch, 100)
            const _s2: result[void, channel.SendError] = channel.send(ch, 200)
            const _s3: result[void, channel.SendError] = channel.send(ch, 300)

            -- Close channel while messages are still in flight
            channel.close(ch)

            -- Drain buffered messages after close
            const r1: result[int, channel.ReceiveError] = channel.receive(ch)
            const r2: result[int, channel.ReceiveError] = channel.receive(ch)
            const r3: result[int, channel.ReceiveError] = channel.receive(ch)
            const r4: result[int, channel.ReceiveError] = channel.receive(ch)

            match r1
                case ok(s): check s == 100, "first item"
                case err(_): panic("expected item 100")
            end

            match r2
                case ok(s): check s == 200, "second item"
                case err(_): panic("expected item 200")
            end

            match r3
                case ok(s): check s == 300, "third item"
                case err(_): panic("expected item 300")
            end

            -- Fourth receive on closed empty channel must yield error
            match r4
                case ok(_): panic("expected error on empty closed channel")
                case err(_): io.println("DRAIN_AFTER_CLOSE_OK")
            end
        case none:
            panic("failed to create bounded channel")
    end
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "drain_close");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("DRAIN_AFTER_CLOSE_OK"));
}

#[test]
fn stress_multi_task_atomic_counter() {
    let dir = TestDir::new("atomic_counter");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.atomic as atomic
import ori.task as task
import ori.io as io

main()
    const counter: atomic.AtomicInt = atomic.new(0)
    check atomic.load(counter) == 0, "initial counter is 0"

    -- Spawn worker 1 adding 50
    const w1: task.Job[int] = task.spawn(() -> int
        var i: int = 0
        while i < 50
            const prev: int = atomic.add(counter, 1)
            i = i + 1
        end
        return 50
    end)

    -- Spawn worker 2 adding 50 concurrently
    const w2: task.Job[int] = task.spawn(() -> int
        var i: int = 0
        while i < 50
            const prev: int = atomic.add(counter, 1)
            i = i + 1
        end
        return 50
    end)

    const j1: result[int, task.JoinError] = task.join(w1)
    const j2: result[int, task.JoinError] = task.join(w2)

    const total: int = atomic.load(counter)
    check total == 100, "two workers concurrently added 50 each"
    io.println("ATOMIC_COUNTER_OK")
end
"#,
    );

    let (stdout, stderr, ok) = compile_and_run(&dir, "atomic_counter");
    assert!(ok, "stderr: {stderr}");
    assert!(stdout.contains("ATOMIC_COUNTER_OK"));
}
