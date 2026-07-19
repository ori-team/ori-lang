mod common;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use common::{assert_check_output_is_well_formed, TestDir};
use ori_driver::pipeline::{run_check, run_doc_with_options, run_fmt, DocFormat, DocOptions};

#[test]
fn check_large_single_file_project_has_stable_performance_shape() {
    let dir = TestDir::new("perf_large_single_file");
    dir.write("main.orl", &large_single_file_source(180));

    let started = Instant::now();
    let out = run_check(&dir.path("main.orl")).unwrap();
    let elapsed = started.elapsed();

    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_check_output_is_well_formed(&out);
    assert_strict_budget("ORI_PERF_CHECK_SINGLE_FILE_BUDGET_MS", elapsed, 2_000);
}

#[test]
fn check_deep_import_graph_has_stable_performance_shape() {
    let dir = TestDir::new("perf_import_graph");
    let module_count = 72;
    for index in 0..module_count {
        let module = format!("app/mod{index}.orl");
        let source = if index + 1 == module_count {
            format!("module app.mod{index}\n\npublic value() -> int\n    return {index}\nend\n")
        } else {
            format!(
                "module app.mod{index}\n\nimport app.mod{} = next\n\npublic value() -> int\n    return next.value() + 1\nend\n",
                index + 1
            )
        };
        dir.write(&module, &source);
    }
    dir.write(
        "main.orl",
        "module app.main\n\nimport app.mod0 = entry\n\nmain()\n    const total: int = entry.value()\nend\n",
    );

    let started = Instant::now();
    let out = run_check(&dir.path("main.orl")).unwrap();
    let elapsed = started.elapsed();

    assert!(!out.has_errors, "{:?}", out.diagnostics);
    assert_check_output_is_well_formed(&out);
    assert_strict_budget("ORI_PERF_CHECK_IMPORT_GRAPH_BUDGET_MS", elapsed, 2_500);
}

#[test]
fn fmt_and_doc_large_public_surface_have_stable_performance_shape() {
    let dir = TestDir::new("perf_fmt_doc_surface");
    dir.write("main.orl", &documented_public_surface_source(96));

    let fmt_started = Instant::now();
    let fmt = run_fmt(&dir.path("main.orl")).unwrap();
    let fmt_elapsed = fmt_started.elapsed();
    assert!(!fmt.has_errors, "{:?}", fmt.diagnostics);
    assert!(fmt.formatted.contains("public item_95"));
    assert_strict_budget("ORI_PERF_FMT_SURFACE_BUDGET_MS", fmt_elapsed, 1_500);

    let doc_started = Instant::now();
    let doc = run_doc_with_options(
        &dir.path("main.orl"),
        DocOptions {
            format: DocFormat::Html,
        },
    )
    .unwrap();
    let doc_elapsed = doc_started.elapsed();
    assert!(!doc.has_errors, "{:?}", doc.diagnostics);
    assert!(doc.html.contains("<!DOCTYPE html>"));
    assert!(doc.html.contains("app.main.item_95"));
    assert_strict_budget("ORI_PERF_DOC_SURFACE_BUDGET_MS", doc_elapsed, 1_500);
}

#[test]
#[ignore = "heavy performance probe; run with `ORI_PERF_STRICT=1 cargo test -p ori-driver --test performance_guard -- --ignored`"]
fn strict_generated_code_runtime_probe() {
    let dir = TestDir::new("perf_runtime_probe");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io

fib(n: int) -> int
    if n <= 1
        return n
    end
    var a: int = 0
    var b: int = 1
    var i: int = 2
    while i <= n
        const next: int = a + b
        a = b
        b = next
        i = i + 1
    end
    return b
end

main()
    var total: int = 0
    var i: int = 0
    while i < 2_000
        total = total + fib(20)
        i = i + 1
    end
    io.print(string(total))
end
"#,
    );

    let main_path = dir.path("main.orl");
    let started = Instant::now();
    let output = common::run_ori(&["run", main_path.to_str().unwrap()]);
    let elapsed = started.elapsed();
    let stdout = common::normalize_stdout(output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "`ori run` failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "13530000");
    assert_strict_budget("ORI_PERF_RUNTIME_PROBE_BUDGET_MS", elapsed, 3_500);
}

#[test]
fn run_ffi_boundary_cost_stays_flat_with_many_live_allocations() {
    // Regression: LANG-PERF-3 — the ARC registry used linear scans, so every
    // retain/release cost O(live allocations) and FFI-heavy loops in large
    // programs collapsed (~1.5ms per extern call in the Studio shell). With
    // the keyed registry the loop below stays in the µs-per-iteration range
    // regardless of the 20k strings held alive during the loop.
    let dir = TestDir::new("perf_ffi_arc_registry");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.io = io
    ori.list = lists
end

extern c
    labs(x: int) -> int
end

main()
    const keep_count: int = 20_000
    const calls: int = 20_000
    var keep: list[string] = lists.with_capacity(keep_count)
    var k: int = 0
    while k < keep_count
        lists.push(keep, f"pad{k}")
        k = k + 1
    end
    var acc: int = 0
    var i: int = 0
    while i < calls
        const tmp: string = f"t{i}"
        acc = acc + labs(0 - lists.len([tmp]))
        i = i + 1
    end
    io.print(f"{lists.len(keep)}:{acc}")
end
"#,
    );

    let main_path = dir.path("main.orl");
    let started = Instant::now();
    let output = common::run_ori(&["run", main_path.to_str().unwrap()]);
    let elapsed = started.elapsed();
    let stdout = common::normalize_stdout(output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "`ori run` failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "20000:20000");
    assert_strict_budget("ORI_PERF_FFI_ARC_REGISTRY_BUDGET_MS", elapsed, 5_000);
}

#[test]
fn run_function_root_collect_stays_cheap_with_many_live_allocations() {
    // Regression: LANG-MEM-3 partial (LANG-PERF-3 residual) — function-root
    // cleanup used to call full `ori_arc_collect_cycles` (O(live heap)) on
    // every return. With 20k live strings that made even empty helper calls
    // dominate. Roots now call `ori_arc_maybe_collect_cycles` (threshold gate).
    let dir = TestDir::new("perf_function_root_collect");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.io = io
    ori.list = lists
end

tick(x: int) -> int
    return x + 1
end

main()
    const keep_count: int = 20_000
    const calls: int = 50_000
    var keep: list[string] = lists.with_capacity(keep_count)
    var k: int = 0
    while k < keep_count
        lists.push(keep, f"pad{k}")
        k = k + 1
    end
    var acc: int = 0
    var i: int = 0
    while i < calls
        acc = tick(acc)
        i = i + 1
    end
    io.print(f"{lists.len(keep)}:{acc}")
end
"#,
    );

    let main_path = dir.path("main.orl");
    let started = Instant::now();
    let output = common::run_ori(&["run", main_path.to_str().unwrap()]);
    let elapsed = started.elapsed();
    let stdout = common::normalize_stdout(output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "`ori run` failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "20000:50000");
    // Strict budget is generous for debug JIT; the pre-fix path was multi-second.
    assert_strict_budget("ORI_PERF_FUNCTION_ROOT_COLLECT_BUDGET_MS", elapsed, 5_000);
}

fn large_single_file_source(function_count: usize) -> String {
    let mut source = String::from(
        r#"module app.main

trait Named
    name(self) -> string
end

struct Item
    id: int
    label: string
end

apply Item
    use Named
        name(self) -> string
            return self.label
        end
    end
end

"#,
    );
    for index in 0..function_count {
        let _ = writeln!(
            source,
            "step_{index}(value: int) -> int\n    return value + {index}\nend\n"
        );
    }
    source.push_str("main()\n    const item: Item = Item {id: 1, label: \"ori\"}\n    var total: int = item.id\n");
    for index in 0..function_count {
        let _ = writeln!(source, "    total = step_{index}(total)");
    }
    source.push_str("    check total > 0, item.name()\nend\n");
    source
}

fn documented_public_surface_source(function_count: usize) -> String {
    let mut source = String::from("module app.main\n\n");
    for index in 0..function_count {
        let _ = writeln!(
            source,
            "--|\nReturns item {index}.\n\n@param value Input value.\n@returns The adjusted value.\n|--\npublic item_{index}(value: int) -> int\n    return value + {index}\nend\n"
        );
    }
    source.push_str("main()\nend\n");
    source
}

fn assert_strict_budget(env_name: &str, elapsed: Duration, default_budget_ms: u64) {
    if std::env::var_os("ORI_PERF_STRICT").is_none() {
        eprintln!(
            "{env_name}: elapsed={}ms; strict budget disabled",
            elapsed.as_millis()
        );
        return;
    }

    let budget_ms = std::env::var(env_name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_budget_ms);
    let budget = Duration::from_millis(budget_ms);
    assert!(
        elapsed <= budget,
        "{env_name}: elapsed={}ms exceeded budget={}ms",
        elapsed.as_millis(),
        budget_ms
    );
}

#[test]
fn run_cooperative_partial_pass_stays_flat_with_100k_live_allocations() {
    // Plan F3 acceptance (LANG-MEM-3): with a large live heap and no cycle
    // suspects, a cooperative pass must cost O(suspect subgraph) — i.e.
    // effectively O(1) — instead of the old full O(live allocations) trial
    // deletion. `ORI_COOPERATIVE_COLLECT_THRESHOLD=1` forces a pass at every
    // allocating safe point: ~20k passes over a 100k-string heap finish in
    // milliseconds with the suspect buffer, and took minutes with full scans.
    let dir = TestDir::new("perf_partial_pass_100k");
    dir.write(
        "main.orl",
        r#"module app.main

imports
    ori.io = io
    ori.list = lists
end

tick(i: int) -> int
    const tmp: string = f"t{i}"
    return len(tmp)
end

main()
    const keep_count: int = 100_000
    var keep: list[string] = lists.with_capacity(keep_count)
    var k: int = 0
    while k < keep_count
        lists.push(keep, f"pad{k}")
        k = k + 1
    end
    var acc: int = 0
    var i: int = 0
    while i < 20_000
        acc = acc + tick(i)
        i = i + 1
    end
    io.print(f"{lists.len(keep)}:{acc}")
end
"#,
    );

    let main_path = dir.path("main.orl");
    let started = Instant::now();
    let output = std::process::Command::new(common::ori_exe())
        .args(["run", main_path.to_str().unwrap()])
        .env("ORI_COOPERATIVE_COLLECT_THRESHOLD", "1")
        .output()
        .expect("failed to spawn `ori` subprocess");
    let elapsed = started.elapsed();
    let stdout = common::normalize_stdout(output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "`ori run` failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // tick lengths: t0..t9 -> 2 (x10), up to t19999 -> 6 (x10000):
    // 10*2 + 90*3 + 900*4 + 9000*5 + 10000*6 = 108890.
    assert_eq!(stdout.trim(), "100000:108890");
    assert_strict_budget("ORI_PERF_PARTIAL_PASS_100K_BUDGET_MS", elapsed, 10_000);
}
