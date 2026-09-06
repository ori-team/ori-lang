# Ori project start

This file is the shortest reliable entry point for contributors, maintainers, and AI agents.

## Project identity

Ori is a reading-first, explicitly typed programming language compiled to native code. The compiler is written in Rust and the current canonical project version is **0.3.8**.

Ori prioritizes:

- readable source code and diagnostics;
- explicit contracts over hidden behavior;
- native AOT compilation, with JIT available for `ori run`;
- a documented runtime and ABI;
- accessibility for people who benefit from lower cognitive load;
- compiler study and AI-assisted development.

## Read in this order

1. [`docs/ATLAS.md`](docs/ATLAS.md) — canonical map of documents, code, tests, and decisions.
2. [`AGENTS.md`](AGENTS.md) — mandatory operational rules and validation gates.
3. [`docs/product/status.md`](docs/product/status.md) — current product and version status.
4. [`docs/architecture/overview.md`](docs/architecture/overview.md) — system boundaries and component map.
5. [`docs/implementation/standards.md`](docs/implementation/standards.md) — implementation rules.
6. The domain document related to the change.

Do not begin with archived plans or historical snapshots. They explain past work but do not define the current system.

## Repository map

```text
compiler/       Rust workspace: frontend, HIR, codegen, runtime, LSP, CLI
stdlib/         Ori standard library source and sidecar documentation
runtime/        staged native runtime artifacts and link metadata
examples/       executable Ori projects and conformance examples
docs/           product, architecture, contracts, implementation, quality, security, plans
tests/          repository-level fixtures and test documentation
extensions/     local editor integrations
tools/          QA, packaging, release, benchmark, and documentation tooling
```

The Cargo workspace is in `compiler/`, not at the repository root.

## Common commands

From the repository root:

```bash
cargo check --manifest-path compiler/Cargo.toml --workspace
cargo test --manifest-path compiler/Cargo.toml --workspace
cargo test --manifest-path compiler/Cargo.toml -p ori-driver --test diagnostic_catalog
cargo test --manifest-path compiler/Cargo.toml -p ori-lsp
```

Fast project validation:

```bash
sh tools/qa/daily_fast.sh
```

Run an example through the development compiler:

```bash
cargo run --manifest-path compiler/Cargo.toml -p ori-driver -- check examples/hello/main.orl
cargo run --manifest-path compiler/Cargo.toml -p ori-driver -- run examples/hello/main.orl
```

## Before changing anything

Record or understand:

- the user-visible problem;
- the component that owns the behavior;
- the canonical document for the subject;
- the public contract and compatibility impact;
- required positive, negative, regression, and parity tests;
- runtime, ABI, security, performance, packaging, and documentation impact;
- whether the change requires an ADR, RFC, or execution plan.

Prefer the smallest vertical change that can be reviewed and validated independently.

## Source-of-truth order

When sources conflict, use this order:

1. security and data-integrity requirements;
2. accepted product requirements and current compatibility policy;
3. normative language, ABI, project, and runtime contracts;
4. accepted ADRs and RFCs;
5. current architecture and implementation documents;
6. `AGENTS.md` operational rules;
7. automated conformance and regression tests;
8. implementation behavior;
9. planning documents;
10. archived material and old examples.

A conflict between a higher-level contract and implementation is a defect. Do not silently rewrite the contract to match accidental behavior.

## Definition of done

A change is complete only when:

- the implementation is focused and follows the owning component's boundaries;
- relevant tests pass;
- new behavior has regression coverage;
- diagnostics remain catalogued and actionable;
- public behavior updates the normative specification and user documentation;
- architecture or decisions are updated when their contracts change;
- compatibility, ABI, security, performance, and release effects are recorded;
- the changelog is updated when users can observe the change;
- completed plans are closed or archived rather than left active.

## Where work belongs

- Current language contract: `docs/spec/`
- Product direction and current status: `docs/product/`
- Current system design: `docs/architecture/`
- Implementation rules and how-to guides: `docs/implementation/`
- Tests, diagnostics, performance, and conformance: `docs/quality/`
- Security engineering: `docs/security/`
- Decisions and proposals: `docs/decisions/`, `docs/rfcs/`
- Active complex execution work: `docs/plans/active/`
- Operational procedures: `docs/operations/`
- Historical evidence: `docs/archive/`

Start from the ATLAS whenever the correct location is unclear.