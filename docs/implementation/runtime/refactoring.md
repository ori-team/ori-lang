# Runtime refactoring guide

The current runtime contains many native domains and exported ABI functions. Refactoring is recommended, but it must preserve every public symbol, layout, ownership rule, and observable behavior.

## Objective

Move from a large implementation file toward cohesive domain modules with thin FFI adapters and safe internal logic.

This is a structural program. It must not introduce unrelated language or stdlib behavior.

## Target module boundaries

```text
ori-runtime/src/
├── lib.rs                 module declarations, ABI version, controlled re-exports
├── abi/                   shared layout and conversion primitives
├── ffi/                   raw pointer/tag/length validation and exported adapters
├── arc/                   allocation registry, retain/release, edges, cycle collection
├── values/                optional/result/tuple/managed wrapper construction
├── strings/               text primitives and representation helpers
├── bytes/                 length-aware binary operations
├── collections/           list/map/set/deque/queue/stack/tree/graph/heap families
├── io/                    stream abstractions
├── fs/                    filesystem handles and operations
├── net/                   TCP, TLS, UDP, listeners, reactor integration
├── process/               arguments, environment, subprocess support
├── async_runtime/         futures, executor, cancellation, readiness
├── concurrency/           channels, atomics, transfer-safe primitives
├── debugger/              debug agent and runtime instrumentation
├── test_support/          runtime-facing Ori test hooks
└── platform/              isolated OS/architecture integration
```

The final names may differ, but each module must own a real domain and avoid circular ownership.

## Non-goals

- changing `ori-native-abi-1`;
- renaming exported `ori_*` symbols;
- altering collection or string semantics;
- changing ARC policy;
- adding COW, arenas, or new memory models;
- rewriting the runtime in Ori;
- optimizing without measurement;
- hiding all low-level behavior behind code generation.

## Preconditions

Before each extraction:

- identify exported symbols and internal callers;
- document pointer, ownership, and layout invariants;
- add characterization tests for behavior and failure paths;
- record a performance baseline for hot paths;
- confirm AOT/JIT and static/cdylib evidence;
- identify global-state and lock interactions.

## Extraction sequence

### 1. Pure helpers and types

Move functions with no exported symbol and minimal global-state coupling.

Examples:

- bounds and checked-size helpers;
- typed result conversion;
- string/bytes utility functions;
- platform artifact naming;
- internal data structures.

### 2. Safe domain logic

Extract algorithms that can operate on typed references and Rust results.

Keep raw pointer validation at the existing boundary until the safe API is proven.

### 3. Thin FFI adapters

Move or rewrite exported functions to:

```text
validate raw input
-> convert to typed domain input
-> call safe logic
-> encode ABI result
```

The native symbol remains unchanged even when the Rust module path changes.

### 4. Domain global state

Only after behavior is isolated, move related global state and initialization into a focused module.

Document lock ordering, callback behavior, reset/testing, and shutdown.

### 5. Controlled re-exports

`lib.rs` should expose only symbols and internal contracts needed by other runtime modules or tests. Avoid `pub` as a migration shortcut.

## Recommended PR slices

1. ABI/value construction helpers.
2. String and bytes domains.
3. Filesystem and I/O handles.
4. Collection families in small groups.
5. Process/network domains.
6. Futures/executor/reactor.
7. ARC registry and collector only after all dependants and tests are clear.
8. Debug/test support.
9. Final `lib.rs` façade cleanup and allowed-dependency check.

ARC should not be the first extraction simply because it appears near the top of the file. It is the most cross-cutting and should move after boundaries are better understood.

## ABI preservation

For every PR:

- list affected exported symbols;
- compare names and signatures;
- validate `ORI_ABI_VERSION` remains unchanged;
- preserve `repr(C)` layouts and discriminants;
- preserve ownership on input and output;
- validate generated headers where relevant;
- rebuild staticlib and cdylib;
- verify runtime-link metadata;
- run embedding tests if affected.

A Rust-internal move does not justify an ABI bump. Any actual incompatible behavior must stop the refactor and enter the decision process.

## Ownership review

Each function should document:

- managed versus unmanaged input;
- borrowed versus owned references;
- whether it retains;
- whether it consumes or releases;
- ownership of returned payloads;
- registered edge behavior;
- deterministic resource closure;
- destructor responsibilities.

Use the single-cascade-owner ADR and runtime invariants.

## Global state

Potential state families include:

- ARC registry and suspect buffer;
- executor and task queues;
- I/O reactor;
- debugger state;
- process arguments;
- test/leak counters;
- caches and platform initialization.

Do not move state without documenting:

- initialization;
- synchronization;
- lock order;
- callback/reentrancy;
- process lifetime;
- test isolation.

## Error handling

Refactoring should improve the internal error boundary without changing the public ABI:

- domain functions return typed Rust errors;
- FFI adapters encode Ori `result` or documented sentinel values;
- invalid raw input cannot cause undefined behavior;
- panics do not cross FFI;
- abort remains limited to documented traps/corruption.

## Tests per extraction

Minimum:

- focused domain unit tests;
- exported-symbol behavior test;
- ownership/leak regression where managed values are involved;
- AOT integration;
- JIT integration;
- staticlib/cdylib symbol or load test;
- full runtime and workspace tests;
- target-specific CI when platform code moves.

## Performance

Measure affected hot paths before and after:

- allocation/free;
- retain/release;
- edge registration;
- string/bytes operations;
- collection mutation;
- executor scheduling;
- reactor wakeups;
- network/I/O throughput where relevant.

A module split should be performance-neutral. Unexpected changes require investigation rather than assumption that the compiler will inline everything.

## Completion criteria

The program is complete when:

- `lib.rs` is a focused runtime façade rather than the implementation of every domain;
- raw pointer handling is concentrated at reviewed boundaries;
- safe domain logic is independently testable;
- module dependencies are documented and acyclic;
- all exported symbols, layouts, and semantics remain compatible;
- AOT/JIT/package gates pass;
- hot-path performance has not regressed;
- architecture, unsafe policy, and crate map reflect the final structure.