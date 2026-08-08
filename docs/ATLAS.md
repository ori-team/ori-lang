# Ori documentation Atlas

This file is the human-readable entry point for the documentation coverage
map. The compiler and its tests are the source of truth; this Atlas records
where each user-visible feature is implemented, tested, explained, and
illustrated.

The machine-readable registry is [`atlas/features.yaml`](atlas/features.yaml).
It is intentionally small and path-based so a future CI check can detect a
feature whose implementation changed without a corresponding documentation
update.

## Current baseline

| Item | Value |
|---|---|
| Language surface | S3 (`0.3.0`) + local inference B (`0.3.1`) |
| Workspace baseline | `0.3.8-dev` |
| Latest released baseline | `v0.3.7` |
| Native ABI | `ori-native-abi-1` |
| Execution | Native AOT; `ori run` may use the staged Cranelift JIT |
| Normative source | [`spec/`](spec/README.md) |
| User guide | [`language/`](language/tour.md) and [`guides/`](guides/README.md) |
| Open implementation list | [`planning/BACKLOG.md`](planning/BACKLOG.md) |

FREEZE-1 closed on 2026-07-19. The workspace has not been bumped to `0.4.0`,
so documents must not describe `0.4` as an already released compiler line.
ABI-1 remains in force.

## How to use the Atlas

When changing a language feature:

1. update the implementation and regression tests;
2. update the corresponding registry entry;
3. update the normative reference and user guide when behavior is public;
4. add or repair a canonical example;
5. run `tools/qa/docs_coverage.sh` and the relevant compiler tests.

Statuses are deliberately conservative:

- `stable`: supported on the documented native path;
- `implemented`: available, but not yet a broad user contract;
- `partial`: restricted by a documented backend or semantic limitation;
- `experimental`: exposed for exploration and allowed to change;
- `unclear`: implementation exists but the public contract is not settled.

The Atlas is not a second language specification. Detailed grammar and
semantics remain in [`spec/`](spec/README.md).
