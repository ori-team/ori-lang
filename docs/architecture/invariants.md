# Architectural invariants

This document collects cross-cutting rules that must remain true across compiler phases, runtime, tooling, documentation, and releases.

An invariant is stronger than a coding preference. A change that violates one requires either a defect fix or an explicit decision that updates the relevant contract, tests, and migration path.

## Language and source invariants

- Every valid source file declares its module using the current canonical syntax.
- Removed legacy syntax is rejected with stable diagnostics rather than silently reinterpreted.
- Public behavior described by the specification is accepted consistently by check, compile, run, test, formatter, and LSP paths where applicable.
- A source-language rule is enforced in the earliest phase that has enough information to diagnose it accurately.
- Parser recovery must not invent valid semantics for malformed source.
- Formatting preserves program meaning and is idempotent.

## Semantic invariants

- Name resolution produces stable identities that later phases can reference without repeating textual lookup rules.
- Type checking finishes before native code generation receives the program.
- HIR values consumed by backends do not contain unresolved user types except explicit internal error or deferred forms documented by the owning phase.
- `Ty::Error` and equivalent recovery values prevent diagnostic cascades; they do not represent valid runtime values.
- Public visibility, module boundaries, and package boundaries are checked before lowering or linking.
- Traits, associated types, generics, and monomorphization use one consistent substitution model across checker and HIR.
- Exhaustiveness and definite-return facts used later in a phase must be recorded explicitly rather than re-guessed from incomplete data.

## Pipeline invariants

- Each compiler phase has a clear input, output, and diagnostic boundary.
- The driver façade orchestrates phases but does not own their internal algorithms.
- Lower phases do not depend on CLI formatting or command-specific presentation.
- A failed phase does not pass partially valid output to a backend unless the contract explicitly supports recovery output.
- Timing, reporting, and debug metadata observe phase behavior without becoming hidden semantic dependencies.
- Incremental reuse is allowed only when source graph, manifests, lockfile, compiler version, public interfaces, build options, and relevant metadata match.

## Backend invariants

- The Cranelift native backend is the semantic reference.
- Unsupported native shapes must be rejected explicitly instead of silently generating different semantics.
- AOT and JIT must agree on accepted behavior, values, failures, cleanup, and runtime symbol contracts for their shared support surface.
- Optimization may change performance but not observable semantics.
- Optimization passes must preserve type and control-flow validity.
- Invalid source is not repaired in codegen.
- Platform-specific behavior is isolated behind target-aware modules and metadata.

## Runtime and memory invariants

- Every managed allocation has a valid header matching the documented ABI layout.
- Retain and release operations act only on valid managed payloads or explicitly permitted null values.
- Registered ARC edges are the single owner of cascaded managed-child release.
- A destructor may observe edge-owned children but must not release them independently.
- Custom destructors run at most once for an object.
- Cycle collection finalization order and edge removal must not produce double release, use-after-free, or dangling registry entries.
- Runtime collection operations preserve ownership of inserted managed values for the documented duration.
- Values crossing task or channel boundaries satisfy the documented transferability rules.
- A staged static runtime and cdylib correspond to the same project and ABI versions.

## ABI and FFI invariants

- Exported `ori_*` symbols required by the compiler remain discoverable in the expected runtime artifact.
- Layout, alignment, tagging, ownership, and calling-convention rules match the normative ABI document.
- Incompatible layout or lifecycle changes require a new ABI tag.
- FFI adapters validate pointers, lengths, tags, and ownership assumptions at the boundary whenever validation is possible.
- Safe internal logic should not require raw pointers after conversion at the FFI boundary.
- Every `unsafe` block has a reviewable safety argument.
- Generated C headers and shared-library exports agree with the HIR and ABI metadata used to produce them.

## Standard-library invariants

- Canonical stdlib paths resolve through a shared manifest or documented source-module discovery path.
- Downstream crates do not maintain independent lists that can drift from the stdlib manifest.
- Every runtime-backed stdlib operation has semantic type information and native ABI metadata.
- Backend-support flags describe implemented behavior rather than desired behavior.
- Layer 2 and Layer 3 `.orl` modules use public functions for cross-module calls and canonical S3 syntax.
- User documentation, `.oridoc`, LSP catalog, runtime symbols, and implementation tests remain synchronized.

## Diagnostic invariants

- Every emitted public diagnostic code is registered in the diagnostic catalog.
- A diagnostic code keeps its semantic meaning across compatible releases.
- Primary spans identify the location the user can act on.
- User-facing messages use current Ori terminology and syntax.
- Diagnostics for removed syntax point to the current replacement or migration route.
- LSP diagnostics and CLI diagnostics derive from the same semantic result.

## Documentation invariants

- Each subject has one canonical current document.
- Current architecture describes implemented behavior, not a planned future design.
- Normative specification describes behavior accepted today.
- Plans do not become normative merely because implementation work started.
- Archived documents are not linked as current operational instructions.
- Current product version is 0.3.8 unless a document explicitly discusses release history or another version class.
- Documentation changes affecting users preserve supported language parallels where the project maintains them.
- Examples presented as runnable are validated against the current compiler.

## Quality and release invariants

- Every bug fix includes a regression test at the lowest useful layer and at the user-visible layer when needed.
- Every new diagnostic has a negative test and catalog entry.
- Every new language feature has positive, negative, formatter/tooling, and backend evidence appropriate to its support matrix.
- Release packages are tested outside the source workspace and do not silently depend on developer-only files.
- Checksums, artifact names, runtime metadata, and compiler version agree.
- A performance claim includes a reproducible method and caveats.
- A known failing gate cannot be hidden by weakening the gate without an explicit, documented decision.

## Changing an invariant

To change an invariant:

1. identify the user or engineering problem;
2. open an RFC or ADR appropriate to the scope;
3. list affected specifications, architecture, code, tests, migration, ABI, and operations;
4. add characterization tests for the old behavior when needed;
5. implement the change in vertical slices;
6. update this document and the machine-readable catalog;
7. record compatibility and release consequences.

Local convenience is not sufficient reason to weaken a cross-cutting invariant.