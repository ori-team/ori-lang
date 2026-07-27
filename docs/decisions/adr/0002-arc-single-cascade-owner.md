---
id: ADR-0002
title: Registered ARC edges are the single cascade owner
status: accepted
date: 2026-07-17
deciders: [project-maintainer]
supersedes: []
superseded_by: []
related_docs:
  - docs/architecture/runtime-and-memory.md
  - docs/architecture/invariants.md
  - docs/spec/10-memory.md
  - docs/spec/16-runtime-ffi-safety.md
  - docs/spec/19-abi.md
related_code:
  - compiler/crates/ori-runtime
  - compiler/crates/ori-codegen
---

# ADR-0002: Registered ARC edges are the single cascade owner

## Context

Managed composite values stored child references while two mechanisms attempted to release those children:

1. compiler-generated destructor hooks for structs, enums, and tuples;
2. registered ARC ownership edges, each of which retained the child.

This double ownership caused double release. A child shared by a live binding and a dying composite could be freed while the binding was still in scope. At the same time, stores that did not release an owned temporary after registering an edge leaked references, with some leaks hidden by the duplicate release.

The runtime needed one unambiguous owner for cascaded managed-child cleanup.

## Decision drivers

- prevent double release and use-after-free;
- preserve one ownership rule for all managed containers;
- keep edge registration meaningful for ordinary cleanup and cycle collection;
- support borrowed and newly owned sources correctly;
- avoid a broad type-driven destructor redesign during stabilization;
- make ownership review and tests mechanically understandable.

## Considered options

### Destructors own child release; edges are trace-only

This resembles type-driven tracing systems where destructors release fields and graph edges only support collection.

Rejected for the current architecture because every managed wrapper and container would need complete destructor coverage, edge retains would need redesign, and the change would touch optional/result wrappers, closure environments, async frames, and collections at once.

### Registered edges own child release

Selected because the runtime already registers owning edges and the rule works uniformly for borrowed and newly allocated sources.

## Decision

A registered ARC edge is the sole owner of cascaded release for a stored managed child.

The store sequence is:

```text
store child
-> register or update owner -> child edge
-> if the expression produced an owned temporary, release that temporary's own reference
```

Rules:

1. `ori_arc_register_edge` retains the child.
2. Borrowed sources keep their original owner's reference; the edge retain creates the composite's ownership.
3. Fresh/owned temporaries transfer ownership by releasing their temporary reference after edge registration.
4. Composite destructors do not release compiler-registered managed children.
5. Runtime-internal destructor hooks may release non-edge resources or internal storage, but never the same edge-owned child.
6. Cycle collection uses the same registered edges and must remove them consistently.

## Consequences

### Positive

- One release path for stored managed children.
- Double release in composites is eliminated.
- Borrowed and fresh expressions follow one explicit store rule.
- Cycle-collection graph and ordinary cascade ownership agree.
- Compiler-generated composite destructor functions can be removed where their only role was edge-owned release.

### Negative

- Edge completeness is load-bearing. A missing edge becomes a leak or ownership defect.
- Every managed store path must classify fresh versus borrowed values correctly.
- Runtime wrappers and custom destructors require careful review to avoid manually releasing edge-owned payloads.

## Invariants established

- A managed child has one cascaded-release owner per stored relationship.
- Destructors run at most once and do not duplicate edge cleanup.
- Registering an edge retains the child.
- Fresh temporary ownership is balanced after edge registration.
- Removing an object removes its forward and reverse edge relationships safely.
- AOT and JIT generated ownership behavior must agree.

## Affected components

- HIR/codegen expression-ownership classification;
- struct, enum, tuple, optional, result, closure, async-frame, and collection stores;
- runtime edge indexes and free paths;
- cycle collection;
- custom destructor contract;
- memory and ABI specifications;
- memory, AOT, JIT, and leak tests.

## Validation

Evidence includes:

- shared-child lifetime regressions;
- zero-leak tests for nested managed composites and lists;
- runtime edge registration/removal tests;
- cycle collection tests;
- custom destructor single-execution tests;
- AOT/JIT parity;
- strict runtime/codegen static checks.

## Reconsideration criteria

A future move to type-driven tracing/destruction may supersede this decision only with:

- a complete ownership model for every managed wrapper/container;
- measured justification;
- ABI and migration analysis;
- no mixed edge/destructor ownership period;
- equivalent or stronger memory-safety and performance evidence.