# Implementation documentation

Implementation documents explain how to change the current system safely. They complement architecture and specification; they do not replace either.

## Canonical documents

- [`standards.md`](standards.md) — repository-wide implementation standards.
- [`feature-delivery.md`](feature-delivery.md) — vertical delivery checklist.
- [`compiler/adding-syntax.md`](compiler/adding-syntax.md) — end-to-end syntax implementation guide.
- [`compiler/type-checker.md`](compiler/type-checker.md) — semantic-checker state, rule ownership, and refactoring path.
- [`runtime/refactoring.md`](runtime/refactoring.md) — incremental runtime modularization with ABI/symbol preservation.
- [`stdlib/adding-api.md`](stdlib/adding-api.md) — Layer 1/2/3 standard-library delivery path.
- Component-specific source READMEs — local build and module details.

## Boundary rules

- Specification defines accepted behavior.
- Architecture defines current component boundaries.
- Implementation docs define the approved way to modify those components.
- ADRs explain durable decisions.
- RFCs evaluate significant proposals.
- Plans sequence a specific complex accepted change.

An implementation document should be updated when the supported extension path, required tests, ownership model, unsafe boundary, or phase contract changes.