# Architecture decisions

This directory is the canonical home for durable Architecture Decision Records (ADRs).

## What belongs in an ADR

Use an ADR when a decision:

- changes or establishes a long-lived component boundary;
- defines ownership, lifecycle, ABI, storage, package, or target behavior;
- selects between meaningful alternatives;
- constrains future implementation;
- would be difficult to infer from code alone;
- is likely to be revisited without a written rationale.

Do not use an ADR for ordinary implementation steps, task tracking, meeting notes, or a list of future ideas.

## Directory

```text
docs/decisions/
  README.md
  TEMPLATE.md
  adr/
    0001-short-title.md
```

Existing ADR-like files under `docs/planning/` should be migrated without rewriting their historical rationale. Add front matter, assign a number, update links, and mark the old path as moved or archive it.

Initial migration candidates include the accepted S3 surface decision and ARC ownership decisions.

## Status values

- `proposed`
- `accepted`
- `rejected`
- `deprecated`
- `superseded`

A superseded ADR remains in the repository and links to its replacement.

## Required content

- context;
- decision;
- alternatives considered;
- consequences;
- affected contracts and code;
- validation/evidence;
- status and date;
- superseding/superseded relationships.

## ADR versus architecture

The ADR explains why a choice was made. Current architecture documents explain how the system works now.

When implementation evolves without changing the decision, update architecture only. When the decision itself changes, add a new ADR and supersede the old one.

## ADR versus RFC

An RFC evaluates a significant public proposal. An ADR records a durable system decision. A language RFC may result in ADRs for parser, runtime, or ABI design.

## Index policy

Every ADR should be listed here after migration:

| ID | Title | Status | Date | Supersedes |
|---|---|---|---|---|
| — | Migration pending for existing accepted decisions | — | — | — |

The index must be updated in the same PR that adds or changes an ADR status.