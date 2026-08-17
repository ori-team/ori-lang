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
use std::ffi::CStr;
use std::path::Path;
use std::{collections::HashMap, mem};

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

struct RuntimeErrorApi {
    _library: Library,
    clear: HostClearError,
    code: HostErrorCode,
    message: HostErrorMessage,
}

impl RuntimeErrorApi {
    fn load(path: &Path) -> Result<Self, String> {
        // SAFETY: `path` is resolved by the compiler as the runtime cdylib.
        // The library handle is retained for as long as the function pointers
        // are used by the compiled module.
        let library = unsafe { Library::new(path) }
            .map_err(|error| format!("load runtime error API `{}`: {error}", path.display()))?;
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
        Ok(Self {
            _library: library,
            clear,
            code,
            message,
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
    runtime_error_api: RuntimeErrorApi,
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
        let lookup = runtime_symbol_lookup(cdylib_path, native_libs_paths, host_symbols)?;
        let runtime_error_api = RuntimeErrorApi::load(cdylib_path)?;
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
            runtime_error_api,
        })
    }

    pub fn functions(&self) -> impl Iterator<Item = &JitFunctionInfo> {
        self.functions.values().map(|function| &function.info)
    }

    pub fn function(&self, name: &str) -> Option<&JitFunctionInfo> {
        self.functions.get(name).map(|function| &function.info)
    }

    /// Invoke a scalar function while this module remains alive.
    pub fn call(&self, name: &str, args: &[JitValue]) -> Result<Option<JitValue>, JitCallError> {
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
    let mut libraries = Vec::with_capacity(1 + native_libs_paths.len());
    // SAFETY: the compiler resolved this path as a runtime artifact and the
    // library handle is retained by the lookup closure for the JIT lifetime.
    let runtime_lib = unsafe { Library::new(cdylib_path) }
        .map_err(|e| format!("load runtime cdylib `{}`: {e}", cdylib_path.display()))?;
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
}
