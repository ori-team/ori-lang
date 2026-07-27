# Archived maturity plan — June 2026

> Status: **archived**  
> Original planning period: 2026-06-19 through 2026-06-29  
> Archived during documentation-framework migration: 2026-07-27  
> Current project version: **0.3.8**  
> Current sources: [`../../ATLAS.md`](../../ATLAS.md), [`../../product/status.md`](../../product/status.md), and [`../../../CHANGELOG.md`](../../../CHANGELOG.md)

This record summarizes a completed maturity program that previously existed as a large active-style checklist. It is preserved as historical evidence, not as a current backlog or implementation guide.

## Program objective

The program aimed to move the early Ori compiler from an inconsistent development snapshot toward a validated native language toolchain with synchronized specification, runtime, standard library, LSP, diagnostics, repository organization, and release procedures.

## Completed stages

| Stage | Historical outcome |
|---|---|
| Workspace stabilization | Compiler workspace built and tests were restored after backend changes |
| Blocking language/runtime behavior | Async branching, deterministic cleanup paths, file/runtime primitives, cancellation, and structural behavior were validated |
| Advanced type-system work | Traits, associated behavior, generics, collection rules, and structured JSON work were integrated according to that period's contracts |
| Normative documentation synchronization | Specification chapters and diagnostics were reconciled with implementation and tests |
| Compiler technical debt | Important async, codegen, and semantic gaps were closed or explicitly classified |
| Runtime and ARC | Ownership regressions, cycle-collection paths, and leak checks received focused coverage |
| LSP and tooling | Cross-file semantic features and project-level diagnostics were expanded |
| Diagnostic catalog | Emitted codes and naming conventions were audited |
| Repository and quality | Stdlib catalog centralization, modularization, toolchain pinning, and QA gates were improved |
| Release preparation | A pre-1.0 native release path and smoke validation were established |

## Evidence locations

Historical detail is distributed across:

- `CHANGELOG.md` release and unreleased sections;
- `docs/planning/IMPLEMENTADOS.md` during the transitional planning era;
- compiler and runtime regression tests;
- `docs/planning/qa/test-matrix-ori.md`;
- archived investigations and completed plans;
- Git history and merged pull requests.

The current architecture and implementation rules have since evolved. Do not infer current support, syntax, commands, version, or priorities from this archive.

## Lessons retained

The program established several practices that remain valuable:

- restore a green baseline before feature expansion;
- synchronize normative docs with implementation and tests;
- prefer one semantic catalog over repeated hardcoded lists;
- require regression evidence for runtime ownership changes;
- use staged QA rather than one opaque full-suite command;
- separate native semantic support from the partial C/debug route;
- validate release packages outside the source workspace;
- record residual limitations instead of presenting them as completed support.

## Superseded planning model

The former document mixed:

- active tasks;
- completed checklists;
- current architecture;
- historical evidence;
- release notes;
- known limitations.

The documentation framework now separates these responsibilities:

- open work → `docs/planning/BACKLOG.md` during migration;
- current product status → `docs/product/status.md`;
- current architecture → `docs/architecture/`;
- accepted decisions → `docs/decisions/`;
- complex active execution → `docs/plans/active/`;
- released behavior → `CHANGELOG.md`;
- historical evidence → `docs/archive/`.

## Archival rule

Do not update this file with new tasks. Correct only historical inaccuracies, broken references, or archive metadata.