# Ori documentation atlas

The ATLAS is the canonical navigation and impact map for humans and AI agents. It identifies which document owns each subject and connects product requirements, architecture, implementation, code, tests, decisions, and operations.

## Canonical rules

1. Each subject has one current canonical source.
2. Other documents link to that source instead of copying the full contract.
3. Product docs define identity, status, audience, and support.
4. Architecture describes the implemented system today.
5. The specification defines accepted language/runtime/project/ABI behavior.
6. Implementation docs define safe extension and refactoring paths.
7. ADRs record durable decisions; RFCs evaluate significant proposals.
8. ExecPlans sequence complex accepted work; the backlog lists open outcomes.
9. Archived material is evidence, not current instruction.
10. Product/compiler/workspace version is **0.3.8**; native ABI is **`ori-native-abi-1`**.

Machine-readable relationships live in [`catalog.yaml`](catalog.yaml).

## Entry points

| Need | Canonical source |
|---|---|
| Understand the project | [`../PROJECT_START.md`](../PROJECT_START.md) |
| Mandatory contribution/agent rules | [`../AGENTS.md`](../AGENTS.md) |
| Public project introduction | [`../README.md`](../README.md) · [`../README.pt-BR.md`](../README.pt-BR.md) |
| Contribute | [`../CONTRIBUTING.md`](../CONTRIBUTING.md) |
| Report a vulnerability | [`../SECURITY.md`](../SECURITY.md) |
| Install Ori | [`install.md`](install.md) · [`install.pt-BR.md`](install.pt-BR.md) |
| Learn Ori | [`language/tour.md`](language/tour.md) |
| Find any document | This ATLAS |

## Product

| Subject | Canonical source |
|---|---|
| Product domain index | [`product/README.md`](product/README.md) |
| Current implementation/priorities/limitations | [`product/status.md`](product/status.md) |
| Version classes and compatibility | [`product/versioning.md`](product/versioning.md) |
| Backend/target/tooling support | [`product/support-matrix.md`](product/support-matrix.md) |
| Accessibility and cognitive-load principles | [`product/accessibility-principles.md`](product/accessibility-principles.md) |
| Purpose and values | [`spec/00-manifesto.md`](spec/00-manifesto.md) |

Product documents answer **what, for whom, at what maturity, and with what support level**.

## Architecture

| Subject | Canonical source | Primary code/evidence |
|---|---|---|
| Architecture index | [`architecture/README.md`](architecture/README.md) | — |
| System context and domains | [`architecture/overview.md`](architecture/overview.md) | repository tree, workspace tests |
| Crate responsibilities/dependencies | [`architecture/crate-map.md`](architecture/crate-map.md) | `compiler/Cargo.toml`, `compiler/crates/` |
| Compiler phases and contracts | [`architecture/compiler-pipeline.md`](architecture/compiler-pipeline.md) | driver phase modules, `ori_spec` |
| Cross-cutting invariants | [`architecture/invariants.md`](architecture/invariants.md) | conformance, ABI, memory, package tests |
| Runtime/ARC/cycle/FFI | [`architecture/runtime-and-memory.md`](architecture/runtime-and-memory.md) | `ori-runtime`, codegen, memory tests |
| Standard-library layering | [`architecture/stdlib.md`](architecture/stdlib.md) | stdlib catalog, runtime, `.orl` modules |
| Repository and user-project layout | [`architecture/repository-and-project-layout.md`](architecture/repository-and-project-layout.md) | project loader/scaffold/path tests |

Architecture changes update their owning document and an ADR when the underlying durable decision changes.

## Normative specification

[`spec/README.md`](spec/README.md) is the specification index. The specification is English-only and describes implemented 0.3.8 behavior.

| Contract area | Canonical source |
|---|---|
| Language overview | [`spec/01-overview.md`](spec/01-overview.md) |
| Modules/declarations/grammar | Chapter 02 and syntax chapters |
| Types | [`spec/04-types.md`](spec/04-types.md) |
| Expressions | [`spec/05-expressions.md`](spec/05-expressions.md) |
| Statements/control flow | [`spec/06-statements.md`](spec/06-statements.md) |
| Functions | [`spec/07-functions.md`](spec/07-functions.md) |
| Traits | [`spec/08-traits.md`](spec/08-traits.md) |
| Errors/optional/result | [`spec/09-errors.md`](spec/09-errors.md) |
| Memory/ARC/cleanup | [`spec/10-memory.md`](spec/10-memory.md) |
| Generics | [`spec/11-generics.md`](spec/11-generics.md) |
| Standard library | [`spec/12-stdlib.md`](spec/12-stdlib.md) |
| Diagnostic codes | [`spec/13-error-catalog.md`](spec/13-error-catalog.md) |
| Backend support | [`spec/14-backend-support.md`](spec/14-backend-support.md) |
| Stdlib maintenance | [`spec/15-stdlib-maintenance.md`](spec/15-stdlib-maintenance.md) |
| Runtime FFI safety | [`spec/16-runtime-ffi-safety.md`](spec/16-runtime-ffi-safety.md) |
| Projects/documentation formats | [`spec/17-project-and-docs.md`](spec/17-project-and-docs.md) |
| Stability/compatibility | [`spec/18-stability-and-compatibility.md`](spec/18-stability-and-compatibility.md) |
| Native ABI | [`spec/19-abi.md`](spec/19-abi.md) |

Future ideas belong in RFCs, not normative chapters.

## Implementation

| Task | Canonical source |
|---|---|
| Implementation domain index | [`implementation/README.md`](implementation/README.md) |
| Repository-wide coding patterns | [`implementation/standards.md`](implementation/standards.md) |
| Complete feature delivery | [`implementation/feature-delivery.md`](implementation/feature-delivery.md) |
| Add/change syntax | [`implementation/compiler/adding-syntax.md`](implementation/compiler/adding-syntax.md) |
| Extend/refactor type checker | [`implementation/compiler/type-checker.md`](implementation/compiler/type-checker.md) |
| Modularize runtime safely | [`implementation/runtime/refactoring.md`](implementation/runtime/refactoring.md) |
| Add stdlib API | [`implementation/stdlib/adding-api.md`](implementation/stdlib/adding-api.md) |

Implementation docs describe **how to change** the current architecture without becoming a second specification.

## Quality and evidence

| Subject | Canonical source |
|---|---|
| Quality index | [`quality/README.md`](quality/README.md) |
| Test layers and gates | [`quality/test-strategy.md`](quality/test-strategy.md) |
| Spec-to-test conformance | [`quality/language-conformance.md`](quality/language-conformance.md) |
| Diagnostic design | [`quality/diagnostic-design.md`](quality/diagnostic-design.md) |
| Performance and benchmarks | [`quality/performance-policy.md`](quality/performance-policy.md) |
| Fuzzing | [`quality/fuzzing.md`](quality/fuzzing.md) |
| Differential testing | [`quality/differential-testing.md`](quality/differential-testing.md) |
| Property-based testing | [`quality/property-testing.md`](quality/property-testing.md) |
| Documentation quality/CI | [`quality/documentation-quality.md`](quality/documentation-quality.md) |
| Detailed transitional test matrix | [`planning/qa/test-matrix-ori.md`](planning/qa/test-matrix-ori.md) |

## Security

| Subject | Canonical source |
|---|---|
| Security domain index | [`security/README.md`](security/README.md) |
| Public reporting | [`../SECURITY.md`](../SECURITY.md) |
| Threat model | [`security/threat-model.md`](security/threat-model.md) |
| Unsafe/FFI policy | [`security/unsafe-code-policy.md`](security/unsafe-code-policy.md) |
| Supply chain | [`security/supply-chain.md`](security/supply-chain.md) |
| Normative runtime FFI | [`spec/16-runtime-ffi-safety.md`](spec/16-runtime-ffi-safety.md) |
| Normative ABI | [`spec/19-abi.md`](spec/19-abi.md) |

## Governance and decisions

| Artifact | Purpose | Canonical source |
|---|---|---|
| Language evolution | Classify and review public changes | [`governance/language-evolution.md`](governance/language-evolution.md) |
| RFC process | Evaluate significant proposals | [`governance/rfc-process.md`](governance/rfc-process.md) |
| ADRs | Record durable accepted decisions | [`decisions/README.md`](decisions/README.md) |
| RFC template | Structure a proposal | [`rfcs/0000-template.md`](rfcs/0000-template.md) |
| ADR template | Structure a decision | [`decisions/TEMPLATE.md`](decisions/TEMPLATE.md) |

Current migrated ADRs:

- [`ADR-0001`](decisions/adr/0001-s3-language-surface.md) — S3 surface;
- [`ADR-0002`](decisions/adr/0002-arc-single-cascade-owner.md) — ARC cascade ownership;
- [`ADR-0003`](decisions/adr/0003-defer-copy-on-write-collections.md) — defer implicit collection COW;
- [`ADR-0004`](decisions/adr/0004-repository-and-project-layout.md) — repository/workspace/project layout.

## Planning

| Subject | Canonical source |
|---|---|
| Planning and ExecPlan policy | [`plans/README.md`](plans/README.md) |
| Open outcomes | [`planning/BACKLOG.md`](planning/BACKLOG.md) during migration |
| ExecPlan template | [`templates/EXEC_PLAN.md`](templates/EXEC_PLAN.md) |
| Retired duplicate pending file | [`planning/PENDENTES.md`](planning/PENDENTES.md) |

Rules:

- backlog contains only open/in-progress/deferred outcomes;
- complex accepted work uses `plans/active/`;
- completed plans move to `archive/plans/`;
- accepted decisions move to ADRs;
- GitHub issues track operational work;
- changelog records user-visible delivery.

## Operations

| Procedure | Canonical source |
|---|---|
| Operations index | [`operations/README.md`](operations/README.md) |
| Local development and troubleshooting | [`operations/development.md`](operations/development.md) |
| Release creation/publication | [`operations/release.md`](operations/release.md) |
| Reproducible builds/provenance | [`operations/reproducible-builds.md`](operations/reproducible-builds.md) |
| Runtime staging details | [`../runtime/README.md`](../runtime/README.md) |
| End-user install | [`install.md`](install.md) |

## User documentation

| Need | Location |
|---|---|
| Language tour | `language/` |
| Advanced language topics | [`language/advanced.md`](language/advanced.md) · [`language/concurrency.md`](language/concurrency.md) · [`language/interop.md`](language/interop.md) |
| Task-oriented guides | `guides/` |
| CLI reference | `guides/cli-reference*.md` |
| Debugging and DAP | [`guides/debugging.md`](guides/debugging.md) · [`guides/debugging.pt-BR.md`](guides/debugging.pt-BR.md) |
| Standard library guide | [`guides/stdlib-reference.md`](guides/stdlib-reference.md) · [`guides/stdlib-reference.pt-BR.md`](guides/stdlib-reference.pt-BR.md) |
| Documentation audit evidence | [`quality/documentation-audit-2026-08-08.md`](quality/documentation-audit-2026-08-08.md) |
| Long-form Portuguese book | `book/` |
| Executable examples | `../examples/` |
| Stdlib symbol documentation | `../stdlib/**/*.oridoc` |

English is canonical for primary GitHub user docs. Maintained EN/PT siblings change together. The normative specification remains English-only.

## Archive

Archive policy: [`archive/README.md`](archive/README.md).

```text
archive/
├── plans/
├── audits/
├── investigations/
├── sessions/
└── legacy/
```

The former `planning/historico/` root has been removed. Historical files now live only in the categorized archive directories. See [`archive/MIGRATION_REPORT.md`](archive/MIGRATION_REPORT.md).

## Templates and machine context

| Purpose | Source |
|---|---|
| Canonical document template | [`templates/DOCUMENT.md`](templates/DOCUMENT.md) |
| ExecPlan template | [`templates/EXEC_PLAN.md`](templates/EXEC_PLAN.md) |
| RFC template | [`rfcs/0000-template.md`](rfcs/0000-template.md) |
| Machine catalog | [`catalog.yaml`](catalog.yaml) |
| Portable AI routing | [`../.ai/README.md`](../.ai/README.md) |
| Context packs | [`../.ai/context-packs/README.md`](../.ai/context-packs/README.md) |

## Change-impact routes

| Change | Inspect/update |
|---|---|
| Syntax | product accessibility · spec · lexer/AST/parser · formatter/migration · checker/HIR/backends · LSP · diagnostics · conformance · changelog |
| Type/trait/generic | spec · checker · HIR · codegen/runtime if represented · backend matrix · diagnostics · conformance |
| Runtime/ARC | runtime architecture/invariants · unsafe policy · Specs 10/16/19 · AOT/JIT · symbol/staging/package · memory/performance/security |
| Standard library | stdlib architecture · Specs 12/15 · semantic catalog · runtime/`.orl`/`.oridoc` · LSP/export · tests/changelog |
| Diagnostic | Spec 13 · diagnostic policy · emitting phase · CLI/LSP · catalog/golden/conformance |
| Package/project | Spec 17/18 · versioning · supply chain · loader/lock/cache · migration · isolated smoke |
| Target/release | support matrix · operations · runtime metadata · CI · reproducibility/provenance · install docs |
| Documentation | ATLAS/catalog · canonical ownership · links/examples/translations · archive status · documentation CI |
| Refactoring | architecture/invariants · implementation standard · characterization tests · performance/security · no semantic drift |

## Documentation validation

Permanent validation:

```bash
python tools/docs/check_docs.py
```

CI workflow: `.github/workflows/documentation.yml`.

When adding a canonical document, update this ATLAS and [`catalog.yaml`](catalog.yaml) in the same change.
