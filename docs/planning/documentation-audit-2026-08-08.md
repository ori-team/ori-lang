# Ori documentation audit — 2026-08-08

> **Status:** completed audit and first synchronization pass.
> **Canonical coverage map:** [`../ATLAS.md`](../ATLAS.md) and
> [`../atlas/features.yaml`](../atlas/features.yaml).
> **Scope:** compiler, runtime, tests, examples, language specification, user
> guides, CLI/install docs, editor docs, and planning metadata.

## Executive summary

The user-facing documentation now follows the implemented S3 language surface,
local-inference option B, workspace `0.3.8-dev`, latest release `v0.3.7`, and
ABI contract `ori-native-abi-1`. The audit catalogued **21 feature/tooling
areas**: **11 stable**, **1 implemented**, and **9 partial**. Partial means
that the native path exists but a documented restriction remains; it does not
mean that the syntax is aspirational.

The main accuracy problems were version drift, a false statement that chapter
03 did not exist, an ABI section that predated aggregate bridges, a stale
stdlib architecture description, and planning pages that mixed current work
with historical or external package experiments. Those discrepancies are now
explicitly corrected or labeled historical.

## Coverage matrix

The YAML registry is the source of exact paths. This table is the review index;
`yes` means the registry has at least one path in that column.

| Feature | Implemented | Tests | User guide | Reference | Example | Status |
|---|---:|---:|---:|---:|---:|---|
| Lexical syntax and literals | yes | yes | yes | yes | yes | stable |
| Modules, imports, visibility | yes | yes | yes | yes | yes | stable |
| Bindings, mutability, inference | yes | yes | yes | yes | yes | stable |
| Primitive and composite types | yes | yes | yes | yes | yes | stable |
| Arrays and slices | yes | yes | yes | yes | no | partial |
| Aliases and newtypes | yes | yes | yes | yes | yes | stable |
| Functions and contracts | yes | yes | yes | yes | yes | stable |
| Closures and captures | yes | yes | yes | yes | yes | stable |
| Control flow and cleanup | yes | yes | yes | yes | yes | stable |
| Pattern matching | yes | yes | yes | yes | yes | stable |
| Generics and constraints | yes | yes | yes | yes | yes | partial |
| Const generics | yes | yes | yes | yes | yes | partial |
| Traits and associated items | yes | yes | yes | yes | yes | partial |
| Optional/result errors | yes | yes | yes | yes | yes | stable |
| Async, tasks, channels, atomics | yes | yes | yes | yes | yes | partial |
| Inline iter generators | yes | yes | yes | yes | yes | partial |
| ARC and resource cleanup | yes | yes | yes | yes | yes | partial |
| Declaration attributes | yes | yes | yes | yes | yes | partial |
| C FFI and native ABI | yes | yes | yes | yes | yes | partial |
| Standard library | yes | yes | yes | yes | yes | implemented |
| CLI and project tooling | yes | yes | yes | yes | yes | stable |
| LSP, editors, DAP | yes | yes | yes | yes | yes | partial |

## Major gaps and current limitations

- Fixed arrays are implemented inline but reject managed element types; the
  C/debug backend does not have full parity.
- Generic traits, higher-kinded constructors, and symbolic arithmetic over const
  parameters remain restricted. The docs show only accepted forms.
- Async/concurrency is a native state-machine feature. The C/debug backend
  rejects async, tasks, channels, atomics, and native networking.
- `@inline`, `@no_inline`, and `@cfg` are validated and stored but do not yet
  change optimization or conditional inclusion.
- ABI-1 intentionally keeps collection layouts private. Direct collections,
  callbacks, nested sum bridges, generic/empty structs, and dynamic collection
  handles are outside the direct export contract.
- Zed currently provides LSP integration; automatic debugger wiring is a VS Code
  capability only.

These are implementation-backed limitations, not missing promises hidden in a
tutorial. They are linked from the relevant Atlas entries and specifications.

## Incorrect or outdated documentation corrected

- `docs/spec/01-overview.md` now links the real `03-grammar.ebnf` chapter.
- `docs/spec/19-abi.md` and the book explain scalar-struct pointer/out bridges,
  opaque managed handles, and direct `optional`/`result` bridges.
- `docs/spec/15-stdlib-maintenance.md` now describes Layer 1 Rust plus Layer 2
  and Layer 3 `.orl` modules instead of claiming the manifest was the whole
  standard library.
- README/install/extension pages distinguish the compiler workspace (`0.3.8-dev`)
  from extension artifacts (`0.3.5`) and the latest release (`v0.3.7`).
- Linker documentation follows the real order: bundled `rust-lld`, system
  linker, then the legacy Rust driver fallback.
- Generated book outputs and historical performance numbers are no longer
  presented as checked-in/current artifacts.
- Completed or external package plans are separated from the active compiler
  backlog; cancelled game/imgui material was moved under `planning/historico/`.

No current user guide was found to describe a compiler feature that does not
exist. Historical package/web plans remain available only with an explicit
historical or external-scope label.

## Documentation architecture

The maintained structure is:

1. `docs/language/` — progressive language teaching;
2. `docs/guides/` — installation, CLI, stdlib, debugging, testing, and recipes;
3. `docs/spec/` — English normative syntax, semantics, backend, diagnostics,
   project, memory, and ABI contracts;
4. `docs/atlas/` — machine-readable implementation/test/documentation mapping;
5. `docs/planning/` — maintainer plans, with `historico/` for closed snapshots;
6. `docs/book/` — Portuguese narrative draft, never a replacement for `spec/`.

This keeps tutorial text, normative rules, implementation planning, and
historical evidence from competing as sources of truth.

## Example validation strategy

`tools/qa/docs_examples.sh` now performs the repeatable baseline:

```text
documentation paths → Atlas path check
canonical .orl files → check with the real Ori compiler
project/sidecar docs → ori doc check
```

The 23 canonical example directories pass. Markdown snippets remain readable
teaching material; when a snippet becomes a compatibility contract, promote it
to a standalone file under `examples/` so CI can compile it without maintaining
two copies.

## Machine-readable mapping

`docs/atlas/features.yaml` uses stable feature IDs and records implementation,
tests, user docs, references, examples, status, and limitations. It is
deliberately path-based and dependency-free. `tools/qa/docs_coverage.sh` checks
every referenced path and rejects stale canonical spellings. Future compiler
changes should update the registry in the same change as the implementation
and regression test; no second knowledge graph is needed yet.

## Completed refactoring plan

1. **Source of truth:** created `docs/ATLAS.md`, the registry, and the current
   version/baseline contract.
2. **Accuracy pass:** corrected syntax, versions, linker/install behavior,
   ABI, stdlib layering, links, and status labels.
3. **Coverage pass:** added advanced-language, concurrency, interop, stdlib,
   debugging, and bootstrapping guides with Portuguese siblings.
4. **Validation pass:** added Atlas/path and example/`ori doc` QA scripts and
   connected the inexpensive Atlas gate to `daily_fast.sh`.
5. **Maintenance pass:** recorded the audit here and added the changelog entry.

## Validation executed

- `sh tools/qa/docs_coverage.sh` — passed.
- `sh tools/qa/docs_examples.sh` — 23 examples, 0 failures; `ori doc check` passed.
- `cargo test -p ori-driver --test doc_export --test diagnostic_catalog --test summary --test doctor --quiet` — passed.
- `cargo test -p ori-driver --test ori_spec -- --quiet` — **238 passed**.
- `cargo test -p ori-driver --test multifile_imports -- --quiet` — **364 passed**.
- `sh tools/qa/daily_fast.sh` — passed, including workspace check, strict clippy,
  frontend tests, diagnostics, ARC/security, and residual product gates.
- `git diff --check` — passed.
