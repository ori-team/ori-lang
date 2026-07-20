//! Turn a stack overflow into a readable message instead of a bare `SIGSEGV`.
//!
//! Runaway recursion used to kill an Ori program with signal 11 and print
//! nothing at all: the shell reported exit 139 and the reader was left guessing.
//! A language that promises "every error says what happened" cannot ship that.
//!
//! The guard installs a `SIGSEGV`/`SIGBUS` handler on an **alternate signal
//! stack** — the normal stack is exhausted precisely when this fires, so the
//! handler could not run on it. When the faulting address lands in the guard
//! region just past the end of the thread stack, the fault is a stack overflow
//! and we say so. Anything else is re-raised with the default handler so real
//! memory bugs still produce a normal crash and a core dump.
//!
//! Everything inside the handler is async-signal-safe: a direct `write(2)` and
//! `_exit(2)`, no allocation and no formatting.

#![allow(clippy::missing_safety_doc)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Low address of the current thread stack, and the guard size below it.
static STACK_LOW: AtomicUsize = AtomicUsize::new(0);
static GUARD_SIZE: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);

const MESSAGE: &[u8] = b"ori: stack overflow -- a function recursed until the stack ran out.\nori: check for recursion without a base case, or move large local data to the heap.\n";

/// Exit code for a stack overflow. Mirrors the shell convention for a fatal
/// signal (128 + SIGSEGV) so scripts that inspect `$?` keep working.
const STACK_OVERFLOW_EXIT: i32 = 128 + libc::SIGSEGV;

#[cfg(target_os = "linux")]
unsafe fn current_stack_bounds() -> Option<(usize, usize)> {
    let mut attr: libc::pthread_attr_t = std::mem::zeroed();
    if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) != 0 {
        return None;
    }
    let mut base: *mut libc::c_void = std::ptr::null_mut();
    let mut size: libc::size_t = 0;
    let got_stack = libc::pthread_attr_getstack(&attr, &mut base, &mut size) == 0;
    let mut guard: libc::size_t = 0;
    let got_guard = libc::pthread_attr_getguardsize(&attr, &mut guard) == 0;
    libc::pthread_attr_destroy(&mut attr);
    if !got_stack {
        return None;
    }
    // A guard size of zero still leaves one unmapped page in practice; assume a
    // page so a fault right below the stack is still recognised.
    let guard = if got_guard && guard > 0 {
        guard
    } else {
        page_size()
    };
    Some((base as usize, guard))
}

#[cfg(not(target_os = "linux"))]
unsafe fn current_stack_bounds() -> Option<(usize, usize)> {
    None
}

fn page_size() -> usize {
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if size > 0 {
        size as usize
    } else {
        4096
    }
}

/// True when `addr` sits in the guard region immediately below the stack.
fn is_stack_overflow(addr: usize) -> bool {
    let low = STACK_LOW.load(Ordering::Relaxed);
    let guard = GUARD_SIZE.load(Ordering::Relaxed);
    if low == 0 {
        return false;
    }
    // Be generous by one page above `low`: the faulting access may be a few
    // bytes into the frame that could not be committed.
    let page = page_size();
    let bottom = low.saturating_sub(guard);
    let top = low.saturating_add(page);
    addr >= bottom && addr < top
}

unsafe extern "C" fn handler(
    signal: libc::c_int,
    info: *mut libc::siginfo_t,
    _context: *mut libc::c_void,
) {
    let addr = if info.is_null() {
        0
    } else {
        (*info).si_addr() as usize
    };

    if is_stack_overflow(addr) {
        // `write` is one of the few calls allowed from a signal handler.
        let _ = libc::write(
            libc::STDERR_FILENO,
            MESSAGE.as_ptr() as *const libc::c_void,
            MESSAGE.len(),
        );
        libc::_exit(STACK_OVERFLOW_EXIT);
    }

    // Not a stack overflow: restore the default action and re-raise so the
    // process dies exactly as it would have without this guard.
    let mut action: libc::sigaction = std::mem::zeroed();
    action.sa_sigaction = libc::SIG_DFL;
    libc::sigemptyset(&mut action.sa_mask);
    libc::sigaction(signal, &action, std::ptr::null_mut());
    libc::raise(signal);
}

/// Install the stack-overflow guard for the current thread.
///
/// Idempotent and best-effort: if any step fails the program keeps its previous
/// behaviour (a plain crash) rather than refusing to start.
pub unsafe fn install() {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some((low, guard)) = current_stack_bounds() else {
        return;
    };
    STACK_LOW.store(low, Ordering::Relaxed);
    GUARD_SIZE.store(guard, Ordering::Relaxed);

    // The alternate stack must outlive the handler, so it is leaked on purpose.
    let alt_size = std::cmp::max(libc::SIGSTKSZ, 32 * 1024);
    let alt = libc::mmap(
        std::ptr::null_mut(),
        alt_size,
        libc::PROT_READ | libc::PROT_WRITE,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        -1,
        0,
    );
    if alt == libc::MAP_FAILED {
        return;
    }
    let stack = libc::stack_t {
        ss_sp: alt,
        ss_flags: 0,
        ss_size: alt_size,
    };
    if libc::sigaltstack(&stack, std::ptr::null_mut()) != 0 {
        return;
    }

    let mut action: libc::sigaction = std::mem::zeroed();
    action.sa_sigaction = handler as usize;
    action.sa_flags = libc::SA_ONSTACK | libc::SA_SIGINFO;
    libc::sigemptyset(&mut action.sa_mask);
    libc::sigaction(libc::SIGSEGV, &action, std::ptr::null_mut());
    libc::sigaction(libc::SIGBUS, &action, std::ptr::null_mut());
}

/// C entry point so generated `main` can install the guard explicitly.
#[no_mangle]
pub unsafe extern "C" fn ori_rt_install_stack_guard() {
    install();
}

/// Run the guard installation before `main` on ELF targets.
///
/// This covers both the AOT binary and any host that loads the cdylib, without
/// the code generator having to emit a call.
#[cfg(all(target_os = "linux", not(test)))]
#[used]
#[link_section = ".init_array"]
static INSTALL_STACK_GUARD: unsafe extern "C" fn() = {
    unsafe extern "C" fn ctor() {
        install();
    }
    ctor
};
