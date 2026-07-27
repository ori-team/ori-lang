# Documentation archive

The archive preserves historical evidence without allowing obsolete material to compete with current contracts.

## Categories

```text
docs/archive/
  plans/             completed or cancelled execution plans
  audits/            dated architecture, implementation, or quality audits
  investigations/    bug checks, experiments, performance and design investigations
  sessions/          temporary resume points and session notes worth preserving
  legacy/            obsolete product identities, retired syntax, and superseded structures
```

Existing files directly under `docs/archive/` and `docs/planning/historico/` should migrate gradually into these categories.

## Archive header

Archived files should begin with a notice containing:

- status: archived;
- original date;
- reason for archival;
- current replacement or canonical source;
- warning that commands, versions, syntax, and links may be obsolete.

## Rules

- Archived content is not deleted merely because it is old.
- Active documents must not depend on archived instructions for current behavior.
- Historical claims may be preserved, but obsolete product identity must not appear as the current project identity.
- Completed plans move here after their outcome is recorded.
- Superseded decisions remain in the decision log rather than being hidden in the general archive.
- Moving a file requires updating inbound links.
- Do not keep duplicate full copies in both active and archived locations.

## Migration from `docs/planning/historico/`

Classify each file by purpose, not only by its original directory:

- PR plans and maturity plans → `archive/plans/`;
- bugcheck, benchmark, or research notes → `archive/investigations/`;
- documentation or implementation audits → `archive/audits/`;
- machine-switch/resume notes → `archive/sessions/`;
- retired product/syntax material → `archive/legacy/`.

The migration should be performed in focused batches with link validation rather than one unreviewable mass move.

## Finding current information

Start from [`../ATLAS.md`](../ATLAS.md). The ATLAS identifies current canonical documents and should always take precedence over archive content.