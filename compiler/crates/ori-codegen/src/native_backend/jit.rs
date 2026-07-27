//! JIT execution backend (Rust removal Phase 3).
//!
//! Lowers the HIR into a `JITModule` (in-memory Cranelift code), resolves the
//! `ori_*` runtime symbols from the staged cdylib via `libloading`, finalizes
//! definitions, and invokes the C `main` wrapper in-process — no `.o` file,
//! no linker, no subprocess.
//!
//! Opt-in via `ORI_USE_JIT=1` in the driver. `ori compile` and `ori test`
//! remain AOT (distribution + process isolation for `ori_test_assert`).

use cranelift_jit::{JITBuilder, JITModule};
use libloading::Library;
use std::path::Path;

use crate::native_backend::NativeBackend;
use ori_hir::HirModule;

type RuntimeSymbolLookup = dyn Fn(&str) -> Option<*const u8> + Send;

/// Execute the given HIR module in-process via Cranelift JIT.
///
/// `cdylib_path` must point at the staged `ori_runtime.{dll,so,dylib}` built
/// from `ori-runtime` with `crate-type = ["cdylib"]`. The runtime's
/// `#[no_mangle] extern "C"` symbols are looked up by name and registered in
/// the `JITBuilder` so the JIT'd code can call them directly.
///
/// Returns the exit code from the C `main` wrapper. If the Ori program calls
/// `os.exit(code)`, the runtime invokes `std::process::exit(code)` and this
/// function never returns — the driver process terminates with that code,
/// matching AOT `ori run` semantics.
pub fn run_jit(
    hir: &HirModule,
    cdylib_path: &Path,
    native_libs_paths: &[std::path::PathBuf],
) -> Result<i32, String> {
    // 1. Load the runtime cdylib and any package-provided native cdylibs.
    let mut libraries = Vec::with_capacity(1 + native_libs_paths.len());
    let runtime_lib = unsafe { Library::new(cdylib_path) }
        .map_err(|e| format!("load runtime cdylib `{}`: {e}", cdylib_path.display()))?;
    libraries.push(runtime_lib);

    for lib_path in native_libs_paths {
        let lib = unsafe { Library::new(lib_path) }
            .map_err(|e| format!("load native cdylib `{}`: {e}", lib_path.display()))?;
        libraries.push(lib);
    }

    // 2. Build the JIT module with a symbol-lookup callback that resolves any
    //    `ori_*` import (as well as `strlen`/`strcmp` from the C runtime) on
    //    demand from the cdylib. This covers every `Linkage::Import` declared
    //    by `declare_stdlib` without needing to enumerate them statically.
    //
    //    `Library` is not `Send` by default in `libloading 0.8` (the handle is
    //    opaque), but platform module handles are safe to share across threads
    //    (HMODULE on Windows, `void *` from `dlopen` on Unix). The lookup fn
    //    is only invoked from the thread that finalizes definitions, so the
    //    unsafe `Send` wrapper is sound in practice.
    struct SendLibrary(Library);
    unsafe impl Send for SendLibrary {}

    let send_libs: Vec<SendLibrary> = libraries.into_iter().map(SendLibrary).collect();
    let lookup: Box<RuntimeSymbolLookup> = Box::new(move |name: &str| unsafe {
        for lib in &send_libs {
            if let Ok(sym) = lib.0.get::<unsafe extern "C" fn()>(name.as_bytes()) {
                return Some(*sym as *const () as *const u8);
            }
        }
        None
    });
    // Product flags: verifier off, opt_level none for faster `ori run` startup.
    // `with_flags` takes string pairs (not a Flags object).
    let mut builder = JITBuilder::with_flags(
        &[("enable_verifier", "false"), ("opt_level", "none")],
        cranelift_module::default_libcall_names(),
    )
    .map_err(|e| format!("JITBuilder: {e}"))?;
    builder.symbol_lookup_fn(lookup);
    let module = JITModule::new(builder);

    // 3. Lower the HIR into the JIT module (declare + define all functions
    //    and data, including the C `main` wrapper).
    let backend = NativeBackend::new(module)?.prepare(hir)?;
    let main_id = backend
        .main_func_id()
        .ok_or_else(|| "JIT entry point missing: HIR has no `main` function".to_string())?;

    // 4. Finalize definitions — this allocates executable memory, patches
    //    relocations, and makes function pointers retrievable.
    let mut module = backend.into_module();
    module
        .finalize_definitions()
        .map_err(|e| format!("JIT finalize: {e}"))?;

    // 5. Retrieve the entry pointer and invoke it. The C `main` wrapper has
    //    signature `(i32 argc, *mut u8 argv) -> i32`. We pass `0`/`null`
    //    because `ori_os_set_args` is called inside the wrapper with those
    //    values, matching the AOT behavior when no args are forwarded.
    let main_ptr = module.get_finalized_function(main_id);
    if main_ptr.is_null() {
        return Err("JIT main wrapper compiled to null address".to_string());
    }
    let entry: extern "C" fn(i32, *mut u8) -> i32 = unsafe { std::mem::transmute(main_ptr) };
    let code = entry(0, std::ptr::null_mut());

    // 6. Drop the module only after the call returns. The `Library` is owned
    //    by the symbol-lookup closure inside the `JITModule`, so dropping the
    //    module also drops the library. If the Ori program called `os.exit`,
    //    the process terminates above and we never reach here — that's
    //    expected.
    drop(module);
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_hir::HirModule;
    use smol_str::SmolStr;

    fn empty_hir() -> HirModule {
        HirModule {
            namespace: SmolStr::default(),
            structs: Vec::new(),
            enums: Vec::new(),
            traits: Vec::new(),
            trait_impls: Vec::new(),
            funcs: Vec::new(),
            consts: Vec::new(),
            externs: Vec::new(),
        }
    }

    #[test]
    fn run_jit_reports_missing_cdylib_with_descriptive_error() {
        let hir = empty_hir();
        let bogus = Path::new("/nonexistent/ori_runtime.so");
        let err = run_jit(&hir, bogus, &[]).unwrap_err();
        assert!(
            err.contains("load runtime cdylib") || err.contains("runtime cdylib"),
            "expected descriptive cdylib load error, got: {err}"
        );
    }
}
