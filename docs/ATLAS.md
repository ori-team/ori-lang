# Ori documentation atlas

The ATLAS is the canonical navigation map for humans and AI agents. It identifies the source of truth for each subject and connects documents to code, tests, decisions, and operational procedures.

## Rules of the atlas

1. Each subject has one canonical document.
2. Other documents link to the canonical source instead of repeating its full content.
3. Architecture describes the system as it exists now.
4. ADRs explain why durable decisions were made.
5. RFCs describe proposals that are not yet accepted contracts.
6. Plans describe how a specific complex change will be executed.
7. The normative specification describes accepted language and runtime behavior.
8. Archived material is evidence, not current instruction.
9. Product version, compiler version, and workspace version are currently **0.3.8**.
10. The native ABI has its own version: **`ori-native-abi-1`**.

## Entry points

| Need | Canonical source |
|---|---|
| Start contributing | [`../PROJECT_START.md`](../PROJECT_START.md) |
| Mandatory project rules | [`../AGENTS.md`](../AGENTS.md) |
| Product overview | [`product/README.md`](product/README.md) |
| Current status and versions | [`product/status.md`](product/status.md) |
| Versioning policy | [`product/versioning.md`](product/versioning.md) |
| Language learning | [`language/tour.md`](language/tour.md) |
| Installation | [`install.md`](install.md) |
| Language contract | [`spec/README.md`](spec/README.md) |
| System architecture | [`architecture/README.md`](architecture/README.md) |
| Implementation standards | [`implementation/standards.md`](implementation/standards.md) |
| Testing and conformance | [`quality/README.md`](quality/README.md) |
| Security engineering | [`security/README.md`](security/README.md) |
| Decisions and RFCs | [`decisions/README.md`](decisions/README.md) |
| Active plans | [`plans/README.md`](plans/README.md) |
| Operations | [`operations/README.md`](operations/README.md) |
| Historical records | [`archive/README.md`](archive/README.md) |

## Product

| Document | Owns |
|---|---|
| [`product/README.md`](product/README.md) | Product identity, audience, goals, and boundaries |
| [`product/status.md`](product/status.md) | Current 0.3.8 status, maturity, supported paths, known limitations |
| [`product/versioning.md`](product/versioning.md) | Version numbers, compatibility classes, release semantics |
| [`product/accessibility-principles.md`](product/accessibility-principles.md) | Accessibility and cognitive-load principles for language, diagnostics, and docs |
| [`spec/00-manifesto.md`](spec/00-manifesto.md) | Project purpose and values |

## Architecture

| Document | Code relationship | Primary evidence |
|---|---|---|
| [`architecture/overview.md`](architecture/overview.md) | Entire repository | Workspace tests and component READMEs |
| [`architecture/compiler-pipeline.md`](architecture/compiler-pipeline.md) | `compiler/crates/ori-*` | `ori_spec`, driver integration tests |
| [`architecture/invariants.md`](architecture/invariants.md) | Cross-cutting | Conformance, catalog, ABI and runtime tests |
| [`architecture/runtime-and-memory.md`](architecture/runtime-and-memory.md) | `ori-runtime`, native backend | `memory_arc`, runtime tests, Spec 10/16/19 |
| [`architecture/stdlib.md`](architecture/stdlib.md) | `ori-types/src/stdlib.rs`, `stdlib/`, runtime | stdlib parity and multifile tests |
| [`planning/repo-and-project-layout.md`](planning/repo-and-project-layout.md) | Repository and Ori project layout | project loading and scaffold tests |

## Normative contracts

`docs/spec/` remains the normative implementation-facing contract. It is English-first and must describe behavior implemented today.

| Contract area | Canonical source |
|---|---|
| Identity and overview | `spec/00-manifesto.md`, `spec/01-overview.md` |
| Modules and declarations | `spec/02-*` and related chapters |
| Types, expressions, statements | `spec/04-types.md` through `spec/06-statements.md` |
| Traits and generics | `spec/08-traits.md`, `spec/11-generics.md` |
| Errors and diagnostics | `spec/09-errors.md`, `spec/13-error-catalog.md` |
| Memory | `spec/10-memory.md` |
| Standard library | `spec/12-stdlib.md`, `spec/15-stdlib-maintenance.md` |
| Backend support | `spec/14-backend-support.md` |
| Runtime and FFI safety | `spec/16-runtime-ffi-safety.md` |
| Projects and documentation | `spec/17-project-and-docs.md` |
| Stability and compatibility | `spec/18-stability-and-compatibility.md` |
| Native ABI | `spec/19-abi.md` |

Normative documents do not contain speculative designs. Proposed behavior belongs in an RFC.

## Implementation

| Document | Purpose |
|---|---|
| [`implementation/standards.md`](implementation/standards.md) | Repository-wide coding and component-boundary standards |
| [`implementation/feature-delivery.md`](implementation/feature-delivery.md) | Vertical checklist for a language or tooling feature |
| [`implementation/compiler/adding-syntax.md`](implementation/compiler/adding-syntax.md) | End-to-end syntax implementation path |
| Existing component READMEs | Local setup and component-specific details |

## Quality

| Document | Purpose | Related tests/tools |
|---|---|---|
| [`quality/test-strategy.md`](quality/test-strategy.md) | Test layers, gates, ownership, and required evidence | `tools/qa/`, Cargo tests |
| [`quality/language-conformance.md`](quality/language-conformance.md) | Specification-to-test traceability | `ori_spec`, examples |
| [`quality/diagnostic-design.md`](quality/diagnostic-design.md) | Diagnostic codes, messages, labels, actions, and compatibility | `diagnostic_catalog` |
| [`quality/performance-policy.md`](quality/performance-policy.md) | Benchmark method and regression policy | benchmark scripts, `performance_guard` |
| [`planning/qa/test-matrix-ori.md`](planning/qa/test-matrix-ori.md) | Detailed current test coverage matrix | S0–S8 QA stages |

## Security

| Document | Purpose |
|---|---|
| [`../SECURITY.md`](../SECURITY.md) | Public vulnerability reporting policy |
| [`security/threat-model.md`](security/threat-model.md) | Assets, trust boundaries, threats, and mitigations |
| [`security/unsafe-code-policy.md`](security/unsafe-code-policy.md) | Rules for Rust `unsafe`, FFI, pointers, and ABI boundaries |
| `spec/16-runtime-ffi-safety.md` | Normative runtime FFI safety contract |

## Governance and decisions

| Artifact | Use it when |
|---|---|
| Issue | The work is concrete, bounded, and does not need a durable design record |
| ADR | A durable architecture or implementation decision has been accepted |
| RFC | A language, ABI, package, tooling, or ecosystem proposal needs design review |
| ExecPlan | A complex accepted change needs a staged implementation plan |
| Changelog | A released or user-observable behavior changed |

See [`governance/language-evolution.md`](governance/language-evolution.md), [`governance/rfc-process.md`](governance/rfc-process.md), and [`decisions/README.md`](decisions/README.md).

## Planning

`docs/planning/BACKLOG.md` remains the current implementation backlog during migration. New planning documents must follow [`plans/README.md`](plans/README.md).

Rules:

- the backlog contains open work, not a full history;
- completed complex plans move to `docs/archive/plans/`;
- accepted decisions move to `docs/decisions/adr/`;
- discussion documents that never became contracts are archived;
- plans must link to the contract they implement and the tests that prove completion.

## Operations

| Procedure | Canonical source |
|---|---|
| Local development and validation | [`operations/development.md`](operations/development.md) |
| Packaging and releases | [`operations/release.md`](operations/release.md) |
| Installation for users | [`install.md`](install.md) |
| Runtime staging details | [`../runtime/README.md`](../runtime/README.md) |
| Standard library inventory | [`../stdlib/README.md`](../stdlib/README.md) |

## User documentation

User-facing documentation is separate from implementation-facing contracts:

- `docs/language/` — language tour;
- `docs/guides/` — task-oriented guides;
- `docs/book/` — long-form Portuguese book;
- `examples/` — executable examples;
- root README files — concise project presentation.

English is the canonical GitHub-facing language. Portuguese user documentation is maintained in parallel where a sibling `*.pt-BR.md` file exists. The normative specification remains English-only.

## Archive policy

Archived content must contain or inherit these facts:

- it is not a current source of truth;
- its original date and purpose are preserved;
- its replacement is linked when one exists;
- commands, versions, and syntax in it may be obsolete;
- active documents must not depend on archived instructions for current behavior.

## Change-impact routes

| Change | Documents and evidence to inspect |
|---|---|
| Syntax | Spec, parser, formatter, LSP, diagnostics, conformance, migration, changelog |
| Type system | Spec, checker, HIR, backend support, diagnostics, conformance, changelog |
| Runtime/ARC | Memory architecture, Specs 10/16/19, ABI tests, AOT/JIT parity, security |
| Stdlib | Specs 12/15, stdlib architecture, manifest, `.orl`/`.oridoc`, LSP, tests |
| CLI | CLI guide, operations, help text, integration tests, changelog |
| Package/project format | Specs 17/18, versioning, project loader, lock tests, migration |
| Diagnostic | Spec 13, diagnostic policy, catalog test, LSP behavior |
| Release | Status, versioning, changelog, packaging, checksums, smoke matrix |
| Documentation framework | ATLAS, catalog, links, metadata, archive state, CI validation |

## Machine-readable index

[`catalog.yaml`](catalog.yaml) records canonical documents, domains, owners, related code, and evidence. It is intended for validation scripts and context-pack generation. The Markdown ATLAS remains the human-readable entry point.

When adding a canonical document, update both the ATLAS and catalog in the same change.