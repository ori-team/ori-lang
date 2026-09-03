//! Async-signal-safe stack-overflow diagnostics for Linux.
//!
//! The process handler is installed once and restored during runtime shutdown.
//! Alternate signal stacks are per-thread: runtime workers attach themselves,
//! while foreign host threads use `ori_rt_thread_attach`/`detach` explicitly.

#[cfg(target_os = "linux")]
use std::cell::RefCell;
#[cfg(target_os = "linux")]
use std::mem::MaybeUninit;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(target_os = "linux")]
use std::sync::Mutex;

#[cfg(target_os = "linux")]
const MESSAGE: &[u8] = b"ori: stack overflow -- a function recursed until the stack ran out.\nori: check for recursion without a base case, or move large local data to the heap.\n";
#[cfg(target_os = "linux")]
const STACK_OVERFLOW_EXIT: i32 = 128 + libc::SIGSEGV;
#[cfg(target_os = "linux")]
const FALLBACK_PAGE_SIZE: usize = 4096;

#[cfg(target_os = "linux")]
static PROCESS_INSTALLED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "linux")]
static PAGE_SIZE: AtomicUsize = AtomicUsize::new(FALLBACK_PAGE_SIZE);
#[cfg(target_os = "linux")]
static ATTACHED_THREADS: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_os = "linux")]
static PROCESS_INSTALL_LOCK: Mutex<()> = Mutex::new(());
#[cfg(target_os = "linux")]
static mut PREVIOUS_SEGV: MaybeUninit<libc::sigaction> = MaybeUninit::uninit();
#[cfg(target_os = "linux")]
static mut PREVIOUS_BUS: MaybeUninit<libc::sigaction> = MaybeUninit::uninit();

#[cfg(target_os = "linux")]
struct ThreadSignalStack {
    memory: *mut libc::c_void,
    length: usize,
    previous: libc::stack_t,
}

#[cfg(target_os = "linux")]
impl Drop for ThreadSignalStack {
    fn drop(&mut self) {
        // SAFETY: this value is dropped on the thread that installed the
        // alternate stack. Restoring the previous descriptor removes all
        // kernel references before the mmap region is released.
        unsafe {
            let _ = libc::sigaltstack(&self.previous, std::ptr::null_mut());
            let _ = libc::munmap(self.memory, self.length);
        }
        ATTACHED_THREADS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(target_os = "linux")]
thread_local! {
    static THREAD_SIGNAL_STACK: RefCell<Option<ThreadSignalStack>> = const { RefCell::new(None) };
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn interrupted_stack_pointer(context: *mut libc::c_void) -> usize {
    if context.is_null() {
        return 0;
    }
    let context = &*(context as *const libc::ucontext_t);
    context.uc_mcontext.gregs[libc::REG_RSP as usize] as usize
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
unsafe fn interrupted_stack_pointer(context: *mut libc::c_void) -> usize {
    if context.is_null() {
        return 0;
    }
    let context = &*(context as *const libc::ucontext_t);
    context.uc_mcontext.sp as usize
}

#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86_64", target_arch = "aarch64"))
))]
unsafe fn interrupted_stack_pointer(_context: *mut libc::c_void) -> usize {
    // The ucontext register layout is architecture-specific. Unsupported
    // Linux architectures retain the prior handler instead of guessing.
    0
}

#[cfg(target_os = "linux")]
fn is_probable_stack_overflow(fault_address: usize, stack_pointer: usize) -> bool {
    if fault_address == 0 || stack_pointer == 0 {
        return false;
    }
    let page = PAGE_SIZE.load(Ordering::Relaxed).max(FALLBACK_PAGE_SIZE);
    let tolerance = page.saturating_mul(16).max(64 * 1024);
    fault_address.abs_diff(stack_pointer) <= tolerance
}

#[cfg(target_os = "linux")]
unsafe fn restore_previous_and_raise(signal: libc::c_int) {
    let previous = if signal == libc::SIGBUS {
        std::ptr::read(std::ptr::addr_of!(PREVIOUS_BUS).cast::<libc::sigaction>())
    } else {
        std::ptr::read(std::ptr::addr_of!(PREVIOUS_SEGV).cast::<libc::sigaction>())
    };
    let _ = libc::sigaction(signal, &previous, std::ptr::null_mut());
    let _ = libc::raise(signal);
}

#[cfg(target_os = "linux")]
unsafe extern "C" fn handler(
    signal: libc::c_int,
    info: *mut libc::siginfo_t,
    context: *mut libc::c_void,
) {
    let fault_address = if info.is_null() {
        0
    } else {
        (*info).si_addr() as usize
    };
    let stack_pointer = interrupted_stack_pointer(context);
    if is_probable_stack_overflow(fault_address, stack_pointer) {
        let _ = libc::write(
            libc::STDERR_FILENO,
            MESSAGE.as_ptr().cast::<libc::c_void>(),
            MESSAGE.len(),
        );
        libc::_exit(STACK_OVERFLOW_EXIT);
    }
    restore_previous_and_raise(signal);
}

#[cfg(target_os = "linux")]
unsafe fn install_process_handler() -> bool {
    let _guard = match PROCESS_INSTALL_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if PROCESS_INSTALLED.load(Ordering::Acquire) {
        return true;
    }
    let page = libc::sysconf(libc::_SC_PAGESIZE);
    if page > 0 {
        PAGE_SIZE.store(page as usize, Ordering::Relaxed);
    }
    let mut action: libc::sigaction = std::mem::zeroed();
    action.sa_sigaction = handler as *const () as usize;
    action.sa_flags = libc::SA_ONSTACK | libc::SA_SIGINFO;
    libc::sigemptyset(&mut action.sa_mask);
    let mut previous_segv: libc::sigaction = std::mem::zeroed();
    if libc::sigaction(libc::SIGSEGV, &action, &mut previous_segv) != 0 {
        return false;
    }
    let mut previous_bus: libc::sigaction = std::mem::zeroed();
    if libc::sigaction(libc::SIGBUS, &action, &mut previous_bus) != 0 {
        let _ = libc::sigaction(libc::SIGSEGV, &previous_segv, std::ptr::null_mut());
        return false;
    }
    std::ptr::write(
        std::ptr::addr_of_mut!(PREVIOUS_SEGV).cast::<libc::sigaction>(),
        previous_segv,
    );
    std::ptr::write(
        std::ptr::addr_of_mut!(PREVIOUS_BUS).cast::<libc::sigaction>(),
        previous_bus,
    );
    PROCESS_INSTALLED.store(true, Ordering::Release);
    true
}

#[cfg(target_os = "linux")]
unsafe fn attach_current_thread() -> bool {
    THREAD_SIGNAL_STACK.with(|slot| {
        if slot.borrow().is_some() {
            return true;
        }
        let length = std::cmp::max(libc::SIGSTKSZ, 32 * 1024);
        let memory = libc::mmap(
            std::ptr::null_mut(),
            length,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if memory == libc::MAP_FAILED {
            return false;
        }
        let mut previous: libc::stack_t = std::mem::zeroed();
        let stack = libc::stack_t {
            ss_sp: memory,
            ss_flags: 0,
            ss_size: length,
        };
        if libc::sigaltstack(&stack, &mut previous) != 0 {
            let _ = libc::munmap(memory, length);
            return false;
        }
        *slot.borrow_mut() = Some(ThreadSignalStack {
            memory,
            length,
            previous,
        });
        ATTACHED_THREADS.fetch_add(1, Ordering::AcqRel);
        true
    })
}

#[cfg(target_os = "linux")]
unsafe fn detach_current_thread() {
    THREAD_SIGNAL_STACK.with(|slot| drop(slot.borrow_mut().take()));
}

/// Install the process handler and attach the current thread.
#[cfg(target_os = "linux")]
pub(super) unsafe fn install() -> bool {
    let was_installed = PROCESS_INSTALLED.load(Ordering::Acquire);
    if !install_process_handler() {
        return false;
    }
    if attach_current_thread() {
        return true;
    }
    // Do not leave a process-wide handler pointing into a failed runtime
    // initialization. If another owner installed it first (for example a
    // standalone binary constructor), that owner remains responsible for
    // teardown and we only report the attachment failure.
    if !was_installed {
        uninstall();
    }
    false
}

/// Restore prior process handlers and detach the calling thread.
#[cfg(target_os = "linux")]
pub(super) unsafe fn uninstall() {
    {
        let _guard = match PROCESS_INSTALL_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if PROCESS_INSTALLED.swap(false, Ordering::AcqRel) {
            let previous_segv =
                std::ptr::read(std::ptr::addr_of!(PREVIOUS_SEGV).cast::<libc::sigaction>());
            let previous_bus =
                std::ptr::read(std::ptr::addr_of!(PREVIOUS_BUS).cast::<libc::sigaction>());
            let _ = libc::sigaction(libc::SIGSEGV, &previous_segv, std::ptr::null_mut());
            let _ = libc::sigaction(libc::SIGBUS, &previous_bus, std::ptr::null_mut());
        }
    }
    detach_current_thread();
}

/// Whether every attached thread except the caller has detached.
#[cfg(target_os = "linux")]
pub(super) fn can_uninstall_from_current_thread() -> bool {
    let current_is_attached = THREAD_SIGNAL_STACK.with(|slot| slot.borrow().is_some());
    ATTACHED_THREADS.load(Ordering::Acquire) == usize::from(current_is_attached)
}

#[cfg(not(target_os = "linux"))]
pub(super) unsafe fn install() -> bool {
    true
}

#[cfg(not(target_os = "linux"))]
pub(super) unsafe fn uninstall() {}

#[cfg(not(target_os = "linux"))]
pub(super) fn can_uninstall_from_current_thread() -> bool {
    true
}

/// Attach a foreign host thread before it enters Ori code.
///
/// # Safety
///
/// The caller must invoke this on the thread that will enter Ori and pair a
/// successful attachment with [`ori_rt_thread_detach`] on the same thread
/// after its last Ori frame has returned.
#[no_mangle]
pub unsafe extern "C" fn ori_rt_thread_attach() -> i32 {
    if !crate::runtime_accepts_thread_attach() {
        return -1;
    }
    if install() {
        0
    } else {
        -1
    }
}

/// Detach a foreign host thread after its last call into Ori code.
///
/// # Safety
///
/// The caller must invoke this on the same thread that was attached, with no
/// Ori frame or runtime callback still active on that thread.
#[no_mangle]
pub unsafe extern "C" fn ori_rt_thread_detach() {
    #[cfg(target_os = "linux")]
    detach_current_thread();
}

/// Backward-compatible entry point used by generated standalone binaries.
#[no_mangle]
unsafe extern "C" fn ori_rt_install_stack_guard() {
    let _ = install();
}

/// Install before generated `main` for standalone ELF binaries. Embed hosts
/// still own explicit teardown through `ori_rt_shutdown` before `dlclose`.
#[cfg(all(target_os = "linux", not(test)))]
#[used]
#[link_section = ".init_array"]
static INSTALL_STACK_GUARD: unsafe extern "C" fn() = {
    unsafe extern "C" fn constructor() {
        let _ = install();
    }
    constructor
};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    static SIGNAL_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[cfg(target_os = "linux")]
    #[test]
    fn overflow_classification_uses_cached_stack_pointer_distance() {
        PAGE_SIZE.store(4096, Ordering::Relaxed);
        assert!(is_probable_stack_overflow(0x1000, 0x1800));
        assert!(!is_probable_stack_overflow(0, 0x1800));
        assert!(!is_probable_stack_overflow(0x1000, 0x10_0000));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn thread_attach_and_detach_are_idempotent() {
        let _guard = SIGNAL_TEST_LOCK.lock().unwrap();
        unsafe {
            uninstall();
            let mut previous: libc::sigaction = std::mem::zeroed();
            assert_eq!(
                libc::sigaction(libc::SIGSEGV, std::ptr::null(), &mut previous),
                0
            );
            assert_eq!(ori_rt_thread_attach(), 0);
            assert_eq!(ori_rt_thread_attach(), 0);
            assert_eq!(ATTACHED_THREADS.load(Ordering::Acquire), 1);
            let mut installed: libc::sigaction = std::mem::zeroed();
            assert_eq!(
                libc::sigaction(libc::SIGSEGV, std::ptr::null(), &mut installed),
                0
            );
            assert_eq!(installed.sa_sigaction, handler as *const () as usize);
            ori_rt_thread_detach();
            ori_rt_thread_detach();
            assert_eq!(ATTACHED_THREADS.load(Ordering::Acquire), 0);
            uninstall();
            let mut restored: libc::sigaction = std::mem::zeroed();
            assert_eq!(
                libc::sigaction(libc::SIGSEGV, std::ptr::null(), &mut restored),
                0
            );
            assert_eq!(restored.sa_sigaction, previous.sa_sigaction);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unload_requires_every_foreign_thread_to_detach() {
        let _guard = SIGNAL_TEST_LOCK.lock().unwrap();
        let (attached_tx, attached_rx) = std::sync::mpsc::channel();
        let (detach_tx, detach_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || unsafe {
            assert_eq!(ori_rt_thread_attach(), 0);
            attached_tx.send(()).unwrap();
            detach_rx.recv().unwrap();
            ori_rt_thread_detach();
        });
        attached_rx.recv().unwrap();
        assert!(!can_uninstall_from_current_thread());
        detach_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(can_uninstall_from_current_thread());
        unsafe { uninstall() };
    }
}
