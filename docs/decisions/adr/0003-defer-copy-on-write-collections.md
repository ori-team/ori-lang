---
id: ADR-0003
title: Defer copy-on-write collection semantics
status: accepted
date: 2026-07-18
deciders: [project-maintainer]
supersedes: []
superseded_by: []
related_docs:
  - docs/spec/10-memory.md
  - docs/spec/18-stability-and-compatibility.md
  - docs/architecture/runtime-and-memory.md
related_code:
  - compiler/crates/ori-runtime
  - compiler/crates/ori-codegen
---

# ADR-0003: Defer copy-on-write collection semantics

## Context

Ori collections currently use shared reference semantics for mutating operations. If two bindings refer to the same collection, an in-place mutation through one binding is observable through the other.

Copy-on-write (COW) was considered as a way to provide value-like collection behavior: mutate in place when uniquely referenced, clone before mutation when shared.

That change would not be a transparent optimization. It would change observable aliasing behavior and require mutators to return or update a potentially new collection pointer.

## Decision drivers

- preserve the documented 0.3.x collection semantics;
- avoid an unmeasured semantic and ABI change;
- keep reading and ownership behavior explicit;
- avoid adding an atomic uniqueness check to every mutation without demonstrated need;
- require real user or performance evidence before changing collection semantics.

## Considered options

### Adopt COW immediately

Rejected for the current compatibility line. It would change aliasing semantics, mutation APIs, runtime symbols, generated code, and possibly ABI signatures.

### Keep shared in-place mutation

Selected for 0.3.8. It matches current implementation and avoids introducing hidden copies.

### Introduce new explicitly value-semantic collection types

Not selected as part of this decision. It remains a possible future proposal if a distinct type and use case justify the added surface.

## Decision

Do not adopt implicit COW for current Ori collections.

Collection mutators continue to mutate the shared collection object in place according to their documented contracts.

COW may be reconsidered only after deciding the public semantic model first. The mechanism must not be introduced as a private runtime optimization when it changes observable aliasing.

## Consequences

### Positive

- Current source and runtime behavior remain compatible.
- Mutation cost does not include a uniqueness check and possible clone.
- Aliasing remains visible rather than being hidden behind implicit copy behavior.
- Runtime and ABI mutator signatures remain unchanged.

### Negative

- Users who need independent values must clone or construct them explicitly.
- Shared aliases can observe mutation and must be understood by users.
- Ori does not currently provide Swift-style value semantics for its managed collections.

## Technical implications of future COW

A future COW design would need to address:

- uniqueness based on the managed reference count;
- whether ownership edges count toward uniqueness;
- shallow versus deep clone;
- retain/release for cloned elements, keys, and values;
- mutator return or out-parameter ABI;
- binding reassignment semantics;
- iterator invalidation;
- thread safety;
- AOT/JIT parity;
- embedding and generated-header changes;
- migration and version transition.

Because current mutators operate on a pointer and may return no replacement pointer, COW would require a public runtime/codegen contract change.

## Validation

Current behavior should remain covered by:

- aliasing tests for list, map, set, and other mutable collections;
- mutation and iterator-invalidity tests;
- ownership/leak tests;
- standard-library documentation;
- AOT/JIT parity;
- performance baselines for collection mutation.

## Reconsideration criteria

Reopen only when both are true:

1. real programs show that shared in-place mutation causes significant usability or correctness problems, or defensive cloning dominates measured workloads;
2. the project is prepared to review collection semantics, compatibility, versioning, runtime ABI, migration, accessibility, and performance through an RFC.

A performance experiment alone cannot decide the user-visible semantic model.