# Documentation quality

Documentation is part of the Ori product and compiler contract. It must be versioned, reviewed, tested, and maintained with the same discipline as code.

## Quality dimensions

### Correctness

- active syntax compiles;
- commands use the correct working directory and options;
- version/status claims match canonical metadata;
- architecture describes current implementation;
- specification describes accepted behavior;
- backend/platform claims have evidence.

### Canonical ownership

- each subject has one current source;
- summaries link rather than copy full contracts;
- decisions, architecture, plans, and history are not mixed;
- archived material is visibly non-current.

### Completeness

A document covers the dimensions required by its class:

- product: audience, status, boundaries, limitations;
- architecture: responsibilities, flows, invariants, evidence;
- specification: normative rule, invalid behavior, compatibility;
- implementation: extension path, failure modes, tests;
- operations: prerequisites, commands, outputs, recovery;
- security: assets, boundaries, threats, mitigations;
- plans: objective, phases, risks, evidence, completion.

### Accessibility

- headings are descriptive;
- terminology is introduced before use;
- paragraphs stay focused;
- examples introduce one main idea;
- important meaning does not depend on color;
- instructions state exact actions;
- jargon is limited or explained;
- long documents provide navigation and context.

### Traceability

- normative rules link to conformance evidence;
- architecture links to code and tests;
- ADRs link to affected contracts;
- plans link to issues/PRs and completion evidence;
- release notes link to migration/user docs where appropriate.

## Document classes

Every maintained document should be classifiable as one of:

- entry/index;
- product;
- architecture;
- normative specification;
- implementation guide;
- quality policy/report;
- security policy/model;
- governance/decision/proposal;
- execution plan;
- operations/runbook;
- user guide/reference;
- archive.

A document that serves several unrelated classes should be split.

## Metadata

Canonical maintained documents should progressively include or register:

- stable ID;
- title;
- status;
- canonical flag;
- audience;
- owner/domain;
- related documents;
- related code;
- verification evidence;
- last verified date.

`docs/catalog.yaml` is the initial machine-readable registry. Front matter may be adopted per domain when validation is ready.

## Version rules

- Current project/compiler/workspace version is derived from `compiler/Cargo.toml`.
- Active current-status documents use that exact version.
- Historical numbers use explicit contexts such as “introduced in,” “released in,” or changelog headings.
- ABI, manifest schema, package format, and book artifact versions are named as separate version classes.
- A release update changes all current-version surfaces in one PR.

## Links

- Use relative repository links for repository files.
- Link to the canonical source or stable heading.
- Avoid links to temporary branches or local paths.
- Archived files link forward to current replacements.
- Moving a file updates inbound links or leaves a clear compatibility pointer.
- External references should be durable and necessary.

## Examples and commands

Runnable examples should be:

- current S3;
- minimal but realistic;
- formatted;
- included in CI or generated from tested source where practical;
- explicit about files, arguments, output, and prerequisites.

Commands state the working directory. Root Cargo commands use `--manifest-path compiler/Cargo.toml` unless the document first instructs readers to enter `compiler/`.

Do not paste output that cannot be reproduced or identify the environment.

## Translation parity

English is canonical for primary GitHub-facing user docs and the normative specification.

Where an EN/PT sibling is maintained:

- structure and feature coverage should remain equivalent;
- code examples use the same current syntax;
- version/status banners agree;
- user-visible changes update both in one PR;
- translation may adapt wording, but not behavior.

The spec remains English-only to avoid competing normative translations.

## Review checklist

- Is this the correct document class/location?
- Does another file already own the subject?
- Are status and version current?
- Are architecture and future plans separated?
- Do examples/commands work?
- Are limitations and unsupported routes explicit?
- Are security/performance/compatibility claims supported?
- Is the reading path clear?
- Are maintained translations updated?
- Should old material be archived instead of edited as current?

## Automated gates

The initial validator checks:

- required entry files;
- catalog paths;
- version agreement;
- relative links in canonical docs;
- retired identity terms.

Planned gates:

- canonical ownership duplication;
- orphan maintained documents;
- invalid metadata/status values;
- archive links used as current instruction;
- EN/PT missing or structurally divergent siblings;
- stale version claims;
- executable `.orl` snippets/examples;
- ADR/RFC/ExecPlan index/status consistency;
- spec-to-conformance coverage;
- code/test paths that no longer exist;
- ATLAS and context-pack generation drift.

## Documentation debt

Documentation debt should be represented as a specific gap:

- incorrect contract;
- missing domain document;
- untested example;
- duplicate source;
- missing translation;
- missing conformance link;
- stale archive classification;
- broken operational procedure.

Avoid generic tasks such as “improve docs” without completion evidence.

## Definition of done

A documentation change is done when:

- canonical ownership is clear;
- content matches 0.3.8 implementation/contracts;
- links and examples are valid;
- version and terminology are consistent;
- related code/tests/decisions are traceable;
- maintained translations are aligned;
- old competing material is redirected or archived;
- documentation CI passes.