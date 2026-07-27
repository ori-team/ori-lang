# Governance documentation

Governance documents define how Ori changes its language, architecture, ABI, packages, tooling, and documentation contracts.

## Canonical documents

- [`language-evolution.md`](language-evolution.md) — route and review language changes.
- [`rfc-process.md`](rfc-process.md) — proposal lifecycle and acceptance rules.
- [`../decisions/README.md`](../decisions/README.md) — ADR policy and decision index.
- [`../plans/README.md`](../plans/README.md) — execution-plan policy.
- [`../product/versioning.md`](../product/versioning.md) — compatibility and version classes.

## Governance principle

The amount of process should match the durability and risk of the decision.

- A small bug fix needs an issue and regression test.
- A durable implementation boundary needs an ADR.
- A significant language, ABI, package, or ecosystem proposal needs an RFC.
- A complex accepted change may need an ExecPlan.

Documentation must record the final decision without preserving every discussion as active instruction.