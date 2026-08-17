//! BRASA-ORI-CALL-001: persistent hosted scalar-call baseline.

use ori_embed::{OriConfig, OriEngine, OriValue};
use std::time::Instant;

const CALLS: u64 = 1_000_000;

fn main() {
    let mut engine = OriEngine::new(OriConfig::default());
    engine
        .compile_source(
            "bench.orl",
            "module app.bench\n\npublic add(left: int, right: int) -> int\n    return left + right\nend\n",
        )
        .expect("compile benchmark module");
    let handle = engine
        .function("bench.orl", "add")
        .expect("resolve benchmark function");

    let started = Instant::now();
    let mut checksum = 0i64;
    for _ in 0..CALLS {
        checksum += match engine
            .call(&handle, &[OriValue::Int(2), OriValue::Int(3)])
            .expect("hosted scalar call")
            .expect("integer return")
        {
            OriValue::Int(value) => value,
            value => panic!("unexpected benchmark value: {value:?}"),
        };
    }
    let elapsed = started.elapsed();
    let nanos = elapsed.as_secs_f64() * 1_000_000_000.0;
    println!(
        "BRASA-ORI-CALL-001 calls={CALLS} total_ms={:.3} ns_per_call={:.3} checksum={checksum}",
        nanos / 1_000_000.0,
        nanos / CALLS as f64,
    );
}
