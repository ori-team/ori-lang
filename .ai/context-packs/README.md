# Context packs

A context pack is a compact, task-specific routing document for an AI agent or contributor. It reduces broad repository searches without becoming a second source of truth.

## Required fields

- task type;
- canonical documents;
- code ownership paths;
- invariants;
- required tests and commands;
- security/performance/compatibility checks;
- known traps;
- expected completion evidence.

## Rules

- Link to canonical prose; do not paste full documents.
- Keep packs portable across ChatGPT, Codex, Claude, Gemini, and other agents.
- Do not encode hidden permissions or autonomous merge rules.
- Update packs when paths or canonical documents change.
- Generated packs belong in `.ai/generated/`; maintained packs may live here.

## Example outline

```yaml
id: runtime-arc-change
canonical_docs:
  - docs/architecture/runtime-and-memory.md
  - docs/architecture/invariants.md
  - docs/security/unsafe-code-policy.md
code:
  - compiler/crates/ori-runtime
  - compiler/crates/ori-codegen
required_tests:
  - cargo --manifest-path compiler/Cargo.toml test -p ori-runtime
  - cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test memory_arc
invariants:
  - single-cascade-owner
  - aot-jit-parity
```

Context packs may be generated from `docs/catalog.yaml` once documentation validation tooling is implemented.