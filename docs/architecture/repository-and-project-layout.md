# Repository and project layout

This document is the canonical current architecture for the `ori-lang` repository and user-created Ori projects.

The original accepted layout record remains available through the transitional pointer at `docs/planning/repo-and-project-layout.md`. Historical rationale should migrate to an ADR; this document owns the current shape.

## 1. Core repository

```text
ori-lang/
├── AGENTS.md
├── PROJECT_START.md
├── README.md
├── CHANGELOG.md
├── SECURITY.md
├── compiler/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── crates/
├── stdlib/
├── runtime/
├── examples/
├── extensions/
├── tests/
├── tools/
├── docs/
└── .ai/
```

## 2. Root responsibilities

The repository root contains only project-wide entry points, policies, licenses, toolchain configuration, and the major product domains.

It should not contain:

- temporary patches;
- personal scratch files;
- session notes;
- generated benchmark output not intended for version control;
- duplicated plans;
- application-specific packages without an explicit product decision.

## 3. Cargo workspace

The Rust workspace is located at:

```text
compiler/Cargo.toml
```

Canonical root commands use:

```bash
cargo --manifest-path compiler/Cargo.toml <command>
```

Entering `compiler/` before running Cargo is also valid. Documentation must state the working directory clearly.

The workspace contains focused crates for lexer, AST, parser, types, HIR, codegen, runtime, diagnostics, LSP, and driver.

## 4. Runtime source versus staged artifacts

```text
compiler/crates/ori-runtime/   source implementation
runtime/<target-triple>/       staged staticlib, cdylib, and metadata
```

Staged artifacts are distribution inputs. They do not replace runtime source or architecture documentation.

A runtime source change must consider both AOT static and JIT dynamic artifacts.

## 5. Standard library

```text
stdlib/
├── *.orl
├── *.oridoc
└── <submodules>/
```

Public module identity follows the declared `module ori...` path. Directory organization supports module discovery but does not create arbitrary user-visible namespace layers.

The semantic/runtime catalog remains under `ori-types` and `ori-runtime`; source modules complement it according to the layer model.

## 6. Examples

Each substantial example should be a valid Ori project:

```text
examples/example_name/
├── ori.proj
├── main.orl
├── optional supporting modules
└── README.md when setup or purpose is not obvious
```

Examples are not a miscellaneous source dump. Each example should teach or validate a distinct capability.

## 7. Documentation

The documentation domains are defined by `docs/ATLAS.md`:

```text
docs/
├── ATLAS.md
├── catalog.yaml
├── product/
├── architecture/
├── spec/
├── implementation/
├── quality/
├── security/
├── governance/
├── decisions/
├── rfcs/
├── plans/
├── operations/
├── language/
├── guides/
├── book/
└── archive/
```

New root-level documentation files require a project-wide reason. Domain documents belong in their owning directory.

## 8. Tools

`tools/` is divided by operational purpose rather than by contributor:

- QA and test orchestration;
- benchmarks;
- runtime staging;
- packaging and installation;
- release validation;
- documentation validation/generation;
- focused migration utilities.

A tool should invoke public compiler/domain APIs or commands where practical. It should not become a hidden alternate implementation.

## 9. AI context

`.ai/` contains routing, query packs, and generated context indexes. It does not contain the only copy of a product or architecture rule.

Agent behavior is governed by root `AGENTS.md` and canonical docs, independent of model vendor.

## 10. User project layout

An Ori application uses a root-first project:

```text
my-project/
├── ori.proj
├── main.orl
├── domain-a/
│   └── module.orl
├── domain-b/
│   └── module.orl
└── docs/
    └── optional sidecars
```

Rules:

- `ori.proj` is at the project root;
- an application entry defaults or points to a file such as `main.orl`;
- `src/` is optional, not required by the language;
- domain folders are optional;
- module declarations and project namespaces follow the project contract;
- documentation sidecars may mirror source domains;
- generated build state belongs under `.ori/` or another documented tool-owned directory;
- dependency identity is isolated by package.

## 11. Library and package projects

A library/project manifest explicitly declares its kind, version, entry/root behavior, and dependencies according to the current project/package specifications.

Publishable package metadata must not be confused with an application's ordinary project manifest when separate files/contracts apply.

## 12. Generated and ignored content

Generated content should have:

- a documented producer;
- deterministic output where practical;
- a clear version-control policy;
- no hidden source-of-truth role;
- cleanup behavior.

Large local references, downloaded comparison repositories, build outputs, package caches, and temporary benchmark results remain ignored unless an explicit evidence policy says otherwise.

## 13. Layout change policy

A structural change requires:

- reason and affected workflows;
- path migration and compatibility strategy;
- updates to scripts, CI, docs, links, package builders, and editor tooling;
- tests for path discovery;
- ADR when the structure establishes a durable new boundary;
- ATLAS/catalog update.

Do not reorganize paths only for visual symmetry when it creates broad link and tooling churn without a functional benefit.