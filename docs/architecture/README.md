# Architecture documentation

Architecture documents describe the current system and its boundaries. They do not serve as historical decision logs and they do not describe unimplemented designs.

## Canonical documents

- [`overview.md`](overview.md) — repository and subsystem map.
- [`compiler-pipeline.md`](compiler-pipeline.md) — phase boundaries from source to execution.
- [`invariants.md`](invariants.md) — cross-cutting rules that implementation must preserve.
- [`runtime-and-memory.md`](runtime-and-memory.md) — runtime, ARC, cycle collection, FFI, and native ABI relationships.
- [`stdlib.md`](stdlib.md) — standard-library layers and source-of-truth rules.
- [`../planning/repo-and-project-layout.md`](../planning/repo-and-project-layout.md) — accepted repository and Ori project layout.

## Writing architecture documents

Each architecture document should include:

- purpose and scope;
- component responsibilities;
- inputs and outputs;
- allowed dependencies;
- important data flows;
- invariants and failure boundaries;
- extension points;
- related code;
- tests or operational evidence;
- ADRs that explain durable decisions.

Architecture must be updated in the same change when a component boundary, public phase contract, ownership rule, or persistent data flow changes.