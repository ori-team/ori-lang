---
id: ADR-0004
title: Keep the Cargo workspace under compiler and use root-first Ori projects
status: accepted
date: 2026-07-13
deciders: [project-maintainer]
supersedes: []
superseded_by: []
related_docs:
  - docs/architecture/repository-and-project-layout.md
  - docs/spec/17-project-and-docs.md
related_code:
  - compiler/Cargo.toml
  - compiler/crates/ori-driver
  - examples
---

# ADR-0004: Keep the Cargo workspace under `compiler/` and use root-first Ori projects

## Context

The repository contains more than a Rust compiler workspace. It also owns runtime artifacts, standard-library source, examples, documentation, editor extensions, QA, packaging, and release tooling.

Placing the Cargo workspace at the repository root would make Rust build structure appear to own every product domain. At the same time, early user-project scaffolds risked imposing a `src/` convention that was not required by Ori's module model.

A durable layout was needed for both the language monorepo and user-created projects.

## Decision drivers

- make repository-domain responsibilities visible;
- keep the Rust workspace cohesive without treating the whole repository as only a Cargo project;
- allow scripts and documentation to locate compiler and runtime sources predictably;
- make examples real Ori projects;
- keep beginner projects small and direct;
- avoid forcing `src/` when it adds no semantic value;
- support optional domain directories and sidecar documentation.

## Considered options

### Cargo workspace at repository root

Rejected because the root also owns non-Cargo product domains and distribution artifacts. It would blur repository-level and compiler-workspace responsibilities.

### Mandatory `src/` in Ori projects

Rejected because Ori explicitly declares modules, the project manifest already defines entry/root behavior, and a required extra directory would add ceremony without a language requirement.

### Compiler workspace under `compiler/` and root-first projects

Selected.

## Decision

The Rust workspace lives under:

```text
compiler/Cargo.toml
```

The repository root contains language-product domains including:

```text
compiler/
runtime/
stdlib/
docs/
examples/
tools/
extensions/
tests/
```

A normal Ori application uses:

```text
project/
├── ori.proj
├── main.orl
├── optional-domain-directories/
└── docs/
```

Rules:

- `ori.proj` is at the project root;
- `main.orl` is the recommended default application entry and may be changed by the manifest;
- `src/` and `app/` are optional conventions;
- domain directories are optional;
- publishable package metadata follows its own documented contract;
- examples are project-shaped rather than loose source scraps;
- temporary work and generated build state do not belong in the repository root.

## Consequences

### Positive

- Clear separation between compiler workspace and full product repository.
- Root commands can use `--manifest-path compiler/Cargo.toml` predictably.
- User projects begin with only the files they need.
- Domain organization remains flexible.
- Examples exercise actual project loading and manifests.

### Negative

- Rust contributors must enter `compiler/` or use `--manifest-path`.
- Generic Rust tooling may not discover the workspace from the repository root automatically.
- Scripts and docs must state their working directory.
- Repository-wide tools need explicit paths to compiler artifacts.

## Invariants established

- Cargo commands documented from the root identify `compiler/Cargo.toml`.
- Runtime source remains under the compiler workspace while staged artifacts remain under root `runtime/`.
- `ori new` does not force `src/` for ordinary projects.
- Examples use the same project rules taught to users.
- Generated content has a documented tool-owned location.

## Compatibility and migration

Moving the Rust workspace required coordinated updates to CI, scripts, runtime staging, tests, editor tooling, and documentation.

User-project layout remains root-first. A future mandatory-directory change would be a project-format compatibility event and requires an RFC/migration plan.

## Validation

- project scaffold tests;
- source-graph and entry discovery tests;
- workspace/root path tests;
- runtime staging and packaging tests;
- example checks;
- documentation link and command validation.

## Reconsideration criteria

Reconsider only if repository tooling, package distribution, or a proven project-scale requirement demonstrates that the current boundary materially obstructs development. Visual preference alone is insufficient reason for another broad move.