use super::*;
use ori_types::stdlib::stdlib_runtime_functions;
use std::collections::{BinaryHeap, HashSet};
use std::ffi::CStr;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Mutex;

static TEST_DTOR_CALLS: AtomicUsize = AtomicUsize::new(0);
static TEST_EXECUTOR_CALLBACKS: AtomicUsize = AtomicUsize::new(0);
static TEST_ARC_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn exported_runtime_version_queries_are_stable_and_nul_terminated() {
    let runtime_version = unsafe { CStr::from_ptr(ori_rt_version()) };
    let abi_version = unsafe { CStr::from_ptr(ori_rt_abi_version()) };
    let target = unsafe { CStr::from_ptr(ori_rt_target()) };

    assert_eq!(runtime_version.to_str().unwrap(), ORI_RUNTIME_VERSION);
    assert_eq!(abi_version.to_str().unwrap(), ORI_ABI_VERSION);
    assert_eq!(target.to_str().unwrap(), ORI_RUNTIME_TARGET);
    assert!(!runtime_version.to_bytes().is_empty());
    assert!(!abi_version.to_bytes().is_empty());
    assert!(!target.to_bytes().is_empty());
}

#[test]
fn borrowed_handle_null_probe_only_checks_pointer_sentinel() {
    assert!(unsafe { ori_handle_null() }.is_null());
    assert_eq!(unsafe { ori_handle_is_null(std::ptr::null_mut()) }, 1);
    let non_null = std::ptr::NonNull::<u8>::dangling().as_ptr();
    assert_eq!(unsafe { ori_handle_is_null(non_null) }, 0);
}

#[test]
fn opaque_handle_validation_accepts_only_live_runtime_allocations() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let handle = ori_alloc(8, None);
        ori_handle_validate(handle);
        ori_arc_release(handle);
    }
    assert_eq!(ori_arc_live_allocations(), 0);
}

#[test]
fn opaque_handle_size_validation_accepts_the_registered_payload_layout() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let handle = ori_alloc(24, None);
        ori_handle_validate_size(handle, 24);
        ori_arc_release(handle);
    }
    assert_eq!(ori_arc_live_allocations(), 0);
}

#[test]
fn opaque_handle_type_validation_accepts_the_compiler_tagged_payload() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let handle = ori_alloc_typed(24, None, 7);
        ori_handle_validate_size_type(handle, 24, 7);
        ori_arc_release(handle);
    }
    assert_eq!(ori_arc_live_allocations(), 0);
}

#[test]
fn managed_allocation_size_check_rejects_header_overflow() {
    assert_eq!(allocation_total(usize::MAX), None);
    assert_eq!(
        allocation_total(0),
        Some(std::mem::size_of::<OriHeapHeader>())
    );
}

#[test]
fn nul_terminated_payload_size_is_checked_before_the_sentinel() {
    assert_eq!(nul_terminated_payload_size(0), 1);
    assert_eq!(nul_terminated_payload_size(usize::MAX - 1), usize::MAX);
}

#[test]
fn arc_registry_contention_counter_is_observable() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();
    let before = arc_lock_contention_count();

    let (tx_start, rx_start) = std::sync::mpsc::channel();
    let (tx_release, rx_release) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let guard = lock_arc_state();
        tx_start.send(()).unwrap();
        rx_release.recv().unwrap();
        drop(guard);
    });

    rx_start.recv().unwrap();
    let waiter = std::thread::spawn(|| {
        let _guard = lock_arc_state();
    });
    // Wait briefly so waiter attempts try_lock and records WouldBlock
    std::thread::sleep(std::time::Duration::from_millis(20));
    tx_release.send(()).unwrap();
    holder.join().unwrap();
    waiter.join().unwrap();

    assert!(
        arc_lock_contention_count() > before,
        "contention counter must observe actual contended acquisitions"
    );
}

#[test]
fn timer_heap_pops_earliest_deadline_with_stable_ties() {
    let now = Instant::now();
    let mut timers = BinaryHeap::new();
    timers.push(TimerEntry {
        due: now + Duration::from_millis(20),
        future: 20,
        sequence: 0,
    });
    timers.push(TimerEntry {
        due: now + Duration::from_millis(10),
        future: 10,
        sequence: 1,
    });
    timers.push(TimerEntry {
        due: now + Duration::from_millis(10),
        future: 11,
        sequence: 2,
    });

    assert_eq!(timers.pop().map(|entry| entry.future), Some(10));
    assert_eq!(timers.pop().map(|entry| entry.future), Some(11));
    assert_eq!(timers.pop().map(|entry| entry.future), Some(20));
}

#[test]
fn timer_heap_compaction_returns_terminal_future_owners() {
    let now = Instant::now();
    let mut timers = BinaryHeap::new();
    timers.push(TimerEntry {
        due: now + Duration::from_millis(20),
        future: 20,
        sequence: 0,
    });
    timers.push(TimerEntry {
        due: now + Duration::from_millis(10),
        future: 10,
        sequence: 1,
    });

    let stale = compact_timer_heap(&mut timers, |future| future != 20);

    assert_eq!(stale, vec![20]);
    assert_eq!(timers.len(), 1);
    assert_eq!(timers.pop().map(|entry| entry.future), Some(10));
}

#[test]
fn cancellation_token_association_storage_compacts_after_burst() {
    let mut associations = Vec::with_capacity(128);
    associations.push(std::ptr::null_mut());
    let peak = associations.capacity();

    compact_cancel_token_associations(&mut associations);

    assert!(
        associations.capacity() < peak,
        "long-lived cancellation tokens must not retain burst capacity"
    );
    assert_eq!(associations.len(), 1);
}

#[test]
fn checked_calloc_zeroes_storage_after_validating_size() {
    unsafe {
        let ptr = checked_calloc(3, std::mem::size_of::<i64>()) as *mut i64;
        assert!(!ptr.is_null());
        let values = std::slice::from_raw_parts(ptr, 3);
        assert_eq!(values, &[0, 0, 0]);
        libc::free(ptr.cast());
    }
}

#[test]
fn fallible_runtime_buffers_reject_invalid_sizes_without_unwinding() {
    assert!(try_zeroed_bytes(-1).is_err());
    assert!(try_zeroed_bytes(i64::MAX).is_err());
    assert_eq!(repeat_string_or_abort("ab", 3), "ababab");
}

#[test]
fn public_allocation_boundary_matrix_and_failure_contracts() {
    // 1. Header and allocation total bounds
    assert!(allocation_total(0).is_some());
    assert!(allocation_total(1024).is_some());
    assert_eq!(allocation_total(usize::MAX), None);
    assert_eq!(
        allocation_total(usize::MAX - std::mem::size_of::<OriHeapHeader>() + 1),
        None
    );

    // 2. NUL-terminated payload bounds
    assert_eq!(nul_terminated_payload_size(0), 1);
    assert_eq!(nul_terminated_payload_size(100), 101);

    // 3. Fallible zeroed byte buffer allocations
    assert!(try_zeroed_bytes(-1).is_err());
    assert!(try_zeroed_bytes(-100).is_err());
    assert!(try_zeroed_bytes(i64::MAX).is_err());
    assert_eq!(try_zeroed_bytes(0), Ok(Vec::new()));
    assert_eq!(try_zeroed_bytes(16), Ok(vec![0; 16]));

    // 4. Repeat string contracts
    assert_eq!(repeat_string_or_abort("", 100), "");
    assert_eq!(repeat_string_or_abort("hello", 0), "");
    assert_eq!(repeat_string_or_abort("hello", -5), "");
    assert_eq!(repeat_string_or_abort("ab", 3), "ababab");

    // 5. Pad string contracts
    unsafe {
        let p1 = pad_string("test", 4, " ", true);
        assert_eq!(cstr_str(p1), "test");
        ori_arc_release(p1);

        let p2 = pad_string("test", 2, " ", true);
        assert_eq!(cstr_str(p2), "test");
        ori_arc_release(p2);

        let p3 = pad_string("test", 6, "", true);
        assert_eq!(cstr_str(p3), "  test");
        ori_arc_release(p3);

        let p4 = pad_string("abc", 6, " ", true);
        assert_eq!(cstr_str(p4), "   abc");
        ori_arc_release(p4);

        let p5 = pad_string("abc", 6, " ", false);
        assert_eq!(cstr_str(p5), "abc   ");
        ori_arc_release(p5);
    }

    // 6. Capacity and growth calculations
    assert_eq!(capacity_bytes(-10, 8), 0);
    assert_eq!(capacity_bytes(0, 8), 0);
    assert_eq!(capacity_bytes(10, 8), 80);
    assert!(grown_capacity(8, 20, 8) >= 20);

    // 7. Checked external slice lengths and bounds
    assert_eq!(checked_external_slice_len(0, "test"), 0);
    assert_eq!(checked_external_slice_len(42, "test"), 42);
    assert_eq!(checked_slice_bounds(10, 2, 7, "test"), (2, 7));
}

#[test]
fn pointer_provenance_and_typed_utf8_ffi_contracts() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    ori_host_clear_error();

    // 1. Foreign / stack pointer provenance: retain and release safely no-op
    let stack_val = 123456_i64;
    let foreign_ptr = &stack_val as *const i64 as *mut u8;
    assert!(!unsafe { retain_registered_payload(foreign_ptr) });
    unsafe {
        ori_arc_release(foreign_ptr);
    }
    assert!(!lock_arc_state()
        .allocations
        .contains_key(&(foreign_ptr as usize)));

    // 2. Managed typed aggregate allocation provenance
    let payload = unsafe { ori_alloc_typed(24, None, 101) };
    assert!(!payload.is_null());
    assert!(lock_arc_state()
        .allocations
        .contains_key(&(payload as usize)));
    unsafe {
        ori_handle_validate(payload);
        ori_handle_validate_size(payload, 24);
        ori_handle_validate_size_type(payload, 24, 101);
        ori_arc_release(payload);
    }
    assert!(!lock_arc_state()
        .allocations
        .contains_key(&(payload as usize)));

    // 3. Typed UTF-8 validation across FFI ingress
    assert_eq!(unsafe { cstr_str_result(std::ptr::null()) }, Ok(""));

    ori_host_clear_error();
    let invalid_nul_term = [0xC3_u8, 0x28_u8, 0]; // Invalid UTF-8 sequence
    assert!(unsafe { cstr_str_result(invalid_nul_term.as_ptr()).is_err() });
    assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_INVALID_UTF8);

    ori_host_clear_error();
    let invalid_bytes = [0xFF_u8];
    assert!(unsafe { bounded_cstr_str(invalid_bytes.as_ptr(), 1).is_err() });
    assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_INVALID_UTF8);
}

#[test]
fn string_concat_rejects_invalid_utf8_in_both_abi_shapes() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    let invalid = [0xff_u8, 0];
    let valid = b"ok\0";

    unsafe {
        ori_host_clear_error();
        let result = ori_string_concat(invalid.as_ptr(), valid.as_ptr());
        assert_eq!(cstr_str(result), "");
        assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_INVALID_UTF8);
        ori_arc_release(result);

        ori_host_clear_error();
        let result = ori_string_concat_parts(invalid.as_ptr(), 1, valid.as_ptr(), 2);
        assert_eq!(cstr_str(result), "");
        assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_INVALID_UTF8);
        ori_arc_release(result);
    }
}

#[test]
fn string_case_fold_uses_full_non_turkic_unicode_mapping() {
    let input = b"Stra\xC3\x9Fe\0";
    unsafe {
        let result = ori_string_case_fold(input.as_ptr());
        assert_eq!(cstr_str(result), "strasse");
        ori_arc_release(result);
    }

    // Validate against generated Unicode conformance vectors
    let vectors_json = include_str!("../../../../tests/unicode_case_fold_conformance.json");
    let parsed: serde_json::Value = serde_json::from_str(vectors_json).unwrap();
    let vectors = parsed["vectors"].as_array().unwrap();
    for v in vectors {
        let input_str = v["input"].as_str().unwrap();
        let expected_str = v["expected"].as_str().unwrap();
        let mut c_input = input_str.as_bytes().to_vec();
        c_input.push(0);
        unsafe {
            let result = ori_string_case_fold(c_input.as_ptr());
            assert_eq!(
                cstr_str(result),
                expected_str,
                "conformance vector failed for category '{}': input '{}'",
                v["category"].as_str().unwrap_or(""),
                input_str
            );
            ori_arc_release(result);
        }
    }
}

#[test]
fn worker_spawn_failure_completes_future_with_terminal_failure() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();
    ori_host_clear_error();
    TEST_FORCE_THREAD_SPAWN_FAILURE.store(true, AtomicOrdering::SeqCst);

    let future = unsafe { spawn_io_result_future(std::ptr::null_mut::<u8>) };

    TEST_FORCE_THREAD_SPAWN_FAILURE.store(false, AtomicOrdering::SeqCst);
    unsafe {
        assert_eq!(ori_future_poll(future), 2);
        assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_THREAD_SPAWN);
        let message = CStr::from_ptr(ori_host_error_message());
        assert!(message.to_string_lossy().contains("ori-io-worker"));
        ori_arc_release(future as *mut u8);
    }
}

#[test]
fn operating_system_spawn_failure_is_reported_without_unwinding() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    ori_host_clear_error();
    TEST_THREAD_STACK_SIZE.store(u64::MAX, AtomicOrdering::SeqCst);
    let result = try_spawn_named("ori-runtime-os-failure", || {});
    TEST_THREAD_STACK_SIZE.store(0, AtomicOrdering::SeqCst);

    assert!(result.is_err(), "an impossible stack reservation must fail");
    unsafe {
        assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_THREAD_SPAWN);
        let message = CStr::from_ptr(ori_host_error_message());
        assert!(message.to_string_lossy().contains("ori-runtime-os-failure"));
    }
}

#[test]
fn task_spawn_failure_releases_closure_and_returns_failed_join() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();
    TEST_FORCE_THREAD_SPAWN_FAILURE.store(true, AtomicOrdering::SeqCst);

    let (job, result) = unsafe {
        let closure = test_closure_object();
        let job = ori_task_spawn(closure);
        let result = ori_task_join(job);
        (job, result)
    };

    TEST_FORCE_THREAD_SPAWN_FAILURE.store(false, AtomicOrdering::SeqCst);
    unsafe {
        assert_eq!(result_flag(result), 0);
        free_result(result);
        ori_arc_release(job as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn lazy_worker_start_retries_after_transient_spawn_failure() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    let state = std::sync::OnceLock::new();

    TEST_FORCE_THREAD_SPAWN_FAILURE.store(true, AtomicOrdering::SeqCst);
    let first = ensure_named_thread(&state, "ori-runtime-test-retry", || {});
    assert!(first.is_err());

    TEST_FORCE_THREAD_SPAWN_FAILURE.store(false, AtomicOrdering::SeqCst);
    let second = ensure_named_thread(&state, "ori-runtime-test-retry", || {});
    assert!(second.is_ok());

    // Once a worker starts successfully, future callers do not create
    // duplicate threads and therefore do not execute this closure.
    let third = ensure_named_thread(&state, "ori-runtime-test-retry", || {
        panic!("a started worker must not be spawned twice")
    });
    assert!(third.is_ok());
}

#[test]
fn cancelling_associated_future_releases_every_association_reference() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let token = ori_task_create_token();
        let future = alloc_pending_future();
        assert!(!token.is_null());
        assert!(!future.is_null());

        ori_task_associate(token, future as *mut u8);
        ori_task_cancel(token);
        assert_eq!(ori_future_poll(future), 3);

        ori_arc_release(future as *mut u8);
        ori_arc_release(token);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn association_after_cancellation_cancels_without_leaking() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let token = ori_task_create_token();
        let future = alloc_pending_future();
        ori_task_cancel(token);
        ori_task_associate(token, future as *mut u8);
        assert_eq!(ori_future_poll(future), 3);

        ori_arc_release(future as *mut u8);
        ori_arc_release(token);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn io_read_reports_an_unrepresentable_buffer_size() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let stream = ori_io_stdin();
        let result = ori_io_read(stream, i64::MAX);
        assert_eq!(result_flag(result), 0);
        free_result(result);
        ori_arc_release(stream);
    }
}

#[test]
fn hosted_error_slot_is_thread_local_and_clearable() {
    unsafe { ori_host_report_error(8, c"list index out of bounds".as_ptr().cast()) };
    assert_eq!(ori_host_error_code(), 8);
    let message = unsafe { CStr::from_ptr(ori_host_error_message()) };
    assert_eq!(message.to_str().unwrap(), "list index out of bounds");

    ori_host_clear_error();
    assert_eq!(ori_host_error_code(), 0);
    assert!(ori_host_error_message().is_null());
}

#[test]
fn invalid_utf8_cstr_sets_recoverable_host_error_instead_of_becoming_empty() {
    ori_host_clear_error();
    let invalid = [0xff_u8, 0];

    unsafe {
        assert_eq!(ori_string_len(invalid.as_ptr()), 0);
    }

    assert_eq!(ori_host_error_code(), ORI_HOST_ERROR_INVALID_UTF8);
    let message = unsafe { CStr::from_ptr(ori_host_error_message()) };
    assert_eq!(
        message.to_bytes(),
        b"invalid UTF-8 string passed across the Ori host ABI"
    );

    ori_host_clear_error();
    assert_eq!(ori_host_error_code(), 0);
}

unsafe extern "C" fn test_destructor(_ptr: *mut u8) {
    TEST_DTOR_CALLS.fetch_add(1, AtomicOrdering::SeqCst);
}

unsafe extern "C" fn test_graph_node_equals(left: *mut u8, right: *mut u8) -> c_uchar {
    if left.is_null() || right.is_null() {
        return 0;
    }
    // SAFETY: the test installs this callback only for the two live eight-byte
    // allocations created below, each containing one initialized `i64`.
    u8::from(*(left as *const i64) == *(right as *const i64)) as c_uchar
}

unsafe extern "C" fn test_structural_key_hash(value: i64) -> i64 {
    if value == 0 {
        return 0;
    }
    *(value as *const i64)
}

unsafe extern "C" fn test_structural_key_equals(left: *mut u8, right: *mut u8) -> c_uchar {
    if left.is_null() || right.is_null() {
        return 0;
    }
    u8::from(*(left as *const i64) == *(right as *const i64)) as c_uchar
}

unsafe fn header_for(ptr: *mut u8) -> *mut OriHeapHeader {
    ptr.sub(std::mem::size_of::<OriHeapHeader>()) as *mut OriHeapHeader
}

unsafe fn result_flag(ptr: *mut u8) -> u8 {
    *ptr
}

unsafe fn result_i64_payload(ptr: *mut u8) -> i64 {
    std::ptr::read_unaligned(ptr.add(std::mem::size_of::<*mut u8>()) as *const i64)
}

unsafe fn result_ptr_payload(ptr: *mut u8) -> *mut u8 {
    *(ptr.add(std::mem::size_of::<*mut u8>()) as *mut *mut u8)
}

// LANG-MEM-9: result wrappers are ARC-managed (`ori_alloc` + ownership
// edge to the payload). A single release cascades; the manual
// payload-release + raw free of the malloc era would double-release the
// payload and hand `libc::free` an interior (post-header) pointer.
unsafe fn release_result_payload_and_free(ptr: *mut u8) {
    ori_arc_release(ptr);
}

unsafe fn free_result(ptr: *mut u8) {
    ori_arc_release(ptr);
}

unsafe extern "C" fn test_task_entry(_env: *mut u8) -> i64 {
    41
}

unsafe extern "C" fn test_counting_async_entry(_env: *mut u8) -> i64 {
    TEST_EXECUTOR_CALLBACKS.fetch_add(1, AtomicOrdering::SeqCst);
    123
}

unsafe extern "C" fn test_failed_await_entry(_env: *mut u8) -> i64 {
    let failed = alloc_pending_future();
    ori_future_fail(failed);
    let _ = ori_task_block_on(failed);
    ori_arc_release(failed as *mut u8);
    99
}

unsafe extern "C" fn test_cancelled_await_entry(_env: *mut u8) -> i64 {
    let cancelled = alloc_pending_future();
    ori_future_cancel(cancelled);
    let _ = ori_task_block_on(cancelled);
    ori_arc_release(cancelled as *mut u8);
    99
}

unsafe extern "C" fn test_executor_entry(_env: *mut u8) -> i64 {
    TEST_EXECUTOR_CALLBACKS.fetch_add(1, AtomicOrdering::SeqCst);
    0
}

unsafe extern "C-unwind" fn test_panicking_task_entry(_env: *mut u8) -> i64 {
    panic!("injected task callback panic")
}

unsafe fn test_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_task_entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = 0;
    closure
}

unsafe fn test_panicking_task_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_panicking_task_entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = 0;
    closure
}

unsafe fn test_counting_async_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_counting_async_entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = 0;
    closure
}

unsafe fn test_failed_await_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_failed_await_entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = 0;
    closure
}

unsafe fn test_cancelled_await_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_cancelled_await_entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = 0;
    closure
}

unsafe fn test_executor_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_executor_entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = 0;
    closure
}

#[test]
fn string_and_bytes_use_nul_terminated_payload_layout() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let text = cstring_from_str("\u{00e1}\u{00e9}\u{1f642}");
        assert_eq!(ori_len(text), 3);
        assert_eq!(ori_string_len(text), 3);
        assert_eq!(cstr_bytes(text), "\u{00e1}\u{00e9}\u{1f642}".as_bytes());
        assert_eq!(*text.add("\u{00e1}\u{00e9}\u{1f642}".len()), 0);
        assert!(header_for_registered(text).is_some());
        ori_arc_release(text);

        let bytes = cstring_from_bytes(vec![1, 2, 3]);
        assert_eq!(ori_bytes_len(bytes), 3);
        assert_eq!(bytes_payload(bytes), &[1, 2, 3]);
        assert_eq!(*bytes.add(3), 0);
        assert!(header_for_registered(bytes).is_some());
        ori_arc_release(bytes);

        let bytes_with_nul = cstring_from_bytes(vec![1, 0, 3]);
        assert_eq!(ori_bytes_len(bytes_with_nul), 3);
        assert_eq!(bytes_payload(bytes_with_nul), &[1, 0, 3]);
        assert_eq!(*bytes_with_nul.add(3), 0);
        assert!(header_for_registered(bytes_with_nul).is_some());
        ori_arc_release(bytes_with_nul);

        let literal = std::ffi::CString::new("ping").unwrap();
        let copied = ori_string_to_bytes(literal.as_ptr().cast());
        assert_eq!(bytes_payload(copied), b"ping");
        assert!(header_for_registered(copied).is_some());
        ori_arc_release(copied);
    }
}

#[test]
fn bytes_equality_compares_length_and_embedded_nul_content() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let same_left = cstring_from_bytes(vec![0x41, 0x00, 0x42]);
        let same_right = cstring_from_bytes(vec![0x41, 0x00, 0x42]);
        let different = cstring_from_bytes(vec![0x41, 0x00, 0x43]);
        let different_length = cstring_from_bytes(vec![0x41, 0x00]);

        assert_eq!(ori_bytes_eq(same_left, same_right), 1);
        assert_eq!(ori_bytes_eq(same_left, different), 0);
        assert_eq!(ori_bytes_eq(same_left, different_length), 0);

        ori_arc_release(same_left);
        ori_arc_release(same_right);
        ori_arc_release(different);
        ori_arc_release(different_length);
    }
}

#[test]
fn debug_payload_preview_is_bounded_and_registry_backed() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    unsafe {
        let text = cstring_from_str(&"x".repeat(300));
        let (preview, content_len) = registered_payload_preview(text, 16).expect("managed text");
        assert_eq!(content_len, 300);
        assert_eq!(preview.len(), 16);
        assert_eq!(preview, vec![b'x'; 16]);
        ori_arc_release(text);

        let foreign = c"foreign".as_ptr().cast::<u8>();
        assert!(registered_payload_preview(foreign, 16).is_none());
    }
}

#[test]
fn string_len_and_slice_use_unicode_scalar_indices() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let text = cstring_from_str("\u{00e1}\u{00e9}");
        assert_eq!(ori_string_len(text), 2);

        let slice = ori_string_slice(text, 0, 1);
        assert_eq!(cstr_str(slice), "\u{00e1}");

        ori_arc_release(slice);
        ori_arc_release(text);
    }
}

#[test]
fn string_index_of_uses_unicode_scalar_indices() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let text = cstring_from_str("a\u{00e9}");
        let accent = cstring_from_str("\u{00e9}");
        assert_eq!(ori_string_index_of(text, accent), 1);

        let emoji_text = cstring_from_str("\u{1f642}x");
        let x = cstring_from_str("x");
        assert_eq!(ori_string_index_of(emoji_text, x), 1);

        ori_arc_release(text);
        ori_arc_release(accent);
        ori_arc_release(emoji_text);
        ori_arc_release(x);
    }
}

#[test]
fn bytes_fs_and_string_conversions_handle_nul_contract() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let base = std::env::temp_dir().join(format!("ori-bytes-nul-{}", std::process::id()));
        let input_path = base.with_extension("in");
        let output_path = base.with_extension("out");
        std::fs::write(&input_path, b"A\0B").unwrap();

        let input = cstring_from_str(input_path.to_str().unwrap());
        let read_result = ori_files_read_bytes(input);
        assert_eq!(result_flag(read_result), 1);
        let read_bytes = result_ptr_payload(read_result);
        assert_eq!(bytes_payload(read_bytes), b"A\0B");

        let output = cstring_from_str(output_path.to_str().unwrap());
        let write_result = ori_files_write_bytes(output, read_bytes);
        assert_eq!(result_flag(write_result), 1);
        assert_eq!(std::fs::read(&output_path).unwrap(), b"A\0B");

        let decode_result = ori_bytes_decode_utf8(read_bytes);
        assert_eq!(result_flag(decode_result), 0);
        assert!(cstr_str(result_ptr_payload(decode_result)).contains("NUL"));

        let from_bytes_result = ori_string_from_bytes(read_bytes);
        assert_eq!(result_flag(from_bytes_result), 0);
        assert!(cstr_str(result_ptr_payload(from_bytes_result)).contains("NUL"));

        release_result_payload_and_free(from_bytes_result);
        release_result_payload_and_free(decode_result);
        release_result_payload_and_free(write_result);
        release_result_payload_and_free(read_result);
        ori_arc_release(input);
        ori_arc_release(output);
        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn json_stringify_pretty_formats_valid_json() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let text = cstring_from_str("{\"name\":\"ori\",\"ok\":true}");
        let parse_res = ori_json_parse(text);
        assert_eq!(*parse_res, 1); // is_ok

        let value = *(parse_res.add(std::mem::size_of::<*mut u8>()) as *mut *mut u8);
        let compact = ori_json_stringify(value);
        let pretty = ori_json_stringify_pretty(value);

        assert_eq!(cstr_str(compact), "{\"name\":\"ori\",\"ok\":true}");
        assert_eq!(
            cstr_str(pretty),
            "{\n  \"name\": \"ori\",\n  \"ok\": true\n}"
        );

        ori_arc_release(text);
        ori_arc_release(compact);
        ori_arc_release(pretty);
        release_result_payload_and_free(parse_res);
    }
}

#[test]
fn list_map_and_set_layouts_keep_native_backend_offsets() {
    let ptr_size = std::mem::size_of::<*mut i64>();

    assert_eq!(std::mem::offset_of!(OriList, data), 0);
    assert_eq!(std::mem::offset_of!(OriList, len), ptr_size);
    assert_eq!(std::mem::offset_of!(OriList, cap), ptr_size + 8);
    assert_eq!(std::mem::offset_of!(OriList, version), ptr_size + 16);

    assert_eq!(std::mem::offset_of!(OriSet, items), 0);
    assert_eq!(std::mem::offset_of!(OriSet, len), ptr_size);
    assert_eq!(std::mem::offset_of!(OriSet, cap), ptr_size + 8);
    assert_eq!(std::mem::offset_of!(OriSet, version), ptr_size + 16);
    assert_eq!(std::mem::offset_of!(OriSet, ht), ptr_size + 24);
    assert_eq!(
        std::mem::offset_of!(OriSet, ht_cap),
        ptr_size + 24 + ptr_size
    );

    assert_eq!(std::mem::offset_of!(OriMap, keys), 0);
    assert_eq!(std::mem::offset_of!(OriMap, values), ptr_size);
    assert_eq!(std::mem::offset_of!(OriMap, len), ptr_size * 2);
    assert_eq!(std::mem::offset_of!(OriMap, cap), ptr_size * 2 + 8);
    assert_eq!(std::mem::offset_of!(OriMap, version), ptr_size * 2 + 16);
    assert_eq!(std::mem::offset_of!(OriMap, ht), ptr_size * 2 + 24);
    assert_eq!(
        std::mem::offset_of!(OriMap, ht_cap),
        ptr_size * 2 + 24 + ptr_size
    );
}

#[test]
fn collection_and_abi_bridge_layouts_match_native_contract() {
    let ptr_size = std::mem::size_of::<*mut u8>();

    // OriHeap layout
    assert_eq!(std::mem::offset_of!(OriHeap, data), 0);
    assert_eq!(std::mem::offset_of!(OriHeap, len), ptr_size);
    assert_eq!(std::mem::offset_of!(OriHeap, cap), ptr_size + 8);
    assert_eq!(std::mem::offset_of!(OriHeap, version), ptr_size + 16);
    assert_eq!(std::mem::offset_of!(OriHeap, item_kind), ptr_size + 24);
    assert_eq!(std::mem::offset_of!(OriHeap, compare_fn), ptr_size + 32);

    // OriDeque layout
    assert_eq!(std::mem::offset_of!(OriDeque, values), 0);
    assert_eq!(
        std::mem::offset_of!(OriDeque, version),
        std::mem::size_of::<VecDeque<i64>>()
    );

    // OriGraph layout
    assert_eq!(std::mem::offset_of!(OriGraph, nodes), 0);
    assert_eq!(std::mem::offset_of!(OriGraph, len), ptr_size);
    assert_eq!(std::mem::offset_of!(OriGraph, cap), ptr_size + 8);
    assert_eq!(std::mem::offset_of!(OriGraph, version), ptr_size + 16);
    assert_eq!(std::mem::offset_of!(OriGraph, edge_from), ptr_size + 24);
    assert_eq!(std::mem::offset_of!(OriGraph, edge_to), ptr_size * 2 + 24);
    assert_eq!(
        std::mem::offset_of!(OriGraph, edge_weight),
        ptr_size * 3 + 24
    );
    assert_eq!(std::mem::offset_of!(OriGraph, edge_len), ptr_size * 4 + 24);
    assert_eq!(std::mem::offset_of!(OriGraph, edge_cap), ptr_size * 4 + 32);
    assert_eq!(std::mem::offset_of!(OriGraph, directed), ptr_size * 4 + 40);

    // OriBytes FFI bridge layout
    assert_eq!(std::mem::offset_of!(OriBytes, data), 0);
    assert_eq!(std::mem::offset_of!(OriBytes, len), ptr_size);
    assert_eq!(std::mem::size_of::<OriBytes>(), ptr_size + 8);
}

#[test]
fn list_backed_collection_handles_keep_list_layout_and_empty_optionals() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    let ptr_size = std::mem::size_of::<*mut i64>();

    unsafe {
        let deque = ori_deque_new();
        ori_deque_push_back(deque, 10);
        assert_eq!(std::mem::offset_of!(OriList, data), 0);
        assert_eq!(std::mem::offset_of!(OriList, len), ptr_size);
        assert_eq!(std::mem::offset_of!(OriList, cap), ptr_size + 8);
        assert_eq!(ori_deque_len(deque), 1);
        ori_deque_clear(deque);
        assert_eq!(ori_deque_is_empty(deque), 1);
        let empty = ori_deque_pop_front(deque) as *mut OriOptionalInt;
        assert_eq!((*empty).has_value, 0);
        ori_arc_release(empty as *mut u8);
        ori_arc_release(deque as *mut u8);

        let linked = ori_linked_list_new();
        ori_linked_list_push_front(linked, 1);
        ori_linked_list_push_back(linked, 2);
        assert_eq!(ori_linked_list_len(linked), 2);
        let linked_front = ori_linked_list_front(linked) as *mut OriOptionalInt;
        assert_eq!((*linked_front).has_value, 1);
        assert_eq!((*linked_front).value, 1);
        let snapshot = ori_linked_list_to_list(linked);
        assert_eq!(ori_list_get(snapshot, 1), 2);
        ori_arc_release(linked_front as *mut u8);
        ori_arc_release(snapshot as *mut u8);
        ori_linked_list_clear(linked);
        assert_eq!(ori_linked_list_is_empty(linked), 1);
        ori_arc_release(linked as *mut u8);

        let doubly = ori_doubly_linked_list_new();
        for value in 0..128 {
            ori_doubly_linked_list_push_back(doubly, value);
        }
        assert_eq!(ori_doubly_linked_list_len(doubly), 128);
        ori_doubly_linked_list_clear(doubly);
        assert_eq!(ori_doubly_linked_list_is_empty(doubly), 1);

        ori_doubly_linked_list_push_front(doubly, 3);
        ori_doubly_linked_list_push_back(doubly, 4);
        let front = ori_doubly_linked_list_pop_front(doubly) as *mut OriOptionalInt;
        let back = ori_doubly_linked_list_pop_back(doubly) as *mut OriOptionalInt;
        assert_eq!((*front).has_value, 1);
        assert_eq!((*front).value, 3);
        assert_eq!((*back).has_value, 1);
        assert_eq!((*back).value, 4);
        assert_eq!(ori_doubly_linked_list_is_empty(doubly), 1);
        ori_arc_release(front as *mut u8);
        ori_arc_release(back as *mut u8);
        ori_arc_release(doubly as *mut u8);
    }
}

#[test]
fn deque_grows_and_preserves_front_back_order() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let deque = ori_deque_new();
        for value in 0..80 {
            if value % 2 == 0 {
                ori_deque_push_back(deque, value);
            } else {
                ori_deque_push_front(deque, value);
            }
        }

        assert_eq!(ori_deque_len(deque), 80);

        let front = ori_deque_pop_front(deque) as *mut OriOptionalInt;
        let back = ori_deque_pop_back(deque) as *mut OriOptionalInt;
        assert_eq!((*front).has_value, 1);
        assert_eq!((*front).value, 79);
        assert_eq!((*back).has_value, 1);
        assert_eq!((*back).value, 78);
        assert_eq!(ori_deque_len(deque), 78);

        ori_arc_release(front as *mut u8);
        ori_arc_release(back as *mut u8);
        ori_arc_release(deque as *mut u8);
    }
}

#[test]
fn hash_table_wrappers_cover_collision_resize_and_optionals() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let table = ori_hash_table_with_capacity(1);
        let mask = (*table).ht_cap as usize - 1;
        let first = 1_i64;
        let first_slot = hash_i64(first) & mask;
        let second = (2_i64..10_000)
            .find(|candidate| hash_i64(*candidate) & mask == first_slot)
            .expect("expected a colliding key for the initial hash table");

        ori_hash_table_set(table, first, 10);
        ori_hash_table_set(table, second, 20);
        let first_value = ori_hash_table_get(table, first);
        let second_value = ori_hash_table_get(table, second);
        assert_eq!((*first_value).has_value, 1);
        assert_eq!((*first_value).value, 10);
        assert_eq!((*second_value).has_value, 1);
        assert_eq!((*second_value).value, 20);
        ori_arc_release(first_value as *mut u8);
        ori_arc_release(second_value as *mut u8);

        for key in 10_000..10_040 {
            ori_hash_table_set(table, key, key * 10);
        }
        assert!(ori_hash_table_capacity(table) >= 40);
        assert_eq!(ori_hash_table_len(table), 42);

        let removed = ori_hash_table_remove(table, second);
        assert_eq!((*removed).has_value, 1);
        assert_eq!(ori_hash_table_contains(table, second), 0);
        ori_arc_release(removed as *mut u8);

        let missing = ori_hash_table_get(table, second);
        assert_eq!((*missing).has_value, 0);
        ori_arc_release(missing as *mut u8);
        ori_arc_release(table as *mut u8);
    }
}

#[repr(C)]
struct TestScore {
    value: i64,
}

unsafe extern "C" fn test_score_compare(left: i64, right: i64) -> i64 {
    // Mirrors the product contract: Ori compare functions release their
    // managed parameters on exit (callee-owns), and `heap_compare` retains
    // both operands before each call precisely so the callee can consume
    // them. A comparator that kept the references would leak per comparison.
    let result = {
        let l = &*(left as *const TestScore);
        let r = &*(right as *const TestScore);
        l.value - r.value
    };
    ori_arc_release(left as *mut u8);
    ori_arc_release(right as *mut u8);
    result
}

#[test]
fn heap_orders_int_string_and_custom_comparable_values() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let ints = ori_heap_new();
        for value in [40, 10, 30, 20] {
            ori_heap_push(ints, value);
        }
        assert_eq!(ori_heap_len(ints), 4);
        let peek = ori_heap_peek(ints);
        assert_eq!((*peek).has_value, 1);
        assert_eq!((*peek).value, 10);
        ori_arc_release(peek as *mut u8);
        for expected in [10, 20, 30, 40] {
            let item = ori_heap_pop(ints);
            assert_eq!((*item).has_value, 1);
            assert_eq!((*item).value, expected);
            ori_arc_release(item as *mut u8);
        }
        let empty = ori_heap_pop(ints);
        assert_eq!((*empty).has_value, 0);
        ori_arc_release(empty as *mut u8);
        ori_arc_release(ints as *mut u8);

        let strings = ori_heap_new();
        let pear = cstring_from_str("pear");
        let apple = cstring_from_str("apple");
        let orange = cstring_from_str("orange");
        ori_heap_push_string(strings, pear);
        ori_heap_push_string(strings, apple);
        ori_heap_push_string(strings, orange);
        let first = ori_heap_pop(strings);
        assert_eq!((*first).value as *mut u8, apple);
        ori_arc_release(first as *mut u8);
        ori_arc_release(strings as *mut u8);
        ori_arc_release(pear);
        ori_arc_release(apple);
        ori_arc_release(orange);

        let custom = ori_heap_new();
        let scores: Vec<*mut TestScore> = [5, 2, 7]
            .into_iter()
            .map(|value| Box::into_raw(Box::new(TestScore { value })))
            .collect();
        for score in &scores {
            ori_heap_push_custom(
                custom,
                *score as i64,
                test_score_compare as *const std::ffi::c_void,
            );
        }
        for expected in [2, 5, 7] {
            let item = ori_heap_pop(custom);
            let score = &*((*item).value as *const TestScore);
            assert_eq!(score.value, expected);
            ori_arc_release(item as *mut u8);
        }
        ori_arc_release(custom as *mut u8);
        for score in scores {
            drop(Box::from_raw(score));
        }
    }
}

#[test]
fn heap_custom_compare_releases_temporary_retains() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let heap = ori_heap_new_custom(test_score_compare as *const std::ffi::c_void);
        for value in [3, 1, 2] {
            let score = ori_alloc(std::mem::size_of::<TestScore>(), Some(test_destructor))
                as *mut TestScore;
            (*score).value = value;
            ori_heap_push_custom(
                heap,
                score as i64,
                test_score_compare as *const std::ffi::c_void,
            );
            ori_arc_register_edge(heap as *mut u8, score as *mut u8);
            ori_arc_release(score as *mut u8);
        }

        assert_eq!(ori_arc_live_allocations(), 4);
        ori_arc_release(heap as *mut u8);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 3);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn heap_pop_and_peek_keep_managed_values_alive() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let heap = ori_heap_new_custom(test_score_compare as *const std::ffi::c_void);
        let score =
            ori_alloc(std::mem::size_of::<TestScore>(), Some(test_destructor)) as *mut TestScore;
        (*score).value = 9;
        ori_heap_push_custom(
            heap,
            score as i64,
            test_score_compare as *const std::ffi::c_void,
        );
        ori_arc_release(score as *mut u8);

        let peeked = ori_heap_peek(heap);
        assert_eq!((*peeked).has_value, 1);
        assert_eq!((*((*peeked).value as *mut TestScore)).value, 9);
        ori_arc_release(heap as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 2);
        ori_arc_release(peeked as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let heap = ori_heap_new_custom(test_score_compare as *const std::ffi::c_void);
        let score =
            ori_alloc(std::mem::size_of::<TestScore>(), Some(test_destructor)) as *mut TestScore;
        (*score).value = 4;
        ori_heap_push_custom(
            heap,
            score as i64,
            test_score_compare as *const std::ffi::c_void,
        );
        ori_arc_release(score as *mut u8);

        let popped = ori_heap_pop(heap);
        assert_eq!((*popped).has_value, 1);
        assert_eq!((*((*popped).value as *mut TestScore)).value, 4);
        ori_arc_release(heap as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 2);
        ori_arc_release(popped as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn optional_and_result_layouts_match_native_backend() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    let ptr_size = std::mem::size_of::<*mut u8>();

    assert_eq!(std::mem::offset_of!(OriOptionalInt, has_value), 0);
    assert_eq!(std::mem::offset_of!(OriOptionalInt, value), 8);
    assert_eq!(std::mem::size_of::<OriOptionalInt>(), 16);

    assert_eq!(std::mem::offset_of!(OriOptionalFloat, has_value), 0);
    assert_eq!(std::mem::offset_of!(OriOptionalFloat, value), 8);
    assert_eq!(std::mem::size_of::<OriOptionalFloat>(), 16);

    unsafe {
        let payload = cstring_from_str("ok");
        let result = new_result(true, payload);
        assert_eq!(*result, 1);
        assert_eq!(*(result.add(ptr_size) as *mut *mut u8), payload);
        free_result(result);
        ori_arc_release(payload);
    }
}

#[test]
fn borrowed_managed_optional_releases_its_payload_edge_once() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let list = ori_list_new();
        let item = cstring_from_str("item");
        ori_list_push_borrowed_maybe_managed(list, item as i64);
        ori_arc_release(item);

        let optional = ori_list_try_get(list, 0);
        let borrowed_item = (*optional).value as *mut u8;
        assert_eq!(
            (*header_for(borrowed_item))
                .refcount
                .load(AtomicOrdering::SeqCst),
            2
        );

        ori_arc_release(optional as *mut u8);

        assert!(header_for_registered(borrowed_item).is_some());
        assert_eq!(cstr_str(borrowed_item), "item");
        assert_eq!(
            (*header_for(borrowed_item))
                .refcount
                .load(AtomicOrdering::SeqCst),
            1
        );

        ori_arc_release(list as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn runtime_created_collection_snapshots_keep_managed_elements_alive() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let source = ori_list_new();
        let item = cstring_from_str("item");
        ori_list_push_borrowed_maybe_managed(source, item as i64);
        ori_arc_release(item);

        let slice = ori_list_slice(source, 0, 1);
        let slice_item = ori_list_get(slice, 0) as *mut u8;
        ori_arc_release(source as *mut u8);
        assert_eq!(cstr_str(slice_item), "item");
        ori_arc_release(slice as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let map = ori_map_new();
        let key = cstring_from_str("key");
        let value = cstring_from_str("value");
        ori_map_set_string(map, key, value as i64);
        ori_arc_release(key);
        ori_arc_release(value);

        let keys = ori_map_keys(map);
        let values = ori_map_values(map);
        let entries = ori_map_entries(map);
        let key_snapshot = ori_list_get(keys, 0) as *mut u8;
        let value_snapshot = ori_list_get(values, 0) as *mut u8;
        let entry = ori_list_get(entries, 0) as *mut i64;
        ori_arc_release(map as *mut u8);
        assert_eq!(cstr_str(key_snapshot), "key");
        assert_eq!(cstr_str(value_snapshot), "value");
        assert_eq!(cstr_str(*entry as *mut u8), "key");
        assert_eq!(cstr_str(*entry.add(1) as *mut u8), "value");
        ori_arc_release(keys as *mut u8);
        ori_arc_release(values as *mut u8);
        ori_arc_release(entries as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let graph = ori_graph_new(1);
        let from = cstring_from_str("from");
        let to = cstring_from_str("to");
        ori_graph_add_node_string(graph, from);
        ori_graph_add_node_string(graph, to);
        ori_graph_add_edge_string(graph, from, to);
        ori_arc_release(from);
        ori_arc_release(to);

        let nodes = ori_graph_nodes(graph);
        let lookup = cstring_from_str("from");
        let neighbors = ori_graph_neighbors_string(graph, lookup);
        ori_arc_release(lookup);
        let edges = ori_graph_edges(graph);
        let node_snapshot = ori_list_get(nodes, 0) as *mut u8;
        let neighbor_snapshot = ori_list_get(neighbors, 0) as *mut u8;
        let edge = ori_list_get(edges, 0) as *mut i64;
        ori_arc_release(graph as *mut u8);
        assert_eq!(cstr_str(node_snapshot), "from");
        assert_eq!(cstr_str(neighbor_snapshot), "to");
        assert_eq!(cstr_str(*edge as *mut u8), "from");
        assert_eq!(cstr_str(*edge.add(1) as *mut u8), "to");
        ori_arc_release(nodes as *mut u8);
        ori_arc_release(neighbors as *mut u8);
        ori_arc_release(edges as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn map_set_owns_managed_key_and_value_without_manual_registration() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let map = ori_map_new();
        let key = cstring_from_str("status");
        let value = cstring_from_str("0");
        ori_map_set_string(map, key, value as i64);

        // The map must retain both temporaries after their producer releases
        // its references. This is the same ownership pattern used by process
        // capture results and exposes allocator-dependent use-after-free bugs.
        ori_arc_release(key);
        ori_arc_release(value);

        let lookup = cstring_from_str("status");
        let stored = ori_map_get_string(map, lookup);
        assert_eq!(cstr_str(stored as *mut u8), "0");
        ori_arc_release(lookup);
        ori_arc_release(map as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn list_reserve_and_with_capacity_grow_once() {
    unsafe {
        let list = ori_list_with_capacity(1000);
        assert!(ori_list_capacity(list) >= 1000);
        assert_eq!(ori_list_len(list), 0);
        for i in 0..1000 {
            ori_list_push(list, i);
        }
        assert_eq!(ori_list_len(list), 1000);
        assert!(ori_list_capacity(list) >= 1000);
        ori_list_reserve(list, 2000);
        assert!(ori_list_capacity(list) >= 2000);
        assert_eq!(ori_list_get(list, 0), 0);
        assert_eq!(ori_list_get(list, 999), 999);
        ori_arc_release(list as *mut u8);
    }
}

#[test]
fn collection_removal_paths_unregister_arc_edges() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let list = ori_list_new();
        let list_item = cstring_from_str("list");
        ori_list_push_borrowed_maybe_managed(list, list_item as i64);
        ori_arc_release(list_item);
        assert_eq!(ori_arc_live_allocations(), 2);
        ori_list_clear(list);
        assert_eq!(ori_arc_live_allocations(), 1);
        ori_arc_release(list as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let deque = ori_deque_new();
        let deque_item = cstring_from_str("deque");
        deque_push_borrowed_maybe_managed(deque, deque_item as i64, false);
        ori_arc_release(deque_item);
        let popped = ori_deque_pop_front(deque) as *mut OriOptionalInt;
        assert_eq!((*popped).has_value, 1);
        assert_eq!(cstr_str((*popped).value as *mut u8), "deque");
        ori_arc_release(deque as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 2);
        ori_arc_release(popped as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let set = ori_set_new();
        let set_item = cstring_from_str("set");
        ori_set_add_string(set, set_item);
        ori_set_register_borrowed_maybe_managed(set, set_item as i64);
        ori_arc_release(set_item);
        assert_eq!(ori_arc_live_allocations(), 2);
        ori_set_remove_string(set, set_item);
        assert_eq!(ori_arc_live_allocations(), 1);
        ori_arc_release(set as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let map = ori_map_new();
        let key = cstring_from_str("key");
        let old_value = cstring_from_str("old");
        ori_map_set_string(map, key, old_value as i64);
        ori_arc_release(key);
        ori_arc_release(old_value);
        assert_eq!(ori_arc_live_allocations(), 3);

        let new_value = cstring_from_str("new");
        ori_map_set_string(map, key, new_value as i64);
        ori_arc_release(new_value);
        assert_eq!(ori_arc_live_allocations(), 3);

        ori_map_clear(map);
        assert_eq!(ori_arc_live_allocations(), 1);
        ori_arc_release(map as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn tree_and_graph_runtime_own_managed_edges() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let root = cstring_from_str("root");
        let tree = ori_tree_new(root as i64);
        ori_arc_release(root);
        let tree_live = ori_arc_live_allocations();
        let child_value = cstring_from_str("child");
        let child = ori_tree_add_child(tree, ori_tree_root(tree), child_value as i64);
        ori_arc_release(child_value);
        assert_eq!(ori_arc_live_allocations(), tree_live + 2);

        ori_tree_remove_subtree(tree, child);
        assert_eq!(ori_arc_live_allocations(), tree_live);
        ori_arc_release(tree as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);

        let left = cstring_from_str("left");
        let right = cstring_from_str("right");
        let graph = ori_graph_new(0);
        ori_graph_add_edge_string(graph, left, right);
        ori_arc_release(left);
        ori_arc_release(right);
        assert_eq!(ori_arc_live_allocations(), 3);

        let lookup_left = cstring_from_str("left");
        ori_graph_remove_node_string(graph, lookup_left);
        ori_arc_release(lookup_left);
        assert_eq!(ori_arc_live_allocations(), 2);

        let closure = ori_graph_transitive_closure(graph);
        ori_arc_release(graph as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 2);
        ori_arc_release(closure as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn concurrency_handle_layouts_are_opaque_references() {
    assert_eq!(std::mem::offset_of!(OriTaskJob, handle), 0);
    assert_eq!(std::mem::offset_of!(OriChannel, state), 0);
    assert_eq!(std::mem::offset_of!(OriAtomicInt, value), 0);
    assert_eq!(std::mem::offset_of!(OriFuture, state), 0);
    assert!(std::mem::size_of::<OriTaskJob>() > 0);
    assert!(std::mem::size_of::<OriChannel>() > 0);
    assert!(std::mem::size_of::<OriAtomicInt>() > 0);
    assert!(std::mem::size_of::<OriFuture>() > 0);
}

#[test]
fn arc_retain_release_updates_refcount_and_runs_destructor() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let ptr = ori_alloc(8, Some(test_destructor));
        assert!(!ptr.is_null());

        let header = header_for(ptr);
        assert_eq!((*header).refcount.load(AtomicOrdering::SeqCst), 1);

        ori_arc_retain(ptr);
        assert_eq!((*header).refcount.load(AtomicOrdering::SeqCst), 2);

        ori_arc_release(ptr);
        assert_eq!((*header).refcount.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);

        ori_arc_release(ptr);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 1);
    }
}

/// Each managed field/slot is an independent ownership edge, even when two
/// slots happen to contain the same pointer. Removing one slot must not release
/// the child still held by the other slot.
#[test]
fn arc_edges_preserve_duplicate_slots() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let owner = ori_alloc(8, None);
        let child = ori_alloc(8, Some(test_destructor));
        assert!(!owner.is_null() && !child.is_null());

        ori_arc_register_edge(owner, child);
        ori_arc_register_edge(owner, child);
        assert_eq!(
            arc_state()
                .lock()
                .unwrap()
                .edges
                .by_owner
                .get(&(owner as usize))
                .map(Vec::len),
            Some(2)
        );
        assert_eq!((*header_for(child)).refcount.load(Ordering::SeqCst), 3);

        // Drop the child's original external reference. The two slot
        // references must keep the child alive while the owner stays live so
        // its slots can be removed safely.
        ori_arc_release(child);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);

        ori_arc_unregister_edge(owner, child);
        assert_eq!(
            arc_state()
                .lock()
                .unwrap()
                .edges
                .by_owner
                .get(&(owner as usize))
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(ori_arc_live_allocations(), 2);

        ori_arc_unregister_edge(owner, child);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_arc_live_allocations(), 1);

        ori_arc_release(owner);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

/// Releasing an owner must cascade once per stored slot, including when every
/// slot contains the same child pointer. This exercises `take_children_of`, not
/// the explicit unregister path covered above.
#[test]
fn arc_owner_teardown_releases_every_duplicate_slot() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let owner = ori_alloc(8, None);
        let child = ori_alloc(8, Some(test_destructor));

        ori_arc_register_edge(owner, child);
        ori_arc_register_edge(owner, child);
        ori_arc_release(child);

        assert_eq!(ori_arc_live_allocations(), 2);
        ori_arc_release(owner);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

/// Trial deletion must subtract every internal slot reference. Deduplicating
/// either direction of the edge index would leave this two-node cycle live.
#[test]
fn arc_cycle_collection_counts_parallel_edges() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let left = ori_alloc(8, Some(test_destructor));
        let right = ori_alloc(8, Some(test_destructor));

        ori_arc_register_edge(left, right);
        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);
        ori_arc_release(left);
        ori_arc_release(right);

        assert_eq!(ori_arc_live_allocations(), 2);
        assert_eq!(ori_arc_collect_cycles(), 2);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn arc_collect_cycles_reclaims_struct_like_registered_cycle() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let left = ori_alloc(8, Some(test_destructor));
        let right = ori_alloc(8, Some(test_destructor));
        assert!(!left.is_null());
        assert!(!right.is_null());

        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);

        assert_eq!((*header_for(left)).refcount.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(
            (*header_for(right)).refcount.load(AtomicOrdering::SeqCst),
            2
        );

        ori_arc_release(left);
        ori_arc_release(right);

        assert_eq!((*header_for(left)).refcount.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(
            (*header_for(right)).refcount.load(AtomicOrdering::SeqCst),
            1
        );

        assert_eq!(ori_arc_collect_cycles(), 2);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(ori_arc_collect_cycles(), 0);
    }
}

#[test]
fn cooperative_collect_fires_after_allocation_threshold() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    // Reset the cooperative counter window so the test controls the delta
    // regardless of allocations performed by earlier tests.
    let baseline = COOPERATIVE_ALLOC_COUNTER.load(AtomicOrdering::SeqCst);
    COOPERATIVE_LAST_COLLECTED.store(baseline, AtomicOrdering::SeqCst);
    let threshold = cooperative_collect_threshold();

    unsafe {
        // Build an orphan cycle: left <-> right. After releasing the external
        // refs both objects survive only via the cycle edges (refcount 1).
        let left = ori_alloc(8, Some(test_destructor));
        let right = ori_alloc(8, Some(test_destructor));
        assert!(!left.is_null() && !right.is_null());
        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);
        ori_arc_release(left);
        ori_arc_release(right);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);

        // A second cooperative call with no new allocations must be a no-op.
        maybe_collect_cycles_cooperative();
        assert_eq!(
            TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst),
            0,
            "no collection before crossing the threshold"
        );

        // Allocate enough managed objects to cross the cooperative threshold.
        let mut buffers = Vec::new();
        for _ in 0..(threshold + 4) {
            let buf = ori_alloc(8, None);
            assert!(!buf.is_null());
            buffers.push(buf);
        }

        // Now the cooperative trigger must call ori_arc_collect_cycles and
        // reclaim the orphaned cycle.
        maybe_collect_cycles_cooperative();
        assert_eq!(
            TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst),
            2,
            "cooperative collection should reclaim the orphan cycle"
        );

        // A repeat call without new allocations must not collect again.
        maybe_collect_cycles_cooperative();
        assert_eq!(
            TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst),
            2,
            "no double collection within the same window"
        );

        for buf in buffers {
            ori_arc_release(buf);
        }
    }
}

#[test]
fn arc_collect_cycles_reclaims_list_map_set_cycle() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let list = ori_list_new() as *mut u8;
        let map = ori_map_new() as *mut u8;
        let set = ori_set_new() as *mut u8;

        ori_arc_register_edge(list, map);
        ori_arc_register_edge(map, set);
        ori_arc_register_edge(set, list);

        ori_arc_release(list);
        ori_arc_release(map);
        ori_arc_release(set);

        assert_eq!(ori_arc_collect_cycles(), 3);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn arc_collect_cycles_reclaims_graph_cycle() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let left = ori_graph_new(1) as *mut u8;
        let right = ori_graph_new(1) as *mut u8;

        ori_graph_add_node(left as *mut OriGraph, right as i64);
        ori_graph_add_node(right as *mut OriGraph, left as i64);
        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);

        ori_arc_release(left);
        ori_arc_release(right);

        assert_eq!(ori_arc_collect_cycles(), 2);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn graph_custom_nodes_use_equatable_callback_for_lookup_and_edges() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    // SAFETY: the test serializes all ARC state with `TEST_ARC_LOCK` and keeps
    // every graph/node allocation live until its explicit release below.
    unsafe {
        let callback = test_graph_node_equals as *const () as *const std::ffi::c_void;
        let graph = ori_graph_new(0);
        ori_graph_set_eq(graph, callback);
        let first = ori_alloc(8, None);
        let equivalent = ori_alloc(8, None);
        *(first as *mut i64) = 7;
        *(equivalent as *mut i64) = 7;

        ori_graph_add_node(graph, first as i64);
        assert_eq!(ori_graph_has_node(graph, equivalent as i64), 1);
        ori_graph_add_edge(graph, first as i64, equivalent as i64);
        assert_eq!(
            ori_graph_has_edge(graph, equivalent as i64, first as i64),
            1
        );

        ori_arc_release(graph as *mut u8);
        ori_arc_release(first);
        ori_arc_release(equivalent);
    }
    assert_eq!(ori_arc_live_allocations(), 0);
}

#[test]
fn custom_map_and_set_use_hash_callback_and_repair_dense_slots() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    // SAFETY: callbacks read only the initialized i64 payloads below.  Every
    // managed allocation is released after the containers drop their edges.
    unsafe {
        let hash = test_structural_key_hash as *const () as *const std::ffi::c_void;
        let equals = test_structural_key_equals as *const () as *const std::ffi::c_void;
        let first = ori_alloc(8, None);
        let equivalent = ori_alloc(8, None);
        let different = ori_alloc(8, None);
        *(first as *mut i64) = 11;
        *(equivalent as *mut i64) = 11;
        *(different as *mut i64) = 22;

        let set = ori_set_new_custom_with_hash(hash, equals);
        ori_set_add_custom_with_hash_eq(set, first as i64, hash, equals);
        ori_set_add_custom_with_hash_eq(set, different as i64, hash, equals);
        assert_eq!(ori_set_contains_custom(set, equivalent as i64), 1);
        assert_eq!(ori_set_len(set), 2);
        ori_set_remove_custom(set, first as i64);
        assert_eq!(ori_set_contains_custom(set, different as i64), 1);
        assert_eq!(ori_set_len(set), 1);
        ori_arc_release(set as *mut u8);

        let map = ori_map_new_custom(hash, equals);
        ori_map_set_custom(map, first as i64, 101);
        ori_map_set_custom(map, different as i64, 202);
        assert_eq!(ori_map_contains_custom(map, equivalent as i64), 1);
        assert_eq!(ori_map_get_custom(map, equivalent as i64), 101);
        ori_map_remove_custom(map, first as i64);
        assert_eq!(ori_map_contains_custom(map, different as i64), 1);
        ori_arc_release(map as *mut u8);

        ori_arc_release(first);
        ori_arc_release(equivalent);
        ori_arc_release(different);
    }
    assert_eq!(ori_arc_live_allocations(), 0);
}

#[test]
fn arc_collect_cycles_reclaims_closure_environment_cycle() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let closure = test_closure_object();
        let env = ori_alloc(8, None);

        ori_arc_register_edge(closure, env);
        ori_arc_register_edge(env, closure);

        ori_arc_release(closure);
        ori_arc_release(env);

        assert_eq!(ori_arc_collect_cycles(), 2);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn arc_retain_release_stress_keeps_single_owner_alive() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let ptr = ori_alloc(8, Some(test_destructor));
        for _ in 0..10_000 {
            ori_arc_retain(ptr);
        }
        for _ in 0..10_000 {
            ori_arc_release(ptr);
        }

        assert_eq!((*header_for(ptr)).refcount.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);

        ori_arc_release(ptr);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn arc_retain_release_concurrency_stress_keeps_refcount_balanced() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let ptr = ori_alloc(8, Some(test_destructor));
        let ptr_addr = ptr as usize;
        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(std::thread::spawn(move || {
                let ptr = ptr_addr as *mut u8;
                for _ in 0..2_000 {
                    ori_arc_retain(ptr);
                    ori_arc_release(ptr);
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!((*header_for(ptr)).refcount.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);

        ori_arc_release(ptr);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn task_spawn_join_returns_result_payload() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let closure = test_closure_object();
        let job = ori_task_spawn(closure);
        assert!(!job.is_null());
        let result = ori_task_join(job);
        assert_eq!(result_flag(result), 1);
        assert_eq!(result_i64_payload(result), 41);
        free_result(result);
        ori_arc_release(job as *mut u8);
    }
}

#[test]
fn task_callback_panic_fails_join_and_releases_the_closure() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let closure = test_panicking_task_closure_object();
        let job = ori_task_spawn(closure);
        let result = ori_task_join(job);
        assert_eq!(result_flag(result), 0);
        free_result(result);
        ori_arc_release(job as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn channel_send_receive_uses_synchronized_queue() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let channel = ori_channel_create();
        assert!(!channel.is_null());
        let send = ori_channel_send(channel, 7);
        assert_eq!(result_flag(send), 1);
        free_result(send);
        assert!(!arc_state()
            .lock()
            .unwrap()
            .edges
            .by_owner
            .contains_key(&(channel as usize)));

        let received = ori_channel_receive(channel);
        assert_eq!(result_flag(received), 1);
        assert_eq!(result_i64_payload(received), 7);
        free_result(received);

        ori_channel_close(channel);
        let closed = ori_channel_receive(channel);
        assert_eq!(result_flag(closed), 0);
        free_result(closed);
        ori_arc_release(channel as *mut u8);
    }
}

#[test]
fn bounded_channel_applies_backpressure_and_rejects_invalid_capacity() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        for capacity in [0, -1] {
            let invalid = ori_channel_create_bounded(capacity);
            assert_eq!(result_flag(invalid), 0);
            free_result(invalid);
        }

        let channel_option = ori_channel_create_bounded(1);
        assert_eq!(result_flag(channel_option), 1);
        let channel = result_ptr_payload(channel_option);
        let first = ori_channel_send(channel as *mut OriChannel, 7);
        assert_eq!(result_flag(first), 1);
        free_result(first);

        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let channel_addr = channel as usize;
        std::thread::spawn(move || {
            let second = ori_channel_send(channel_addr as *mut OriChannel, 8);
            let accepted = result_flag(second) == 1;
            free_result(second);
            finished_tx.send(accepted).unwrap();
        });

        assert!(
            finished_rx
                .recv_timeout(std::time::Duration::from_millis(20))
                .is_err(),
            "a full bounded channel must block a sender"
        );

        let received = ori_channel_receive(channel as *mut OriChannel);
        assert_eq!(result_flag(received), 1);
        assert_eq!(result_i64_payload(received), 7);
        free_result(received);
        assert!(finished_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap());

        ori_channel_close(channel as *mut OriChannel);
        free_result(channel_option);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn channel_scalar_route_does_not_classify_pointer_shaped_integer() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let channel = ori_channel_create();
        let managed = cstring_from_str("not-a-managed-channel-element");
        assert!(!channel.is_null() && !managed.is_null());

        let sent = ori_channel_send(channel, managed as i64);
        assert_eq!(result_flag(sent), 1);
        free_result(sent);
        assert!(!arc_state()
            .lock()
            .unwrap()
            .edges
            .by_owner
            .contains_key(&(channel as usize)));

        let received = ori_channel_receive(channel);
        assert_eq!(result_flag(received), 1);
        assert_eq!(result_i64_payload(received), managed as i64);
        free_result(received);

        ori_arc_release(managed);
        ori_channel_close(channel);
        ori_arc_release(channel as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn channel_managed_route_rejects_unregistered_payload() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let channel = ori_channel_create();
        let sent = ori_channel_send_managed(channel, 1);
        assert_eq!(result_flag(sent), 0);
        free_result(sent);

        ori_channel_close(channel);
        let received = ori_channel_receive(channel);
        assert_eq!(result_flag(received), 0);
        free_result(received);
        ori_arc_release(channel as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn channel_transfers_managed_value_ownership_to_result() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let channel = ori_channel_create();
        let value = cstring_from_str("queued");
        assert!(!channel.is_null() && !value.is_null());

        let sent = ori_channel_send_managed(channel, value as i64);
        assert_eq!(result_flag(sent), 1);
        free_result(sent);
        // The queue owns the value after the caller drops its temporary.
        ori_arc_release(value);
        assert_eq!(ori_arc_live_allocations(), 2);

        let received = ori_channel_receive(channel);
        assert_eq!(result_flag(received), 1);
        let received_value = result_ptr_payload(received);
        assert_eq!(cstr_str(received_value), "queued");

        // The result now owns the value; destroying the channel must not
        // release it a second time or leave a dangling queue edge.
        ori_channel_close(channel);
        ori_arc_release(channel as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 2);

        free_result(received);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn channel_drops_unreceived_managed_values_with_queue() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let channel = ori_channel_create();
        let value = cstring_from_str("discarded");
        let sent = ori_channel_send_managed(channel, value as i64);
        free_result(sent);
        ori_arc_release(value);
        assert_eq!(ori_arc_live_allocations(), 2);

        ori_channel_close(channel);
        ori_arc_release(channel as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

/// `send` and `close` share the channel-state mutex: every racing send must
/// either publish one independently owned queue slot or fail without taking an
/// edge. Reusing the same managed pointer stresses edge multiplicity as well as
/// the close boundary.
#[test]
fn channel_close_race_balances_duplicate_managed_slots() {
    const SENDER_COUNT: usize = 4;
    const ATTEMPTS_PER_SENDER: usize = 128;

    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let channel = ori_channel_create();
        let value = cstring_from_str("shared");
        assert!(!channel.is_null() && !value.is_null());

        // Seed two identical slots so multiplicity is exercised even when the
        // closer wins immediately after the barrier below.
        for _ in 0..2 {
            let sent = ori_channel_send_managed(channel, value as i64);
            assert_eq!(result_flag(sent), 1);
            free_result(sent);
        }

        let start = std::sync::Arc::new(std::sync::Barrier::new(SENDER_COUNT + 1));
        let mut senders = Vec::with_capacity(SENDER_COUNT);
        // The main thread owns both allocations until every sender joins, so
        // the raw addresses remain live for the complete contention window.
        for _ in 0..SENDER_COUNT {
            let start = std::sync::Arc::clone(&start);
            let channel_addr = channel as usize;
            let value_addr = value as usize;
            senders.push(std::thread::spawn(move || {
                start.wait();
                let mut successful = 0;
                for _ in 0..ATTEMPTS_PER_SENDER {
                    let sent = ori_channel_send_managed(
                        channel_addr as *mut OriChannel,
                        value_addr as i64,
                    );
                    let was_sent = result_flag(sent) == 1;
                    free_result(sent);
                    successful += usize::from(was_sent);
                    std::thread::yield_now();
                }
                successful
            }));
        }

        start.wait();
        std::thread::yield_now();
        ori_channel_close(channel);

        let successful_sends = 2 + senders
            .into_iter()
            .map(|sender| sender.join().expect("channel sender must not panic"))
            .sum::<usize>();

        let mut received_values = 0;
        loop {
            let received = ori_channel_receive(channel);
            if result_flag(received) == 0 {
                free_result(received);
                break;
            }
            assert_eq!(result_ptr_payload(received), value);
            received_values += 1;
            free_result(received);
        }
        assert_eq!(received_values, successful_sends);

        ori_arc_release(value);
        ori_arc_release(channel as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn async_udp_operation_keeps_socket_alive_until_completion() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let host = cstring_from_str("127.0.0.1");
        let bind_result = ori_net_udp_bind(host, 0);
        assert_eq!(result_flag(bind_result), 1);
        let socket = result_ptr_payload(bind_result);
        assert!(!socket.is_null());

        // Keep a caller-owned reference for the explicit close operation. The
        // bind result and the async job each own independent references too.
        ori_arc_retain(socket);
        let future = ori_net_udp_recv_from_async(socket, 1);
        assert!(!future.is_null());
        ori_net_udp_close(socket);

        let result = ori_task_block_on_ptr(future);
        assert!(!result.is_null());
        assert_eq!(result_flag(result), 0, "closed socket must report an error");

        ori_arc_release(result);
        ori_arc_release(future as *mut u8);
        free_result(bind_result);
        ori_arc_release(host);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn cancelled_udp_job_releases_future_and_socket_keepalives() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let host = cstring_from_str("127.0.0.1");
        let bind_result = ori_net_udp_bind(host, 0);
        assert_eq!(result_flag(bind_result), 1);
        let socket = result_ptr_payload(bind_result);
        assert!(!socket.is_null());

        ori_arc_retain(socket);
        let future = ori_net_udp_recv_from_async(socket, 1);
        let token = ori_task_create_token();
        assert!(!future.is_null() && !token.is_null());
        ori_task_associate(token, future as *mut u8);
        ori_task_cancel(token);
        ori_net_udp_close(socket);

        assert!(ori_task_block_on_ptr(future).is_null());
        assert_eq!(ori_task_last_await_status(), 3);

        ori_arc_release(future as *mut u8);
        ori_arc_release(token);
        free_result(bind_result);
        ori_arc_release(host);

        let deadline = Instant::now() + Duration::from_secs(2);
        while ori_arc_live_allocations() != 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn managed_bytes_preserve_embedded_nul_across_list_and_udp_paths() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let bytes = cstring_from_bytes(vec![0x41, 0x00, 0x42]);
        let list = ori_bytes_to_list(bytes);
        assert_eq!(ori_list_len(list), 3);
        assert_eq!(ori_list_get(list, 0), 0x41);
        assert_eq!(ori_list_get(list, 1), 0x00);
        assert_eq!(ori_list_get(list, 2), 0x42);
        ori_arc_release(list as *mut u8);

        let host = cstring_from_str("127.0.0.1");
        let bind_result = ori_net_udp_bind(host, 0);
        assert_eq!(result_flag(bind_result), 1);
        let sender = result_ptr_payload(bind_result);
        let receiver = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind UDP receiver");
        receiver
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .expect("set UDP read timeout");
        let receiver_port = receiver.local_addr().expect("receiver address").port();

        let send_result = ori_net_udp_send_to(sender, host, i64::from(receiver_port), bytes);
        assert_eq!(result_flag(send_result), 1);
        assert_eq!(result_i64_payload(send_result), 3);
        free_result(send_result);

        let mut received = [0_u8; 8];
        let (count, _) = receiver
            .recv_from(&mut received)
            .expect("receive UDP payload");
        assert_eq!(&received[..count], &[0x41, 0x00, 0x42]);

        ori_arc_release(bytes);
        free_result(bind_result);
        ori_arc_release(host);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn managed_bytes_preserve_embedded_nul_across_sync_tcp_write() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind TCP listener");
    let port = listener.local_addr().expect("listener address").port();
    let receiver = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept TCP connection");
        let mut received = [0_u8; 8];
        use std::io::Read;
        let count = stream.read(&mut received).expect("read TCP payload");
        (count, received)
    });

    unsafe {
        let host = cstring_from_str("127.0.0.1");
        let connect_result = ori_net_connect(host, i64::from(port), 1_000);
        assert_eq!(result_flag(connect_result), 1);
        let connection = result_ptr_payload(connect_result);
        let bytes = cstring_from_bytes(vec![0x41, 0x00, 0x42]);
        let write_result = ori_net_write_all(connection, bytes);
        assert_eq!(result_flag(write_result), 1);
        free_result(write_result);

        ori_arc_retain(connection);
        ori_net_close(connection);
        free_result(connect_result);
        ori_arc_release(bytes);
        ori_arc_release(host);
    }

    let (count, received) = receiver.join().expect("join TCP receiver");
    assert_eq!(&received[..count], &[0x41, 0x00, 0x42]);
    assert_eq!(ori_arc_live_allocations(), 0);
}

#[test]
fn io_reactor_contains_panicking_jobs_and_keeps_processing() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let failed = spawn_readiness_io_future(
            std::ptr::null_mut(),
            || None,
            IoInterest::Read,
            || panic!("test I/O job panic"),
        );
        assert_eq!(ori_task_block_on(failed), 0);
        assert_eq!(ori_task_last_await_status(), 2);
        ori_arc_release(failed as *mut u8);

        let succeeded = spawn_readiness_io_future(
            std::ptr::null_mut(),
            || None,
            IoInterest::Read,
            || 7usize as *mut u8,
        );
        assert_eq!(ori_task_block_on(succeeded), 7);
        assert_eq!(ori_task_last_await_status(), 1);
        ori_arc_release(succeeded as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn io_reactor_recovers_a_poisoned_queue_lock() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _queue = io_reactor().jobs.lock().unwrap();
        panic!("poison the reactor queue for recovery testing");
    }));
    assert!(poisoned.is_err());

    unsafe {
        let future = spawn_readiness_io_future(
            std::ptr::null_mut(),
            || None,
            IoInterest::Read,
            || 11usize as *mut u8,
        );
        assert_eq!(ori_task_block_on(future), 11);
        assert_eq!(ori_task_last_await_status(), 1);
        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn atomic_int_load_store_and_add_are_thread_safe() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let value = ori_atomic_new(10);
        assert_eq!(ori_atomic_load(value), 10);
        ori_atomic_store(value, 12);
        assert_eq!(ori_atomic_load(value), 12);
        assert_eq!(ori_atomic_add(value, 5), 17);
        assert_eq!(ori_atomic_load(value), 17);
        ori_arc_release(value as *mut u8);
    }
}

#[test]
fn executor_runs_scheduled_continuations() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    unsafe {
        ori_executor_drain();
    }
    TEST_EXECUTOR_CALLBACKS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let first = test_executor_closure_object();
        let second = test_executor_closure_object();
        ori_executor_schedule(first);
        ori_executor_schedule(second);

        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_executor_drain(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(ori_executor_run_one(), 0);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn future_poll_reports_ready_failed_and_cancelled_states() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let future = alloc_pending_future();
        assert_eq!(ori_future_poll(future), 0);
        ori_future_complete_i64(future, 42);
        assert_eq!(ori_future_poll(future), 1);
        assert_eq!(ori_future_value_i64(future), 42);
        assert_eq!(ori_task_block_on(future), 42);
        ori_arc_release(future as *mut u8);

        let failed = alloc_pending_future();
        ori_future_fail(failed);
        assert_eq!(ori_future_poll(failed), 2);
        assert_eq!(ori_task_block_on(failed), 0);
        ori_arc_release(failed as *mut u8);

        let cancelled = alloc_pending_future();
        ori_future_cancel(cancelled);
        assert_eq!(ori_future_poll(cancelled), 3);
        assert_eq!(ori_task_block_on(cancelled), 0);
        ori_arc_release(cancelled as *mut u8);
    }
}

#[test]
fn future_pending_constructor_returns_pollable_pending_future() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let future = ori_future_pending();
        assert!(!future.is_null());
        assert_eq!(ori_future_poll(future), 0);
        ori_future_complete_i64(future, 77);
        assert_eq!(ori_future_poll(future), 1);
        assert_eq!(ori_future_value_i64(future), 77);
        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn future_complete_ptr_keeps_managed_payload_until_future_release() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let payload = ori_alloc(1, None);
        let future = ori_future_pending();
        assert_eq!(ori_arc_live_allocations(), 2);

        ori_future_complete_ptr(future, payload);
        ori_arc_release(payload);

        assert_eq!(ori_future_poll(future), 1);
        assert_eq!(ori_future_value_ptr(future), payload);
        assert_eq!(ori_arc_live_allocations(), 2);

        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn future_on_ready_schedules_registered_continuation() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    unsafe {
        ori_executor_drain();
    }
    TEST_EXECUTOR_CALLBACKS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let future = alloc_pending_future();
        let continuation = test_executor_closure_object();
        ori_future_on_ready(future, continuation);

        assert_eq!(ori_executor_run_one(), 0);
        ori_future_complete_void(future);
        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_executor_run_one(), 0);
        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn pending_future_continuation_does_not_block_executor_queue() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    unsafe {
        ori_executor_drain();
    }
    TEST_EXECUTOR_CALLBACKS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let future = alloc_pending_future();
        let continuation = test_executor_closure_object();
        ori_future_on_ready(future, continuation);

        let independent = test_executor_closure_object();
        ori_executor_schedule(independent);

        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_executor_run_one(), 0);

        ori_future_complete_void(future);
        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 2);

        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn two_pending_futures_resume_in_ready_order() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    unsafe {
        ori_executor_drain();
    }
    TEST_EXECUTOR_CALLBACKS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let first = alloc_pending_future();
        let second = alloc_pending_future();
        ori_future_on_ready(first, test_executor_closure_object());
        ori_future_on_ready(second, test_executor_closure_object());

        assert_eq!(ori_executor_run_one(), 0);

        ori_future_complete_void(second);
        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_executor_run_one(), 0);

        ori_future_complete_void(first);
        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(ori_executor_run_one(), 0);

        ori_arc_release(first as *mut u8);
        ori_arc_release(second as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn sleep_future_can_be_blocked_on() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let future = ori_task_sleep(25);
        assert!(!future.is_null());
        assert_eq!(ori_future_poll(future), 0);
        assert_eq!(ori_task_block_on(future), 0);
        assert_eq!(ori_future_poll(future), 1);
        ori_arc_release(future as *mut u8);
    }
}

#[test]
fn ready_futures_preserve_scalar_float_and_pointer_payloads() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let int_future = ori_future_ready_i64(42);
        assert_eq!(ori_task_block_on(int_future), 42);
        ori_arc_release(int_future as *mut u8);

        let float_future = ori_future_ready_f64(3.5);
        assert_eq!(ori_task_block_on_f64(float_future), 3.5);
        ori_arc_release(float_future as *mut u8);

        let payload = ori_alloc(1, None);
        let ptr_future = ori_future_ready_ptr(payload);
        assert_eq!(ori_task_block_on_ptr(ptr_future), payload);
        ori_arc_release(ptr_future as *mut u8);
        ori_arc_release(payload);
    }
}

#[test]
fn async_spawn_i64_completes_future_from_native_closure() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let future = ori_async_spawn_i64(test_closure_object());
        assert!(!future.is_null());

        assert_eq!(ori_task_block_on(future), 41);

        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn async_spawn_i64_runs_on_executor_without_running_immediately() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();
    unsafe {
        ori_executor_drain();
    }
    TEST_EXECUTOR_CALLBACKS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let future = ori_async_spawn_i64(test_counting_async_closure_object());
        assert!(!future.is_null());
        assert_eq!(ori_future_poll(future), 0);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 0);

        assert_eq!(ori_executor_run_one(), 1);
        assert_eq!(TEST_EXECUTOR_CALLBACKS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_future_poll(future), 1);
        assert_eq!(ori_task_block_on(future), 123);
        assert_eq!(ori_arc_live_allocations(), 1);

        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn async_spawn_i64_propagates_failed_await_status() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let future = ori_async_spawn_i64(test_failed_await_closure_object());
        assert!(!future.is_null());

        assert_eq!(ori_task_block_on(future), 0);
        assert_eq!(ori_task_last_await_status(), 2);

        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn async_spawn_i64_propagates_cancelled_await_status() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    arc_state().lock().unwrap().allocations.clear();
    arc_state().lock().unwrap().edges.clear();

    unsafe {
        let future = ori_async_spawn_i64(test_cancelled_await_closure_object());
        assert!(!future.is_null());

        assert_eq!(ori_task_block_on(future), 0);
        assert_eq!(ori_task_last_await_status(), 3);

        ori_arc_release(future as *mut u8);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

#[test]
fn rust_runtime_exports_manifest_native_symbols() {
    let source = include_str!("lib.rs");
    let test_harness = include_str!("test_harness.rs");
    let combined = format!("{source}\n{test_harness}");
    let mut checked = HashSet::new();
    let mut missing = Vec::new();
    for entry in stdlib_runtime_functions()
        .iter()
        .filter(|entry| entry.native_runtime)
    {
        if checked.insert(entry.runtime_symbol) {
            let needle = format!("fn {}", entry.runtime_symbol);
            if !combined.contains(&needle) {
                missing.push(entry.runtime_symbol);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "manifest runtime symbols missing from Rust runtime: {missing:#?}"
    );
}

/// ABI layout guard (LANG-MEM-0): generated code and the spec (19-abi)
/// assume this exact header shape in front of every managed payload.
#[test]
fn ori_heap_header_layout_is_stable() {
    assert_eq!(std::mem::size_of::<OriHeapHeader>(), 16);
    assert_eq!(std::mem::align_of::<OriHeapHeader>(), 8);
    assert_eq!(std::mem::offset_of!(OriHeapHeader, refcount), 0);
    assert_eq!(std::mem::offset_of!(OriHeapHeader, destructor), 8);
}

// ── C3 — suspect buffer + partial trial deletion ────────────────────────────

fn reset_arc_state_for_test() {
    let mut state = arc_state().lock().unwrap();
    state.allocations.clear();
    state.edges.clear();
    state.suspects.clear();
    TEST_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);
}

/// Releasing into a cycle records a suspect; the partial pass reclaims the
/// cycle without scanning unrelated allocations.
#[test]
fn partial_collect_reclaims_suspect_cycle() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        // Unrelated live allocations must not be touched (and must not be
        // candidates: no suspects point at them).
        let bystander = ori_alloc(8, Some(test_destructor));
        assert!(!bystander.is_null());

        let left = ori_alloc(8, Some(test_destructor));
        let right = ori_alloc(8, Some(test_destructor));
        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);
        ori_arc_release(left);
        ori_arc_release(right);

        assert!(
            !arc_state().lock().unwrap().suspects.is_empty(),
            "releases into the cycle must record suspects"
        );

        let (freed, touched) = collect_cycles_from_suspects();
        assert_eq!(freed, 2, "the orphan cycle is reclaimed");
        assert_eq!(touched, 2, "only the cycle subgraph is examined");
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 2);
        assert!(arc_state().lock().unwrap().suspects.is_empty());

        ori_arc_release(bystander);
    }
}

/// Objects without outgoing edges can never be cycle members: their releases
/// must not create suspects and the partial pass must be a no-op.
#[test]
fn partial_collect_no_suspects_is_noop() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let a = ori_alloc(8, None);
        let b = ori_alloc(8, None);
        ori_arc_retain(a);
        ori_arc_release(a); // 2 -> 1, no outgoing edges: not a suspect

        assert!(arc_state().lock().unwrap().suspects.is_empty());
        let (freed, touched) = collect_cycles_from_suspects();
        assert_eq!((freed, touched), (0, 0));

        ori_arc_release(a);
        ori_arc_release(b);
    }
}

/// A cycle that is still externally referenced survives the partial pass and
/// is reclaimed by a later pass once the external reference is dropped.
#[test]
fn partial_collect_keeps_externally_referenced_cycle() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let left = ori_alloc(8, Some(test_destructor));
        let right = ori_alloc(8, Some(test_destructor));
        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);
        // Keep one extra external reference on `left`.
        ori_arc_retain(left);

        // Drop the allocation references: left 3 -> 2, right 2 -> 1.
        ori_arc_release(left);
        ori_arc_release(right);

        let (freed, touched) = collect_cycles_from_suspects();
        assert_eq!(freed, 0, "externally referenced cycle must survive");
        assert_eq!(touched, 2);
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 0);

        // Drop the last external reference: left 2 -> 1 records a suspect.
        ori_arc_release(left);
        let (freed, _) = collect_cycles_from_suspects();
        assert_eq!(freed, 2, "cycle reclaimed after external ref is gone");
        assert_eq!(TEST_DTOR_CALLS.load(AtomicOrdering::SeqCst), 2);
    }
}

/// An object freed through the normal zero path leaves no stale suspect
/// behind (O(1) swap-remove bookkeeping).
#[test]
fn suspect_slot_cleared_when_object_freed_normally() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();

    unsafe {
        let owner = ori_alloc(8, None);
        let child = ori_alloc(8, None);
        ori_arc_register_edge(owner, child);
        ori_arc_retain(owner);
        ori_arc_release(owner); // 2 -> 1 with outgoing edge: suspect
        assert_eq!(arc_state().lock().unwrap().suspects.len(), 1);

        ori_arc_release(owner); // 1 -> 0: freed; suspect slot must go too
        assert!(arc_state().lock().unwrap().suspects.is_empty());
        let (freed, touched) = collect_cycles_from_suspects();
        assert_eq!((freed, touched), (0, 0));
    }
}

// ── Plan F1 gap closure — destructor reentrancy (Nim bug #22927 analog) ─────

static REENTRANT_DTOR_CALLS: AtomicUsize = AtomicUsize::new(0);

/// A destructor that re-enters the ARC runtime: allocates a fresh managed
/// object, registers an edge on it, then releases it. Mirrors what a
/// runtime-internal destructor could legitimately do (e.g. logging into a
/// managed buffer while closing a resource).
unsafe extern "C" fn reentrant_destructor(_ptr: *mut u8) {
    REENTRANT_DTOR_CALLS.fetch_add(1, AtomicOrdering::SeqCst);
    let scratch = ori_alloc(16, None);
    assert!(!scratch.is_null());
    let child = ori_alloc(8, None);
    assert!(!child.is_null());
    ori_arc_register_edge(scratch, child);
    ori_arc_release(child);
    ori_arc_release(scratch);
}

/// Destructors run without the global ARC lock held: a dtor that allocates,
/// registers edges and releases must neither deadlock nor corrupt the
/// registry, both on the plain release-to-zero path...
#[test]
fn destructor_reentering_arc_runtime_on_release_is_safe() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();
    REENTRANT_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let obj = ori_alloc(8, Some(reentrant_destructor));
        assert!(!obj.is_null());
        ori_arc_release(obj);
        assert_eq!(REENTRANT_DTOR_CALLS.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}

/// ...and on the cycle-collector path, where the state lock is dropped
/// before destructors run and freshly registered suspects/allocations from
/// a dtor must not corrupt the pass (the Nim analog protected `roots` with
/// a length reset during frees — bug #22927).
#[test]
fn destructor_reentering_arc_runtime_during_cycle_collect_is_safe() {
    let _guard = TEST_ARC_LOCK.lock().unwrap();
    reset_arc_state_for_test();
    REENTRANT_DTOR_CALLS.store(0, AtomicOrdering::SeqCst);

    unsafe {
        let left = ori_alloc(8, Some(reentrant_destructor));
        let right = ori_alloc(8, Some(reentrant_destructor));
        ori_arc_register_edge(left, right);
        ori_arc_register_edge(right, left);
        ori_arc_release(left);
        ori_arc_release(right);

        assert_eq!(ori_arc_collect_cycles(), 2);
        assert_eq!(REENTRANT_DTOR_CALLS.load(AtomicOrdering::SeqCst), 2);
        assert_eq!(ori_arc_live_allocations(), 0);

        // Partial (suspect-driven) pass with reentrant dtors as well.
        let a = ori_alloc(8, Some(reentrant_destructor));
        let b = ori_alloc(8, Some(reentrant_destructor));
        ori_arc_register_edge(a, b);
        ori_arc_register_edge(b, a);
        ori_arc_release(a);
        ori_arc_release(b);
        let (freed, _) = collect_cycles_from_suspects();
        assert_eq!(freed, 2);
        assert_eq!(REENTRANT_DTOR_CALLS.load(AtomicOrdering::SeqCst), 4);
        assert_eq!(ori_arc_live_allocations(), 0);
    }
}
