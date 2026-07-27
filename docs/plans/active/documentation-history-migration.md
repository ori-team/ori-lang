---
id: DOC-MIGRATE-1
title: Migrate historical documentation into the canonical archive
status: ready
owners: [project-maintainer]
created: 2026-07-27
updated: 2026-07-27
related_issues: []
related_adrs: []
related_rfcs: []
---

# Migrate historical documentation into the canonical archive

## Objective

Move remaining historical documents from `docs/planning/historico/`, loose `docs/archive/` paths, and completed planning files into categorized archive directories without losing unique evidence or breaking current links.

This plan does not redesign current product, architecture, specification, or implementation content. Those canonical domains are already established.

## Current state

The documentation framework now defines:

```text
docs/archive/
├── plans/
├── audits/
├── investigations/
├── sessions/
└── legacy/
```

However, many older files remain under:

- `docs/planning/historico/`;
- loose paths directly below `docs/archive/`;
- `docs/planning/` even though their work is completed or their decision has migrated.

Important competing sources have already been handled:

- `PENDENTES.md` is retired;
- `BACKLOG.md` contains open/deferred outcomes only;
- key ADRs are under `docs/decisions/adr/`;
- the old full maturity plan is replaced by an archive summary/pointer;
- current layout architecture has a canonical document and ADR.

## In scope

- inventory historical files and inbound links;
- classify each file by purpose;
- add archive metadata notices;
- move files in small domain batches;
- replace high-traffic old paths with compatibility pointers when useful;
- update all maintained inbound links;
- remove duplicate full copies;
- preserve dates, authorship, outcomes, measurements, and unique rationale;
- ensure retired project identity remains absent;
- validate archive/current-source boundaries.

## Out of scope

- rewriting every historical statement to current terminology;
- making archived commands or syntax executable;
- changing language/compiler/runtime behavior;
- reopening rejected or completed work;
- moving accepted ADRs into the general archive;
- treating historical version numbers as documentation errors when clearly contextual.

## Classification rules

### `archive/plans/`

Use for:

- completed PR plans;
- maturity programs;
- implementation waves;
- completed migration plans;
- cancelled execution plans.

### `archive/audits/`

Use for:

- architecture audits;
- implementation completeness audits;
- documentation audits;
- dated gap analyses;
- quality/security assessment snapshots.

### `archive/investigations/`

Use for:

- bug checks;
- performance investigations;
- external-language studies;
- prototypes and experiments;
- root-cause notes;
- design exploration that did not become a current contract.

### `archive/sessions/`

Use for:

- machine-switch resume points;
- work-session logs;
- temporary operational notes preserved for traceability.

### `archive/legacy/`

Use for:

- retired product identities;
- removed syntax references;
- obsolete repository layouts;
- discontinued tooling/product directions;
- compatibility documents no longer useful as current pointers.

### Decisions

Accepted, rejected, deprecated, or superseded durable decisions belong in `docs/decisions/adr/`, not the general archive.

## Archive notice

Each moved file begins with or links to metadata equivalent to:

```text
Status: archived
Original date: YYYY-MM-DD
Archived date: YYYY-MM-DD
Reason: completed / superseded / investigation closed / legacy
Current replacement: <canonical source or none>
Warning: versions, syntax, commands, and paths may be obsolete
```

Preserve the original title and body after the notice unless a concise summary intentionally replaces a redundant large document. Summarization must preserve unique decisions/evidence or link Git history for the removed detail.

## Risks

| Risk | Impact | Mitigation | Evidence |
|---|---|---|---|
| Broken inbound links | Readers/agents cannot find evidence | Search links before each batch; update/pointer; run docs validator | Link check green |
| Historical file still treated as current | Wrong implementation guidance | Archive notice; ATLAS/catalog rules; remove from active indexes | Current indexes contain canonical paths only |
| Duplicate full copies | Competing sources and drift | Move rather than copy; delete old full file or leave concise pointer | Duplicate-content audit |
| Loss of unique rationale/measurements | Engineering history lost | Preserve body or write explicit summary; use Git history reference | Batch review |
| Huge unreviewable diff | Errors hidden | Small category/topic PRs | Focused PRs and file lists |
| Old identity reintroduced | Public inconsistency | Permanent identity validator | Documentation CI |

## PR sequence

### PR 1 — Inventory and link graph

**Outcome:** Machine-readable inventory of historical files, proposed category, original date/status, current replacement, and inbound references.

**Changes:**

- add an inventory under `docs/archive/` or generated report under `.ai/generated/`;
- detect duplicate names/content and missing targets;
- classify files without moving them yet.

**Validation:**

- every file in transitional historical roots has a category;
- every maintained inbound link is recorded;
- no content changes.

### PR 2 — Completed plans

**Outcome:** Completed/cancelled implementation plans move to `archive/plans/`.

**Changes:**

- move in topic batches;
- add archive notices;
- update planning index/backlog/ATLAS links;
- preserve pointers for frequently referenced legacy paths.

**Validation:**

- docs links green;
- no completed plan remains in active planning index;
- no duplicate full file.

### PR 3 — Audits and investigations

**Outcome:** Dated audits, studies, bug checks, and performance investigations are categorized.

**Validation:**

- current architecture/spec docs link only to investigations when historical evidence is necessary;
- archive notices identify current replacement;
- benchmark evidence remains attributable.

### PR 4 — Sessions and legacy material

**Outcome:** Session logs and obsolete product/syntax/layout material are isolated from current navigation.

**Validation:**

- active docs do not depend on session notes;
- retired identity remains absent;
- removed syntax appears only in clearly archived/migration contexts.

### PR 5 — Remove transitional historical root

**Outcome:** `docs/planning/historico/` is empty/removed or contains only a concise redirect index.

**Changes:**

- update `docs/catalog.yaml` historical roots;
- update ATLAS/archive policy;
- add CI rule preventing new files in the retired path;
- generate final migration report.

**Validation:**

- all links and catalog paths green;
- archive classification complete;
- no active index points to the retired root;
- Git history preserves old paths.

## Automation

Add or extend tooling to report:

- files under retired historical roots;
- missing archive notices;
- active docs linking to archived instructions without an explicit historical context;
- duplicate canonical ownership;
- broken links after moves;
- ADR-like files outside `docs/decisions/`;
- completed plans outside `docs/archive/plans/`;
- session-note naming outside `archive/sessions/`.

Automation should produce actionable file paths and suggested categories, not move files automatically without review.

## Compatibility

Repository documentation paths change, but user source/project/ABI behavior does not.

High-traffic historical links may keep concise compatibility pointers. Low-value paths can rely on GitHub move history when all repository links are updated.

## Security

- Avoid exposing previously redacted tokens or personal paths while moving logs.
- Preserve vulnerability details privately if any historical file contains sensitive security material.
- Permanent identity and link validation remain active.

## Performance

Not applicable to compiler/runtime performance. Documentation CI must remain fast enough for ordinary PRs; inventory/deep duplicate checks may run in a scheduled or focused workflow.

## Completion criteria

- [ ] Every historical file has a category and archive notice.
- [ ] Accepted decisions are in the ADR index.
- [ ] Completed plans are outside active planning.
- [ ] `docs/planning/historico/` is removed or reduced to a redirect.
- [ ] Current docs do not depend on archived instructions.
- [ ] Inbound links are updated or protected by pointers.
- [ ] No duplicate full copies remain.
- [ ] ATLAS/catalog/archive policy reflect final paths.
- [ ] Documentation CI blocks regression.
- [ ] Migration report records moved files and intentional exclusions.

## Final outcome

Complete this section after the final migration PR with links, counts, residual exclusions, and validation results.