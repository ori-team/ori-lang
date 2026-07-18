// ori-runtime implementation

pub const ORI_ABI_VERSION: &str = "ori-native-abi-1";

use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CStr;
use std::io::Write;
use std::os::raw::{c_char, c_uchar};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ── Atomic ARC ──────────────────────────────────────────────────────────────
//
// Every heap-allocated managed object starts with a 16-byte OriHeapHeader:
//   [refcount: AtomicI64][destructor: Option<fn(*mut u8)>][... payload ...]
//                                                          ^── ptr passed to
//                                                              retain/release
//
// The destructor hook is reserved for runtime-internal cleanup (e.g. a
// list's element storage). Compiler-registered ARC edges are the single
// owner of stored managed children (see docs/planning/
// adr-arc-single-cascade-owner.md); a hook must never release them.
// Layout guard: `ori_heap_header_layout_is_stable` in tests.rs.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

#[repr(C)]
pub struct OriHeapHeader {
    pub refcount: AtomicI64,
    pub destructor: Option<unsafe extern "C" fn(*mut u8)>,
}

#[derive(Clone, Copy)]
struct ArcAllocation {
    header: usize,
    size: usize,
    /// Index into `ArcState::suspects`, or `NOT_SUSPECT`. Gives O(1)
    /// dedupe/removal of cycle-root candidates (Nim ORC keeps the same idea
    /// in the header's `rootIdx`).
    suspect_idx: usize,
}

const NOT_SUSPECT: usize = usize::MAX;

/// Ownership edges indexed both ways so edge registration/removal and the bulk
/// lookups on free (`take_owned_edges`, `remove_incoming_edges`) cost
/// O(degree) instead of a scan over every edge in the process.
#[derive(Default)]
struct ArcEdges {
    by_owner: HashMap<usize, Vec<usize>>,
    by_child: HashMap<usize, Vec<usize>>,
}

impl ArcEdges {
    #[cfg(test)]
    fn clear(&mut self) {
        self.by_owner.clear();
        self.by_child.clear();
    }

    fn contains(&self, owner: usize, child: usize) -> bool {
        self.by_owner
            .get(&owner)
            .is_some_and(|children| children.contains(&child))
    }

    fn insert(&mut self, owner: usize, child: usize) {
        self.by_owner.entry(owner).or_default().push(child);
        self.by_child.entry(child).or_default().push(owner);
    }

    fn remove(&mut self, owner: usize, child: usize) -> bool {
        if !remove_one_edge_entry(&mut self.by_owner, owner, child) {
            return false;
        }
        remove_one_edge_entry(&mut self.by_child, child, owner);
        true
    }

    /// Removes and returns every child owned by `owner`.
    fn take_children_of(&mut self, owner: usize) -> Vec<usize> {
        let children = self.by_owner.remove(&owner).unwrap_or_default();
        for &child in &children {
            remove_one_edge_entry(&mut self.by_child, child, owner);
        }
        children
    }

    /// Drops every edge whose child is `child` (owners keep no dangling entry).
    fn remove_edges_to(&mut self, child: usize) {
        for owner in self.by_child.remove(&child).unwrap_or_default() {
            remove_one_edge_entry(&mut self.by_owner, owner, child);
        }
    }
}

fn remove_one_edge_entry(index: &mut HashMap<usize, Vec<usize>>, key: usize, value: usize) -> bool {
    let Some(values) = index.get_mut(&key) else {
        return false;
    };
    let Some(position) = values.iter().position(|v| *v == value) else {
        return false;
    };
    values.swap_remove(position);
    if values.is_empty() {
        index.remove(&key);
    }
    true
}

#[derive(Default)]
struct ArcState {
    /// Live managed allocations keyed by payload address. Keyed lookup keeps
    /// `ori_arc_retain`/`ori_arc_release` O(1); the previous linear registry
    /// made every retain/release O(live allocations) (LANG-PERF-3).
    allocations: HashMap<usize, ArcAllocation>,
    edges: ArcEdges,
    /// Possible cycle roots (Bacon trial deletion): payloads whose refcount
    /// was decremented to a non-zero value while they owned outgoing edges.
    /// Only these subgraphs can contain newly unreachable cycles.
    suspects: Vec<usize>,
}

impl ArcState {
    fn mark_suspect(&mut self, payload: usize) {
        let Some(allocation) = self.allocations.get_mut(&payload) else {
            return;
        };
        if allocation.suspect_idx != NOT_SUSPECT {
            return;
        }
        allocation.suspect_idx = self.suspects.len();
        self.suspects.push(payload);
    }

    /// Drops `payload` from the suspect buffer in O(1) (swap-remove).
    fn clear_suspect(&mut self, payload: usize) {
        let Some(allocation) = self.allocations.get_mut(&payload) else {
            return;
        };
        let idx = allocation.suspect_idx;
        if idx == NOT_SUSPECT {
            return;
        }
        allocation.suspect_idx = NOT_SUSPECT;
        self.suspects.swap_remove(idx);
        if let Some(&moved) = self.suspects.get(idx) {
            if let Some(moved_alloc) = self.allocations.get_mut(&moved) {
                moved_alloc.suspect_idx = idx;
            }
        }
    }

    fn remove_allocation(&mut self, payload: usize) -> Option<ArcAllocation> {
        self.clear_suspect(payload);
        self.allocations.remove(&payload)
    }
}

static ARC_STATE: OnceLock<Mutex<ArcState>> = OnceLock::new();

/// Total managed allocations since process start. Incremented in `ori_alloc`
/// without holding `ARC_STATE` so the fast path stays lock-free. Used by the
/// async executor to trigger cooperative cycle collection at safe points.
static COOPERATIVE_ALLOC_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Counter value at the last cooperative collection. The executor collects when
/// `COOPERATIVE_ALLOC_COUNTER - COOPERATIVE_LAST_COLLECTED` crosses the threshold.
static COOPERATIVE_LAST_COLLECTED: AtomicU64 = AtomicU64::new(0);

/// Number of managed allocations between cooperative cycle collections.
/// Adapted at runtime by pass efficacy (see `adapt_cooperative_threshold`)
/// unless `ORI_COOPERATIVE_COLLECT_THRESHOLD` pins it (tests).
const DEFAULT_COOPERATIVE_COLLECT_THRESHOLD: u64 = 256;
const MIN_COOPERATIVE_COLLECT_THRESHOLD: u64 = 64;
const MAX_COOPERATIVE_COLLECT_THRESHOLD: u64 = 65_536;

/// Current adaptive threshold (0 = not yet initialized).
static COOPERATIVE_THRESHOLD: AtomicU64 = AtomicU64::new(0);

/// Returns `(base_threshold, pinned_by_env)`.
fn cooperative_threshold_config() -> (u64, bool) {
    static CONFIG: OnceLock<(u64, bool)> = OnceLock::new();
    *CONFIG.get_or_init(|| {
        match std::env::var("ORI_COOPERATIVE_COLLECT_THRESHOLD")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|value| *value > 0)
        {
            Some(pinned) => (pinned, true),
            None => (DEFAULT_COOPERATIVE_COLLECT_THRESHOLD, false),
        }
    })
}

fn cooperative_collect_threshold() -> u64 {
    let (base, _) = cooperative_threshold_config();
    let current = COOPERATIVE_THRESHOLD.load(Ordering::Relaxed);
    if current == 0 {
        COOPERATIVE_THRESHOLD.store(base, Ordering::Relaxed);
        base
    } else {
        current
    }
}

/// Nim-ORC-style feedback: an effective pass (freed >= half of touched)
/// shrinks the window so cycles are reclaimed sooner; an ineffective pass
/// grows it so quiet workloads stop paying for scans.
fn adapt_cooperative_threshold(freed: i64, touched: usize) {
    let (_, pinned) = cooperative_threshold_config();
    if pinned || touched == 0 {
        return;
    }
    let current = cooperative_collect_threshold();
    let next = if freed.saturating_mul(2) >= touched as i64 {
        (current * 2 / 3).max(MIN_COOPERATIVE_COLLECT_THRESHOLD)
    } else {
        (current + current / 2).min(MAX_COOPERATIVE_COLLECT_THRESHOLD)
    };
    COOPERATIVE_THRESHOLD.store(next, Ordering::Relaxed);
}

/// Trigger cycle collection when the allocation counter has advanced past the
/// threshold since the last collection. Called at executor safe points (after a
/// task batch in `ori_task_block_on` and at the end of `ori_executor_drain`) and
/// from generated function-root cleanups via `ori_arc_maybe_collect_cycles`.
/// Uses `compare_exchange` so only one thread pays the collection cost per
/// window; `ori_arc_collect_cycles` itself is safe to call concurrently because
/// it locks `ARC_STATE`.
fn maybe_collect_cycles_cooperative() {
    let threshold = cooperative_collect_threshold();
    let counter = COOPERATIVE_ALLOC_COUNTER.load(Ordering::Acquire);
    let last = COOPERATIVE_LAST_COLLECTED.load(Ordering::Acquire);
    if counter.wrapping_sub(last) < threshold {
        return;
    }
    if COOPERATIVE_LAST_COLLECTED
        .compare_exchange(last, counter, Ordering::SeqCst, Ordering::Relaxed)
        .is_ok()
    {
        // Partial pass over suspect subgraphs only: cooperative safe points
        // no longer pay O(live heap). Explicit `ori_arc_collect_cycles` /
        // `ori.test.collect_cycles` keep full-scan semantics.
        let (freed, touched) = unsafe { collect_cycles_from_suspects() };
        adapt_cooperative_threshold(freed, touched);
    }
}

/// Amortized cycle-collection safe point for generated code and the executor.
///
/// Full trial-deletion is O(live allocations). Calling it on every function
/// return kept large-heap apps at ~2fps (LANG-PERF-3 residual). Generated
/// function roots now call this instead: a full pass runs only when the
/// allocation counter has advanced by `ORI_COOPERATIVE_COLLECT_THRESHOLD`
/// (default 256) since the last pass. Explicit `ori_arc_collect_cycles` and
/// `ori.test.collect_cycles` still force a full scan.
///
/// # Safety
///
/// Same invariants as `ori_arc_collect_cycles` when a pass actually runs.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_maybe_collect_cycles() {
    maybe_collect_cycles_cooperative();
}

thread_local! {
    static TASK_LAST_AWAIT_STATUS: Cell<i64> = const { Cell::new(1) };
}

fn arc_state() -> &'static Mutex<ArcState> {
    ARC_STATE.get_or_init(|| Mutex::new(ArcState::default()))
}

unsafe fn header_for_registered(ptr: *mut u8) -> Option<*mut OriHeapHeader> {
    if ptr.is_null() {
        return None;
    }
    let payload = ptr as usize;
    let state = arc_state().lock().unwrap_or_else(|e| e.into_inner());
    state
        .allocations
        .get(&payload)
        .map(|allocation| allocation.header as *mut OriHeapHeader)
}

unsafe fn register_allocation(header: *mut OriHeapHeader, payload: *mut u8, size: usize) {
    if let Ok(mut state) = arc_state().lock() {
        state.allocations.insert(
            payload as usize,
            ArcAllocation {
                header: header as usize,
                size,
                suspect_idx: NOT_SUSPECT,
            },
        );
    }
}

unsafe fn registered_payload_size(ptr: *const u8) -> Option<usize> {
    if ptr.is_null() {
        return None;
    }
    let payload = ptr as usize;
    let state = arc_state().lock().ok()?;
    state
        .allocations
        .get(&payload)
        .map(|allocation| allocation.size)
}

unsafe fn unregister_allocation(ptr: *mut u8) -> Option<*mut OriHeapHeader> {
    let payload = ptr as usize;
    let mut state = arc_state().lock().ok()?;
    state
        .remove_allocation(payload)
        .map(|allocation| allocation.header as *mut OriHeapHeader)
}

unsafe fn remove_incoming_edges(ptr: *mut u8) {
    if let Ok(mut state) = arc_state().lock() {
        state.edges.remove_edges_to(ptr as usize);
    }
}

unsafe fn take_owned_edges(ptr: *mut u8) -> Vec<*mut u8> {
    let owner = ptr as usize;
    if let Ok(mut state) = arc_state().lock() {
        state
            .edges
            .take_children_of(owner)
            .into_iter()
            .map(|child| child as *mut u8)
            .collect()
    } else {
        Vec::new()
    }
}

unsafe fn free_registered_object(ptr: *mut u8, release_owned_edges: bool) {
    let Some(header) = unregister_allocation(ptr) else {
        return;
    };
    let children = if release_owned_edges {
        take_owned_edges(ptr)
    } else {
        Vec::new()
    };
    remove_incoming_edges(ptr);
    if let Some(dtor) = (*header).destructor {
        dtor(ptr);
    }
    std::ptr::drop_in_place(&mut (*header).refcount);
    libc::free(header as *mut libc::c_void);
    for child in children {
        ori_arc_release(child);
    }
}

/// Allocates one managed runtime object and returns a pointer to its payload.
///
/// The returned payload starts with a reference count of 1. If `destructor` is
/// present, it is called once with the payload pointer before the allocation is
/// freed.
///
/// # Safety
///
/// `size` must be the exact payload size expected by all later reads and writes.
/// `destructor`, when provided, must accept the same payload layout and must not
/// free the allocation itself. The returned pointer must be released with
/// `ori_arc_release` when the owner is done with it.
#[no_mangle]
pub unsafe extern "C" fn ori_alloc(
    size: usize,
    destructor: Option<unsafe extern "C" fn(*mut u8)>,
) -> *mut u8 {
    let total = size + std::mem::size_of::<OriHeapHeader>();
    let ptr = libc::malloc(total) as *mut u8;
    if !ptr.is_null() {
        let header = ptr as *mut OriHeapHeader;
        std::ptr::write(&mut (*header).refcount, AtomicI64::new(1));
        (*header).destructor = destructor;
        let payload = ptr.add(std::mem::size_of::<OriHeapHeader>());
        register_allocation(header, payload, size);
        COOPERATIVE_ALLOC_COUNTER.fetch_add(1, Ordering::Relaxed);
        payload
    } else {
        ptr
    }
}

/// Increment the reference count of a managed object.
/// Silently ignores null pointers and non-managed values (e.g. static strings).
///
/// # Safety
///
/// `ptr` may be null or a non-managed runtime value. When it is managed, it must
/// be a live payload pointer previously returned by `ori_alloc` or another
/// runtime constructor that uses `ori_alloc`.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_retain(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let Some(header) = header_for_registered(ptr) else {
        return;
    };
    (*header).refcount.fetch_add(1, Ordering::Relaxed);
}

/// Decrement the reference count. When it reaches zero, the object is freed.
///
/// # Safety
///
/// `ptr` may be null or a non-managed runtime value. When it is managed, it must
/// be a live payload pointer. Each owning reference must be released exactly once;
/// using `ptr` after the final release is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_release(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let payload = ptr as usize;
    let old = {
        let mut state = arc_state().lock().unwrap_or_else(|e| e.into_inner());
        let Some(allocation) = state.allocations.get(&payload) else {
            return;
        };
        let header = allocation.header as *mut OriHeapHeader;
        let old = (*header).refcount.fetch_sub(1, Ordering::Release);
        if old != 1 && state.edges.by_owner.contains_key(&payload) {
            // Bacon possible root: a refcount decremented to non-zero on an
            // object with outgoing edges may have disconnected a cycle.
            state.mark_suspect(payload);
        }
        old
    };
    if old == 1 {
        let Some(header) = header_for_registered(ptr) else {
            return;
        };
        (*header).refcount.load(Ordering::Acquire); // synchronize
        free_registered_object(ptr, true);
    }
}

/// Registers that `owner` holds a managed reference to `child`.
///
/// The runtime retains `child` while the edge is registered. Duplicate edges are
/// ignored.
///
/// # Safety
///
/// `owner` and `child` may be null, but non-null managed values must be live
/// payload pointers. Callers must unregister or update the edge before replacing,
/// removing, or freeing the slot that owns the child reference.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_register_edge(owner: *mut u8, child: *mut u8) {
    if owner.is_null() || child.is_null() {
        return;
    }
    let owner_key = owner as usize;
    let child_key = child as usize;
    let Some(child_header) = header_for_registered(child) else {
        return;
    };
    if header_for_registered(owner).is_none() {
        return;
    }
    if let Ok(mut state) = arc_state().lock() {
        if state.edges.contains(owner_key, child_key) {
            return;
        }
        state.edges.insert(owner_key, child_key);
        (*child_header).refcount.fetch_add(1, Ordering::Relaxed);
    }
}

// -- Native concurrency -----------------------------------------------------

#[repr(C)]
pub struct OriTaskJob {
    handle: Mutex<Option<std::thread::JoinHandle<i64>>>,
}

#[repr(C)]
pub struct OriChannel {
    state: Mutex<OriChannelState>,
    available: Condvar,
}

struct OriChannelState {
    queue: VecDeque<i64>,
    closed: bool,
}

#[repr(C)]
pub struct OriAtomicInt {
    value: AtomicI64,
}

#[repr(C)]
pub struct OriFuture {
    state: Mutex<OriFutureState>,
    ready: Condvar,
}

#[repr(C)]
struct OriAsyncSpawnJob {
    runner: unsafe fn(usize, usize, usize),
    closure: usize,
    future: usize,
}

#[repr(C)]
pub struct RuntimeCancelToken {
    cancelled: std::sync::atomic::AtomicBool,
    associated_futures: Mutex<Vec<*mut OriFuture>>,
}

struct OriFutureState {
    status: OriFutureStatus,
    value: i64,
    value_is_managed: bool,
    waiters: VecDeque<usize>,
    cancel_token: *mut u8,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OriFutureStatus {
    Pending,
    Ready,
    Failed,
    Cancelled,
}

struct OriExecutor {
    queue: Mutex<VecDeque<usize>>,
    available: Condvar,
}

struct OriTimerState {
    timers: Vec<TimerEntry>,
}

#[derive(Clone, Copy)]
struct TimerEntry {
    due: Instant,
    future: usize,
}

static EXECUTOR: OnceLock<OriExecutor> = OnceLock::new();
static TIMER_STATE: OnceLock<(Mutex<OriTimerState>, Condvar)> = OnceLock::new();
static TIMER_THREAD: OnceLock<()> = OnceLock::new();

fn executor() -> &'static OriExecutor {
    EXECUTOR.get_or_init(|| OriExecutor {
        queue: Mutex::new(VecDeque::new()),
        available: Condvar::new(),
    })
}

fn timer_state() -> &'static (Mutex<OriTimerState>, Condvar) {
    TIMER_STATE.get_or_init(|| {
        (
            Mutex::new(OriTimerState { timers: Vec::new() }),
            Condvar::new(),
        )
    })
}

fn ensure_timer_thread() {
    TIMER_THREAD.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("ori-runtime-timer".to_string())
            .spawn(timer_loop);
    });
}

fn timer_loop() {
    loop {
        let ready = {
            let (lock, cvar) = timer_state();
            let mut guard = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            loop {
                if guard.timers.is_empty() {
                    guard = match cvar.wait(guard) {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    continue;
                }

                guard.timers.sort_by_key(|entry| entry.due);
                let now = Instant::now();
                let next_due = guard.timers[0].due;
                if next_due <= now {
                    let mut ready = Vec::new();
                    let mut index = 0;
                    while index < guard.timers.len() {
                        if guard.timers[index].due <= now {
                            ready.push(guard.timers.swap_remove(index));
                        } else {
                            index += 1;
                        }
                    }
                    break ready;
                }

                let wait = next_due.saturating_duration_since(now);
                guard = match cvar.wait_timeout(guard, wait) {
                    Ok((guard, _)) => guard,
                    Err(poisoned) => poisoned.into_inner().0,
                };
            }
        };

        for entry in ready {
            unsafe {
                complete_future_owned(entry.future as *mut OriFuture, OriFutureStatus::Ready, 0);
            }
        }
    }
}

unsafe fn schedule_executor_task_owned(closure_addr: usize) {
    if closure_addr == 0 {
        return;
    }
    let exec = executor();
    let mut guard = match exec.queue.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.push_back(closure_addr);
    exec.available.notify_one();
}

unsafe fn pop_executor_task() -> Option<usize> {
    let exec = executor();
    let mut guard = match exec.queue.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.pop_front()
}

unsafe fn run_executor_task(closure_addr: usize) {
    if closure_addr == 0 {
        return;
    }
    let closure = closure_addr as *mut u8;
    let ptr_size = std::mem::size_of::<*mut u8>();
    let fn_addr = *(closure as *const usize);
    let env_addr = *(closure.add(ptr_size) as *const usize);
    type TaskFn = unsafe extern "C" fn(*mut u8) -> i64;
    let task_fn: TaskFn = std::mem::transmute(fn_addr);
    let _ = task_fn(env_addr as *mut u8);
    ori_arc_release(closure);
}

unsafe fn schedule_sleep_timer(future: *mut OriFuture, ms: i64) {
    if future.is_null() {
        return;
    }
    ensure_timer_thread();
    let duration = Duration::from_millis(ms.max(0) as u64);
    let now = Instant::now();
    let due = now.checked_add(duration).unwrap_or(now);
    let (lock, cvar) = timer_state();
    let mut guard = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.timers.push(TimerEntry {
        due,
        future: future as usize,
    });
    cvar.notify_one();
}

unsafe extern "C" fn ori_task_job_dtor(ptr: *mut u8) {
    std::ptr::drop_in_place(ptr as *mut OriTaskJob);
}

unsafe extern "C" fn ori_channel_dtor(ptr: *mut u8) {
    std::ptr::drop_in_place(ptr as *mut OriChannel);
}

unsafe extern "C" fn ori_atomic_int_dtor(ptr: *mut u8) {
    std::ptr::drop_in_place(ptr as *mut OriAtomicInt);
}

unsafe extern "C" fn ori_future_dtor(ptr: *mut u8) {
    let future = ptr as *mut OriFuture;
    let (waiters, managed_value, token) = match (*future).state.lock() {
        Ok(mut guard) => {
            let managed_value = if guard.status == OriFutureStatus::Ready && guard.value_is_managed
            {
                Some(guard.value as *mut u8)
            } else {
                None
            };
            let waiters = guard.waiters.drain(..).collect::<Vec<_>>();
            let token = guard.cancel_token;
            guard.cancel_token = std::ptr::null_mut();
            guard.value_is_managed = false;
            (waiters, managed_value, token)
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            let managed_value = if guard.status == OriFutureStatus::Ready && guard.value_is_managed
            {
                Some(guard.value as *mut u8)
            } else {
                None
            };
            let waiters = guard.waiters.drain(..).collect::<Vec<_>>();
            let token = guard.cancel_token;
            guard.cancel_token = std::ptr::null_mut();
            guard.value_is_managed = false;
            (waiters, managed_value, token)
        }
    };
    if !token.is_null() {
        deregister_future_from_token(future, token);
    }
    for waiter in waiters {
        ori_arc_release(waiter as *mut u8);
    }
    if let Some(value) = managed_value {
        ori_arc_release(value);
    }
    std::ptr::drop_in_place(future);
}

unsafe extern "C" fn ori_async_spawn_job_dtor(ptr: *mut u8) {
    let job = ptr as *mut OriAsyncSpawnJob;
    let closure = (*job).closure;
    let future = (*job).future;
    (*job).closure = 0;
    (*job).future = 0;
    if closure != 0 {
        ori_arc_release(closure as *mut u8);
    }
    if future != 0 {
        ori_arc_release(future as *mut u8);
    }
    std::ptr::drop_in_place(job);
}

unsafe fn alloc_task_job(job: OriTaskJob) -> *mut OriTaskJob {
    let ptr =
        ori_alloc(std::mem::size_of::<OriTaskJob>(), Some(ori_task_job_dtor)) as *mut OriTaskJob;
    if !ptr.is_null() {
        std::ptr::write(ptr, job);
    }
    ptr
}

unsafe fn alloc_channel(channel: OriChannel) -> *mut OriChannel {
    let ptr =
        ori_alloc(std::mem::size_of::<OriChannel>(), Some(ori_channel_dtor)) as *mut OriChannel;
    if !ptr.is_null() {
        std::ptr::write(ptr, channel);
    }
    ptr
}

unsafe fn alloc_atomic_int(value: i64) -> *mut OriAtomicInt {
    let ptr = ori_alloc(
        std::mem::size_of::<OriAtomicInt>(),
        Some(ori_atomic_int_dtor),
    ) as *mut OriAtomicInt;
    if !ptr.is_null() {
        std::ptr::write(
            ptr,
            OriAtomicInt {
                value: AtomicI64::new(value),
            },
        );
    }
    ptr
}

unsafe fn alloc_future(future: OriFuture) -> *mut OriFuture {
    let ptr = ori_alloc(std::mem::size_of::<OriFuture>(), Some(ori_future_dtor)) as *mut OriFuture;
    if !ptr.is_null() {
        std::ptr::write(ptr, future);
    }
    ptr
}

unsafe fn alloc_ready_future(value: i64) -> *mut OriFuture {
    alloc_future(OriFuture {
        state: Mutex::new(OriFutureState {
            status: OriFutureStatus::Ready,
            value,
            value_is_managed: false,
            waiters: VecDeque::new(),
            cancel_token: std::ptr::null_mut(),
        }),
        ready: Condvar::new(),
    })
}

unsafe fn alloc_ready_future_ptr(value: *mut u8) -> *mut OriFuture {
    let value_is_managed = retain_registered_future_payload(value);
    alloc_future(OriFuture {
        state: Mutex::new(OriFutureState {
            status: OriFutureStatus::Ready,
            value: value as i64,
            value_is_managed,
            waiters: VecDeque::new(),
            cancel_token: std::ptr::null_mut(),
        }),
        ready: Condvar::new(),
    })
}

unsafe fn alloc_pending_future() -> *mut OriFuture {
    alloc_future(OriFuture {
        state: Mutex::new(OriFutureState {
            status: OriFutureStatus::Pending,
            value: 0,
            value_is_managed: false,
            waiters: VecDeque::new(),
            cancel_token: std::ptr::null_mut(),
        }),
        ready: Condvar::new(),
    })
}

unsafe fn retain_registered_future_payload(value: *mut u8) -> bool {
    if header_for_registered(value).is_none() {
        return false;
    }
    ori_arc_retain(value);
    true
}

fn future_status_code(status: OriFutureStatus) -> i64 {
    match status {
        OriFutureStatus::Pending => 0,
        OriFutureStatus::Ready => 1,
        OriFutureStatus::Failed => 2,
        OriFutureStatus::Cancelled => 3,
    }
}

fn set_task_last_await_status(status: OriFutureStatus) {
    let code = future_status_code(status);
    TASK_LAST_AWAIT_STATUS.with(|cell| cell.set(code));
}

fn take_task_last_await_status() -> i64 {
    TASK_LAST_AWAIT_STATUS.with(|cell| {
        let status = cell.get();
        cell.set(future_status_code(OriFutureStatus::Ready));
        status
    })
}

#[no_mangle]
pub extern "C" fn ori_task_last_await_status() -> i64 {
    TASK_LAST_AWAIT_STATUS.with(Cell::get)
}

unsafe fn set_future_status(
    future: *mut OriFuture,
    status: OriFutureStatus,
    value: i64,
    value_is_managed: bool,
) -> Vec<usize> {
    if future.is_null() {
        if value_is_managed {
            ori_arc_release(value as *mut u8);
        }
        return Vec::new();
    }
    let mut guard = match (*future).state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.status != OriFutureStatus::Pending {
        if value_is_managed {
            ori_arc_release(value as *mut u8);
        }
        return Vec::new();
    }
    let waiters = guard.waiters.drain(..).collect::<Vec<_>>();
    guard.status = status;
    guard.value = value;
    guard.value_is_managed = status == OriFutureStatus::Ready && value_is_managed;
    let token = guard.cancel_token;
    guard.cancel_token = std::ptr::null_mut();
    (*future).ready.notify_all();
    drop(guard);

    if !token.is_null() {
        deregister_future_from_token(future, token);
    }
    waiters
}

unsafe fn schedule_future_waiters(waiters: Vec<usize>) {
    for waiter in waiters {
        schedule_executor_task_owned(waiter);
    }
}

unsafe fn complete_future_owned(future: *mut OriFuture, status: OriFutureStatus, value: i64) {
    let waiters = set_future_status(future, status, value, false);
    schedule_future_waiters(waiters);
    ori_arc_release(future as *mut u8);
}

unsafe fn complete_future_owned_ptr(
    future: *mut OriFuture,
    status: OriFutureStatus,
    value: *mut u8,
) {
    let value_is_managed =
        status == OriFutureStatus::Ready && header_for_registered(value).is_some();
    let waiters = set_future_status(future, status, value as i64, value_is_managed);
    schedule_future_waiters(waiters);
    ori_arc_release(future as *mut u8);
}

unsafe extern "C" fn async_spawn_job_entry(env: *mut u8) -> i64 {
    if env.is_null() {
        return 0;
    }
    let job = env as *mut OriAsyncSpawnJob;
    let runner = (*job).runner;
    let closure = (*job).closure;
    let future = (*job).future;
    (*job).closure = 0;
    (*job).future = 0;
    if closure == 0 || future == 0 {
        if closure != 0 {
            ori_arc_release(closure as *mut u8);
        }
        if future != 0 {
            complete_future_owned(future as *mut OriFuture, OriFutureStatus::Failed, 0);
        }
        return 0;
    }
    runner(closure, future, 0);
    0
}

unsafe fn alloc_executor_closure(
    entry: unsafe extern "C" fn(*mut u8) -> i64,
    env: *mut u8,
) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let closure = ori_alloc(ptr_size * 2, None);
    if closure.is_null() {
        return closure;
    }
    *(closure as *mut usize) = entry as *const () as usize;
    *(closure.add(ptr_size) as *mut usize) = env as usize;
    if !env.is_null() {
        ori_arc_register_edge(closure, env);
        ori_arc_release(env);
    }
    closure
}

unsafe fn alloc_async_spawn_job(
    runner: unsafe fn(usize, usize, usize),
    closure: *mut u8,
    future: *mut OriFuture,
) -> *mut OriAsyncSpawnJob {
    let job = ori_alloc(
        std::mem::size_of::<OriAsyncSpawnJob>(),
        Some(ori_async_spawn_job_dtor),
    ) as *mut OriAsyncSpawnJob;
    if !job.is_null() {
        std::ptr::write(
            job,
            OriAsyncSpawnJob {
                runner,
                closure: closure as usize,
                future: future as usize,
            },
        );
    }
    job
}

unsafe fn new_result_raw(is_ok: bool, raw: i64) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let total = ptr_size * 2;
    let ptr = ori_alloc(total, None);
    if ptr.is_null() {
        return ptr;
    }
    std::ptr::write_bytes(ptr, 0, total);
    *ptr = u8::from(is_ok);
    std::ptr::write_unaligned(ptr.add(ptr_size) as *mut i64, raw);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_spawn(closure: *mut u8) -> *mut OriTaskJob {
    if closure.is_null() {
        return alloc_task_job(OriTaskJob {
            handle: Mutex::new(None),
        });
    }

    let ptr_size = std::mem::size_of::<*mut u8>();
    let fn_addr = *(closure as *const usize);
    let env_addr = *(closure.add(ptr_size) as *const usize);
    let closure_addr = closure as usize;
    let handle = std::thread::spawn(move || {
        type TaskFn = unsafe extern "C" fn(*mut u8) -> i64;
        let task_fn: TaskFn = unsafe { std::mem::transmute(fn_addr) };
        let value = unsafe { task_fn(env_addr as *mut u8) };
        unsafe {
            ori_arc_release(closure_addr as *mut u8);
        }
        value
    });

    alloc_task_job(OriTaskJob {
        handle: Mutex::new(Some(handle)),
    })
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_join(job: *mut OriTaskJob) -> *mut u8 {
    if job.is_null() {
        return new_result_raw(false, 0);
    }
    let handle = match (*job).handle.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    };
    let Some(handle) = handle else {
        return new_result_raw(false, 0);
    };
    match handle.join() {
        Ok(value) => new_result_raw(true, value),
        Err(_) => new_result_raw(false, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_detach(job: *mut OriTaskJob) {
    if job.is_null() {
        return;
    }
    match (*job).handle.lock() {
        Ok(mut guard) => {
            guard.take();
        }
        Err(poisoned) => {
            poisoned.into_inner().take();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_sleep(ms: i64) -> *mut OriFuture {
    let future = alloc_pending_future();
    if future.is_null() {
        return future;
    }

    ori_arc_retain(future as *mut u8);
    schedule_sleep_timer(future, ms);
    future
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_pending() -> *mut OriFuture {
    alloc_pending_future()
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_ready_i64(value: i64) -> *mut OriFuture {
    alloc_ready_future(value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_ready_f64(value: f64) -> *mut OriFuture {
    alloc_ready_future(value.to_bits() as i64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_ready_ptr(value: *mut u8) -> *mut OriFuture {
    alloc_ready_future_ptr(value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_ready_void() -> *mut OriFuture {
    alloc_ready_future(0)
}

unsafe fn async_spawn_with_runner(
    closure: *mut u8,
    runner: unsafe fn(usize, usize, usize),
) -> *mut OriFuture {
    let future = alloc_pending_future();
    if future.is_null() {
        if !closure.is_null() {
            ori_arc_release(closure);
        }
        return future;
    }
    if closure.is_null() {
        ori_future_cancel(future);
        return future;
    }

    ori_arc_retain(future as *mut u8);
    let job = alloc_async_spawn_job(runner, closure, future);
    if job.is_null() {
        ori_arc_release(closure);
        complete_future_owned(future, OriFutureStatus::Failed, 0);
        return future;
    }
    let executor_closure = alloc_executor_closure(async_spawn_job_entry, job as *mut u8);
    if executor_closure.is_null() {
        ori_future_fail(future);
        ori_arc_release(job as *mut u8);
        return future;
    }
    schedule_executor_task_owned(executor_closure as usize);
    future
}

unsafe fn complete_future_from_task_status(future: *mut OriFuture, ready_value: i64) {
    match take_task_last_await_status() {
        2 => complete_future_owned(future, OriFutureStatus::Failed, 0),
        3 => complete_future_owned(future, OriFutureStatus::Cancelled, 0),
        _ => complete_future_owned(future, OriFutureStatus::Ready, ready_value),
    }
}

unsafe fn complete_future_ptr_from_task_status(future: *mut OriFuture, ready_value: *mut u8) {
    match take_task_last_await_status() {
        2 => complete_future_owned(future, OriFutureStatus::Failed, 0),
        3 => complete_future_owned(future, OriFutureStatus::Cancelled, 0),
        _ => complete_future_owned_ptr(future, OriFutureStatus::Ready, ready_value),
    }
}

unsafe fn async_runner_i64(closure_addr: usize, future_addr: usize, _: usize) {
    let closure = closure_addr as *mut u8;
    let ptr_size = std::mem::size_of::<*mut u8>();
    let fn_addr = *(closure as *const usize);
    let env_addr = *(closure.add(ptr_size) as *const usize);
    type AsyncFn = unsafe extern "C" fn(*mut u8) -> i64;
    let async_fn: AsyncFn = std::mem::transmute(fn_addr);
    let value = async_fn(env_addr as *mut u8);
    ori_arc_release(closure);
    complete_future_from_task_status(future_addr as *mut OriFuture, value);
}

unsafe fn async_runner_f64(closure_addr: usize, future_addr: usize, _: usize) {
    let closure = closure_addr as *mut u8;
    let ptr_size = std::mem::size_of::<*mut u8>();
    let fn_addr = *(closure as *const usize);
    let env_addr = *(closure.add(ptr_size) as *const usize);
    type AsyncFn = unsafe extern "C" fn(*mut u8) -> f64;
    let async_fn: AsyncFn = std::mem::transmute(fn_addr);
    let value = async_fn(env_addr as *mut u8);
    ori_arc_release(closure);
    complete_future_from_task_status(future_addr as *mut OriFuture, value.to_bits() as i64);
}

unsafe fn async_runner_ptr(closure_addr: usize, future_addr: usize, _: usize) {
    let closure = closure_addr as *mut u8;
    let ptr_size = std::mem::size_of::<*mut u8>();
    let fn_addr = *(closure as *const usize);
    let env_addr = *(closure.add(ptr_size) as *const usize);
    type AsyncFn = unsafe extern "C" fn(*mut u8) -> *mut u8;
    let async_fn: AsyncFn = std::mem::transmute(fn_addr);
    let value = async_fn(env_addr as *mut u8);
    ori_arc_release(closure);
    complete_future_ptr_from_task_status(future_addr as *mut OriFuture, value);
}

unsafe fn async_runner_void(closure_addr: usize, future_addr: usize, _: usize) {
    let closure = closure_addr as *mut u8;
    let ptr_size = std::mem::size_of::<*mut u8>();
    let fn_addr = *(closure as *const usize);
    let env_addr = *(closure.add(ptr_size) as *const usize);
    type AsyncFn = unsafe extern "C" fn(*mut u8);
    let async_fn: AsyncFn = std::mem::transmute(fn_addr);
    async_fn(env_addr as *mut u8);
    ori_arc_release(closure);
    complete_future_from_task_status(future_addr as *mut OriFuture, 0);
}

#[no_mangle]
pub unsafe extern "C" fn ori_async_spawn_i64(closure: *mut u8) -> *mut OriFuture {
    async_spawn_with_runner(closure, async_runner_i64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_async_spawn_f64(closure: *mut u8) -> *mut OriFuture {
    async_spawn_with_runner(closure, async_runner_f64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_async_spawn_ptr(closure: *mut u8) -> *mut OriFuture {
    async_spawn_with_runner(closure, async_runner_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn ori_async_spawn_void(closure: *mut u8) -> *mut OriFuture {
    async_spawn_with_runner(closure, async_runner_void)
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_poll(future: *mut OriFuture) -> i64 {
    if future.is_null() {
        return future_status_code(OriFutureStatus::Cancelled);
    }
    let guard = match (*future).state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    future_status_code(guard.status)
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_value_i64(future: *mut OriFuture) -> i64 {
    if future.is_null() {
        return 0;
    }
    let guard = match (*future).state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.value
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_value_f64(future: *mut OriFuture) -> f64 {
    f64::from_bits(ori_future_value_i64(future) as u64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_value_ptr(future: *mut OriFuture) -> *mut u8 {
    ori_future_value_i64(future) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_on_ready(future: *mut OriFuture, closure: *mut u8) {
    if closure.is_null() {
        return;
    }
    if future.is_null() {
        ori_arc_release(closure);
        return;
    }

    let mut schedule_now = false;
    {
        let mut guard = match (*future).state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.status == OriFutureStatus::Pending {
            guard.waiters.push_back(closure as usize);
        } else {
            schedule_now = true;
        }
    }

    if schedule_now {
        schedule_executor_task_owned(closure as usize);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_complete_i64(future: *mut OriFuture, value: i64) {
    let waiters = set_future_status(future, OriFutureStatus::Ready, value, false);
    schedule_future_waiters(waiters);
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_complete_f64(future: *mut OriFuture, value: f64) {
    ori_future_complete_i64(future, value.to_bits() as i64);
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_complete_ptr(future: *mut OriFuture, value: *mut u8) {
    let value_is_managed = retain_registered_future_payload(value);
    let waiters = set_future_status(
        future,
        OriFutureStatus::Ready,
        value as i64,
        value_is_managed,
    );
    schedule_future_waiters(waiters);
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_complete_void(future: *mut OriFuture) {
    ori_future_complete_i64(future, 0);
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_fail(future: *mut OriFuture) {
    let waiters = set_future_status(future, OriFutureStatus::Failed, 0, false);
    schedule_future_waiters(waiters);
}

#[no_mangle]
pub unsafe extern "C" fn ori_future_cancel(future: *mut OriFuture) {
    let waiters = set_future_status(future, OriFutureStatus::Cancelled, 0, false);
    schedule_future_waiters(waiters);
}

unsafe fn deregister_future_from_token(future: *mut OriFuture, cancel_token: *mut u8) {
    if cancel_token.is_null() {
        return;
    }
    let token = cancel_token as *mut RuntimeCancelToken;
    let mut t_guard = match (*token).associated_futures.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(pos) = t_guard.iter().position(|&x| x == future) {
        t_guard.remove(pos);
        ori_arc_release(future as *mut u8);
    }
    drop(t_guard);
    ori_arc_release(cancel_token);
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_create_token() -> *mut u8 {
    let token = ori_alloc(
        std::mem::size_of::<RuntimeCancelToken>(),
        Some(ori_task_cancel_token_dtor),
    ) as *mut RuntimeCancelToken;
    if !token.is_null() {
        std::ptr::write(
            token,
            RuntimeCancelToken {
                cancelled: std::sync::atomic::AtomicBool::new(false),
                associated_futures: Mutex::new(Vec::new()),
            },
        );
    }
    token as *mut u8
}

unsafe extern "C" fn ori_task_cancel_token_dtor(ptr: *mut u8) {
    let token = ptr as *mut RuntimeCancelToken;
    std::ptr::drop_in_place(token);
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_cancel(token_ptr: *mut u8) {
    if token_ptr.is_null() {
        return;
    }
    let token = token_ptr as *mut RuntimeCancelToken;
    (*token).cancelled.store(true, Ordering::SeqCst);
    let futures = {
        let mut guard = match (*token).associated_futures.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        std::mem::take(&mut *guard)
    };
    for future in futures {
        ori_future_cancel(future);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_is_cancelled(token_ptr: *mut u8) -> i8 {
    if token_ptr.is_null() {
        return 0;
    }
    let token = token_ptr as *mut RuntimeCancelToken;
    if (*token).cancelled.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_associate(token_ptr: *mut u8, future_ptr: *mut u8) {
    if token_ptr.is_null() || future_ptr.is_null() {
        return;
    }
    let token = token_ptr as *mut RuntimeCancelToken;
    let future = future_ptr as *mut OriFuture;
    if (*token).cancelled.load(Ordering::SeqCst) {
        ori_future_cancel(future);
        return;
    }
    let mut f_guard = match (*future).state.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if f_guard.status != OriFutureStatus::Pending || !f_guard.cancel_token.is_null() {
        return;
    }
    f_guard.cancel_token = token_ptr;
    ori_arc_retain(token_ptr);
    let mut t_guard = match (*token).associated_futures.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    t_guard.push(future);
    ori_arc_retain(future as *mut u8);
}

#[no_mangle]
pub unsafe extern "C" fn ori_executor_schedule(closure: *mut u8) {
    schedule_executor_task_owned(closure as usize);
}

#[no_mangle]
pub unsafe extern "C" fn ori_executor_run_one() -> i64 {
    let Some(closure_addr) = pop_executor_task() else {
        return 0;
    };
    run_executor_task(closure_addr);
    1
}

#[no_mangle]
pub unsafe extern "C" fn ori_executor_drain() -> i64 {
    let mut ran = 0;
    while ori_executor_run_one() != 0 {
        ran += 1;
    }
    maybe_collect_cycles_cooperative();
    ran
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_block_on(future: *mut OriFuture) -> i64 {
    if future.is_null() {
        set_task_last_await_status(OriFutureStatus::Cancelled);
        return 0;
    }

    loop {
        {
            let guard = match (*future).state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.status {
                OriFutureStatus::Ready => {
                    set_task_last_await_status(OriFutureStatus::Ready);
                    return guard.value;
                }
                OriFutureStatus::Failed => {
                    set_task_last_await_status(OriFutureStatus::Failed);
                    return 0;
                }
                OriFutureStatus::Cancelled => {
                    set_task_last_await_status(OriFutureStatus::Cancelled);
                    return 0;
                }
                OriFutureStatus::Pending => {}
            }
        }

        while ori_executor_run_one() != 0 {}

        // Cooperative safe point: between task batches while the awaited future
        // is still pending. No Ori task is mid-flight on this thread, so managed
        // refcounts are quiescent and the trial-deletion scan sees a consistent
        // heap.
        maybe_collect_cycles_cooperative();

        let guard = match (*future).state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.status == OriFutureStatus::Pending {
            drop(
                match (*future)
                    .ready
                    .wait_timeout(guard, Duration::from_millis(1))
                {
                    Ok((guard, _)) => guard,
                    Err(poisoned) => poisoned.into_inner().0,
                },
            );
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_block_on_f64(future: *mut OriFuture) -> f64 {
    f64::from_bits(ori_task_block_on(future) as u64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_task_block_on_ptr(future: *mut OriFuture) -> *mut u8 {
    ori_task_block_on(future) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_channel_create() -> *mut OriChannel {
    alloc_channel(OriChannel {
        state: Mutex::new(OriChannelState {
            queue: VecDeque::new(),
            closed: false,
        }),
        available: Condvar::new(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn ori_channel_send(channel: *mut OriChannel, value: i64) -> *mut u8 {
    if channel.is_null() {
        return new_result_raw(false, 0);
    }
    let mut guard = match (*channel).state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.closed {
        return new_result_raw(false, 0);
    }
    guard.queue.push_back(value);
    (*channel).available.notify_one();
    new_result_raw(true, 0)
}

#[no_mangle]
pub unsafe extern "C" fn ori_channel_receive(channel: *mut OriChannel) -> *mut u8 {
    if channel.is_null() {
        return new_result_raw(false, 0);
    }
    let mut guard = match (*channel).state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    loop {
        if let Some(value) = guard.queue.pop_front() {
            return new_result_raw(true, value);
        }
        if guard.closed {
            return new_result_raw(false, 0);
        }
        guard = match (*channel).available.wait(guard) {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_channel_close(channel: *mut OriChannel) {
    if channel.is_null() {
        return;
    }
    let mut guard = match (*channel).state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.closed = true;
    (*channel).available.notify_all();
}

#[no_mangle]
pub unsafe extern "C" fn ori_atomic_new(value: i64) -> *mut OriAtomicInt {
    alloc_atomic_int(value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_atomic_load(value: *mut OriAtomicInt) -> i64 {
    if value.is_null() {
        return 0;
    }
    (*value).value.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn ori_atomic_store(value: *mut OriAtomicInt, next: i64) {
    if value.is_null() {
        return;
    }
    (*value).value.store(next, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn ori_atomic_add(value: *mut OriAtomicInt, delta: i64) -> i64 {
    if value.is_null() {
        return 0;
    }
    (*value).value.fetch_add(delta, Ordering::SeqCst) + delta
}

/// Removes one registered managed edge from `owner` to `child`.
///
/// If the edge exists, the runtime releases the retained child reference.
///
/// # Safety
///
/// `owner` and `child` may be null, but non-null managed values must be live
/// payload pointers. The caller must only unregister edges that represent slots
/// it owns; unregistering the wrong edge can release another owner's child too
/// early.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_unregister_edge(owner: *mut u8, child: *mut u8) {
    if owner.is_null() || child.is_null() {
        return;
    }
    let owner_key = owner as usize;
    let child_key = child as usize;
    let mut removed = false;
    if let Ok(mut state) = arc_state().lock() {
        removed = state.edges.remove(owner_key, child_key);
    }
    if removed {
        ori_arc_release(child);
    }
}

/// Replaces the registered child edge for one owner slot.
///
/// This unregisters `old_child` and registers `new_child`. Passing the same
/// pointer for both children is a no-op.
///
/// # Safety
///
/// `owner`, `old_child`, and `new_child` may be null, but non-null managed values
/// must be live payload pointers. Callers must use this for real slot
/// replacement only, so ARC retain/release balance matches the container state.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_update_edge(
    owner: *mut u8,
    old_child: *mut u8,
    new_child: *mut u8,
) {
    if old_child == new_child {
        return;
    }
    ori_arc_unregister_edge(owner, old_child);
    ori_arc_register_edge(owner, new_child);
}

/// Collects cycles whose strong edges were registered by generated code.
/// Returns the number of reclaimed heap objects.
///
/// # Safety
///
/// All registered allocations and edges must have been produced through the Ori
/// runtime ARC API. Callers must not mutate registered edges concurrently while
/// cycle collection is running.
#[no_mangle]
pub unsafe extern "C" fn ori_arc_collect_cycles() -> i64 {
    let Ok(state) = arc_state().lock() else {
        return 0;
    };
    let candidates: Vec<usize> = state.allocations.keys().copied().collect();
    collect_cycles_over_candidates(state, candidates)
}

/// Trial deletion restricted to `candidates` (Bacon synchronous collection).
///
/// Edges from owners outside the candidate set are *not* discounted, so they
/// count as external references — restricting the set can only under-collect,
/// never free a reachable object. The full collector passes every live
/// allocation as the candidate set.
///
/// Consumes the state guard: it is dropped before destructors run.
unsafe fn collect_cycles_over_candidates(
    mut state: std::sync::MutexGuard<'_, ArcState>,
    candidates: Vec<usize>,
) -> i64 {
    struct Mark {
        payload: usize,
        header: usize,
        trial_count: i64,
        marked: bool,
    }

    let mut marks: Vec<Mark> = candidates
        .iter()
        .filter_map(|payload| {
            state.allocations.get(payload).map(|allocation| {
                let header = allocation.header as *mut OriHeapHeader;
                Mark {
                    payload: *payload,
                    header: allocation.header,
                    trial_count: (*header).refcount.load(Ordering::Acquire),
                    marked: false,
                }
            })
        })
        .collect();
    let index_by_payload: HashMap<usize, usize> = marks
        .iter()
        .enumerate()
        .map(|(index, mark)| (mark.payload, index))
        .collect();
    // Trial deletion: discount references coming from registered edges whose
    // owner is inside the candidate set, so `trial_count > 0` means
    // "referenced from outside the candidate subgraph".
    for mark_index in 0..marks.len() {
        let owner = marks[mark_index].payload;
        let Some(children) = state.edges.by_owner.get(&owner) else {
            continue;
        };
        for child in children.clone() {
            if let Some(&child_index) = index_by_payload.get(&child) {
                marks[child_index].trial_count -= 1;
            }
        }
    }
    // Mark everything reachable from an externally-referenced root.
    let mut stack: Vec<usize> = (0..marks.len())
        .filter(|&index| marks[index].trial_count > 0)
        .collect();
    while let Some(index) = stack.pop() {
        if marks[index].marked {
            continue;
        }
        marks[index].marked = true;
        if let Some(children) = state.edges.by_owner.get(&marks[index].payload) {
            for child in children {
                if let Some(&child_index) = index_by_payload.get(child) {
                    if !marks[child_index].marked {
                        stack.push(child_index);
                    }
                }
            }
        }
    }

    let collected_payloads: HashSet<usize> = marks
        .iter()
        .filter(|mark| !mark.marked)
        .map(|mark| mark.payload)
        .collect();
    if collected_payloads.is_empty() {
        return 0;
    }
    let collected = collected_payloads.len() as i64;

    let collected_pairs: Vec<(usize, usize)> = marks
        .iter()
        .filter(|mark| !mark.marked)
        .map(|mark| (mark.payload, mark.header))
        .collect();
    let mut children_to_release = Vec::new();

    for &(payload, _) in &collected_pairs {
        state.remove_allocation(payload);
        // Surviving children owned by a collected object get their edge
        // reference released after the lock is dropped.
        for child in state.edges.take_children_of(payload) {
            if !collected_payloads.contains(&child) {
                children_to_release.push(child as *mut u8);
            }
        }
        // Edges from surviving owners into a collected object just disappear.
        state.edges.remove_edges_to(payload);
    }
    drop(state);

    for (payload, header) in collected_pairs {
        let payload = payload as *mut u8;
        let header = header as *mut OriHeapHeader;
        (*header).refcount.store(0, Ordering::Release);
        if let Some(dtor) = (*header).destructor {
            dtor(payload);
        }
        std::ptr::drop_in_place(&mut (*header).refcount);
        libc::free(header as *mut libc::c_void);
    }

    for child in children_to_release {
        ori_arc_release(child);
    }

    collected
}

/// Partial collection: trial deletion only over the subgraph reachable from
/// the suspect buffer (possible cycle roots recorded by `ori_arc_release`).
/// Returns `(freed, touched)`; `touched` is the candidate-subgraph size and
/// feeds the adaptive cooperative threshold.
unsafe fn collect_cycles_from_suspects() -> (i64, usize) {
    let Ok(mut state) = arc_state().lock() else {
        return (0, 0);
    };
    if state.suspects.is_empty() {
        return (0, 0);
    }
    let roots: Vec<usize> = std::mem::take(&mut state.suspects);
    for &payload in &roots {
        if let Some(allocation) = state.allocations.get_mut(&payload) {
            allocation.suspect_idx = NOT_SUSPECT;
        }
    }
    // Closure over outgoing edges: every member of a garbage cycle is
    // reachable from the suspect that disconnected it.
    let mut candidate_set: HashSet<usize> = HashSet::new();
    let mut stack: Vec<usize> = roots
        .into_iter()
        .filter(|payload| state.allocations.contains_key(payload))
        .collect();
    while let Some(payload) = stack.pop() {
        if !candidate_set.insert(payload) {
            continue;
        }
        if let Some(children) = state.edges.by_owner.get(&payload) {
            for &child in children {
                if state.allocations.contains_key(&child) && !candidate_set.contains(&child) {
                    stack.push(child);
                }
            }
        }
    }
    let candidates: Vec<usize> = candidate_set.into_iter().collect();
    let touched = candidates.len();
    let freed = collect_cycles_over_candidates(state, candidates);
    (freed, touched)
}

#[no_mangle]
pub extern "C" fn ori_arc_live_allocations() -> i64 {
    arc_state()
        .lock()
        .map(|state| state.allocations.len() as i64)
        .unwrap_or(0)
}

// ── ori.io ────────────────────────────────────────────────────────────────────

/// Print `len` bytes from `ptr` to stdout, followed by a newline.
#[no_mangle]
pub unsafe extern "C" fn ori_io_print(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        println!();
        return;
    }
    let data = std::slice::from_raw_parts(ptr, len as usize);
    let _ = std::io::stdout().write_all(data);
    let _ = std::io::stdout().write_all(b"\n");
    let _ = std::io::stdout().flush();
}

/// Print `len` bytes from `ptr` to stderr, followed by a newline.
#[no_mangle]
pub unsafe extern "C" fn ori_io_eprint(ptr: *const u8, len: i64) {
    if ptr.is_null() || len <= 0 {
        eprintln!();
        return;
    }
    let data = std::slice::from_raw_parts(ptr, len as usize);
    let _ = std::io::stderr().write_all(data);
    let _ = std::io::stderr().write_all(b"\n");
    let _ = std::io::stderr().flush();
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_read_line() -> *mut u8 {
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) => new_optional_ptr(false, std::ptr::null_mut()),
        Ok(_) => {
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            new_optional_ptr(true, cstring_from_str(&line))
        }
        Err(_) => new_optional_ptr(false, std::ptr::null_mut()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeStreamKind {
    Stdin,
    Stdout,
    Stderr,
    /// File opened for reading (`ori.io.open_input`).
    FileRead,
    /// File opened for writing (`ori.io.open_output`).
    FileWrite,
}

struct RuntimeIoStream {
    kind: RuntimeStreamKind,
    closed: bool,
    /// Owned file handle for `FileRead` / `FileWrite`; unused for stdio.
    file: Option<std::fs::File>,
}

unsafe extern "C" fn ori_io_stream_destructor(ptr: *mut u8) {
    if !ptr.is_null() {
        std::ptr::drop_in_place(ptr as *mut RuntimeIoStream);
    }
}

unsafe fn alloc_stream(kind: RuntimeStreamKind) -> *mut u8 {
    let layout_size = std::mem::size_of::<RuntimeIoStream>();
    let payload = ori_alloc(layout_size, Some(ori_io_stream_destructor)) as *mut RuntimeIoStream;
    if payload.is_null() {
        return std::ptr::null_mut();
    }
    std::ptr::write(
        payload,
        RuntimeIoStream {
            kind,
            closed: false,
            file: None,
        },
    );
    payload as *mut u8
}

unsafe fn alloc_file_stream(kind: RuntimeStreamKind, file: std::fs::File) -> *mut u8 {
    let layout_size = std::mem::size_of::<RuntimeIoStream>();
    let payload = ori_alloc(layout_size, Some(ori_io_stream_destructor)) as *mut RuntimeIoStream;
    if payload.is_null() {
        return std::ptr::null_mut();
    }
    std::ptr::write(
        payload,
        RuntimeIoStream {
            kind,
            closed: false,
            file: Some(file),
        },
    );
    payload as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_stdin() -> *mut u8 {
    alloc_stream(RuntimeStreamKind::Stdin)
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_stdout() -> *mut u8 {
    alloc_stream(RuntimeStreamKind::Stdout)
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_stderr() -> *mut u8 {
    alloc_stream(RuntimeStreamKind::Stderr)
}

/// Open a path as `ori.io.Input` (read-only file stream).
///
/// Returns `result[Input, string]`.
#[no_mangle]
pub unsafe extern "C" fn ori_io_open_input(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::File::open(path_str) {
        Ok(file) => new_result(true, alloc_file_stream(RuntimeStreamKind::FileRead, file)),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

/// Open a path as `ori.io.Output` (create/truncate write file stream).
///
/// Returns `result[Output, string]`.
#[no_mangle]
pub unsafe extern "C" fn ori_io_open_output(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::File::create(path_str) {
        Ok(file) => new_result(true, alloc_file_stream(RuntimeStreamKind::FileWrite, file)),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_read(stream_ptr: *mut u8, max_bytes: i64) -> *mut u8 {
    if stream_ptr.is_null() {
        return new_result(false, cstring_from_str("Invalid input stream"));
    }
    if max_bytes < 0 {
        return new_result(false, cstring_from_str("Invalid byte count"));
    }
    let stream = &mut *(stream_ptr as *mut RuntimeIoStream);
    if stream.closed {
        return new_result(false, cstring_from_str("Input stream is closed"));
    }
    let mut buf = vec![0u8; max_bytes as usize];
    use std::io::Read;
    let read_result = match stream.kind {
        RuntimeStreamKind::Stdin => std::io::stdin().read(&mut buf),
        RuntimeStreamKind::FileRead => {
            let Some(ref mut file) = stream.file else {
                return new_result(false, cstring_from_str("File input stream has no handle"));
            };
            file.read(&mut buf)
        }
        RuntimeStreamKind::Stdout | RuntimeStreamKind::Stderr | RuntimeStreamKind::FileWrite => {
            return new_result(false, cstring_from_str("Cannot read from output stream"));
        }
    };
    match read_result {
        Ok(0) => new_result(true, new_optional_ptr(false, std::ptr::null_mut())),
        Ok(n) => {
            buf.truncate(n);
            new_result(true, new_optional_ptr(true, cstring_from_bytes(buf)))
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_write(stream_ptr: *mut u8, data: *const u8) -> *mut u8 {
    if stream_ptr.is_null() {
        return new_result(false, cstring_from_str("Invalid output stream"));
    }
    let stream = &mut *(stream_ptr as *mut RuntimeIoStream);
    if stream.closed {
        return new_result(false, cstring_from_str("Output stream is closed"));
    }
    let bytes = bytes_payload(data);
    use std::io::Write;
    let write_result = match stream.kind {
        RuntimeStreamKind::Stdout => std::io::stdout().write(bytes),
        RuntimeStreamKind::Stderr => std::io::stderr().write(bytes),
        RuntimeStreamKind::FileWrite => {
            let Some(ref mut file) = stream.file else {
                return new_result(false, cstring_from_str("File output stream has no handle"));
            };
            file.write(bytes)
        }
        RuntimeStreamKind::Stdin | RuntimeStreamKind::FileRead => {
            return new_result(false, cstring_from_str("Cannot write to input stream"));
        }
    };
    match write_result {
        Ok(n) => new_result_i64_ok(n as i64),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_flush(stream_ptr: *mut u8) -> *mut u8 {
    if stream_ptr.is_null() {
        return new_result(false, cstring_from_str("Invalid output stream"));
    }
    let stream = &mut *(stream_ptr as *mut RuntimeIoStream);
    if stream.closed {
        return new_result(false, cstring_from_str("Output stream is closed"));
    }
    use std::io::Write;
    let flush_result = match stream.kind {
        RuntimeStreamKind::Stdout => std::io::stdout().flush(),
        RuntimeStreamKind::Stderr => std::io::stderr().flush(),
        RuntimeStreamKind::FileWrite => {
            let Some(ref mut file) = stream.file else {
                return new_result(false, cstring_from_str("File output stream has no handle"));
            };
            file.flush()
        }
        RuntimeStreamKind::Stdin | RuntimeStreamKind::FileRead => {
            return new_result(false, cstring_from_str("Cannot flush input stream"));
        }
    };
    match flush_result {
        Ok(()) => new_result(true, std::ptr::null_mut()),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_close_input(stream_ptr: *mut u8) {
    if stream_ptr.is_null() {
        return;
    }
    let stream = &mut *(stream_ptr as *mut RuntimeIoStream);
    stream.closed = true;
    if matches!(stream.kind, RuntimeStreamKind::FileRead) {
        stream.file = None;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_io_close_output(stream_ptr: *mut u8) {
    if stream_ptr.is_null() {
        return;
    }
    let stream = &mut *(stream_ptr as *mut RuntimeIoStream);
    stream.closed = true;
    if matches!(stream.kind, RuntimeStreamKind::FileWrite) {
        stream.file = None;
    }
}

// ── ori.string ────────────────────────────────────────────────────────────────

/// Convert an i64 to a null-terminated C string allocated with malloc.
/// Caller is responsible for freeing the result.
#[no_mangle]
pub unsafe extern "C" fn ori_int_to_cstr(n: i64) -> *mut u8 {
    let mut ptr = std::ptr::null_mut();
    let mut len = 0;
    ori_to_string_parts(n, &mut ptr, &mut len);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn ori_to_string(n: i64) -> *mut u8 {
    ori_int_to_cstr(n)
}

#[no_mangle]
pub extern "C" fn ori_to_int(value: i64) -> i64 {
    value
}

#[no_mangle]
pub unsafe extern "C" fn ori_to_string_parts(n: i64, out_ptr: *mut *mut u8, out_len: *mut i64) {
    write_string_parts(n.to_string(), out_ptr, out_len);
}

#[no_mangle]
pub unsafe extern "C" fn ori_float_to_string_parts(
    value: f64,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    write_string_parts(value.to_string(), out_ptr, out_len);
}

#[no_mangle]
pub unsafe extern "C" fn ori_bool_to_string_parts(
    value: c_uchar,
    out_ptr: *mut *mut u8,
    out_len: *mut i64,
) {
    write_string_parts(
        if value != 0 { "true" } else { "false" }.to_string(),
        out_ptr,
        out_len,
    );
}

unsafe fn write_string_parts(body: String, out_ptr: *mut *mut u8, out_len: *mut i64) {
    let ptr = cstring_from_str(&body);
    if !out_ptr.is_null() {
        *out_ptr = ptr;
    }
    if !out_len.is_null() {
        *out_len = body.len() as i64;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_len(ptr: *const u8) -> i64 {
    cstr_byte_len(ptr) as i64
}

unsafe fn cstr_byte_len(ptr: *const u8) -> usize {
    if ptr.is_null() {
        return 0;
    }
    CStr::from_ptr(ptr as *const c_char).to_bytes().len()
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_len(ptr: *const u8) -> i64 {
    cstr_str(ptr).chars().count() as i64
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_concat(a: *const u8, b: *const u8) -> *mut u8 {
    ori_string_concat_parts(a, cstr_byte_len(a) as i64, b, cstr_byte_len(b) as i64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_concat_parts(
    a: *const u8,
    a_len: i64,
    b: *const u8,
    b_len: i64,
) -> *mut u8 {
    let a = bounded_cstr_bytes(a, a_len);
    let b = bounded_cstr_bytes(b, b_len);
    let mut out = Vec::with_capacity(a.len() + b.len());
    out.extend_from_slice(a);
    out.extend_from_slice(b);
    cstring_from_bytes(out)
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_slice(s: *const u8, start: i64, end: i64) -> *mut u8 {
    if s.is_null() {
        abort_bounds("ori string slice bounds out of range");
    }
    let s = cstr_str(s);
    let (start, end) = checked_slice_bounds(
        s.chars().count() as i64,
        start,
        end,
        "ori string slice bounds out of range",
    );
    let start = char_index_to_byte_index(s, start);
    let end = char_index_to_byte_index(s, end);
    cstring_from_str(&s[start..end])
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_contains(s: *const u8, sub: *const u8) -> c_uchar {
    let s = cstr_str(s);
    let sub = cstr_str(sub);
    u8::from(s.contains(sub)) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_starts_with(s: *const u8, prefix: *const u8) -> c_uchar {
    let s = cstr_str(s);
    let prefix = cstr_str(prefix);
    u8::from(s.starts_with(prefix)) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_ends_with(s: *const u8, suffix: *const u8) -> c_uchar {
    let s = cstr_str(s);
    let suffix = cstr_str(suffix);
    u8::from(s.ends_with(suffix)) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_trim(s: *const u8) -> *mut u8 {
    cstring_from_str(cstr_str(s).trim())
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_trim_start(s: *const u8) -> *mut u8 {
    cstring_from_str(cstr_str(s).trim_start())
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_trim_end(s: *const u8) -> *mut u8 {
    cstring_from_str(cstr_str(s).trim_end())
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_to_upper(s: *const u8) -> *mut u8 {
    cstring_from_str(&cstr_str(s).to_uppercase())
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_to_lower(s: *const u8) -> *mut u8 {
    cstring_from_str(&cstr_str(s).to_lowercase())
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_replace(
    s: *const u8,
    from: *const u8,
    to: *const u8,
) -> *mut u8 {
    cstring_from_str(&cstr_str(s).replace(cstr_str(from), cstr_str(to)))
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_index_of(s: *const u8, sub: *const u8) -> i64 {
    let s = cstr_str(s);
    let sub = cstr_str(sub);
    s.find(sub)
        .map(|index| s[..index].chars().count() as i64)
        .unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_join(list: *mut OriList, sep: *const u8) -> *mut u8 {
    if list.is_null() {
        return cstring_from_str("");
    }
    let sep = cstr_str(sep);
    let mut out = String::new();
    for i in 0..(*list).len {
        if i > 0 {
            out.push_str(sep);
        }
        let item = *(*list).data.add(i as usize) as *const u8;
        out.push_str(cstr_str(item));
    }
    cstring_from_str(&out)
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_repeat(s: *const u8, count: i64) -> *mut u8 {
    if count <= 0 {
        return cstring_from_str("");
    }
    cstring_from_str(&cstr_str(s).repeat(count as usize))
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_pad_left(
    s: *const u8,
    target_len: i64,
    fill: *const u8,
) -> *mut u8 {
    pad_string(cstr_str(s), target_len, cstr_str(fill), true)
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_pad_right(
    s: *const u8,
    target_len: i64,
    fill: *const u8,
) -> *mut u8 {
    pad_string(cstr_str(s), target_len, cstr_str(fill), false)
}

#[cfg(test)]
unsafe fn cstr_bytes<'a>(ptr: *const u8) -> &'a [u8] {
    if ptr.is_null() {
        &[]
    } else {
        CStr::from_ptr(ptr as *const c_char).to_bytes()
    }
}

unsafe fn bounded_cstr_bytes<'a>(ptr: *const u8, len: i64) -> &'a [u8] {
    if ptr.is_null() || len <= 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len as usize)
    }
}

unsafe fn cstr_str<'a>(ptr: *const u8) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(ptr as *const c_char).to_str().unwrap_or("")
    }
}

fn cstring_from_str(s: &str) -> *mut u8 {
    cstring_from_bytes(s.as_bytes().to_vec())
}

fn cstring_from_bytes(bytes: Vec<u8>) -> *mut u8 {
    let len = bytes.len();
    unsafe {
        let ptr = ori_alloc(len + 1, None);
        if ptr.is_null() {
            return ptr;
        }
        if len > 0 {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
        }
        *ptr.add(len) = 0;
        ptr
    }
}

unsafe fn bytes_payload<'a>(ptr: *const u8) -> &'a [u8] {
    if ptr.is_null() {
        return &[];
    }
    if let Some(size) = registered_payload_size(ptr) {
        return std::slice::from_raw_parts(ptr, size.saturating_sub(1));
    }
    CStr::from_ptr(ptr as *const c_char).to_bytes()
}

fn char_index_to_byte_index(s: &str, index: usize) -> usize {
    if index == s.chars().count() {
        return s.len();
    }
    s.char_indices()
        .nth(index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(s.len())
}

unsafe fn ori_list_push_owned_managed(list: *mut OriList, value: *mut u8) {
    if list.is_null() {
        ori_arc_release(value);
        return;
    }
    ori_list_push_borrowed_maybe_managed(list, value as i64);
    ori_arc_release(value);
}

unsafe fn ori_list_push_borrowed_maybe_managed(list: *mut OriList, value: i64) {
    if list.is_null() {
        return;
    }
    ori_list_push(list, value);
    ori_arc_register_edge(list as *mut u8, value as *mut u8);
}

unsafe fn ori_set_register_borrowed_maybe_managed(set: *mut OriSet, value: i64) {
    ori_arc_register_edge(set as *mut u8, value as *mut u8);
}

unsafe fn ori_map_register_borrowed_key_value_maybe_managed(
    map: *mut OriMap,
    key: i64,
    value: i64,
) {
    ori_arc_register_edge(map as *mut u8, key as *mut u8);
    ori_arc_register_edge(map as *mut u8, value as *mut u8);
}

unsafe fn unregister_collection_edge(owner: *mut u8, value: i64) {
    ori_arc_unregister_edge(owner, value as *mut u8);
}

unsafe fn transfer_collection_edge_to_return_value(owner: *mut u8, value: i64) -> i64 {
    let value_ptr = value as *mut u8;
    ori_arc_retain(value_ptr);
    ori_arc_unregister_edge(owner, value_ptr);
    value
}

fn pad_string(s: &str, target_len: i64, fill: &str, left: bool) -> *mut u8 {
    let target_len = target_len.max(0) as usize;
    let current_len = s.chars().count();
    if current_len >= target_len {
        return cstring_from_str(s);
    }
    let fill = if fill.is_empty() { " " } else { fill };
    let pad_len = target_len - current_len;
    let padding: String = fill.chars().cycle().take(pad_len).collect();
    if left {
        cstring_from_str(&(padding + s))
    } else {
        cstring_from_str(&(s.to_owned() + &padding))
    }
}

#[repr(C)]
pub struct OriList {
    pub data: *mut i64,
    pub len: i64,
    pub cap: i64,
    pub version: i64,
}

unsafe extern "C" fn ori_list_dtor(ptr: *mut u8) {
    let list = ptr as *mut OriList;
    if !(*list).data.is_null() {
        libc::free((*list).data as *mut libc::c_void);
    }
}

/// Grow list storage so `cap >= min_cap` (no-op when already large enough).
unsafe fn list_ensure_capacity(list: *mut OriList, min_cap: i64) {
    if list.is_null() {
        return;
    }
    let min_cap = min_cap.max(0);
    if min_cap <= (*list).cap {
        return;
    }
    let mut next_cap = (*list).cap.max(8);
    while next_cap < min_cap {
        next_cap = (next_cap * 2).max(1);
    }
    let bytes = next_cap as usize * std::mem::size_of::<i64>();
    (*list).data = libc::realloc((*list).data as *mut libc::c_void, bytes) as *mut i64;
    (*list).cap = next_cap;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_new() -> *mut OriList {
    let cap = 8_i64;
    let bytes = cap as usize * std::mem::size_of::<i64>();
    let data = libc::malloc(bytes) as *mut i64;
    let list = ori_alloc(std::mem::size_of::<OriList>(), Some(ori_list_dtor)) as *mut OriList;
    if !list.is_null() {
        (*list).data = data;
        (*list).len = 0;
        (*list).cap = cap;
        (*list).version = 0;
    }
    list
}

/// Create an empty list with at least `capacity` slots pre-allocated.
#[no_mangle]
pub unsafe extern "C" fn ori_list_with_capacity(capacity: i64) -> *mut OriList {
    let list = ori_list_new();
    if !list.is_null() {
        list_ensure_capacity(list, capacity.max(0));
    }
    list
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_capacity(list: *mut OriList) -> i64 {
    if list.is_null() {
        0
    } else {
        (*list).cap
    }
}

/// Ensure the list can hold at least `capacity` elements without further realloc.
#[no_mangle]
pub unsafe extern "C" fn ori_list_reserve(list: *mut OriList, capacity: i64) {
    list_ensure_capacity(list, capacity);
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_push(list: *mut OriList, value: i64) {
    if list.is_null() {
        return;
    }
    if (*list).len >= (*list).cap {
        list_ensure_capacity(list, (*list).len + 1);
    }
    *(*list).data.add((*list).len as usize) = value;
    (*list).len += 1;
    (*list).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_get(list: *mut OriList, index: i64) -> i64 {
    if list.is_null() || index < 0 || index >= (*list).len {
        abort_bounds("ori list index out of bounds");
    }
    *(*list).data.add(index as usize)
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_set(list: *mut OriList, index: i64, value: i64) {
    if list.is_null() || index < 0 || index >= (*list).len {
        return;
    }
    *(*list).data.add(index as usize) = value;
    (*list).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_len(list: *mut OriList) -> i64 {
    if list.is_null() {
        0
    } else {
        (*list).len
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_is_empty(list: *mut OriList) -> c_uchar {
    u8::from(ori_list_len(list) == 0) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_pop(list: *mut OriList) -> i64 {
    if list.is_null() || (*list).len <= 0 {
        return 0;
    }
    (*list).len -= 1;
    let value = *(*list).data.add((*list).len as usize);
    (*list).version += 1;
    transfer_collection_edge_to_return_value(list as *mut u8, value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_try_pop(list: *mut OriList) -> *mut OriOptionalInt {
    if list.is_null() || (*list).len <= 0 {
        return alloc_optional_int(0, 0);
    }
    let value = ori_list_pop(list);
    alloc_optional_owned_managed_value(1, value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_remove(list: *mut OriList, index: i64) {
    if list.is_null() || index < 0 || index >= (*list).len {
        return;
    }
    let removed = *(*list).data.add(index as usize);
    unregister_collection_edge(list as *mut u8, removed);
    for i in index..((*list).len - 1) {
        let next = *(*list).data.add((i + 1) as usize);
        *(*list).data.add(i as usize) = next;
    }
    (*list).len -= 1;
    (*list).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_try_remove(list: *mut OriList, index: i64) -> c_uchar {
    if list.is_null() || index < 0 || index >= (*list).len {
        return 0;
    }
    ori_list_remove(list, index);
    1
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_insert(list: *mut OriList, index: i64, value: i64) {
    if list.is_null() {
        return;
    }
    if (*list).len >= (*list).cap {
        list_ensure_capacity(list, (*list).len + 1);
    }
    let index = index.max(0).min((*list).len) as usize;
    for i in (index..(*list).len as usize).rev() {
        let current = *(*list).data.add(i);
        *(*list).data.add(i + 1) = current;
    }
    *(*list).data.add(index) = value;
    (*list).len += 1;
    (*list).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_contains(list: *mut OriList, value: i64) -> c_uchar {
    u8::from(ori_list_index_of(list, value) >= 0) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_index_of(list: *mut OriList, value: i64) -> i64 {
    if list.is_null() {
        return -1;
    }
    for i in 0..(*list).len {
        if *(*list).data.add(i as usize) == value {
            return i;
        }
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_sort(list: *mut OriList) {
    if list.is_null() || (*list).len <= 1 {
        return;
    }
    let data = std::slice::from_raw_parts_mut((*list).data, (*list).len as usize);
    data.sort_unstable();
    (*list).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_reverse(list: *mut OriList) {
    if list.is_null() || (*list).len <= 1 {
        return;
    }
    let data = std::slice::from_raw_parts_mut((*list).data, (*list).len as usize);
    data.reverse();
    (*list).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_slice(list: *mut OriList, start: i64, end: i64) -> *mut OriList {
    if list.is_null() {
        abort_bounds("ori list slice bounds out of range");
    }
    let (start, end) = checked_slice_bounds(
        (*list).len,
        start,
        end,
        "ori list slice bounds out of range",
    );
    let out = ori_list_with_capacity((end - start) as i64);
    for i in start..end {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i as usize));
    }
    out
}

unsafe fn list_optional_at(list: *mut OriList, index: i64) -> *mut u8 {
    if list.is_null() || index < 0 || index >= (*list).len {
        return alloc_optional_int(0, 0) as *mut u8;
    }
    alloc_optional_int(1, *(*list).data.add(index as usize)) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_try_get(list: *mut OriList, index: i64) -> *mut OriOptionalInt {
    list_optional_at(list, index) as *mut OriOptionalInt
}

unsafe fn list_clear(list: *mut OriList) {
    if !list.is_null() {
        for i in 0..(*list).len {
            let value = *(*list).data.add(i as usize);
            unregister_collection_edge(list as *mut u8, value);
        }
        (*list).len = 0;
        (*list).version += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_clear(list: *mut OriList) {
    list_clear(list);
}

unsafe fn list_copy(list: *mut OriList) -> *mut OriList {
    if list.is_null() {
        return ori_list_new();
    }
    ori_list_slice(list, 0, (*list).len)
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_clone(list: *mut OriList) -> *mut OriList {
    list_copy(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_to_list(list: *mut OriList) -> *mut OriList {
    list_copy(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_from_list(list: *mut OriList) -> *mut OriList {
    list_copy(list)
}

#[repr(C)]
pub struct OriDeque {
    values: VecDeque<i64>,
    pub version: i64,
}

unsafe extern "C" fn ori_deque_dtor(ptr: *mut u8) {
    let deque = ptr as *mut OriDeque;
    if !deque.is_null() {
        std::ptr::drop_in_place(deque);
    }
}

unsafe fn deque_alloc() -> *mut OriDeque {
    let deque = ori_alloc(std::mem::size_of::<OriDeque>(), Some(ori_deque_dtor)) as *mut OriDeque;
    if !deque.is_null() {
        std::ptr::write(
            deque,
            OriDeque {
                values: VecDeque::new(),
                version: 0,
            },
        );
    }
    deque
}

unsafe fn deque_push_borrowed_maybe_managed(deque: *mut OriDeque, value: i64, front: bool) {
    if deque.is_null() {
        return;
    }
    if front {
        (*deque).values.push_front(value);
    } else {
        (*deque).values.push_back(value);
    }
    (*deque).version += 1;
    ori_arc_register_edge(deque as *mut u8, value as *mut u8);
}

unsafe fn deque_optional_value(value: Option<i64>) -> *mut u8 {
    match value {
        Some(value) => alloc_optional_int(1, value) as *mut u8,
        None => alloc_optional_int(0, 0) as *mut u8,
    }
}

unsafe fn deque_optional_removed_value(deque: *mut OriDeque, value: Option<i64>) -> *mut u8 {
    match value {
        Some(value) => {
            let value = transfer_collection_edge_to_return_value(deque as *mut u8, value);
            alloc_optional_owned_managed_value(1, value) as *mut u8
        }
        None => alloc_optional_int(0, 0) as *mut u8,
    }
}

unsafe fn deque_to_list(deque: *mut OriDeque) -> *mut OriList {
    let out = ori_list_new();
    if deque.is_null() {
        return out;
    }
    for value in (*deque).values.iter().copied() {
        ori_list_push_borrowed_maybe_managed(out, value);
    }
    out
}

unsafe fn deque_cursor_front(deque: *mut OriDeque) -> *mut u8 {
    deque_optional_value(if deque.is_null() || (*deque).values.is_empty() {
        None
    } else {
        Some(0)
    })
}

unsafe fn deque_cursor_back(deque: *mut OriDeque) -> *mut u8 {
    deque_optional_value(if deque.is_null() || (*deque).values.is_empty() {
        None
    } else {
        Some((*deque).values.len() as i64 - 1)
    })
}

unsafe fn deque_value_at(deque: *mut OriDeque, cursor: i64) -> *mut u8 {
    deque_optional_value(if deque.is_null() || cursor < 0 {
        None
    } else {
        (*deque).values.get(cursor as usize).copied()
    })
}

unsafe fn deque_insert_after(deque: *mut OriDeque, cursor: i64, value: i64) -> c_uchar {
    if deque.is_null() || cursor < 0 || cursor as usize >= (*deque).values.len() {
        return 0;
    }
    (*deque).values.insert(cursor as usize + 1, value);
    (*deque).version += 1;
    ori_arc_register_edge(deque as *mut u8, value as *mut u8);
    1
}

unsafe fn deque_insert_before(deque: *mut OriDeque, cursor: i64, value: i64) -> c_uchar {
    if deque.is_null() || cursor < 0 || cursor as usize >= (*deque).values.len() {
        return 0;
    }
    (*deque).values.insert(cursor as usize, value);
    (*deque).version += 1;
    ori_arc_register_edge(deque as *mut u8, value as *mut u8);
    1
}

unsafe fn deque_remove_at(deque: *mut OriDeque, cursor: i64) -> *mut u8 {
    if deque.is_null() || cursor < 0 || cursor as usize >= (*deque).values.len() {
        return alloc_optional_int(0, 0) as *mut u8;
    }
    let removed = (*deque).values.remove(cursor as usize);
    (*deque).version += 1;
    deque_optional_removed_value(deque, removed)
}

unsafe fn deque_find_raw(deque: *mut OriDeque, value: i64, value_kind: u8) -> *mut u8 {
    if deque.is_null() {
        return alloc_optional_int(0, 0) as *mut u8;
    }
    for (index, stored) in (*deque).values.iter().copied().enumerate() {
        let found = if value_kind == MAP_KEY_STRING {
            cstr_key_bytes(stored) == cstr_key_bytes(value)
        } else {
            stored == value
        };
        if found {
            return alloc_optional_int(1, index as i64) as *mut u8;
        }
    }
    alloc_optional_int(0, 0) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_new() -> *mut OriDeque {
    deque_alloc()
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_push_front(deque: *mut OriDeque, value: i64) {
    deque_push_borrowed_maybe_managed(deque, value, true);
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_push_back(deque: *mut OriDeque, value: i64) {
    deque_push_borrowed_maybe_managed(deque, value, false);
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_pop_front(deque: *mut OriDeque) -> *mut u8 {
    if deque.is_null() || (*deque).values.is_empty() {
        return alloc_optional_int(0, 0) as *mut u8;
    }
    let val = (*deque).values.pop_front();
    (*deque).version += 1;
    deque_optional_removed_value(deque, val)
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_pop_back(deque: *mut OriDeque) -> *mut u8 {
    if deque.is_null() || (*deque).values.is_empty() {
        return alloc_optional_int(0, 0) as *mut u8;
    }
    let val = (*deque).values.pop_back();
    (*deque).version += 1;
    deque_optional_removed_value(deque, val)
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_front(deque: *mut OriDeque) -> *mut u8 {
    deque_optional_value(if deque.is_null() {
        None
    } else {
        (*deque).values.front().copied()
    })
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_back(deque: *mut OriDeque) -> *mut u8 {
    deque_optional_value(if deque.is_null() {
        None
    } else {
        (*deque).values.back().copied()
    })
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_len(deque: *mut OriDeque) -> i64 {
    if deque.is_null() {
        0
    } else {
        (*deque).values.len() as i64
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_is_empty(deque: *mut OriDeque) -> c_uchar {
    u8::from(deque.is_null() || (*deque).values.is_empty()) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_clear(deque: *mut OriDeque) {
    if !deque.is_null() {
        for value in (*deque).values.iter().copied() {
            unregister_collection_edge(deque as *mut u8, value);
        }
        (*deque).values.clear();
        (*deque).version += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_to_list(deque: *mut OriDeque) -> *mut OriList {
    deque_to_list(deque)
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_clone(deque: *mut OriDeque) -> *mut OriDeque {
    let out = ori_deque_new();
    if deque.is_null() {
        return out;
    }
    for value in (*deque).values.iter().copied() {
        deque_push_borrowed_maybe_managed(out, value, false);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_new() -> *mut OriDeque {
    ori_deque_new()
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_enqueue(queue: *mut OriDeque, value: i64) {
    ori_deque_push_back(queue, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_dequeue(queue: *mut OriDeque) -> *mut u8 {
    ori_deque_pop_front(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_peek(queue: *mut OriDeque) -> *mut u8 {
    ori_deque_front(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_len(queue: *mut OriDeque) -> i64 {
    ori_deque_len(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_is_empty(queue: *mut OriDeque) -> c_uchar {
    ori_deque_is_empty(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_clear(queue: *mut OriDeque) {
    ori_deque_clear(queue);
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_to_list(queue: *mut OriDeque) -> *mut OriList {
    ori_deque_to_list(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_clone(queue: *mut OriDeque) -> *mut OriDeque {
    ori_deque_clone(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_new() -> *mut OriDeque {
    ori_deque_new()
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_push(stack: *mut OriDeque, value: i64) {
    ori_deque_push_back(stack, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_pop(stack: *mut OriDeque) -> *mut u8 {
    ori_deque_pop_back(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_peek(stack: *mut OriDeque) -> *mut u8 {
    ori_deque_back(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_len(stack: *mut OriDeque) -> i64 {
    ori_deque_len(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_is_empty(stack: *mut OriDeque) -> c_uchar {
    ori_deque_is_empty(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_clear(stack: *mut OriDeque) {
    ori_deque_clear(stack);
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_to_list(stack: *mut OriDeque) -> *mut OriList {
    ori_deque_to_list(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_clone(stack: *mut OriDeque) -> *mut OriDeque {
    ori_deque_clone(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_new() -> *mut OriDeque {
    ori_deque_new()
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_push_front(list: *mut OriDeque, value: i64) {
    ori_deque_push_front(list, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_push_back(list: *mut OriDeque, value: i64) {
    ori_deque_push_back(list, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_pop_front(list: *mut OriDeque) -> *mut u8 {
    ori_deque_pop_front(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_front(list: *mut OriDeque) -> *mut u8 {
    ori_deque_front(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_cursor_front(list: *mut OriDeque) -> *mut u8 {
    deque_cursor_front(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_cursor_back(list: *mut OriDeque) -> *mut u8 {
    deque_cursor_back(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_value_at(list: *mut OriDeque, cursor: i64) -> *mut u8 {
    deque_value_at(list, cursor)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_insert_after(
    list: *mut OriDeque,
    cursor: i64,
    value: i64,
) -> c_uchar {
    deque_insert_after(list, cursor, value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_remove_at(list: *mut OriDeque, cursor: i64) -> *mut u8 {
    deque_remove_at(list, cursor)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_find(list: *mut OriDeque, value: i64) -> *mut u8 {
    deque_find_raw(list, value, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_find_string(
    list: *mut OriDeque,
    value: *const u8,
) -> *mut u8 {
    deque_find_raw(list, value as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_len(list: *mut OriDeque) -> i64 {
    ori_deque_len(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_is_empty(list: *mut OriDeque) -> c_uchar {
    ori_deque_is_empty(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_clear(list: *mut OriDeque) {
    ori_deque_clear(list);
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_to_list(list: *mut OriDeque) -> *mut OriList {
    ori_deque_to_list(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_clone(list: *mut OriDeque) -> *mut OriDeque {
    ori_deque_clone(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_new() -> *mut OriDeque {
    ori_deque_new()
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_push_front(list: *mut OriDeque, value: i64) {
    ori_deque_push_front(list, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_push_back(list: *mut OriDeque, value: i64) {
    ori_deque_push_back(list, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_pop_front(list: *mut OriDeque) -> *mut u8 {
    ori_deque_pop_front(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_pop_back(list: *mut OriDeque) -> *mut u8 {
    ori_deque_pop_back(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_front(list: *mut OriDeque) -> *mut u8 {
    ori_deque_front(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_back(list: *mut OriDeque) -> *mut u8 {
    ori_deque_back(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_cursor_front(list: *mut OriDeque) -> *mut u8 {
    deque_cursor_front(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_cursor_back(list: *mut OriDeque) -> *mut u8 {
    deque_cursor_back(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_value_at(
    list: *mut OriDeque,
    cursor: i64,
) -> *mut u8 {
    deque_value_at(list, cursor)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_insert_after(
    list: *mut OriDeque,
    cursor: i64,
    value: i64,
) -> c_uchar {
    deque_insert_after(list, cursor, value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_insert_before(
    list: *mut OriDeque,
    cursor: i64,
    value: i64,
) -> c_uchar {
    deque_insert_before(list, cursor, value)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_remove_at(
    list: *mut OriDeque,
    cursor: i64,
) -> *mut u8 {
    deque_remove_at(list, cursor)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_find(list: *mut OriDeque, value: i64) -> *mut u8 {
    deque_find_raw(list, value, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_find_string(
    list: *mut OriDeque,
    value: *const u8,
) -> *mut u8 {
    deque_find_raw(list, value as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_len(list: *mut OriDeque) -> i64 {
    ori_deque_len(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_is_empty(list: *mut OriDeque) -> c_uchar {
    ori_deque_is_empty(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_clear(list: *mut OriDeque) {
    ori_deque_clear(list);
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_to_list(list: *mut OriDeque) -> *mut OriList {
    ori_deque_to_list(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_clone(list: *mut OriDeque) -> *mut OriDeque {
    ori_deque_clone(list)
}

#[repr(C)]
pub struct OriTree {
    values: *mut i64,
    parents: *mut i64,
    children: *mut *mut OriList,
    alive: *mut c_uchar,
    nodes_len: i64,
    len: i64,
    cap: i64,
    root: i64,
}

unsafe extern "C" fn ori_tree_dtor(ptr: *mut u8) {
    let tree = ptr as *mut OriTree;
    if tree.is_null() {
        return;
    }
    if !(*tree).children.is_null() {
        for i in 0..(*tree).nodes_len {
            let children = *(*tree).children.add(i as usize);
            if !children.is_null() {
                ori_arc_release(children as *mut u8);
            }
        }
    }
    if !(*tree).values.is_null() {
        libc::free((*tree).values as *mut libc::c_void);
    }
    if !(*tree).parents.is_null() {
        libc::free((*tree).parents as *mut libc::c_void);
    }
    if !(*tree).children.is_null() {
        libc::free((*tree).children as *mut libc::c_void);
    }
    if !(*tree).alive.is_null() {
        libc::free((*tree).alive as *mut libc::c_void);
    }
}

unsafe fn tree_realloc_i64(ptr: *mut i64, cap: i64) -> *mut i64 {
    let bytes = cap as usize * std::mem::size_of::<i64>();
    libc::realloc(ptr as *mut libc::c_void, bytes) as *mut i64
}

unsafe fn tree_realloc_children(ptr: *mut *mut OriList, cap: i64) -> *mut *mut OriList {
    let bytes = cap as usize * std::mem::size_of::<*mut OriList>();
    libc::realloc(ptr as *mut libc::c_void, bytes) as *mut *mut OriList
}

unsafe fn tree_realloc_alive(ptr: *mut c_uchar, cap: i64) -> *mut c_uchar {
    let bytes = cap as usize * std::mem::size_of::<c_uchar>();
    libc::realloc(ptr as *mut libc::c_void, bytes) as *mut c_uchar
}

unsafe fn tree_reserve(tree: *mut OriTree, min_cap: i64) {
    if tree.is_null() || (*tree).cap >= min_cap {
        return;
    }
    let old_cap = (*tree).cap;
    let mut next_cap = if old_cap <= 0 { 8 } else { old_cap * 2 };
    while next_cap < min_cap {
        next_cap *= 2;
    }
    (*tree).values = tree_realloc_i64((*tree).values, next_cap);
    (*tree).parents = tree_realloc_i64((*tree).parents, next_cap);
    (*tree).children = tree_realloc_children((*tree).children, next_cap);
    (*tree).alive = tree_realloc_alive((*tree).alive, next_cap);
    for i in old_cap..next_cap {
        *(*tree).values.add(i as usize) = 0;
        *(*tree).parents.add(i as usize) = -1;
        *(*tree).children.add(i as usize) = std::ptr::null_mut();
        *(*tree).alive.add(i as usize) = 0;
    }
    (*tree).cap = next_cap;
}

unsafe fn tree_push_node(tree: *mut OriTree, parent: i64, value: i64) -> i64 {
    tree_reserve(tree, (*tree).nodes_len + 1);
    let id = (*tree).nodes_len;
    *(*tree).values.add(id as usize) = value;
    *(*tree).parents.add(id as usize) = parent;
    *(*tree).children.add(id as usize) = ori_list_new();
    *(*tree).alive.add(id as usize) = 1;
    (*tree).nodes_len += 1;
    (*tree).len += 1;
    ori_arc_register_edge(tree as *mut u8, value as *mut u8);
    id
}

unsafe fn tree_valid_node(tree: *mut OriTree, node: i64) -> usize {
    if tree.is_null()
        || node < 0
        || node >= (*tree).nodes_len
        || *(*tree).alive.add(node as usize) == 0
    {
        abort_bounds("ori tree node id is invalid");
    }
    node as usize
}

unsafe fn tree_is_valid_node(tree: *mut OriTree, node: i64) -> bool {
    !tree.is_null()
        && node >= 0
        && node < (*tree).nodes_len
        && *(*tree).alive.add(node as usize) != 0
}

unsafe fn tree_is_descendant(tree: *mut OriTree, ancestor: i64, candidate: i64) -> bool {
    if !tree_is_valid_node(tree, ancestor) || !tree_is_valid_node(tree, candidate) {
        return false;
    }
    let mut current = candidate;
    while current >= 0 {
        if current == ancestor {
            return true;
        }
        current = *(*tree).parents.add(current as usize);
    }
    false
}

unsafe fn tree_remove_child_ref(tree: *mut OriTree, parent: i64, child: i64) {
    if parent < 0 || parent >= (*tree).nodes_len {
        return;
    }
    let children = *(*tree).children.add(parent as usize);
    if children.is_null() {
        return;
    }
    for i in 0..(*children).len {
        if *(*children).data.add(i as usize) == child {
            ori_list_remove(children, i);
            return;
        }
    }
}

unsafe fn tree_remove_subtree_inner(tree: *mut OriTree, node: i64) {
    if node < 0 || node >= (*tree).nodes_len || *(*tree).alive.add(node as usize) == 0 {
        return;
    }
    let children = *(*tree).children.add(node as usize);
    if !children.is_null() {
        let child_ids = list_copy(children);
        for i in 0..(*child_ids).len {
            tree_remove_subtree_inner(tree, *(*child_ids).data.add(i as usize));
        }
        ori_arc_release(child_ids as *mut u8);
        ori_arc_release(children as *mut u8);
        *(*tree).children.add(node as usize) = std::ptr::null_mut();
    }
    let value = *(*tree).values.add(node as usize);
    ori_arc_unregister_edge(tree as *mut u8, value as *mut u8);
    *(*tree).alive.add(node as usize) = 0;
    (*tree).len -= 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_new(root_value: i64) -> *mut OriTree {
    let tree = ori_alloc(std::mem::size_of::<OriTree>(), Some(ori_tree_dtor)) as *mut OriTree;
    if tree.is_null() {
        return tree;
    }
    (*tree).values = std::ptr::null_mut();
    (*tree).parents = std::ptr::null_mut();
    (*tree).children = std::ptr::null_mut();
    (*tree).alive = std::ptr::null_mut();
    (*tree).nodes_len = 0;
    (*tree).len = 0;
    (*tree).cap = 0;
    (*tree).root = -1;
    let root = tree_push_node(tree, -1, root_value);
    (*tree).root = root;
    tree
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_root(tree: *mut OriTree) -> i64 {
    if tree.is_null()
        || (*tree).root < 0
        || (*tree).root >= (*tree).nodes_len
        || *(*tree).alive.add((*tree).root as usize) == 0
    {
        abort_bounds("ori tree root is missing");
    }
    (*tree).root
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_value(tree: *mut OriTree, node: i64) -> i64 {
    let index = tree_valid_node(tree, node);
    *(*tree).values.add(index)
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_try_value(tree: *mut OriTree, node: i64) -> *mut OriOptionalInt {
    if !tree_is_valid_node(tree, node) {
        return alloc_optional_int(0, 0);
    }
    alloc_optional_int(1, *(*tree).values.add(node as usize))
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_contains_node(tree: *mut OriTree, node: i64) -> c_uchar {
    u8::from(tree_is_valid_node(tree, node)) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_set_value(tree: *mut OriTree, node: i64, value: i64) -> c_uchar {
    if !tree_is_valid_node(tree, node) {
        return 0;
    }
    let slot = (*tree).values.add(node as usize);
    let old_value = *slot;
    *slot = value;
    ori_arc_update_edge(tree as *mut u8, old_value as *mut u8, value as *mut u8);
    1
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_add_child(tree: *mut OriTree, parent: i64, value: i64) -> i64 {
    tree_valid_node(tree, parent);
    let child = tree_push_node(tree, parent, value);
    let children = *(*tree).children.add(parent as usize);
    ori_list_push(children, child);
    child
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_children(tree: *mut OriTree, node: i64) -> *mut OriList {
    let index = tree_valid_node(tree, node);
    list_copy(*(*tree).children.add(index))
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_parent(tree: *mut OriTree, node: i64) -> *mut OriOptionalInt {
    let index = tree_valid_node(tree, node);
    let parent = *(*tree).parents.add(index);
    if parent < 0 {
        alloc_optional_int(0, 0)
    } else {
        alloc_optional_int(1, parent)
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_remove_subtree(tree: *mut OriTree, node: i64) {
    let index = tree_valid_node(tree, node);
    let parent = *(*tree).parents.add(index);
    if parent >= 0 {
        tree_remove_child_ref(tree, parent, node);
    }
    tree_remove_subtree_inner(tree, node);
    if node == (*tree).root {
        (*tree).root = -1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_move_subtree(
    tree: *mut OriTree,
    node: i64,
    new_parent: i64,
) -> c_uchar {
    if !tree_is_valid_node(tree, node)
        || !tree_is_valid_node(tree, new_parent)
        || node == (*tree).root
        || tree_is_descendant(tree, node, new_parent)
    {
        return 0;
    }
    let old_parent = *(*tree).parents.add(node as usize);
    if old_parent >= 0 {
        tree_remove_child_ref(tree, old_parent, node);
    }
    *(*tree).parents.add(node as usize) = new_parent;
    let children = *(*tree).children.add(new_parent as usize);
    ori_list_push(children, node);
    1
}

unsafe fn tree_find_raw(tree: *mut OriTree, value: i64, string_mode: bool) -> *mut OriOptionalInt {
    if tree.is_null() {
        return alloc_optional_int(0, 0);
    }
    for node in 0..(*tree).nodes_len {
        if *(*tree).alive.add(node as usize) == 0 {
            continue;
        }
        let stored = *(*tree).values.add(node as usize);
        let matches = if string_mode {
            cstr_key_bytes(stored) == cstr_key_bytes(value)
        } else {
            stored == value
        };
        if matches {
            return alloc_optional_int(1, node);
        }
    }
    alloc_optional_int(0, 0)
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_find(tree: *mut OriTree, value: i64) -> *mut OriOptionalInt {
    tree_find_raw(tree, value, false)
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_find_string(
    tree: *mut OriTree,
    value: *const u8,
) -> *mut OriOptionalInt {
    tree_find_raw(tree, value as i64, true)
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_len(tree: *mut OriTree) -> i64 {
    if tree.is_null() {
        0
    } else {
        (*tree).len
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_depth(tree: *mut OriTree, node: i64) -> i64 {
    let mut index = tree_valid_node(tree, node) as i64;
    let mut depth = 0;
    loop {
        let parent = *(*tree).parents.add(index as usize);
        if parent < 0 {
            return depth;
        }
        tree_valid_node(tree, parent);
        depth += 1;
        index = parent;
    }
}

unsafe fn tree_pre_order(tree: *mut OriTree, node: i64, out: *mut OriList) {
    if node < 0 || node >= (*tree).nodes_len || *(*tree).alive.add(node as usize) == 0 {
        return;
    }
    ori_list_push(out, node);
    let children = *(*tree).children.add(node as usize);
    if children.is_null() {
        return;
    }
    for i in 0..(*children).len {
        tree_pre_order(tree, *(*children).data.add(i as usize), out);
    }
}

unsafe fn tree_post_order(tree: *mut OriTree, node: i64, out: *mut OriList) {
    if node < 0 || node >= (*tree).nodes_len || *(*tree).alive.add(node as usize) == 0 {
        return;
    }
    let children = *(*tree).children.add(node as usize);
    if !children.is_null() {
        for i in 0..(*children).len {
            tree_post_order(tree, *(*children).data.add(i as usize), out);
        }
    }
    ori_list_push(out, node);
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_pre_order(tree: *mut OriTree) -> *mut OriList {
    let out = ori_list_new();
    if tree.is_null() || (*tree).root < 0 {
        return out;
    }
    tree_pre_order(tree, (*tree).root, out);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_post_order(tree: *mut OriTree) -> *mut OriList {
    let out = ori_list_new();
    if tree.is_null() || (*tree).root < 0 {
        return out;
    }
    tree_post_order(tree, (*tree).root, out);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_breadth_first(tree: *mut OriTree) -> *mut OriList {
    let out = ori_list_new();
    if tree.is_null() || (*tree).root < 0 {
        return out;
    }
    let mut queue = VecDeque::new();
    queue.push_back((*tree).root);
    while let Some(node) = queue.pop_front() {
        if node < 0 || node >= (*tree).nodes_len || *(*tree).alive.add(node as usize) == 0 {
            continue;
        }
        ori_list_push(out, node);
        let children = *(*tree).children.add(node as usize);
        if children.is_null() {
            continue;
        }
        for i in 0..(*children).len {
            queue.push_back(*(*children).data.add(i as usize));
        }
    }
    out
}

unsafe fn tree_clone_subtree(
    source: *mut OriTree,
    target: *mut OriTree,
    source_node: i64,
    target_parent: i64,
) -> i64 {
    let value = *(*source).values.add(source_node as usize);
    let target_node = if target_parent < 0 {
        (*target).root
    } else {
        ori_tree_add_child(target, target_parent, value)
    };
    let children = *(*source).children.add(source_node as usize);
    if !children.is_null() {
        for i in 0..(*children).len {
            let child = *(*children).data.add(i as usize);
            if tree_is_valid_node(source, child) {
                tree_clone_subtree(source, target, child, target_node);
            }
        }
    }
    target_node
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_clone(tree: *mut OriTree) -> *mut OriTree {
    if tree.is_null() || (*tree).root < 0 || !tree_is_valid_node(tree, (*tree).root) {
        return ori_tree_new(0);
    }
    let root_value = *(*tree).values.add((*tree).root as usize);
    let out = ori_tree_new(root_value);
    let children = *(*tree).children.add((*tree).root as usize);
    if !children.is_null() {
        for i in 0..(*children).len {
            let child = *(*children).data.add(i as usize);
            if tree_is_valid_node(tree, child) {
                tree_clone_subtree(tree, out, child, (*out).root);
            }
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_tree_clone_subtree(tree: *mut OriTree, node: i64) -> *mut OriTree {
    if !tree_is_valid_node(tree, node) {
        return ori_tree_new(0);
    }
    let root_value = *(*tree).values.add(node as usize);
    let out = ori_tree_new(root_value);
    let children = *(*tree).children.add(node as usize);
    if !children.is_null() {
        for i in 0..(*children).len {
            let child = *(*children).data.add(i as usize);
            if tree_is_valid_node(tree, child) {
                tree_clone_subtree(tree, out, child, (*out).root);
            }
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_list_free(list: *mut OriList) {
    ori_arc_release(list as *mut u8);
}

/// Map: applies fn_ptr(env_ptr, elem) to each element and returns a new list.
/// fn_ptr must be compatible with `fn(*const c_void, i64) -> i64`.
#[no_mangle]
pub unsafe extern "C" fn ori_list_map(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() || fn_ptr.is_null() {
        return out;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> i64 = std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        ori_list_push_owned_managed(out, f(env_ptr, elem) as *mut u8);
    }
    out
}

/// Filter: keeps elements for which fn_ptr(env_ptr, elem) returns non-zero.
#[no_mangle]
pub unsafe extern "C" fn ori_list_filter(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() || fn_ptr.is_null() {
        return out;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> c_uchar =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        if f(env_ptr, elem) != 0 {
            ori_list_push_borrowed_maybe_managed(out, elem);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_flat_map(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() || fn_ptr.is_null() {
        return out;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> *mut OriList =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        let inner = f(env_ptr, elem);
        if inner.is_null() {
            continue;
        }
        for j in 0..(*inner).len {
            ori_list_push_borrowed_maybe_managed(out, *(*inner).data.add(j as usize));
        }
        ori_arc_release(inner as *mut u8);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_any(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> c_uchar {
    if list.is_null() || fn_ptr.is_null() {
        return 0;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> c_uchar =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        if f(env_ptr, elem) != 0 {
            return 1;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_all(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> c_uchar {
    if list.is_null() || fn_ptr.is_null() {
        return 0;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> c_uchar =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        if f(env_ptr, elem) == 0 {
            return 0;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_count_where(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> i64 {
    if list.is_null() || fn_ptr.is_null() {
        return 0;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> c_uchar =
        std::mem::transmute(fn_ptr);
    let mut count = 0;
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        if f(env_ptr, elem) != 0 {
            count += 1;
        }
    }
    count
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_take(list: *mut OriList, n: i64) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() || n <= 0 {
        return out;
    }
    let limit = std::cmp::min(n as usize, (*list).len as usize);
    for i in 0..limit {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_skip(list: *mut OriList, n: i64) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    let start = if n <= 0 {
        0
    } else {
        std::cmp::min(n as usize, (*list).len as usize)
    };
    for i in start..(*list).len as usize {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_reverse(list: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    for i in (0..(*list).len as usize).rev() {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_reduce(
    list: *mut OriList,
    initial: i64,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> i64 {
    if list.is_null() || fn_ptr.is_null() {
        return initial;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64, i64) -> i64 =
        std::mem::transmute(fn_ptr);
    let mut acc = initial;
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        acc = f(env_ptr, acc, elem);
    }
    acc
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_find(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriOptionalInt {
    if list.is_null() || fn_ptr.is_null() {
        return alloc_optional_int(0, 0);
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> c_uchar =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        if f(env_ptr, elem) != 0 {
            return alloc_optional_int(1, elem);
        }
    }
    alloc_optional_int(0, 0)
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_sort(list: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i));
    }
    if out.is_null() || (*out).len <= 1 {
        return out;
    }
    for i in 1..(*out).len as usize {
        let value = *(*out).data.add(i);
        let mut j = i;
        while j > 0 {
            let prev = *(*out).data.add(j - 1);
            if prev <= value {
                break;
            }
            *(*out).data.add(j) = prev;
            j -= 1;
        }
        *(*out).data.add(j) = value;
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_sort_string(list: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i));
    }
    if out.is_null() || (*out).len <= 1 {
        return out;
    }
    for i in 1..(*out).len as usize {
        let value = *(*out).data.add(i);
        let mut j = i;
        while j > 0 {
            let prev = *(*out).data.add(j - 1);
            if cstr_str(prev as *const u8) <= cstr_str(value as *const u8) {
                break;
            }
            *(*out).data.add(j) = prev;
            j -= 1;
        }
        *(*out).data.add(j) = value;
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_sort_by(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        ori_list_push_borrowed_maybe_managed(out, *(*list).data.add(i));
    }
    if out.is_null() || (*out).len <= 1 || fn_ptr.is_null() {
        return out;
    }
    let compare: unsafe extern "C" fn(*const std::ffi::c_void, i64, i64) -> i64 =
        std::mem::transmute(fn_ptr);
    for i in 1..(*out).len as usize {
        let value = *(*out).data.add(i);
        let mut j = i;
        while j > 0 {
            let prev = *(*out).data.add(j - 1);
            if compare(env_ptr, prev, value) <= 0 {
                break;
            }
            *(*out).data.add(j) = prev;
            j -= 1;
        }
        *(*out).data.add(j) = value;
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_unique(list: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        let elem = *(*list).data.add(i);
        let mut seen = false;
        for j in 0..(*out).len as usize {
            if *(*out).data.add(j) == elem {
                seen = true;
                break;
            }
        }
        if !seen {
            ori_list_push_borrowed_maybe_managed(out, elem);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_unique_string(list: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        let elem = *(*list).data.add(i);
        let mut seen = false;
        for j in 0..(*out).len as usize {
            if cstr_str(*(*out).data.add(j) as *const u8) == cstr_str(elem as *const u8) {
                seen = true;
                break;
            }
        }
        if !seen {
            ori_list_push_borrowed_maybe_managed(out, elem);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_zip(left: *mut OriList, right: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if left.is_null() || right.is_null() {
        return out;
    }
    let limit = std::cmp::min((*left).len, (*right).len);
    for i in 0..limit as usize {
        let pair = ori_alloc(16, None) as *mut i64;
        if pair.is_null() {
            continue;
        }
        *pair = *(*left).data.add(i);
        *pair.add(1) = *(*right).data.add(i);
        ori_arc_register_edge(pair as *mut u8, *pair as *mut u8);
        ori_arc_register_edge(pair as *mut u8, *pair.add(1) as *mut u8);
        ori_list_push_owned_managed(out, pair as *mut u8);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_partition(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut u8 {
    let tuple = ori_alloc(16, None) as *mut i64;
    if tuple.is_null() {
        return std::ptr::null_mut();
    }
    let matches = ori_list_new();
    let rest = ori_list_new();
    *tuple = matches as i64;
    *tuple.add(1) = rest as i64;
    ori_arc_register_edge(tuple as *mut u8, matches as *mut u8);
    ori_arc_register_edge(tuple as *mut u8, rest as *mut u8);
    ori_arc_release(matches as *mut u8);
    ori_arc_release(rest as *mut u8);
    if list.is_null() || fn_ptr.is_null() {
        return tuple as *mut u8;
    }
    let f: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> c_uchar =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        if f(env_ptr, elem) != 0 {
            ori_list_push_borrowed_maybe_managed(matches, elem);
        } else {
            ori_list_push_borrowed_maybe_managed(rest, elem);
        }
    }
    tuple as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_group_by(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriMap {
    let out = ori_map_new();
    if list.is_null() || fn_ptr.is_null() {
        return out;
    }
    let key_fn: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> i64 =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        let key = key_fn(env_ptr, elem);
        let bucket = if ori_map_contains(out, key) != 0 {
            ori_map_get(out, key) as *mut OriList
        } else {
            let bucket = ori_list_new();
            ori_map_set(out, key, bucket as i64);
            ori_map_register_borrowed_key_value_maybe_managed(out, key, bucket as i64);
            ori_arc_release(bucket as *mut u8);
            bucket
        };
        ori_list_push_borrowed_maybe_managed(bucket, elem);
        if ori_map_contains(out, key) != 0 {
            ori_arc_release(key as *mut u8);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_group_by_string(
    list: *mut OriList,
    fn_ptr: *const std::ffi::c_void,
    env_ptr: *const std::ffi::c_void,
) -> *mut OriMap {
    let out = ori_map_new();
    if list.is_null() || fn_ptr.is_null() {
        return out;
    }
    let key_fn: unsafe extern "C" fn(*const std::ffi::c_void, i64) -> i64 =
        std::mem::transmute(fn_ptr);
    for i in 0..(*list).len {
        let elem = *(*list).data.add(i as usize);
        let key = key_fn(env_ptr, elem) as *const u8;
        let bucket = if ori_map_contains_string(out, key) != 0 {
            ori_map_get_string(out, key) as *mut OriList
        } else {
            let bucket = ori_list_new();
            ori_map_set_string(out, key, bucket as i64);
            ori_map_register_borrowed_key_value_maybe_managed(out, key as i64, bucket as i64);
            ori_arc_release(bucket as *mut u8);
            bucket
        };
        ori_list_push_borrowed_maybe_managed(bucket, elem);
        ori_arc_release(key as *mut u8);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_iter_flatten(nested: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if nested.is_null() {
        return out;
    }
    for i in 0..(*nested).len as usize {
        let inner = *(*nested).data.add(i) as *mut OriList;
        if inner.is_null() {
            continue;
        }
        for j in 0..(*inner).len as usize {
            ori_list_push_borrowed_maybe_managed(out, *(*inner).data.add(j));
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_split(s: *const u8, sep: *const u8) -> *mut OriList {
    let list = ori_list_new();
    let text = cstr_str(s);
    let sep = cstr_str(sep);
    if sep.is_empty() {
        for ch in text.chars() {
            ori_list_push_owned_managed(list, cstring_from_str(&ch.to_string()));
        }
    } else {
        for part in text.split(sep) {
            ori_list_push_owned_managed(list, cstring_from_str(part));
        }
    }
    list
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_chars(s: *const u8) -> *mut OriList {
    let list = ori_list_new();
    for ch in cstr_str(s).chars() {
        ori_list_push_owned_managed(list, cstring_from_str(&ch.to_string()));
    }
    list
}

// ── Hash helpers (splitmix64) ─────────────────────────────────────────────────

const HT_EMPTY: i64 = -1;
const HT_TOMB: i64 = -2;

fn hash_i64(k: i64) -> usize {
    let x = k as u64;
    let x = x ^ (x >> 30);
    let x = x.wrapping_mul(0xbf58476d1ce4e5b9_u64);
    let x = x ^ (x >> 27);
    let x = x.wrapping_mul(0x94d049bb133111eb_u64);
    (x ^ (x >> 31)) as usize
}

fn hash_bytes(bytes: &[u8]) -> usize {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash as usize
}

// ── ori.set — open-addressing hash set with dense item array ─────────────────
//
// Layout prefix is identical to OriList (items/len/cap at offsets 0/8/16) so
// that the native-backend for-loop can call ori_list_len / ori_list_get on a
// *OriSet pointer without knowing about the extra hash fields.

#[repr(C)]
pub struct OriSet {
    pub items: *mut i64, // dense elements [0..len); same offset as OriList.data
    pub len: i64,        // element count;           same offset as OriList.len
    pub cap: i64,        // dense array capacity;    same offset as OriList.cap
    pub version: i64,    // version;                 same offset as OriList.version
    pub ht: *mut i64,    // hash table slots (HT_EMPTY / HT_TOMB / dense_index)
    pub ht_cap: i64,     // hash table capacity (power of 2)
    pub item_kind: u8,
}

unsafe extern "C" fn ori_set_dtor(ptr: *mut u8) {
    let set = ptr as *mut OriSet;
    if !(*set).items.is_null() {
        libc::free((*set).items as *mut libc::c_void);
    }
    if !(*set).ht.is_null() {
        libc::free((*set).ht as *mut libc::c_void);
    }
}

const INITIAL_SET_HT_CAP: usize = 16;
const SET_ITEM_UNKNOWN: u8 = 0;
const SET_ITEM_INT: u8 = 1;
const SET_ITEM_STRING: u8 = 2;

unsafe fn cstr_value_bytes(value: i64) -> &'static [u8] {
    if value == 0 {
        &[]
    } else {
        CStr::from_ptr(value as *const c_char).to_bytes()
    }
}

unsafe fn set_item_hash(kind: u8, value: i64) -> usize {
    if kind == SET_ITEM_STRING {
        hash_bytes(cstr_value_bytes(value))
    } else {
        hash_i64(value)
    }
}

unsafe fn set_item_equals(kind: u8, stored: i64, query: i64) -> bool {
    if kind == SET_ITEM_STRING {
        cstr_value_bytes(stored) == cstr_value_bytes(query)
    } else {
        stored == query
    }
}

unsafe fn ht_lookup_set(
    ht: *mut i64,
    ht_cap: usize,
    items: *mut i64,
    item_kind: u8,
    value: i64,
) -> usize {
    let mask = ht_cap - 1;
    let mut slot = set_item_hash(item_kind, value) & mask;
    loop {
        let v = *ht.add(slot);
        if v == HT_EMPTY {
            return usize::MAX;
        }
        if v != HT_TOMB && set_item_equals(item_kind, *items.add(v as usize), value) {
            return slot;
        }
        slot = (slot + 1) & mask;
    }
}

unsafe fn ht_find_insert_slot_set(ht: *mut i64, ht_cap: usize, item_kind: u8, value: i64) -> usize {
    let mask = ht_cap - 1;
    let mut slot = set_item_hash(item_kind, value) & mask;
    loop {
        let v = *ht.add(slot);
        if v == HT_EMPTY || v == HT_TOMB {
            return slot;
        }
        slot = (slot + 1) & mask;
    }
}

unsafe fn set_prepare_item_kind(set: *mut OriSet, item_kind: u8) -> bool {
    if (*set).item_kind == SET_ITEM_UNKNOWN {
        (*set).item_kind = item_kind;
        return true;
    }
    (*set).item_kind == item_kind
}

unsafe fn set_alloc() -> *mut OriSet {
    let set = ori_alloc(std::mem::size_of::<OriSet>(), Some(ori_set_dtor)) as *mut OriSet;
    if !set.is_null() {
        let items = libc::malloc(8 * std::mem::size_of::<i64>()) as *mut i64;
        let ht_bytes = INITIAL_SET_HT_CAP * std::mem::size_of::<i64>();
        let ht = libc::malloc(ht_bytes) as *mut i64;
        std::ptr::write_bytes(ht as *mut u8, 0xFF, ht_bytes); // fill with -1
        (*set).items = items;
        (*set).len = 0;
        (*set).cap = 8;
        (*set).version = 0;
        (*set).ht = ht;
        (*set).ht_cap = INITIAL_SET_HT_CAP as i64;
        (*set).item_kind = SET_ITEM_UNKNOWN;
    }
    set
}

unsafe fn set_rebuild_ht(set: *mut OriSet) {
    let ht_cap = (*set).ht_cap as usize;
    std::ptr::write_bytes(
        (*set).ht as *mut u8,
        0xFF,
        ht_cap * std::mem::size_of::<i64>(),
    );
    for i in 0..(*set).len as usize {
        let item = *(*set).items.add(i);
        let slot = ht_find_insert_slot_set((*set).ht, ht_cap, (*set).item_kind, item);
        *(*set).ht.add(slot) = i as i64;
    }
}

unsafe fn set_grow(set: *mut OriSet) {
    let new_ht_cap = (*set).ht_cap as usize * 2;
    let ht_bytes = new_ht_cap * std::mem::size_of::<i64>();
    let new_ht = libc::realloc((*set).ht as *mut libc::c_void, ht_bytes) as *mut i64;
    (*set).ht = new_ht;
    (*set).ht_cap = new_ht_cap as i64;
    set_rebuild_ht(set);
}

unsafe fn set_reserve_capacity(set: *mut OriSet, capacity: i64) {
    if set.is_null() || capacity <= (*set).cap {
        return;
    }
    let target_cap = capacity.max(1);
    (*set).items = libc::realloc(
        (*set).items as *mut libc::c_void,
        target_cap as usize * std::mem::size_of::<i64>(),
    ) as *mut i64;
    (*set).cap = target_cap;

    let mut target_ht_cap = INITIAL_SET_HT_CAP;
    let required_slots = (target_cap as usize).saturating_mul(2).max(1);
    while target_ht_cap < required_slots {
        target_ht_cap = target_ht_cap.saturating_mul(2);
    }
    if target_ht_cap > (*set).ht_cap as usize {
        let ht_bytes = target_ht_cap * std::mem::size_of::<i64>();
        (*set).ht = libc::realloc((*set).ht as *mut libc::c_void, ht_bytes) as *mut i64;
        (*set).ht_cap = target_ht_cap as i64;
        set_rebuild_ht(set);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_new() -> *mut OriSet {
    set_alloc()
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_add(set: *mut OriSet, value: i64) {
    ori_set_add_raw(set, value, SET_ITEM_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_add_string(set: *mut OriSet, value: *const u8) {
    ori_set_add_raw(set, value as i64, SET_ITEM_STRING);
}

unsafe fn ori_set_add_raw(set: *mut OriSet, value: i64, item_kind: u8) {
    if set.is_null() {
        return;
    }
    if !set_prepare_item_kind(set, item_kind) {
        return;
    }
    let ht_cap = (*set).ht_cap as usize;
    if ht_lookup_set((*set).ht, ht_cap, (*set).items, item_kind, value) != usize::MAX {
        return; // already present
    }
    // Grow dense array if full
    if (*set).len >= (*set).cap {
        let new_cap = (*set).cap * 2;
        (*set).items = libc::realloc(
            (*set).items as *mut libc::c_void,
            new_cap as usize * std::mem::size_of::<i64>(),
        ) as *mut i64;
        (*set).cap = new_cap;
    }
    // Grow hash table at 50% load
    if (*set).len as usize * 2 >= ht_cap {
        set_grow(set);
    }
    let dense_idx = (*set).len as usize;
    *(*set).items.add(dense_idx) = value;
    (*set).len += 1;
    let ht_cap = (*set).ht_cap as usize;
    let slot = ht_find_insert_slot_set((*set).ht, ht_cap, item_kind, value);
    *(*set).ht.add(slot) = dense_idx as i64;
    (*set).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_contains(set: *mut OriSet, value: i64) -> c_uchar {
    ori_set_contains_raw(set, value, SET_ITEM_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_contains_string(set: *mut OriSet, value: *const u8) -> c_uchar {
    ori_set_contains_raw(set, value as i64, SET_ITEM_STRING)
}

unsafe fn ori_set_contains_raw(set: *mut OriSet, value: i64, item_kind: u8) -> c_uchar {
    if set.is_null() {
        return 0;
    }
    if (*set).item_kind != SET_ITEM_UNKNOWN && (*set).item_kind != item_kind {
        return 0;
    }
    let ht_cap = (*set).ht_cap as usize;
    u8::from(ht_lookup_set((*set).ht, ht_cap, (*set).items, item_kind, value) != usize::MAX)
        as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_is_empty(set: *mut OriSet) -> c_uchar {
    u8::from(ori_set_len(set) == 0) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_remove(set: *mut OriSet, value: i64) {
    ori_set_remove_raw(set, value, SET_ITEM_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_remove_string(set: *mut OriSet, value: *const u8) {
    ori_set_remove_raw(set, value as i64, SET_ITEM_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_try_remove(set: *mut OriSet, value: i64) -> c_uchar {
    if ori_set_contains(set, value) == 0 {
        0
    } else {
        ori_set_remove(set, value);
        1
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_try_remove_string(set: *mut OriSet, value: *const u8) -> c_uchar {
    if ori_set_contains_string(set, value) == 0 {
        0
    } else {
        ori_set_remove_string(set, value);
        1
    }
}

unsafe fn ori_set_remove_raw(set: *mut OriSet, value: i64, item_kind: u8) {
    if set.is_null() {
        return;
    }
    if (*set).item_kind != SET_ITEM_UNKNOWN && (*set).item_kind != item_kind {
        return;
    }
    let ht_cap = (*set).ht_cap as usize;
    let slot = ht_lookup_set((*set).ht, ht_cap, (*set).items, item_kind, value);
    if slot == usize::MAX {
        return;
    }
    let dense_idx = *(*set).ht.add(slot) as usize;
    let removed = *(*set).items.add(dense_idx);
    unregister_collection_edge(set as *mut u8, removed);
    *(*set).ht.add(slot) = HT_TOMB;
    let last_idx = (*set).len as usize - 1;
    if dense_idx != last_idx {
        let last_val = *(*set).items.add(last_idx);
        *(*set).items.add(dense_idx) = last_val;
        // Update hash slot pointing to last_idx → now points to dense_idx
        let last_slot = ht_lookup_set((*set).ht, ht_cap, (*set).items, item_kind, last_val);
        if last_slot != usize::MAX {
            *(*set).ht.add(last_slot) = dense_idx as i64;
        }
    }
    (*set).len -= 1;
    (*set).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_len(set: *mut OriSet) -> i64 {
    if set.is_null() {
        0
    } else {
        (*set).len
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_capacity(set: *mut OriSet) -> i64 {
    if set.is_null() {
        0
    } else {
        (*set).cap
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_reserve(set: *mut OriSet, capacity: i64) {
    set_reserve_capacity(set, capacity);
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_clear(set: *mut OriSet) {
    if set.is_null() {
        return;
    }
    for i in 0..(*set).len {
        let value = *(*set).items.add(i as usize);
        unregister_collection_edge(set as *mut u8, value);
    }
    (*set).len = 0;
    (*set).item_kind = SET_ITEM_UNKNOWN;
    std::ptr::write_bytes(
        (*set).ht as *mut u8,
        0xFF,
        (*set).ht_cap as usize * std::mem::size_of::<i64>(),
    );
    (*set).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_to_list(set: *mut OriSet) -> *mut OriList {
    let out = ori_list_new();
    if set.is_null() {
        return out;
    }
    for i in 0..(*set).len as usize {
        ori_list_push_borrowed_maybe_managed(out, *(*set).items.add(i));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_clone(set: *mut OriSet) -> *mut OriSet {
    let out = ori_set_new();
    if set.is_null() {
        return out;
    }
    for i in 0..(*set).len as usize {
        let value = *(*set).items.add(i);
        ori_set_add_raw(out, value, (*set).item_kind);
        ori_set_register_borrowed_maybe_managed(out, value);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_from_list(list: *mut OriList) -> *mut OriSet {
    let out = ori_set_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        let value = *(*list).data.add(i);
        ori_set_add(out, value);
        ori_set_register_borrowed_maybe_managed(out, value);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_from_list_string(list: *mut OriList) -> *mut OriSet {
    let out = ori_set_new();
    if list.is_null() {
        return out;
    }
    for i in 0..(*list).len as usize {
        let value = *(*list).data.add(i);
        ori_set_add_raw(out, value, MAP_KEY_STRING);
        ori_set_register_borrowed_maybe_managed(out, value);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_free(set: *mut OriSet) {
    ori_arc_release(set as *mut u8);
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_union(a: *mut OriSet, b: *mut OriSet) -> *mut OriSet {
    let out = ori_set_new();
    if !a.is_null() {
        for i in 0..(*a).len as usize {
            let value = *(*a).items.add(i);
            ori_set_add_raw(out, value, (*a).item_kind);
            ori_set_register_borrowed_maybe_managed(out, value);
        }
    }
    if !b.is_null() {
        for i in 0..(*b).len as usize {
            let value = *(*b).items.add(i);
            ori_set_add_raw(out, value, (*b).item_kind);
            ori_set_register_borrowed_maybe_managed(out, value);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_intersection(a: *mut OriSet, b: *mut OriSet) -> *mut OriSet {
    let out = ori_set_new();
    if a.is_null() || b.is_null() {
        return out;
    }
    for i in 0..(*a).len as usize {
        let v = *(*a).items.add(i);
        if ori_set_contains_raw(b, v, (*a).item_kind) != 0 {
            ori_set_add_raw(out, v, (*a).item_kind);
            ori_set_register_borrowed_maybe_managed(out, v);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_set_difference(a: *mut OriSet, b: *mut OriSet) -> *mut OriSet {
    let out = ori_set_new();
    if a.is_null() {
        return out;
    }
    for i in 0..(*a).len as usize {
        let v = *(*a).items.add(i);
        if b.is_null() || ori_set_contains_raw(b, v, (*a).item_kind) == 0 {
            ori_set_add_raw(out, v, (*a).item_kind);
            ori_set_register_borrowed_maybe_managed(out, v);
        }
    }
    out
}

// ── ori.map — open-addressing hash map with dense key/value arrays ────────────
//
// keys[0..len) and values[0..len) are dense (no gaps).
// ht[] stores dense indices for O(1) lookup; HT_EMPTY / HT_TOMB as sentinels.
// ori_map_key_at / ori_map_value_at iterate over the dense arrays directly,
// so for-loops remain O(n) without touching the hash table.

#[repr(C)]
pub struct OriMap {
    pub keys: *mut i64,
    pub values: *mut i64,
    pub len: i64,
    pub cap: i64,
    pub version: i64,
    pub ht: *mut i64, // hash table slots (HT_EMPTY / HT_TOMB / dense_index)
    pub ht_cap: i64,  // hash table capacity (power of 2)
    pub key_kind: u8,
    pub hash_fn: *const std::ffi::c_void,
    pub eq_fn: *const std::ffi::c_void,
}

unsafe extern "C" fn ori_map_dtor(ptr: *mut u8) {
    let map = ptr as *mut OriMap;
    if !(*map).keys.is_null() {
        libc::free((*map).keys as *mut libc::c_void);
    }
    if !(*map).values.is_null() {
        libc::free((*map).values as *mut libc::c_void);
    }
    if !(*map).ht.is_null() {
        libc::free((*map).ht as *mut libc::c_void);
    }
}

const INITIAL_MAP_HT_CAP: usize = 16;
const MAP_KEY_UNKNOWN: u8 = 0;
const MAP_KEY_INT: u8 = 1;
const MAP_KEY_STRING: u8 = 2;
const MAP_KEY_CUSTOM: u8 = 3;

unsafe fn cstr_key_bytes(key: i64) -> &'static [u8] {
    if key == 0 {
        &[]
    } else {
        CStr::from_ptr(key as *const c_char).to_bytes()
    }
}

unsafe fn map_key_hash(map: *mut OriMap, key: i64) -> usize {
    let kind = (*map).key_kind;
    if kind == MAP_KEY_STRING {
        hash_bytes(cstr_key_bytes(key))
    } else if kind == MAP_KEY_CUSTOM {
        let hash_fn: extern "C" fn(i64) -> i64 = std::mem::transmute((*map).hash_fn);
        hash_fn(key) as usize
    } else {
        hash_i64(key)
    }
}

unsafe fn map_key_equals(map: *mut OriMap, stored: i64, query: i64) -> bool {
    let kind = (*map).key_kind;
    if kind == MAP_KEY_STRING {
        cstr_key_bytes(stored) == cstr_key_bytes(query)
    } else if kind == MAP_KEY_CUSTOM {
        let eq_fn: extern "C" fn(i64, i64) -> c_uchar = std::mem::transmute((*map).eq_fn);
        eq_fn(stored, query) != 0
    } else {
        stored == query
    }
}

unsafe fn ht_lookup_map(map: *mut OriMap, key: i64) -> usize {
    let mask = (*map).ht_cap as usize - 1;
    let mut slot = map_key_hash(map, key) & mask;
    loop {
        let v = *(*map).ht.add(slot);
        if v == HT_EMPTY {
            return usize::MAX;
        }
        if v != HT_TOMB && map_key_equals(map, *(*map).keys.add(v as usize), key) {
            return slot;
        }
        slot = (slot + 1) & mask;
    }
}

unsafe fn ht_find_insert_slot_map(map: *mut OriMap, key: i64) -> usize {
    let mask = (*map).ht_cap as usize - 1;
    let mut slot = map_key_hash(map, key) & mask;
    loop {
        let v = *(*map).ht.add(slot);
        if v == HT_EMPTY || v == HT_TOMB {
            return slot;
        }
        slot = (slot + 1) & mask;
    }
}

unsafe fn map_prepare_key_kind(map: *mut OriMap, key_kind: u8) -> bool {
    if (*map).key_kind == MAP_KEY_UNKNOWN {
        (*map).key_kind = key_kind;
        return true;
    }
    (*map).key_kind == key_kind
}

unsafe fn map_alloc_with(
    hash_fn: *const std::ffi::c_void,
    eq_fn: *const std::ffi::c_void,
    kind: u8,
) -> *mut OriMap {
    let cap = 8_i64;
    let bytes = cap as usize * std::mem::size_of::<i64>();
    let ht_bytes = INITIAL_MAP_HT_CAP * std::mem::size_of::<i64>();
    let map = ori_alloc(std::mem::size_of::<OriMap>(), Some(ori_map_dtor)) as *mut OriMap;
    if !map.is_null() {
        let ht = libc::malloc(ht_bytes) as *mut i64;
        std::ptr::write_bytes(ht as *mut u8, 0xFF, ht_bytes); // fill with -1
        (*map).keys = libc::malloc(bytes) as *mut i64;
        (*map).values = libc::malloc(bytes) as *mut i64;
        (*map).len = 0;
        (*map).cap = cap;
        (*map).version = 0;
        (*map).ht = ht;
        (*map).ht_cap = INITIAL_MAP_HT_CAP as i64;
        (*map).key_kind = kind;
        (*map).hash_fn = hash_fn;
        (*map).eq_fn = eq_fn;
    }
    map
}

unsafe fn map_rebuild_ht(map: *mut OriMap) {
    let ht_cap = (*map).ht_cap as usize;
    std::ptr::write_bytes(
        (*map).ht as *mut u8,
        0xFF,
        ht_cap * std::mem::size_of::<i64>(),
    );
    for i in 0..(*map).len as usize {
        let key = *(*map).keys.add(i);
        let slot = ht_find_insert_slot_map(map, key);
        *(*map).ht.add(slot) = i as i64;
    }
}

unsafe fn map_grow(map: *mut OriMap) {
    let new_ht_cap = (*map).ht_cap as usize * 2;
    let ht_bytes = new_ht_cap * std::mem::size_of::<i64>();
    let new_ht = libc::realloc((*map).ht as *mut libc::c_void, ht_bytes) as *mut i64;
    (*map).ht = new_ht;
    (*map).ht_cap = new_ht_cap as i64;
    map_rebuild_ht(map);
}

unsafe fn map_reserve_capacity(map: *mut OriMap, capacity: i64) {
    if map.is_null() || capacity <= (*map).cap {
        return;
    }
    let target_cap = capacity.max(1);
    let bytes = target_cap as usize * std::mem::size_of::<i64>();
    (*map).keys = libc::realloc((*map).keys as *mut libc::c_void, bytes) as *mut i64;
    (*map).values = libc::realloc((*map).values as *mut libc::c_void, bytes) as *mut i64;
    (*map).cap = target_cap;

    let mut target_ht_cap = INITIAL_MAP_HT_CAP;
    let required_slots = (target_cap as usize).saturating_mul(2).max(1);
    while target_ht_cap < required_slots {
        target_ht_cap = target_ht_cap.saturating_mul(2);
    }
    if target_ht_cap > (*map).ht_cap as usize {
        let ht_bytes = target_ht_cap * std::mem::size_of::<i64>();
        (*map).ht = libc::realloc((*map).ht as *mut libc::c_void, ht_bytes) as *mut i64;
        (*map).ht_cap = target_ht_cap as i64;
        map_rebuild_ht(map);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_new() -> *mut OriMap {
    map_alloc_with(std::ptr::null(), std::ptr::null(), MAP_KEY_UNKNOWN)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_new_custom(
    hash_fn: *const std::ffi::c_void,
    eq_fn: *const std::ffi::c_void,
) -> *mut OriMap {
    map_alloc_with(hash_fn, eq_fn, MAP_KEY_CUSTOM)
}

unsafe fn ori_map_set_raw(map: *mut OriMap, key: i64, value: i64, key_kind: u8) {
    if map.is_null() {
        return;
    }
    if !map_prepare_key_kind(map, key_kind) {
        return;
    }
    let existing = ht_lookup_map(map, key);
    if existing != usize::MAX {
        let dense_idx = *(*map).ht.add(existing) as usize;
        let old_value = *(*map).values.add(dense_idx);
        ori_arc_update_edge(map as *mut u8, old_value as *mut u8, value as *mut u8);
        *(*map).values.add(dense_idx) = value;
        (*map).version += 1;
        return;
    }
    // Grow dense arrays if needed
    if (*map).len >= (*map).cap {
        let new_cap = (*map).cap * 2;
        let bytes = new_cap as usize * std::mem::size_of::<i64>();
        (*map).keys = libc::realloc((*map).keys as *mut libc::c_void, bytes) as *mut i64;
        (*map).values = libc::realloc((*map).values as *mut libc::c_void, bytes) as *mut i64;
        (*map).cap = new_cap;
    }
    // Grow hash table at 50% load
    let ht_cap = (*map).ht_cap as usize;
    if (*map).len as usize * 2 >= ht_cap {
        map_grow(map);
    }
    let dense_idx = (*map).len as usize;
    *(*map).keys.add(dense_idx) = key;
    *(*map).values.add(dense_idx) = value;
    (*map).len += 1;
    let slot = ht_find_insert_slot_map(map, key);
    *(*map).ht.add(slot) = dense_idx as i64;
    (*map).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_set(map: *mut OriMap, key: i64, value: i64) {
    ori_map_set_raw(map, key, value, MAP_KEY_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_set_string(map: *mut OriMap, key: *const u8, value: i64) {
    ori_map_set_raw(map, key as i64, value, MAP_KEY_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_set_custom(map: *mut OriMap, key: i64, value: i64) {
    ori_map_set_raw(map, key, value, MAP_KEY_CUSTOM);
}

unsafe fn ori_map_get_raw(map: *mut OriMap, key: i64, key_kind: u8) -> i64 {
    if map.is_null() {
        return 0;
    }
    if (*map).key_kind != MAP_KEY_UNKNOWN && (*map).key_kind != key_kind {
        return 0;
    }
    let slot = ht_lookup_map(map, key);
    if slot == usize::MAX {
        return 0;
    }
    let dense_idx = *(*map).ht.add(slot) as usize;
    *(*map).values.add(dense_idx)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_get(map: *mut OriMap, key: i64) -> i64 {
    ori_map_get_raw(map, key, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_get_string(map: *mut OriMap, key: *const u8) -> i64 {
    ori_map_get_raw(map, key as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_get_custom(map: *mut OriMap, key: i64) -> i64 {
    ori_map_get_raw(map, key, MAP_KEY_CUSTOM)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_try_get(map: *mut OriMap, key: i64) -> *mut OriOptionalInt {
    if ori_map_contains(map, key) == 0 {
        alloc_optional_int(0, 0)
    } else {
        // The optional must own its managed payload via an edge so the
        // payload survives the map (scalar payloads are unaffected).
        alloc_optional_borrowed_managed_value(1, ori_map_get(map, key))
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_try_get_string(
    map: *mut OriMap,
    key: *const u8,
) -> *mut OriOptionalInt {
    if ori_map_contains_string(map, key) == 0 {
        alloc_optional_int(0, 0)
    } else {
        alloc_optional_borrowed_managed_value(1, ori_map_get_string(map, key))
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_try_get_custom(map: *mut OriMap, key: i64) -> *mut OriOptionalInt {
    if ori_map_contains_custom(map, key) == 0 {
        alloc_optional_int(0, 0)
    } else {
        alloc_optional_borrowed_managed_value(1, ori_map_get_custom(map, key))
    }
}

unsafe fn ori_map_contains_raw(map: *mut OriMap, key: i64, key_kind: u8) -> c_uchar {
    if map.is_null() {
        return 0;
    }
    if (*map).key_kind != MAP_KEY_UNKNOWN && (*map).key_kind != key_kind {
        return 0;
    }
    u8::from(ht_lookup_map(map, key) != usize::MAX) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_contains(map: *mut OriMap, key: i64) -> c_uchar {
    ori_map_contains_raw(map, key, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_contains_string(map: *mut OriMap, key: *const u8) -> c_uchar {
    ori_map_contains_raw(map, key as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_contains_custom(map: *mut OriMap, key: i64) -> c_uchar {
    ori_map_contains_raw(map, key, MAP_KEY_CUSTOM)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_is_empty(map: *mut OriMap) -> c_uchar {
    u8::from(ori_map_len(map) == 0) as c_uchar
}

unsafe fn ori_map_remove_raw(map: *mut OriMap, key: i64, key_kind: u8) {
    if map.is_null() {
        return;
    }
    if (*map).key_kind != MAP_KEY_UNKNOWN && (*map).key_kind != key_kind {
        return;
    }
    let slot = ht_lookup_map(map, key);
    if slot == usize::MAX {
        return;
    }
    let dense_idx = *(*map).ht.add(slot) as usize;
    let removed_key = *(*map).keys.add(dense_idx);
    let removed_value = *(*map).values.add(dense_idx);
    unregister_collection_edge(map as *mut u8, removed_key);
    unregister_collection_edge(map as *mut u8, removed_value);
    *(*map).ht.add(slot) = HT_TOMB;
    let last_idx = (*map).len as usize - 1;
    if dense_idx != last_idx {
        let last_key = *(*map).keys.add(last_idx);
        let last_val = *(*map).values.add(last_idx);
        *(*map).keys.add(dense_idx) = last_key;
        *(*map).values.add(dense_idx) = last_val;
        // Update hash slot that pointed to last_idx → now points to dense_idx
        let last_slot = ht_lookup_map(map, last_key);
        if last_slot != usize::MAX {
            *(*map).ht.add(last_slot) = dense_idx as i64;
        }
    }
    (*map).len -= 1;
    (*map).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_remove(map: *mut OriMap, key: i64) {
    ori_map_remove_raw(map, key, MAP_KEY_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_remove_string(map: *mut OriMap, key: *const u8) {
    ori_map_remove_raw(map, key as i64, MAP_KEY_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_remove_custom(map: *mut OriMap, key: i64) {
    ori_map_remove_raw(map, key, MAP_KEY_CUSTOM);
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_try_remove(map: *mut OriMap, key: i64) -> *mut OriOptionalInt {
    if ori_map_contains(map, key) == 0 {
        alloc_optional_int(0, 0)
    } else {
        let value = ori_map_get(map, key);
        ori_arc_retain(value as *mut u8);
        ori_map_remove(map, key);
        alloc_optional_owned_managed_value(1, value)
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_try_remove_string(
    map: *mut OriMap,
    key: *const u8,
) -> *mut OriOptionalInt {
    if ori_map_contains_string(map, key) == 0 {
        alloc_optional_int(0, 0)
    } else {
        let value = ori_map_get_string(map, key);
        ori_arc_retain(value as *mut u8);
        ori_map_remove_string(map, key);
        alloc_optional_owned_managed_value(1, value)
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_try_remove_custom(
    map: *mut OriMap,
    key: i64,
) -> *mut OriOptionalInt {
    if ori_map_contains_custom(map, key) == 0 {
        alloc_optional_int(0, 0)
    } else {
        let value = ori_map_get_custom(map, key);
        ori_arc_retain(value as *mut u8);
        ori_map_remove_custom(map, key);
        alloc_optional_owned_managed_value(1, value)
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_keys(map: *mut OriMap) -> *mut OriList {
    let out = ori_list_new();
    if map.is_null() {
        return out;
    }
    for i in 0..(*map).len {
        ori_list_push_borrowed_maybe_managed(out, *(*map).keys.add(i as usize));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_values(map: *mut OriMap) -> *mut OriList {
    let out = ori_list_new();
    if map.is_null() {
        return out;
    }
    for i in 0..(*map).len {
        ori_list_push_borrowed_maybe_managed(out, *(*map).values.add(i as usize));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_entries(map: *mut OriMap) -> *mut OriList {
    let out = ori_list_new();
    if map.is_null() {
        return out;
    }
    for i in 0..(*map).len {
        let tuple = ori_alloc(16, None) as *mut i64;
        if tuple.is_null() {
            continue;
        }
        *tuple.add(0) = *(*map).keys.add(i as usize);
        *tuple.add(1) = *(*map).values.add(i as usize);
        ori_arc_register_edge(tuple as *mut u8, *tuple.add(0) as *mut u8);
        ori_arc_register_edge(tuple as *mut u8, *tuple.add(1) as *mut u8);
        ori_list_push_owned_managed(out, tuple as *mut u8);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_len(map: *mut OriMap) -> i64 {
    if map.is_null() {
        0
    } else {
        (*map).len
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_capacity(map: *mut OriMap) -> i64 {
    if map.is_null() {
        0
    } else {
        (*map).cap
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_reserve(map: *mut OriMap, capacity: i64) {
    map_reserve_capacity(map, capacity);
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_key_at(map: *mut OriMap, index: i64) -> i64 {
    if map.is_null() || index < 0 || index >= (*map).len {
        return 0;
    }
    *(*map).keys.add(index as usize)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_value_at(map: *mut OriMap, index: i64) -> i64 {
    if map.is_null() || index < 0 || index >= (*map).len {
        return 0;
    }
    *(*map).values.add(index as usize)
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_clear(map: *mut OriMap) {
    if map.is_null() {
        return;
    }
    for i in 0..(*map).len {
        let key = *(*map).keys.add(i as usize);
        let value = *(*map).values.add(i as usize);
        unregister_collection_edge(map as *mut u8, key);
        unregister_collection_edge(map as *mut u8, value);
    }
    (*map).len = 0;
    (*map).key_kind = MAP_KEY_UNKNOWN;
    std::ptr::write_bytes(
        (*map).ht as *mut u8,
        0xFF,
        (*map).ht_cap as usize * std::mem::size_of::<i64>(),
    );
    (*map).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_clone(map: *mut OriMap) -> *mut OriMap {
    let out = ori_map_new();
    if map.is_null() {
        return out;
    }
    ori_map_reserve(out, (*map).len);
    for i in 0..(*map).len as usize {
        let key = *(*map).keys.add(i);
        let value = *(*map).values.add(i);
        let key_kind = if (*map).key_kind == MAP_KEY_UNKNOWN {
            MAP_KEY_INT
        } else {
            (*map).key_kind
        };
        ori_map_set_raw(out, key, value, key_kind);
        ori_map_register_borrowed_key_value_maybe_managed(out, key, value);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_from_entries(entries: *mut OriList) -> *mut OriMap {
    let out = ori_map_new();
    if entries.is_null() {
        return out;
    }
    for i in 0..(*entries).len as usize {
        let tuple = *(*entries).data.add(i) as *mut i64;
        if tuple.is_null() {
            continue;
        }
        let key = *tuple.add(0);
        let value = *tuple.add(1);
        ori_map_set(out, key, value);
        ori_map_register_borrowed_key_value_maybe_managed(out, key, value);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_from_entries_string(entries: *mut OriList) -> *mut OriMap {
    let out = ori_map_new();
    if entries.is_null() {
        return out;
    }
    for i in 0..(*entries).len as usize {
        let tuple = *(*entries).data.add(i) as *mut i64;
        if tuple.is_null() {
            continue;
        }
        let key = *tuple.add(0);
        let value = *tuple.add(1);
        ori_map_set_raw(out, key, value, MAP_KEY_STRING);
        ori_map_register_borrowed_key_value_maybe_managed(out, key, value);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_map_free(map: *mut OriMap) {
    ori_arc_release(map as *mut u8);
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_new() -> *mut OriMap {
    ori_map_new()
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_with_capacity(capacity: i64) -> *mut OriMap {
    let table = ori_map_new();
    ori_map_reserve(table, capacity);
    table
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_set(table: *mut OriMap, key: i64, value: i64) {
    ori_map_set(table, key, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_set_string(table: *mut OriMap, key: *const u8, value: i64) {
    ori_map_set_string(table, key, value);
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_get(table: *mut OriMap, key: i64) -> *mut OriOptionalInt {
    ori_map_try_get(table, key)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_get_string(
    table: *mut OriMap,
    key: *const u8,
) -> *mut OriOptionalInt {
    ori_map_try_get_string(table, key)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_remove(
    table: *mut OriMap,
    key: i64,
) -> *mut OriOptionalInt {
    ori_map_try_remove(table, key)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_remove_string(
    table: *mut OriMap,
    key: *const u8,
) -> *mut OriOptionalInt {
    ori_map_try_remove_string(table, key)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_contains(table: *mut OriMap, key: i64) -> c_uchar {
    ori_map_contains(table, key)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_contains_string(
    table: *mut OriMap,
    key: *const u8,
) -> c_uchar {
    ori_map_contains_string(table, key)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_is_empty(table: *mut OriMap) -> c_uchar {
    ori_map_is_empty(table)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_len(table: *mut OriMap) -> i64 {
    ori_map_len(table)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_capacity(table: *mut OriMap) -> i64 {
    ori_map_capacity(table)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_reserve(table: *mut OriMap, capacity: i64) {
    ori_map_reserve(table, capacity);
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_clear(table: *mut OriMap) {
    ori_map_clear(table);
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_clone(table: *mut OriMap) -> *mut OriMap {
    ori_map_clone(table)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_from_entries(entries: *mut OriList) -> *mut OriMap {
    ori_map_from_entries(entries)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_from_entries_string(entries: *mut OriList) -> *mut OriMap {
    ori_map_from_entries_string(entries)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_keys(table: *mut OriMap) -> *mut OriList {
    ori_map_keys(table)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_values(table: *mut OriMap) -> *mut OriList {
    ori_map_values(table)
}

#[no_mangle]
pub unsafe extern "C" fn ori_hash_table_entries(table: *mut OriMap) -> *mut OriList {
    ori_map_entries(table)
}

#[repr(C)]
pub struct OriGraph {
    nodes: *mut i64,
    len: i64,
    cap: i64,
    pub version: i64,
    edge_from: *mut i64,
    edge_to: *mut i64,
    edge_weight: *mut i64,
    edge_len: i64,
    edge_cap: i64,
    directed: c_uchar,
    node_kind: u8,
}

unsafe extern "C" fn ori_graph_dtor(ptr: *mut u8) {
    let graph = ptr as *mut OriGraph;
    if graph.is_null() {
        return;
    }
    if !(*graph).nodes.is_null() {
        libc::free((*graph).nodes as *mut libc::c_void);
    }
    if !(*graph).edge_from.is_null() {
        libc::free((*graph).edge_from as *mut libc::c_void);
    }
    if !(*graph).edge_to.is_null() {
        libc::free((*graph).edge_to as *mut libc::c_void);
    }
    if !(*graph).edge_weight.is_null() {
        libc::free((*graph).edge_weight as *mut libc::c_void);
    }
}

unsafe fn graph_alloc_array(cap: i64) -> *mut i64 {
    libc::calloc(cap as usize, std::mem::size_of::<i64>()) as *mut i64
}

unsafe fn graph_reserve_nodes(graph: *mut OriGraph, min_cap: i64) {
    if graph.is_null() || (*graph).cap >= min_cap {
        return;
    }
    let mut next_cap = if (*graph).cap <= 0 {
        8
    } else {
        (*graph).cap * 2
    };
    while next_cap < min_cap {
        next_cap *= 2;
    }
    let bytes = next_cap as usize * std::mem::size_of::<i64>();
    (*graph).nodes = libc::realloc((*graph).nodes as *mut libc::c_void, bytes) as *mut i64;
    (*graph).cap = next_cap;
}

unsafe fn graph_reserve_edges(graph: *mut OriGraph, min_cap: i64) {
    if graph.is_null() || (*graph).edge_cap >= min_cap {
        return;
    }
    let mut next_cap = if (*graph).edge_cap <= 0 {
        8
    } else {
        (*graph).edge_cap * 2
    };
    while next_cap < min_cap {
        next_cap *= 2;
    }
    let bytes = next_cap as usize * std::mem::size_of::<i64>();
    (*graph).edge_from = libc::realloc((*graph).edge_from as *mut libc::c_void, bytes) as *mut i64;
    (*graph).edge_to = libc::realloc((*graph).edge_to as *mut libc::c_void, bytes) as *mut i64;
    (*graph).edge_weight =
        libc::realloc((*graph).edge_weight as *mut libc::c_void, bytes) as *mut i64;
    (*graph).edge_cap = next_cap;
}

unsafe fn graph_prepare_kind(graph: *mut OriGraph, node_kind: u8) -> bool {
    if graph.is_null() {
        return false;
    }
    if (*graph).node_kind == MAP_KEY_UNKNOWN {
        (*graph).node_kind = node_kind;
        return true;
    }
    (*graph).node_kind == node_kind
}

unsafe fn graph_value_equals(kind: u8, stored: i64, query: i64) -> bool {
    if kind == MAP_KEY_STRING {
        cstr_key_bytes(stored) == cstr_key_bytes(query)
    } else {
        stored == query
    }
}

unsafe fn graph_find_node(graph: *mut OriGraph, node: i64, node_kind: u8) -> i64 {
    if graph.is_null() || ((*graph).node_kind != MAP_KEY_UNKNOWN && (*graph).node_kind != node_kind)
    {
        return -1;
    }
    let kind = if (*graph).node_kind == MAP_KEY_UNKNOWN {
        node_kind
    } else {
        (*graph).node_kind
    };
    for i in 0..(*graph).len {
        if graph_value_equals(kind, *(*graph).nodes.add(i as usize), node) {
            return i;
        }
    }
    -1
}

unsafe fn graph_edge_matches(
    graph: *mut OriGraph,
    index: i64,
    from: i64,
    to: i64,
    node_kind: u8,
) -> bool {
    let stored_from = *(*graph).edge_from.add(index as usize);
    let stored_to = *(*graph).edge_to.add(index as usize);
    graph_value_equals(node_kind, stored_from, from) && graph_value_equals(node_kind, stored_to, to)
}

unsafe fn graph_has_edge_raw(graph: *mut OriGraph, from: i64, to: i64, node_kind: u8) -> c_uchar {
    if graph.is_null() || ((*graph).node_kind != MAP_KEY_UNKNOWN && (*graph).node_kind != node_kind)
    {
        return 0;
    }
    let kind = if (*graph).node_kind == MAP_KEY_UNKNOWN {
        node_kind
    } else {
        (*graph).node_kind
    };
    for i in 0..(*graph).edge_len {
        if graph_edge_matches(graph, i, from, to, kind)
            || ((*graph).directed == 0 && graph_edge_matches(graph, i, to, from, kind))
        {
            return 1;
        }
    }
    0
}

unsafe fn graph_edge_index(graph: *mut OriGraph, from: i64, to: i64, node_kind: u8) -> i64 {
    if graph.is_null() || ((*graph).node_kind != MAP_KEY_UNKNOWN && (*graph).node_kind != node_kind)
    {
        return -1;
    }
    let kind = if (*graph).node_kind == MAP_KEY_UNKNOWN {
        node_kind
    } else {
        (*graph).node_kind
    };
    for i in 0..(*graph).edge_len {
        if graph_edge_matches(graph, i, from, to, kind)
            || ((*graph).directed == 0 && graph_edge_matches(graph, i, to, from, kind))
        {
            return i;
        }
    }
    -1
}

unsafe fn graph_add_node_raw(graph: *mut OriGraph, node: i64, node_kind: u8) {
    if !graph_prepare_kind(graph, node_kind) || graph_find_node(graph, node, node_kind) >= 0 {
        return;
    }
    graph_reserve_nodes(graph, (*graph).len + 1);
    *(*graph).nodes.add((*graph).len as usize) = node;
    (*graph).len += 1;
    (*graph).version += 1;
    ori_arc_register_edge(graph as *mut u8, node as *mut u8);
}

unsafe fn graph_add_edge_raw(graph: *mut OriGraph, from: i64, to: i64, node_kind: u8) {
    graph_add_weighted_edge_raw(graph, from, to, node_kind, 1);
}

unsafe fn graph_add_weighted_edge_raw(
    graph: *mut OriGraph,
    from: i64,
    to: i64,
    node_kind: u8,
    weight: i64,
) {
    if !graph_prepare_kind(graph, node_kind) {
        return;
    }
    graph_add_node_raw(graph, from, node_kind);
    graph_add_node_raw(graph, to, node_kind);
    let existing = graph_edge_index(graph, from, to, node_kind);
    if existing >= 0 {
        let old_weight = *(*graph).edge_weight.add(existing as usize);
        let new_weight = weight.max(0);
        if old_weight != new_weight {
            *(*graph).edge_weight.add(existing as usize) = new_weight;
            (*graph).version += 1;
        }
        return;
    }
    graph_reserve_edges(graph, (*graph).edge_len + 1);
    *(*graph).edge_from.add((*graph).edge_len as usize) = from;
    *(*graph).edge_to.add((*graph).edge_len as usize) = to;
    *(*graph).edge_weight.add((*graph).edge_len as usize) = weight.max(0);
    (*graph).edge_len += 1;
    (*graph).version += 1;
}

unsafe fn graph_remove_edge_at(graph: *mut OriGraph, index: i64) {
    if graph.is_null() || index < 0 || index >= (*graph).edge_len {
        return;
    }
    for i in index..((*graph).edge_len - 1) {
        let next = (i + 1) as usize;
        *(*graph).edge_from.add(i as usize) = *(*graph).edge_from.add(next);
        *(*graph).edge_to.add(i as usize) = *(*graph).edge_to.add(next);
        *(*graph).edge_weight.add(i as usize) = *(*graph).edge_weight.add(next);
    }
    (*graph).edge_len -= 1;
    (*graph).version += 1;
}

unsafe fn graph_remove_edge_raw(graph: *mut OriGraph, from: i64, to: i64, node_kind: u8) {
    if graph.is_null() || ((*graph).node_kind != MAP_KEY_UNKNOWN && (*graph).node_kind != node_kind)
    {
        return;
    }
    let kind = if (*graph).node_kind == MAP_KEY_UNKNOWN {
        node_kind
    } else {
        (*graph).node_kind
    };
    let mut i = 0;
    while i < (*graph).edge_len {
        if graph_edge_matches(graph, i, from, to, kind)
            || ((*graph).directed == 0 && graph_edge_matches(graph, i, to, from, kind))
        {
            graph_remove_edge_at(graph, i);
        } else {
            i += 1;
        }
    }
}

unsafe fn graph_remove_node_raw(graph: *mut OriGraph, node: i64, node_kind: u8) {
    let index = graph_find_node(graph, node, node_kind);
    if index < 0 {
        return;
    }
    let stored = *(*graph).nodes.add(index as usize);
    ori_arc_unregister_edge(graph as *mut u8, stored as *mut u8);
    for i in index..((*graph).len - 1) {
        *(*graph).nodes.add(i as usize) = *(*graph).nodes.add((i + 1) as usize);
    }
    (*graph).len -= 1;
    (*graph).version += 1;
    let mut edge = 0;
    let kind = (*graph).node_kind;
    while edge < (*graph).edge_len {
        let from = *(*graph).edge_from.add(edge as usize);
        let to = *(*graph).edge_to.add(edge as usize);
        if graph_value_equals(kind, from, node) || graph_value_equals(kind, to, node) {
            graph_remove_edge_at(graph, edge);
        } else {
            edge += 1;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_new(directed: c_uchar) -> *mut OriGraph {
    let graph = ori_alloc(std::mem::size_of::<OriGraph>(), Some(ori_graph_dtor)) as *mut OriGraph;
    if !graph.is_null() {
        (*graph).nodes = graph_alloc_array(8);
        (*graph).len = 0;
        (*graph).cap = 8;
        (*graph).version = 0;
        (*graph).edge_from = graph_alloc_array(8);
        (*graph).edge_to = graph_alloc_array(8);
        (*graph).edge_weight = graph_alloc_array(8);
        (*graph).edge_len = 0;
        (*graph).edge_cap = 8;
        (*graph).directed = u8::from(directed != 0) as c_uchar;
        (*graph).node_kind = MAP_KEY_UNKNOWN;
    }
    graph
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_add_node(graph: *mut OriGraph, node: i64) {
    graph_add_node_raw(graph, node, MAP_KEY_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_add_node_string(graph: *mut OriGraph, node: *const u8) {
    graph_add_node_raw(graph, node as i64, MAP_KEY_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_remove_node(graph: *mut OriGraph, node: i64) {
    graph_remove_node_raw(graph, node, MAP_KEY_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_remove_node_string(graph: *mut OriGraph, node: *const u8) {
    graph_remove_node_raw(graph, node as i64, MAP_KEY_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_add_edge(graph: *mut OriGraph, from: i64, to: i64) {
    graph_add_edge_raw(graph, from, to, MAP_KEY_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_add_edge_string(
    graph: *mut OriGraph,
    from: *const u8,
    to: *const u8,
) {
    graph_add_edge_raw(graph, from as i64, to as i64, MAP_KEY_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_add_weighted_edge(
    graph: *mut OriGraph,
    from: i64,
    to: i64,
    weight: i64,
) {
    graph_add_weighted_edge_raw(graph, from, to, MAP_KEY_INT, weight);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_add_weighted_edge_string(
    graph: *mut OriGraph,
    from: *const u8,
    to: *const u8,
    weight: i64,
) {
    graph_add_weighted_edge_raw(graph, from as i64, to as i64, MAP_KEY_STRING, weight);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_remove_edge(graph: *mut OriGraph, from: i64, to: i64) {
    graph_remove_edge_raw(graph, from, to, MAP_KEY_INT);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_remove_edge_string(
    graph: *mut OriGraph,
    from: *const u8,
    to: *const u8,
) {
    graph_remove_edge_raw(graph, from as i64, to as i64, MAP_KEY_STRING);
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_has_node(graph: *mut OriGraph, node: i64) -> c_uchar {
    u8::from(graph_find_node(graph, node, MAP_KEY_INT) >= 0) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_has_node_string(
    graph: *mut OriGraph,
    node: *const u8,
) -> c_uchar {
    u8::from(graph_find_node(graph, node as i64, MAP_KEY_STRING) >= 0) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_has_edge(graph: *mut OriGraph, from: i64, to: i64) -> c_uchar {
    graph_has_edge_raw(graph, from, to, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_has_edge_string(
    graph: *mut OriGraph,
    from: *const u8,
    to: *const u8,
) -> c_uchar {
    graph_has_edge_raw(graph, from as i64, to as i64, MAP_KEY_STRING)
}

unsafe fn graph_edge_weight_raw(
    graph: *mut OriGraph,
    from: i64,
    to: i64,
    node_kind: u8,
) -> *mut OriOptionalInt {
    let index = graph_edge_index(graph, from, to, node_kind);
    if index < 0 {
        return alloc_optional_int(0, 0);
    }
    alloc_optional_int(1, *(*graph).edge_weight.add(index as usize))
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_edge_weight(
    graph: *mut OriGraph,
    from: i64,
    to: i64,
) -> *mut OriOptionalInt {
    graph_edge_weight_raw(graph, from, to, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_edge_weight_string(
    graph: *mut OriGraph,
    from: *const u8,
    to: *const u8,
) -> *mut OriOptionalInt {
    graph_edge_weight_raw(graph, from as i64, to as i64, MAP_KEY_STRING)
}

unsafe fn graph_neighbors_raw(graph: *mut OriGraph, node: i64, node_kind: u8) -> *mut OriList {
    let out = ori_list_new();
    if graph_find_node(graph, node, node_kind) < 0 {
        return out;
    }
    let kind = (*graph).node_kind;
    for i in 0..(*graph).edge_len {
        let from = *(*graph).edge_from.add(i as usize);
        let to = *(*graph).edge_to.add(i as usize);
        if graph_value_equals(kind, from, node) {
            ori_list_push_borrowed_maybe_managed(out, to);
        } else if (*graph).directed == 0 && graph_value_equals(kind, to, node) {
            ori_list_push_borrowed_maybe_managed(out, from);
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_neighbors(graph: *mut OriGraph, node: i64) -> *mut OriList {
    graph_neighbors_raw(graph, node, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_neighbors_string(
    graph: *mut OriGraph,
    node: *const u8,
) -> *mut OriList {
    graph_neighbors_raw(graph, node as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_nodes(graph: *mut OriGraph) -> *mut OriList {
    let out = ori_list_new();
    if graph.is_null() {
        return out;
    }
    for i in 0..(*graph).len {
        ori_list_push_borrowed_maybe_managed(out, *(*graph).nodes.add(i as usize));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_edges(graph: *mut OriGraph) -> *mut OriList {
    let out = ori_list_new();
    if graph.is_null() {
        return out;
    }
    for i in 0..(*graph).edge_len {
        let tuple = ori_alloc(16, None) as *mut i64;
        if tuple.is_null() {
            continue;
        }
        *tuple.add(0) = *(*graph).edge_from.add(i as usize);
        *tuple.add(1) = *(*graph).edge_to.add(i as usize);
        ori_arc_register_edge(tuple as *mut u8, *tuple.add(0) as *mut u8);
        ori_arc_register_edge(tuple as *mut u8, *tuple.add(1) as *mut u8);
        ori_list_push_owned_managed(out, tuple as *mut u8);
    }
    out
}

unsafe fn graph_neighbor_indices(graph: *mut OriGraph, node_index: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let node = *(*graph).nodes.add(node_index);
    let kind = (*graph).node_kind;
    for edge in 0..(*graph).edge_len {
        let from = *(*graph).edge_from.add(edge as usize);
        let to = *(*graph).edge_to.add(edge as usize);
        if graph_value_equals(kind, from, node) {
            let idx = graph_find_node(graph, to, kind);
            if idx >= 0 {
                out.push(idx as usize);
            }
        } else if (*graph).directed == 0 && graph_value_equals(kind, to, node) {
            let idx = graph_find_node(graph, from, kind);
            if idx >= 0 {
                out.push(idx as usize);
            }
        }
    }
    out
}

unsafe fn graph_bfs_raw(graph: *mut OriGraph, start: i64, node_kind: u8) -> *mut OriList {
    let out = ori_list_new();
    let start_idx = graph_find_node(graph, start, node_kind);
    if graph.is_null() || start_idx < 0 {
        return out;
    }
    let mut visited = vec![false; (*graph).len as usize];
    let mut queue = VecDeque::new();
    visited[start_idx as usize] = true;
    queue.push_back(start_idx as usize);
    while let Some(index) = queue.pop_front() {
        ori_list_push_borrowed_maybe_managed(out, *(*graph).nodes.add(index));
        for next in graph_neighbor_indices(graph, index) {
            if !visited[next] {
                visited[next] = true;
                queue.push_back(next);
            }
        }
    }
    out
}

unsafe fn graph_dfs_raw(graph: *mut OriGraph, start: i64, node_kind: u8) -> *mut OriList {
    let out = ori_list_new();
    let start_idx = graph_find_node(graph, start, node_kind);
    if graph.is_null() || start_idx < 0 {
        return out;
    }
    let mut visited = vec![false; (*graph).len as usize];
    let mut stack = vec![start_idx as usize];
    while let Some(index) = stack.pop() {
        if visited[index] {
            continue;
        }
        visited[index] = true;
        ori_list_push_borrowed_maybe_managed(out, *(*graph).nodes.add(index));
        let mut neighbors = graph_neighbor_indices(graph, index);
        neighbors.reverse();
        for next in neighbors {
            if !visited[next] {
                stack.push(next);
            }
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_bfs(graph: *mut OriGraph, start: i64) -> *mut OriList {
    graph_bfs_raw(graph, start, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_bfs_string(
    graph: *mut OriGraph,
    start: *const u8,
) -> *mut OriList {
    graph_bfs_raw(graph, start as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_dfs(graph: *mut OriGraph, start: i64) -> *mut OriList {
    graph_dfs_raw(graph, start, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_dfs_string(
    graph: *mut OriGraph,
    start: *const u8,
) -> *mut OriList {
    graph_dfs_raw(graph, start as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_topological_sort(graph: *mut OriGraph) -> *mut OriList {
    let out = ori_list_new();
    if graph.is_null() || (*graph).directed == 0 {
        return out;
    }
    let len = (*graph).len as usize;
    let mut indegree = vec![0_i64; len];
    for edge in 0..(*graph).edge_len {
        let to = *(*graph).edge_to.add(edge as usize);
        let to_idx = graph_find_node(graph, to, (*graph).node_kind);
        if to_idx >= 0 {
            indegree[to_idx as usize] += 1;
        }
    }
    let mut queue = VecDeque::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        ori_list_push_borrowed_maybe_managed(out, *(*graph).nodes.add(index));
        for next in graph_neighbor_indices(graph, index) {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }
    if (*out).len != (*graph).len {
        (*out).len = 0;
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_is_directed(graph: *mut OriGraph) -> c_uchar {
    if graph.is_null() {
        0
    } else {
        u8::from((*graph).directed != 0) as c_uchar
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_len(graph: *mut OriGraph) -> i64 {
    if graph.is_null() {
        0
    } else {
        (*graph).len
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_edge_len(graph: *mut OriGraph) -> i64 {
    if graph.is_null() {
        0
    } else {
        (*graph).edge_len
    }
}

unsafe fn graph_has_cycle_directed(graph: *mut OriGraph, index: usize, state: &mut [u8]) -> bool {
    state[index] = 1;
    for next in graph_neighbor_indices(graph, index) {
        if state[next] == 1 || (state[next] == 0 && graph_has_cycle_directed(graph, next, state)) {
            return true;
        }
    }
    state[index] = 2;
    false
}

unsafe fn graph_has_cycle_undirected(
    graph: *mut OriGraph,
    index: usize,
    parent: Option<usize>,
    visited: &mut [bool],
) -> bool {
    visited[index] = true;
    for next in graph_neighbor_indices(graph, index) {
        if !visited[next] {
            if graph_has_cycle_undirected(graph, next, Some(index), visited) {
                return true;
            }
        } else if Some(next) != parent {
            return true;
        }
    }
    false
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_has_cycle(graph: *mut OriGraph) -> c_uchar {
    if graph.is_null() {
        return 0;
    }
    let len = (*graph).len as usize;
    if (*graph).directed != 0 {
        let mut state = vec![0_u8; len];
        for index in 0..len {
            if state[index] == 0 && graph_has_cycle_directed(graph, index, &mut state) {
                return 1;
            }
        }
        return 0;
    }
    let mut visited = vec![false; len];
    for index in 0..len {
        if !visited[index] && graph_has_cycle_undirected(graph, index, None, &mut visited) {
            return 1;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_try_topological_sort(
    graph: *mut OriGraph,
) -> *mut OriOptionalInt {
    if graph.is_null() || (*graph).directed == 0 || ori_graph_has_cycle(graph) != 0 {
        return alloc_optional_int(0, 0);
    }
    let out = ori_graph_topological_sort(graph);
    alloc_optional_owned_managed_value(1, out as i64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_clone(graph: *mut OriGraph) -> *mut OriGraph {
    if graph.is_null() {
        return ori_graph_new(1);
    }
    let out = ori_graph_new((*graph).directed);
    if out.is_null() {
        return out;
    }
    (*out).node_kind = (*graph).node_kind;
    for i in 0..(*graph).len {
        let node = *(*graph).nodes.add(i as usize);
        graph_add_node_raw(out, node, (*graph).node_kind);
    }
    for i in 0..(*graph).edge_len {
        graph_add_weighted_edge_raw(
            out,
            *(*graph).edge_from.add(i as usize),
            *(*graph).edge_to.add(i as usize),
            (*graph).node_kind,
            *(*graph).edge_weight.add(i as usize),
        );
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_components(graph: *mut OriGraph) -> *mut OriList {
    let out = ori_list_new();
    if graph.is_null() {
        return out;
    }
    let len = (*graph).len as usize;
    let mut visited = vec![false; len];
    for start in 0..len {
        if visited[start] {
            continue;
        }
        let component = ori_list_new();
        let mut queue = VecDeque::new();
        visited[start] = true;
        queue.push_back(start);
        while let Some(index) = queue.pop_front() {
            ori_list_push_borrowed_maybe_managed(component, *(*graph).nodes.add(index));
            for next in graph_neighbor_indices(graph, index) {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        ori_list_push_owned_managed(out, component as *mut u8);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_strongly_connected_components(
    graph: *mut OriGraph,
) -> *mut OriList {
    let out = ori_list_new();
    if graph.is_null() {
        return out;
    }
    let len = (*graph).len as usize;
    let mut index_counter = 0_i64;
    let mut stack: Vec<usize> = Vec::new();
    let mut on_stack = vec![false; len];
    let mut indices = vec![-1_i64; len];
    let mut lowlink = vec![0_i64; len];

    unsafe fn strongconnect(
        graph: *mut OriGraph,
        vertex: usize,
        index_counter: &mut i64,
        stack: &mut Vec<usize>,
        on_stack: &mut [bool],
        indices: &mut [i64],
        lowlink: &mut [i64],
        out: *mut OriList,
    ) {
        indices[vertex] = *index_counter;
        lowlink[vertex] = *index_counter;
        *index_counter += 1;
        stack.push(vertex);
        on_stack[vertex] = true;

        for next in graph_neighbor_indices(graph, vertex) {
            if indices[next] < 0 {
                strongconnect(
                    graph,
                    next,
                    index_counter,
                    stack,
                    on_stack,
                    indices,
                    lowlink,
                    out,
                );
                lowlink[vertex] = lowlink[vertex].min(lowlink[next]);
            } else if on_stack[next] {
                lowlink[vertex] = lowlink[vertex].min(indices[next]);
            }
        }

        if lowlink[vertex] == indices[vertex] {
            let component = ori_list_new();
            loop {
                let Some(next) = stack.pop() else {
                    break;
                };
                on_stack[next] = false;
                ori_list_push_borrowed_maybe_managed(component, *(*graph).nodes.add(next));
                if next == vertex {
                    break;
                }
            }
            ori_list_push_owned_managed(out, component as *mut u8);
        }
    }

    for vertex in 0..len {
        if indices[vertex] < 0 {
            strongconnect(
                graph,
                vertex,
                &mut index_counter,
                &mut stack,
                &mut on_stack,
                &mut indices,
                &mut lowlink,
                out,
            );
        }
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_transitive_closure(graph: *mut OriGraph) -> *mut OriGraph {
    if graph.is_null() {
        return ori_graph_new(1);
    }
    let out = ori_graph_new(1);
    if out.is_null() {
        return out;
    }
    (*out).node_kind = (*graph).node_kind;
    for i in 0..(*graph).len {
        let node = *(*graph).nodes.add(i as usize);
        graph_add_node_raw(out, node, (*graph).node_kind);
    }
    for start in 0..(*graph).len as usize {
        let mut visited = vec![false; (*graph).len as usize];
        let mut queue = VecDeque::new();
        visited[start] = true;
        queue.push_back(start);
        while let Some(index) = queue.pop_front() {
            for next in graph_neighbor_indices(graph, index) {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                    graph_add_edge_raw(
                        out,
                        *(*graph).nodes.add(start),
                        *(*graph).nodes.add(next),
                        (*graph).node_kind,
                    );
                }
            }
        }
    }
    out
}

unsafe fn graph_neighbor_weighted_indices(
    graph: *mut OriGraph,
    node_index: usize,
) -> Vec<(usize, i64)> {
    let mut out = Vec::new();
    let node = *(*graph).nodes.add(node_index);
    let kind = (*graph).node_kind;
    for edge in 0..(*graph).edge_len {
        let from = *(*graph).edge_from.add(edge as usize);
        let to = *(*graph).edge_to.add(edge as usize);
        let weight = *(*graph).edge_weight.add(edge as usize);
        if graph_value_equals(kind, from, node) {
            let idx = graph_find_node(graph, to, kind);
            if idx >= 0 {
                out.push((idx as usize, weight));
            }
        } else if (*graph).directed == 0 && graph_value_equals(kind, to, node) {
            let idx = graph_find_node(graph, from, kind);
            if idx >= 0 {
                out.push((idx as usize, weight));
            }
        }
    }
    out
}

unsafe fn graph_shortest_path_raw(
    graph: *mut OriGraph,
    start: i64,
    goal: i64,
    node_kind: u8,
) -> *mut OriOptionalInt {
    let start_idx = graph_find_node(graph, start, node_kind);
    let goal_idx = graph_find_node(graph, goal, node_kind);
    if graph.is_null() || start_idx < 0 || goal_idx < 0 {
        return alloc_optional_int(0, 0);
    }
    let len = (*graph).len as usize;
    let mut visited = vec![false; len];
    let mut previous: Vec<Option<usize>> = vec![None; len];
    let mut queue = VecDeque::new();
    visited[start_idx as usize] = true;
    queue.push_back(start_idx as usize);
    while let Some(index) = queue.pop_front() {
        if index == goal_idx as usize {
            break;
        }
        for next in graph_neighbor_indices(graph, index) {
            if !visited[next] {
                visited[next] = true;
                previous[next] = Some(index);
                queue.push_back(next);
            }
        }
    }
    if !visited[goal_idx as usize] {
        return alloc_optional_int(0, 0);
    }
    let mut reversed = Vec::new();
    let mut cursor = goal_idx as usize;
    reversed.push(cursor);
    while cursor != start_idx as usize {
        let Some(prev) = previous[cursor] else {
            return alloc_optional_int(0, 0);
        };
        cursor = prev;
        reversed.push(cursor);
    }
    reversed.reverse();
    let path = ori_list_new();
    for index in reversed {
        ori_list_push_borrowed_maybe_managed(path, *(*graph).nodes.add(index));
    }
    alloc_optional_owned_managed_value(1, path as i64)
}

unsafe fn graph_shortest_weighted_path_raw(
    graph: *mut OriGraph,
    start: i64,
    goal: i64,
    node_kind: u8,
) -> *mut OriOptionalInt {
    let start_idx = graph_find_node(graph, start, node_kind);
    let goal_idx = graph_find_node(graph, goal, node_kind);
    if graph.is_null() || start_idx < 0 || goal_idx < 0 {
        return alloc_optional_int(0, 0);
    }
    let len = (*graph).len as usize;
    let mut dist = vec![i64::MAX; len];
    let mut previous: Vec<Option<usize>> = vec![None; len];
    let mut visited = vec![false; len];
    dist[start_idx as usize] = 0;

    for _ in 0..len {
        let mut current: Option<usize> = None;
        for index in 0..len {
            if !visited[index]
                && current
                    .map(|candidate| dist[index] < dist[candidate])
                    .unwrap_or(true)
            {
                current = Some(index);
            }
        }
        let Some(index) = current else {
            break;
        };
        if dist[index] == i64::MAX || index == goal_idx as usize {
            break;
        }
        visited[index] = true;
        for (next, weight) in graph_neighbor_weighted_indices(graph, index) {
            if visited[next] {
                continue;
            }
            let Some(candidate) = dist[index].checked_add(weight.max(0)) else {
                continue;
            };
            if candidate < dist[next] {
                dist[next] = candidate;
                previous[next] = Some(index);
            }
        }
    }

    if dist[goal_idx as usize] == i64::MAX {
        return alloc_optional_int(0, 0);
    }
    let mut reversed = Vec::new();
    let mut cursor = goal_idx as usize;
    reversed.push(cursor);
    while cursor != start_idx as usize {
        let Some(prev) = previous[cursor] else {
            return alloc_optional_int(0, 0);
        };
        cursor = prev;
        reversed.push(cursor);
    }
    reversed.reverse();
    let path = ori_list_new();
    for index in reversed {
        ori_list_push_borrowed_maybe_managed(path, *(*graph).nodes.add(index));
    }
    alloc_optional_owned_managed_value(1, path as i64)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_shortest_path(
    graph: *mut OriGraph,
    start: i64,
    goal: i64,
) -> *mut OriOptionalInt {
    graph_shortest_path_raw(graph, start, goal, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_shortest_path_string(
    graph: *mut OriGraph,
    start: *const u8,
    goal: *const u8,
) -> *mut OriOptionalInt {
    graph_shortest_path_raw(graph, start as i64, goal as i64, MAP_KEY_STRING)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_shortest_weighted_path(
    graph: *mut OriGraph,
    start: i64,
    goal: i64,
) -> *mut OriOptionalInt {
    graph_shortest_weighted_path_raw(graph, start, goal, MAP_KEY_INT)
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_shortest_weighted_path_string(
    graph: *mut OriGraph,
    start: *const u8,
    goal: *const u8,
) -> *mut OriOptionalInt {
    graph_shortest_weighted_path_raw(graph, start as i64, goal as i64, MAP_KEY_STRING)
}

#[repr(C)]
pub struct OriHeap {
    data: *mut i64,
    len: i64,
    cap: i64,
    pub version: i64,
    item_kind: u8,
    compare_fn: *const std::ffi::c_void,
}

const HEAP_ITEM_INT: u8 = 1;
const HEAP_ITEM_STRING: u8 = 2;
const HEAP_ITEM_CUSTOM: u8 = 3;
const HEAP_ITEM_UNKNOWN: u8 = 0;

type HeapCompareFn = unsafe extern "C" fn(i64, i64) -> i64;

unsafe extern "C" fn ori_heap_dtor(ptr: *mut u8) {
    let heap = ptr as *mut OriHeap;
    if heap.is_null() {
        return;
    }
    if !(*heap).data.is_null() {
        libc::free((*heap).data as *mut libc::c_void);
    }
}

unsafe fn heap_new_with(kind: u8, compare_fn: *const std::ffi::c_void) -> *mut OriHeap {
    let heap = ori_alloc(std::mem::size_of::<OriHeap>(), Some(ori_heap_dtor)) as *mut OriHeap;
    if heap.is_null() {
        return heap;
    }
    let cap = 8_i64;
    (*heap).data = libc::calloc(cap as usize, std::mem::size_of::<i64>()) as *mut i64;
    (*heap).len = 0;
    (*heap).cap = cap;
    (*heap).version = 0;
    (*heap).item_kind = kind;
    (*heap).compare_fn = compare_fn;
    heap
}

unsafe fn heap_compare(heap: *mut OriHeap, left: i64, right: i64) -> i64 {
    if heap.is_null() {
        return match left.cmp(&right) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        };
    }
    match (*heap).item_kind {
        HEAP_ITEM_STRING => match cstr_str(left as *const u8).cmp(cstr_str(right as *const u8)) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
        HEAP_ITEM_CUSTOM if !(*heap).compare_fn.is_null() => {
            let compare: HeapCompareFn = std::mem::transmute((*heap).compare_fn);
            ori_arc_retain(left as *mut u8);
            ori_arc_retain(right as *mut u8);
            let result = compare(left, right);
            ori_arc_release(left as *mut u8);
            ori_arc_release(right as *mut u8);
            result
        }
        _ => match left.cmp(&right) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
    }
}

unsafe fn heap_reserve(heap: *mut OriHeap, min_cap: i64) {
    if heap.is_null() || (*heap).cap >= min_cap {
        return;
    }
    let mut next_cap = if (*heap).cap <= 0 { 8 } else { (*heap).cap * 2 };
    while next_cap < min_cap {
        next_cap *= 2;
    }
    let bytes = next_cap as usize * std::mem::size_of::<i64>();
    (*heap).data = libc::realloc((*heap).data as *mut libc::c_void, bytes) as *mut i64;
    (*heap).cap = next_cap;
}

unsafe fn heap_sift_up(heap: *mut OriHeap, mut index: i64) {
    while index > 0 {
        let parent = (index - 1) / 2;
        let value = *(*heap).data.add(index as usize);
        let parent_value = *(*heap).data.add(parent as usize);
        if heap_compare(heap, parent_value, value) <= 0 {
            break;
        }
        *(*heap).data.add(index as usize) = parent_value;
        *(*heap).data.add(parent as usize) = value;
        index = parent;
    }
}

unsafe fn heap_sift_down(heap: *mut OriHeap, mut index: i64) {
    loop {
        let left = index * 2 + 1;
        let right = left + 1;
        let mut smallest = index;
        if left < (*heap).len {
            let left_value = *(*heap).data.add(left as usize);
            let current = *(*heap).data.add(smallest as usize);
            if heap_compare(heap, left_value, current) < 0 {
                smallest = left;
            }
        }
        if right < (*heap).len {
            let right_value = *(*heap).data.add(right as usize);
            let current = *(*heap).data.add(smallest as usize);
            if heap_compare(heap, right_value, current) < 0 {
                smallest = right;
            }
        }
        if smallest == index {
            break;
        }
        let value = *(*heap).data.add(index as usize);
        let next = *(*heap).data.add(smallest as usize);
        *(*heap).data.add(index as usize) = next;
        *(*heap).data.add(smallest as usize) = value;
        index = smallest;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_new() -> *mut OriHeap {
    heap_new_with(HEAP_ITEM_UNKNOWN, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_new_string() -> *mut OriHeap {
    heap_new_with(HEAP_ITEM_STRING, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_new_custom(compare_fn: *const std::ffi::c_void) -> *mut OriHeap {
    heap_new_with(HEAP_ITEM_CUSTOM, compare_fn)
}

unsafe fn heap_prepare_kind(
    heap: *mut OriHeap,
    item_kind: u8,
    compare_fn: *const std::ffi::c_void,
) -> bool {
    if heap.is_null() {
        return false;
    }
    if (*heap).item_kind == HEAP_ITEM_UNKNOWN {
        (*heap).item_kind = item_kind;
        (*heap).compare_fn = compare_fn;
        return true;
    }
    (*heap).item_kind == item_kind
}

unsafe fn heap_push_raw(
    heap: *mut OriHeap,
    value: i64,
    item_kind: u8,
    compare_fn: *const std::ffi::c_void,
) {
    if !heap_prepare_kind(heap, item_kind, compare_fn) {
        return;
    }
    heap_reserve(heap, (*heap).len + 1);
    *(*heap).data.add((*heap).len as usize) = value;
    (*heap).len += 1;
    (*heap).version += 1;
    heap_sift_up(heap, (*heap).len - 1);
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_push(heap: *mut OriHeap, value: i64) {
    heap_push_raw(heap, value, HEAP_ITEM_INT, std::ptr::null());
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_push_string(heap: *mut OriHeap, value: *const u8) {
    heap_push_raw(heap, value as i64, HEAP_ITEM_STRING, std::ptr::null());
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_push_custom(
    heap: *mut OriHeap,
    value: i64,
    compare_fn: *const std::ffi::c_void,
) {
    // Register ARC edge before pushing to prevent the element from being
    // collected during heap_sift_up comparisons (which call retain/release).
    ori_arc_register_edge(heap as *mut u8, value as *mut u8);
    heap_push_raw(heap, value, HEAP_ITEM_CUSTOM, compare_fn);
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_pop(heap: *mut OriHeap) -> *mut OriOptionalInt {
    if heap.is_null() || (*heap).len == 0 {
        return alloc_optional_int(0, 0);
    }
    let root = *(*heap).data;
    (*heap).len -= 1;
    (*heap).version += 1;
    if (*heap).len > 0 {
        *(*heap).data = *(*heap).data.add((*heap).len as usize);
        heap_sift_down(heap, 0);
    }
    heap_alloc_optional_value(heap, 1, root, true)
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_peek(heap: *mut OriHeap) -> *mut OriOptionalInt {
    if heap.is_null() || (*heap).len == 0 {
        return alloc_optional_int(0, 0);
    }
    heap_alloc_optional_value(heap, 1, *(*heap).data, false)
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_len(heap: *mut OriHeap) -> i64 {
    if heap.is_null() {
        0
    } else {
        (*heap).len
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_is_empty(heap: *mut OriHeap) -> c_uchar {
    u8::from(heap.is_null() || (*heap).len == 0) as c_uchar
}

unsafe fn heap_push_borrowed_maybe_managed(
    heap: *mut OriHeap,
    value: i64,
    item_kind: u8,
    compare_fn: *const std::ffi::c_void,
) {
    ori_arc_register_edge(heap as *mut u8, value as *mut u8);
    heap_push_raw(heap, value, item_kind, compare_fn);
}

unsafe fn heap_alloc_optional_value(
    heap: *mut OriHeap,
    has_value: c_uchar,
    value: i64,
    transfer_from_heap: bool,
) -> *mut OriOptionalInt {
    if has_value == 0 || heap.is_null() {
        return alloc_optional_int(has_value, value);
    }
    match (*heap).item_kind {
        HEAP_ITEM_STRING | HEAP_ITEM_CUSTOM => {
            if transfer_from_heap {
                let value = transfer_collection_edge_to_return_value(heap as *mut u8, value);
                alloc_optional_owned_managed_value(1, value)
            } else {
                alloc_optional_borrowed_managed_value(1, value)
            }
        }
        _ => alloc_optional_int(has_value, value),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_clear(heap: *mut OriHeap) {
    if heap.is_null() {
        return;
    }
    for i in 0..(*heap).len {
        ori_arc_unregister_edge(heap as *mut u8, *(*heap).data.add(i as usize) as *mut u8);
    }
    (*heap).len = 0;
    (*heap).version += 1;
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_to_list(heap: *mut OriHeap) -> *mut OriList {
    let out = ori_list_new();
    if heap.is_null() {
        return out;
    }
    for i in 0..(*heap).len {
        ori_list_push_borrowed_maybe_managed(out, *(*heap).data.add(i as usize));
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_clone(heap: *mut OriHeap) -> *mut OriHeap {
    if heap.is_null() {
        return ori_heap_new();
    }
    let out = heap_new_with((*heap).item_kind, (*heap).compare_fn);
    if out.is_null() {
        return out;
    }
    for i in 0..(*heap).len {
        heap_push_borrowed_maybe_managed(
            out,
            *(*heap).data.add(i as usize),
            (*heap).item_kind,
            (*heap).compare_fn,
        );
    }
    out
}

unsafe fn heap_from_list_raw(
    list: *mut OriList,
    item_kind: u8,
    compare_fn: *const std::ffi::c_void,
) -> *mut OriHeap {
    let out = heap_new_with(item_kind, compare_fn);
    if list.is_null() || out.is_null() {
        return out;
    }
    for i in 0..(*list).len {
        heap_push_borrowed_maybe_managed(out, *(*list).data.add(i as usize), item_kind, compare_fn);
    }
    out
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_from_list(list: *mut OriList) -> *mut OriHeap {
    heap_from_list_raw(list, HEAP_ITEM_INT, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_from_list_string(list: *mut OriList) -> *mut OriHeap {
    heap_from_list_raw(list, HEAP_ITEM_STRING, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_from_list_custom(
    list: *mut OriList,
    compare_fn: *const std::ffi::c_void,
) -> *mut OriHeap {
    heap_from_list_raw(list, HEAP_ITEM_CUSTOM, compare_fn)
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_merge(left: *mut OriHeap, right: *mut OriHeap) -> *mut OriHeap {
    if left.is_null() && right.is_null() {
        return ori_heap_new();
    }
    let seed = if left.is_null() { right } else { left };
    let out = heap_new_with((*seed).item_kind, (*seed).compare_fn);
    if out.is_null() {
        return out;
    }
    for source in [left, right] {
        if source.is_null() {
            continue;
        }
        for i in 0..(*source).len {
            heap_push_borrowed_maybe_managed(
                out,
                *(*source).data.add(i as usize),
                (*source).item_kind,
                (*source).compare_fn,
            );
        }
    }
    out
}

unsafe fn heap_remove_raw(
    heap: *mut OriHeap,
    value: i64,
    item_kind: u8,
    compare_fn: *const std::ffi::c_void,
) -> c_uchar {
    if heap.is_null() || !heap_prepare_kind(heap, item_kind, compare_fn) {
        return 0;
    }
    for index in 0..(*heap).len {
        let current = *(*heap).data.add(index as usize);
        if heap_compare(heap, current, value) != 0 {
            continue;
        }
        ori_arc_unregister_edge(heap as *mut u8, current as *mut u8);
        (*heap).len -= 1;
        if index != (*heap).len {
            *(*heap).data.add(index as usize) = *(*heap).data.add((*heap).len as usize);
            heap_sift_down(heap, index);
            heap_sift_up(heap, index);
        }
        return 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_remove(heap: *mut OriHeap, value: i64) -> c_uchar {
    heap_remove_raw(heap, value, HEAP_ITEM_INT, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_remove_string(heap: *mut OriHeap, value: *const u8) -> c_uchar {
    heap_remove_raw(heap, value as i64, HEAP_ITEM_STRING, std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_remove_custom(
    heap: *mut OriHeap,
    value: i64,
    compare_fn: *const std::ffi::c_void,
) -> c_uchar {
    heap_remove_raw(heap, value, HEAP_ITEM_CUSTOM, compare_fn)
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_into_sorted_list(heap: *mut OriHeap) -> *mut OriList {
    let out = ori_list_new();
    let work = ori_heap_clone(heap);
    if work.is_null() {
        return out;
    }
    loop {
        if (*work).len <= 0 {
            break;
        }
        let item = ori_heap_pop(work);
        if !item.is_null() && (*item).has_value != 0 {
            ori_list_push_borrowed_maybe_managed(out, (*item).value);
        }
        ori_arc_release(item as *mut u8);
    }
    ori_arc_release(work as *mut u8);
    out
}

#[no_mangle]
pub extern "C" fn ori_math_sqrt(value: f64) -> f64 {
    value.sqrt()
}

#[no_mangle]
pub extern "C" fn ori_math_pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

#[no_mangle]
pub extern "C" fn ori_math_floor(value: f64) -> i64 {
    value.floor() as i64
}

#[no_mangle]
pub extern "C" fn ori_math_ceil(value: f64) -> i64 {
    value.ceil() as i64
}

#[no_mangle]
pub extern "C" fn ori_math_round(value: f64) -> i64 {
    value.round() as i64
}

#[no_mangle]
pub extern "C" fn ori_math_log(value: f64) -> f64 {
    value.ln()
}

#[no_mangle]
pub extern "C" fn ori_math_log2(value: f64) -> f64 {
    value.log2()
}

#[no_mangle]
pub extern "C" fn ori_math_sin(value: f64) -> f64 {
    value.sin()
}

#[no_mangle]
pub extern "C" fn ori_math_cos(value: f64) -> f64 {
    value.cos()
}

#[no_mangle]
pub extern "C" fn ori_math_tan(value: f64) -> f64 {
    value.tan()
}

#[no_mangle]
pub extern "C" fn ori_math_abs(value: i64) -> i64 {
    value.abs()
}

#[no_mangle]
pub extern "C" fn ori_math_abs_float(value: f64) -> f64 {
    value.abs()
}

#[no_mangle]
pub extern "C" fn ori_math_min(a: i64, b: i64) -> i64 {
    a.min(b)
}

#[no_mangle]
pub extern "C" fn ori_math_min_float(a: f64, b: f64) -> f64 {
    a.min(b)
}

#[no_mangle]
pub extern "C" fn ori_math_max(a: i64, b: i64) -> i64 {
    a.max(b)
}

#[no_mangle]
pub extern "C" fn ori_math_max_float(a: f64, b: f64) -> f64 {
    a.max(b)
}

#[no_mangle]
pub extern "C" fn ori_math_clamp(value: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
}

#[no_mangle]
pub extern "C" fn ori_math_is_nan(value: f64) -> c_uchar {
    u8::from(value.is_nan()) as c_uchar
}

#[no_mangle]
pub extern "C" fn ori_math_is_infinite(value: f64) -> c_uchar {
    u8::from(value.is_infinite()) as c_uchar
}

#[no_mangle]
pub extern "C" fn ori_time_now() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    millis.min(i64::MAX as u128) as i64
}

#[no_mangle]
pub extern "C" fn ori_time_sleep(millis: i64) {
    if millis > 0 {
        std::thread::sleep(Duration::from_millis(millis as u64));
    }
}

#[no_mangle]
pub extern "C" fn ori_time_duration_ms(start: i64, end: i64) -> i64 {
    end - start
}

#[no_mangle]
pub extern "C" fn ori_format_number(value: f64, decimals: i64) -> *mut u8 {
    cstring_from_str(&format_float_fixed(value, decimals))
}

#[no_mangle]
pub extern "C" fn ori_format_percent(value: f64, decimals: i64) -> *mut u8 {
    cstring_from_str(&format!("{}%", format_float_fixed(value * 100.0, decimals)))
}

#[no_mangle]
pub extern "C" fn ori_format_hex(value: i64) -> *mut u8 {
    cstring_from_str(&format!("{value:x}"))
}

#[no_mangle]
pub extern "C" fn ori_format_binary(value: i64) -> *mut u8 {
    cstring_from_str(&format!("{value:b}"))
}

#[no_mangle]
pub unsafe extern "C" fn ori_format_date(millis: i64, style: *const u8) -> *mut u8 {
    let _style = cstr_str(style);
    let (year, month, day, _, _, _) = utc_parts_from_millis(millis);
    cstring_from_str(&format!("{year:04}-{month:02}-{day:02}"))
}

#[no_mangle]
pub unsafe extern "C" fn ori_format_datetime(
    millis: i64,
    style: *const u8,
    locale: *const u8,
) -> *mut u8 {
    let _style = cstr_str(style);
    let _locale = cstr_str(locale);
    let (year, month, day, hour, minute, second) = utc_parts_from_millis(millis);
    cstring_from_str(&format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

#[no_mangle]
pub unsafe extern "C" fn ori_format_bytes_size(bytes: i64, style: *const u8) -> *mut u8 {
    let binary = cstr_str(style).eq_ignore_ascii_case("binary");
    let units = if binary {
        ["B", "KiB", "MiB", "GiB", "TiB"]
    } else {
        ["B", "KB", "MB", "GB", "TB"]
    };
    let base = if binary { 1024.0 } else { 1000.0 };
    let sign = if bytes < 0 { "-" } else { "" };
    let mut value = (bytes as f64).abs();
    let mut unit = 0usize;
    while value >= base && unit + 1 < units.len() {
        value /= base;
        unit += 1;
    }
    let text = if unit == 0 {
        format!("{sign}{} {}", value as i64, units[unit])
    } else {
        format!("{sign}{value:.1} {}", units[unit])
    };
    cstring_from_str(&text)
}

#[no_mangle]
pub unsafe extern "C" fn ori_os_set_args(_argc: i32, _argv: *mut *mut c_char) {}

#[no_mangle]
pub unsafe extern "C" fn ori_os_args() -> *mut OriList {
    let list = ori_list_new();
    for arg in std::env::args() {
        ori_list_push_owned_managed(list, cstring_from_str(&arg));
    }
    list
}

#[no_mangle]
pub unsafe extern "C" fn ori_os_env(name: *const u8) -> *mut u8 {
    match std::env::var(cstr_str(name)) {
        Ok(value) => new_optional_ptr(true, cstring_from_str(&value)),
        Err(_) => new_optional_ptr(false, std::ptr::null_mut()),
    }
}

#[no_mangle]
pub extern "C" fn ori_os_exit(code: i64) {
    std::process::exit(code as i32);
}

#[no_mangle]
pub extern "C" fn ori_os_pid() -> i64 {
    i64::from(std::process::id())
}

#[no_mangle]
pub extern "C" fn ori_os_platform() -> *mut u8 {
    let name = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        _ => "unknown",
    };
    cstring_from_str(name)
}

#[no_mangle]
pub extern "C" fn ori_os_arch() -> *mut u8 {
    let name = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "x86" => "x86",
        "arm" => "arm",
        _ => "unknown",
    };
    cstring_from_str(name)
}

static RANDOM_STATE: AtomicU64 = AtomicU64::new(0);

fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos() as u64;
    let addr = (&RANDOM_STATE as *const AtomicU64 as usize) as u64;
    let seed = nanos ^ u64::from(std::process::id()) ^ addr;
    if seed == 0 {
        0x9e37_79b9_7f4a_7c15
    } else {
        seed
    }
}

fn random_next_u64() -> u64 {
    loop {
        let current = RANDOM_STATE.load(Ordering::Relaxed);
        let state = if current == 0 { random_seed() } else { current };
        let next = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        if RANDOM_STATE
            .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return next;
        }
    }
}

#[no_mangle]
pub extern "C" fn ori_random_int(mut min: i64, mut max: i64) -> i64 {
    if max < min {
        std::mem::swap(&mut min, &mut max);
    }
    let span = (max as u64).wrapping_sub(min as u64).wrapping_add(1);
    let offset = if span == 0 {
        random_next_u64()
    } else {
        random_next_u64() % span
    };
    (min as u64).wrapping_add(offset) as i64
}

fn random_unit_float() -> f64 {
    ((random_next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}

#[no_mangle]
pub extern "C" fn ori_random_float(mut min: f64, mut max: f64) -> f64 {
    if max < min {
        std::mem::swap(&mut min, &mut max);
    }
    min + (max - min) * random_unit_float()
}

#[no_mangle]
pub extern "C" fn ori_random_bool() -> c_uchar {
    u8::from((random_next_u64() & 1) != 0) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_random_choice(items: *mut OriList) -> *mut OriOptionalInt {
    if items.is_null() || (*items).len <= 0 {
        return alloc_optional_int(0, 0);
    }
    let index = (random_next_u64() % (*items).len as u64) as usize;
    alloc_optional_int(1, *(*items).data.add(index))
}

#[no_mangle]
pub unsafe extern "C" fn ori_random_shuffle(items: *mut OriList) -> *mut OriList {
    let out = ori_list_new();
    if items.is_null() {
        return out;
    }
    for i in 0..(*items).len as usize {
        ori_list_push_borrowed_maybe_managed(out, *(*items).data.add(i));
    }
    let mut remaining = (*out).len as usize;
    while remaining > 1 {
        let j = (random_next_u64() % remaining as u64) as usize;
        let last = remaining - 1;
        let a = *(*out).data.add(last);
        let b = *(*out).data.add(j);
        *(*out).data.add(last) = b;
        *(*out).data.add(j) = a;
        remaining -= 1;
    }
    out
}

unsafe fn to_ori_json_value(val: serde_json::Value) -> *mut u8 {
    let ptr = ori_alloc(16, None);
    if ptr.is_null() {
        return ptr;
    }
    match val {
        serde_json::Value::Null => {
            *(ptr as *mut i32) = 0;
            *(ptr.add(8) as *mut u64) = 0;
        }
        serde_json::Value::Bool(b) => {
            *(ptr as *mut i32) = 1;
            *(ptr.add(8) as *mut u8) = if b { 1 } else { 0 };
        }
        serde_json::Value::Number(num) => {
            *(ptr as *mut i32) = 2;
            let f = num.as_f64().unwrap_or(0.0);
            *(ptr.add(8) as *mut f64) = f;
        }
        serde_json::Value::String(s) => {
            *(ptr as *mut i32) = 3;
            let s_ptr = cstring_from_str(&s);
            *(ptr.add(8) as *mut *mut u8) = s_ptr;
            ori_arc_register_edge(ptr, s_ptr);
            ori_arc_release(s_ptr);
        }
        serde_json::Value::Array(arr) => {
            *(ptr as *mut i32) = 4;
            let list_ptr = ori_list_new() as *mut u8;
            for item in arr {
                let item_ptr = to_ori_json_value(item);
                ori_list_push_borrowed_maybe_managed(list_ptr as *mut OriList, item_ptr as i64);
                ori_arc_release(item_ptr);
            }
            *(ptr.add(8) as *mut *mut u8) = list_ptr;
            ori_arc_register_edge(ptr, list_ptr);
            ori_arc_release(list_ptr);
        }
        serde_json::Value::Object(obj) => {
            *(ptr as *mut i32) = 5;
            let map_ptr = ori_map_new() as *mut u8;
            for (k, v) in obj {
                let key_ptr = cstring_from_str(&k);
                let val_ptr = to_ori_json_value(v);
                ori_map_set_string(map_ptr as *mut OriMap, key_ptr, val_ptr as i64);
                ori_arc_register_edge(map_ptr, key_ptr);
                ori_arc_register_edge(map_ptr, val_ptr);
                ori_arc_release(key_ptr);
                ori_arc_release(val_ptr);
            }
            *(ptr.add(8) as *mut *mut u8) = map_ptr;
            ori_arc_register_edge(ptr, map_ptr);
            ori_arc_release(map_ptr);
        }
    }
    ptr
}

unsafe fn from_ori_json_value(ptr: *const u8) -> serde_json::Value {
    if ptr.is_null() {
        return serde_json::Value::Null;
    }
    let tag = *(ptr as *const i32);
    match tag {
        0 => serde_json::Value::Null,
        1 => {
            let b = *(ptr.add(8) as *const u8);
            serde_json::Value::Bool(b != 0)
        }
        2 => {
            let f = *(ptr.add(8) as *const f64);
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        3 => {
            let s_ptr = *(ptr.add(8) as *const *const u8);
            let s = cstr_str(s_ptr);
            serde_json::Value::String(s.to_string())
        }
        4 => {
            let list_ptr = *(ptr.add(8) as *const *mut OriList);
            if list_ptr.is_null() {
                serde_json::Value::Array(Vec::new())
            } else {
                let len = (*list_ptr).len as usize;
                let mut arr = Vec::with_capacity(len);
                for i in 0..len {
                    let item_ptr = *(*list_ptr).data.add(i) as *const u8;
                    arr.push(from_ori_json_value(item_ptr));
                }
                serde_json::Value::Array(arr)
            }
        }
        5 => {
            let map_ptr = *(ptr.add(8) as *const *mut OriMap);
            if map_ptr.is_null() {
                serde_json::Value::Object(serde_json::Map::new())
            } else {
                let len = (*map_ptr).len as usize;
                let mut obj = serde_json::Map::with_capacity(len);
                for i in 0..len {
                    let key_ptr = *(*map_ptr).keys.add(i) as *const u8;
                    let val_ptr = *(*map_ptr).values.add(i) as *const u8;
                    let key_str = cstr_str(key_ptr).to_string();
                    obj.insert(key_str, from_ori_json_value(val_ptr));
                }
                serde_json::Value::Object(obj)
            }
        }
        _ => serde_json::Value::Null,
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_json_parse(text: *const u8) -> *mut u8 {
    match serde_json::from_str::<serde_json::Value>(cstr_str(text)) {
        Ok(value) => {
            let val_ptr = to_ori_json_value(value);
            new_result(true, val_ptr)
        }
        Err(error) => {
            let err_ptr = cstring_from_str(&error.to_string());
            new_result(false, err_ptr)
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_json_stringify(value: *const u8) -> *mut u8 {
    let val = from_ori_json_value(value);
    cstring_from_str(&val.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn ori_json_stringify_pretty(value: *const u8) -> *mut u8 {
    let val = from_ori_json_value(value);
    match serde_json::to_string_pretty(&val) {
        Ok(text) => cstring_from_str(&text),
        Err(_) => cstring_from_str("null"),
    }
}

mod test_harness;

// `ori_test_*` entry points moved to `test_harness.rs` (Etapa 8.3).

#[no_mangle]
pub unsafe extern "C" fn ori_panic(message: *const u8) {
    eprintln!("ori panic: {}", cstr_str(message));
    std::process::abort();
}

unsafe fn new_optional_ptr(has_value: bool, payload: *mut u8) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let total = ptr_size * 2;
    let ptr = ori_alloc(total, None);
    if ptr.is_null() {
        return ptr;
    }
    std::ptr::write_bytes(ptr, 0, total);
    *ptr = u8::from(has_value);
    *(ptr.add(ptr_size) as *mut *mut u8) = payload;
    // LANG-MEM-9: the wrapper owns its fresh managed payload via an edge
    // (it was stored as a raw pointer before — released wrappers left the
    // payload orphaned).
    wrapper_owns_payload(ptr, payload);
    ptr
}

fn format_float_fixed(value: f64, decimals: i64) -> String {
    let precision = decimals.clamp(0, 15) as usize;
    format!("{value:.precision$}")
}

fn utc_parts_from_millis(millis: i64) -> (i64, u32, u32, u32, u32, u32) {
    let seconds = millis.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = (second_of_day / 3600) as u32;
    let minute = ((second_of_day % 3600) / 60) as u32;
    let second = (second_of_day % 60) as u32;
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * mp + 2).div_euclid(5) + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u32, day as u32)
}

#[repr(C)]
pub struct OriOptionalInt {
    has_value: c_uchar,
    value: i64,
}

#[repr(C)]
pub struct OriOptionalFloat {
    has_value: c_uchar,
    value: f64,
}

unsafe fn alloc_optional_int(has_value: c_uchar, value: i64) -> *mut OriOptionalInt {
    let ptr = ori_alloc(std::mem::size_of::<OriOptionalInt>(), None) as *mut OriOptionalInt;
    if !ptr.is_null() {
        (*ptr).has_value = has_value;
        (*ptr).value = value;
    }
    ptr
}

unsafe fn alloc_optional_owned_managed_value(
    has_value: c_uchar,
    value: i64,
) -> *mut OriOptionalInt {
    let ptr = alloc_optional_int(has_value, value);
    if has_value != 0 {
        ori_arc_register_edge(ptr as *mut u8, value as *mut u8);
        ori_arc_release(value as *mut u8);
    }
    ptr
}

unsafe fn alloc_optional_borrowed_managed_value(
    has_value: c_uchar,
    value: i64,
) -> *mut OriOptionalInt {
    let ptr = alloc_optional_int(has_value, value);
    if has_value != 0 {
        ori_arc_register_edge(ptr as *mut u8, value as *mut u8);
    }
    ptr
}

unsafe fn alloc_optional_float(has_value: c_uchar, value: f64) -> *mut OriOptionalFloat {
    let ptr = ori_alloc(std::mem::size_of::<OriOptionalFloat>(), None) as *mut OriOptionalFloat;
    if !ptr.is_null() {
        (*ptr).has_value = has_value;
        (*ptr).value = value;
    }
    ptr
}

#[no_mangle]
pub extern "C" fn ori_float_to_string(value: f64) -> *mut u8 {
    cstring_from_str(&value.to_string())
}

#[no_mangle]
pub extern "C" fn ori_to_float(value: i64) -> f64 {
    value as f64
}

#[no_mangle]
pub extern "C" fn ori_bool_to_string(value: c_uchar) -> *mut u8 {
    cstring_from_str(if value != 0 { "true" } else { "false" })
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_to_int(s: *const u8) -> *mut OriOptionalInt {
    let parsed = cstr_str(s).trim().parse::<i64>().ok();
    match parsed {
        Some(value) => alloc_optional_int(1, value),
        None => alloc_optional_int(0, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_to_float(s: *const u8) -> *mut OriOptionalFloat {
    let parsed = cstr_str(s).trim().parse::<f64>().ok();
    match parsed {
        Some(value) => alloc_optional_float(1, value),
        None => alloc_optional_float(0, 0.0),
    }
}

// ── ori.files ─────────────────────────────────────────────────────────────────
//
// result[T, string] layout (matches native_backend result_layout):
//   offset 0              : is_ok (u8)
//   offset 1..ptr_size-1  : padding
//   offset ptr_size       : payload (*mut u8 — string or list pointer)
//
// Both ok and err payloads are pointer-sized, so the same helper covers
// result[string, string] AND result[list[string], string].

/// Transfers ownership of a freshly created managed payload to a wrapper:
/// the wrapper's ARC edge becomes the payload's owner (+1 via the edge,
/// then the temporary's own +1 is dropped — single cascade owner). No-op
/// for null or unregistered (non-managed) payloads.
unsafe fn wrapper_owns_payload(wrapper: *mut u8, payload: *mut u8) {
    if wrapper.is_null() || payload.is_null() {
        return;
    }
    ori_arc_register_edge(wrapper, payload);
    ori_arc_release(payload);
}

unsafe fn new_result(is_ok: bool, payload: *mut u8) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let total = ptr_size * 2;
    // ARC-managed (LANG-MEM-9): result wrappers used to be raw malloc,
    // invisible to the registry — codegen releases were silent no-ops and
    // every wrapper (plus any payload only it referenced) leaked.
    let ptr = ori_alloc(total, None);
    if ptr.is_null() {
        return ptr;
    }
    std::ptr::write_bytes(ptr, 0, total);
    *ptr = u8::from(is_ok);
    *(ptr.add(ptr_size) as *mut *mut u8) = payload;
    wrapper_owns_payload(ptr, payload);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn ori_new_result(is_ok: c_uchar, payload: *mut u8) -> *mut u8 {
    new_result(is_ok != 0, payload)
}

unsafe fn new_result_i64_ok(value: i64) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let total = ptr_size * 2;
    let ptr = ori_alloc(total, None);
    if ptr.is_null() {
        return ptr;
    }
    std::ptr::write_bytes(ptr, 0, total);
    *ptr = 1;
    std::ptr::write_unaligned(ptr.add(ptr_size) as *mut i64, value);
    ptr
}

unsafe fn new_result_f64_ok(value: f64) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let total = ptr_size * 2;
    let ptr = ori_alloc(total, None);
    if ptr.is_null() {
        return ptr;
    }
    std::ptr::write_bytes(ptr, 0, total);
    *ptr = 1;
    std::ptr::write_unaligned(ptr.add(ptr_size) as *mut f64, value);
    ptr
}

unsafe fn new_result_bool_ok(value: bool) -> *mut u8 {
    let ptr_size = std::mem::size_of::<*mut u8>();
    let total = ptr_size * 2;
    let ptr = ori_alloc(total, None);
    if ptr.is_null() {
        return ptr;
    }
    std::ptr::write_bytes(ptr, 0, total);
    *ptr = 1;
    *ptr.add(ptr_size) = u8::from(value);
    ptr
}

unsafe fn new_result_void_ok() -> *mut u8 {
    new_result(true, std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_parse_int(s: *const u8) -> *mut u8 {
    match cstr_str(s).trim().parse::<i64>() {
        Ok(value) => new_result_i64_ok(value),
        Err(_) => new_result(false, cstring_from_str("invalid int")),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_parse_float(s: *const u8) -> *mut u8 {
    match cstr_str(s).trim().parse::<f64>() {
        Ok(value) => new_result_f64_ok(value),
        Err(_) => new_result(false, cstring_from_str("invalid float")),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_read_text(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::read_to_string(path_str) {
        Ok(content) => new_result(true, cstring_from_str(&content)),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_write_text(path: *const u8, content: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    let content_str = cstr_str(content);
    match std::fs::write(path_str, content_str.as_bytes()) {
        Ok(_) => new_result(true, cstring_from_str("")),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_read_bytes(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::read(path_str) {
        Ok(content) => new_result(true, cstring_from_bytes(content)),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_write_bytes(path: *const u8, content: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::write(path_str, bytes_payload(content)) {
        Ok(_) => new_result(true, cstring_from_str("")),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_read_all(path: *const u8) -> *mut u8 {
    ori_files_read_text(path)
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_read_text_async(path: *const u8) -> *mut OriFuture {
    let path_str = cstr_str(path).to_string();
    let future = alloc_pending_future();
    if future.is_null() {
        return future;
    }

    ori_arc_retain(future as *mut u8);
    let future_addr = future as usize;
    std::thread::spawn(move || {
        let result = match std::fs::read_to_string(path_str) {
            Ok(content) => unsafe { new_result(true, cstring_from_str(&content)) },
            Err(e) => unsafe { new_result(false, cstring_from_str(&e.to_string())) },
        };
        unsafe {
            complete_future_owned(
                future_addr as *mut OriFuture,
                OriFutureStatus::Ready,
                result as i64,
            );
        }
    });
    future
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_write_text_async(
    path: *const u8,
    content: *const u8,
) -> *mut OriFuture {
    let path_str = cstr_str(path).to_string();
    let content_str = cstr_str(content).to_string();
    let future = alloc_pending_future();
    if future.is_null() {
        return future;
    }

    ori_arc_retain(future as *mut u8);
    let future_addr = future as usize;
    std::thread::spawn(move || {
        let result = match std::fs::write(path_str, content_str.as_bytes()) {
            Ok(_) => unsafe { new_result(true, cstring_from_str("")) },
            Err(e) => unsafe { new_result(false, cstring_from_str(&e.to_string())) },
        };
        unsafe {
            complete_future_owned(
                future_addr as *mut OriFuture,
                OriFutureStatus::Ready,
                result as i64,
            );
        }
    });
    future
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_append_text(path: *const u8, content: *const u8) -> *mut u8 {
    use std::io::Write;
    let path_str = cstr_str(path);
    let content_str = cstr_str(content);
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path_str)
        .and_then(|mut f| f.write_all(content_str.as_bytes()))
    {
        Ok(()) => new_result_void_ok(),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_exists(path: *const u8) -> *mut u8 {
    new_result_bool_ok(std::path::Path::new(cstr_str(path)).exists())
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_delete(path: *const u8) -> *mut u8 {
    let p = std::path::Path::new(cstr_str(path));
    let result = if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    };
    match result {
        Ok(()) => new_result_void_ok(),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_list_dir(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::read_dir(path_str) {
        Ok(entries) => {
            let list = ori_list_new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                ori_list_push_owned_managed(list, cstring_from_str(&name));
            }
            new_result(true, list as *mut u8)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_create_dir(path: *const u8) -> *mut u8 {
    match std::fs::create_dir_all(cstr_str(path)) {
        Ok(()) => new_result_void_ok(),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_is_file(path: *const u8) -> *mut u8 {
    new_result_bool_ok(std::path::Path::new(cstr_str(path)).is_file())
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_is_dir(path: *const u8) -> *mut u8 {
    new_result_bool_ok(std::path::Path::new(cstr_str(path)).is_dir())
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_copy(src: *const u8, dst: *const u8) -> *mut u8 {
    match std::fs::copy(cstr_str(src), cstr_str(dst)) {
        Ok(_) => new_result_void_ok(),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_rename(src: *const u8, dst: *const u8) -> *mut u8 {
    match std::fs::rename(cstr_str(src), cstr_str(dst)) {
        Ok(()) => new_result_void_ok(),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

struct RuntimeFile {
    file: Option<std::fs::File>,
}

unsafe extern "C" fn ori_files_destructor(ptr: *mut u8) {
    if !ptr.is_null() {
        std::ptr::drop_in_place(ptr as *mut RuntimeFile);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_open_read(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::File::open(path_str) {
        Ok(file) => {
            let layout_size = std::mem::size_of::<RuntimeFile>();
            let payload = ori_alloc(layout_size, Some(ori_files_destructor)) as *mut RuntimeFile;
            if payload.is_null() {
                return std::ptr::null_mut();
            }
            std::ptr::write(payload, RuntimeFile { file: Some(file) });
            new_result(true, payload as *mut u8)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_open_write(path: *const u8) -> *mut u8 {
    let path_str = cstr_str(path);
    match std::fs::File::create(path_str) {
        Ok(file) => {
            let layout_size = std::mem::size_of::<RuntimeFile>();
            let payload = ori_alloc(layout_size, Some(ori_files_destructor)) as *mut RuntimeFile;
            if payload.is_null() {
                return std::ptr::null_mut();
            }
            std::ptr::write(payload, RuntimeFile { file: Some(file) });
            new_result(true, payload as *mut u8)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_read(file_ptr: *mut u8, bytes_count: i64) -> *mut u8 {
    if file_ptr.is_null() {
        return new_result(false, cstring_from_str("Invalid file handle"));
    }
    if bytes_count < 0 {
        return new_result(false, cstring_from_str("Invalid byte count"));
    }
    let runtime_file = &mut *(file_ptr as *mut RuntimeFile);
    let Some(ref mut file) = runtime_file.file else {
        return new_result(false, cstring_from_str("File is closed"));
    };
    let mut buf = vec![0u8; bytes_count as usize];
    use std::io::Read;
    match file.read(&mut buf) {
        Ok(n) => {
            buf.truncate(n);
            new_result(true, cstring_from_bytes(buf))
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_write(file_ptr: *mut u8, data: *const u8) -> *mut u8 {
    if file_ptr.is_null() {
        return new_result(false, cstring_from_str("Invalid file handle"));
    }
    let runtime_file = &mut *(file_ptr as *mut RuntimeFile);
    let Some(ref mut file) = runtime_file.file else {
        return new_result(false, cstring_from_str("File is closed"));
    };
    let bytes = bytes_payload(data);
    use std::io::Write;
    match file.write(bytes) {
        Ok(n) => new_result_i64_ok(n as i64),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_close(file_ptr: *mut u8) {
    if file_ptr.is_null() {
        return;
    }
    let runtime_file = &mut *(file_ptr as *mut RuntimeFile);
    runtime_file.file = None;
}

// ── ori.bytes ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_len(ptr: *const u8) -> i64 {
    bytes_payload(ptr).len() as i64
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_concat(a: *const u8, b: *const u8) -> *mut u8 {
    let a = bytes_payload(a);
    let b = bytes_payload(b);
    let mut out = Vec::with_capacity(a.len() + b.len());
    out.extend_from_slice(a);
    out.extend_from_slice(b);
    cstring_from_bytes(out)
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_slice(ptr: *const u8, start: i64, end: i64) -> *mut u8 {
    if ptr.is_null() {
        abort_bounds("ori bytes slice bounds out of range");
    }
    let bytes = bytes_payload(ptr);
    let (start, end) = checked_slice_bounds(
        bytes.len() as i64,
        start,
        end,
        "ori bytes slice bounds out of range",
    );
    cstring_from_bytes(bytes[start..end].to_vec())
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_to_hex(ptr: *const u8) -> *mut u8 {
    let bytes = bytes_payload(ptr);
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02x}", b);
    }
    cstring_from_str(&hex)
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_from_hex(ptr: *const u8) -> *mut u8 {
    let s = cstr_str(ptr);
    if s.len() % 2 != 0 {
        return new_result(false, cstring_from_str("Invalid hex string length"));
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    let chars = s.as_bytes();
    for chunk in chars.chunks(2) {
        let str_chunk = unsafe { std::str::from_utf8_unchecked(chunk) };
        match u8::from_str_radix(str_chunk, 16) {
            Ok(b) => bytes.push(b),
            Err(_) => return new_result(false, cstring_from_str("Invalid hex character")),
        }
    }
    new_result(true, cstring_from_bytes(bytes))
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_decode_utf8(ptr: *const u8) -> *mut u8 {
    let bytes = bytes_payload(ptr);
    if bytes.contains(&0) {
        return new_result(
            false,
            cstring_from_str("bytes containing NUL cannot be decoded to string"),
        );
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => new_result(true, cstring_from_str(s)),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_get(ptr: *const u8, index: i64) -> u8 {
    let bytes = bytes_payload(ptr);
    if index < 0 || index as usize >= bytes.len() {
        abort_bounds("ori bytes index out of bounds");
    } else {
        bytes[index as usize]
    }
}

fn abort_bounds(message: &str) -> ! {
    eprintln!("{message}");
    std::process::abort();
}

fn checked_slice_bounds(len: i64, start: i64, end: i64, message: &str) -> (usize, usize) {
    if start < 0 || end < start || end > len {
        abort_bounds(message);
    }
    (start as usize, end as usize)
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_to_bytes(ptr: *const u8) -> *mut u8 {
    ori_arc_retain(ptr as *mut u8);
    ptr as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn ori_string_from_bytes(ptr: *const u8) -> *mut u8 {
    let bytes = bytes_payload(ptr);
    if bytes.contains(&0) {
        return new_result(
            false,
            cstring_from_str("bytes containing NUL cannot be converted to string"),
        );
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => new_result(true, cstring_from_str(s)),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_abort_concurrent_modification() -> ! {
    abort_bounds("concurrent modification during iteration");
}

#[repr(C)]
pub struct OriDequeIterator {
    pub deque: *mut OriDeque,
    pub expected_version: i64,
    pub index: i64,
    pub last_value: i64,
}

unsafe extern "C" fn ori_deque_iterator_dtor(ptr: *mut u8) {
    let iter = ptr as *mut OriDequeIterator;
    if !iter.is_null() {
        ori_arc_release((*iter).deque as *mut u8);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_iterator_new(deque: *mut OriDeque) -> *mut OriDequeIterator {
    if deque.is_null() {
        return std::ptr::null_mut();
    }
    let iter = ori_alloc(
        std::mem::size_of::<OriDequeIterator>(),
        Some(ori_deque_iterator_dtor),
    ) as *mut OriDequeIterator;
    if !iter.is_null() {
        ori_arc_retain(deque as *mut u8);
        (*iter).deque = deque;
        (*iter).expected_version = (*deque).version;
        (*iter).index = 0;
        (*iter).last_value = 0;
    }
    iter
}

#[no_mangle]
pub unsafe extern "C" fn ori_deque_iterator_next(iter: *mut OriDequeIterator) -> *mut i64 {
    if iter.is_null() || (*iter).deque.is_null() {
        return std::ptr::null_mut();
    }
    let deque = (*iter).deque;
    if (*deque).version != (*iter).expected_version {
        ori_abort_concurrent_modification();
    }
    if (*iter).index >= (*deque).values.len() as i64 {
        return std::ptr::null_mut();
    }
    let val = (*deque)
        .values
        .get((*iter).index as usize)
        .copied()
        .unwrap_or(0);
    (*iter).last_value = val;
    (*iter).index += 1;
    &mut (*iter).last_value
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_iterator_new(queue: *mut OriDeque) -> *mut OriDequeIterator {
    ori_deque_iterator_new(queue)
}

#[no_mangle]
pub unsafe extern "C" fn ori_queue_iterator_next(iter: *mut OriDequeIterator) -> *mut i64 {
    ori_deque_iterator_next(iter)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_iterator_new(stack: *mut OriDeque) -> *mut OriDequeIterator {
    ori_deque_iterator_new(stack)
}

#[no_mangle]
pub unsafe extern "C" fn ori_stack_iterator_next(iter: *mut OriDequeIterator) -> *mut i64 {
    ori_deque_iterator_next(iter)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_iterator_new(
    list: *mut OriDeque,
) -> *mut OriDequeIterator {
    ori_deque_iterator_new(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_linked_list_iterator_next(iter: *mut OriDequeIterator) -> *mut i64 {
    ori_deque_iterator_next(iter)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_iterator_new(
    list: *mut OriDeque,
) -> *mut OriDequeIterator {
    ori_deque_iterator_new(list)
}

#[no_mangle]
pub unsafe extern "C" fn ori_doubly_linked_list_iterator_next(
    iter: *mut OriDequeIterator,
) -> *mut i64 {
    ori_deque_iterator_next(iter)
}

#[repr(C)]
pub struct OriHeapIterator {
    pub heap: *mut OriHeap,
    pub expected_version: i64,
    pub index: i64,
    pub last_value: i64,
}

unsafe extern "C" fn ori_heap_iterator_dtor(ptr: *mut u8) {
    let iter = ptr as *mut OriHeapIterator;
    if !iter.is_null() {
        ori_arc_release((*iter).heap as *mut u8);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_iterator_new(heap: *mut OriHeap) -> *mut OriHeapIterator {
    if heap.is_null() {
        return std::ptr::null_mut();
    }
    let iter = ori_alloc(
        std::mem::size_of::<OriHeapIterator>(),
        Some(ori_heap_iterator_dtor),
    ) as *mut OriHeapIterator;
    if !iter.is_null() {
        ori_arc_retain(heap as *mut u8);
        (*iter).heap = heap;
        (*iter).expected_version = (*heap).version;
        (*iter).index = 0;
        (*iter).last_value = 0;
    }
    iter
}

#[no_mangle]
pub unsafe extern "C" fn ori_heap_iterator_next(iter: *mut OriHeapIterator) -> *mut i64 {
    if iter.is_null() || (*iter).heap.is_null() {
        return std::ptr::null_mut();
    }
    let heap = (*iter).heap;
    if (*heap).version != (*iter).expected_version {
        ori_abort_concurrent_modification();
    }
    if (*iter).index >= (*heap).len {
        return std::ptr::null_mut();
    }
    let val = *(*heap).data.add((*iter).index as usize);
    (*iter).last_value = val;
    (*iter).index += 1;
    &mut (*iter).last_value
}

#[repr(C)]
pub struct OriGraphIterator {
    pub graph: *mut OriGraph,
    pub expected_version: i64,
    pub index: i64,
    pub last_value: i64,
}

unsafe extern "C" fn ori_graph_iterator_dtor(ptr: *mut u8) {
    let iter = ptr as *mut OriGraphIterator;
    if !iter.is_null() {
        ori_arc_release((*iter).graph as *mut u8);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_iterator_new(graph: *mut OriGraph) -> *mut OriGraphIterator {
    if graph.is_null() {
        return std::ptr::null_mut();
    }
    let iter = ori_alloc(
        std::mem::size_of::<OriGraphIterator>(),
        Some(ori_graph_iterator_dtor),
    ) as *mut OriGraphIterator;
    if !iter.is_null() {
        ori_arc_retain(graph as *mut u8);
        (*iter).graph = graph;
        (*iter).expected_version = (*graph).version;
        (*iter).index = 0;
        (*iter).last_value = 0;
    }
    iter
}

#[no_mangle]
pub unsafe extern "C" fn ori_graph_iterator_next(iter: *mut OriGraphIterator) -> *mut i64 {
    if iter.is_null() || (*iter).graph.is_null() {
        return std::ptr::null_mut();
    }
    let graph = (*iter).graph;
    if (*graph).version != (*iter).expected_version {
        ori_abort_concurrent_modification();
    }
    if (*iter).index >= (*graph).len {
        return std::ptr::null_mut();
    }
    let val = *(*graph).nodes.add((*iter).index as usize);
    (*iter).last_value = val;
    (*iter).index += 1;
    &mut (*iter).last_value
}

// ── Stdlib parity extensions (gap closure) ─────────────────────────────────

struct RuntimeConnection {
    stream: Option<ConnectionTransport>,
}

enum ConnectionTransport {
    Plain(std::net::TcpStream),
    Tls(rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>),
}

impl ConnectionTransport {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::Read;
        match self {
            ConnectionTransport::Plain(stream) => stream.read(buf),
            ConnectionTransport::Tls(stream) => stream.read(buf),
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        match self {
            ConnectionTransport::Plain(stream) => stream.write_all(buf),
            ConnectionTransport::Tls(stream) => stream.write_all(buf),
        }
    }

    fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        match self {
            ConnectionTransport::Plain(stream) => stream.shutdown(how),
            ConnectionTransport::Tls(stream) => stream.get_ref().shutdown(how),
        }
    }

    /// B7: socket read deadline. `None` clears the timeout (blocking).
    fn set_read_timeout(&self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            ConnectionTransport::Plain(stream) => stream.set_read_timeout(timeout),
            ConnectionTransport::Tls(stream) => stream.get_ref().set_read_timeout(timeout),
        }
    }

    fn set_write_timeout(&self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            ConnectionTransport::Plain(stream) => stream.set_write_timeout(timeout),
            ConnectionTransport::Tls(stream) => stream.get_ref().set_write_timeout(timeout),
        }
    }
}

struct RuntimeListener {
    listener: Option<std::net::TcpListener>,
}

struct RuntimeUdpSocket {
    socket: Option<std::net::UdpSocket>,
}

unsafe extern "C" fn ori_net_destructor(ptr: *mut u8) {
    if !ptr.is_null() {
        std::ptr::drop_in_place(ptr as *mut RuntimeConnection);
    }
}

unsafe extern "C" fn ori_net_listener_destructor(ptr: *mut u8) {
    if !ptr.is_null() {
        std::ptr::drop_in_place(ptr as *mut RuntimeListener);
    }
}

unsafe extern "C" fn ori_net_udp_destructor(ptr: *mut u8) {
    if !ptr.is_null() {
        std::ptr::drop_in_place(ptr as *mut RuntimeUdpSocket);
    }
}

fn parse_socket_addr(address: &str) -> Result<std::net::SocketAddr, String> {
    use std::net::ToSocketAddrs;
    address
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("could not resolve address `{address}`"))
}

fn tcp_connect(address: &str, timeout_ms: i64) -> Result<std::net::TcpStream, String> {
    use std::net::TcpStream;
    if timeout_ms > 0 {
        let timeout = Duration::from_millis(timeout_ms as u64);
        let addr = parse_socket_addr(address)?;
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| e.to_string())
    } else {
        TcpStream::connect(address).map_err(|e| e.to_string())
    }
}

fn alloc_runtime_connection(stream: ConnectionTransport) -> Result<*mut u8, String> {
    let layout_size = std::mem::size_of::<RuntimeConnection>();
    let payload =
        unsafe { ori_alloc(layout_size, Some(ori_net_destructor)) as *mut RuntimeConnection };
    if payload.is_null() {
        return Err("allocation failed".to_string());
    }
    unsafe {
        std::ptr::write(
            payload,
            RuntimeConnection {
                stream: Some(stream),
            },
        );
    }
    Ok(payload as *mut u8)
}

fn ensure_rustls_crypto_provider() {
    static INSTALL: OnceLock<()> = OnceLock::new();
    INSTALL.get_or_init(|| {
        // rustls 0.23 requires an explicit process-level CryptoProvider unless
        // exactly one provider feature is selected *and* defaults are installed.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn tls_client_config() -> Result<rustls::ClientConfig, String> {
    ensure_rustls_crypto_provider();
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth())
}

unsafe fn list_string_at(list: *mut OriList, index: i64) -> Option<String> {
    if list.is_null() || index < 0 || index >= (*list).len {
        return None;
    }
    let ptr = ori_list_get(list, index) as *const u8;
    if ptr.is_null() {
        return None;
    }
    Some(cstr_str(ptr).to_string())
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_file_size(path: *const u8) -> *mut u8 {
    match std::fs::metadata(cstr_str(path)) {
        Ok(meta) => new_result_i64_ok(meta.len() as i64),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_modified_at(path: *const u8) -> *mut u8 {
    match std::fs::metadata(cstr_str(path)) {
        Ok(meta) => match meta.modified() {
            Ok(time) => {
                let millis = time
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_millis() as i64;
                new_result_i64_ok(millis)
            }
            Err(e) => new_result(false, cstring_from_str(&e.to_string())),
        },
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_files_created_at(path: *const u8) -> *mut u8 {
    match std::fs::metadata(cstr_str(path)) {
        Ok(meta) => {
            let optional = match meta.created() {
                Ok(time) => {
                    let millis = time
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or(Duration::ZERO)
                        .as_millis() as i64;
                    alloc_optional_int(1, millis) as *mut u8
                }
                Err(_) => alloc_optional_int(0, 0) as *mut u8,
            };
            new_result(true, optional)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_os_current_dir() -> *mut u8 {
    match std::env::current_dir() {
        Ok(path) => new_result(true, cstring_from_str(&path.to_string_lossy())),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_os_change_dir(path: *const u8) -> *mut u8 {
    match std::env::set_current_dir(cstr_str(path)) {
        Ok(()) => new_result(true, std::ptr::null_mut()),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub extern "C" fn ori_random_seed(seed: i64) {
    let value = if seed == 0 {
        random_seed()
    } else {
        seed as u64
    };
    RANDOM_STATE.store(value, Ordering::Relaxed);
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_from_list(values: *mut OriList) -> *mut u8 {
    if values.is_null() {
        return std::ptr::null_mut();
    }
    let mut bytes = Vec::new();
    for i in 0..(*values).len {
        let n = ori_list_get(values, i);
        if n < 0 || n > 255 {
            return std::ptr::null_mut();
        }
        bytes.push(n as u8);
    }
    cstring_from_bytes(bytes)
}

#[no_mangle]
pub unsafe extern "C" fn ori_bytes_to_list(value: *const u8) -> *mut OriList {
    let data = if value.is_null() {
        &[][..]
    } else {
        CStr::from_ptr(value as *const i8).to_bytes()
    };
    let list = ori_list_new();
    for byte in data {
        ori_list_push(list, i64::from(*byte));
    }
    list
}

#[no_mangle]
pub unsafe extern "C" fn ori_process_run(program: *const u8, args: *mut OriList) -> *mut u8 {
    let program_str = cstr_str(program);
    let mut command = std::process::Command::new(program_str);
    if !args.is_null() {
        for i in 0..(*args).len {
            if let Some(arg) = list_string_at(args, i) {
                command.arg(arg);
            }
        }
    }
    match command.status() {
        Ok(status) => new_result_i64_ok(status.code().unwrap_or(-1) as i64),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_process_run_capture(
    program: *const u8,
    args: *mut OriList,
) -> *mut u8 {
    let program_str = cstr_str(program);
    let mut command = std::process::Command::new(program_str);
    if !args.is_null() {
        for i in 0..(*args).len {
            if let Some(arg) = list_string_at(args, i) {
                command.arg(arg);
            }
        }
    }
    match command.output() {
        Ok(output) => {
            let map = ori_map_new();
            let status = output.status.code().unwrap_or(-1).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let status_key = cstring_from_str("status");
            let stdout_key = cstring_from_str("stdout");
            let stderr_key = cstring_from_str("stderr");
            let status_val = cstring_from_str(&status);
            let stdout_val = cstring_from_str(&stdout);
            let stderr_val = cstring_from_str(&stderr);
            ori_map_set_string(map, status_key, status_val as i64);
            ori_map_set_string(map, stdout_key, stdout_val as i64);
            ori_map_set_string(map, stderr_key, stderr_val as i64);
            ori_arc_release(status_key);
            ori_arc_release(stdout_key);
            ori_arc_release(stderr_key);
            ori_arc_release(status_val);
            ori_arc_release(stdout_val);
            ori_arc_release(stderr_val);
            new_result(true, map as *mut u8)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_connect(host: *const u8, port: i64, timeout_ms: i64) -> *mut u8 {
    let host_str = cstr_str(host);
    let address = format!("{host_str}:{port}");
    match tcp_connect(&address, timeout_ms) {
        Ok(stream) => match alloc_runtime_connection(ConnectionTransport::Plain(stream)) {
            Ok(payload) => new_result(true, payload),
            Err(msg) => new_result(false, cstring_from_str(&msg)),
        },
        Err(e) => new_result(false, cstring_from_str(&e)),
    }
}

/// STDLIB-4b/4k: await-able TCP connect — readiness-multiplexed I/O worker
/// (poll-based on Unix; worker fallback elsewhere). Does not block the async
/// executor while waiting.
#[no_mangle]
pub unsafe extern "C" fn ori_net_connect_async(
    host: *const u8,
    port: i64,
    timeout_ms: i64,
) -> *mut OriFuture {
    let host_str = cstr_str(host).to_string();
    spawn_io_result_future(move || match std::ffi::CString::new(host_str) {
        Ok(c_host) => ori_net_connect(c_host.as_ptr() as *const u8, port, timeout_ms),
        Err(_) => new_result(false, cstring_from_str("invalid host string")),
    })
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_connect_tls_async(
    host: *const u8,
    port: i64,
    timeout_ms: i64,
) -> *mut OriFuture {
    let host_str = cstr_str(host).to_string();
    // TLS handshake stays on the shared I/O pool (blocking path); still does not
    // block the Ori async executor.
    spawn_io_result_future(move || match std::ffi::CString::new(host_str) {
        Ok(c_host) => ori_net_connect_tls(c_host.as_ptr() as *const u8, port, timeout_ms),
        Err(_) => new_result(false, cstring_from_str("invalid host string")),
    })
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_accept_async(listener_ptr: *mut u8) -> *mut OriFuture {
    let listener_addr = listener_ptr as usize;
    spawn_readiness_io_future(
        move || listener_raw_fd(listener_addr as *mut u8),
        IoInterest::Read,
        move || ori_net_accept(listener_addr as *mut u8),
    )
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_read_some_async(conn: *mut u8, max_bytes: i64) -> *mut OriFuture {
    // Caller retains Connection across await; do not retain/release here
    // (extra release races the async frame and yields "invalid connection").
    let conn_addr = conn as usize;
    spawn_readiness_io_future(
        move || connection_raw_fd(conn_addr as *mut u8),
        IoInterest::Read,
        move || ori_net_read_some(conn_addr as *mut u8, max_bytes),
    )
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_write_all_async(conn: *mut u8, data: *const u8) -> *mut OriFuture {
    // Copy payload so the await frame may free the original bytes handle.
    // STDLIB-4k: wait for POLLOUT then write without blocking the executor.
    // Returns result[int, string] (bytes written) — avoids void-result await slots
    // clobbering live Connection locals across suspension.
    let conn_addr = conn as usize;
    let owned = bytes_payload(data).to_vec();
    let n = owned.len() as i64;
    spawn_readiness_io_future(
        move || connection_raw_fd(conn_addr as *mut u8),
        IoInterest::Write,
        move || {
            if conn_addr == 0 {
                return new_result(false, cstring_from_str("invalid connection"));
            }
            let connection = &mut *(conn_addr as *mut RuntimeConnection);
            let Some(stream) = connection.stream.as_mut() else {
                return new_result(false, cstring_from_str("connection closed"));
            };
            match stream.write_all(&owned) {
                Ok(()) => new_result_i64_ok(n),
                Err(e) => new_result(false, cstring_from_str(&e.to_string())),
            }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_recv_from_async(
    sock_ptr: *mut u8,
    max_bytes: i64,
) -> *mut OriFuture {
    let sock_addr = sock_ptr as usize;
    spawn_readiness_io_future(
        move || udp_raw_fd(sock_addr as *mut u8),
        IoInterest::Read,
        move || ori_net_udp_recv_from(sock_addr as *mut u8, max_bytes),
    )
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_send_to_async(
    sock_ptr: *mut u8,
    host: *const u8,
    port: i64,
    data: *const u8,
) -> *mut OriFuture {
    let sock_addr = sock_ptr as usize;
    let host_str = cstr_str(host).to_string();
    let owned = bytes_payload(data).to_vec();
    spawn_readiness_io_future(
        move || udp_raw_fd(sock_addr as *mut u8),
        IoInterest::Write,
        move || {
            match std::ffi::CString::new(host_str) {
                Ok(c_host) => {
                    // Build a temporary NUL-terminated bytes buffer for payload.
                    let mut payload = owned;
                    payload.push(0);
                    ori_net_udp_send_to(
                        sock_addr as *mut u8,
                        c_host.as_ptr() as *const u8,
                        port,
                        payload.as_ptr(),
                    )
                }
                Err(_) => new_result(false, cstring_from_str("invalid host string")),
            }
        },
    )
}

#[derive(Clone, Copy)]
enum IoInterest {
    Read,
    Write,
}

struct IoJob {
    interest: IoInterest,
    fd_probe: Box<dyn Fn() -> Option<i32> + Send>,
    work: Box<dyn FnOnce() -> *mut u8 + Send>,
    future: usize,
}

struct IoReactor {
    jobs: Mutex<VecDeque<IoJob>>,
    available: Condvar,
}

static IO_REACTOR: OnceLock<IoReactor> = OnceLock::new();
static IO_REACTOR_THREAD: OnceLock<()> = OnceLock::new();

fn io_reactor() -> &'static IoReactor {
    IO_REACTOR.get_or_init(|| IoReactor {
        jobs: Mutex::new(VecDeque::new()),
        available: Condvar::new(),
    })
}

fn ensure_io_reactor_thread() {
    IO_REACTOR_THREAD.get_or_init(|| {
        std::thread::Builder::new()
            .name("ori-io-reactor".into())
            .spawn(|| io_reactor_loop())
            .expect("spawn ori-io-reactor");
    });
}

fn io_reactor_loop() {
    let reactor = io_reactor();
    loop {
        let job = {
            let mut jobs = reactor.jobs.lock().unwrap();
            while jobs.is_empty() {
                jobs = reactor.available.wait(jobs).unwrap();
            }
            jobs.pop_front().expect("non-empty after wait")
        };

        // STDLIB-4k: readiness wait via poll(2) when a live fd is available,
        // then run the blocking-safe completion work. Multiplexes many waits
        // on one reactor thread instead of one OS thread per I/O op.
        if let Some(fd) = (job.fd_probe)() {
            wait_fd_ready(fd, job.interest);
        }
        let result = (job.work)();
        unsafe {
            complete_future_owned(
                job.future as *mut OriFuture,
                OriFutureStatus::Ready,
                result as i64,
            );
        }
    }
}

fn wait_fd_ready(fd: i32, interest: IoInterest) {
    #[cfg(unix)]
    {
        use std::os::raw::c_short;
        let events: c_short = match interest {
            IoInterest::Read => libc::POLLIN,
            IoInterest::Write => libc::POLLOUT,
        };
        // Cap wait so a closed peer cannot hang the reactor forever.
        let mut remaining_ms: i32 = 60_000;
        while remaining_ms > 0 {
            let mut pfd = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            let slice_ms = remaining_ms.min(1_000);
            let rc = unsafe { libc::poll(&mut pfd, 1, slice_ms) };
            if rc < 0 {
                break;
            }
            if rc > 0
                && (pfd.revents & (events | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0
            {
                break;
            }
            remaining_ms -= slice_ms;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (fd, interest);
        // Windows path: fall through immediately; work uses blocking sockets.
    }
}

unsafe fn connection_raw_fd(conn: *mut u8) -> Option<i32> {
    if conn.is_null() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let connection = &*(conn as *mut RuntimeConnection);
        match connection.stream.as_ref()? {
            ConnectionTransport::Plain(stream) => Some(stream.as_raw_fd()),
            ConnectionTransport::Tls(stream) => Some(stream.get_ref().as_raw_fd()),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = conn;
        None
    }
}

unsafe fn listener_raw_fd(listener_ptr: *mut u8) -> Option<i32> {
    if listener_ptr.is_null() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let listener = &*(listener_ptr as *mut RuntimeListener);
        Some(listener.listener.as_ref()?.as_raw_fd())
    }
    #[cfg(not(unix))]
    {
        let _ = listener_ptr;
        None
    }
}

unsafe fn udp_raw_fd(sock_ptr: *mut u8) -> Option<i32> {
    if sock_ptr.is_null() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let socket = &*(sock_ptr as *mut RuntimeUdpSocket);
        Some(socket.socket.as_ref()?.as_raw_fd())
    }
    #[cfg(not(unix))]
    {
        let _ = sock_ptr;
        None
    }
}

/// STDLIB-4k readiness path: register with the shared I/O reactor (poll-based).
unsafe fn spawn_readiness_io_future(
    fd_probe: impl Fn() -> Option<i32> + Send + 'static,
    interest: IoInterest,
    work: impl FnOnce() -> *mut u8 + Send + 'static,
) -> *mut OriFuture {
    let future = alloc_pending_future();
    if future.is_null() {
        return future;
    }
    ori_arc_retain(future as *mut u8);
    ensure_io_reactor_thread();
    let reactor = io_reactor();
    {
        let mut jobs = reactor.jobs.lock().unwrap();
        jobs.push_back(IoJob {
            interest,
            fd_probe: Box::new(fd_probe),
            work: Box::new(work),
            future: future as usize,
        });
    }
    reactor.available.notify_one();
    future
}

/// Run blocking I/O on a worker thread and complete an `OriFuture`.
/// Used for connect/TLS/FS where no pollable fd exists until after the op starts.
/// Readiness-based net ops use `spawn_readiness_io_future` instead (STDLIB-4k).
unsafe fn spawn_io_result_future(
    work: impl FnOnce() -> *mut u8 + Send + 'static,
) -> *mut OriFuture {
    let future = alloc_pending_future();
    if future.is_null() {
        return future;
    }
    ori_arc_retain(future as *mut u8);
    let future_addr = future as usize;
    std::thread::spawn(move || {
        let result = work();
        complete_future_owned(
            future_addr as *mut OriFuture,
            OriFutureStatus::Ready,
            result as i64,
        );
    });
    future
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_connect_tls(
    host: *const u8,
    port: i64,
    timeout_ms: i64,
) -> *mut u8 {
    use rustls::pki_types::ServerName;
    let host_str = cstr_str(host);
    let address = format!("{host_str}:{port}");
    let tcp = match tcp_connect(&address, timeout_ms) {
        Ok(stream) => stream,
        Err(e) => return new_result(false, cstring_from_str(&e)),
    };
    let config = match tls_client_config() {
        Ok(config) => config,
        Err(e) => return new_result(false, cstring_from_str(&e)),
    };
    let server_name = match ServerName::try_from(host_str.to_string()) {
        Ok(name) => name,
        Err(e) => {
            return new_result(
                false,
                cstring_from_str(&format!("invalid TLS server name: {e}")),
            )
        }
    };
    let conn = match rustls::ClientConnection::new(std::sync::Arc::new(config), server_name) {
        Ok(conn) => conn,
        Err(e) => return new_result(false, cstring_from_str(&e.to_string())),
    };
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    if let Err(e) = std::io::Write::flush(&mut tls) {
        return new_result(false, cstring_from_str(&e.to_string()));
    }
    match alloc_runtime_connection(ConnectionTransport::Tls(tls)) {
        Ok(payload) => new_result(true, payload),
        Err(msg) => new_result(false, cstring_from_str(&msg)),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_listen(host: *const u8, port: i64) -> *mut u8 {
    let host_str = cstr_str(host);
    let address = format!("{host_str}:{port}");
    match std::net::TcpListener::bind(&address) {
        Ok(listener) => {
            let layout_size = std::mem::size_of::<RuntimeListener>();
            let payload =
                ori_alloc(layout_size, Some(ori_net_listener_destructor)) as *mut RuntimeListener;
            if payload.is_null() {
                return new_result(false, cstring_from_str("allocation failed"));
            }
            std::ptr::write(
                payload,
                RuntimeListener {
                    listener: Some(listener),
                },
            );
            new_result(true, payload as *mut u8)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_accept(listener_ptr: *mut u8) -> *mut u8 {
    if listener_ptr.is_null() {
        return new_result(false, cstring_from_str("invalid listener"));
    }
    let listener = &mut *(listener_ptr as *mut RuntimeListener);
    let Some(ref tcp_listener) = listener.listener else {
        return new_result(false, cstring_from_str("listener closed"));
    };
    match tcp_listener.accept() {
        Ok((stream, _addr)) => match alloc_runtime_connection(ConnectionTransport::Plain(stream)) {
            Ok(payload) => new_result(true, payload),
            Err(msg) => new_result(false, cstring_from_str(&msg)),
        },
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_close_listener(listener_ptr: *mut u8) {
    if listener_ptr.is_null() {
        return;
    }
    let listener = &mut *(listener_ptr as *mut RuntimeListener);
    listener.listener = None;
    ori_arc_release(listener_ptr);
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_listener_port(listener_ptr: *mut u8) -> i64 {
    if listener_ptr.is_null() {
        return -1;
    }
    let listener = &*(listener_ptr as *mut RuntimeListener);
    listener
        .listener
        .as_ref()
        .and_then(|l| l.local_addr().ok())
        .map(|addr| addr.port() as i64)
        .unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_bind(host: *const u8, port: i64) -> *mut u8 {
    let host_str = cstr_str(host);
    let address = format!("{host_str}:{port}");
    match std::net::UdpSocket::bind(&address) {
        Ok(socket) => {
            let layout_size = std::mem::size_of::<RuntimeUdpSocket>();
            let payload =
                ori_alloc(layout_size, Some(ori_net_udp_destructor)) as *mut RuntimeUdpSocket;
            if payload.is_null() {
                return new_result(false, cstring_from_str("allocation failed"));
            }
            std::ptr::write(
                payload,
                RuntimeUdpSocket {
                    socket: Some(socket),
                },
            );
            new_result(true, payload as *mut u8)
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_send_to(
    sock_ptr: *mut u8,
    host: *const u8,
    port: i64,
    data: *const u8,
) -> *mut u8 {
    if sock_ptr.is_null() {
        return new_result(false, cstring_from_str("invalid UDP socket"));
    }
    let socket = &mut *(sock_ptr as *mut RuntimeUdpSocket);
    let Some(ref udp) = socket.socket else {
        return new_result(false, cstring_from_str("UDP socket closed"));
    };
    let host_str = cstr_str(host);
    let address = format!("{host_str}:{port}");
    let target = match parse_socket_addr(&address) {
        Ok(addr) => addr,
        Err(e) => return new_result(false, cstring_from_str(&e)),
    };
    let bytes = if data.is_null() {
        &[][..]
    } else {
        CStr::from_ptr(data as *const i8).to_bytes()
    };
    match udp.send_to(bytes, target) {
        Ok(n) => new_result_i64_ok(n as i64),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_recv_from(sock_ptr: *mut u8, max_bytes: i64) -> *mut u8 {
    if sock_ptr.is_null() {
        return new_result(false, cstring_from_str("invalid UDP socket"));
    }
    let socket = &mut *(sock_ptr as *mut RuntimeUdpSocket);
    let Some(ref udp) = socket.socket else {
        return new_result(false, cstring_from_str("UDP socket closed"));
    };
    let cap = max_bytes.clamp(0, 65_536) as usize;
    let mut buf = vec![0_u8; cap.max(1)];
    match udp.recv_from(&mut buf) {
        Ok((n, _addr)) => {
            buf.truncate(n);
            new_result(true, cstring_from_bytes(buf))
        }
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_close(sock_ptr: *mut u8) {
    if sock_ptr.is_null() {
        return;
    }
    let socket = &mut *(sock_ptr as *mut RuntimeUdpSocket);
    socket.socket = None;
    ori_arc_release(sock_ptr);
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_udp_local_port(sock_ptr: *mut u8) -> i64 {
    if sock_ptr.is_null() {
        return -1;
    }
    let socket = &*(sock_ptr as *mut RuntimeUdpSocket);
    socket
        .socket
        .as_ref()
        .and_then(|s| s.local_addr().ok())
        .map(|addr| addr.port() as i64)
        .unwrap_or(-1)
}

/// Set TCP read timeout in milliseconds. `timeout_ms <= 0` clears the deadline.
/// Returns `result[void, string]` (ok/err pointer pair).
#[no_mangle]
pub unsafe extern "C" fn ori_net_set_read_timeout_ms(conn: *mut u8, timeout_ms: i64) -> *mut u8 {
    if conn.is_null() {
        return new_result(false, cstring_from_str("invalid connection"));
    }
    let connection = &*(conn as *mut RuntimeConnection);
    let Some(stream) = connection.stream.as_ref() else {
        return new_result(false, cstring_from_str("connection closed"));
    };
    let dur = if timeout_ms <= 0 {
        None
    } else {
        Some(std::time::Duration::from_millis(timeout_ms as u64))
    };
    match stream.set_read_timeout(dur) {
        Ok(()) => new_result(true, std::ptr::null_mut()),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

/// Set TCP write timeout in milliseconds. `timeout_ms <= 0` clears the deadline.
#[no_mangle]
pub unsafe extern "C" fn ori_net_set_write_timeout_ms(conn: *mut u8, timeout_ms: i64) -> *mut u8 {
    if conn.is_null() {
        return new_result(false, cstring_from_str("invalid connection"));
    }
    let connection = &*(conn as *mut RuntimeConnection);
    let Some(stream) = connection.stream.as_ref() else {
        return new_result(false, cstring_from_str("connection closed"));
    };
    let dur = if timeout_ms <= 0 {
        None
    } else {
        Some(std::time::Duration::from_millis(timeout_ms as u64))
    };
    match stream.set_write_timeout(dur) {
        Ok(()) => new_result(true, std::ptr::null_mut()),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_read_some(conn: *mut u8, max_bytes: i64) -> *mut u8 {
    if conn.is_null() {
        return new_result(false, cstring_from_str("invalid connection"));
    }
    let connection = &mut *(conn as *mut RuntimeConnection);
    let Some(stream) = connection.stream.as_mut() else {
        return new_result(false, cstring_from_str("connection closed"));
    };
    let cap = max_bytes.clamp(0, 1_048_576) as usize;
    let mut buf = vec![0_u8; cap.max(1)];
    match stream.read(&mut buf) {
        Ok(n) => {
            buf.truncate(n);
            new_result(true, cstring_from_bytes(buf))
        }
        Err(e) => {
            // TimedOut surfaces as a normal error string so Ori can retry/close.
            new_result(false, cstring_from_str(&e.to_string()))
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_write_all(conn: *mut u8, data: *const u8) -> *mut u8 {
    if conn.is_null() {
        return new_result(false, cstring_from_str("invalid connection"));
    }
    let connection = &mut *(conn as *mut RuntimeConnection);
    let Some(stream) = connection.stream.as_mut() else {
        return new_result(false, cstring_from_str("connection closed"));
    };
    let bytes = if data.is_null() {
        &[][..]
    } else {
        CStr::from_ptr(data as *const i8).to_bytes()
    };
    match stream.write_all(bytes) {
        Ok(()) => new_result(true, std::ptr::null_mut()),
        Err(e) => new_result(false, cstring_from_str(&e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_close(conn: *mut u8) {
    if conn.is_null() {
        return;
    }
    let connection = &mut *(conn as *mut RuntimeConnection);
    if let Some(stream) = connection.stream.take() {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
    ori_arc_release(conn);
}

#[no_mangle]
pub unsafe extern "C" fn ori_net_is_closed(conn: *mut u8) -> c_uchar {
    if conn.is_null() {
        return 1;
    }
    let connection = &*(conn as *mut RuntimeConnection);
    u8::from(connection.stream.is_none()) as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn ori_mem_string_as_ptr(ptr: *const u8) -> i64 {
    ptr as i64
}

#[no_mangle]
pub unsafe extern "C" fn ori_mem_string_len(ptr: *const u8) -> i64 {
    cstr_byte_len(ptr) as i64
}

#[no_mangle]
pub extern "C" fn ori_math_trunc(x: f64) -> f64 {
    x.trunc()
}

#[no_mangle]
pub extern "C" fn ori_math_ln(x: f64) -> f64 {
    x.ln()
}

#[no_mangle]
pub extern "C" fn ori_math_exp(x: f64) -> f64 {
    x.exp()
}

#[no_mangle]
pub extern "C" fn ori_math_asin(x: f64) -> f64 {
    x.asin()
}

#[no_mangle]
pub extern "C" fn ori_math_acos(x: f64) -> f64 {
    x.acos()
}

#[no_mangle]
pub extern "C" fn ori_math_atan(x: f64) -> f64 {
    x.atan()
}

#[no_mangle]
pub extern "C" fn ori_math_atan2(y: f64, x: f64) -> f64 {
    y.atan2(x)
}

#[no_mangle]
pub extern "C" fn ori_math_log10(x: f64) -> f64 {
    x.log10()
}

#[no_mangle]
pub extern "C" fn ori_math_is_finite(x: f64) -> c_uchar {
    u8::from(x.is_finite()) as c_uchar
}

// ── Password hashing (argon2id, PHC string) — web C10 / SEC9 ─────────────────

/// Hash a password with argon2id. Returns a PHC-encoded string, or empty on failure.
#[no_mangle]
pub unsafe extern "C" fn ori_password_hash(password: *const u8) -> *mut u8 {
    use argon2::{
        password_hash::{PasswordHasher, SaltString},
        Argon2,
    };
    use rand_core::OsRng;

    let password = cstr_str(password);
    if password.is_empty() {
        return cstring_from_str("");
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    match argon2.hash_password(password.as_bytes(), &salt) {
        Ok(hash) => cstring_from_str(&hash.to_string()),
        Err(_) => cstring_from_str(""),
    }
}

/// Verify password against a PHC argon2 hash. Returns 1 if ok, 0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn ori_password_verify(password: *const u8, encoded: *const u8) -> c_uchar {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2,
    };

    let password = cstr_str(password);
    let encoded = cstr_str(encoded);
    if password.is_empty() || encoded.is_empty() {
        return 0;
    }
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return 0;
    };
    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

// ── TOTP (RFC 6238, HMAC-SHA1, 30s, 6 digits) — web C3 ──────────────────────

fn hotp_code(key: &[u8], counter: u64) -> u32 {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    type HmacSha1 = Hmac<Sha1>;
    let mut mac = match HmacSha1::new_from_slice(key) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();
    let offset = (result[19] & 0x0f) as usize;
    let bin = ((u32::from(result[offset]) & 0x7f) << 24)
        | (u32::from(result[offset + 1]) << 16)
        | (u32::from(result[offset + 2]) << 8)
        | u32::from(result[offset + 3]);
    bin % 1_000_000
}

fn decode_totp_secret(b32: &str) -> Option<Vec<u8>> {
    use data_encoding::BASE32_NOPAD;
    let cleaned: String = b32
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Pad to multiple of 8 for standard Base32 if needed.
    let pad = (8 - (cleaned.len() % 8)) % 8;
    let padded = format!("{}{}", cleaned, "=".repeat(pad));
    data_encoding::BASE32
        .decode(padded.as_bytes())
        .or_else(|_| BASE32_NOPAD.decode(cleaned.as_bytes()))
        .ok()
}

/// Generate a new base32 TOTP secret (160-bit / 20 bytes).
#[no_mangle]
pub unsafe extern "C" fn ori_totp_generate_secret() -> *mut u8 {
    use data_encoding::BASE32_NOPAD;
    let mut buf = [0u8; 20];
    if getrandom::getrandom(&mut buf).is_err() {
        return cstring_from_str("");
    }
    cstring_from_str(&BASE32_NOPAD.encode(&buf))
}

/// Compute current TOTP code for `secret_b32` at `unix_secs` (step 30s).
/// Returns a 6-digit string, or empty on failure.
#[no_mangle]
pub unsafe extern "C" fn ori_totp_code(secret_b32: *const u8, unix_secs: i64) -> *mut u8 {
    let secret = cstr_str(secret_b32);
    let Some(key) = decode_totp_secret(secret) else {
        return cstring_from_str("");
    };
    if unix_secs < 0 {
        return cstring_from_str("");
    }
    let counter = (unix_secs as u64) / 30;
    let code = hotp_code(&key, counter);
    cstring_from_str(&format!("{code:06}"))
}

/// Verify TOTP `code` against `secret_b32` at `unix_secs` with ±`window` steps.
/// Returns 1 if ok, 0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn ori_totp_verify(
    secret_b32: *const u8,
    code: *const u8,
    unix_secs: i64,
    window: i64,
) -> c_uchar {
    let secret = cstr_str(secret_b32);
    let code = cstr_str(code).trim();
    if secret.is_empty() || code.is_empty() || unix_secs < 0 {
        return 0;
    }
    let Some(key) = decode_totp_secret(secret) else {
        return 0;
    };
    let win = window.clamp(0, 10) as i64;
    let counter = (unix_secs as u64) / 30;
    for delta in -win..=win {
        let c = counter as i64 + delta;
        if c < 0 {
            continue;
        }
        let expected = format!("{:06}", hotp_code(&key, c as u64));
        // Constant-time-ish compare of fixed 6-char codes
        if expected.len() == code.len() {
            let mut diff = 0u8;
            for (a, b) in expected.bytes().zip(code.bytes()) {
                diff |= a ^ b;
            }
            if diff == 0 {
                return 1;
            }
        }
    }
    0
}

// ── Embeddable runtime boot (cdylib / `ori compile --lib`) ─────────────────
//
// Hosts that load an Ori shared library must:
//   1. `ori_rt_init()`            — once (idempotent)
//   2. `__ori_module_init()`      — if the library defines globals (emitted by
//                                   `ori compile --lib`; optional for pure
//                                   scalar `@c_export` modules)
//   3. call `@c_export` symbols
//   4. `ori_rt_shutdown()`        — best-effort teardown
//
// Module init is a separate exported symbol (not a weak import) so the
// runtime builds on stable Rust without `#[linkage]`.

static RT_INIT_COUNT: AtomicU64 = AtomicU64::new(0);

/// Initialize the Ori runtime for embed hosts.
///
/// Returns `0` on success. Idempotent: subsequent calls return `0`.
#[no_mangle]
pub unsafe extern "C" fn ori_rt_init() -> i32 {
    if RT_INIT_COUNT
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return 0;
    }
    // Ensure ARC state exists (lazy on first alloc, but touch for predictability).
    let _ = ARC_STATE.get_or_init(|| Mutex::new(ArcState::default()));
    0
}

/// Best-effort runtime teardown for embed hosts. The host process may continue.
#[no_mangle]
pub unsafe extern "C" fn ori_rt_shutdown() {
    // Phase 1: no-op beyond a fence — managed heaps are process-lifetime for
    // embed hosts that do not retain Ori objects across unload.
    std::sync::atomic::fence(Ordering::SeqCst);
    let _ = RT_INIT_COUNT.swap(0, Ordering::SeqCst);
}

mod debug_agent;

#[cfg(test)]
mod tests;
