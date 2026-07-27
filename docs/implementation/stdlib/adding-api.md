# Adding a standard-library API

This guide turns the Chapter 15 maintenance contract into a concrete implementation path.

## 1. Start with the public contract

Define:

- canonical `ori.*` path;
- purpose and real use case;
- parameters and return value;
- recoverable failures;
- mutation and aliasing;
- ownership and resource lifecycle;
- complexity and allocation behavior;
- platform differences;
- native, JIT, and C/debug support;
- compatibility or experimental status.

Check whether an existing operation already solves the problem. Do not create a second canonical name for convenience alone.

## 2. Choose the correct layer

### Layer 1 — runtime backed

Use when the operation needs:

- OS or platform APIs;
- native memory/runtime representation;
- ARC or managed collection internals;
- asynchronous executor/reactor integration;
- performance that cannot be achieved acceptably in Ori source;
- an embedding/native ABI primitive.

### Layer 2 — safe Ori wrapper

Use for composition, validation, convenient defaults, and user-facing safe wrappers over Layer 1.

### Layer 3 — pure algorithm

Use for algorithms that can be written clearly in Ori and do not require privileged runtime behavior.

Measure before moving a pure operation into the runtime solely for performance.

## 3. Runtime-backed path

Update the canonical semantic catalog with:

- path;
- aliases;
- semantic signature;
- runtime symbol;
- native ABI signature;
- backend flags;
- module classification.

Implement the runtime symbol as a thin ABI adapter over safe domain logic where practical.

Review:

- nullability and length validation;
- borrowed/owned inputs;
- retained/stored values;
- returned ownership;
- ARC edges;
- cleanup of partial failures;
- thread and lock behavior;
- panic containment;
- target differences.

## 4. Source-module path

Create or extend the canonical `.orl` module.

Rules:

- module declaration matches the path;
- public functions use `public`;
- current S3 syntax only;
- runtime primitives are imported normally;
- internal helpers remain private;
- no hidden dependence on source-tree location outside the module contract;
- code is formatted and checked by the current compiler.

## 5. Documentation

Update:

- Chapter 12 public contract;
- Chapter 15 only when the maintenance mechanism changes;
- `.oridoc` sidecar or source docs;
- `stdlib/README.md` inventory;
- user guide or cookbook for important workflows;
- generated documentation/LSP catalog;
- changelog for user-visible additions or changes.

Every public operation documents failure, ownership, mutation, and platform behavior when relevant.

## 6. Tests

### Required baseline

- type-check acceptance;
- invalid type/argument behavior;
- import/module discovery;
- native AOT execution;
- JIT execution;
- C/debug support or explicit rejection;
- docs/LSP visibility.

### Managed values

Add:

- retain/release or leak evidence;
- inserted/stored ownership cases;
- borrowed and fresh arguments;
- empty and nested values;
- failure cleanup;
- collection aliasing/iteration cases.

### Resources and I/O

Add:

- invalid handle/path/input;
- close/dispose behavior;
- repeated close according to contract;
- partial failure;
- timeout/cancellation;
- target-specific behavior.

### Algorithms

Add:

- empty/minimal input;
- ordinary cases;
- boundary and duplicate values;
- large input;
- complexity/performance guard when regression risk is material.

## 7. Declarative catalog direction

When changing the catalog infrastructure, prefer one typed operation declaration capable of generating or validating:

- semantic signature;
- native ABI signature;
- runtime symbol;
- backend support;
- docs/LSP export metadata;
- module inventory;
- parity tests.

Do not hide ownership or ABI semantics in an unreadable macro expansion. Generated outputs and validation failures must identify the operation and missing contract.

## 8. Review checklist

- Is the canonical module/name correct?
- Is this the correct layer?
- Is ownership explicit?
- Are failure and cleanup defined?
- Do AOT and JIT agree?
- Is C/debug accurately classified?
- Does LSP/docs export see it?
- Does a duplicate fallback table appear anywhere?
- Are security and performance costs understood?
- Are user docs and examples current?

## 9. Commands

Typical focused validation:

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-types --lib stdlib
cargo --manifest-path compiler/Cargo.toml test -p ori-runtime
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test multifile_imports
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test jit_run
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test diagnostic_catalog
```

Run the full workspace and relevant QA stages before completion.