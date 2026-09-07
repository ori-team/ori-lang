# Language conformance

Language conformance is the traceable agreement between the normative specification and observable compiler behavior.

## Conformance unit

Each significant normative rule should have a stable identifier:

```text
ORI-<DOMAIN>-<TOPIC>-<NUMBER>
```

Examples:

- `ORI-MOD-IMPORT-001`
- `ORI-TYPE-OPTIONAL-001`
- `ORI-CTRL-MATCH-003`
- `ORI-MEM-USING-002`
- `ORI-ABI-RESULT-001`

Identifiers describe rules, not individual test functions.

## Required mapping

A conformance record should identify:

```yaml
id: ORI-MOD-IMPORT-001
spec: docs/spec/02-modules.md#import-alias
summary: Module alias imports use path-left, alias-right syntax.
positive:
  - compiler/crates/ori-driver/tests/ori_spec.rs
negative:
  - parse.import_as_removed
backends:
  native_aot: required
  jit: required
  c_debug: required
formatter: required
lsp: required
introduced: 0.3.0
current: 0.3.8
```

The future machine-readable registry may live under `docs/quality/conformance/` and generate a human-readable matrix.

## Evidence types

### Positive evidence

Shows a valid program is accepted and behaves as specified.

### Negative evidence

Shows invalid or removed behavior is rejected with the expected diagnostic class.

### Tooling evidence

Shows formatter, LSP, docs extraction, migration, and syntax highlighting agree where the rule affects them.

### Backend evidence

Shows native AOT/JIT behavior matches the support matrix.

### Runtime evidence

Shows cleanup, memory, concurrency, I/O, or ABI behavior that cannot be proven only by checking source.

## Rule states

| State | Meaning |
|---|---|
| `covered` | Required evidence exists and is green |
| `partial` | Some required dimensions lack evidence |
| `unsupported` | Normative contract explicitly excludes the route |
| `experimental` | Behavior is implemented but classified as experimental |
| `planned` | Proposal only; must not appear as normative current behavior |
| `regressed` | Previously covered rule currently fails |

Normative documents must not label a planned feature as implemented.

## Minimum conformance by feature

### Syntax

- valid parse;
- invalid parse or removed form;
- formatter output and idempotence;
- editor/incremental parse behavior where relevant.

### Semantic rule

- valid type/behavior case;
- invalid case with stable diagnostic;
- interaction with generic and edge cases;
- lowering/backend behavior where runtime-observable.

### Runtime rule

- native AOT execution;
- JIT execution;
- memory/cleanup evidence;
- ABI evidence where externally visible.

### Standard library rule

- semantic signature;
- documented failure behavior;
- native execution;
- docs and LSP visibility;
- ownership/resource behavior where applicable.

## Specification coverage review

For every normative chapter:

1. enumerate normative statements;
2. assign or reuse conformance identifiers;
3. link tests and diagnostics;
4. classify backend/tooling requirements;
5. identify untested or ambiguous rules;
6. open focused issues for real gaps;
7. avoid writing fake tests for behavior that is explicitly not part of Ori.

## Relationship to existing suites

- `ori_spec` remains the primary language conformance suite.
- `diagnostic_catalog` validates emitted-code registration.
- `multifile_imports` provides stdlib and project-boundary evidence.
- runtime, memory, async, JIT, and codegen tests provide execution evidence.
- examples provide realistic surface evidence but do not replace focused tests.

## Change policy

A public rule change must update:

- the normative statement;
- conformance identifier or its version applicability;
- positive and negative evidence;
- formatter/LSP/backend evidence;
- compatibility and migration documentation;
- changelog.

Do not silently repurpose an existing conformance identifier for a different semantic rule.

## Reporting

A generated report should eventually show:

- rules by specification chapter;
- covered, partial, experimental, and unsupported counts;
- missing backend or tooling dimensions;
- diagnostics without negative conformance links;
- tests referencing removed rules;
- examples not exercised by CI;
- rule coverage changes per PR.

Conformance coverage should become a release input, not only a documentation metric.