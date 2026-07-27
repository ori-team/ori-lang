# Ecosystem execution branches — consolidation record

> Status: **archived**  
> Archived on: 2026-07-27  
> Category: `plans`  
> Source branches:
> - `execute-plan/b927af56-pr-10-phase-os-scaffolding-last`
> - `execute-plan/5b7bfbb0-pr-1-maturity-5-plan-lock-in`
> Current replacement: [`../../planning/BACKLOG.md`](../../planning/BACKLOG.md), [`../../product/support-matrix.md`](../../product/support-matrix.md), and the sibling ecosystem repositories  
> Warning: paths, package versions, maturity scores, priorities, and operating-system claims below are historical evidence and may be obsolete.

## Why these branches were not applied as current documentation

Both branches were created from commit `832d1809ad23391b2c9461f2397c4c43f2fc7afa`, long before the canonical documentation framework and the current `0.3.8` project state.

The newer branch, `execute-plan/5b7bfbb0-pr-1-maturity-5-plan-lock-in`, contains the older `execute-plan/b927af56-pr-10-phase-os-scaffolding-last` branch and five additional commits. Merging both independently would duplicate the same history.

Their documents primarily describe external sibling repositories such as game, UI, physics, asset, compression, networking, and database packages. Those projects are explicitly outside the Ori compiler repository's active core backlog unless separately accepted through governance.

Applying the branch tree directly would have:

- recreated completed plans under `docs/planning/`;
- reintroduced machine-specific absolute paths;
- presented dated Linux/Windows/macOS claims as current product status;
- competed with the canonical backlog, support matrix, archive policy, and ATLAS;
- conflicted with the completed historical-document migration.

## Preserved branch content

The branch history preserves the following documents and their original revisions:

- `docs/planning/PHASE-OS.md`;
- `docs/planning/eco-library-ports-catalog.md`;
- `docs/planning/eco-packages-status.md`;
- `docs/planning/game-ports-maturity-matrix.md`;
- `docs/planning/pr-plan-eco-maturity-5.md`;
- `docs/planning/pr-plan-eco-ports-e2e.md`;
- `docs/planning/pr-plan-imgui-tools-maturity-5.md`;
- related changes to `CHANGELOG.md`, `docs/planning/README.md`, and `docs/planning/package-ecosystem-guidelines.md`.

The material records:

- maturity criteria for sibling native packages;
- Linux-first validation decisions;
- deferred Windows/macOS scaffolding;
- package/version inventories;
- game and ImGui ecosystem plans;
- external repository layout and smoke-test conventions.

## Merge resolution

The branch histories are merged for traceability, but the canonical documentation tree remains the resolution of the merge.

No dated ecosystem plan is restored as active work. Relevant future work must be reopened in the responsible sibling repository or proposed through the Ori governance process when it changes the core compiler, language, package contracts, or supported platform surface.

## Current routing

| Historical concern | Current source |
|---|---|
| Core open work | [`../../planning/BACKLOG.md`](../../planning/BACKLOG.md) |
| Backend/platform support | [`../../product/support-matrix.md`](../../product/support-matrix.md) |
| Package and project contracts | [`../../spec/17-project-and-docs.md`](../../spec/17-project-and-docs.md) |
| Stability and compatibility | [`../../spec/18-stability-and-compatibility.md`](../../spec/18-stability-and-compatibility.md) |
| Supply-chain expectations | [`../../security/supply-chain.md`](../../security/supply-chain.md) |
| Significant ecosystem proposal | [`../../governance/rfc-process.md`](../../governance/rfc-process.md) |
| Historical evidence policy | [`../README.md`](../README.md) |

## Outcome

- The older execution branch is treated as an ancestor of the newer branch.
- The newer branch is merged once, preserving both histories.
- Current documentation remains internally consistent and versioned for Ori `0.3.8`.
- External ecosystem work is not silently promoted into the compiler repository's active scope.
