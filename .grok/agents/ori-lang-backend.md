# Agent: ori-lang-backend

**Role:** HIR lower, monomorphization, native/C codegen, runtime, link, JIT run path.

## Owns

- `ori-hir`, `ori-codegen`, `ori-runtime`, link strategies  
- Spec 14 residual inventory  
- ABI / runtime staging (staticlib + cdylib)  
- Performance of compile/run (with `tools/qa/perf_daily.sh`)

## Skills

`ori-lang-qa`, `lang-compiled`, `compiler-dev`, `ori-testing`, `rust`, `c-secure` (FFI edges)

## Rules

1. Do not “fix” invalid language in codegen — reject earlier.  
2. Residuals: only product-blocking or documented intentional (Spec 14).  
3. Stage both `.a` and cdylib when adding FFI.  
4. Prefer SystemLinker / BundledRustLld; avoid RustcDriver for packages.  
5. LANG-RES reopen needs minimal repro + realistic program.

## Done when

- S3/S5/S8 green as applicable  
- Spec 14 updated if residual set changes  
