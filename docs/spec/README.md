# Specs

> Audience: maintainer, contributor  
> Status: **current (2026-07-20)** — swept against the compiler at `0.3.x` HEAD  
> Surface: **S3** (`0.3.0`) + local inference **`0.3.1` / option B** + pipe `|>`  
> Post-FREEZE-1 additions: `newtype`, match expressions, or-patterns, `if ok` /
> `if err`, destructuring bindings, associated type aliases, const generics with
> named arguments  
> Milestones closed: **M2** stdlib · **M3** ABI · **M1** Rust-free install path  
> QA: skill **`ori-lang-qa`** · stages `tools/qa/` · matrix [`../planning/qa/test-matrix-ori.md`](../planning/qa/test-matrix-ori.md)

This directory contains **normative** implementation-facing specifications for
Ori. Chapters 01–19 are **English only** (single source of truth).
[`00-manifesto.md`](00-manifesto.md) is the one deliberate exception: it states
identity and purpose for a Brazilian project and stays in Portuguese.

User tutorials live under [../guides/](../guides/) and
[../language/](../language/) (EN + PT).

Product docs index: [../README.md](../README.md).

### Living product facts (keep in sync)

| Fact | Value |
|------|--------|
| Canonical surface | S3: `module`, `public`, `import path = alias`, `list[T]`, `end` blocks, `apply`/`use` |
| Identifiers | **snake_case** functions/modules; **PascalCase** types (not camelCase product default) |
| Visibility | **`public`** (not `pub`) |
| Memory | ARC + cooperative cycle collection |
| Execution | AOT native primary; `ori run` may JIT when cdylib staged |
| Freeze | FREEZE-1 on **0.3.x** — additive/fix only without freeze exit |
| ABI | `ori-native-abi-1` — [`19-abi.md`](19-abi.md) |
| Residuals | [`14-backend-support.md`](14-backend-support.md) · [`../planning/historico/lang-res-closure.md`](../planning/historico/lang-res-closure.md) |

Use:

- [`00-manifesto.md`](00-manifesto.md) for identity and purpose (**study**,
  **AI-assisted programming**, **ND readability**). **Ori is not market
  competition.**
- `01-overview.md` through `13-error-catalog.md` for the language contract
  under the **S3** surface (plus inference B and pipe as living features).
  **There is no chapter 03** — a standalone EBNF grammar was planned and never
  written; chapters 02 and 04–08 define the grammar between them.
- `04-types.md` / `05-expressions.md` / `06-statements.md` for local inference
  rules and the pipe operator.
- `13-error-catalog.md` for **emitted** diagnostics, including pre-S3 form
  rejections (`parse.*_removed`, `parse.poetic_call_nested`, …) and **message quality**.
- `14-backend-support.md` for the feature × backend matrix.
- `15-stdlib-maintenance.md` for the stdlib update flow.
- `16-runtime-ffi-safety.md` for runtime FFI safety contracts.
- `17-project-and-docs.md` for `ori.proj` and `.oridoc`.
- `18-stability-and-compatibility.md` for pre-1.0 stability rules.
- `19-abi.md` for the **native ABI contract** (`ori-native-abi-1`, M3): layouts,
  ARC header, mangling, link versioning.

Surface S3 product decisions and ADR:

- [`docs/planning/ori-surface-s3-auk9.md`](../planning/ori-surface-s3-auk9.md)
- [`docs/planning/adr-ori-surface-s3-auk9.md`](../planning/adr-ori-surface-s3-auk9.md)
- [`docs/planning/pr-plan-ori-surface-s3.md`](../planning/historico/pr-plan-ori-surface-s3.md)

Breaking list: repository root [`CHANGELOG.md`](../../CHANGELOG.md) section
`[0.3.0]`. Migration helper: `ori migrate-syntax`.

Do not place public tutorials here. User-facing tutorials may live under
`docs/guides/`. Implementation docs stay in `docs/spec/` and `docs/planning/`.
