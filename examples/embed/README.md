# Embed Ori as a shared library (`ori compile --lib`)

Hosts (Godot GDExtension, Python, C engines) can `dlopen` an Ori **cdylib**
and call functions marked `@c_export`.

## Language

```orl
module app.embed_add

@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end

@c_export("mul_scores")
public mul(a: int, b: int) -> int
    return a * b
end
```

- Only `public` free functions.
- Accepted values: numeric scalars, `bool`, `string`, and non-empty,
  non-generic structs.
- Scalar structs use `const OriType *` parameters and a final `OriType *out`
  for returns.
- Structs with managed fields use opaque `OriTypeHandle *` values. Parameters
  are borrowed; returned handles are owned and must be passed to
  `ori_arc_release`.
- Optional rename: `@c_export("symbol_name")`.

## Compile

```bash
# Needs a staged runtime with cdylib (libori_runtime.so):
#   sh tools/stage_native_runtime.sh --profile release
export ORI_USE_SYSTEM_LINKER=1   # recommended for --lib on Linux
ori compile --lib examples/embed/add_scores.orl -o libadd_scores.so
```

The command writes both `libadd_scores.so` and the canonical
`libadd_scores.h`. Include that generated header instead of maintaining C
declarations by hand.

The library **dynamically** depends on `libori_runtime.so` (same triple under
`runtime/<triple>/`). Keep that directory on `LD_LIBRARY_PATH` / `rpath` /
next to the host binary.

## Host contract

```c
#include "libadd_scores.h"

void *h = dlopen("libadd_scores.so", RTLD_NOW);
int  (*runtime_init)(void) = dlsym(h, "ori_rt_init");
void (*runtime_shutdown)(void) = dlsym(h, "ori_rt_shutdown");
void (*module_init)(void) = dlsym(h, "__ori_module_init"); // optional globals
int64_t (*add_scores_fn)(int64_t, int64_t) = dlsym(h, "add_scores");

runtime_init();
if (module_init) module_init();
printf("%lld\n", (long long)add_scores_fn(2, 3)); // 5
runtime_shutdown();
```

## Smoke test

```bash
sh tools/qa/embed_smoke.sh
```

## Phases (see `docs/archive/plans/PLANO-CDYLIB-EMBED.md`)

| Phase | Status |
|-------|--------|
| P1 `--lib` + `@c_export` scalars + `ori_rt_*` | **done** |
| P2 strings + scalar/managed structs + generated header | **done** |
| P3 host→Ori callbacks | planned |
| P4 Godot GDExtension example | planned |
| P5 Windows / macOS | planned |
