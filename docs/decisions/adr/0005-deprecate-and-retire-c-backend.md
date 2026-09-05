---
id: ADR-0005
title: Deprecate and retire the C backend; reference pipeline becomes Rust-native, then self-hosted
status: proposed
date: 2026-09-05
deciders: []
supersedes: []
superseded_by: []
related_docs:
  - docs/spec/14-backend-support.md
  - docs/product/requirements.md
related_code:
  - compiler/crates/ori-codegen/src/c_backend.rs
  - compiler/crates/ori-codegen/src/c_header.rs
  - compiler/crates/ori-codegen/src/c_casefold.rs
  - compiler/crates/ori-driver/src/main.rs
  - compiler/crates/ori-types/src/stdlib.rs
---

# ADR-0005: Deprecate and retire the C backend; reference pipeline becomes Rust-native, then self-hosted

## Context

Ori 0.3.8 has two code generation routes: native Cranelift (AOT object plus JIT)
and Cebug (`ori emit c`, `compiler/crates/ori-codegen/src/c_backend.rs`,
`c_header.rs`, `c_casefold.rs`, ~8.4k lines).

Cebug is intentionally partial: `docs/spec/14-backend-support.md` excludes
`async`/`await`/`task`/`channel`/`atomic`, `net.*` (TCP/TLS/UDP),
`json.parse`/`json.Value` lowering, and `core.Destructor`. The native backend is
the semantic reference in every parity dispute.

Since the direction is Rust reference implementation and then self-hosted Ori
compiler, the C route becomes a maintenance line item without a semantic role:
every frontend change needs a second lowering, a second runtime header
(`ORI_RUNTIME_H`), and a shadow matrix (`c_backend` flag in
`compiler/crates/ori-types/src/stdlib.rs`, 36 `build_c_backend` tests).

## Decision drivers

- No language promise depends on C emission.
- Every C execution path can be served by native AOT plus JIT.
- Frontend changes pay double: HIR lowering plus C lowering plus inline C runtime.
- CI flake surface is dominated by C link and inline-runtime drift.
- Self-host needs one reference target, not two.

## Considered options

### Option A — Keep C indefinitely as debug route

Pros: readable C dump, gcc sanitizer path, second vote in differentials.
Cons: permanent double maintenance, shadow stdlib matrix, blocks self-host focus.

### Option B — Retire C after deprecation window (selected)

Pros: single reference pipeline, `stdlib!` flag removed, 36 C execution tests
migrate to native differential tests, CI shrinks.
Cons: loses human-readable C dump and gcc sanitizer route; needs replacements
below.

### Option C — Extract C to `ori-cdebug` crate now

Pros: unblocks native CI fast without deciding retirement.
Cons: preserves the maintenance cost behind a feature flag.

## Decision

Deprecate the C backend in 0.3.x, retire it before the self-host milestone,
following the staged projection below.

### Phase 0 — Freeze (0.3.8)

- No new language feature gets C lowering.
- New `stdlib!` runtime functions default to `c_backend_runtime: false`.
- New async, net, json, and destructor work is native-only by policy.
- Document the freeze in `docs/spec/14-backend-support.md`.

### Phase 1 — Deprecate (0.3.9)

- `ori emit c` prints a deprecation warning pointing at the native route.
- CI: C execution tests move from required to advisory (`allow-failure`).
- Stop extending `c_header.rs`; additions are stubs that emit
  `backend.c_unsupported` with a native-route hint.
- Publish the removal checklist: files, CLI surface, `stdlib!` flag,
  test migration map, spec rewrite.

### Phase 2 — Retire (0.4.0)

Delete in one vertical PR:

- `compiler/crates/ori-codegen/src/c_backend.rs`
- `compiler/crates/ori-codegen/src/c_header.rs`
- `compiler/crates/ori-codegen/src/c_casefold.rs`
- `emit_c` from `compiler/crates/ori-codegen/src/lib.rs`
- `EmitAction::C` from `compiler/crates/ori-driver/src/main.rs`
- `run_emit_c` / `c_source` from `compiler/crates/ori-driver/src/pipeline/compile.rs`
- `c_backend_runtime` flag from `compiler/crates/ori-types/src/stdlib.rs`
- 36 `build_c_backend` / `c_backend` execution tests, migrated to
  `differential_testing.rs` (AOT identical JIT) and sanitizer smoke on native
- Rewrite `docs/spec/14-backend-support.md` as native-plus-JIT matrix

No `cranelift-*` dependency is removed: the reference stays Cranelift AOT plus
Cranelift JIT until self-host.

### Phase 3 — Rust-native reference (0.4.x)

- Single reference: Cranelift AOT plus Cranelift JIT, shared HIR input.
- Differential battery becomes AOT identical JIT identical aggressive-JIT.
- Readable-IR role moves to HIR pretty-print plus Cranelift IR dump, not C.
- Sanitizer role moves to native builds under ASan/UBSan/Valgrind smoke
  (`tools/qa/sanitizer_smoke.sh` retargeted).
- Portable-target role (platforms without Cranelift) is explicitly deferred
  until after self-host; no replacement backend is promised in 0.4.x.

### Phase 4 — Self-hosted backend (post-0.4)

- The Ori compiler, written in Ori, emits the reference object format directly.
- The Rust compiler becomes the bootstrap, kept only for building the
  bootstrap binary.
- Backend acceptance stays behavior-based: the existing native test batteries
  (`differential_testing`, `feature_interaction_matrix`, `concurrency_stress`,
  `simd_edge_cases`, `incremental_invalidation`) must pass unchanged against
  binaries produced by the self-hosted compiler.
- No C backend is reintroduced as part of self-host.

## Consequences

### Positive

- One lowering to maintain; frontend velocity increases.
- `stdlib!` manifest loses its shadow flag.
- CI required path shrinks to native plus JIT.
- Self-host has a single acceptance target.

### Negative

- Loss of `ori emit c` readable output; replaced by HIR plus Cranelift IR dumps.
- Loss of gcc-based sanitizer execution; replaced by native sanitizer smoke.
- Loss of independent C vote in differentials; replaced by AOT/JIT/aggressive triple.

### Neutral or follow-up

- Update `docs/spec/14-backend-support.md`, `docs/product/requirements.md`,
  `CHANGELOG.md`, and `tests/README.md` in the Phase 2 PR.
- Decide explicitly whether any future portable backend (LLVM, direct object,
  WASM) is needed after self-host; default answer is no until a target demands it.

## Invariants established

- The native backend is the sole semantic reference from Phase 1 onward.
- No new feature ships with C lowering after 0.3.8.
- Removal is one vertical PR, not a slow bit-rot.
- Self-host acceptance is behavioral (existing batteries pass), not textual
  (no IR-to-IR comparison against the Rust compiler).

## Affected contracts and components

- `compiler/crates/ori-codegen`: delete C route, keep `native_backend.rs`.
- `compiler/crates/ori-driver`: remove `EmitAction::C`, `run_emit_c`.
- `compiler/crates/ori-types`: remove `c_backend_runtime` flag.
- `docs/spec/14-backend-support.md`: native-plus-JIT matrix.
- `tools/qa/`: retarget sanitizer and differential harnesses to native.
- `CHANGELOG.md`: deprecation notice (0.3.9), removal notice (0.4.0).
