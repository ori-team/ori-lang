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
use ori_hir::HirFunc;
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use std::io::Read;
use std::path::Path;
use std::{collections::HashMap, mem, sync::Arc};

use crate::native_backend::NativeBackend;
use ori_hir::HirModule;

type RuntimeSymbolLookup = dyn Fn(&str) -> Option<*const u8> + Send;

/// Scalar types supported by the first hosted JIT invocation boundary.
///
/// Scalar and pointer types supported by the hosted JIT invocation boundary.
///
/// Aggregate, managed, closure, and async values remain compiler-owned until
/// the Host ABI defines their ownership and layout contract. Slices, strings,
/// and bytes are accepted as pointers; the host reads them through checked
/// accessors while the module is alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JitScalarType {
    Bool,
    Int,
    Float,
    Slice,
    String,
    Bytes,
    Unsupported,
}

/// Signature metadata for one lowered Ori function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JitFunctionSignature {
    pub params: Vec<JitScalarType>,
    pub return_type: Option<JitScalarType>,
}

/// Function metadata owned by a compiled JIT module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JitFunctionInfo {
    pub name: String,
    pub signature: JitFunctionSignature,
    pub is_public: bool,
}

/// Values accepted by the hosted invocation boundary.
///
/// [`Self::Slice`], [`Self::String`], and [`Self::Bytes`] carry raw pointers;
/// they are only meaningful while the module that returned them is alive and
/// should be read through checked accessors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JitValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Slice(*const u8),
    String(*const u8),
    Bytes(*const u8),
}

/// Failure returned by a persistent hosted-JIT invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JitCallError {
    Invocation(String),
    Runtime { code: i32, message: String },
}

impl std::fmt::Display for JitCallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invocation(message) => formatter.write_str(message),
            Self::Runtime { code, message } if message.is_empty() => {
                write!(formatter, "Ori runtime trap (code {code})")
            }
            Self::Runtime { code, message } => {
                write!(formatter, "Ori runtime trap (code {code}): {message}")
            }
        }
    }
}

/// A host-owned native symbol made available to a JIT module.
///
/// The address is stored as an integer so the lookup closure can cross the
/// compiler worker's `Send` boundary. The host is responsible for keeping the
/// function address valid until every compiled module using it is dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JitHostSymbol {
    pub name: String,
    pub address: usize,
    /// Callback ID encoded as a hidden leading argument by the hosted backend.
    pub callback_id: Option<u64>,
}

struct CompiledFunction {
    info: JitFunctionInfo,
    address: *const u8,
}

type HostClearError = unsafe extern "C" fn();
type HostErrorCode = unsafe extern "C" fn() -> i32;
type HostErrorMessage = unsafe extern "C" fn() -> *const std::os::raw::c_char;
type RuntimeArcRetain = unsafe extern "C" fn(*mut u8);
type RuntimeArcRelease = unsafe extern "C" fn(*mut u8);
type RuntimeBytesLen = unsafe extern "C" fn(*const u8) -> i64;
type RuntimeStringCopyParts = unsafe extern "C" fn(*const u8, i64, *const u8, i64) -> *mut u8;
type RuntimeBytesCopyParts = unsafe extern "C" fn(*const u8) -> *mut u8;
type RuntimeIdentityQuery = unsafe extern "C" fn() -> *const std::os::raw::c_char;
type RuntimeInit = unsafe extern "C" fn() -> i32;
type RuntimeShutdown = unsafe extern "C" fn(i64) -> i32;

static RUNTIME_LEASES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct RuntimeLease {
    _library: Arc<Library>,
    shutdown: RuntimeShutdown,
}

impl Drop for RuntimeLease {
    fn drop(&mut self) {
        if RUNTIME_LEASES.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 1 {
            // SAFETY: this lease retains the runtime library until shutdown
            // returns, and the exported function is idempotent.
            if unsafe { (self.shutdown)(5_000) } != 0 {
                // A busy runtime must not be unloaded: retain one leaked
                // library handle so worker/foreign-thread code cannot jump
                // into unmapped text. The host can terminate the process or
                // explicitly coordinate shutdown through the C ABI.
                let _ = Arc::into_raw(Arc::clone(&self._library));
            }
        }
    }
}

const NATIVE_ABI_MISMATCH: &str = "native.abi_mismatch";

fn runtime_metadata_string<'a>(
    metadata: &'a serde_json::Value,
    field: &str,
    path: &Path,
) -> Result<&'a str, String> {
    metadata
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "{NATIVE_ABI_MISMATCH}: runtime metadata `{}` is missing string field `{field}`",
                path.display()
            )
        })
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "{NATIVE_ABI_MISMATCH}: cannot read runtime artifact `{}`: {error}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!(
                "{NATIVE_ABI_MISMATCH}: cannot hash runtime artifact `{}`: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn is_fresh_cargo_development_runtime(path: &Path) -> Result<bool, String> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(false);
    };
    let dependency_path = parent.join(format!("{stem}.d"));
    if !dependency_path.is_file() || !path.is_file() {
        return Ok(false);
    }
    let artifact_modified = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| {
            format!(
                "{NATIVE_ABI_MISMATCH}: cannot inspect runtime artifact `{}`: {error}",
                path.display()
            )
        })?;
    let dependencies = std::fs::read_to_string(&dependency_path).map_err(|error| {
        format!(
            "{NATIVE_ABI_MISMATCH}: cannot read Cargo runtime dependency file `{}`: {error}",
            dependency_path.display()
        )
    })?;
    let target_prefix = format!("{}:", path.display());
    let Some(dependencies) = dependencies.trim().strip_prefix(&target_prefix) else {
        return Ok(false);
    };
    let mut saw_runtime_source = false;
    for dependency in dependencies.split_ascii_whitespace() {
        let dependency = Path::new(dependency);
        if dependency.ends_with("ori-runtime/src/lib.rs") {
            saw_runtime_source = true;
        }
        if dependency.is_file()
            && std::fs::metadata(dependency)
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified > artifact_modified)
        {
            return Err(format!(
                "{NATIVE_ABI_MISMATCH}: Cargo runtime artifact `{}` is older than dependency `{}`",
                path.display(),
                dependency.display()
            ));
        }
    }
    Ok(saw_runtime_source)
}

/// Validate staged metadata before `dlopen`, so a replaced artifact cannot run
/// constructors or register symbols merely because its filename looks right.
fn validate_staged_runtime_artifact(path: &Path) -> Result<(), String> {
    let metadata_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime-link.json");
    if !metadata_path.is_file() {
        // Cargo development artifacts do not have staging metadata. Accept
        // only a fresh Cargo output proven by its adjacent dependency file;
        // arbitrary paths must carry staged identity and a digest.
        if is_fresh_cargo_development_runtime(path)? {
            return Ok(());
        }
        return Err(format!(
            "{NATIVE_ABI_MISMATCH}: runtime cdylib `{}` has no staged metadata or fresh Cargo dependency record",
            path.display()
        ));
    }
    let source = std::fs::read_to_string(&metadata_path).map_err(|error| {
        format!(
            "{NATIVE_ABI_MISMATCH}: cannot read runtime metadata `{}`: {error}",
            metadata_path.display()
        )
    })?;
    let metadata: serde_json::Value = serde_json::from_str(source.trim_start_matches('\u{feff}'))
        .map_err(|error| {
        format!(
            "{NATIVE_ABI_MISMATCH}: invalid runtime metadata `{}`: {error}",
            metadata_path.display()
        )
    })?;
    let expected_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "{NATIVE_ABI_MISMATCH}: runtime artifact path `{}` has no UTF-8 filename",
                path.display()
            )
        })?;
    for (field, expected) in [
        ("runtime_cdylib", expected_name),
        ("ori_version", ori_runtime::ORI_RUNTIME_VERSION),
        ("abi_version", ori_runtime::ORI_ABI_VERSION),
        ("target", ori_runtime::ORI_RUNTIME_TARGET),
    ] {
        let actual = runtime_metadata_string(&metadata, field, &metadata_path)?;
        if actual != expected {
            return Err(format!(
                "{NATIVE_ABI_MISMATCH}: runtime metadata `{}` has {field} `{actual}`, expected `{expected}`",
                metadata_path.display()
            ));
        }
    }
    let expected_digest =
        runtime_metadata_string(&metadata, "runtime_cdylib_sha256", &metadata_path)?;
    let actual_digest = sha256_hex(path)?;
    if !actual_digest.eq_ignore_ascii_case(expected_digest) {
        return Err(format!(
            "{NATIVE_ABI_MISMATCH}: runtime cdylib digest mismatch for `{}` (metadata {}, actual {})",
            path.display(), expected_digest, actual_digest
        ));
    }
    Ok(())
}

fn loaded_runtime_string(
    library: &Library,
    symbol: &[u8],
    display_name: &str,
) -> Result<String, String> {
    // SAFETY: identity queries are fixed zero-argument C functions returning
    // process-lifetime NUL-terminated storage. No generated code is executed.
    let query = unsafe {
        *library
            .get::<RuntimeIdentityQuery>(symbol)
            .map_err(|error| {
                format!("{NATIVE_ABI_MISMATCH}: runtime identity symbol `{display_name}`: {error}")
            })?
    };
    let pointer = unsafe { query() };
    if pointer.is_null() {
        return Err(format!(
            "{NATIVE_ABI_MISMATCH}: runtime identity `{display_name}` returned null"
        ));
    }
    // SAFETY: guaranteed by the runtime identity ABI above.
    Ok(unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned())
}

fn validate_loaded_runtime_identity(library: &Library) -> Result<(), String> {
    for (symbol, display_name, expected) in [
        (
            b"ori_rt_version".as_slice(),
            "ori_rt_version",
            ori_runtime::ORI_RUNTIME_VERSION,
        ),
        (
            b"ori_rt_abi_version".as_slice(),
            "ori_rt_abi_version",
            ori_runtime::ORI_ABI_VERSION,
        ),
        (
            b"ori_rt_target".as_slice(),
            "ori_rt_target",
            ori_runtime::ORI_RUNTIME_TARGET,
        ),
    ] {
        let actual = loaded_runtime_string(library, symbol, display_name)?;
        if actual != expected {
            return Err(format!(
                "{NATIVE_ABI_MISMATCH}: loaded runtime `{display_name}` is `{actual}`, expected `{expected}`"
            ));
        }
    }
    Ok(())
}

#[repr(C)]
struct RuntimeBytesView {
    data: *const u8,
    len: i64,
}

struct RuntimeErrorApi {
    _library: Arc<Library>,
    lease: Arc<RuntimeLease>,
    clear: HostClearError,
    code: HostErrorCode,
    message: HostErrorMessage,
    retain: RuntimeArcRetain,
    release: RuntimeArcRelease,
    bytes_len: RuntimeBytesLen,
    copy_string_parts: RuntimeStringCopyParts,
    copy_bytes_parts: RuntimeBytesCopyParts,
}

impl RuntimeErrorApi {
    fn load(path: &Path) -> Result<Self, String> {
        validate_staged_runtime_artifact(path)?;
        // SAFETY: `path` is resolved by the compiler as the runtime cdylib.
        // The library handle is retained for as long as the function pointers
        // are used by the compiled module.
        let library =
            Arc::new(unsafe { Library::new(path) }.map_err(|error| {
                format!("load runtime error API `{}`: {error}", path.display())
            })?);
        validate_loaded_runtime_identity(&library)?;
        let init = unsafe {
            *library
                .get::<RuntimeInit>(b"ori_rt_init")
                .map_err(|error| format!("runtime symbol `ori_rt_init`: {error}"))?
        };
        let shutdown = unsafe {
            *library
                .get::<RuntimeShutdown>(b"ori_rt_shutdown_ex")
                .map_err(|error| format!("runtime symbol `ori_rt_shutdown_ex`: {error}"))?
        };
        // SAFETY: identity was validated and the symbol has the generated C
        // header signature. A non-zero result means the runtime was not made
        // ready, so no JIT symbols are registered.
        if unsafe { init() } != 0 {
            return Err(format!(
                "{NATIVE_ABI_MISMATCH}: runtime initialization failed before JIT registration"
            ));
        }
        RUNTIME_LEASES.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let lease = Arc::new(RuntimeLease {
            _library: Arc::clone(&library),
            shutdown,
        });
        // SAFETY: these symbols are part of the hosted runtime contract and
        // have the exact C signatures declared below in the generated header.
        let clear = unsafe {
            *library
                .get::<HostClearError>(b"ori_host_clear_error")
                .map_err(|error| format!("runtime symbol `ori_host_clear_error`: {error}"))?
        };
        let code = unsafe {
            *library
                .get::<HostErrorCode>(b"ori_host_error_code")
                .map_err(|error| format!("runtime symbol `ori_host_error_code`: {error}"))?
        };
        let message = unsafe {
            *library
                .get::<HostErrorMessage>(b"ori_host_error_message")
                .map_err(|error| format!("runtime symbol `ori_host_error_message`: {error}"))?
        };
        let retain = unsafe {
            *library
                .get::<RuntimeArcRetain>(b"ori_arc_retain")
                .map_err(|error| format!("runtime symbol `ori_arc_retain`: {error}"))?
        };
        let release = unsafe {
            *library
                .get::<RuntimeArcRelease>(b"ori_arc_release")
                .map_err(|error| format!("runtime symbol `ori_arc_release`: {error}"))?
        };
        let bytes_len = unsafe {
            *library
                .get::<RuntimeBytesLen>(b"ori_bytes_len")
                .map_err(|error| format!("runtime symbol `ori_bytes_len`: {error}"))?
        };
        let copy_string_parts = unsafe {
            *library
                .get::<RuntimeStringCopyParts>(b"ori_string_concat_parts")
                .map_err(|error| format!("runtime symbol `ori_string_concat_parts`: {error}"))?
        };
        let copy_bytes_parts = unsafe {
            *library
                .get::<RuntimeBytesCopyParts>(b"ori_bytes_copy_parts")
                .map_err(|error| format!("runtime symbol `ori_bytes_copy_parts`: {error}"))?
        };
        Ok(Self {
            _library: library,
            lease,
            clear,
            code,
            message,
            retain,
            release,
            bytes_len,
            copy_string_parts,
            copy_bytes_parts,
        })
    }

    fn clear(&self) {
        // SAFETY: the function pointer came from the retained runtime library.
        unsafe { (self.clear)() };
    }

    fn take(&self) -> Option<(i32, String)> {
        // SAFETY: both function pointers came from the retained runtime
        // library; the returned message is borrowed until the next operation.
        let code = unsafe { (self.code)() };
        if code == 0 {
            return None;
        }
        let message = unsafe { (self.message)() };
        let message = if message.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        };
        Some((code, message))
    }

    fn value_owner(&self) -> JitManagedValueOwner {
        JitManagedValueOwner {
            api: Arc::new(RuntimeValueApi {
                _library: Arc::clone(&self._library),
                _lease: Arc::clone(&self.lease),
                retain: self.retain,
                release: self.release,
                bytes_len: self.bytes_len,
                copy_string_parts: self.copy_string_parts,
                copy_bytes_parts: self.copy_bytes_parts,
            }),
        }
    }
}

struct RuntimeValueApi {
    _library: Arc<Library>,
    _lease: Arc<RuntimeLease>,
    retain: RuntimeArcRetain,
    release: RuntimeArcRelease,
    bytes_len: RuntimeBytesLen,
    copy_string_parts: RuntimeStringCopyParts,
    copy_bytes_parts: RuntimeBytesCopyParts,
}

/// Runtime ownership operations for a managed value returned by hosted JIT.
///
/// This is intentionally a small, opaque capability. The host cannot obtain
/// the function pointers or runtime library directly, but `ori-embed` can use
/// the capability to keep returned strings/bytes/slices alive across calls and
/// module replacement.
#[derive(Clone)]
pub struct JitManagedValueOwner {
    api: Arc<RuntimeValueApi>,
}

impl std::fmt::Debug for JitManagedValueOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JitManagedValueOwner(..)")
    }
}

impl JitManagedValueOwner {
    /// Copy a Rust string into this runtime as one owned Ori value.
    pub fn copy_string_input(&self, value: &str) -> Result<*const u8, String> {
        if value.as_bytes().contains(&0) {
            return Err("hosted Ori strings cannot contain an interior NUL byte".to_string());
        }
        let length = i64::try_from(value.len())
            .map_err(|_| "hosted string length exceeds the runtime ABI limit".to_string())?;
        // SAFETY: `value` is readable for `length` bytes during the call. The
        // second part is empty, so its null pointer is never dereferenced.
        let pointer =
            unsafe { (self.api.copy_string_parts)(value.as_ptr(), length, std::ptr::null(), 0) };
        if pointer.is_null() {
            Err("runtime returned a null managed string".to_string())
        } else {
            Ok(pointer)
        }
    }

    /// Copy a Rust byte slice into this runtime as one owned Ori value.
    pub fn copy_bytes_input(&self, value: &[u8]) -> Result<*const u8, String> {
        let length = i64::try_from(value.len())
            .map_err(|_| "hosted bytes length exceeds the runtime ABI limit".to_string())?;
        let view = RuntimeBytesView {
            data: if value.is_empty() {
                std::ptr::null()
            } else {
                value.as_ptr()
            },
            len: length,
        };
        // SAFETY: `view` has the runtime's C layout and its data is readable
        // for `len` bytes for the duration of the copying call.
        let pointer = unsafe { (self.api.copy_bytes_parts)((&raw const view).cast::<u8>()) };
        if pointer.is_null() {
            Err("runtime returned a null managed bytes value".to_string())
        } else {
            Ok(pointer)
        }
    }

    /// Copy and consume one owned Ori string returned by hosted code.
    ///
    /// # Safety
    ///
    /// `pointer` must be one owned, live Ori string reference produced by this
    /// runtime. This method consumes that reference even when UTF-8 validation
    /// fails.
    pub unsafe fn take_string(&self, pointer: *const u8) -> Result<String, String> {
        let result = if pointer.is_null() {
            Err("hosted function returned a null string pointer".to_string())
        } else {
            // SAFETY: upheld by the caller; the copy completes before release.
            unsafe { CStr::from_ptr(pointer.cast()) }
                .to_str()
                .map(str::to_owned)
                .map_err(|_| "hosted function returned invalid UTF-8".to_string())
        };
        // SAFETY: this method consumes the caller-provided owned reference.
        unsafe { self.release(pointer) };
        result
    }

    /// Copy one borrowed Ori string without consuming its ownership reference.
    ///
    /// # Safety
    ///
    /// `pointer` must remain a live Ori string allocation for the duration of
    /// this call.
    pub unsafe fn copy_borrowed_string(&self, pointer: *const u8) -> Result<String, String> {
        if pointer.is_null() {
            return Err("host callback received a null string pointer".to_string());
        }
        // SAFETY: upheld by the caller; the copy completes before returning.
        unsafe { CStr::from_ptr(pointer.cast()) }
            .to_str()
            .map(str::to_owned)
            .map_err(|_| "host callback received invalid UTF-8".to_string())
    }

    /// Copy and consume one owned Ori bytes value returned by hosted code.
    ///
    /// # Safety
    ///
    /// `pointer` must be one owned, live Ori bytes reference produced by this
    /// runtime. This method consumes that reference after copying its exact
    /// registered payload.
    pub unsafe fn take_bytes(&self, pointer: *const u8) -> Result<Vec<u8>, String> {
        let length = unsafe { self.bytes_len(pointer) };
        let result = match length {
            Some(length) if length <= isize::MAX as usize && !pointer.is_null() => {
                // SAFETY: the runtime registry supplied the exact live length.
                Ok(unsafe { std::slice::from_raw_parts(pointer, length) }.to_vec())
            }
            Some(0) => Ok(Vec::new()),
            Some(_) => Err("hosted bytes length exceeds the Rust slice limit".to_string()),
            None => Err("hosted function returned an invalid bytes pointer".to_string()),
        };
        // SAFETY: this method consumes the caller-provided owned reference.
        unsafe { self.release(pointer) };
        result
    }

    /// Copy one borrowed Ori bytes value without consuming its reference.
    ///
    /// # Safety
    ///
    /// `pointer` must remain a live managed Ori bytes allocation for the
    /// duration of this call.
    pub unsafe fn copy_borrowed_bytes(&self, pointer: *const u8) -> Result<Vec<u8>, String> {
        let Some(length) = (unsafe { self.bytes_len(pointer) }) else {
            return Err("host callback received an invalid bytes pointer".to_string());
        };
        if length == 0 {
            return Ok(Vec::new());
        }
        if pointer.is_null() || length > isize::MAX as usize {
            return Err("host callback bytes length exceeds the Rust slice limit".to_string());
        }
        // SAFETY: the runtime registry supplied the exact live length and the
        // caller keeps the allocation live throughout this copy.
        Ok(unsafe { std::slice::from_raw_parts(pointer, length) }.to_vec())
    }

    /// Retain a runtime allocation before passing it as an owned parameter.
    ///
    /// # Safety
    ///
    /// `pointer` must be either a live allocation from this runtime or a null
    /// pointer. Foreign pointers are accepted by the runtime as no-ops, but
    /// they must not be used with the managed length accessor.
    pub unsafe fn retain(&self, pointer: *const u8) {
        if !pointer.is_null() {
            // SAFETY: caller guarantees the pointer is a live runtime value;
            // the function pointer comes from the retained runtime library.
            unsafe { (self.api.retain)(pointer as *mut u8) };
        }
    }

    /// Release one ownership reference held by a hosted value.
    ///
    /// # Safety
    ///
    /// `pointer` must be a pointer previously retained or returned by Ori and
    /// must not be released more times than it was retained.
    pub unsafe fn release(&self, pointer: *const u8) {
        if !pointer.is_null() {
            // SAFETY: caller guarantees the ownership count is balanced.
            unsafe { (self.api.release)(pointer as *mut u8) };
        }
    }

    /// Return the exact byte length of a managed bytes allocation.
    ///
    /// # Safety
    ///
    /// `pointer` must refer to a live managed Ori bytes allocation retained by
    /// this owner. A foreign or non-bytes pointer returns `None` only if the
    /// runtime rejects its registry lookup; callers must still uphold the
    /// pointer lifetime contract before constructing a slice.
    pub unsafe fn bytes_len(&self, pointer: *const u8) -> Option<usize> {
        if pointer.is_null() {
            return Some(0);
        }
        // SAFETY: function pointer is resolved from the retained runtime
        // library and the caller guarantees pointer validity.
        let length = unsafe { (self.api.bytes_len)(pointer) };
        usize::try_from(length).ok()
    }
}

/// A finalized Cranelift module that remains executable until it is dropped.
///
/// Unlike [`run_jit`], this type does not invoke `main` or discard executable
/// memory after one call. It is the ownership unit for hosted function
/// addresses. The current boundary intentionally supports only homogeneous
/// scalar arguments and at most four arguments per call.
pub struct CompiledJitModule {
    _module: JITModule,
    functions: HashMap<String, CompiledFunction>,
    main_address: Option<*const u8>,
    global_drop_address: Option<*const u8>,
    runtime_error_api: RuntimeErrorApi,
}

impl Drop for CompiledJitModule {
    fn drop(&mut self) {
        let Some(address) = self.global_drop_address.take() else {
            return;
        };
        // SAFETY: the teardown address belongs to `_module`, which remains
        // alive until after this `Drop` implementation returns. The backend
        // declares it with a zero-argument, void C signature.
        let teardown: unsafe extern "C" fn() = unsafe { mem::transmute(address) };
        unsafe { teardown() };
    }
}

impl std::fmt::Debug for CompiledJitModule {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledJitModule")
            .field("functions", &self.functions.len())
            .field("has_main", &self.main_address.is_some())
            .finish_non_exhaustive()
    }
}

impl CompiledJitModule {
    /// Compile and finalize one HIR module without invoking an entry point.
    pub fn compile(
        hir: &HirModule,
        cdylib_path: &Path,
        native_libs_paths: &[std::path::PathBuf],
    ) -> Result<Self, String> {
        Self::compile_with_host_symbols(hir, cdylib_path, native_libs_paths, &[])
    }

    /// Compile and finalize one HIR module with host-owned native symbols.
    pub fn compile_with_host_symbols(
        hir: &HirModule,
        cdylib_path: &Path,
        native_libs_paths: &[std::path::PathBuf],
        host_symbols: &[JitHostSymbol],
    ) -> Result<Self, String> {
        let runtime_error_api = RuntimeErrorApi::load(cdylib_path)?;
        let lookup = runtime_symbol_lookup(cdylib_path, native_libs_paths, host_symbols)?;
        let mut builder = JITBuilder::with_flags(
            &[("enable_verifier", "false"), ("opt_level", "none")],
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| format!("JITBuilder: {e}"))?;
        builder.symbol_lookup_fn(lookup);
        let module = JITModule::new(builder);
        let mut backend = NativeBackend::new(module)?;
        backend.set_hosted_traps(true);
        let callback_ids = hir
            .externs
            .iter()
            .filter_map(|external| {
                let ori_hir::HirExtern::Func {
                    path, name, abi, ..
                } = external
                else {
                    return None;
                };
                if abi != "host" {
                    return None;
                }
                host_symbols
                    .iter()
                    .find(|symbol| {
                        symbol.callback_id.is_some()
                            && (symbol.name == name.as_str() || symbol.name == path.as_str())
                    })
                    .and_then(|symbol| symbol.callback_id)
                    .map(|id| (path.to_string(), id))
            })
            .collect::<Vec<_>>();
        backend.set_host_callback_ids(&callback_ids);
        let backend = backend.prepare(hir)?;
        let main_id = backend.main_func_id();
        let global_init_id = backend.global_init_func_id();
        let global_drop_id = backend.global_drop_func_id();
        let function_ids = hir
            .funcs
            .iter()
            .filter_map(|function| {
                backend
                    .function_id(function.name.as_str())
                    .map(|id| (function, id))
            })
            .collect::<Vec<_>>();

        let mut module = backend.into_module();
        module
            .finalize_definitions()
            .map_err(|e| format!("JIT finalize: {e}"))?;
        let global_drop_address = global_drop_id.map(|id| module.get_finalized_function(id));

        if let Some(init_id) = global_init_id {
            runtime_error_api.clear();
            let address = module.get_finalized_function(init_id);
            // SAFETY: `init_id` was declared with the zero-argument, void
            // signature by `NativeBackend`. The finalized module owns the
            // executable address for this entire call.
            let initialize: unsafe extern "C" fn() = unsafe { mem::transmute(address) };
            unsafe { initialize() };
            if let Some((code, message)) = runtime_error_api.take() {
                if let Some(address) = global_drop_address {
                    // SAFETY: paired teardown has the same module-owned
                    // zero-argument C ABI and remains live here.
                    let teardown: unsafe extern "C" fn() = unsafe { mem::transmute(address) };
                    unsafe { teardown() };
                }
                return Err(format!(
                    "hosted module global initialization failed (runtime error {code}): {message}"
                ));
            }
        }

        let functions = function_ids
            .into_iter()
            .map(|(function, id)| {
                let address = module.get_finalized_function(id);
                (
                    function.name.to_string(),
                    CompiledFunction {
                        info: JitFunctionInfo {
                            name: function.name.to_string(),
                            signature: signature_for(function),
                            is_public: function.is_public,
                        },
                        address,
                    },
                )
            })
            .collect();
        let main_address = main_id.map(|id| module.get_finalized_function(id));

        Ok(Self {
            _module: module,
            functions,
            main_address,
            global_drop_address,
            runtime_error_api,
        })
    }

    pub fn functions(&self) -> impl Iterator<Item = &JitFunctionInfo> {
        self.functions.values().map(|function| &function.info)
    }

    pub fn function(&self, name: &str) -> Option<&JitFunctionInfo> {
        self.functions.get(name).map(|function| &function.info)
    }

    /// Return an ownership capability for managed values produced by this JIT
    /// generation. Keeping the capability alive also keeps the runtime
    /// library loaded after the executable generation is retired.
    pub fn managed_value_owner(&self) -> JitManagedValueOwner {
        self.runtime_error_api.value_owner()
    }

    /// Invoke a scalar function while this module remains alive.
    pub fn call(&self, name: &str, args: &[JitValue]) -> Result<Option<JitValue>, JitCallError> {
        self.validate_call(name, args)?;
        let function = self.functions.get(name).ok_or_else(|| {
            JitCallError::Invocation(format!("JIT function `{name}` was not found"))
        })?;
        self.runtime_error_api.clear();

        let result = match argument_type(args) {
            None => call_without_arguments(function.address, &function.info.signature),
            Some(JitScalarType::Int) => {
                let values = args
                    .iter()
                    .map(|value| match value {
                        JitValue::Int(value) => *value,
                        _ => unreachable!("call signature was validated above"),
                    })
                    .collect::<Vec<_>>();
                call_with_int_arguments(function.address, &function.info.signature, &values)
            }
            Some(JitScalarType::Float) => {
                let values = args
                    .iter()
                    .map(|value| match value {
                        JitValue::Float(value) => *value,
                        _ => unreachable!("call signature was validated above"),
                    })
                    .collect::<Vec<_>>();
                call_with_float_arguments(function.address, &function.info.signature, &values)
            }
            Some(JitScalarType::Bool) => {
                let values = args
                    .iter()
                    .map(|value| match value {
                        JitValue::Bool(value) => i8::from(*value),
                        _ => unreachable!("call signature was validated above"),
                    })
                    .collect::<Vec<_>>();
                call_with_bool_arguments(function.address, &function.info.signature, &values)
            }
            Some(JitScalarType::Slice) => {
                let values = args
                    .iter()
                    .map(|value| match value {
                        JitValue::Slice(pointer) => *pointer,
                        _ => unreachable!("call signature was validated above"),
                    })
                    .collect::<Vec<_>>();
                call_with_pointer_arguments(function.address, &function.info.signature, &values)
            }
            Some(JitScalarType::String) => {
                let values = args
                    .iter()
                    .map(|value| match value {
                        JitValue::String(pointer) => *pointer,
                        _ => unreachable!("call signature was validated above"),
                    })
                    .collect::<Vec<_>>();
                call_with_pointer_arguments(function.address, &function.info.signature, &values)
            }
            Some(JitScalarType::Bytes) => {
                let values = args
                    .iter()
                    .map(|value| match value {
                        JitValue::Bytes(pointer) => *pointer,
                        _ => unreachable!("call signature was validated above"),
                    })
                    .collect::<Vec<_>>();
                call_with_pointer_arguments(function.address, &function.info.signature, &values)
            }
            Some(JitScalarType::Unsupported) => Err(
                "hosted JIT calls support only homogeneous bool, int, float, slice, string, or bytes arguments"
                    .to_string(),
            ),
        };
        if let Some((code, message)) = self.runtime_error_api.take() {
            return Err(JitCallError::Runtime { code, message });
        }
        result.map_err(JitCallError::Invocation)
    }

    /// Validate a call without invoking generated code.
    ///
    /// Hosts use this before retaining managed arguments so a rejected call
    /// cannot leak an extra ownership reference.
    pub fn validate_call(&self, name: &str, args: &[JitValue]) -> Result<(), JitCallError> {
        let function = self.functions.get(name).ok_or_else(|| {
            JitCallError::Invocation(format!("JIT function `{name}` was not found"))
        })?;
        if function.address.is_null() {
            return Err(JitCallError::Invocation(format!(
                "JIT function `{name}` compiled to a null address"
            )));
        }
        if args.len() > 4 {
            return Err(JitCallError::Invocation(
                "hosted JIT calls support at most four arguments".to_string(),
            ));
        }
        validate_call_signature(&function.info.signature, args)
            .map_err(JitCallError::Invocation)?;
        Ok(())
    }

    /// Invoke the process entry point, preserving the legacy `ori run` ABI.
    pub fn call_main(&self) -> Result<i32, String> {
        let address = self
            .main_address
            .ok_or_else(|| "JIT entry point missing: HIR has no `main` function".to_string())?;
        if address.is_null() {
            return Err("JIT main wrapper compiled to null address".to_string());
        }
        // SAFETY: `main_address` is the finalized pointer for the backend's
        // generated C `main` wrapper, whose ABI is fixed by `NativeBackend`.
        let entry: extern "C" fn(i32, *mut u8) -> i32 = unsafe { mem::transmute(address) };
        Ok(entry(0, std::ptr::null_mut()))
    }
}

fn scalar_type_for(ty: &ori_types::Ty) -> JitScalarType {
    match ty {
        ori_types::Ty::Bool => JitScalarType::Bool,
        ori_types::Ty::Int => JitScalarType::Int,
        ori_types::Ty::Float => JitScalarType::Float,
        ori_types::Ty::Slice(_) => JitScalarType::Slice,
        ori_types::Ty::String => JitScalarType::String,
        ori_types::Ty::Bytes => JitScalarType::Bytes,
        _ => JitScalarType::Unsupported,
    }
}

fn signature_for(function: &HirFunc) -> JitFunctionSignature {
    JitFunctionSignature {
        params: function
            .params
            .iter()
            .map(|param| scalar_type_for(&param.ty))
            .collect(),
        return_type: (!matches!(function.return_ty, ori_types::Ty::Void))
            .then(|| scalar_type_for(&function.return_ty)),
    }
}

fn value_type(value: JitValue) -> JitScalarType {
    match value {
        JitValue::Bool(_) => JitScalarType::Bool,
        JitValue::Int(_) => JitScalarType::Int,
        JitValue::Float(_) => JitScalarType::Float,
        JitValue::Slice(_) => JitScalarType::Slice,
        JitValue::String(_) => JitScalarType::String,
        JitValue::Bytes(_) => JitScalarType::Bytes,
    }
}

fn argument_type(args: &[JitValue]) -> Option<JitScalarType> {
    args.first().copied().map(value_type)
}

fn validate_call_signature(
    signature: &JitFunctionSignature,
    args: &[JitValue],
) -> Result<(), String> {
    if signature.params.len() != args.len() {
        return Err(format!(
            "JIT function expects {} argument(s), received {}",
            signature.params.len(),
            args.len()
        ));
    }
    let Some(first_type) = argument_type(args) else {
        if signature.params.is_empty() {
            return Ok(());
        }
        return Err("JIT function has unsupported non-scalar arguments".to_string());
    };
    if first_type == JitScalarType::Unsupported
        || args.iter().any(|arg| value_type(*arg) != first_type)
        || signature.params.iter().any(|param| *param != first_type)
    {
        return Err(
            "hosted JIT calls support only homogeneous bool, int, float, slice, string, or bytes arguments"
                .to_string(),
        );
    }
    Ok(())
}

macro_rules! define_invoker {
    ($name:ident, $argument:ty, $return:ty) => {
        unsafe fn $name(address: *const u8, args: &[$argument]) -> $return {
            // SAFETY: `address` comes from `JITModule::get_finalized_function`;
            // the caller validates the exact homogeneous parameter and return
            // ABI before selecting this arity/type-specific invoker.
            match args {
                [] => {
                    let function: extern "C" fn() -> $return = unsafe { mem::transmute(address) };
                    function()
                }
                [a] => {
                    let function: extern "C" fn($argument) -> $return =
                        unsafe { mem::transmute(address) };
                    function(*a)
                }
                [a, b] => {
                    let function: extern "C" fn($argument, $argument) -> $return =
                        unsafe { mem::transmute(address) };
                    function(*a, *b)
                }
                [a, b, c] => {
                    let function: extern "C" fn($argument, $argument, $argument) -> $return =
                        unsafe { mem::transmute(address) };
                    function(*a, *b, *c)
                }
                [a, b, c, d] => {
                    let function: extern "C" fn(
                        $argument,
                        $argument,
                        $argument,
                        $argument,
                    ) -> $return = unsafe { mem::transmute(address) };
                    function(*a, *b, *c, *d)
                }
                _ => unreachable!("hosted JIT call arity was checked before invocation"),
            }
        }
    };
}

define_invoker!(invoke_int_i64, i64, i64);
define_invoker!(invoke_int_f64, i64, f64);
define_invoker!(invoke_int_i8, i64, i8);
define_invoker!(invoke_int_void, i64, ());
define_invoker!(invoke_int_ptr, i64, *mut u8);
define_invoker!(invoke_float_i64, f64, i64);
define_invoker!(invoke_float_f64, f64, f64);
define_invoker!(invoke_float_i8, f64, i8);
define_invoker!(invoke_float_void, f64, ());
define_invoker!(invoke_float_ptr, f64, *mut u8);
define_invoker!(invoke_bool_i64, i8, i64);
define_invoker!(invoke_bool_f64, i8, f64);
define_invoker!(invoke_bool_i8, i8, i8);
define_invoker!(invoke_bool_void, i8, ());
define_invoker!(invoke_bool_ptr, i8, *mut u8);

fn call_without_arguments(
    address: *const u8,
    signature: &JitFunctionSignature,
) -> Result<Option<JitValue>, String> {
    match signature.return_type {
        None => {
            unsafe { invoke_int_void(address, &[]) };
            Ok(None)
        }
        Some(JitScalarType::Int) => {
            Ok(Some(JitValue::Int(unsafe { invoke_int_i64(address, &[]) })))
        }
        Some(JitScalarType::Float) => Ok(Some(JitValue::Float(unsafe {
            invoke_int_f64(address, &[])
        }))),
        Some(JitScalarType::Bool) => Ok(Some(JitValue::Bool(unsafe {
            invoke_int_i8(address, &[]) != 0
        }))),
        Some(JitScalarType::Slice) => Ok(Some(JitValue::Slice(unsafe {
            invoke_int_ptr(address, &[])
        }))),
        Some(JitScalarType::String) => Ok(Some(JitValue::String(unsafe {
            invoke_int_ptr(address, &[])
        }))),
        Some(JitScalarType::Bytes) => Ok(Some(JitValue::Bytes(unsafe {
            invoke_int_ptr(address, &[])
        }))),
        Some(JitScalarType::Unsupported) => {
            Err("JIT function has an unsupported return type".to_string())
        }
    }
}

fn call_with_int_arguments(
    address: *const u8,
    signature: &JitFunctionSignature,
    args: &[i64],
) -> Result<Option<JitValue>, String> {
    match signature.return_type {
        None => {
            unsafe { invoke_int_void(address, args) };
            Ok(None)
        }
        Some(JitScalarType::Int) => Ok(Some(JitValue::Int(unsafe {
            invoke_int_i64(address, args)
        }))),
        Some(JitScalarType::Float) => Ok(Some(JitValue::Float(unsafe {
            invoke_int_f64(address, args)
        }))),
        Some(JitScalarType::Bool) => Ok(Some(JitValue::Bool(unsafe {
            invoke_int_i8(address, args) != 0
        }))),
        Some(JitScalarType::Slice) => Ok(Some(JitValue::Slice(unsafe {
            invoke_int_ptr(address, args)
        }))),
        Some(JitScalarType::String) => Ok(Some(JitValue::String(unsafe {
            invoke_int_ptr(address, args)
        }))),
        Some(JitScalarType::Bytes) => Ok(Some(JitValue::Bytes(unsafe {
            invoke_int_ptr(address, args)
        }))),
        Some(JitScalarType::Unsupported) => {
            Err("JIT function has an unsupported return type".to_string())
        }
    }
}

fn call_with_float_arguments(
    address: *const u8,
    signature: &JitFunctionSignature,
    args: &[f64],
) -> Result<Option<JitValue>, String> {
    match signature.return_type {
        None => {
            unsafe { invoke_float_void(address, args) };
            Ok(None)
        }
        Some(JitScalarType::Int) => Ok(Some(JitValue::Int(unsafe {
            invoke_float_i64(address, args)
        }))),
        Some(JitScalarType::Float) => Ok(Some(JitValue::Float(unsafe {
            invoke_float_f64(address, args)
        }))),
        Some(JitScalarType::Bool) => Ok(Some(JitValue::Bool(unsafe {
            invoke_float_i8(address, args) != 0
        }))),
        Some(JitScalarType::Slice) => Ok(Some(JitValue::Slice(unsafe {
            invoke_float_ptr(address, args)
        }))),
        Some(JitScalarType::String) => Ok(Some(JitValue::String(unsafe {
            invoke_float_ptr(address, args)
        }))),
        Some(JitScalarType::Bytes) => Ok(Some(JitValue::Bytes(unsafe {
            invoke_float_ptr(address, args)
        }))),
        Some(JitScalarType::Unsupported) => {
            Err("JIT function has an unsupported return type".to_string())
        }
    }
}

/// Invoke a function whose arguments are all pointer-sized window or heap handles (slice, string, bytes).
///
/// The host passes opaque pointers; the generated function receives them as
/// raw pointer-sized integers (the runtime ABI is pointer). Returns are
/// dispatched exactly like the homogeneous scalar callers.
fn call_with_pointer_arguments(
    address: *const u8,
    signature: &JitFunctionSignature,
    args: &[*const u8],
) -> Result<Option<JitValue>, String> {
    let args = args
        .iter()
        .map(|pointer| *pointer as usize as i64)
        .collect::<Vec<_>>();
    match signature.return_type {
        None => {
            unsafe { invoke_int_void(address, &args) };
            Ok(None)
        }
        Some(JitScalarType::Int) => Ok(Some(JitValue::Int(unsafe {
            invoke_int_i64(address, &args)
        }))),
        Some(JitScalarType::Float) => Ok(Some(JitValue::Float(unsafe {
            invoke_int_f64(address, &args)
        }))),
        Some(JitScalarType::Bool) => Ok(Some(JitValue::Bool(unsafe {
            invoke_int_i8(address, &args) != 0
        }))),
        Some(JitScalarType::Slice) => Ok(Some(JitValue::Slice(unsafe {
            invoke_int_ptr(address, &args)
        }))),
        Some(JitScalarType::String) => Ok(Some(JitValue::String(unsafe {
            invoke_int_ptr(address, &args)
        }))),
        Some(JitScalarType::Bytes) => Ok(Some(JitValue::Bytes(unsafe {
            invoke_int_ptr(address, &args)
        }))),
        Some(JitScalarType::Unsupported) => {
            Err("JIT function has an unsupported return type".to_string())
        }
    }
}

fn call_with_bool_arguments(
    address: *const u8,
    signature: &JitFunctionSignature,
    args: &[i8],
) -> Result<Option<JitValue>, String> {
    match signature.return_type {
        None => {
            unsafe { invoke_bool_void(address, args) };
            Ok(None)
        }
        Some(JitScalarType::Int) => Ok(Some(JitValue::Int(unsafe {
            invoke_bool_i64(address, args)
        }))),
        Some(JitScalarType::Float) => Ok(Some(JitValue::Float(unsafe {
            invoke_bool_f64(address, args)
        }))),
        Some(JitScalarType::Bool) => Ok(Some(JitValue::Bool(unsafe {
            invoke_bool_i8(address, args) != 0
        }))),
        Some(JitScalarType::Slice) => Ok(Some(JitValue::Slice(unsafe {
            invoke_bool_ptr(address, args)
        }))),
        Some(JitScalarType::String) => Ok(Some(JitValue::String(unsafe {
            invoke_bool_ptr(address, args)
        }))),
        Some(JitScalarType::Bytes) => Ok(Some(JitValue::Bytes(unsafe {
            invoke_bool_ptr(address, args)
        }))),
        Some(JitScalarType::Unsupported) => {
            Err("JIT function has an unsupported return type".to_string())
        }
    }
}

struct SendLibrary(Library);

// SAFETY: libloading's platform handles are process-level module references.
// The lookup closure only reads symbols while the JIT module is finalized.
unsafe impl Send for SendLibrary {}

fn runtime_symbol_lookup(
    cdylib_path: &Path,
    native_libs_paths: &[std::path::PathBuf],
    host_symbols: &[JitHostSymbol],
) -> Result<Box<RuntimeSymbolLookup>, String> {
    validate_staged_runtime_artifact(cdylib_path)?;
    let mut libraries = Vec::with_capacity(1 + native_libs_paths.len());
    // SAFETY: the compiler resolved this path as a runtime artifact and the
    // library handle is retained by the lookup closure for the JIT lifetime.
    let runtime_lib = unsafe { Library::new(cdylib_path) }
        .map_err(|e| format!("load runtime cdylib `{}`: {e}", cdylib_path.display()))?;
    validate_loaded_runtime_identity(&runtime_lib)?;
    libraries.push(runtime_lib);
    for lib_path in native_libs_paths {
        // SAFETY: native library paths come from the resolved package graph;
        // the handle is retained by the lookup closure for the JIT lifetime.
        let lib = unsafe { Library::new(lib_path) }
            .map_err(|e| format!("load native cdylib `{}`: {e}", lib_path.display()))?;
        libraries.push(lib);
    }

    let send_libs: Vec<SendLibrary> = libraries.into_iter().map(SendLibrary).collect();
    let host_addresses: HashMap<String, usize> = host_symbols
        .iter()
        .map(|symbol| (symbol.name.clone(), symbol.address))
        .collect();
    Ok(Box::new(move |name: &str| unsafe {
        for lib in &send_libs {
            // SAFETY: lookup only obtains the symbol address; the generated
            // call site supplies the symbol's ABI declared by the compiler.
            if let Ok(sym) = lib.0.get::<unsafe extern "C" fn()>(name.as_bytes()) {
                return Some(*sym as *const () as *const u8);
            }
        }
        host_addresses
            .get(name)
            .copied()
            .map(|address| address as *const u8)
    }))
}

/// Execute the given HIR module in-process via Cranelift JIT.
///
/// `cdylib_path` must point at the staged `ori_runtime.{dll,so,dylib}` built
/// from `ori-runtime` with `crate-type = ["cdylib"]`. The runtime's
/// `#[no_mangle] extern "C"` symbols are looked up by name and registered in
/// the `JITBuilder` so the JIT'd code can call them directly.
///
/// Returns the exit code from the C `main` wrapper. If the Ori program calls
/// `os.exit(code)`, the runtime invokes `std::process::exit(code)` and this
pub fn run_jit(
    hir: &HirModule,
    cdylib_path: &Path,
    native_libs_paths: &[std::path::PathBuf],
) -> Result<i32, String> {
    run_jit_with_args(hir, cdylib_path, native_libs_paths, &[])
}

/// Run JIT compiled HIR module with argument forwarding to `main(argc, argv)`.
pub fn run_jit_with_args(
    hir: &HirModule,
    cdylib_path: &Path,
    native_libs_paths: &[std::path::PathBuf],
    args: &[String],
) -> Result<i32, String> {
    // 1. Load the runtime cdylib and any package-provided native cdylibs.
    validate_staged_runtime_artifact(cdylib_path)?;
    let mut libraries = Vec::with_capacity(1 + native_libs_paths.len());
    let runtime_lib = unsafe { Library::new(cdylib_path) }
        .map_err(|e| format!("load runtime cdylib `{}`: {e}", cdylib_path.display()))?;
    validate_loaded_runtime_identity(&runtime_lib)?;
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
    //    signature `(i32 argc, *mut *mut c_char argv) -> i32`.
    let main_ptr = module.get_finalized_function(main_id);
    if main_ptr.is_null() {
        return Err("JIT main wrapper compiled to null address".to_string());
    }
    let entry: extern "C" fn(i32, *mut *mut std::ffi::c_char) -> i32 =
        unsafe { std::mem::transmute(main_ptr) };

    let c_args: Vec<std::ffi::CString> = args
        .iter()
        .map(|arg| std::ffi::CString::new(arg.as_str()).unwrap_or_default())
        .collect();
    let mut c_ptrs: Vec<*mut std::ffi::c_char> = c_args
        .iter()
        .map(|c_str| c_str.as_ptr() as *mut std::ffi::c_char)
        .collect();
    c_ptrs.push(std::ptr::null_mut());

    let (argc, argv) = if args.is_empty() {
        (0, std::ptr::null_mut())
    } else {
        (args.len() as i32, c_ptrs.as_mut_ptr())
    };

    let code = entry(argc, argv);

    // 6. Drop the module only after the call returns.
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

    #[test]
    fn staged_runtime_digest_is_verified_before_loading() {
        let directory = std::env::temp_dir().join(format!(
            "ori_jit_runtime_identity_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("create identity fixture");
        let artifact = directory.join(if cfg!(windows) {
            "ori_runtime.dll"
        } else if cfg!(target_os = "macos") {
            "libori_runtime.dylib"
        } else {
            "libori_runtime.so"
        });
        std::fs::write(&artifact, b"runtime artifact bytes").expect("write fake artifact");
        let digest = sha256_hex(&artifact).expect("hash fake artifact");
        let error = validate_staged_runtime_artifact(&artifact)
            .expect_err("an arbitrary unstaged cdylib must be rejected before loading");
        assert!(error.contains(NATIVE_ABI_MISMATCH), "{error}");
        assert!(error.contains("no staged metadata"), "{error}");
        let metadata = serde_json::json!({
            "target": ori_runtime::ORI_RUNTIME_TARGET,
            "runtime": "unused-static-runtime",
            "runtime_cdylib": artifact.file_name().unwrap().to_str().unwrap(),
            "runtime_cdylib_sha256": digest.clone(),
            "ori_version": ori_runtime::ORI_RUNTIME_VERSION,
            "abi_version": ori_runtime::ORI_ABI_VERSION,
            "native_static_libs": [],
        });
        std::fs::write(
            directory.join("runtime-link.json"),
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .expect("write identity metadata");
        validate_staged_runtime_artifact(&artifact).expect("matching digest should pass");

        let stale_metadata = serde_json::json!({
            "target": "stale-unknown-target",
            "runtime": "unused-static-runtime",
            "runtime_cdylib": artifact.file_name().unwrap().to_str().unwrap(),
            "runtime_cdylib_sha256": digest,
            "ori_version": ori_runtime::ORI_RUNTIME_VERSION,
            "abi_version": ori_runtime::ORI_ABI_VERSION,
            "native_static_libs": [],
        });
        std::fs::write(
            directory.join("runtime-link.json"),
            serde_json::to_vec_pretty(&stale_metadata).unwrap(),
        )
        .expect("write stale identity metadata");
        let error = validate_staged_runtime_artifact(&artifact)
            .expect_err("stale target metadata must be rejected before loading");
        assert!(error.contains(NATIVE_ABI_MISMATCH), "{error}");
        assert!(error.contains("stale-unknown-target"), "{error}");

        std::fs::write(
            directory.join("runtime-link.json"),
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .expect("restore matching identity metadata");

        std::fs::write(&artifact, b"tampered runtime artifact").expect("tamper artifact");
        let error = validate_staged_runtime_artifact(&artifact)
            .expect_err("tampered runtime must be rejected before loading");
        assert!(error.contains(NATIVE_ABI_MISMATCH), "{error}");
        assert!(error.contains("digest mismatch"), "{error}");
        let _ = std::fs::remove_dir_all(directory);
    }
}
