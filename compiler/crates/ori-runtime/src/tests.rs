use super::*;
use ori_types::stdlib::stdlib_runtime_functions;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Mutex;

static TEST_DTOR_CALLS: AtomicUsize = AtomicUsize::new(0);
static TEST_EXECUTOR_CALLBACKS: AtomicUsize = AtomicUsize::new(0);
static TEST_ARC_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" fn test_destructor(_ptr: *mut u8) {
    TEST_DTOR_CALLS.fetch_add(1, AtomicOrdering::SeqCst);
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

unsafe fn test_closure_object() -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    *(closure as *mut usize) = test_task_entry as *const () as usize;
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
        let text = cstring_from_str("ori");
        assert_eq!(ori_len(text), 3);
        assert_eq!(cstr_bytes(text), b"ori");
        assert_eq!(*text.add(3), 0);
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
    let left = &*(left as *const TestScore);
    let right = &*(right as *const TestScore);
    left.value - right.value
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
        ori_map_register_borrowed_key_value_maybe_managed(map, key as i64, value as i64);
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
        ori_map_register_borrowed_key_value_maybe_managed(map, key as i64, old_value as i64);
        ori_arc_release(key);
        ori_arc_release(old_value);
        assert_eq!(ori_arc_live_allocations(), 3);

        let new_value = cstring_from_str("new");
        ori_map_set_string(map, key, new_value as i64);
        ori_map_register_borrowed_key_value_maybe_managed(map, key as i64, new_value as i64);
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
