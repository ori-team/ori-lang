# Product documentation

This domain defines what Ori is, who it serves, its current maturity, and how public compatibility is communicated.

## Canonical documents

- [`status.md`](status.md) — current implementation and ecosystem status.
- [`versioning.md`](versioning.md) — version numbers and compatibility policy.
- [`accessibility-principles.md`](accessibility-principles.md) — readability and cognitive-accessibility requirements.
- [`../spec/00-manifesto.md`](../spec/00-manifesto.md) — project purpose and values.
- [`../spec/18-stability-and-compatibility.md`](../spec/18-stability-and-compatibility.md) — normative stability contract.

## Product statement

Ori is a reading-first, explicitly typed programming language compiled to native code. It is designed as:

- a serious compiler-engineering project;
- a language for learning and building small to medium native programs;
- an environment for AI-assisted programming with visible contracts;
- a language and documentation experiment focused on reducing cognitive load.

Ori is pre-1.0. The current version is **0.3.8**. Pre-1.0 does not mean undocumented or arbitrary: public contracts, experimental areas, compatibility rules, and known limitations must be explicit.

## Product boundaries

The core repository owns:

- the compiler and command-line interface;
- native AOT and JIT execution paths;
- runtime and memory management;
- the standard library;
- project and package formats;
- diagnostics, formatter, documentation extraction, LSP, and local editor extensions;
- examples, tests, release tooling, and normative documentation.

External frameworks, game engines, editor suites, and application-specific packages are not automatically part of the core product. They require an explicit decision before entering the repository.

## Documentation boundary

Product documents answer **what and why**. Architecture answers **how the current system is divided**. The specification defines **accepted behavior**. Plans describe **how a specific change will be delivered**.