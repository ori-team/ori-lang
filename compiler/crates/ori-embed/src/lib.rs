//! Hosted Ori session primitives.
//!
//! This crate is the first Rust boundary for native hosts such as Brasa.
//! It deliberately exposes checked source modules and owned diagnostics, not
//! compiler internals, Cranelift pointers, or a false promise of hot reload.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use ori_diagnostics::{Diagnostic, FileId, Severity, SourceCache, Span};
use ori_driver::pipeline::{
    lower_jit_source_with_options, run_check_source_with_options, CheckOptions,
};
use ori_hir::{HirExtern, HirModule};
use ori_types::conditional::{is_supported_target_triple, CfgContext};
use ori_types::Ty;
use thiserror::Error;

pub use ori_types::conditional::ExecutionProfile;

const FIRST_MODULE_GENERATION: u64 = 1;
/// Error code returned when a compiled module calls an unregistered callback.
pub const ORI_HOST_CALLBACK_CANCELLED: i32 = 1001;
/// Error code returned when callback recursion reaches the hosted limit.
pub const ORI_HOST_CALLBACK_REENTRANCY_LIMIT: i32 = 1002;
const MAX_HOST_CALLBACK_DEPTH: usize = 64;
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_CALLBACK_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static CALLBACK_ERROR: RefCell<Option<OriExecutionError>> = const { RefCell::new(None) };
    static CALLBACK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

static CALLBACK_STATES: OnceLock<Mutex<HashMap<u64, Arc<CallbackState>>>> = OnceLock::new();

fn callback_states() -> &'static Mutex<HashMap<u64, Arc<CallbackState>>> {
    CALLBACK_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn set_callback_error(code: i32, message: impl Into<String>) {
    CALLBACK_ERROR.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(OriExecutionError {
                code,
                message: message.into(),
            });
        }
    });
}

fn clear_callback_error() {
    CALLBACK_ERROR.with(|slot| slot.borrow_mut().take());
}

fn take_callback_error() -> Option<OriExecutionError> {
    CALLBACK_ERROR.with(|slot| slot.borrow_mut().take())
}

/// Configuration owned by a hosted Ori session.
#[derive(Clone, Debug)]
pub struct OriConfig {
    cfg: CfgContext,
}

impl Default for OriConfig {
    fn default() -> Self {
        Self {
            cfg: CfgContext::host_default(),
        }
    }
}

impl OriConfig {
    /// Select a target represented by the current structured cfg contract.
    pub fn with_target(mut self, target: impl Into<String>) -> Result<Self, OriConfigError> {
        self.set_target(target)?;
        Ok(self)
    }

    /// Select the execution profile used by `@cfg`.
    pub fn with_execution_profile(mut self, profile: ExecutionProfile) -> Self {
        self.cfg.execution_profile = profile;
        self
    }

    /// Declare a feature available to this source session.
    pub fn declare_feature(&mut self, feature: impl Into<String>) -> Result<(), OriConfigError> {
        let feature = feature.into();
        validate_feature_name(&feature)?;
        self.cfg.declared_features.insert(feature);
        Ok(())
    }

    /// Enable a previously declared feature.
    pub fn enable_feature(&mut self, feature: impl Into<String>) -> Result<(), OriConfigError> {
        let feature = feature.into();
        validate_feature_name(&feature)?;
        if !self.cfg.declared_features.contains(&feature) {
            return Err(OriConfigError::UndeclaredFeature(feature));
        }
        self.cfg.enabled_features.insert(feature);
        Ok(())
    }

    /// Return the selected target triple.
    pub fn target(&self) -> &str {
        &self.cfg.target_triple
    }

    /// Return the selected execution profile.
    pub fn execution_profile(&self) -> ExecutionProfile {
        self.cfg.execution_profile
    }

    /// Return the declared features in deterministic order.
    pub fn declared_features(&self) -> impl Iterator<Item = &str> {
        self.cfg.declared_features.iter().map(String::as_str)
    }

    /// Return the enabled features in deterministic order.
    pub fn enabled_features(&self) -> impl Iterator<Item = &str> {
        self.cfg.enabled_features.iter().map(String::as_str)
    }

    fn set_target(&mut self, target: impl Into<String>) -> Result<(), OriConfigError> {
        let target = target.into();
        if target.is_empty() || target.chars().any(char::is_whitespace) {
            return Err(OriConfigError::InvalidTarget(target));
        }
        if !is_supported_target_triple(&target) {
            return Err(OriConfigError::InvalidTarget(target));
        }
        self.cfg = CfgContext::new(
            target,
            self.cfg.execution_profile,
            self.cfg.declared_features.clone(),
            self.cfg.enabled_features.clone(),
        );
        Ok(())
    }

    fn cfg_context(&self) -> CfgContext {
        self.cfg.clone()
    }
}

/// Errors found while building hosted session configuration.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OriConfigError {
    #[error("unsupported target triple `{0}`")]
    InvalidTarget(String),
    #[error("invalid feature name `{0}`")]
    InvalidFeature(String),
    #[error("feature `{0}` is not declared")]
    UndeclaredFeature(String),
}

/// Stable identity of a logical module inside an `OriEngine`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(u64);

impl ModuleId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Stable identity of one hosted compiler session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(u64);

impl SessionId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Generation of a successfully accepted module snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleGeneration(u64);

impl ModuleGeneration {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// A stable module identity and its last accepted generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleSnapshot {
    pub id: ModuleId,
    pub generation: Option<ModuleGeneration>,
}

/// Result of checking a source update in a hosted session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckResult {
    pub module: ModuleSnapshot,
    pub accepted: bool,
    pub diagnostics: Vec<OriDiagnostic>,
}

/// Scalar types supported by the experimental hosted invocation boundary.
///
/// Scalar and pointer types supported by the experimental hosted invocation boundary.
///
/// [`Self::Slice`], [`Self::String`], and [`Self::Bytes`] are read-only or
/// opaque handles produced or consumed by Ori functions. The host reads them
/// through checked accessors on [`OriValue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriScalarType {
    Bool,
    Int,
    Float,
    Slice,
    String,
    Bytes,
    Unsupported,
}

/// Signature metadata for a public function exposed by a compiled module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriFunctionSignature {
    pub params: Vec<OriScalarType>,
    pub return_type: Option<OriScalarType>,
}

/// Public function metadata returned by a successful hosted compilation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriFunctionInfo {
    pub name: String,
    pub signature: OriFunctionSignature,
    pub is_public: bool,
}

/// Values accepted by the experimental hosted invocation boundary.
///
/// [`Self::Slice`], [`Self::String`], and [`Self::Bytes`] carry raw pointers;
/// they are only meaningful while the module that produced them is alive and
/// can be inspected safely through accessors such as [`Self::as_str`] and
/// [`Self::as_bytes`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OriValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Slice(*const u8),
    String(*const u8),
    Bytes(*const u8),
}

impl OriValue {
    /// Return the boolean value if this is [`Self::Bool`].
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Return the integer value if this is [`Self::Int`].
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// Return the floating-point value if this is [`Self::Float`].
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Return the raw slice pointer if this is [`Self::Slice`].
    pub fn as_slice_ptr(&self) -> Option<*const u8> {
        match self {
            Self::Slice(pointer) => Some(*pointer),
            _ => None,
        }
    }

    /// Return the raw string pointer if this is [`Self::String`].
    pub fn as_string_ptr(&self) -> Option<*const u8> {
        match self {
            Self::String(pointer) => Some(*pointer),
            _ => None,
        }
    }

    /// Return the raw bytes pointer if this is [`Self::Bytes`].
    pub fn as_bytes_ptr(&self) -> Option<*const u8> {
        match self {
            Self::Bytes(pointer) => Some(*pointer),
            _ => None,
        }
    }

    /// Return the string slice if this value is a valid UTF-8 string pointer.
    ///
    /// # Safety / Lifetimes
    ///
    /// The returned reference is borrowed from the C string pointed to by this
    /// value and is valid as long as the underlying JIT module / allocation is alive.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(pointer) if !pointer.is_null() => {
                unsafe { std::ffi::CStr::from_ptr(*pointer as *const std::os::raw::c_char) }
                    .to_str()
                    .ok()
            }
            _ => None,
        }
    }

    /// Return the byte slice if this value is a string or bytes pointer.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(pointer) | Self::String(pointer) if !pointer.is_null() => {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(*pointer as *const std::os::raw::c_char) }
                        .to_bytes(),
                )
            }
            _ => None,
        }
    }
}

/// A recoverable failure reported by an embedded Ori invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriExecutionError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for OriExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.message.is_empty() {
            write!(formatter, "code {}", self.code)
        } else {
            write!(formatter, "code {}: {}", self.code, self.message)
        }
    }
}

/// A validated scalar native function made available to `extern host`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriHostFunction {
    pub name: String,
    pub signature: OriFunctionSignature,
}

/// Stable identity of a callback registration inside a hosted process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OriCallbackId(u64);

impl OriCallbackId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Metadata for one host callback imported by Ori.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriHostCallback {
    pub id: OriCallbackId,
    pub name: String,
    pub signature: OriFunctionSignature,
}

/// Fixed native ABI for homogeneous integer callbacks.
pub type OriIntCallback = unsafe extern "C" fn(*mut u8, i64, i64, i64, i64) -> i64;
/// Fixed native ABI for homogeneous floating-point callbacks.
pub type OriFloatCallback = unsafe extern "C" fn(*mut u8, f64, f64, f64, f64) -> f64;
/// Fixed native ABI for homogeneous boolean callbacks.
pub type OriBoolCallback = unsafe extern "C" fn(*mut u8, i8, i8, i8, i8) -> i8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallbackValueKind {
    Int,
    Float,
    Bool,
}

#[derive(Debug)]
struct CallbackState {
    callback: usize,
    user_data: usize,
    kind: CallbackValueKind,
    active_calls: AtomicUsize,
    orphaned: AtomicBool,
}

// SAFETY: callback addresses and user_data are opaque values supplied by the
// trusted native host. The registry never dereferences user_data itself and
// protects callback lifecycle with the global mutex and active-call counter.
unsafe impl Send for CallbackState {}
unsafe impl Sync for CallbackState {}

#[derive(Clone, Debug)]
struct RegisteredHostCallback {
    info: OriHostCallback,
    kind: CallbackValueKind,
}

#[derive(Debug, Default)]
pub struct OriHostRegistry {
    functions: HashMap<String, RegisteredHostFunction>,
    callbacks: HashMap<String, RegisteredHostCallback>,
}

#[derive(Clone, Debug)]
struct RegisteredHostFunction {
    info: OriHostFunction,
    address: usize,
}

impl OriHostRegistry {
    /// Register a host-owned C-ABI function address.
    ///
    /// # Safety
    ///
    /// `address` must point to a live `extern "C"` function whose ABI exactly
    /// matches `signature`. The address must remain valid until every compiled
    /// module using this registry has been dropped.
    pub unsafe fn register_function(
        &mut self,
        name: impl Into<String>,
        signature: OriFunctionSignature,
        address: *const u8,
    ) -> Result<(), OriHostRegistryError> {
        let name = name.into();
        validate_host_function_name(&name)?;
        if address.is_null() {
            return Err(OriHostRegistryError::NullAddress(name));
        }
        validate_host_signature(&signature)?;
        if self.contains_name(&name) {
            return Err(OriHostRegistryError::Duplicate(name));
        }
        self.functions.insert(
            name.clone(),
            RegisteredHostFunction {
                info: OriHostFunction { name, signature },
                address: address as usize,
            },
        );
        Ok(())
    }

    /// Register an integer callback used by an `extern host` declaration.
    ///
    /// The callback address and `user_data` must remain valid until
    /// [`Self::remove_callback`] succeeds. Unregistering while a callback is
    /// active returns [`OriHostRegistryError::CallbackActive`], which keeps
    /// the host-owned context alive without blocking or deadlocking a
    /// reentrant callback.
    ///
    /// # Safety
    ///
    /// The callback must use the exact [`OriIntCallback`] ABI. `user_data`
    /// must point to host-owned state that is valid for every callback call.
    pub unsafe fn register_int_callback(
        &mut self,
        name: impl Into<String>,
        signature: OriFunctionSignature,
        user_data: *mut u8,
        callback: OriIntCallback,
    ) -> Result<OriCallbackId, OriHostRegistryError> {
        self.register_callback(
            name,
            signature,
            user_data,
            callback as *const () as usize,
            CallbackValueKind::Int,
        )
    }

    /// Register a floating-point callback used by an `extern host` declaration.
    ///
    /// The callback signature must use homogeneous `float` parameters and an
    /// optional `float` return, with at most four parameters.
    ///
    /// # Safety
    ///
    /// The callback must use the exact [`OriFloatCallback`] ABI. `user_data`
    /// must point to host-owned state that is valid for every callback call.
    pub unsafe fn register_float_callback(
        &mut self,
        name: impl Into<String>,
        signature: OriFunctionSignature,
        user_data: *mut u8,
        callback: OriFloatCallback,
    ) -> Result<OriCallbackId, OriHostRegistryError> {
        self.register_callback(
            name,
            signature,
            user_data,
            callback as *const () as usize,
            CallbackValueKind::Float,
        )
    }

    /// Register a boolean callback used by an `extern host` declaration.
    ///
    /// The callback signature must use homogeneous `bool` parameters and an
    /// optional `bool` return, with at most four parameters.
    ///
    /// # Safety
    ///
    /// The callback must use the exact [`OriBoolCallback`] ABI. `user_data`
    /// must point to host-owned state that is valid for every callback call.
    pub unsafe fn register_bool_callback(
        &mut self,
        name: impl Into<String>,
        signature: OriFunctionSignature,
        user_data: *mut u8,
        callback: OriBoolCallback,
    ) -> Result<OriCallbackId, OriHostRegistryError> {
        self.register_callback(
            name,
            signature,
            user_data,
            callback as *const () as usize,
            CallbackValueKind::Bool,
        )
    }

    fn register_callback(
        &mut self,
        name: impl Into<String>,
        signature: OriFunctionSignature,
        user_data: *mut u8,
        callback: usize,
        kind: CallbackValueKind,
    ) -> Result<OriCallbackId, OriHostRegistryError> {
        let name = name.into();
        validate_host_function_name(&name)?;
        validate_callback_signature(&signature, kind)?;
        if self.contains_name(&name) {
            return Err(OriHostRegistryError::Duplicate(name));
        }
        let id = OriCallbackId(NEXT_CALLBACK_ID.fetch_add(1, Ordering::Relaxed));
        let state = Arc::new(CallbackState {
            callback,
            user_data: user_data as usize,
            kind,
            active_calls: AtomicUsize::new(0),
            orphaned: AtomicBool::new(false),
        });
        callback_states()
            .lock()
            .expect("host callback registry mutex poisoned")
            .insert(id.get(), Arc::clone(&state));
        self.callbacks.insert(
            name.clone(),
            RegisteredHostCallback {
                info: OriHostCallback {
                    id,
                    name,
                    signature,
                },
                kind,
            },
        );
        Ok(id)
    }

    pub fn remove_function(&mut self, name: &str) -> bool {
        self.functions.remove(name).is_some()
    }

    /// Remove a callback when no invocation is active.
    pub fn remove_callback(&mut self, name: &str) -> Result<(), OriHostRegistryError> {
        let callback = self
            .callbacks
            .get(name)
            .ok_or_else(|| OriHostRegistryError::CallbackNotFound(name.to_owned()))?;
        unregister_callback_state(callback.info.id)?;
        self.callbacks.remove(name);
        Ok(())
    }

    pub fn function(&self, name: &str) -> Option<&OriHostFunction> {
        self.functions.get(name).map(|function| &function.info)
    }

    pub fn functions(&self) -> impl Iterator<Item = &OriHostFunction> {
        self.functions.values().map(|function| &function.info)
    }

    pub fn callback(&self, name: &str) -> Option<&OriHostCallback> {
        self.callbacks.get(name).map(|callback| &callback.info)
    }

    pub fn callbacks(&self) -> impl Iterator<Item = &OriHostCallback> {
        self.callbacks.values().map(|callback| &callback.info)
    }

    fn jit_symbols(&self) -> Vec<ori_codegen::JitHostSymbol> {
        self.functions
            .values()
            .map(|function| ori_codegen::JitHostSymbol {
                name: function.info.name.clone(),
                address: function.address,
                callback_id: None,
            })
            .collect()
    }

    fn jit_symbols_for_hir(
        &self,
        hir: &HirModule,
    ) -> Result<Vec<ori_codegen::JitHostSymbol>, OriEmbedError> {
        validate_host_imports(hir, self)?;
        let mut symbols = self.jit_symbols();
        for external in &hir.externs {
            let HirExtern::Func {
                path, name, abi, ..
            } = external
            else {
                continue;
            };
            if abi != "host" {
                continue;
            }
            if let Some(callback) = self.resolve_callback(path, name) {
                if !symbols.iter().any(|symbol| symbol.name == name.as_str()) {
                    symbols.push(ori_codegen::JitHostSymbol {
                        name: name.to_string(),
                        address: callback_dispatch_address(
                            &callback.info.signature,
                            callback.kind,
                        )?,
                        callback_id: Some(callback.info.id.get()),
                    });
                }
                continue;
            }
            let registered = self
                .resolve_function(path, name)
                .expect("validated host import must be registered");
            if !symbols.iter().any(|symbol| symbol.name == name.as_str()) {
                symbols.push(ori_codegen::JitHostSymbol {
                    name: name.to_string(),
                    address: registered.address,
                    callback_id: None,
                });
            }
        }
        Ok(symbols)
    }

    fn contains_name(&self, name: &str) -> bool {
        self.functions.contains_key(name) || self.callbacks.contains_key(name)
    }

    fn resolve_function(&self, path: &str, short_name: &str) -> Option<&RegisteredHostFunction> {
        self.functions
            .get(path)
            .or_else(|| self.functions.get(short_name))
    }

    fn resolve_callback(&self, path: &str, short_name: &str) -> Option<&RegisteredHostCallback> {
        self.callbacks
            .get(path)
            .or_else(|| self.callbacks.get(short_name))
    }
}

impl Drop for OriHostRegistry {
    fn drop(&mut self) {
        for callback in self.callbacks.values() {
            orphan_callback_state(callback.info.id);
        }
    }
}

fn orphan_callback_state(id: OriCallbackId) {
    let mut states = callback_states()
        .lock()
        .expect("host callback registry mutex poisoned");
    let Some(state) = states.get(&id.get()).cloned() else {
        return;
    };
    state.orphaned.store(true, Ordering::Release);
    if state.active_calls.load(Ordering::Acquire) == 0 {
        states.remove(&id.get());
    }
}

fn unregister_callback_state(id: OriCallbackId) -> Result<(), OriHostRegistryError> {
    let mut states = callback_states()
        .lock()
        .expect("host callback registry mutex poisoned");
    let Some(state) = states.get(&id.get()) else {
        return Err(OriHostRegistryError::CallbackNotFound(id.get().to_string()));
    };
    if state.active_calls.load(Ordering::Acquire) != 0 {
        return Err(OriHostRegistryError::CallbackActive(id));
    }
    state.orphaned.store(true, Ordering::Release);
    states.remove(&id.get());
    Ok(())
}

fn acquire_callback(id: u64) -> Option<Arc<CallbackState>> {
    let states = callback_states()
        .lock()
        .expect("host callback registry mutex poisoned");
    let state = states.get(&id)?.clone();
    if state.orphaned.load(Ordering::Acquire) {
        return None;
    }
    state.active_calls.fetch_add(1, Ordering::AcqRel);
    Some(state)
}

fn release_callback(id: u64, state: &Arc<CallbackState>) {
    if state.active_calls.fetch_sub(1, Ordering::AcqRel) != 1
        || !state.orphaned.load(Ordering::Acquire)
    {
        return;
    }
    let mut states = callback_states()
        .lock()
        .expect("host callback registry mutex poisoned");
    if states
        .get(&id)
        .is_some_and(|current| Arc::ptr_eq(current, state))
    {
        states.remove(&id);
    }
}

fn begin_callback(id: u64) -> Option<(usize, Arc<CallbackState>)> {
    let depth = CALLBACK_DEPTH.with(Cell::get);
    if depth >= MAX_HOST_CALLBACK_DEPTH {
        set_callback_error(
            ORI_HOST_CALLBACK_REENTRANCY_LIMIT,
            format!("host callback recursion exceeded depth {MAX_HOST_CALLBACK_DEPTH}"),
        );
        return None;
    }
    let Some(state) = acquire_callback(id) else {
        set_callback_error(
            ORI_HOST_CALLBACK_CANCELLED,
            format!("host callback {id} is no longer registered"),
        );
        return None;
    };
    CALLBACK_DEPTH.with(|current| current.set(depth + 1));
    Some((depth, state))
}

fn finish_callback(id: u64, depth: usize, state: &Arc<CallbackState>) {
    CALLBACK_DEPTH.with(|current| current.set(depth));
    release_callback(id, state);
}

fn invoke_int_callback(id: u64, args: [i64; 4]) -> i64 {
    let Some((depth, state)) = begin_callback(id) else {
        return 0;
    };
    if state.kind != CallbackValueKind::Int {
        set_callback_error(
            ORI_HOST_CALLBACK_CANCELLED,
            format!("host callback {id} was invoked with an incompatible scalar ABI"),
        );
        finish_callback(id, depth, &state);
        return 0;
    }
    // SAFETY: registration requires the exact callback ABI and keeps the
    // function address live until unregister succeeds.
    let callback: OriIntCallback = unsafe { std::mem::transmute(state.callback) };
    // SAFETY: the host owns `user_data` and promised its lifetime at
    // registration; this dispatcher only forwards the opaque pointer.
    let result = unsafe {
        callback(
            state.user_data as *mut u8,
            args[0],
            args[1],
            args[2],
            args[3],
        )
    };
    finish_callback(id, depth, &state);
    result
}

fn invoke_float_callback(id: u64, args: [f64; 4]) -> f64 {
    let Some((depth, state)) = begin_callback(id) else {
        return 0.0;
    };
    if state.kind != CallbackValueKind::Float {
        set_callback_error(
            ORI_HOST_CALLBACK_CANCELLED,
            format!("host callback {id} was invoked with an incompatible scalar ABI"),
        );
        finish_callback(id, depth, &state);
        return 0.0;
    }
    // SAFETY: registration requires the exact callback ABI and keeps the
    // function address live until unregister succeeds.
    let callback: OriFloatCallback = unsafe { std::mem::transmute(state.callback) };
    // SAFETY: the host owns `user_data` and promised its lifetime at
    // registration; this dispatcher only forwards the opaque pointer.
    let result = unsafe {
        callback(
            state.user_data as *mut u8,
            args[0],
            args[1],
            args[2],
            args[3],
        )
    };
    finish_callback(id, depth, &state);
    result
}

fn invoke_bool_callback(id: u64, args: [i8; 4]) -> i8 {
    let Some((depth, state)) = begin_callback(id) else {
        return 0;
    };
    if state.kind != CallbackValueKind::Bool {
        set_callback_error(
            ORI_HOST_CALLBACK_CANCELLED,
            format!("host callback {id} was invoked with an incompatible scalar ABI"),
        );
        finish_callback(id, depth, &state);
        return 0;
    }
    // SAFETY: registration requires the exact callback ABI and keeps the
    // function address live until unregister succeeds.
    let callback: OriBoolCallback = unsafe { std::mem::transmute(state.callback) };
    // SAFETY: the host owns `user_data` and promised its lifetime at
    // registration; this dispatcher only forwards the opaque pointer.
    let result = unsafe {
        callback(
            state.user_data as *mut u8,
            args[0],
            args[1],
            args[2],
            args[3],
        )
    };
    finish_callback(id, depth, &state);
    result
}

macro_rules! define_callback_dispatchers {
    ($name:ident, $void_name:ident, $arg_ty:ty, $return_ty:ty, $invoke:path, (), [$($value:expr),*]) => {
        unsafe extern "C" fn $name(id: i64) -> $return_ty {
            $invoke(id as u64, [$($value),*])
        }

        unsafe extern "C" fn $void_name(id: i64) {
            let _ = $invoke(id as u64, [$($value),*]);
        }
    };
    ($name:ident, $void_name:ident, $arg_ty:ty, $return_ty:ty, $invoke:path, ($($arg:ident),*), [$($value:expr),*]) => {
        unsafe extern "C" fn $name(id: i64, $($arg: $arg_ty),*) -> $return_ty {
            $invoke(id as u64, [$($value),*])
        }

        unsafe extern "C" fn $void_name(id: i64, $($arg: $arg_ty),*) {
            let _ = $invoke(id as u64, [$($value),*]);
        }
    };
}

define_callback_dispatchers!(
    dispatch_int_callback_0,
    dispatch_void_callback_0,
    i64,
    i64,
    invoke_int_callback,
    (),
    [0, 0, 0, 0]
);
define_callback_dispatchers!(
    dispatch_int_callback_1,
    dispatch_void_callback_1,
    i64,
    i64,
    invoke_int_callback,
    (a),
    [a, 0, 0, 0]
);
define_callback_dispatchers!(
    dispatch_int_callback_2,
    dispatch_void_callback_2,
    i64,
    i64,
    invoke_int_callback,
    (a, b),
    [a, b, 0, 0]
);
define_callback_dispatchers!(
    dispatch_int_callback_3,
    dispatch_void_callback_3,
    i64,
    i64,
    invoke_int_callback,
    (a, b, c),
    [a, b, c, 0]
);
define_callback_dispatchers!(
    dispatch_int_callback_4,
    dispatch_void_callback_4,
    i64,
    i64,
    invoke_int_callback,
    (a, b, c, d),
    [a, b, c, d]
);
define_callback_dispatchers!(
    dispatch_float_callback_0,
    dispatch_void_float_callback_0,
    f64,
    f64,
    invoke_float_callback,
    (),
    [0.0, 0.0, 0.0, 0.0]
);
define_callback_dispatchers!(
    dispatch_float_callback_1,
    dispatch_void_float_callback_1,
    f64,
    f64,
    invoke_float_callback,
    (a),
    [a, 0.0, 0.0, 0.0]
);
define_callback_dispatchers!(
    dispatch_float_callback_2,
    dispatch_void_float_callback_2,
    f64,
    f64,
    invoke_float_callback,
    (a, b),
    [a, b, 0.0, 0.0]
);
define_callback_dispatchers!(
    dispatch_float_callback_3,
    dispatch_void_float_callback_3,
    f64,
    f64,
    invoke_float_callback,
    (a, b, c),
    [a, b, c, 0.0]
);
define_callback_dispatchers!(
    dispatch_float_callback_4,
    dispatch_void_float_callback_4,
    f64,
    f64,
    invoke_float_callback,
    (a, b, c, d),
    [a, b, c, d]
);
define_callback_dispatchers!(
    dispatch_bool_callback_0,
    dispatch_void_bool_callback_0,
    i8,
    i8,
    invoke_bool_callback,
    (),
    [0, 0, 0, 0]
);
define_callback_dispatchers!(
    dispatch_bool_callback_1,
    dispatch_void_bool_callback_1,
    i8,
    i8,
    invoke_bool_callback,
    (a),
    [a, 0, 0, 0]
);
define_callback_dispatchers!(
    dispatch_bool_callback_2,
    dispatch_void_bool_callback_2,
    i8,
    i8,
    invoke_bool_callback,
    (a, b),
    [a, b, 0, 0]
);
define_callback_dispatchers!(
    dispatch_bool_callback_3,
    dispatch_void_bool_callback_3,
    i8,
    i8,
    invoke_bool_callback,
    (a, b, c),
    [a, b, c, 0]
);
define_callback_dispatchers!(
    dispatch_bool_callback_4,
    dispatch_void_bool_callback_4,
    i8,
    i8,
    invoke_bool_callback,
    (a, b, c, d),
    [a, b, c, d]
);

fn callback_dispatch_address(
    signature: &OriFunctionSignature,
    kind: CallbackValueKind,
) -> Result<usize, OriEmbedError> {
    validate_callback_signature(signature, kind)
        .map_err(|error| OriEmbedError::Compiler(error.to_string()))?;
    let returns_value = signature.return_type.is_some();
    let address = match (kind, signature.params.len(), returns_value) {
        (CallbackValueKind::Int, 0, true) => dispatch_int_callback_0 as *const () as usize,
        (CallbackValueKind::Int, 0, false) => dispatch_void_callback_0 as *const () as usize,
        (CallbackValueKind::Int, 1, true) => dispatch_int_callback_1 as *const () as usize,
        (CallbackValueKind::Int, 1, false) => dispatch_void_callback_1 as *const () as usize,
        (CallbackValueKind::Int, 2, true) => dispatch_int_callback_2 as *const () as usize,
        (CallbackValueKind::Int, 2, false) => dispatch_void_callback_2 as *const () as usize,
        (CallbackValueKind::Int, 3, true) => dispatch_int_callback_3 as *const () as usize,
        (CallbackValueKind::Int, 3, false) => dispatch_void_callback_3 as *const () as usize,
        (CallbackValueKind::Int, 4, true) => dispatch_int_callback_4 as *const () as usize,
        (CallbackValueKind::Int, 4, false) => dispatch_void_callback_4 as *const () as usize,
        (CallbackValueKind::Float, 0, true) => dispatch_float_callback_0 as *const () as usize,
        (CallbackValueKind::Float, 0, false) => {
            dispatch_void_float_callback_0 as *const () as usize
        }
        (CallbackValueKind::Float, 1, true) => dispatch_float_callback_1 as *const () as usize,
        (CallbackValueKind::Float, 1, false) => {
            dispatch_void_float_callback_1 as *const () as usize
        }
        (CallbackValueKind::Float, 2, true) => dispatch_float_callback_2 as *const () as usize,
        (CallbackValueKind::Float, 2, false) => {
            dispatch_void_float_callback_2 as *const () as usize
        }
        (CallbackValueKind::Float, 3, true) => dispatch_float_callback_3 as *const () as usize,
        (CallbackValueKind::Float, 3, false) => {
            dispatch_void_float_callback_3 as *const () as usize
        }
        (CallbackValueKind::Float, 4, true) => dispatch_float_callback_4 as *const () as usize,
        (CallbackValueKind::Float, 4, false) => {
            dispatch_void_float_callback_4 as *const () as usize
        }
        (CallbackValueKind::Bool, 0, true) => dispatch_bool_callback_0 as *const () as usize,
        (CallbackValueKind::Bool, 0, false) => dispatch_void_bool_callback_0 as *const () as usize,
        (CallbackValueKind::Bool, 1, true) => dispatch_bool_callback_1 as *const () as usize,
        (CallbackValueKind::Bool, 1, false) => dispatch_void_bool_callback_1 as *const () as usize,
        (CallbackValueKind::Bool, 2, true) => dispatch_bool_callback_2 as *const () as usize,
        (CallbackValueKind::Bool, 2, false) => dispatch_void_bool_callback_2 as *const () as usize,
        (CallbackValueKind::Bool, 3, true) => dispatch_bool_callback_3 as *const () as usize,
        (CallbackValueKind::Bool, 3, false) => dispatch_void_bool_callback_3 as *const () as usize,
        (CallbackValueKind::Bool, 4, true) => dispatch_bool_callback_4 as *const () as usize,
        (CallbackValueKind::Bool, 4, false) => dispatch_void_bool_callback_4 as *const () as usize,
        _ => {
            return Err(OriEmbedError::Compiler(
                "invalid callback signature".to_string(),
            ))
        }
    };
    Ok(address)
}

/// Errors raised while registering a host function.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OriHostRegistryError {
    #[error("host function name cannot be empty")]
    EmptyName,
    #[error("host function name `{0}` contains whitespace or a NUL byte")]
    InvalidName(String),
    #[error("host function name `{0}` is reserved for the Ori runtime")]
    ReservedName(String),
    #[error("host function `{0}` has a null address")]
    NullAddress(String),
    #[error("host function `{0}` is already registered")]
    Duplicate(String),
    #[error("host callback `{0}` is not registered")]
    CallbackNotFound(String),
    #[error("host callback {0:?} has an invocation in progress")]
    CallbackActive(OriCallbackId),
    #[error("host function signature has more than four parameters")]
    TooManyParameters,
    #[error("host function signature contains an unsupported type")]
    UnsupportedType,
    #[error("host callback must use only integer parameters and an integer or void return")]
    UnsupportedCallbackSignature,
}

/// A function address represented without exposing a raw code pointer.
///
/// The generation makes handles self-invalidating after a successful module
/// replacement. The old executable module is retained internally until the
/// engine is dropped, so an in-flight caller never observes freed JIT memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriFunctionHandle {
    session: SessionId,
    module: ModuleId,
    generation: ModuleGeneration,
    name: String,
}

impl OriFunctionHandle {
    pub fn session(&self) -> SessionId {
        self.session
    }

    pub fn module(&self) -> ModuleId {
        self.module
    }

    pub fn generation(&self) -> ModuleGeneration {
        self.generation
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Result of compiling an in-memory source update into a persistent JIT
/// generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileResult {
    pub module: ModuleSnapshot,
    pub accepted: bool,
    pub diagnostics: Vec<OriDiagnostic>,
    pub functions: Vec<OriFunctionInfo>,
}

/// Result of explicitly unloading one logical module from a hosted session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnloadResult {
    pub module: ModuleSnapshot,
    pub retired_generations: usize,
}

/// Result of unloading every module from a hosted session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnloadAllResult {
    pub modules: usize,
    pub retired_generations: usize,
}

/// A structured diagnostic safe to copy across a host boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriDiagnostic {
    pub severity: OriSeverity,
    pub code: String,
    pub message: String,
    pub labels: Vec<OriLabel>,
    pub why: Option<String>,
    pub action: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriLabel {
    pub path: PathBuf,
    pub span: OriSpan,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriSpan {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Errors produced by the hosted compiler boundary itself.
#[derive(Debug, Error)]
pub enum OriEmbedError {
    #[error("module name cannot be empty")]
    EmptyModuleName,
    #[error("module name `{0}` contains a path separator")]
    InvalidModuleName(String),
    #[error("compiler pipeline failed: {0}")]
    Compiler(String),
    #[error("host function `{0}` is not registered")]
    MissingHostFunction(String),
    #[error(
        "host function `{name}` has signature {registered:?}, but source declares {declared:?}"
    )]
    HostSignatureMismatch {
        name: String,
        registered: OriFunctionSignature,
        declared: OriFunctionSignature,
    },
    #[error("module `{0}` has no accepted executable generation")]
    ModuleNotCompiled(String),
    #[error("public function `{function}` was not found in module `{module}`")]
    FunctionNotFound { module: String, function: String },
    #[error(
        "public function `{function}` in module `{module}` has an unsupported hosted signature"
    )]
    UnsupportedFunctionSignature { module: String, function: String },
    #[error("function handle belongs to another hosted session")]
    WrongSession,
    #[error(
        "function handle for module `{module}` is stale (handle generation {handle_generation}, current generation {current_generation})"
    )]
    StaleFunctionHandle {
        module: String,
        handle_generation: u64,
        current_generation: u64,
    },
    #[error("hosted function call failed: {0}")]
    FunctionCall(String),
    #[error("hosted function trapped ({0})")]
    FunctionTrap(OriExecutionError),
}

/// Long-lived hosted compiler session with generational scalar JIT calls.
///
/// Aggregate/managed values, async entry points, and cross-thread invocation
/// are outside this first execution boundary. Trusted integer callbacks are
/// available through `OriHostRegistry`, with explicit lifecycle and bounded
/// synchronous reentry. Scalar traps use the runtime's explicit per-thread
/// error slot instead of unwinding or aborting.
#[derive(Debug)]
pub struct OriEngine {
    session_id: SessionId,
    config: OriConfig,
    host_registry: OriHostRegistry,
    next_module_id: u64,
    modules: HashMap<String, ModuleState>,
    module_names: HashMap<ModuleId, String>,
}

#[derive(Debug)]
struct ModuleState {
    snapshot: ModuleSnapshot,
    source: String,
    current: Option<ori_codegen::CompiledJitModule>,
    retired: Vec<ori_codegen::CompiledJitModule>,
    functions: Vec<OriFunctionInfo>,
}

impl OriEngine {
    pub fn new(config: OriConfig) -> Self {
        Self::with_host_registry(config, OriHostRegistry::default())
    }

    pub fn with_host_registry(config: OriConfig, host_registry: OriHostRegistry) -> Self {
        Self {
            session_id: SessionId(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed)),
            config,
            host_registry,
            next_module_id: 1,
            modules: HashMap::new(),
            module_names: HashMap::new(),
        }
    }

    pub fn config(&self) -> &OriConfig {
        &self.config
    }

    pub fn host_registry(&self) -> &OriHostRegistry {
        &self.host_registry
    }

    pub fn host_registry_mut(&mut self) -> &mut OriHostRegistry {
        &mut self.host_registry
    }

    /// Check a source update without publishing a new executable generation.
    ///
    /// A valid check never advances the module generation: only
    /// [`Self::compile_source`] publishes a new JIT generation, so checking
    /// cannot invalidate handles issued for the current executable. The
    /// checked source is recorded on acceptance and discarded on error, which
    /// lets a host validate candidate edits without disturbing live calls.
    pub fn check_source(
        &mut self,
        module_name: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<CheckResult, OriEmbedError> {
        let module_name = module_name.into();
        validate_module_name(&module_name)?;
        let source = source.into();
        let path = PathBuf::from(&module_name);
        let cfg = self.config.cfg_context();
        let check_path = path.clone();
        let check_source = source.clone();

        let output = ori_driver::pipeline::with_frontend_stack(move || {
            run_check_source_with_options(
                Path::new(&check_path),
                check_source,
                CheckOptions { cfg: Some(cfg) },
            )
        })
        .map_err(OriEmbedError::Compiler)?;

        let diagnostics = diagnostics_from_output(&output.diagnostics, &output.cache);
        let state = self.ensure_module(&module_name);
        let accepted = !output.has_errors;
        if accepted {
            state.source = source;
        }

        Ok(CheckResult {
            module: state.snapshot,
            accepted,
            diagnostics,
        })
    }

    /// Compile a source update and retain its finalized JIT module on success.
    ///
    /// A source error leaves the current executable generation untouched. A
    /// successful replacement increments the generation and invalidates all
    /// handles issued for the previous generation.
    pub fn compile_source(
        &mut self,
        module_name: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<CompileResult, OriEmbedError> {
        let module_name = module_name.into();
        validate_module_name(&module_name)?;
        let source = source.into();
        let path = PathBuf::from(&module_name);
        let cfg = self.config.cfg_context();
        let compile_path = path.clone();
        let compile_source = source.clone();

        let output = ori_driver::pipeline::with_frontend_stack(move || {
            lower_jit_source_with_options(
                Path::new(&compile_path),
                compile_source,
                CheckOptions { cfg: Some(cfg) },
            )
        })
        .map_err(OriEmbedError::Compiler)?;

        let diagnostics = diagnostics_from_output(&output.diagnostics, &output.cache);
        let module = match (output.has_errors, output.hir, output.cdylib) {
            (false, Some(hir), Some(cdylib)) => {
                let host_symbols = self.host_registry.jit_symbols_for_hir(&hir)?;
                Some(
                    ori_codegen::CompiledJitModule::compile_with_host_symbols(
                        &hir,
                        &cdylib,
                        &output.native_libs,
                        &host_symbols,
                    )
                    .map_err(OriEmbedError::Compiler)?,
                )
            }
            _ => None,
        };
        let state = self.ensure_module(&module_name);

        let Some(module) = module else {
            return Ok(CompileResult {
                module: state.snapshot,
                accepted: false,
                diagnostics,
                functions: state.functions.clone(),
            });
        };

        let next_generation = state
            .snapshot
            .generation
            .map_or(FIRST_MODULE_GENERATION, |generation| generation.0 + 1);
        let functions = module
            .functions()
            .filter(|function| function.is_public)
            .map(function_info_from_jit)
            .collect::<Vec<_>>();
        let mut functions = functions;
        functions.sort_by(|left, right| left.name.cmp(&right.name));
        if let Some(previous) = state.current.replace(module) {
            state.retired.push(previous);
        }
        state.snapshot.generation = Some(ModuleGeneration(next_generation));
        state.source = source;
        state.functions = functions.clone();

        Ok(CompileResult {
            module: state.snapshot,
            accepted: !output.has_errors,
            diagnostics,
            functions,
        })
    }

    /// Resolve a public function to a generation-bound handle.
    pub fn function(
        &self,
        module_name: &str,
        function_name: &str,
    ) -> Result<OriFunctionHandle, OriEmbedError> {
        let state = self
            .modules
            .get(module_name)
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.to_owned()))?;
        let generation = state
            .snapshot
            .generation
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.to_owned()))?;
        let function = find_public_function(&state.functions, function_name).ok_or_else(|| {
            OriEmbedError::FunctionNotFound {
                module: module_name.to_owned(),
                function: function_name.to_owned(),
            }
        })?;
        if !is_supported_hosted_signature(&function.signature) {
            return Err(OriEmbedError::UnsupportedFunctionSignature {
                module: module_name.to_owned(),
                function: function_name.to_owned(),
            });
        }
        Ok(OriFunctionHandle {
            session: self.session_id,
            module: state.snapshot.id,
            generation,
            name: function.name.clone(),
        })
    }

    /// Invoke a scalar function through a generation-bound handle.
    pub fn call(
        &self,
        handle: &OriFunctionHandle,
        args: &[OriValue],
    ) -> Result<Option<OriValue>, OriEmbedError> {
        if handle.session != self.session_id {
            return Err(OriEmbedError::WrongSession);
        }
        let module_name = self
            .module_names
            .get(&handle.module)
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(handle.name.clone()))?;
        let state = self
            .modules
            .get(module_name)
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(handle.name.clone()))?;
        let current_generation = state
            .snapshot
            .generation
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.clone()))?;
        if current_generation != handle.generation {
            return Err(OriEmbedError::StaleFunctionHandle {
                module: module_name.clone(),
                handle_generation: handle.generation.0,
                current_generation: current_generation.0,
            });
        }
        let module = state
            .current
            .as_ref()
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.clone()))?;
        let args = args
            .iter()
            .copied()
            .map(jit_value_from_ori)
            .collect::<Vec<_>>();
        clear_callback_error();
        let result = module
            .call(&handle.name, &args)
            .map(|value| value.map(ori_value_from_jit));
        if let Some(error) = take_callback_error() {
            return Err(OriEmbedError::FunctionTrap(error));
        }
        result.map_err(|error| match error {
            ori_codegen::JitCallError::Invocation(message) => OriEmbedError::FunctionCall(message),
            ori_codegen::JitCallError::Runtime { code, message } => {
                OriEmbedError::FunctionTrap(OriExecutionError { code, message })
            }
        })
    }

    /// Remove one module and release all executable generations it owns.
    ///
    /// Handles for the removed module remain harmless values and fail with
    /// `ModuleNotCompiled` when used afterwards. The mutable borrow prevents
    /// an unload from racing with a call through the same engine instance.
    pub fn unload_module(&mut self, module_name: &str) -> Result<UnloadResult, OriEmbedError> {
        let generation = self
            .modules
            .get(module_name)
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.to_owned()))?
            .snapshot
            .generation
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.to_owned()))?;
        let state = self
            .modules
            .remove(module_name)
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.to_owned()))?;
        self.module_names.remove(&state.snapshot.id);
        let retired_generations = state.retired.len();
        drop(state.current);
        drop(state.retired);
        Ok(UnloadResult {
            module: ModuleSnapshot {
                id: state.snapshot.id,
                generation: Some(generation),
            },
            retired_generations,
        })
    }

    pub fn module(&self, module_name: &str) -> Option<ModuleSnapshot> {
        self.modules.get(module_name).map(|state| state.snapshot)
    }

    /// Return every loaded module with its current snapshot.
    pub fn modules(&self) -> impl Iterator<Item = (&str, ModuleSnapshot)> {
        self.modules
            .iter()
            .map(|(name, state)| (name.as_str(), state.snapshot))
    }

    /// Return the last accepted source for a module.
    ///
    /// A module checked with [`Self::check_source`] records its accepted
    /// candidate here too; a rejected source never replaces it.
    pub fn module_source(&self, module_name: &str) -> Option<&str> {
        self.modules
            .get(module_name)
            .map(|state| state.source.as_str())
    }

    /// Return the public function metadata of a module.
    ///
    /// A module that was only checked (never compiled) has an empty list.
    pub fn functions(&self, module_name: &str) -> Result<&[OriFunctionInfo], OriEmbedError> {
        let state = self
            .modules
            .get(module_name)
            .ok_or_else(|| OriEmbedError::ModuleNotCompiled(module_name.to_owned()))?;
        Ok(&state.functions)
    }

    /// Remove every module and release all executable generations.
    ///
    /// Handles issued for any removed module fail with `ModuleNotCompiled`
    /// afterwards, exactly like [`Self::unload_module`]. The session itself
    /// remains usable and can compile fresh modules.
    pub fn unload_all(&mut self) -> UnloadAllResult {
        let modules = self.modules.len();
        let retired_generations = self.modules.values().map(|state| state.retired.len()).sum();
        self.modules.clear();
        self.module_names.clear();
        UnloadAllResult {
            modules,
            retired_generations,
        }
    }

    fn ensure_module(&mut self, module_name: &str) -> &mut ModuleState {
        if !self.modules.contains_key(module_name) {
            let id = ModuleId(self.next_module_id);
            self.next_module_id += 1;
            self.module_names.insert(id, module_name.to_owned());
            self.modules.insert(
                module_name.to_owned(),
                ModuleState {
                    snapshot: ModuleSnapshot {
                        id,
                        generation: None,
                    },
                    source: String::new(),
                    current: None,
                    retired: Vec::new(),
                    functions: Vec::new(),
                },
            );
        }
        self.modules
            .get_mut(module_name)
            .expect("module was inserted or already existed")
    }
}

fn diagnostics_from_output(diagnostics: &[Diagnostic], cache: &SourceCache) -> Vec<OriDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| OriDiagnostic {
            severity: match diagnostic.severity {
                Severity::Error => OriSeverity::Error,
                Severity::Warning => OriSeverity::Warning,
            },
            code: diagnostic.code.to_owned(),
            message: diagnostic.message.clone(),
            labels: diagnostic
                .labels
                .iter()
                .map(|label| OriLabel {
                    path: cache
                        .get(label.file_id)
                        .map_or_else(PathBuf::new, |file| file.path.clone()),
                    span: span_location(cache, label.file_id, label.span),
                    message: label.message.clone(),
                })
                .collect(),
            why: diagnostic.why.clone(),
            action: diagnostic.action.clone(),
            notes: diagnostic.notes.clone(),
        })
        .collect()
}

fn function_info_from_jit(function: &ori_codegen::JitFunctionInfo) -> OriFunctionInfo {
    OriFunctionInfo {
        name: function.name.clone(),
        signature: OriFunctionSignature {
            params: function
                .signature
                .params
                .iter()
                .copied()
                .map(ori_scalar_type_from_jit)
                .collect(),
            return_type: function.signature.return_type.map(ori_scalar_type_from_jit),
        },
        is_public: function.is_public,
    }
}

fn find_public_function<'a>(
    functions: &'a [OriFunctionInfo],
    requested_name: &str,
) -> Option<&'a OriFunctionInfo> {
    let mut matches = functions.iter().filter(|function| {
        function.name == requested_name || function.name.ends_with(&format!(".{requested_name}"))
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn is_supported_hosted_signature(signature: &OriFunctionSignature) -> bool {
    if signature.params.len() > 4 {
        return false;
    }
    let Some(first) = signature.params.first().copied() else {
        return signature
            .return_type
            .is_none_or(|return_type| return_type != OriScalarType::Unsupported);
    };
    first != OriScalarType::Unsupported
        && signature.params.iter().all(|parameter| *parameter == first)
        && signature
            .return_type
            .is_none_or(|return_type| return_type != OriScalarType::Unsupported)
}

fn validate_host_function_name(name: &str) -> Result<(), OriHostRegistryError> {
    if name.is_empty() {
        return Err(OriHostRegistryError::EmptyName);
    }
    if name
        .chars()
        .any(|character| character.is_whitespace() || character == '\0')
    {
        return Err(OriHostRegistryError::InvalidName(name.to_owned()));
    }
    if name.starts_with("ori_") {
        return Err(OriHostRegistryError::ReservedName(name.to_owned()));
    }
    Ok(())
}

fn validate_host_signature(signature: &OriFunctionSignature) -> Result<(), OriHostRegistryError> {
    if signature.params.len() > 4 {
        return Err(OriHostRegistryError::TooManyParameters);
    }
    if signature.params.contains(&OriScalarType::Unsupported)
        || signature.params.contains(&OriScalarType::Slice)
        || signature.params.contains(&OriScalarType::String)
        || signature.params.contains(&OriScalarType::Bytes)
        || signature.return_type.is_some_and(|return_type| {
            matches!(
                return_type,
                OriScalarType::Unsupported
                    | OriScalarType::Slice
                    | OriScalarType::String
                    | OriScalarType::Bytes
            )
        })
    {
        return Err(OriHostRegistryError::UnsupportedType);
    }
    Ok(())
}

fn validate_callback_signature(
    signature: &OriFunctionSignature,
    kind: CallbackValueKind,
) -> Result<(), OriHostRegistryError> {
    validate_host_signature(signature)?;
    let expected = match kind {
        CallbackValueKind::Int => OriScalarType::Int,
        CallbackValueKind::Float => OriScalarType::Float,
        CallbackValueKind::Bool => OriScalarType::Bool,
    };
    if signature
        .params
        .iter()
        .any(|parameter| *parameter != expected)
        || signature
            .return_type
            .is_some_and(|return_type| return_type != expected)
    {
        return Err(OriHostRegistryError::UnsupportedCallbackSignature);
    }
    Ok(())
}

fn ori_scalar_type_from_ty(ty: &Ty) -> OriScalarType {
    match ty {
        Ty::Bool => OriScalarType::Bool,
        Ty::Int => OriScalarType::Int,
        Ty::Float => OriScalarType::Float,
        Ty::Slice(_) => OriScalarType::Slice,
        Ty::String => OriScalarType::String,
        Ty::Bytes => OriScalarType::Bytes,
        _ => OriScalarType::Unsupported,
    }
}

fn host_signature_from_hir(params: &[ori_hir::HirParam], return_ty: &Ty) -> OriFunctionSignature {
    OriFunctionSignature {
        params: params
            .iter()
            .map(|parameter| ori_scalar_type_from_ty(&parameter.ty))
            .collect(),
        return_type: (!matches!(return_ty, Ty::Void)).then(|| ori_scalar_type_from_ty(return_ty)),
    }
}

fn validate_host_imports(hir: &HirModule, registry: &OriHostRegistry) -> Result<(), OriEmbedError> {
    for external in &hir.externs {
        let HirExtern::Func {
            path,
            name,
            params,
            return_ty,
            abi,
            ..
        } = external
        else {
            continue;
        };
        if abi != "host" {
            continue;
        }
        let declared = host_signature_from_hir(params, return_ty);
        validate_host_signature(&declared).map_err(|_| OriEmbedError::HostSignatureMismatch {
            name: path.to_string(),
            registered: declared.clone(),
            declared: declared.clone(),
        })?;
        if let Some(registered) = registry.resolve_callback(path, name) {
            if registered.info.signature != declared {
                return Err(OriEmbedError::HostSignatureMismatch {
                    name: registered.info.name.clone(),
                    registered: registered.info.signature.clone(),
                    declared,
                });
            }
            continue;
        }
        let registered = registry
            .resolve_function(path, name)
            .map(|function| &function.info)
            .ok_or_else(|| OriEmbedError::MissingHostFunction(name.to_string()))?;
        if registered.signature != declared {
            return Err(OriEmbedError::HostSignatureMismatch {
                name: registered.name.clone(),
                registered: registered.signature.clone(),
                declared,
            });
        }
    }
    Ok(())
}

fn ori_scalar_type_from_jit(ty: ori_codegen::JitScalarType) -> OriScalarType {
    match ty {
        ori_codegen::JitScalarType::Bool => OriScalarType::Bool,
        ori_codegen::JitScalarType::Int => OriScalarType::Int,
        ori_codegen::JitScalarType::Float => OriScalarType::Float,
        ori_codegen::JitScalarType::Slice => OriScalarType::Slice,
        ori_codegen::JitScalarType::String => OriScalarType::String,
        ori_codegen::JitScalarType::Bytes => OriScalarType::Bytes,
        ori_codegen::JitScalarType::Unsupported => OriScalarType::Unsupported,
    }
}

fn jit_value_from_ori(value: OriValue) -> ori_codegen::JitValue {
    match value {
        OriValue::Bool(value) => ori_codegen::JitValue::Bool(value),
        OriValue::Int(value) => ori_codegen::JitValue::Int(value),
        OriValue::Float(value) => ori_codegen::JitValue::Float(value),
        OriValue::Slice(pointer) => ori_codegen::JitValue::Slice(pointer),
        OriValue::String(pointer) => ori_codegen::JitValue::String(pointer),
        OriValue::Bytes(pointer) => ori_codegen::JitValue::Bytes(pointer),
    }
}

fn ori_value_from_jit(value: ori_codegen::JitValue) -> OriValue {
    match value {
        ori_codegen::JitValue::Bool(value) => OriValue::Bool(value),
        ori_codegen::JitValue::Int(value) => OriValue::Int(value),
        ori_codegen::JitValue::Float(value) => OriValue::Float(value),
        ori_codegen::JitValue::Slice(pointer) => OriValue::Slice(pointer),
        ori_codegen::JitValue::String(pointer) => OriValue::String(pointer),
        ori_codegen::JitValue::Bytes(pointer) => OriValue::Bytes(pointer),
    }
}

fn span_location(cache: &SourceCache, file_id: FileId, span: Span) -> OriSpan {
    let Some(file) = cache.get(file_id) else {
        return OriSpan {
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
        };
    };
    let (start_line, start_column) = file.line_col(span.start);
    let (end_line, end_column) = file.line_col(span.end);
    OriSpan {
        start_line,
        start_column,
        end_line,
        end_column,
    }
}

fn validate_module_name(name: &str) -> Result<(), OriEmbedError> {
    if name.is_empty() {
        return Err(OriEmbedError::EmptyModuleName);
    }
    if name.contains('/') || name.contains('\\') {
        return Err(OriEmbedError::InvalidModuleName(name.to_owned()));
    }
    Ok(())
}

fn validate_feature_name(name: &str) -> Result<(), OriConfigError> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && name != "default";
    valid
        .then_some(())
        .ok_or_else(|| OriConfigError::InvalidFeature(name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn host_add(left: i64, right: i64) -> i64 {
        left + right
    }

    unsafe extern "C" fn callback_add_user_data(
        user_data: *mut u8,
        value: i64,
        _arg1: i64,
        _arg2: i64,
        _arg3: i64,
    ) -> i64 {
        // SAFETY: the test registers a pointer to a live `i64` for the whole
        // callback session.
        value + unsafe { *(user_data as *const i64) }
    }

    unsafe extern "C" fn callback_record_value(
        user_data: *mut u8,
        value: i64,
        _arg1: i64,
        _arg2: i64,
        _arg3: i64,
    ) -> i64 {
        // SAFETY: the test registers a pointer to a live `i64` for the whole
        // callback session.
        unsafe { *(user_data as *mut i64) += value };
        0
    }

    struct ReentrantContext {
        engine: *const OriEngine,
        nested: Option<OriFunctionHandle>,
    }

    unsafe extern "C" fn callback_reenters_ori(
        user_data: *mut u8,
        value: i64,
        _arg1: i64,
        _arg2: i64,
        _arg3: i64,
    ) -> i64 {
        // SAFETY: the test keeps the boxed context and engine alive while the
        // callback is reachable; the nested call uses the same session's
        // immutable execution boundary.
        let context = unsafe { &*(user_data as *const ReentrantContext) };
        let handle = context.nested.as_ref().expect("nested handle initialized");
        match unsafe { &*context.engine }.call(handle, &[OriValue::Int(value)]) {
            Ok(Some(OriValue::Int(result))) => result,
            result => panic!("unexpected nested callback result: {result:?}"),
        }
    }

    struct CallbackLifecycleContext {
        registry: *mut OriHostRegistry,
        unregister_blocked: bool,
    }

    unsafe extern "C" fn callback_attempts_unregister(
        user_data: *mut u8,
        value: i64,
        _arg1: i64,
        _arg2: i64,
        _arg3: i64,
    ) -> i64 {
        // SAFETY: the test keeps the context and registry alive while the
        // callback is executing.
        let context = unsafe { &mut *(user_data as *mut CallbackLifecycleContext) };
        let result = unsafe { (*context.registry).remove_callback("app.callback.active") };
        context.unregister_blocked = matches!(result, Err(OriHostRegistryError::CallbackActive(_)));
        value
    }

    struct RecursiveCallbackContext {
        id: OriCallbackId,
    }

    unsafe extern "C" fn callback_recurses(
        user_data: *mut u8,
        value: i64,
        _arg1: i64,
        _arg2: i64,
        _arg3: i64,
    ) -> i64 {
        // SAFETY: the test keeps the context alive for the whole call.
        let context = unsafe { &*(user_data as *const RecursiveCallbackContext) };
        invoke_int_callback(context.id.get(), [value, 0, 0, 0])
    }

    #[test]
    fn hosted_session_keeps_last_valid_generation_on_invalid_update() {
        let mut engine = OriEngine::new(OriConfig::default());
        let first = engine
            .check_source(
                "player.orl",
                "module app.player\n\nupdate() -> int\n    return 1\nend\n",
            )
            .expect("first check");
        assert!(first.accepted);
        assert_eq!(first.module.generation, None);

        let second = engine
            .check_source(
                "player.orl",
                "module app.player\n\nupdate() -> Missing\nend\n",
            )
            .expect("invalid source still returns a check result");
        assert!(!second.accepted);
        assert_eq!(second.module.generation, None);
        assert!(second
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == OriSeverity::Error));
    }

    #[test]
    fn config_requires_declared_features() {
        let mut config = OriConfig::default();
        assert!(config.enable_feature("render").is_err());
        config.declare_feature("render").expect("valid feature");
        config.enable_feature("render").expect("declared feature");
        assert_eq!(
            config.enabled_features().collect::<Vec<_>>(),
            vec!["render"]
        );
    }

    #[test]
    fn hosted_config_selects_conditional_declarations() {
        let mut config = OriConfig::default();
        config.declare_feature("hosted").expect("valid feature");
        config.enable_feature("hosted").expect("declared feature");

        let mut engine = OriEngine::new(config);
        let result = engine
            .check_source(
                "profile.orl",
                "module app.profile\n\n@cfg(feature: hosted)\nactive() -> int\n    return 1\nend\n\n@cfg(not(feature: hosted))\ninactive() -> Missing\nend\n",
            )
            .expect("configured source check");

        assert!(result.accepted);
        assert_eq!(result.module.generation, None);
    }

    #[test]
    fn hosted_slice_return_is_exposed_as_an_opaque_window() {
        let mut engine = OriEngine::new(OriConfig::default());
        let result = engine
            .compile_source(
                "slice.orl",
                "module app.slice\n\nimport ori.list = lists\n\npublic window() -> slice[int]\n    var xs: list[int] = [10, 20, 30]\n    return lists.window(xs, 0, 3)\nend\n",
            )
            .expect("compile slice function");

        assert!(result.accepted);
        let window = result
            .functions
            .iter()
            .find(|function| function.name.ends_with(".window"))
            .expect("window function present");
        assert_eq!(window.signature.return_type, Some(OriScalarType::Slice));
        let handle = engine
            .function("slice.orl", "window")
            .expect("slice function resolves");
        let value = engine
            .call(&handle, &[])
            .expect("slice call")
            .expect("slice result");
        assert!(
            matches!(value, OriValue::Slice(pointer) if !pointer.is_null()),
            "slice window must return a live opaque pointer"
        );
    }

    #[test]
    fn hosted_slice_can_be_consumed_by_a_function_parameter() {
        let mut engine = OriEngine::new(OriConfig::default());
        let _result = engine
            .compile_source(
                "slicesum.orl",
                "module app.slicesum\n\nimport ori.list = lists\nimport ori.slice = sl\n\npublic make() -> slice[int]\n    var xs: list[int] = [10, 20, 30]\n    return lists.window(xs, 0, 3)\nend\n\npublic sum(w: slice[int]) -> int\n    var total: int = 0\n    var i: int = 0\n    while i < sl.len(w)\n        total = total + sl.get(w, i)\n        i = i + 1\n    end\n    return total\nend\n",
            )
            .expect("compile slice functions");

        let make = engine
            .function("slicesum.orl", "make")
            .expect("make resolves");
        let sum = engine
            .function("slicesum.orl", "sum")
            .expect("sum resolves");
        let Some(OriValue::Slice(pointer)) = engine.call(&make, &[]).expect("make call") else {
            panic!("make must return a slice");
        };
        let total = engine
            .call(&sum, &[OriValue::Slice(pointer)])
            .expect("sum call")
            .expect("sum result");
        assert_eq!(total, OriValue::Int(60));
    }

    #[test]
    fn hosted_slice_cannot_be_used_in_host_function_registry() {
        let mut registry = OriHostRegistry::default();
        // A dummy non-null address so the unsupported-type check runs first.
        let dummy = host_add as *const () as *const u8;
        assert!(matches!(
            unsafe {
                registry.register_function(
                    "app.hosted.bad_slice",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Slice],
                        return_type: Some(OriScalarType::Int),
                    },
                    dummy,
                )
            },
            Err(OriHostRegistryError::UnsupportedType)
        ));
    }

    #[test]
    fn hosted_string_return_and_operations() {
        let mut engine = OriEngine::new(OriConfig::default());
        let result = engine
            .compile_source(
                "strings.orl",
                "module app.strings\n\npublic greet() -> string\n    return \"Hello, world!\"\nend\n\npublic concat(a: string, b: string) -> string\n    return a + \" \" + b\nend\n",
            )
            .expect("compile string functions");

        assert!(result.accepted);
        let greet = engine
            .function("strings.orl", "greet")
            .expect("greet resolves");
        let value = engine
            .call(&greet, &[])
            .expect("greet call")
            .expect("greet result");
        assert_eq!(value.as_str(), Some("Hello, world!"));
        assert_eq!(value.as_bytes(), Some(b"Hello, world!".as_slice()));

        let concat = engine
            .function("strings.orl", "concat")
            .expect("concat resolves");
        let arg1 = std::ffi::CString::new("Hello").unwrap();
        let arg2 = std::ffi::CString::new("Ori").unwrap();
        let combined = engine
            .call(
                &concat,
                &[
                    OriValue::String(arg1.as_ptr() as *const u8),
                    OriValue::String(arg2.as_ptr() as *const u8),
                ],
            )
            .expect("concat call")
            .expect("concat result");
        assert_eq!(combined.as_str(), Some("Hello Ori"));
    }

    #[test]
    fn hosted_string_parameter_consumption() {
        let mut engine = OriEngine::new(OriConfig::default());
        let _result = engine
            .compile_source(
                "strlen.orl",
                "module app.strlen\n\npublic length(s: string) -> int\n    return s.len()\nend\n",
            )
            .expect("compile strlen function");

        let length = engine
            .function("strlen.orl", "length")
            .expect("length resolves");
        let test_str = std::ffi::CString::new("testing").unwrap();
        let len = engine
            .call(
                &length,
                &[OriValue::String(test_str.as_ptr() as *const u8)],
            )
            .expect("length call")
            .expect("length result");
        assert_eq!(len, OriValue::Int(7));
    }

    #[test]
    fn hosted_bytes_return_and_parameter() {
        let mut engine = OriEngine::new(OriConfig::default());
        let _result = engine
            .compile_source(
                "bytes_test.orl",
                "module app.bytes_test\n\nimport ori.bytes = by\n\npublic make() -> bytes\n    return \"abc\".to_bytes()\nend\n\npublic len_of(b: bytes) -> int\n    return by.len(b)\nend\n",
            )
            .expect("compile bytes functions");

        let make = engine
            .function("bytes_test.orl", "make")
            .expect("make resolves");
        let len_of = engine
            .function("bytes_test.orl", "len_of")
            .expect("len_of resolves");

        let make_result = engine
            .call(&make, &[])
            .expect("make call")
            .expect("make result");
        assert_eq!(make_result.as_bytes(), Some(b"abc".as_slice()));

        let len_result = engine
            .call(&len_of, &[make_result])
            .expect("len_of call")
            .expect("len_of result");
        assert_eq!(len_result, OriValue::Int(3));
    }

    #[test]
    fn hosted_string_and_bytes_cannot_be_used_in_host_function_registry() {
        let mut registry = OriHostRegistry::default();
        let dummy = host_add as *const () as *const u8;
        assert!(matches!(
            unsafe {
                registry.register_function(
                    "app.hosted.bad_str",
                    OriFunctionSignature {
                        params: vec![OriScalarType::String],
                        return_type: Some(OriScalarType::Int),
                    },
                    dummy,
                )
            },
            Err(OriHostRegistryError::UnsupportedType)
        ));
        assert!(matches!(
            unsafe {
                registry.register_function(
                    "app.hosted.bad_bytes",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Bytes],
                        return_type: Some(OriScalarType::Int),
                    },
                    dummy,
                )
            },
            Err(OriHostRegistryError::UnsupportedType)
        ));
    }

    #[test]
    fn check_source_does_not_advance_generation_or_invalidate_handles() {
        let mut engine = OriEngine::new(OriConfig::default());
        engine
            .compile_source(
                "player.orl",
                "module app.player\n\npublic add(left: int, right: int) -> int\n    return left + right\nend\n",
            )
            .expect("first compile");
        let handle = engine.function("player.orl", "add").expect("handle");

        let checked = engine
            .check_source(
                "player.orl",
                "module app.player\n\npublic add(left: int, right: int) -> int\n    return left + right + 1\nend\n",
            )
            .expect("candidate check");
        assert!(checked.accepted);
        assert_eq!(checked.module.generation, Some(ModuleGeneration(1)));

        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(2), OriValue::Int(3)])
                .expect("generation must remain callable after check"),
            Some(OriValue::Int(5))
        );

        let invalid = engine
            .check_source(
                "player.orl",
                "module app.player\n\npublic add(left: int, right: int) -> Missing\nend\n",
            )
            .expect("invalid candidate check");
        assert!(!invalid.accepted);
        assert_eq!(invalid.module.generation, Some(ModuleGeneration(1)));
        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(2), OriValue::Int(3)])
                .expect("generation must remain callable after rejected check"),
            Some(OriValue::Int(5))
        );
    }

    #[test]
    fn hosted_session_compiles_and_calls_public_scalar_function() {
        let mut engine = OriEngine::new(OriConfig::default());
        let result = engine
            .compile_source(
                "player.orl",
                "module app.player\n\npublic add(left: int, right: int) -> int\n    return left + right\nend\n",
            )
            .expect("compile scalar function");

        assert!(result.accepted);
        assert_eq!(result.module.generation, Some(ModuleGeneration(1)));
        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].signature.params.len(), 2);
        let handle = engine
            .function("player.orl", "add")
            .expect("public function");
        let value = engine
            .call(&handle, &[OriValue::Int(2), OriValue::Int(3)])
            .expect("scalar call")
            .expect("integer result");
        assert_eq!(value, OriValue::Int(5));
    }

    #[test]
    fn successful_replacement_stales_old_handles_but_invalid_source_does_not() {
        let mut engine = OriEngine::new(OriConfig::default());
        let source = |expression: &str| {
            format!(
                "module app.player\n\npublic add(left: int, right: int) -> int\n    return {expression}\nend\n"
            )
        };

        engine
            .compile_source("player.orl", source("left + right"))
            .expect("first compile");
        let old_handle = engine.function("player.orl", "add").expect("first handle");

        engine
            .compile_source("player.orl", source("left + right + 1"))
            .expect("replacement compile");
        let new_handle = engine.function("player.orl", "add").expect("new handle");
        assert!(matches!(
            engine.call(&old_handle, &[OriValue::Int(2), OriValue::Int(3)]),
            Err(OriEmbedError::StaleFunctionHandle { .. })
        ));
        assert_eq!(
            engine
                .call(&new_handle, &[OriValue::Int(2), OriValue::Int(3)])
                .expect("new scalar call"),
            Some(OriValue::Int(6))
        );

        let invalid = engine
            .compile_source(
                "player.orl",
                "module app.player\n\npublic add(left: int, right: int) -> Missing\nend\n",
            )
            .expect("invalid source returns diagnostics");
        assert!(!invalid.accepted);
        assert_eq!(invalid.module.generation, Some(ModuleGeneration(2)));
        assert_eq!(
            engine
                .call(&new_handle, &[OriValue::Int(2), OriValue::Int(3)])
                .expect("last valid generation remains callable"),
            Some(OriValue::Int(6))
        );
    }

    #[test]
    fn hosted_session_calls_float_bool_and_void_signatures() {
        let mut engine = OriEngine::new(OriConfig::default());
        let result = engine
            .compile_source(
                "scalar.orl",
                "module app.scalar\n\npublic bump(value: float) -> float\n    return value + 0.5\nend\n\npublic identity(value: bool) -> bool\n    return value\nend\n\npublic ping() -> void\nend\n",
            )
            .expect("compile scalar signatures");
        assert!(result.accepted);
        assert_eq!(result.functions.len(), 3);

        let bump = engine
            .function("scalar.orl", "bump")
            .expect("float function");
        assert_eq!(
            engine
                .call(&bump, &[OriValue::Float(1.25)])
                .expect("float call"),
            Some(OriValue::Float(1.75))
        );

        let identity = engine
            .function("scalar.orl", "identity")
            .expect("bool function");
        assert_eq!(
            engine
                .call(&identity, &[OriValue::Bool(true)])
                .expect("bool call"),
            Some(OriValue::Bool(true))
        );

        let ping = engine
            .function("scalar.orl", "ping")
            .expect("void function");
        assert_eq!(engine.call(&ping, &[]).expect("void call"), None);
    }

    #[test]
    fn hosted_scalar_traps_return_structured_errors_and_keep_session_alive() {
        let mut engine = OriEngine::new(OriConfig::default());
        engine
            .compile_source(
                "traps.orl",
                "module app.traps\n\npublic divide(value: int) -> int\n    return 100 / value\nend\n\npublic check_positive(value: int) -> int\n    check value > 0, \"value must be positive\"\n    return value\nend\n\npublic list_index(value: int) -> int\n    const values: list[int] = [10]\n    return values[value]\nend\n",
            )
            .expect("compile trap functions");

        let divide = engine
            .function("traps.orl", "divide")
            .expect("divide function");
        assert_eq!(
            engine
                .call(&divide, &[OriValue::Int(4)])
                .expect("successful division"),
            Some(OriValue::Int(25))
        );
        assert!(matches!(
            engine.call(&divide, &[OriValue::Int(0)]),
            Err(OriEmbedError::FunctionTrap(OriExecutionError {
                code: 5,
                message
            })) if message.contains("division")
        ));

        let check = engine
            .function("traps.orl", "check_positive")
            .expect("check function");
        assert!(matches!(
            engine.call(&check, &[OriValue::Int(0)]),
            Err(OriEmbedError::FunctionTrap(OriExecutionError {
                code: 1,
                message
            })) if message.contains("value must be positive")
        ));
        let list_index = engine
            .function("traps.orl", "list_index")
            .expect("list index function");
        assert!(matches!(
            engine.call(&list_index, &[OriValue::Int(5)]),
            Err(OriEmbedError::FunctionTrap(OriExecutionError {
                code: 8,
                message
            })) if message.contains("list index")
        ));
        assert_eq!(
            engine
                .call(&divide, &[OriValue::Int(5)])
                .expect("session remains usable after a trap"),
            Some(OriValue::Int(20))
        );
    }

    #[test]
    fn function_handles_cannot_cross_sessions() {
        let source = "module app.player\n\npublic answer() -> int\n    return 42\nend\n";
        let mut first = OriEngine::new(OriConfig::default());
        first
            .compile_source("player.orl", source)
            .expect("first compile");
        let handle = first
            .function("player.orl", "answer")
            .expect("first handle");

        let mut second = OriEngine::new(OriConfig::default());
        second
            .compile_source("player.orl", source)
            .expect("second compile");
        assert!(matches!(
            second.call(&handle, &[]),
            Err(OriEmbedError::WrongSession)
        ));
    }

    #[test]
    fn unsupported_hosted_signatures_are_not_resolved_to_handles() {
        let mut engine = OriEngine::new(OriConfig::default());
        engine
            .compile_source(
                "mixed.orl",
                "module app.mixed\n\npublic mixed(left: int, right: float) -> int\n    return left\nend\n",
            )
            .expect("mixed signature compiles as a module");
        assert!(matches!(
            engine.function("mixed.orl", "mixed"),
            Err(OriEmbedError::UnsupportedFunctionSignature { .. })
        ));
    }

    #[test]
    fn host_registry_resolves_extern_host_without_per_call_lookup() {
        let mut registry = OriHostRegistry::default();
        unsafe {
            registry
                .register_function(
                    "app.hosted.host_add",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int, OriScalarType::Int],
                        return_type: Some(OriScalarType::Int),
                    },
                    host_add as *const () as usize as *const u8,
                )
                .expect("register host function");
        }
        let mut engine = OriEngine::with_host_registry(OriConfig::default(), registry);
        let result = engine
            .compile_source(
                "hosted.orl",
                "module app.hosted\n\nextern host\n    host_add(left: int, right: int) -> int\nend\n\npublic invoke(value: int) -> int\n    return host_add(value, 4)\nend\n",
            )
            .expect("compile host import");
        assert!(result.accepted);
        let handle = engine
            .function("hosted.orl", "invoke")
            .expect("public wrapper");
        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(8)])
                .expect("call host-backed function"),
            Some(OriValue::Int(12))
        );
    }

    #[test]
    fn integer_callback_forwards_user_data_through_cached_jit_dispatch() {
        let bias = Box::new(5_i64);
        let mut engine = OriEngine::new(OriConfig::default());
        unsafe {
            engine
                .host_registry_mut()
                .register_int_callback(
                    "app.callback.add_bias",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int],
                        return_type: Some(OriScalarType::Int),
                    },
                    (&*bias as *const i64).cast_mut().cast(),
                    callback_add_user_data,
                )
                .expect("register callback");
        }
        engine
            .compile_source(
                "callback.orl",
                "module app.callback\n\nextern host\n    add_bias(value: int) -> int\nend\n\npublic invoke(value: int) -> int\n    return add_bias(value)\nend\n",
            )
            .expect("compile callback import");
        let handle = engine
            .function("callback.orl", "invoke")
            .expect("resolve callback wrapper");
        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(7)])
                .expect("call callback wrapper"),
            Some(OriValue::Int(12))
        );
    }

    #[test]
    fn integer_callback_supports_void_imports() {
        let mut recorded = 0_i64;
        let mut engine = OriEngine::new(OriConfig::default());
        unsafe {
            engine
                .host_registry_mut()
                .register_int_callback(
                    "app.callback.record",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int],
                        return_type: None,
                    },
                    (&mut recorded as *mut i64).cast(),
                    callback_record_value,
                )
                .expect("register void callback");
        }
        engine
            .compile_source(
                "callback.orl",
                "module app.callback\n\nextern host\n    record(value: int) -> void\nend\n\npublic invoke(value: int) -> void\n    record(value)\nend\n",
            )
            .expect("compile void callback import");
        let handle = engine
            .function("callback.orl", "invoke")
            .expect("resolve void callback wrapper");
        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(9)])
                .expect("call void callback"),
            None
        );
        assert_eq!(recorded, 9);
    }

    #[test]
    fn callback_unregister_cancels_future_calls_without_invalidating_jit_code() {
        let mut engine = OriEngine::new(OriConfig::default());
        unsafe {
            engine
                .host_registry_mut()
                .register_int_callback(
                    "app.callback.cancelled",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int],
                        return_type: Some(OriScalarType::Int),
                    },
                    std::ptr::null_mut(),
                    callback_add_user_data,
                )
                .expect("register callback");
        }
        engine
            .compile_source(
                "callback.orl",
                "module app.callback\n\nextern host\n    cancelled(value: int) -> int\nend\n\npublic invoke(value: int) -> int\n    return cancelled(value)\nend\n",
            )
            .expect("compile callback import");
        let handle = engine
            .function("callback.orl", "invoke")
            .expect("resolve callback wrapper");
        engine
            .host_registry_mut()
            .remove_callback("app.callback.cancelled")
            .expect("unregister callback");
        assert!(matches!(
            engine.call(&handle, &[OriValue::Int(7)]),
            Err(OriEmbedError::FunctionTrap(OriExecutionError {
                code: ORI_HOST_CALLBACK_CANCELLED,
                ..
            }))
        ));
    }

    #[test]
    fn callback_can_reenter_the_same_ori_session() {
        let mut engine = OriEngine::new(OriConfig::default());
        let mut context = Box::new(ReentrantContext {
            engine: std::ptr::null(),
            nested: None,
        });
        context.engine = &engine;
        unsafe {
            engine
                .host_registry_mut()
                .register_int_callback(
                    "app.callback.reenter",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int],
                        return_type: Some(OriScalarType::Int),
                    },
                    (&mut *context as *mut ReentrantContext).cast(),
                    callback_reenters_ori,
                )
                .expect("register callback");
        }
        engine
            .compile_source(
                "callback.orl",
                "module app.callback\n\nextern host\n    reenter(value: int) -> int\nend\n\npublic double(value: int) -> int\n    return value * 2\nend\n\npublic invoke(value: int) -> int\n    return reenter(value) + 1\nend\n",
            )
            .expect("compile callback import");
        context.nested = Some(
            engine
                .function("callback.orl", "double")
                .expect("resolve nested function"),
        );
        let handle = engine
            .function("callback.orl", "invoke")
            .expect("resolve callback wrapper");
        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(3)])
                .expect("call reentrant callback"),
            Some(OriValue::Int(7))
        );
    }

    #[test]
    fn active_callback_blocks_unregister_until_the_call_returns() {
        let mut engine = OriEngine::new(OriConfig::default());
        let mut context = Box::new(CallbackLifecycleContext {
            registry: std::ptr::null_mut(),
            unregister_blocked: false,
        });
        context.registry = engine.host_registry_mut();
        unsafe {
            engine
                .host_registry_mut()
                .register_int_callback(
                    "app.callback.active",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int],
                        return_type: Some(OriScalarType::Int),
                    },
                    (&mut *context as *mut CallbackLifecycleContext).cast(),
                    callback_attempts_unregister,
                )
                .expect("register callback");
        }
        engine
            .compile_source(
                "callback.orl",
                "module app.callback\n\nextern host\n    active(value: int) -> int\nend\n\npublic invoke(value: int) -> int\n    return active(value)\nend\n",
            )
            .expect("compile callback import");
        let handle = engine
            .function("callback.orl", "invoke")
            .expect("resolve callback wrapper");
        assert_eq!(
            engine
                .call(&handle, &[OriValue::Int(7)])
                .expect("call active callback"),
            Some(OriValue::Int(7))
        );
        assert!(context.unregister_blocked);
        engine
            .host_registry_mut()
            .remove_callback("app.callback.active")
            .expect("unregister after callback returns");
    }

    #[test]
    fn recursive_callback_is_stopped_at_the_defined_depth_limit() {
        let mut engine = OriEngine::new(OriConfig::default());
        let mut context = Box::new(RecursiveCallbackContext {
            id: OriCallbackId(0),
        });
        let id = unsafe {
            engine
                .host_registry_mut()
                .register_int_callback(
                    "app.callback.recursive",
                    OriFunctionSignature {
                        params: vec![OriScalarType::Int],
                        return_type: Some(OriScalarType::Int),
                    },
                    (&mut *context as *mut RecursiveCallbackContext).cast(),
                    callback_recurses,
                )
                .expect("register callback")
        };
        context.id = id;
        engine
            .compile_source(
                "callback.orl",
                "module app.callback\n\nextern host\n    recursive(value: int) -> int\nend\n\npublic invoke(value: int) -> int\n    return recursive(value)\nend\n",
            )
            .expect("compile callback import");
        let handle = engine
            .function("callback.orl", "invoke")
            .expect("resolve callback wrapper");
        assert!(matches!(
            engine.call(&handle, &[OriValue::Int(1)]),
            Err(OriEmbedError::FunctionTrap(OriExecutionError {
                code: ORI_HOST_CALLBACK_REENTRANCY_LIMIT,
                ..
            }))
        ));
    }

    #[test]
    fn missing_host_function_fails_before_publishing_a_generation() {
        let mut engine = OriEngine::new(OriConfig::default());
        let error = engine
            .compile_source(
                "hosted.orl",
                "module app.hosted\n\nextern host\n    missing(value: int) -> int\nend\n\npublic invoke(value: int) -> int\n    return missing(value)\nend\n",
            )
            .expect_err("missing host import must fail");
        assert!(matches!(error, OriEmbedError::MissingHostFunction(name) if name == "missing"));
        assert_eq!(engine.module("hosted.orl"), None);
    }

    #[test]
    fn unload_all_releases_generations_and_keeps_the_session_usable() {
        let mut engine = OriEngine::new(OriConfig::default());
        let source = "module app.unload\n\npublic answer() -> int\n    return 42\nend\n";
        engine
            .compile_source("a.orl", source)
            .expect("compile module a");
        engine
            .compile_source("b.orl", source)
            .expect("compile module b");
        let handle = engine.function("a.orl", "answer").expect("function handle");
        engine
            .compile_source("a.orl", source)
            .expect("second generation for module a");
        assert_eq!(engine.modules().count(), 2);

        let unloaded = engine.unload_all();
        assert_eq!(unloaded.modules, 2);
        assert_eq!(unloaded.retired_generations, 1);
        assert_eq!(engine.modules().count(), 0);
        assert!(matches!(
            engine.call(&handle, &[]),
            Err(OriEmbedError::ModuleNotCompiled(_))
        ));

        engine
            .compile_source("fresh.orl", source)
            .expect("session remains usable after unload_all");
        let fresh = engine.function("fresh.orl", "answer").expect("fresh handle");
        assert_eq!(
            engine.call(&fresh, &[]).expect("fresh call"),
            Some(OriValue::Int(42))
        );
    }

    #[test]
    fn module_source_records_accepted_checks_and_compiles() {
        let mut engine = OriEngine::new(OriConfig::default());
        let first = "module app.src\n\npublic answer() -> int\n    return 1\nend\n";
        let second = "module app.src\n\npublic answer() -> int\n    return 2\nend\n";
        engine
            .check_source("src.orl", first)
            .expect("accepted check");
        assert_eq!(engine.module_source("src.orl"), Some(first));
        assert!(engine.functions("src.orl").expect("functions").is_empty());

        engine
            .compile_source("src.orl", second)
            .expect("accepted compile");
        assert_eq!(engine.module_source("src.orl"), Some(second));
        assert_eq!(engine.functions("src.orl").expect("functions").len(), 1);

        engine
            .check_source(
                "src.orl",
                "module app.src\n\npublic answer() -> Missing\nend\n",
            )
            .expect("rejected check");
        assert_eq!(engine.module_source("src.orl"), Some(second));
        assert_eq!(engine.functions("src.orl").expect("functions").len(), 1);
    }

    #[test]
    fn unloading_a_module_invalidates_handles_and_releases_generations() {
        let mut engine = OriEngine::new(OriConfig::default());
        let source = "module app.unload\n\npublic answer() -> int\n    return 42\nend\n";
        engine
            .compile_source("unload.orl", source)
            .expect("first compile");
        let handle = engine
            .function("unload.orl", "answer")
            .expect("function handle");
        engine
            .compile_source("unload.orl", source)
            .expect("second generation");

        let unloaded = engine.unload_module("unload.orl").expect("unload module");
        assert_eq!(unloaded.module.generation, Some(ModuleGeneration(2)));
        assert_eq!(unloaded.retired_generations, 1);
        assert_eq!(engine.module("unload.orl"), None);
        assert!(matches!(
            engine.call(&handle, &[]),
            Err(OriEmbedError::ModuleNotCompiled(_))
        ));
    }
}
