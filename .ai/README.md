# AI context directory

This directory is reserved for machine-oriented navigation and context packs. It must not duplicate canonical product or architecture documentation.

## Rules

- Human-readable canonical sources remain under `docs/` and root project files.
- Machine files point to canonical sources rather than copying full prose.
- Generated indexes must be reproducible.
- Context packs are task-specific and small.
- Agent-specific instructions must not conflict with `AGENTS.md`.
- No model vendor is treated as the sole supported agent environment.

## Intended structure

```text
.ai/
  README.md
  context-packs/
  queries/
  generated/
```

## Context-pack examples

- frontend syntax change;
- runtime/ARC investigation;
- stdlib operation delivery;
- diagnostic design and catalog update;
- package/release change;
- documentation audit.

Each pack should contain references to:

- relevant canonical documents;
- owning code paths;
- required tests;
- invariants;
- common failure modes;
- validation commands.

The source for document relationships is `docs/catalog.yaml` and `docs/ATLAS.md`.