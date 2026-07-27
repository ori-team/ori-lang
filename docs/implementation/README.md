# Implementation documentation

Implementation documents explain how to change the current system safely. They complement architecture and specification; they do not replace either.

## Canonical documents

- [`standards.md`](standards.md) — repository-wide implementation standards.
- [`feature-delivery.md`](feature-delivery.md) — vertical delivery checklist.
- [`compiler/adding-syntax.md`](compiler/adding-syntax.md) — end-to-end syntax implementation guide.
- Component-specific source READMEs — local build and module details.

## Boundary rules

- Specification defines accepted behavior.
- Architecture defines current component boundaries.
- Implementation docs define the approved way to modify those components.
- ADRs explain durable decisions.
- Plans sequence a specific complex change.

An implementation document should be updated when the supported extension path, required tests, ownership model, or phase contract changes.