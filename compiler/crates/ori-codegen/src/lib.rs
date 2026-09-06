// ori-codegen  real lib.rs, implementation provided

pub mod c_header;
pub mod debug_symbols;
pub mod native_backend;

pub use c_header::generate_c_header;
pub use debug_symbols::{emit_native_debug_symbols, DebugFunction, DebugVariable};
pub use native_backend::{
    emit_native, emit_native_with_options, has_runtime_global_initializers,
    jit::{
        run_jit, run_jit_with_args, CompiledJitModule, JitCallError, JitFunctionInfo,
        JitFunctionSignature, JitHostSymbol, JitManagedValueOwner, JitScalarType, JitValue,
    },
    link, link_many_with_options, link_with_options, native_func_symbol,
    native_func_wrapper_symbol, native_global_symbol, NativeEmitOptions, NativeLinkOptions,
    NativeLinker,
};
