# Architectural Roadmap: Code Audit, Memory Safety & Performance Optimization

> **Status:** Active Roadmap & Implementation Plan  
> **Date:** 2026-08-17  
> **Baseline:** Ori S3 (`0.3.0`) + Nim-local inference B (`0.3.1`) + Cargo workspace `0.3.8-dev` (Released `v0.3.7`)  
> **Index:** [`README.md`](README.md) · Backlog: [`BACKLOG.md`](BACKLOG.md)

---

## 1. Executive Summary & Codebase Audit Baseline

Following the full implementation of Priority 1, 2, and 3 backlog items (graphics evolution, stdlib extensions, hosted runtime embedding, native bindgen, web primitives, and package ecosystem), an exhaustive code audit of the entire repository was performed across frontend, backend, runtime, and tooling crates.

### 1.1 Key Verification Metrics
- **Strict Clippy Compliance:** `cargo clippy --workspace --all-targets -- -D warnings` passes with **0 errors and 0 warnings** across all 10 workspace crates and integration test targets.
- **QA Suite Integrity:** `./tools/qa/daily_fast.sh` passes 100% (244 `ori_spec` tests, 42 memory ARC tests, diagnostic catalog consistency, D0 documentation validation, S1 unit tests, S8 concurrency).
- **Zero-Unsafe Codegen:** The Cranelift native code generator (`ori-codegen`) contains **0 unsafe blocks**, relying exclusively on safe Rust abstractions for IR generation, register allocation, and emission.
- **Memory Leak-Free:** The cycle collector (Bacon & Rajan trial deletion) successfully reclaims cyclic structures; `ori_test_assert_no_leaks` verifies 0 uncollected live allocations across all tests.

---

## 2. Frontend Subsystem Analysis

### 2.1 Lexer (`ori-lexer`) & Parser (`ori-parser`)
- **Structure:** `logos`-based token stream fed into recursive-descent parser with contextual keyword awareness.
- **Resilience:** Syntax recovery boundaries prevent cascading failures; diagnostic sink captures structured errors with source spans and actionable suggestions.
- **Refactoring Applied:**
  - Standardized `.peek_kind().is_some_and(...)` pattern across parser inspection methods.
  - Eliminated redundant `u32` span offset casts in f-string expression interpolation.
  - Consolidated legacy `implement` / `apply` contextual keyword recovery blocks.

### 2.2 Type Inference & Checker (`ori-types`)
- **Architecture:** DefMap multi-file symbol resolution, Nim-local bidirectional type inference (Option B: calls, field access, indexing, pipes), generic trait bounds, and where-clause unification.
- **Identified Bottlenecks & Optimization Headroom:**
  - **`Ty` Cloning Overhead:** Deeply nested types (`Ty::Generic`, `Ty::Func`, `Ty::Map`) clone structures during trait matching and unify cascades.
  - **Type Representation:** Transitioning from heap-allocated recursive `Ty` to a compact type-interner arena (`TyId`) will significantly reduce allocation churn in large-scale projects (100k+ LOC).

### 2.3 HIR Lowering & Mid-End Optimizations (`ori-hir`)
- **Pipeline:** AST lowered to high-level IR with SSA properties, monomorphized generic functions, and AST-to-AST inline generator lowering.
- **Optimization Passes (`ori-hir/src/optimize/`):**
  - **Constant Folding & DCE:** Propagates constant expressions and eliminates dead variable assignments and unreachable blocks.
  - **Loop Strength Reduction:** Automatically rewrites accumulating integer loops into mathematical closed forms ($n(n+1)/2$, $n^2$) guarded by `MAX_CLOSED_FORM_BOUND` to prevent arithmetic wrap discrepancies.
  - **Leaf Function Inlining:** Transparently inlines monomorphic, single-expression pure functions.
  - **Bounds-Check Elimination (BCE):** Eliminates runtime bounds checks for statically-proven constant indices on inline `array[T, N]`.

---

## 3. Backend Subsystem Analysis

### 3.1 Cranelift Native Backend (`ori-codegen`)
- **ABI Compliance:** Adheres to `ori-native-abi-1` and C calling conventions (System V AMD64 / Windows x64).
- **Layout Calculations:** Exact struct field alignment and padding matching target architecture specifications.
- **Aggregate Passing:**
  - Scalar structs: direct registers or stack pointer/out wrappers.
  - Managed structs: opaque ARC handles maintained via the runtime registry.
  - Tagged unions / `result[T, E]`: compact tag + payload expansion with `OriResultTag`.

### 3.2 C Backend & Generated C Headers (`c_header.rs`)
- **Header Generation (`ori compile --lib`):** Emits canonical C headers declaring runtime lifecycle functions (`ori_arc_retain`, `ori_arc_release`), scalar typedefs, opaque struct handles, export function signatures, and C++ `extern "C"` compatibility wrappers.

---

## 4. Native Runtime (`ori-runtime`) & Memory Management

### 4.1 ARC Lifecycle & Cycle Collection
- **Allocation Layout:** Every managed payload is preceded by `OriHeapHeader` containing an `AtomicI64` reference count and optional destructor function pointer.
- **Locking & Deadlock Avoidance:**
  - `ori_arc_release` consolidates allocation registry operations into a single critical section.
  - User destructors (`core.Destructor`) and recursive child releases execute **outside** the `ArcState` mutex lock, guaranteeing zero deadlock risk if a finalizer allocates or drops managed values.
- **Trial Deletion Algorithm:**
  - Suspect objects with non-zero reference counts and registered outgoing edges enter the suspect queue.
  - Cooperative collection performs trial deletion over the reachable candidate subgraph, marking surviving nodes and freeing disconnected garbage cycles in a single sweep.
- **Contiguous Buffers & Spans (`OriBuffer`, `OriSpan`):**
  - `OriBuffer` provides contiguous, cache-friendly heap blocks.
  - `OriSpan` creates zero-copy mutable windows, registering an ARC edge to the backing buffer to ensure buffer liveness for the lifetime of the span.

---

## 5. Tooling, Package Ecosystem & Embedding

### 5.1 Embedded Compiler Service (`ori-embed`)
- **Session Architecture:** Persistent `OriCompilerSession` with generational handles (`SessionHandle`) preventing stale pointer execution.
- **Host Callbacks & Traps:** Registered host functions support user context (`user_data`), explicit unregistration, reentrancy guards, and structured trap handling without process termination.

### 5.2 Package Ecosystem (`ori-driver/src/package.rs`)
- **Supply Chain Integrity:** Lockfile (`ori.lock`) cryptographically and revision-pinned (`--locked` verification).
- **Safe Directory Operations:** Package copying excludes `.git` and `target` directories.

---

## 6. Future Implementation Roadmap Items

Based on the code audit, the following actionable items are established in the active project backlog:

| ID | Title | Priority | Size | Target Phase | Description |
|---|---|---|---|---|---|
| **OPT-TYPE-INTERN-1** | Type Arena Interning (`TyId`) | P2 | L | Next Optimization Wave | Introduce an arena-backed `TypeInterner` replacing deep `Ty` clones with lightweight 32-bit `TyId` handles in `ori-types` and `ori-hir`. |
| **OPT-PAR-TYPECHECK-1** | Parallel Module Type-Checking | P3 | M | Post-0.3.8 | Utilize `rayon` to parallelize function-body type checking across independent namespace modules in multi-file projects. |
| **DX-LINT-EXT-1** | Extended Semantic Linters | P3 | S | Tooling Maintenance | Add additional lint passes in `ori lint` for unhandled error variants, redundant type annotations, and implicit shadow warnings. |
| **RUST-AUDIT-2** | Full-Workspace Clean Clippy Gate | P1 | S | **done** | Enforce zero-warning clippy gate across all workspace crates and test targets (`--all-targets`) in continuous development. |

---

## 7. Migration & Compatibility Guidelines

All recommendations maintain strict compatibility with **FREEZE-1** and **`ori-native-abi-1`**:
1. Internal compiler changes (such as `TyId` interning) do not alter surface syntax or public language semantics.
2. Runtime ABI layouts remain bit-exact with the documented specifications in [`spec/19-abi.md`](../spec/19-abi.md).
3. The normative specification remains the definitive authority over implementation behavior.
