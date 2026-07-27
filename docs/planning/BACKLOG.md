# Ori implementation backlog

> Canonical open-work list during planning-directory migration  
> Current project version: **0.3.8**  
> Last consolidated: **2026-07-27**

This file contains only work that remains open, in progress, or deliberately deferred. Completed implementation history moved to the changelog, ADRs, architecture, tests, and [`../archive/plans/backlog-snapshot-2026-07-26.md`](../archive/plans/backlog-snapshot-2026-07-26.md).

Planning policy: [`../plans/README.md`](../plans/README.md).

## Status and size

| Field | Values |
|---|---|
| Priority | `P0` urgent · `P1` next · `P2` soon · `P3` later · `P4` deferred milestone |
| Size | `S` focused · `M` multi-module · `L` multi-PR · `XL` long program |
| Status | `todo` · `in_progress` · `blocked` · `deferred` |

`done`, `rejected`, `reverted`, `cancelled`, and historical measurement reports do not remain in this backlog.

## Execution principles

1. Correctness, crash prevention, security, and data integrity outrank feature expansion.
2. Specification, implementation, diagnostics, examples, and tooling must agree.
3. Runtime/ABI changes require AOT/JIT, ownership, symbol, staging, and package evidence.
4. Structural refactoring is separated from semantic language changes.
5. New language work requires a real use case and the governance route appropriate to its compatibility impact.
6. External frameworks and application-specific ecosystems remain outside the core repository unless explicitly accepted.

## Active work

| ID | Outcome | P | Size | Status | Completion evidence |
|---|---|---:|:---:|---|---|
| **DOC-CONFORMANCE-1** | Build a machine-readable specification-to-test conformance registry | P2 | L | todo | Stable rule IDs, chapter coverage report, positive/negative/backend/tooling mappings, CI drift check |
| **DOC-METADATA-1** | Apply canonical metadata to maintained domain documents and validate owners/status/related code/evidence | P2 | M | todo | Metadata checker; no duplicate canonical ownership; paths/evidence resolve |
| **COMP-RUNTIME-MOD-1** | Incrementally split the runtime monolith into ABI/ARC/value/collection/I/O/network/async domain modules | P1 | L | todo | Characterization tests first; exported symbols/layouts unchanged; AOT/JIT/runtime/package gates and performance baselines green |
| **COMP-CHECKER-MOD-1** | Group and split type-checker state/rules into explicit program, function, scope, inference, trait, pattern, concurrency, and diagnostic contexts | P1 | L | todo | Diagnostic and semantic parity; `ori_spec` and workspace green; no language change in refactor PRs |
| **STDLIB-CATALOG-2** | Evolve the stdlib catalog into one strongly typed declarative operation registry | P2 | L | todo | Semantic signature, ABI, symbols, backend flags, docs/LSP export, and parity tests derive from or validate one declaration |
| **DIAG-REGISTRY-1** | Replace repeated diagnostic-code string literals with typed/generated identifiers while preserving public strings | P2 | M | todo | Generated/typed registry, catalog parity, CLI/LSP compatibility, no orphan emitted codes |
| **QA-FUZZ-1** | Establish lexer/parser/formatter/manifest/runtime fuzzing baseline | P2 | L | todo | Reproducible fuzz targets, seed corpus, minimized regressions, scheduled or documented execution route |
| **QA-DIFF-1** | Expand AOT↔JIT and optimized↔unoptimized differential testing | P2 | M | todo | Shared fixtures and property checks compare output, exit behavior, cleanup, and diagnostics |
| **QA-DOC-1** | Expand documentation CI beyond basic path/version/link checks | P2 | M | todo | Orphan docs, EN/PT parity, example compilation, ADR/RFC/plan state, and archive-policy checks |
| **OPS-REPRO-1** | Define and validate reproducible release inputs and artifact provenance | P2 | L | todo | Repeatable package build, checksums, target metadata, SBOM/attestation plan, isolated public-artifact smoke |
| **SEC-SUPPLY-1** | Harden package, registry, dependency, updater, and release supply-chain policy | P2 | L | todo | Threat scenarios covered by tests; token redaction; archive/path validation; dependency audit and release provenance |
| **SUPPORT-MATRIX-1** | Maintain a generated or validated platform/backend/tooling support matrix | P2 | M | todo | Target, AOT, JIT, LSP, debugger, package and CI evidence linked from current product docs |

## Deferred milestones

Deferred items are not part of the current execution queue. Reopening requires the stated evidence and usually an RFC or ADR.

| ID | Milestone | P | Size | Status | Reopening condition |
|---|---|---:|:---:|---|---|
| **M4** | Self-hosting/bootstrap compiler in Ori | P4 | XL | deferred | Stable frontend/HIR/stdlib/ABI window, reproducible bootstrap subset, documented build matrix, and explicit program approval |
| **PKG-REG** | Official hosted registry service | P4 | L | deferred | Approved service ownership, authentication/signing, retention, availability, incident, and funding/operations model |
| **BACK-C-1** | Full C-backend product parity | P4 | L | deferred | Explicit decision to make C a product backend; current route remains partial synchronous debug/transpile |
| **DIST-MARKET** | Broad marketplace and multi-platform promotion | P4 | L | deferred | Language/docs/runtime/package reliability demonstrated on the supported matrix and promotion explicitly prioritized |
| **LANG-COMPTIME** | General compile-time execution/macros | P4 | XL | deferred | Concrete problem not served by CT-0, security/sandbox design, semantic model, and accepted RFC |
| **LANG-COW** | Implicit copy-on-write collection semantics | P4 | L | deferred | Real aliasing/usability or measured workload evidence plus compatibility, ABI, migration, and semantic RFC |
| **LANG-ITER-OBJECT** | Implicit first-class iterator state machines | P4 | L | deferred | Real program where explicit `Iterable` state and inline iterators are materially inadequate |
| **LANG-ARENA** | General scoped arenas/region semantics | P4 | XL | deferred | Measured short-lived-object workload and safe lifetime/escape model with material prototype gain |

## Explicitly outside the core backlog

- External game, UI, physics, database, and application frameworks.
- Marketplace publication for editor extensions unless reopened.
- Global Hindley–Milner inference.
- Higher-kinded types without a new accepted language decision.
- Features copied from another language without an Ori-specific problem and evidence.

Such work may exist in sibling repositories, RFC drafts, or archived investigations. It must not be reintroduced into the core monorepo through an incidental PR.

## Adding an item

A new item must include:

- stable ID;
- outcome rather than an implementation dump;
- priority and size;
- owner/issue when execution begins;
- dependencies;
- contract and component links;
- completion evidence;
- compatibility, security, and performance implications when applicable.

Complex work creates an ExecPlan under `docs/plans/active/`. Small work remains an issue and focused PR.

## Closing an item

When complete:

1. record user-visible change in `CHANGELOG.md` when applicable;
2. update specification, architecture, tests, status, and operations;
3. close linked issue/plan checkpoints;
4. remove the row from this active backlog;
5. preserve unique outcome/history in an ADR, changelog, or archive only when useful.

The backlog is not the project history.
