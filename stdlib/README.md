# Ori Standard Library

> Surface: **S3** + inference B + **`ok`/`err`**.  
> **Canonical public API:** only **`ori.X`** (`stdlib/X.orl`).  
> **STDLIB-1:** nested `ori.X.utils` / `ori.X.algorithms` are **deprecated as public
> surface** — still compile as **silent compat** (do not teach in new docs/examples).

Policy: [`docs/planning/stdlib-merge-policy.md`](../docs/planning/stdlib-merge-policy.md).

---

## Three layers

| Layer | Role | Location |
|-------|------|----------|
| **1** | Hot path FFI / ARC / collections primitives | Rust `stdlib.rs` + `ori-runtime` |
| **2** | Ergonomic wrappers | Prefer `stdlib/X.orl` → `module ori.X` |
| **3** | Algorithms | Same `ori.X` parent; nested `X/algorithms.orl` is compat only |

**STDLIB-5 (mass L1 → pure `.orl`):** **closed as wontfix.** Layer 1 Rust is
permanent product design (ARC, executor, FS/net hot path). Ports only when they
improve maintainability without losing performance contracts — not a checklist.

**STDLIB-4 / STDLIB-4b / STDLIB-4k async I/O (closed):**

| API | Model |
|-----|--------|
| `fs.read_text_async` / `write_text_async` | L1 future + worker (awaitable) |
| `net.connect_async` / `connect_tls_async` | L1 future + worker (awaitable) |
| `net.accept_async` / `read_some_async` / `write_all_async` / `udp_*_async` | L1 future + **poll reactor** (STDLIB-4k) |
| `*_in_background` / `task.run_blocking` | Job offload (still valid) |

---

## Import (canonical)

```ori
import ori.io = io
import ori.fs = fs
import ori.string = str
import ori.queue = queue
import ori.bytes = bytes_mod

import ori.string (is_empty)
import ori.fs (read_text_or)
import ori.bytes (compare_lex)
const q = queue.from_list(["a", "b"])
```

### Compatibility (deprecated public paths — still supported)

```ori
-- Still type-checks; prefer the parent form above.
import ori.fs.utils = fu
import ori.queue.utils = qu
import ori.bytes.algorithms = ba
```

---

## Layout on disk

| Prefer | When |
|--------|------|
| `stdlib/X.orl` | Default — full public helpers for domain `X` |
| `stdlib/X/utils.orl` | Silent compat (same helpers; do not add **new** APIs only here) |
| `stdlib/X/algorithms.orl` | Silent compat for former Layer-3 nests |
| `stdlib/math/vec2.orl` etc. | Multi-file domain assets (`vec2`, not “utils”) |

Every domain with helpers has a **parent** so `import ori.X` is enough.

---

## Notes

- Pure L1 symbols (e.g. `ori.fs.create_dir_all`, `ori.os.pid`) are already on
  `import ori.X` via the runtime manifest — **do not** redeclare them in
  `X.orl` (same-name shadowing breaks call arity and generic monomorphization).
- **Never** add new public APIs only under `ori.*.utils` or `ori.*.algorithms`.
- **Domain type aliases (S3 1.3):** parents export `public alias` where returns
  repeat (`ori.fs.TextResult`, `ori.net.ConnectionResult`, `ori.io.WriteResult`, …).
  Prefer: `import ori.fs (TextResult, …)`.

---

## Adding a function

### Layer 1
Manifest + runtime + tests (see AGENTS.md).

### Layer 2/3
1. Add `public` to `stdlib/X.orl` (`module ori.X`) — **required**.
2. Optionally mirror in `X/utils.orl` / `X/algorithms.orl` for old import paths (optional).
3. Update `.oridoc` sidecar when user-facing.
