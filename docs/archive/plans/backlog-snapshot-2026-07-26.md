# Archived backlog snapshot — 2026-07-26

> Status: **archived summary**  
> Source: former expanded `docs/planning/BACKLOG.md`  
> Current project version: **0.3.8**

The former backlog accumulated completed implementation reports, measured experiments, rejected designs, shelved ecosystem work, performance results, and current priorities in one file.

This archive summarizes that state. Detailed line-by-line history remains available in Git history and the changelog.

## Major completed outcomes recorded by the former backlog

- S3 surface and local inference;
- package installation path and native ABI documentation;
- stdlib layering and runtime-backed APIs;
- native async subset and documented C/debug limitations;
- Linux release packaging;
- driver phase interfaces and thin façade;
- syntax/semantic audit and regression matrix;
- ARC single-cascade-owner fix;
- strict Clippy quality gate;
- realistic Linux integration project;
- fixed-size arrays, generic traits, associated functions, CT-0 constants, inline iterators, and custom destructors;
- ARC registry and allocation performance improvements;
- zero-copy list windows;
- aggregate embedding scope and generated C headers;
- debugger/DAP and VS Code integration;
- large-module compiler scalability fixes;
- per-file incremental native objects;
- dependency lockfile and package namespace isolation;
- numerous memory, codegen, diagnostic, and stdlib regressions.

## Measured and rejected or reverted experiments

- implicit first-class iterator objects;
- explicit `move` syntax;
- broad compile-time execution without a concrete need;
- skip-registration/acyclic ARC variants without measured benefit;
- runtime free-list caching that was slower than the platform allocator;
- small-string optimization and text windows blocked by current representation/ABI;
- general scoped arenas without a demonstrated workload;
- copy-on-write collection semantics under the current compatibility line;
- full C-backend parity;
- higher-kinded types for the current reading-first model.

Rejected and deferred decisions should be represented by ADRs or RFCs, not kept as active backlog rows.

## Shelved product/ecosystem directions

- self-hosting until a stable compiler/stdlib/ABI window and bootstrap criteria exist;
- central hosted registry infrastructure;
- editor marketplace publication;
- external game/UI/physics/database ecosystem packages as core-monorepo products;
- broad multi-platform marketing work without an explicit reopening decision.

## Why the backlog changed

The former file was valuable evidence but no longer functioned as an open-work list because most rows were `done`, `reverted`, `blocked`, `shelved`, or `rejected`.

The current backlog now contains only actionable open outcomes and explicitly deferred milestones. Completed technical narratives belong in:

- `CHANGELOG.md`;
- ADRs;
- architecture and specification;
- archived plans/investigations;
- tests and benchmark evidence;
- Git history.

Do not add current tasks to this archive.