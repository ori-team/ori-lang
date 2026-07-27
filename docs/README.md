# Ori documentation

> Current project version: **0.3.8**  
> Language surface: **S3**  
> Native ABI: **`ori-native-abi-1`**

Start with the canonical [`ATLAS.md`](ATLAS.md). It maps documents to code, tests, decisions, and operational procedures.

## By audience

| Audience | Start here |
|---|---|
| New user | [Install](install.md) → [Language tour](language/tour.md) → [First project](guides/first-project.md) |
| Portuguese reader | [Índice em português](README.pt-BR.md) |
| Language user | [Guides](guides/README.md) · [CLI reference](guides/cli-reference.md) · [Examples](../examples/) |
| Language implementer | [Specification](spec/README.md) · [Compiler pipeline](architecture/compiler-pipeline.md) |
| Contributor | [Project start](../PROJECT_START.md) · [Operational rules](../AGENTS.md) |
| Runtime contributor | [Runtime and memory](architecture/runtime-and-memory.md) · [Unsafe policy](security/unsafe-code-policy.md) |
| Maintainer | [Product status](product/status.md) · [Versioning](product/versioning.md) · [Operations](operations/README.md) |
| AI agent | [ATLAS](ATLAS.md) · [Catalog](catalog.yaml) · [`.ai/`](../.ai/README.md) |

## Domains

```text
docs/
├── ATLAS.md                 canonical navigation and impact map
├── catalog.yaml             machine-readable document relationships
├── product/                 identity, status, versioning, accessibility
├── architecture/            current system and invariants
├── spec/                    normative language, runtime, project, and ABI contracts
├── implementation/          standards and safe extension paths
├── quality/                 tests, conformance, diagnostics, performance
├── security/                threat model and unsafe/FFI policy
├── governance/              language evolution and RFC process
├── decisions/               ADR policy and accepted decisions
├── rfcs/                    significant public proposals
├── plans/                   active complex execution plans
├── operations/              development and release procedures
├── language/                user-facing language learning
├── guides/                  task-oriented user guides
├── book/                    long-form Portuguese book
└── archive/                 categorized historical evidence
```

The historical documentation migration is complete:

- `docs/planning/BACKLOG.md` is the canonical open-work list;
- accepted decisions live in `docs/decisions/adr/`;
- completed plans and investigations live in categorized directories under `docs/archive/`;
- the former `docs/planning/historico/` root has been removed;
- the complete move map is in [`archive/MIGRATION_REPORT.md`](archive/MIGRATION_REPORT.md).

Planning documents never override the normative specification.

## Canonical-source rules

1. One subject has one canonical current document.
2. Other documents link instead of repeating the full explanation.
3. Architecture describes implementation today.
4. Specification describes accepted behavior today.
5. ADRs explain durable decisions.
6. RFCs describe proposals under review.
7. ExecPlans describe complex accepted implementation work.
8. Archived documents may contain obsolete versions, commands, or syntax.
9. Active status documents use project version `0.3.8`.
10. New canonical documents update both `ATLAS.md` and `catalog.yaml`.

## Language policy

| Document class | Language policy |
|---|---|
| Root GitHub presentation and primary user docs | English canonical |
| Maintained user-facing translation | Portuguese sibling such as `*.pt-BR.md` |
| Normative specification | English only |
| Planning and historical evidence | Existing language may be preserved |
| Book | Portuguese |
| Code and code comments | English |

When a user-visible behavior changes, update maintained EN/PT siblings in the same PR.

## Current syntax baseline

Active examples must use current S3 syntax:

- `module app.name`;
- functions without a declaration `func` keyword;
- `public` visibility;
- types with `[]`;
- `import path = alias`;
- `ok`, `err`, and `try`;
- `apply Type` and `use Trait`;
- `end`-delimited blocks.

Historical documents must be clearly archived before preserving removed syntax.

## Quality

Documentation changes should validate:

- internal links;
- canonical-source ownership;
- active version references;
- runnable examples;
- EN/PT parity where maintained;
- document metadata and catalog entries;
- archived-state rules;
- absence of obsolete project identity in active material.

See [quality documentation](quality/README.md) and [documentation templates](templates/DOCUMENT.md).
