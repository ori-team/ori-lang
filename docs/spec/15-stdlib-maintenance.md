# Ori Language Specification — Chapter 15: Standard-library maintenance

> Status: normative maintenance contract for Ori **0.3.8**  
> Audience: standard-library, compiler, runtime, LSP, and documentation contributors

This chapter defines the required synchronization between the standard-library semantic catalog, runtime, Ori source modules, documentation, tooling, and tests.

Architecture overview: [`../architecture/stdlib.md`](../architecture/stdlib.md).  
Public API contract: [`12-stdlib.md`](12-stdlib.md).  
Native ABI: [`19-abi.md`](19-abi.md).

## 1. Layer model

| Layer | Canonical location | Responsibility |
|---|---|---|
| Layer 1 | `compiler/crates/ori-types/src/stdlib.rs` + `ori-runtime` | semantic signatures, ABI metadata, native primitives, OS/runtime integration |
| Layer 2 | `stdlib/**/*.orl` | safe wrappers and compositional functions written in Ori |
| Layer 3 | `stdlib/**/*.orl` | pure algorithms written in Ori |
| Documentation | `.oridoc`, Chapter 12, generated exports | user contract, examples, editor/CLI help |

Layer 1 remains appropriate for ARC, collections, async scheduling, I/O, networking, process integration, encoding primitives, and other native or hot-path behavior.

Layer 2/3 are appropriate when behavior can be expressed clearly and safely in Ori without duplicating a native primitive or causing a measured unacceptable cost.

## 2. Canonical semantic catalog

`compiler/crates/ori-types/src/stdlib.rs` owns the canonical mapping for runtime-backed operations:

- canonical `ori.*` path;
- compatibility aliases;
- runtime symbol;
- native and C/debug support flags;
- semantic signature;
- native ABI metadata;
- implemented module classification.

Downstream crates must query or derive from this catalog. They must not maintain independent complete path/symbol/signature lists.

The implementation should continue evolving toward one strongly typed declaration per operation that generates or validates all secondary tables.

## 3. Runtime-backed operation requirements

Adding or changing a runtime-backed operation requires all applicable items:

1. canonical path;
2. compatibility aliases, only when justified;
3. semantic parameter and return types;
4. native ABI signature and ownership contract;
5. exported `ori_*` runtime symbol;
6. AOT symbol declaration/resolution;
7. JIT cdylib symbol availability;
8. C/debug support flag or explicit unsupported route;
9. Chapter 12 contract;
10. sidecar/source documentation;
11. documentation export and LSP visibility;
12. semantic positive and negative tests;
13. native AOT execution;
14. JIT execution;
15. ownership/resource-lifecycle tests;
16. backend-matrix update;
17. changelog entry when user-visible.

An operation is incomplete when checker acceptance, runtime availability, docs, or editor help disagree.

## 4. Pure compiler intrinsics

A compiler intrinsic that emits no runtime call:

- must not be registered as a runtime-backed symbol;
- has type rules in the owning semantic catalog/checker family;
- lowers in one canonical phase;
- has no duplicate implementation in HIR and codegen unless their responsibilities are distinct and documented;
- has tests proving no runtime symbol is required;
- is documented in Chapter 12 when public.

## 5. Layer 2 and Layer 3 modules

A source module path maps to a file below `stdlib/`:

```text
ori.path              -> stdlib/path.orl
ori.example.helpers   -> stdlib/example/helpers.orl
```

The file declares the matching module name.

Exported functions use `public`. Internal helper functions remain private.

Source modules:

- use current S3 syntax;
- import Layer 1 operations through ordinary canonical imports;
- avoid undocumented compiler/runtime shortcuts;
- preserve module and package boundaries;
- include executable multifile coverage;
- document failure, mutation, allocation, and complexity where relevant.

## 6. API naming

A public operation has one canonical path.

Compatibility aliases:

- must be explicitly listed;
- must have a documented compatibility or ergonomic reason;
- must resolve to the same semantic contract;
- must not create conflicting documentation identities;
- require a migration/deprecation policy before removal.

Parent modules should present coherent APIs. Internal source-file organization must not force users to learn unnecessary `.utils`-style namespaces.

## 7. Ownership and resources

Every operation involving managed or external resources documents:

- whether arguments are borrowed, retained, consumed, or copied;
- ownership of returned managed values;
- ownership when values are inserted into collections/results/optionals;
- mutation and iterator invalidation;
- deterministic close/dispose requirements;
- destructor behavior;
- task/channel transferability;
- null/sentinel handling at the ABI boundary.

The runtime follows the single-cascade-owner invariant. A wrapper or destructor must not release a child already owned by a registered ARC edge.

## 8. Error contracts

Recoverable failures use documented `result[T, E]` contracts rather than process aborts.

Bounds traps, impossible ABI corruption, or other unrecoverable runtime conditions may abort only where the language/runtime contract specifies that behavior.

Error messages should identify the failed operation and useful context without exposing secrets or unrelated local state.

## 9. Platform behavior

Platform-dependent operations document:

- supported targets;
- path and encoding differences;
- required system libraries;
- timeout/cancellation behavior;
- unavailable or degraded behavior;
- security implications.

A function must not claim cross-platform support based solely on compilation. Supported routes require appropriate execution or smoke evidence.

## 10. Documentation

Every public operation should document:

- purpose;
- parameters;
- return value;
- failure cases;
- mutation and ownership behavior;
- resource lifecycle;
- complexity for potentially expensive operations;
- platform differences;
- minimal valid example;
- experimental status where applicable.

`.oridoc` remains concise and symbol-oriented. Deeper design belongs in architecture or an ADR.

## 11. LSP and generated documentation

The LSP and `ori doc export` must derive signatures, availability, and documentation from canonical compiler and sidecar sources.

A stdlib change should verify:

- completion path and signature;
- hover content;
- go-to-documentation/source behavior where supported;
- generated JSON/documentation indexes;
- no orphan documented symbol;
- no runtime operation missing from the public catalog when intended to be public.

## 12. Parity gates

Tests should enforce at least:

- unique canonical paths and aliases;
- every runtime-backed path has a semantic signature;
- every native entry has ABI metadata;
- every declared runtime symbol is exported and resolvable;
- backend flags match Chapter 14;
- importable module classification matches source/runtime reality;
- documentation signatures reference implemented modules;
- `.orl` module declarations match their canonical paths;
- AOT and JIT shared behavior agrees;
- managed operations preserve ownership and cleanup.

When a parity gate fails, correct the canonical catalog or implementation rather than adding another fallback list.

## 13. Testing minimum

For a new public function:

- semantic acceptance test;
- invalid argument/type diagnostic where relevant;
- native AOT execution;
- JIT execution;
- C/debug test or explicit unsupported diagnostic;
- multifile/import test;
- ownership/leak/resource test where managed or external resources are involved;
- documentation/LSP parity test;
- platform test for platform-dependent behavior.

Algorithms should also have edge, empty, large-input, and complexity-oriented coverage.

## 14. Security review

Review is required for operations that process:

- paths and files;
- archives;
- network data or TLS;
- process arguments/environment;
- configuration or serialized data;
- credentials/tokens;
- unbounded allocations;
- concurrency and shared state.

Use the threat model in [`../security/threat-model.md`](../security/threat-model.md).

## 15. Performance review

Runtime hot-path changes measure:

- allocations;
- retain/release count;
- lock contention;
- copying;
- algorithmic complexity;
- AOT/JIT differences;
- realistic application workload.

Pure Ori implementations should be preferred for clarity only when their semantics and measured cost are acceptable.

## 16. Deprecation and removal

Removing or renaming a public operation requires:

- compatibility classification;
- deprecation or transition window where appropriate;
- replacement documentation;
- diagnostic/help behavior;
- migration guidance;
- alias and LSP cleanup;
- changelog and versioning review;
- conformance update.

## 17. Completion checklist

A standard-library change is complete when:

- canonical catalog and implementation agree;
- AOT, JIT, and support matrix agree;
- ownership and resource behavior are tested;
- docs, sidecars, LSP, and generated exports agree;
- compatibility/security/performance effects are recorded;
- the change passes focused and workspace gates;
- no parallel fallback table was introduced.