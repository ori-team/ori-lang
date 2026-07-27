---
id: ADR-0001
title: Adopt the S3 language surface
status: accepted
date: 2026-07-12
deciders: [project-maintainer]
supersedes: []
superseded_by: []
related_docs:
  - docs/spec/01-overview.md
  - docs/spec/18-stability-and-compatibility.md
  - docs/product/accessibility-principles.md
related_code:
  - compiler/crates/ori-lexer
  - compiler/crates/ori-parser
  - compiler/crates/ori-types
  - compiler/crates/ori-hir
  - compiler/crates/ori-driver
---

# ADR-0001: Adopt the S3 language surface

## Context

Early Ori already had a substantial native compiler, async/runtime behavior, traits, standard-library work, LSP support, and diagnostics, but its surface accumulated several forms and legacy spellings.

A separate syntax laboratory, Auk9, explored a more reading-oriented visual rhythm. Maintaining two products or reimplementing the mature compiler under the laboratory was not justified.

Ori needed one canonical surface that preserved its implementation and semantics while reducing syntactic alternatives and making important information easier to recognize.

## Decision drivers

- one canonical form per concept;
- lower reading and migration ambiguity;
- preserve the existing compiler/runtime investment;
- keep `.orl`, the `ori` CLI, and `ori.*` standard-library identity;
- make removed forms fail explicitly;
- support formatter, LSP, diagnostics, and automated migration;
- avoid maintaining a second language product.

## Considered options

### Keep the pre-S3 surface

Rejected because it preserved inconsistent legacy forms and did not meet the reading-first direction.

### Develop the syntax laboratory as a separate product

Rejected because it would duplicate the compiler, runtime, stdlib, tooling, documentation, and compatibility burden.

### Adopt the laboratory surface unchanged

Rejected because some forms conflicted with existing Ori semantics, readability decisions, or established features.

### Adopt an Ori-specific S3 surface

Selected: absorb the useful reading-oriented ideas while keeping Ori semantics, implementation, identity, and explicitly chosen exceptions.

## Decision

Ori adopts the S3 surface as its canonical syntax.

The cut was introduced in version `0.3.0`; the current project version is `0.3.8`.

Key decisions include:

- `module path` for source identity;
- functions without a declaration `func` keyword;
- `public` visibility;
- types such as `list[T]` and `result[T, E]`;
- `import path = alias`, with path on the left and local alias on the right;
- `try expression` for propagation;
- `apply Type` and `use Trait` for trait behavior;
- `(args) => expression` closure syntax;
- canonical `end`-delimited blocks;
- one supported spelling for removed legacy forms;
- local inference limited to documented rules;
- retention of the pipe operator as a typed call form.

The syntax laboratory is retired as a product. It remains historical design evidence only.

## Consequences

### Positive

- One language product and one implementation.
- One canonical active syntax for documentation and diagnostics.
- Formatter, LSP, examples, and migration can target a single surface.
- The native compiler/runtime remains the foundation.
- Reading-oriented goals have a concrete syntax baseline.

### Negative

- The initial S3 cut was intentionally incompatible with older source.
- Parser, formatter, examples, stdlib, docs, and tooling required coordinated migration.
- Removed forms require dedicated diagnostics and maintenance of migration assistance.
- A visually distinctive surface can still create parser ambiguity if additions are not reviewed carefully.

## Invariants established

- Active examples use current S3 syntax.
- Removed forms are rejected rather than accepted as parallel syntax.
- New syntax must be evaluated against accessibility and one-canonical-form principles.
- The specification, formatter, LSP, diagnostics, and migration tool remain synchronized.
- Historical introduction versions do not replace `0.3.8` as the current version.

## Compatibility and migration

The original cut was a breaking pre-1.0 change. `ori migrate-syntax` provides best-effort mechanical rewriting for supported forms. Semantic migrations still require human review.

Future incompatible changes to the S3 contract follow the language-evolution and RFC process.

## Validation

Validation includes:

- lexer/parser tests for current and removed forms;
- `ori_spec` conformance tests;
- formatter idempotence and syntax goldens;
- migration tests;
- AOT/JIT execution examples;
- diagnostic catalog coverage;
- current specification and active example audits.

## Reconsideration criteria

Individual S3 rules may be reconsidered only through a concrete user/engineering problem, an RFC for significant public change, compatibility analysis, accessibility review, migration design, and full tooling/conformance evidence.