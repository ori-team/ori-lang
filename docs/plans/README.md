# Planning documentation

Planning documents describe open work and the execution of specific complex changes. They are not normative language or architecture contracts.

## Canonical structure

```text
docs/plans/
  README.md
  active/
    <feature-or-refactor>-exec-plan.md
```

[`../planning/BACKLOG.md`](../planning/BACKLOG.md) is the canonical open-work list. Completed detail belongs in the archive, changelog, architecture, decisions, or tests—not in the active backlog.

## Active plans

There are currently no active ExecPlans.

The completed documentation-history migration is preserved at:

- [`../archive/plans/documentation-history-migration.md`](../archive/plans/documentation-history-migration.md)
- [`../archive/MIGRATION_REPORT.md`](../archive/MIGRATION_REPORT.md)

Add a plan here only when complex accepted work actually begins.

## Backlog policy

The backlog contains only work that remains open or deliberately deferred.

Each item should include:

- stable ID;
- problem/outcome;
- priority;
- estimated size;
- dependencies;
- current status;
- issue or plan link;
- completion evidence requirement.

Do not keep long implementation histories, completed checklists, session notes, or copied design documents in the active backlog.

## When to create an ExecPlan

Use an ExecPlan when a change:

- spans several components or repositories;
- must preserve behavior through risky refactoring;
- needs staged migration;
- requires checkpoints and rollback strategy;
- will take several PRs;
- contains non-obvious dependencies;
- needs explicit evidence at each phase.

Do not create an ExecPlan for a small feature or bug that fits one focused PR.

## ExecPlan content

- objective and user-visible result;
- current state and evidence;
- accepted contracts and decisions;
- in scope and out of scope;
- affected code and documents;
- risks and invariants;
- phases and PR slices;
- validation commands and evidence;
- compatibility, migration, security, and performance impact;
- rollback and recovery;
- completion criteria;
- decision log for execution-only choices.

An ExecPlan does not redefine accepted language semantics. Design changes return to the RFC/ADR process.

## Status

Active plan statuses:

- `draft`
- `ready`
- `in_progress`
- `blocked`
- `completed`
- `cancelled`

Completed and cancelled plans move to `docs/archive/plans/` with their final status, date, and outcome.

## Historical migration

The former `docs/planning/historico/` tree was migrated into `docs/archive/` by category:

- completed plans → `archive/plans/`;
- investigations and bug checks → `archive/investigations/`;
- audit snapshots → `archive/audits/`;
- session/resume notes → `archive/sessions/`;
- obsolete product identity and experiments → `archive/legacy/`.

The migration moved 51 documents, archived its ExecPlan, rewrote relative links, generated category indexes, and removed the transitional historical root. See the [migration report](../archive/MIGRATION_REPORT.md).

## Relationship to issues and PRs

- Issue: operational unit and status.
- ExecPlan: multi-stage execution model.
- PR: reviewable delivered slice.
- Backlog: ordered open outcomes.

A completed PR should close or update its backlog item and plan checkpoint. Planning state that does not reflect merged work is documentation drift.
