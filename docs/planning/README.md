# Planning compatibility index

> Status: **current compatibility router**  
> Current project version: **0.3.8**

The canonical planning policy lives in [`../plans/README.md`](../plans/README.md).

## Current open work

[`BACKLOG.md`](BACKLOG.md) is the canonical open-work list.

Do not create a second backlog or treat completed plans as current work.

## Where planning material belongs

| Material | Canonical destination |
|---|---|
| Open implementation outcome | `BACKLOG.md` and a GitHub issue |
| Complex accepted multi-PR work | `../plans/active/` |
| Durable architecture decision | `../decisions/adr/` |
| Significant public proposal | `../rfcs/` |
| Completed/cancelled plan | `../archive/plans/` |
| Investigation or benchmark note | `../archive/investigations/` |
| Audit snapshot | `../archive/audits/` |
| Session/resume note | `../archive/sessions/` |
| Retired identity or obsolete design | `../archive/legacy/` |

## Compatibility files retained here

- [`BACKLOG.md`](BACKLOG.md) — current open-work list.
- [`PENDENTES.md`](PENDENTES.md) — retired compatibility pointer; not a backlog.
- `adr-*.md` — concise pointers to numbered ADRs under `docs/decisions/adr/`.
- [`repo-and-project-layout.md`](repo-and-project-layout.md) — pointer to current architecture and ADR.
- [`qa/test-matrix-ori.md`](qa/test-matrix-ori.md) — transitional detailed quality matrix.

The former `historico/` tree was fully migrated. See [`../archive/MIGRATION_REPORT.md`](../archive/MIGRATION_REPORT.md).

## Rules

1. Planning never overrides the normative specification.
2. Completed work must not remain listed as active.
3. Historical implementation detail belongs in the archive, changelog, or a decision record—not in the current backlog.
4. New complex plans use [`../templates/EXEC_PLAN.md`](../templates/EXEC_PLAN.md).
5. A plan links the contract it implements, affected code, required tests, and completion evidence.
6. Closing a PR updates the corresponding issue, backlog item, plan checkpoint, and documentation state.

Use [`../ATLAS.md`](../ATLAS.md) to find current product, architecture, implementation, quality, security, and operations documents.
