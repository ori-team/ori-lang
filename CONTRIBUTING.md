# Contributing to Ori

Thank you for contributing to Ori. The current canonical project version is **0.3.8**.

## Before you begin

Read:

1. [`PROJECT_START.md`](PROJECT_START.md)
2. [`AGENTS.md`](AGENTS.md)
3. [`docs/ATLAS.md`](docs/ATLAS.md)
4. the canonical documents for the component you will change

Search existing issues and the current backlog before creating parallel work.

## Choose the correct artifact

- **Issue:** bug, bounded feature, or concrete maintenance task.
- **ADR:** durable architecture or implementation decision.
- **RFC:** significant language, ABI, project/package, stdlib-direction, or ecosystem proposal.
- **ExecPlan:** complex accepted work spanning several PRs or a risky staged migration.

Small changes should not require heavyweight planning.

## Contribution flow

1. Describe the problem and current behavior.
2. Create a focused branch.
3. Identify the owning phase, component, and canonical documents.
4. Add or update tests that reproduce the required behavior.
5. Implement one coherent vertical change.
6. Update diagnostics, specifications, architecture, examples, and changelog as applicable.
7. Run relevant focused and broad gates.
8. Open a PR with exact validation evidence and residual risks.

## Implementation expectations

Follow [`docs/implementation/standards.md`](docs/implementation/standards.md).

Important rules:

- fix invalid behavior in the earliest informed phase;
- keep the driver and other façades small;
- avoid competing sources of truth;
- preserve native symbols and ABI layouts during refactoring;
- isolate and document `unsafe` code;
- keep AOT and JIT behavior aligned;
- explicitly classify unsupported native behavior;
- do not combine broad refactoring with unrelated language changes;
- do not add external product ecosystems to the core repository without a decision.

## Tests

Every bug fix needs a regression test.

Every user-visible feature needs evidence appropriate to its path, which may include:

- positive and negative semantic tests;
- diagnostic catalog coverage;
- formatter tests;
- AOT and JIT execution;
- explicit rejection of unsupported native shapes;
- multifile, stdlib, or package tests;
- runtime/ARC tests;
- LSP tests;
- isolated package smoke tests;
- benchmarks or security tests.

Use [`docs/quality/test-strategy.md`](docs/quality/test-strategy.md) and [`docs/implementation/feature-delivery.md`](docs/implementation/feature-delivery.md).

## Validation commands

Run from repository root:

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test diagnostic_catalog
cargo --manifest-path compiler/Cargo.toml test -p ori-lsp
```

Fast gate:

```bash
sh tools/qa/daily_fast.sh
```

Runtime changes may require staging:

```bash
sh tools/stage_native_runtime.sh
```

Windows:

```powershell
.\tools\stage_native_runtime.ps1
```

State all commands actually run. Do not claim a full gate when only a focused test passed.

## Documentation

Update documentation in the same PR when behavior or extension paths change.

- Product/status: `docs/product/`
- Architecture: `docs/architecture/`
- Normative contract: `docs/spec/`
- Implementation guidance: `docs/implementation/`
- Quality/security/operations: corresponding domains
- User docs: `docs/language/`, `docs/guides/`, examples
- Decisions/proposals: `docs/decisions/`, `docs/rfcs/`
- Changelog: user-visible behavior

Maintain EN/PT user-facing siblings where the repository already maintains a parallel version.

New canonical documents must be added to `docs/ATLAS.md` and `docs/catalog.yaml`.

## Diagnostics

A new diagnostic must include:

- stable code;
- clear primary message;
- actionable source span;
- explanation/action when useful;
- catalog entry in `docs/spec/13-error-catalog.md`;
- negative test;
- LSP review when applicable.

## Runtime and FFI

Runtime changes must preserve:

- documented header/layout contracts;
- ownership and single-cascade rules;
- exported symbol names;
- staticlib/cdylib parity;
- AOT/JIT semantics;
- panic containment across FFI;
- target-specific behavior.

Read [`docs/security/unsafe-code-policy.md`](docs/security/unsafe-code-policy.md) before changing `unsafe` or exported native code.

## Performance

Measure before optimizing. Include workload, build mode, target, sample method, baseline, and trade-offs. See [`docs/quality/performance-policy.md`](docs/quality/performance-policy.md).

## Pull requests

A PR description should state:

- problem and outcome;
- scope and exclusions;
- implementation approach;
- tests and commands;
- compatibility/migration;
- runtime/ABI/target impact;
- security/performance impact;
- documentation changes;
- residual risk and deferred work.

Keep PRs focused. Avoid unrelated formatting or cleanup that hides the behavior change.

## Third-party code and licenses

Before copying or porting code:

- verify license compatibility with Apache-2.0 OR MIT;
- preserve required notices;
- cite origin in the PR;
- document modifications;
- do not include code with incompatible or unclear terms.

## Contribution license

Unless explicitly stated otherwise, intentional contributions are licensed under:

- Apache-2.0 OR MIT

A DCO-style sign-off is recommended:

```text
Signed-off-by: Your Name <your-email@example.com>
```

```bash
git commit -s -m "fix: describe the focused change"
```

## Definition of done

A contribution is complete when implementation, tests, diagnostics, specification, architecture, user documentation, compatibility, security, performance, operations, changelog, and planning state agree for its scope.