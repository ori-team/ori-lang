//! Stable C-facing embedding context and opaque handles.

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::{c_void, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::slice;
use std::str;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::ThreadId;

use super::{
    OriConfig, OriDiagnostic, OriEngine, OriFunctionHandle, OriFunctionSignature,
    OriHostRegistryError, OriHostValue, OriHostValueTag, OriScalarType, OriSeverity, OriValue,
    OriValueCallback,
};

const EMBED_ABI_VERSION: u32 = 1;
static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Status returned by every fallible C embedding operation.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriEmbedStatus {
    Ok = 0,
    InvalidArgument = 1,
    WrongThread = 2,
    CompileRejected = 3,
    NotFound = 4,
    StaleHandle = 5,
    CapabilityDenied = 6,
    Busy = 7,
    Cancelled = 8,
    CallbackFailed = 9,
    InternalPanic = 10,
    RuntimeError = 11,
}

/// Thread-affinity rule for a registered callback.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriEmbedCallbackAffinity {
    AnyThread = 0,
    RegistrationThread = 1,
}

/// Signature descriptor consumed synchronously during callback registration.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OriEmbedSignature {
    pub params: *const OriHostValueTag,
    pub param_count: usize,
    pub return_tag: OriHostValueTag,
}

/// Input/output value used by `ori_embed_call`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OriEmbedCallValue {
    pub tag: OriHostValueTag,
    pub int_value: i64,
    pub float_value: f64,
    pub data: *const u8,
    pub len: usize,
    pub managed: *const OriEmbedValueHandle,
}

impl OriEmbedCallValue {
    fn void() -> Self {
        Self {
            tag: OriHostValueTag::Void,
            int_value: 0,
            float_value: 0.0,
            data: std::ptr::null(),
            len: 0,
            managed: std::ptr::null(),
        }
    }
}

/// Borrowed diagnostic view, valid until the next context operation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OriEmbedDiagnosticView {
    pub severity: u32,
    pub code: *const std::ffi::c_char,
    pub message: *const std::ffi::c_char,
    pub file: *const std::ffi::c_char,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// C callback receiving a batch of tagged values.
pub type OriEmbedCallback = unsafe extern "C-unwind" fn(
    user_data: *mut c_void,
    args: *const OriHostValue,
    arg_count: usize,
    out_result: *mut OriHostValue,
) -> i32;

/// Synchronous dispatch task used to marshal a callback to its owner thread.
/// Synchronous task passed to a host event-loop dispatcher. `C-unwind` keeps a
/// Rust panic from crossing a non-unwind ABI if the callback is invoked on the
/// required owner thread; the outer aggregate trampoline still translates it
/// into a callback error.
pub type OriEmbedDispatchTask = unsafe extern "C-unwind" fn(task_data: *mut c_void);

/// Host dispatcher. It must invoke `task(task_data)` exactly once before
/// returning success; asynchronous retention of `task_data` is forbidden.
pub type OriEmbedDispatch = unsafe extern "C-unwind" fn(
    user_data: *mut c_void,
    task: OriEmbedDispatchTask,
    task_data: *mut c_void,
) -> i32;

/// Destructor for one host-owned opaque payload.
pub type OriEmbedOpaqueDrop = unsafe extern "C-unwind" fn(*mut c_void);

#[derive(Debug)]
struct StoredDiagnostic {
    severity: u32,
    code: CString,
    message: CString,
    file: CString,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

struct CCallbackRegistration {
    callback: OriEmbedCallback,
    user_data: *mut c_void,
    affinity: OriEmbedCallbackAffinity,
    owner_thread: ThreadId,
    dispatch: Option<OriEmbedDispatch>,
    dispatch_user_data: *mut c_void,
}

// SAFETY: raw host pointers are never dereferenced by Rust. The callback and
// dispatch contracts require the host to keep them alive until unregister.
unsafe impl Send for CCallbackRegistration {}
unsafe impl Sync for CCallbackRegistration {}

struct ContextState {
    engine: OriEngine,
    diagnostics: Vec<StoredDiagnostic>,
    last_error: CString,
    callbacks: HashMap<String, Box<CCallbackRegistration>>,
    next_opaque_type: u64,
}

/// Opaque C embedding context.
pub struct OriEmbedContext {
    id: u64,
    owner_thread: ThreadId,
    outstanding_handles: Arc<AtomicUsize>,
    state: UnsafeCell<ContextState>,
}

// SAFETY: only the atomic cancellation flag (owned by `OriEngine`) and the
// outstanding-handle counter may be touched off-thread. All state accessors
// enforce `owner_thread`; destroy may not race another API call.
unsafe impl Sync for OriEmbedContext {}

/// Opaque generation-bound function handle.
pub struct OriEmbedFunction {
    context_id: u64,
    handle: OriFunctionHandle,
    outstanding_handles: Arc<AtomicUsize>,
}

impl Drop for OriEmbedFunction {
    fn drop(&mut self) {
        self.outstanding_handles.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Opaque owned managed result.
pub struct OriEmbedValueHandle {
    context_id: u64,
    value: OriValue,
    outstanding_handles: Arc<AtomicUsize>,
}

impl Drop for OriEmbedValueHandle {
    fn drop(&mut self) {
        self.outstanding_handles.fetch_sub(1, Ordering::AcqRel);
    }
}

struct OpaqueTypeCore {
    context_id: u64,
    type_id: u64,
    live_values: AtomicUsize,
}

/// Opaque nominal host type. Identity is context-local and never inferred
/// from an integer payload.
pub struct OriEmbedOpaqueType {
    core: Arc<OpaqueTypeCore>,
    outstanding_handles: Arc<AtomicUsize>,
}

impl Drop for OriEmbedOpaqueType {
    fn drop(&mut self) {
        self.outstanding_handles.fetch_sub(1, Ordering::AcqRel);
    }
}

/// One host-owned opaque payload bound to an exact nominal type.
pub struct OriEmbedOpaqueHandle {
    core: Arc<OpaqueTypeCore>,
    payload: *mut c_void,
    drop_payload: Option<OriEmbedOpaqueDrop>,
    outstanding_handles: Arc<AtomicUsize>,
}

impl Drop for OriEmbedOpaqueHandle {
    fn drop(&mut self) {
        if let Some(drop_payload) = self.drop_payload {
            let _ = catch_unwind(AssertUnwindSafe(|| {
                // SAFETY: the host supplied this destructor for this payload.
                unsafe { drop_payload(self.payload) };
            }));
        }
        self.core.live_values.fetch_sub(1, Ordering::AcqRel);
        self.outstanding_handles.fetch_sub(1, Ordering::AcqRel);
    }
}

struct DispatchedCallback<'a> {
    registration: &'a CCallbackRegistration,
    args: *const OriHostValue,
    arg_count: usize,
    out_result: *mut OriHostValue,
    status: i32,
    executed: bool,
}

unsafe extern "C-unwind" fn run_dispatched_callback(task_data: *mut c_void) {
    if task_data.is_null() {
        return;
    }
    // SAFETY: the dispatch contract guarantees a live synchronous task.
    let task = unsafe { &mut *task_data.cast::<DispatchedCallback<'_>>() };
    if task.executed || std::thread::current().id() != task.registration.owner_thread {
        task.status = OriEmbedStatus::WrongThread as i32;
        return;
    }
    task.executed = true;
    // SAFETY: callback registration owns the exact callback ABI and forwards
    // the original borrowed argument/result views synchronously.
    task.status = unsafe {
        (task.registration.callback)(
            task.registration.user_data,
            task.args,
            task.arg_count,
            task.out_result,
        )
    };
}

unsafe extern "C-unwind" fn c_callback_trampoline(
    user_data: *mut u8,
    args: *const OriHostValue,
    arg_count: usize,
    out_result: *mut OriHostValue,
) -> i32 {
    if user_data.is_null() {
        return OriEmbedStatus::InvalidArgument as i32;
    }
    // SAFETY: the boxed registration remains pinned in its context until the
    // core registry confirms unregister has no active callback.
    let registration = unsafe { &*user_data.cast::<CCallbackRegistration>() };
    if registration.affinity == OriEmbedCallbackAffinity::AnyThread
        || std::thread::current().id() == registration.owner_thread
    {
        // SAFETY: callback and user data are live by the registration contract.
        return unsafe {
            (registration.callback)(registration.user_data, args, arg_count, out_result)
        };
    }
    let Some(dispatch) = registration.dispatch else {
        return OriEmbedStatus::WrongThread as i32;
    };
    let mut task = DispatchedCallback {
        registration,
        args,
        arg_count,
        out_result,
        status: OriEmbedStatus::WrongThread as i32,
        executed: false,
    };
    // SAFETY: the dispatcher must execute the stack task synchronously.
    let dispatch_status = unsafe {
        dispatch(
            registration.dispatch_user_data,
            run_dispatched_callback,
            (&raw mut task).cast(),
        )
    };
    if dispatch_status != 0 {
        dispatch_status
    } else if !task.executed {
        OriEmbedStatus::WrongThread as i32
    } else {
        task.status
    }
}

fn c_string(value: impl AsRef<str>) -> CString {
    let sanitized = value.as_ref().replace('\0', "\\0");
    CString::new(sanitized).expect("NUL bytes were replaced")
}

fn status_for_error(error: &super::OriEmbedError) -> OriEmbedStatus {
    match error {
        super::OriEmbedError::WrongThread => OriEmbedStatus::WrongThread,
        super::OriEmbedError::CapabilityDenied { .. } => OriEmbedStatus::CapabilityDenied,
        super::OriEmbedError::StaleFunctionHandle { .. }
        | super::OriEmbedError::StaleManagedValue { .. } => OriEmbedStatus::StaleHandle,
        super::OriEmbedError::FunctionTrap(error) if error.code == super::ORI_HOST_CANCELLED => {
            OriEmbedStatus::Cancelled
        }
        super::OriEmbedError::FunctionTrap(_) => OriEmbedStatus::RuntimeError,
        super::OriEmbedError::FunctionNotFound { .. }
        | super::OriEmbedError::ModuleNotCompiled(_) => OriEmbedStatus::NotFound,
        _ => OriEmbedStatus::RuntimeError,
    }
}

fn store_error(state: &mut ContextState, message: impl AsRef<str>) {
    state.last_error = c_string(message);
}

fn clear_operation_state(state: &mut ContextState) {
    state.last_error = c_string("");
    state.diagnostics.clear();
}

fn store_diagnostics(state: &mut ContextState, diagnostics: &[OriDiagnostic]) {
    state.diagnostics = diagnostics
        .iter()
        .map(|diagnostic| {
            let primary = diagnostic.labels.first();
            StoredDiagnostic {
                severity: match diagnostic.severity {
                    OriSeverity::Error => 1,
                    OriSeverity::Warning => 2,
                },
                code: c_string(&diagnostic.code),
                message: c_string(&diagnostic.message),
                file: c_string(
                    primary
                        .map(|label| label.path.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ),
                start_line: primary.map_or(0, |label| label.span.start_line),
                start_column: primary.map_or(0, |label| label.span.start_column),
                end_line: primary.map_or(0, |label| label.span.end_line),
                end_column: primary.map_or(0, |label| label.span.end_column),
            }
        })
        .collect();
}

unsafe fn context_ref<'a>(
    context: *mut OriEmbedContext,
) -> Result<&'a OriEmbedContext, OriEmbedStatus> {
    if context.is_null() {
        Err(OriEmbedStatus::InvalidArgument)
    } else {
        // SAFETY: validated non-null; caller owns context lifetime.
        Ok(unsafe { &*context })
    }
}

unsafe fn owner_state<'a>(
    context: *mut OriEmbedContext,
) -> Result<&'a mut ContextState, OriEmbedStatus> {
    let context = unsafe { context_ref(context)? };
    if std::thread::current().id() != context.owner_thread {
        return Err(OriEmbedStatus::WrongThread);
    }
    // SAFETY: the owner-thread contract and no-concurrent-call contract give
    // this operation exclusive access to the state.
    Ok(unsafe { &mut *context.state.get() })
}

unsafe fn utf8_input<'a>(data: *const u8, len: usize) -> Result<&'a str, OriEmbedStatus> {
    if len > isize::MAX as usize || (len != 0 && data.is_null()) {
        return Err(OriEmbedStatus::InvalidArgument);
    }
    let bytes = if len == 0 {
        &[][..]
    } else {
        // SAFETY: caller promises readable input for the synchronous call.
        unsafe { slice::from_raw_parts(data, len) }
    };
    str::from_utf8(bytes).map_err(|_| OriEmbedStatus::InvalidArgument)
}

fn tag_to_scalar(tag: OriHostValueTag) -> Result<Option<OriScalarType>, OriEmbedStatus> {
    match tag {
        OriHostValueTag::Void => Ok(None),
        OriHostValueTag::Bool => Ok(Some(OriScalarType::Bool)),
        OriHostValueTag::Int => Ok(Some(OriScalarType::Int)),
        OriHostValueTag::Float => Ok(Some(OriScalarType::Float)),
        OriHostValueTag::String => Ok(Some(OriScalarType::String)),
        OriHostValueTag::Bytes => Ok(Some(OriScalarType::Bytes)),
        OriHostValueTag::Opaque => Err(OriEmbedStatus::InvalidArgument),
    }
}

unsafe fn signature_from_c(
    signature: *const OriEmbedSignature,
) -> Result<OriFunctionSignature, OriEmbedStatus> {
    if signature.is_null() {
        return Err(OriEmbedStatus::InvalidArgument);
    }
    // SAFETY: caller provides one readable signature for registration.
    let signature = unsafe { &*signature };
    if signature.param_count > 4 || (signature.param_count != 0 && signature.params.is_null()) {
        return Err(OriEmbedStatus::InvalidArgument);
    }
    let params = if signature.param_count == 0 {
        &[][..]
    } else {
        // SAFETY: validated pointer/count pair above.
        unsafe { slice::from_raw_parts(signature.params, signature.param_count) }
    };
    let params = params
        .iter()
        .copied()
        .map(tag_to_scalar)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(OriEmbedStatus::InvalidArgument)?;
    Ok(OriFunctionSignature {
        params,
        return_type: tag_to_scalar(signature.return_tag)?,
    })
}

unsafe fn call_value_from_c(
    context_id: u64,
    input: &OriEmbedCallValue,
) -> Result<OriValue, OriEmbedStatus> {
    match input.tag {
        OriHostValueTag::Bool => Ok(OriValue::Bool(input.int_value != 0)),
        OriHostValueTag::Int => Ok(OriValue::Int(input.int_value)),
        OriHostValueTag::Float => Ok(OriValue::Float(input.float_value)),
        OriHostValueTag::String => {
            if !input.managed.is_null() {
                // SAFETY: validated below before borrowing the owned copy.
                let managed = unsafe { &*input.managed };
                if managed.context_id != context_id {
                    return Err(OriEmbedStatus::StaleHandle);
                }
                let value = managed
                    .value
                    .as_str()
                    .ok_or(OriEmbedStatus::InvalidArgument)?;
                OriValue::string(value).map_err(|_| OriEmbedStatus::InvalidArgument)
            } else {
                let value = unsafe { utf8_input(input.data, input.len)? };
                OriValue::string(value).map_err(|_| OriEmbedStatus::InvalidArgument)
            }
        }
        OriHostValueTag::Bytes => {
            let bytes = if !input.managed.is_null() {
                // SAFETY: validated below before borrowing the owned copy.
                let managed = unsafe { &*input.managed };
                if managed.context_id != context_id {
                    return Err(OriEmbedStatus::StaleHandle);
                }
                managed
                    .value
                    .as_bytes_with_len()
                    .ok_or(OriEmbedStatus::InvalidArgument)?
            } else if input.len == 0 {
                &[][..]
            } else {
                if input.data.is_null() || input.len > isize::MAX as usize {
                    return Err(OriEmbedStatus::InvalidArgument);
                }
                // SAFETY: caller supplies the readable pointer/length pair.
                unsafe { slice::from_raw_parts(input.data, input.len) }
            };
            Ok(OriValue::bytes(bytes))
        }
        _ => Err(OriEmbedStatus::InvalidArgument),
    }
}

fn call_value_to_c(context: &OriEmbedContext, value: Option<OriValue>) -> OriEmbedCallValue {
    let Some(value) = value else {
        return OriEmbedCallValue::void();
    };
    match value {
        OriValue::Bool(value) => OriEmbedCallValue {
            tag: OriHostValueTag::Bool,
            int_value: i64::from(value),
            ..OriEmbedCallValue::void()
        },
        OriValue::Int(value) => OriEmbedCallValue {
            tag: OriHostValueTag::Int,
            int_value: value,
            ..OriEmbedCallValue::void()
        },
        OriValue::Float(value) => OriEmbedCallValue {
            tag: OriHostValueTag::Float,
            float_value: value,
            ..OriEmbedCallValue::void()
        },
        value => {
            let tag = match &value {
                OriValue::String(_) => OriHostValueTag::String,
                OriValue::Bytes(_) => OriHostValueTag::Bytes,
                OriValue::Slice(_) => OriHostValueTag::Opaque,
                _ => unreachable!("scalar values returned above"),
            };
            context.outstanding_handles.fetch_add(1, Ordering::AcqRel);
            let managed = Box::new(OriEmbedValueHandle {
                context_id: context.id,
                value,
                outstanding_handles: Arc::clone(&context.outstanding_handles),
            });
            OriEmbedCallValue {
                tag,
                managed: Box::into_raw(managed),
                ..OriEmbedCallValue::void()
            }
        }
    }
}

/// Return the C Host ABI schema version.
#[no_mangle]
pub extern "C" fn ori_embed_abi_version() -> u32 {
    EMBED_ABI_VERSION
}

/// Create a context owned by the current thread.
#[no_mangle]
pub extern "C" fn ori_embed_context_create() -> *mut OriEmbedContext {
    match catch_unwind(AssertUnwindSafe(|| {
        let engine = OriEngine::new(OriConfig::default());
        Box::into_raw(Box::new(OriEmbedContext {
            id: NEXT_CONTEXT_ID.fetch_add(1, Ordering::Relaxed),
            owner_thread: std::thread::current().id(),
            outstanding_handles: Arc::new(AtomicUsize::new(0)),
            state: UnsafeCell::new(ContextState {
                engine,
                diagnostics: Vec::new(),
                last_error: c_string(""),
                callbacks: HashMap::new(),
                next_opaque_type: 1,
            }),
        }))
    })) {
        Ok(context) => context,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Request cancellation. This is the only operation allowed off the context
/// owner thread.
///
/// # Safety
///
/// `context` must remain live for this call and may not race context destroy.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_context_cancel(context: *mut OriEmbedContext) -> OriEmbedStatus {
    let Ok(context) = (unsafe { context_ref(context) }) else {
        return OriEmbedStatus::InvalidArgument;
    };
    context.engine().cancel();
    OriEmbedStatus::Ok
}

impl OriEmbedContext {
    fn engine(&self) -> &OriEngine {
        // SAFETY: cancellation only touches the engine's atomic flag. Other
        // callers use this accessor only on the owner thread.
        unsafe { &(*self.state.get()).engine }
    }
}

/// Clear a cancellation request from the owner thread.
///
/// # Safety
///
/// `context` must be a live context pointer owned by the current thread.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_context_reset_cancel(
    context: *mut OriEmbedContext,
) -> OriEmbedStatus {
    let state = match unsafe { owner_state(context) } {
        Ok(state) => state,
        Err(status) => return status,
    };
    match state.engine.reset_cancellation() {
        Ok(()) => OriEmbedStatus::Ok,
        Err(error) => {
            let status = status_for_error(&error);
            store_error(state, error.to_string());
            status
        }
    }
}

/// Grant one import capability for subsequent compilations.
///
/// # Safety
///
/// `context` and `name[0..len]` must remain readable for this call.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_context_grant_capability(
    context: *mut OriEmbedContext,
    name: *const u8,
    len: usize,
) -> OriEmbedStatus {
    let state = match unsafe { owner_state(context) } {
        Ok(state) => state,
        Err(status) => return status,
    };
    let name = match unsafe { utf8_input(name, len) } {
        Ok(name) => name,
        Err(status) => return status,
    };
    match state.engine.grant_capability(name) {
        Ok(_) => OriEmbedStatus::Ok,
        Err(error) => {
            store_error(state, error.to_string());
            OriEmbedStatus::InvalidArgument
        }
    }
}

/// Compile and atomically publish one module generation.
///
/// # Safety
///
/// All pointer/length pairs must be readable for the call. `context` must be
/// live and owned by the current thread.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_compile(
    context: *mut OriEmbedContext,
    module_name: *const u8,
    module_name_len: usize,
    source: *const u8,
    source_len: usize,
) -> OriEmbedStatus {
    let state = match unsafe { owner_state(context) } {
        Ok(state) => state,
        Err(status) => return status,
    };
    clear_operation_state(state);
    let module_name = match unsafe { utf8_input(module_name, module_name_len) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let source = match unsafe { utf8_input(source, source_len) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    match catch_unwind(AssertUnwindSafe(|| {
        state.engine.compile_source(module_name, source)
    })) {
        Ok(Ok(result)) => {
            store_diagnostics(state, &result.diagnostics);
            if result.accepted {
                OriEmbedStatus::Ok
            } else {
                OriEmbedStatus::CompileRejected
            }
        }
        Ok(Err(error)) => {
            let status = status_for_error(&error);
            store_error(state, error.to_string());
            status
        }
        Err(_) => {
            store_error(state, "panic contained at ori_embed_compile");
            OriEmbedStatus::InternalPanic
        }
    }
}

/// Resolve a generation-bound public function handle.
///
/// # Safety
///
/// Inputs and `out_function` must be valid for this call.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_function_resolve(
    context: *mut OriEmbedContext,
    module_name: *const u8,
    module_name_len: usize,
    function_name: *const u8,
    function_name_len: usize,
    out_function: *mut *mut OriEmbedFunction,
) -> OriEmbedStatus {
    if out_function.is_null() {
        return OriEmbedStatus::InvalidArgument;
    }
    // SAFETY: validated writable output.
    unsafe { out_function.write(std::ptr::null_mut()) };
    let context_ref = match unsafe { context_ref(context) } {
        Ok(context) if std::thread::current().id() == context.owner_thread => context,
        Ok(_) => return OriEmbedStatus::WrongThread,
        Err(status) => return status,
    };
    // SAFETY: owner-thread access is exclusive by contract.
    let state = unsafe { &mut *context_ref.state.get() };
    let module_name = match unsafe { utf8_input(module_name, module_name_len) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let function_name = match unsafe { utf8_input(function_name, function_name_len) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    match state.engine.function(module_name, function_name) {
        Ok(handle) => {
            context_ref
                .outstanding_handles
                .fetch_add(1, Ordering::AcqRel);
            let handle = Box::new(OriEmbedFunction {
                context_id: context_ref.id,
                handle,
                outstanding_handles: Arc::clone(&context_ref.outstanding_handles),
            });
            // SAFETY: caller owns the returned opaque handle.
            unsafe { out_function.write(Box::into_raw(handle)) };
            OriEmbedStatus::Ok
        }
        Err(error) => {
            let status = status_for_error(&error);
            store_error(state, error.to_string());
            status
        }
    }
}

/// Release one function handle.
///
/// # Safety
///
/// `function` must be a live handle returned by this library and released once.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_function_release(function: *mut OriEmbedFunction) {
    if let Some(function) = NonNull::new(function) {
        // SAFETY: ownership transfers back exactly once.
        drop(unsafe { Box::from_raw(function.as_ptr()) });
    }
}

/// Invoke a function and return a scalar or owned managed handle.
///
/// # Safety
///
/// Context/function/output pointers and the argument array must remain valid
/// for this synchronous call.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_call(
    context: *mut OriEmbedContext,
    function: *const OriEmbedFunction,
    args: *const OriEmbedCallValue,
    arg_count: usize,
    out_result: *mut OriEmbedCallValue,
) -> OriEmbedStatus {
    if function.is_null()
        || out_result.is_null()
        || arg_count > 4
        || (arg_count != 0 && args.is_null())
    {
        return OriEmbedStatus::InvalidArgument;
    }
    // SAFETY: validated writable result.
    unsafe { out_result.write(OriEmbedCallValue::void()) };
    let context_ref = match unsafe { context_ref(context) } {
        Ok(context) if std::thread::current().id() == context.owner_thread => context,
        Ok(_) => return OriEmbedStatus::WrongThread,
        Err(status) => return status,
    };
    // SAFETY: validated handle pointer.
    let function = unsafe { &*function };
    if function.context_id != context_ref.id {
        return OriEmbedStatus::StaleHandle;
    }
    let inputs = if arg_count == 0 {
        &[][..]
    } else {
        // SAFETY: validated pointer/count above.
        unsafe { slice::from_raw_parts(args, arg_count) }
    };
    let mut values = Vec::with_capacity(arg_count);
    for input in inputs {
        match unsafe { call_value_from_c(context_ref.id, input) } {
            Ok(value) => values.push(value),
            Err(status) => return status,
        }
    }
    // SAFETY: owner-thread access is exclusive by contract.
    let state = unsafe { &mut *context_ref.state.get() };
    match catch_unwind(AssertUnwindSafe(|| {
        state.engine.call(&function.handle, &values)
    })) {
        Ok(Ok(value)) => {
            // SAFETY: validated writable output.
            unsafe { out_result.write(call_value_to_c(context_ref, value)) };
            OriEmbedStatus::Ok
        }
        Ok(Err(error)) => {
            let status = status_for_error(&error);
            store_error(state, error.to_string());
            status
        }
        Err(_) => {
            store_error(state, "panic contained at ori_embed_call");
            OriEmbedStatus::InternalPanic
        }
    }
}

/// Borrow exact managed string/bytes contents from an owned value handle.
///
/// # Safety
///
/// `value`, `out_data`, and `out_len` must be valid; the returned data remains
/// valid only until `value` is released.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_value_bytes(
    value: *const OriEmbedValueHandle,
    out_data: *mut *const u8,
    out_len: *mut usize,
) -> OriEmbedStatus {
    if value.is_null() || out_data.is_null() || out_len.is_null() {
        return OriEmbedStatus::InvalidArgument;
    }
    // SAFETY: validated pointers.
    let value = unsafe { &*value };
    let Some(bytes) = value.value.as_bytes() else {
        return OriEmbedStatus::InvalidArgument;
    };
    // SAFETY: outputs are writable and borrow from the live value.
    unsafe {
        out_data.write(if bytes.is_empty() {
            std::ptr::null()
        } else {
            bytes.as_ptr()
        });
        out_len.write(bytes.len());
    }
    OriEmbedStatus::Ok
}

/// Release one owned managed value.
///
/// # Safety
///
/// `value` must be returned by this library and released exactly once.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_value_release(value: *mut OriEmbedValueHandle) {
    if let Some(value) = NonNull::new(value) {
        // SAFETY: ownership transfers back exactly once.
        drop(unsafe { Box::from_raw(value.as_ptr()) });
    }
}

/// Register an aggregate C callback with an optional capability requirement.
///
/// # Safety
///
/// All inputs must remain readable for this call. Callback/user/dispatch data
/// must remain live until unregister or context destroy succeeds.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_callback_register(
    context: *mut OriEmbedContext,
    name: *const u8,
    name_len: usize,
    signature: *const OriEmbedSignature,
    capability: *const u8,
    capability_len: usize,
    callback: Option<OriEmbedCallback>,
    callback_user_data: *mut c_void,
    affinity: OriEmbedCallbackAffinity,
    dispatch: Option<OriEmbedDispatch>,
    dispatch_user_data: *mut c_void,
) -> OriEmbedStatus {
    let Some(callback) = callback else {
        return OriEmbedStatus::InvalidArgument;
    };
    let state = match unsafe { owner_state(context) } {
        Ok(state) => state,
        Err(status) => return status,
    };
    let name = match unsafe { utf8_input(name, name_len) } {
        Ok(name) => name.to_owned(),
        Err(status) => return status,
    };
    let signature = match unsafe { signature_from_c(signature) } {
        Ok(signature) => signature,
        Err(status) => return status,
    };
    let capability = if capability_len == 0 {
        None
    } else {
        match unsafe { utf8_input(capability, capability_len) } {
            Ok(capability) => Some(capability.to_owned()),
            Err(status) => return status,
        }
    };
    let mut registration = Box::new(CCallbackRegistration {
        callback,
        user_data: callback_user_data,
        affinity,
        owner_thread: std::thread::current().id(),
        dispatch,
        dispatch_user_data,
    });
    let registration_pointer = (&raw mut *registration).cast::<u8>();
    // SAFETY: the registration is pinned by Box and kept until unregister.
    let result = unsafe {
        state.engine.host_registry_mut().register_value_callback(
            name.clone(),
            signature,
            capability,
            registration_pointer,
            c_callback_trampoline as OriValueCallback,
        )
    };
    match result {
        Ok(_) => {
            state.callbacks.insert(name, registration);
            OriEmbedStatus::Ok
        }
        Err(error) => {
            store_error(state, error.to_string());
            OriEmbedStatus::InvalidArgument
        }
    }
}

/// Unregister a callback after all active calls complete.
///
/// # Safety
///
/// `context` and callback name must be valid for this call.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_callback_unregister(
    context: *mut OriEmbedContext,
    name: *const u8,
    name_len: usize,
) -> OriEmbedStatus {
    let state = match unsafe { owner_state(context) } {
        Ok(state) => state,
        Err(status) => return status,
    };
    let name = match unsafe { utf8_input(name, name_len) } {
        Ok(name) => name,
        Err(status) => return status,
    };
    match state.engine.host_registry_mut().remove_callback(name) {
        Ok(()) => {
            state.callbacks.remove(name);
            OriEmbedStatus::Ok
        }
        Err(OriHostRegistryError::CallbackActive(_)) => OriEmbedStatus::Busy,
        Err(error) => {
            store_error(state, error.to_string());
            OriEmbedStatus::NotFound
        }
    }
}

/// Return the number of structured diagnostics from the last compile.
///
/// # Safety
///
/// `context` must be live and accessed on its owner thread.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_diagnostic_count(context: *mut OriEmbedContext) -> usize {
    unsafe { owner_state(context) }
        .map(|state| state.diagnostics.len())
        .unwrap_or(0)
}

/// Borrow one structured diagnostic.
///
/// # Safety
///
/// `context` and `out_diagnostic` must be valid on the owner thread.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_diagnostic(
    context: *mut OriEmbedContext,
    index: usize,
    out_diagnostic: *mut OriEmbedDiagnosticView,
) -> OriEmbedStatus {
    if out_diagnostic.is_null() {
        return OriEmbedStatus::InvalidArgument;
    }
    let state = match unsafe { owner_state(context) } {
        Ok(state) => state,
        Err(status) => return status,
    };
    let Some(diagnostic) = state.diagnostics.get(index) else {
        return OriEmbedStatus::NotFound;
    };
    // SAFETY: validated writable output, all strings live in context state.
    unsafe {
        out_diagnostic.write(OriEmbedDiagnosticView {
            severity: diagnostic.severity,
            code: diagnostic.code.as_ptr(),
            message: diagnostic.message.as_ptr(),
            file: diagnostic.file.as_ptr(),
            start_line: diagnostic.start_line,
            start_column: diagnostic.start_column,
            end_line: diagnostic.end_line,
            end_column: diagnostic.end_column,
        });
    }
    OriEmbedStatus::Ok
}

/// Borrow the last context error string.
///
/// # Safety
///
/// `context` must be live and accessed on its owner thread.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_last_error(
    context: *mut OriEmbedContext,
) -> *const std::ffi::c_char {
    unsafe { owner_state(context) }
        .map(|state| state.last_error.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Register one nominal opaque type in this context.
///
/// # Safety
///
/// `context`, name, and `out_type` must be valid for this call.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_opaque_type_register(
    context: *mut OriEmbedContext,
    name: *const u8,
    name_len: usize,
    out_type: *mut *mut OriEmbedOpaqueType,
) -> OriEmbedStatus {
    if out_type.is_null() {
        return OriEmbedStatus::InvalidArgument;
    }
    let context_ref = match unsafe { context_ref(context) } {
        Ok(context) if std::thread::current().id() == context.owner_thread => context,
        Ok(_) => return OriEmbedStatus::WrongThread,
        Err(status) => return status,
    };
    let _name = match unsafe { utf8_input(name, name_len) } {
        Ok(name) if !name.is_empty() => name,
        _ => return OriEmbedStatus::InvalidArgument,
    };
    // SAFETY: owner-thread access is exclusive by contract.
    let state = unsafe { &mut *context_ref.state.get() };
    let type_id = state.next_opaque_type;
    state.next_opaque_type += 1;
    context_ref
        .outstanding_handles
        .fetch_add(1, Ordering::AcqRel);
    let opaque_type = Box::new(OriEmbedOpaqueType {
        core: Arc::new(OpaqueTypeCore {
            context_id: context_ref.id,
            type_id,
            live_values: AtomicUsize::new(0),
        }),
        outstanding_handles: Arc::clone(&context_ref.outstanding_handles),
    });
    // SAFETY: caller owns the returned handle.
    unsafe { out_type.write(Box::into_raw(opaque_type)) };
    OriEmbedStatus::Ok
}

/// Release an opaque type after all values of that type are gone.
///
/// # Safety
///
/// `opaque_type` must be a live type handle returned by this library.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_opaque_type_release(
    opaque_type: *mut OriEmbedOpaqueType,
) -> OriEmbedStatus {
    let Some(opaque_type) = NonNull::new(opaque_type) else {
        return OriEmbedStatus::InvalidArgument;
    };
    // SAFETY: validated live handle by caller contract.
    if unsafe { opaque_type.as_ref() }
        .core
        .live_values
        .load(Ordering::Acquire)
        != 0
    {
        return OriEmbedStatus::Busy;
    }
    // SAFETY: ownership transfers back exactly once.
    drop(unsafe { Box::from_raw(opaque_type.as_ptr()) });
    OriEmbedStatus::Ok
}

/// Create one host-owned opaque value.
///
/// # Safety
///
/// Context/type/output must be valid. Payload and destructor remain owned by
/// the new handle until release.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_opaque_create(
    context: *mut OriEmbedContext,
    opaque_type: *const OriEmbedOpaqueType,
    payload: *mut c_void,
    drop_payload: Option<OriEmbedOpaqueDrop>,
    out_handle: *mut *mut OriEmbedOpaqueHandle,
) -> OriEmbedStatus {
    if opaque_type.is_null() || out_handle.is_null() {
        return OriEmbedStatus::InvalidArgument;
    }
    let context_ref = match unsafe { context_ref(context) } {
        Ok(context) if std::thread::current().id() == context.owner_thread => context,
        Ok(_) => return OriEmbedStatus::WrongThread,
        Err(status) => return status,
    };
    // SAFETY: validated live type pointer by caller contract.
    let opaque_type = unsafe { &*opaque_type };
    if opaque_type.core.context_id != context_ref.id {
        return OriEmbedStatus::StaleHandle;
    }
    opaque_type.core.live_values.fetch_add(1, Ordering::AcqRel);
    context_ref
        .outstanding_handles
        .fetch_add(1, Ordering::AcqRel);
    let handle = Box::new(OriEmbedOpaqueHandle {
        core: Arc::clone(&opaque_type.core),
        payload,
        drop_payload,
        outstanding_handles: Arc::clone(&context_ref.outstanding_handles),
    });
    // SAFETY: caller owns the returned opaque value.
    unsafe { out_handle.write(Box::into_raw(handle)) };
    OriEmbedStatus::Ok
}

/// Access an opaque payload only when its exact nominal type matches.
///
/// # Safety
///
/// Context, handle, expected type, and output pointers must all be live.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_opaque_payload(
    context: *mut OriEmbedContext,
    handle: *const OriEmbedOpaqueHandle,
    expected_type: *const OriEmbedOpaqueType,
    out_payload: *mut *mut c_void,
) -> OriEmbedStatus {
    if handle.is_null() || expected_type.is_null() || out_payload.is_null() {
        return OriEmbedStatus::InvalidArgument;
    }
    let context_ref = match unsafe { context_ref(context) } {
        Ok(context) if std::thread::current().id() == context.owner_thread => context,
        Ok(_) => return OriEmbedStatus::WrongThread,
        Err(status) => return status,
    };
    // SAFETY: validated live pointers by caller contract.
    let handle = unsafe { &*handle };
    let expected = unsafe { &*expected_type };
    if handle.core.context_id != context_ref.id
        || expected.core.context_id != context_ref.id
        || handle.core.type_id != expected.core.type_id
    {
        return OriEmbedStatus::StaleHandle;
    }
    // SAFETY: validated writable output.
    unsafe { out_payload.write(handle.payload) };
    OriEmbedStatus::Ok
}

/// Release one host-owned opaque value.
///
/// # Safety
///
/// `handle` must be returned by this library and released exactly once.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_opaque_release(handle: *mut OriEmbedOpaqueHandle) {
    if let Some(handle) = NonNull::new(handle) {
        // SAFETY: ownership transfers back exactly once.
        drop(unsafe { Box::from_raw(handle.as_ptr()) });
    }
}

/// Return the number of unreleased function/value/opaque handles.
///
/// # Safety
///
/// `context` must remain live for this call.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_context_outstanding_handles(
    context: *mut OriEmbedContext,
) -> usize {
    unsafe { context_ref(context) }
        .map(|context| context.outstanding_handles.load(Ordering::Acquire))
        .unwrap_or(0)
}

/// Destroy a context only after every public handle has been released.
///
/// # Safety
///
/// `context` must be a live context pointer owned by the current thread. It is
/// consumed only when this function returns `Ok`.
#[no_mangle]
pub unsafe extern "C" fn ori_embed_context_destroy(
    context: *mut OriEmbedContext,
) -> OriEmbedStatus {
    let context_ref = match unsafe { context_ref(context) } {
        Ok(context) if std::thread::current().id() == context.owner_thread => context,
        Ok(_) => return OriEmbedStatus::WrongThread,
        Err(status) => return status,
    };
    if context_ref.outstanding_handles.load(Ordering::Acquire) != 0 {
        return OriEmbedStatus::Busy;
    }
    // SAFETY: owner-thread access is exclusive by contract.
    let state = unsafe { &mut *context_ref.state.get() };
    let callback_names = state.callbacks.keys().cloned().collect::<Vec<_>>();
    for name in callback_names {
        match state.engine.host_registry_mut().remove_callback(&name) {
            Ok(()) => {
                state.callbacks.remove(&name);
            }
            Err(OriHostRegistryError::CallbackActive(_)) => return OriEmbedStatus::Busy,
            Err(_) => {
                state.callbacks.remove(&name);
            }
        }
    }
    state.engine.unload_all();
    // SAFETY: all lifetimes are drained and ownership transfers back once.
    drop(unsafe { Box::from_raw(context) });
    OriEmbedStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_handles_require_exact_nominal_type_and_block_early_shutdown() {
        let context = ori_embed_context_create();
        assert!(!context.is_null());
        let mut first = std::ptr::null_mut();
        let mut second = std::ptr::null_mut();
        // SAFETY: all pointers are produced and consumed within this test.
        unsafe {
            assert_eq!(
                ori_embed_opaque_type_register(context, b"Window".as_ptr(), 6, &raw mut first),
                OriEmbedStatus::Ok
            );
            assert_eq!(
                ori_embed_opaque_type_register(context, b"Texture".as_ptr(), 7, &raw mut second),
                OriEmbedStatus::Ok
            );
            let mut handle = std::ptr::null_mut();
            let payload = 7_usize as *mut c_void;
            assert_eq!(
                ori_embed_opaque_create(context, first, payload, None, &raw mut handle,),
                OriEmbedStatus::Ok
            );
            let mut observed = std::ptr::null_mut();
            assert_eq!(
                ori_embed_opaque_payload(context, handle, second, &raw mut observed),
                OriEmbedStatus::StaleHandle
            );
            assert_eq!(ori_embed_context_destroy(context), OriEmbedStatus::Busy);
            ori_embed_opaque_release(handle);
            assert_eq!(ori_embed_opaque_type_release(first), OriEmbedStatus::Ok);
            assert_eq!(ori_embed_opaque_type_release(second), OriEmbedStatus::Ok);
            assert_eq!(ori_embed_context_destroy(context), OriEmbedStatus::Ok);
        }
    }

    #[test]
    fn owner_thread_affinity_rejects_foreign_context_operations() {
        let context = ori_embed_context_create();
        let address = context as usize;
        let status = std::thread::spawn(move || {
            // SAFETY: the context remains live; wrong-thread validation runs
            // before mutable state is accessed.
            unsafe {
                ori_embed_context_grant_capability(
                    address as *mut OriEmbedContext,
                    b"clock".as_ptr(),
                    5,
                )
            }
        })
        .join()
        .expect("foreign thread joins");
        assert_eq!(status, OriEmbedStatus::WrongThread);
        // SAFETY: context is live and back on its owner thread.
        assert_eq!(
            unsafe { ori_embed_context_destroy(context) },
            OriEmbedStatus::Ok
        );
    }
}
