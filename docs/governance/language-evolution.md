# Language evolution process

This process governs changes to Ori syntax, semantics, types, traits, generics, memory behavior, standard-library contracts, project/package formats, diagnostics, and public tooling behavior.

## Objectives

- keep language changes evidence-based;
- preserve one canonical form per concept;
- make compatibility and migration explicit;
- prevent parser-first implementation from becoming accidental design;
- include runtime, tooling, documentation, and accessibility impact;
- keep accepted contracts separate from proposals and plans.

## Change classes

### Class A — defect correction

The implementation contradicts an existing normative contract.

Requirements:

- issue or clear PR problem statement;
- regression test;
- implementation fix;
- changelog when user-visible;
- no RFC unless the contract itself is ambiguous or must change.

### Class B — compatible clarification

The specification or diagnostic behavior is ambiguous, but the intended accepted behavior remains compatible.

Requirements:

- issue;
- specification clarification;
- conformance evidence;
- ADR when the clarification resolves a durable architectural ambiguity.

### Class C — additive public behavior

Adds syntax, type behavior, stdlib API, CLI capability, or supported backend behavior without removing an existing contract.

Requirements:

- design note or RFC depending on scope;
- compatibility and accessibility review;
- full vertical delivery and conformance evidence;
- documentation and changelog.

### Class D — incompatible change

Removes, reinterprets, or breaks an existing public contract.

Requirements:

- RFC;
- explicit versioning decision;
- alternatives and migration plan;
- removed-form diagnostics and tooling support where practical;
- ecosystem and documentation impact;
- release notes and compatibility window.

### Class E — experimental behavior

Introduces an explicitly unstable capability.

Requirements:

- clearly documented experimental boundary;
- no claim of compatibility guarantee beyond the stated scope;
- removal or stabilization criteria;
- tests and security review still required.

Experimental is not a synonym for undocumented.

## Proposal intake

A proposal should begin with the problem:

- What program or user need cannot be expressed or understood well today?
- Is the problem shown by real code, diagnostics, or performance evidence?
- Can a library, tool, or documentation change solve it without new language behavior?
- Does the proposal conflict with reading-first and accessibility principles?
- Is the behavior already possible in another canonical form?

Proposals that only copy another language feature without an Ori-specific problem statement should not advance.

## Required design dimensions

A significant proposal must address:

- syntax and grammar;
- semantic model;
- type inference;
- traits and generics;
- control flow and evaluation order;
- memory ownership and cleanup;
- async/concurrency interaction;
- native AOT/JIT support;
- runtime and ABI impact;
- formatter and migration;
- diagnostics;
- LSP/editor support;
- standard library and packages;
- compatibility and versioning;
- accessibility and cognitive load;
- security;
- performance;
- tests and conformance;
- teaching and documentation.

A dimension may be “not affected,” but it should be considered explicitly.

## Decision routes

| Situation | Artifact |
|---|---|
| Small bug within existing contract | Issue/PR |
| Durable implementation boundary | ADR |
| Significant public-language or ABI proposal | RFC |
| Complex accepted implementation | ExecPlan |
| Historical investigation | Archive record |

An RFC decides **what** should become a contract. An ExecPlan explains **how** to implement an accepted design.

## Evaluation criteria

A proposal should improve at least one of:

- expressive power for a real use case;
- safety;
- readability;
- diagnostic quality;
- performance;
- interoperability;
- tooling reliability;
- implementation maintainability.

The benefit must justify:

- grammar and semantic complexity;
- implementation cost;
- teaching burden;
- compatibility cost;
- runtime/ABI burden;
- editor and formatter burden;
- long-term maintenance.

## Prototype policy

A prototype may be useful before acceptance when it answers a specific uncertainty.

Prototype code must:

- be isolated from the public contract;
- state what it is testing;
- avoid changing current docs to claim support;
- include measurements or findings;
- be removed, archived, or converted into production work after the decision.

## Acceptance

An accepted proposal must identify:

- final syntax and semantics;
- compatibility class;
- version target;
- normative documents to update;
- conformance identifiers;
- implementation owner and phase boundaries;
- migration and diagnostic policy;
- unresolved risks;
- whether an ExecPlan is required.

Acceptance is recorded in the RFC and, for durable architecture decisions, an ADR.

## Implementation

Implementation follows [`../implementation/feature-delivery.md`](../implementation/feature-delivery.md).

The implementation should land in vertical, reviewable slices. A temporary partially implemented state must not be presented as full public support.

Feature flags or explicit experimental classification may be used when partial landing is necessary and safe.

## Stabilization

Before behavior is considered stable within the current cycle:

- normative specification is complete;
- conformance evidence is complete;
- formatter and LSP support are complete;
- AOT/JIT behavior agrees;
- unsupported native behavior is explicitly classified and rejected;
- diagnostics and migration are documented;
- performance and security risks are understood;
- user examples are current;
- no high-severity known defect contradicts the contract.

## Deprecation and removal

A deprecation should define:

- replacement;
- reason;
- first deprecated version;
- warning/diagnostic behavior;
- expected removal version or condition;
- migration tooling;
- package and documentation impact.

Pre-1.0 status permits change, but it does not justify surprise removals without explanation.

## Post-implementation review

After a significant feature lands, review:

- real programs using it;
- diagnostic quality;
- implementation complexity;
- performance;
- interaction with existing features;
- documentation clarity;
- whether the original problem was solved.

Unexpected findings become issues or a follow-up RFC, not undocumented behavior changes.