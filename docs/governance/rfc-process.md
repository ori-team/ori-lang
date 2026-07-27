# RFC process

RFCs are used for significant proposals that may change Ori's public language, native ABI, project/package model, standard-library direction, tooling contract, or ecosystem boundary.

## When an RFC is required

Use an RFC for:

- new syntax or major semantic behavior;
- incompatible language changes;
- ABI layout, ownership, or calling-convention changes;
- new package/registry security model;
- durable changes to project or lockfile formats;
- major standard-library naming or layering changes;
- new supported execution model or backend;
- new sandboxing, plugin, macro, or build-script capability;
- changes with large compatibility or teaching cost.

Do not require an RFC for a straightforward defect fix inside an existing contract.

## Location and naming

```text
docs/rfcs/
  0000-template.md
  0001-short-descriptive-name.md
```

RFC numbers are assigned when the proposal is opened for formal review. Draft notes may begin outside the numbered directory but must not be presented as accepted design.

## Required sections

1. **Summary** — one paragraph.
2. **Motivation** — real problem and evidence.
3. **Goals** — outcomes the proposal must achieve.
4. **Non-goals** — nearby behavior intentionally excluded.
5. **Current behavior** — implementation and contract today.
6. **Proposed design** — syntax, semantics, API, or architecture.
7. **Detailed examples** — valid, invalid, and edge cases.
8. **Diagnostics** — expected errors and recovery.
9. **Implementation impact** — phases, runtime, tooling, docs.
10. **Compatibility** — additive, experimental, deprecated, or breaking.
11. **Migration** — tooling and manual steps.
12. **Security** — trust-boundary and abuse impact.
13. **Performance** — expected cost and measurement plan.
14. **Accessibility** — cognitive-load and teaching impact.
15. **Testing and conformance** — required evidence.
16. **Alternatives** — serious alternatives and why rejected.
17. **Unresolved questions** — decisions still needed.
18. **Rollout** — version target, flags, staged delivery.
19. **Decision** — status and final rationale.

## Status values

- `draft`
- `review`
- `accepted`
- `rejected`
- `withdrawn`
- `implemented`
- `superseded`

An accepted RFC is a design decision, not proof that implementation is complete. Mark it implemented only when its completion criteria and documentation are satisfied.

## Review process

### Draft

The author develops the problem statement and complete design dimensions. Informal discussion may refine the proposal.

### Review

Reviewers examine:

- need and evidence;
- language and implementation complexity;
- compatibility;
- accessibility;
- security and performance;
- alternatives;
- completeness of test and migration plans.

Material changes during review are recorded in the RFC rather than hidden in conversation history.

### Decision

The maintainer records:

- accepted/rejected/withdrawn state;
- decision date;
- rationale;
- required follow-up ADR or ExecPlan;
- target version or reconsideration condition.

### Implementation

Accepted work follows the feature-delivery standard. The RFC links to implementation PRs and conformance evidence.

### Completion

The RFC becomes `implemented` only when:

- normative docs are updated;
- implementation and tooling are complete for the accepted scope;
- required tests pass;
- migration and diagnostics exist;
- changelog and status are accurate;
- residual work is explicitly out of scope or tracked.

## Decision quality

An RFC should be rejected or returned to draft when:

- it lacks a real Ori-specific problem;
- a library/tooling solution is clearly sufficient;
- semantics are ambiguous;
- ownership or cleanup is unspecified;
- editor/formatter impact is ignored;
- compatibility cost has no migration route;
- performance claims lack a measurement plan;
- the proposal adds multiple canonical forms for one concept;
- implementation complexity greatly exceeds demonstrated value.

## Relationship to ADRs

- RFC: public proposal and design evaluation.
- ADR: durable architecture decision and rationale.

A significant RFC may result in one or more ADRs for implementation choices. The RFC remains the public design record; ADRs describe the chosen system boundaries.

## Relationship to ExecPlans

An ExecPlan may be created after acceptance when implementation is complex. It must not reopen accepted semantics without returning to the RFC process.

## Superseding an RFC

A later RFC must:

- identify the superseded RFC;
- explain changed evidence or constraints;
- define migration and compatibility;
- update the old RFC status and links.

Do not edit an old accepted design to look as though the new decision had always existed.