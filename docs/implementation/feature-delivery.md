# Feature delivery checklist

Use this checklist for user-visible compiler, language, runtime, stdlib, tooling, or package changes. Not every row applies, but every row must be considered.

## 1. Problem and scope

- [ ] The user or engineering problem is stated without prescribing the implementation.
- [ ] Current behavior is reproduced or measured.
- [ ] In-scope and out-of-scope behavior are explicit.
- [ ] The owning component and phase are identified.
- [ ] The canonical product, architecture, and normative documents are linked.
- [ ] Dependencies and sequencing are understood.
- [ ] The change is small enough to review as one coherent vertical capability.

## 2. Decision level

- [ ] A normal issue is sufficient for a bounded implementation.
- [ ] An ADR exists when a durable architecture decision is required.
- [ ] An RFC exists for a significant language, ABI, package, or ecosystem proposal.
- [ ] An ExecPlan exists only when execution is complex, staged, or crosses many components.
- [ ] Open design questions are resolved before broad implementation begins.

## 3. Language and semantic impact

- [ ] Grammar and syntax effects are documented.
- [ ] Name resolution and visibility effects are documented.
- [ ] Type inference, traits, generics, and control-flow effects are documented.
- [ ] Memory, cleanup, and transferability effects are documented.
- [ ] Error and recovery behavior is defined.
- [ ] Interaction with existing constructs is tested.
- [ ] Compatibility and migration behavior is defined.

## 4. Pipeline implementation

- [ ] Lexer updated if tokens or literals change.
- [ ] AST updated with stable spans and minimal syntax-only data.
- [ ] Parser and recovery updated.
- [ ] Resolution/signature collection updated.
- [ ] Type checker and semantic facts updated.
- [ ] HIR lowering updated.
- [ ] Optimization passes reviewed.
- [ ] Native AOT backend updated.
- [ ] JIT path updated and compared with AOT.
- [ ] C/debug backend implemented or explicitly rejects the feature.
- [ ] Runtime and ABI updated where required.
- [ ] Incremental cache keys and debug metadata reviewed.

## 5. Tooling

- [ ] Formatter supports and preserves the feature.
- [ ] Migration helper updated when replacing old syntax.
- [ ] LSP diagnostics, hover, navigation, completion, semantic tokens, and inlays reviewed.
- [ ] Syntax highlighting reviewed.
- [ ] `ori doc` and generated documentation reviewed.
- [ ] CLI help and command output updated.
- [ ] Debugger metadata and behavior reviewed.

## 6. Standard library and runtime

- [ ] Canonical stdlib path chosen.
- [ ] Semantic signature added.
- [ ] Native ABI metadata added.
- [ ] Runtime symbol exported and staged.
- [ ] Static library and cdylib remain synchronized.
- [ ] Ownership of inputs, outputs, and stored values is explicit.
- [ ] Resource cleanup is deterministic where required.
- [ ] Platform and target differences are documented.
- [ ] LSP/docs export sees the operation.

## 7. Diagnostics

- [ ] Every new emitted code is registered.
- [ ] The primary message is short and concrete.
- [ ] Primary and secondary spans are accurate.
- [ ] Cause and action are included when useful.
- [ ] Suggestions use current Ori syntax.
- [ ] Recovery avoids cascading errors.
- [ ] CLI and LSP render equivalent semantic information.

## 8. Tests

- [ ] Unit test at the owning phase.
- [ ] Positive integration test.
- [ ] Negative diagnostic test.
- [ ] Regression test for the original defect.
- [ ] Formatter round-trip/idempotence test.
- [ ] AOT execution test.
- [ ] JIT execution test.
- [ ] C/debug parity or explicit-rejection test.
- [ ] Multifile/package test when imports or project boundaries are affected.
- [ ] Memory/ARC test when managed values are affected.
- [ ] Concurrency test when tasks/channels/transferability are affected.
- [ ] LSP test when editor behavior changes.
- [ ] Package/release smoke test when distribution changes.

## 9. Quality and security

- [ ] Invalid input cannot panic through an unreviewed path.
- [ ] `unsafe` preconditions are documented.
- [ ] Path, archive, URL, dependency, and environment inputs are validated.
- [ ] Resource and allocation limits are considered.
- [ ] Fuzzing or property tests are added for parser/decoder/state-machine surfaces when valuable.
- [ ] AOT/JIT or optimized/unoptimized differential tests are considered.
- [ ] Strict Clippy and workspace gates pass.

## 10. Performance

- [ ] Baseline measured before optimization-sensitive changes.
- [ ] Compile time, runtime, allocation, binary size, and link time considered as applicable.
- [ ] Benchmark cannot be optimized into a different workload.
- [ ] Results record build mode, target, samples, and environment.
- [ ] Regressions are accepted only with an explicit trade-off.

## 11. Documentation

- [ ] Normative specification updated.
- [ ] Backend support matrix updated.
- [ ] Diagnostic catalog updated.
- [ ] Architecture updated if boundaries or invariants changed.
- [ ] Implementation guide updated if the extension path changed.
- [ ] User guide and examples updated.
- [ ] English/Portuguese parallels updated where maintained.
- [ ] Status/versioning updated when applicable.
- [ ] Security and operations docs updated when applicable.
- [ ] Changelog updated for user-visible behavior.
- [ ] Plans closed or archived.

## 12. Completion evidence

The PR should state:

- exact behavior delivered;
- files/components affected;
- tests and commands executed;
- performance evidence where relevant;
- compatibility and migration impact;
- residual risks and explicitly deferred work;
- documents updated;
- follow-up issue identifiers, only when the follow-up is genuinely out of scope.

A feature is not complete because its happy path compiles. It is complete when its contracts, invalid cases, tooling, runtime behavior, tests, documentation, and operational consequences agree.