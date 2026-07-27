//! Test harness runtime functions (`ori.test.*`).
//!
//! Extracted from `lib.rs` as part of Etapa 8.3 monolith reduction.
//! These `#[no_mangle] extern "C"` entry points are called by Ori test
//! programs (`@test` functions compiled with the test harness injected).
//! They delegate to the ARC subsystem for leak checking and use
//! `super::cstr_str` for C string decoding.

use std::os::raw::c_uchar;

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert(condition: c_uchar, message: *const u8) {
    if condition == 0 {
        eprintln!("ori test assertion failed: {}", super::cstr_str(message));
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_eq(left: i64, right: i64) {
    if left != right {
        eprintln!("ori test assert_eq failed: {left} != {right}");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_ne(left: i64, right: i64) {
    if left == right {
        eprintln!("ori test assert_ne failed: both values are {left}");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_eq_float(left: f64, right: f64) {
    if left != right {
        eprintln!("ori test assert_eq failed: {left} != {right}");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_ne_float(left: f64, right: f64) {
    if left == right {
        eprintln!("ori test assert_ne failed: both values are {left}");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_eq_bool(left: c_uchar, right: c_uchar) {
    if (left != 0) != (right != 0) {
        eprintln!("ori test assert_eq failed: bool values differ");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_ne_bool(left: c_uchar, right: c_uchar) {
    if (left != 0) == (right != 0) {
        eprintln!("ori test assert_ne failed: bool values are equal");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_eq_string(left: *const u8, right: *const u8) {
    if super::cstr_str(left) != super::cstr_str(right) {
        eprintln!("ori test assert_eq failed: strings differ");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_ne_string(left: *const u8, right: *const u8) {
    if super::cstr_str(left) == super::cstr_str(right) {
        eprintln!("ori test assert_ne failed: strings are equal");
        std::process::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_skip(message: *const u8) {
    eprintln!("ori test skipped: {}", super::cstr_str(message));
    std::process::exit(77);
}

#[no_mangle]
pub unsafe extern "C" fn ori_test_fail(message: *const u8) {
    eprintln!("ori test failure: {}", super::cstr_str(message));
    std::process::abort();
}

/// Returns the number of live ARC-managed heap allocations.
/// Intended for test programs to assert no leaks at program exit.
/// Does NOT run the cycle collector — call `ori_test_collect_cycles` first
/// if orphan cycles may exist.
#[no_mangle]
pub extern "C" fn ori_test_live_allocations() -> i64 {
    super::ori_arc_live_allocations()
}

/// Runs the cycle collector and returns the number of cycles reclaimed.
/// Test programs typically call this before `ori_test_live_allocations`
/// so that reclaimable cycles do not appear as leaks.
#[no_mangle]
pub unsafe extern "C" fn ori_test_collect_cycles() -> i64 {
    super::ori_arc_collect_cycles()
}

/// Convenience for test programs: runs the cycle collector, then returns
/// the remaining live allocation count. If the `ORI_TEST_LEAK_CHECK=1`
/// environment variable is set and the count is non-zero, prints a
/// diagnostic to stderr and aborts with a non-zero exit code so the
/// test fails loudly. Returns the live count either way (the abort
/// happens after the return value is computed).
#[no_mangle]
pub unsafe extern "C" fn ori_test_assert_no_leaks(label: *const u8) -> i64 {
    let reclaimed = super::ori_arc_collect_cycles();
    let live = super::ori_arc_live_allocations();
    if live > 0 {
        let label_str = if label.is_null() {
            "(unnamed)".to_string()
        } else {
            super::cstr_str(label).to_string()
        };
        let env_set = std::env::var("ORI_TEST_LEAK_CHECK").ok().as_deref() == Some("1");
        if env_set {
            eprintln!(
                "ori leak check: {live} live allocations after `{label_str}` ({reclaimed} cycles reclaimed); aborting"
            );
            std::process::abort();
        } else {
            eprintln!(
                "ori leak check: {live} live allocations after `{label_str}` ({reclaimed} cycles reclaimed)"
            );
        }
    }
    live
}
