# Ori Language Specification — Chapter 18: Stability and compatibility

> Status: normative for Ori **0.3.8**  
> Maturity: pre-1.0  
> Language surface: S3  
> Native ABI: `ori-native-abi-1`

Ori is still before 1.0, but pre-1.0 does not mean arbitrary behavior. This chapter separates current public contracts from explicitly experimental areas and defines how compatible and incompatible changes are handled.

## 1. Version concepts

The current public project/compiler/workspace version is **0.3.8**.

The S3 language surface was introduced earlier in the 0.3 line and remains the current surface. Historical introduction versions must not be presented as the current project version.

The native ABI uses an independent compatibility tag: **`ori-native-abi-1`**.

See [`../product/versioning.md`](../product/versioning.md).

## 2. Public language contract

The following are public contracts in 0.3.8:

- UTF-8 `.orl` source;
- a required `module` declaration;
- `end`-delimited blocks with documented optional labels;
- the three canonical import forms;
- `public` visibility;
- explicit public types and documented local inference;
- bracketed composite types such as `list[T]`, `optional[T]`, and `result[T, E]`;
- `optional[T]` for absence;
- `result[T, E]` for recoverable failure;
- `try expression` for propagation;
- current struct, enum, pattern, match, closure, trait, generic, and const-argument forms described by the specification;
- `apply Type` and `use Trait` behavior;
- deterministic cleanup through `using` under the documented disposal contract;
- stable diagnostic-code meanings;
- the native backend as the semantic reference.

A public-contract change requires specification and conformance updates. A defect fix that restores this contract does not require treating accidental behavior as compatible.

## 3. Removed behavior

Removed legacy forms are not a compatibility surface merely because archived files contain them.

Where practical, removed forms have:

- a dedicated diagnostic;
- a current replacement;
- migration support through `ori migrate-syntax` for mechanical cases;
- changelog history.

Active documentation and generated suggestions must not teach removed syntax.

## 4. Native ABI contract

[`19-abi.md`](19-abi.md) defines `ori-native-abi-1`, including:

- primitive and aggregate layouts;
- tags and payload rules;
- managed allocation header;
- retain/release and ownership conventions;
- runtime symbols;
- mangling and exported entry points;
- static and dynamic runtime metadata;
- embedding bridges and generated headers.

An incompatible layout, calling convention, ownership, mangling, or required-symbol change requires a new `ori-native-abi-N` tag.

Compatible implementation fixes and additive symbols may keep the ABI tag when they do not invalidate existing compiled/runtime artifacts. The change still requires metadata, tests, and specification review.

The C/debug backend is not the native ABI reference.

## 5. Project, package, and lock compatibility

Project and package formats are public where explicitly specified, but some details remain pre-1.0 and experimental.

A format change must define:

- accepted old versions;
- schema/version field behavior;
- lockfile refresh or rejection behavior;
- deterministic resolution implications;
- migration tooling or manual steps;
- package-cache and registry consequences;
- security implications.

Unknown schema versions should fail clearly rather than being guessed.

## 6. Standard-library compatibility

Public stdlib names and documented behavior are compatibility contracts unless marked experimental.

Compatible changes may include:

- implementation fixes;
- performance improvements preserving semantics;
- additive operations;
- clearer diagnostics and documentation.

Renames, removals, changed failure types, changed ownership, or changed mutation behavior require compatibility and migration review.

Compatibility aliases have one canonical replacement and should not become permanent duplicate identities without a deliberate decision.

## 7. Diagnostics compatibility

Diagnostic codes are public identifiers.

Within compatible releases:

- a code keeps the same semantic meaning;
- wording and labels may improve without changing the underlying rule;
- new diagnostics may split a generic error when migration and tooling impact are considered;
- removed syntax keeps actionable replacement guidance;
- CLI and LSP preserve semantic parity.

Consumers should not depend on complete human message text as a stable machine API unless a separate structured output contract provides that guarantee.

## 8. Experimental areas

The following may evolve before 1.0 where they are explicitly documented as experimental:

- hosted registry protocol and deployment model;
- final package/lockfile details not yet frozen by a normative schema;
- debugger and cooperative DAP details;
- incremental-compilation storage format;
- updater metadata and distribution channels;
- REPL limits;
- C/debug backend public surface beyond its documented subset;
- APIs explicitly marked experimental;
- future ecosystem/plugin/build-script capabilities not yet accepted as stable contracts.

Experimental behavior still requires documentation, tests, security review, and a removal or stabilization path.

## 9. Compatibility classes

### Defect correction

Restores documented behavior. Requires regression evidence and user-visible release notes when material.

### Compatible clarification

Makes an existing rule more precise without invalidating conforming programs.

### Additive change

Adds a new capability without removing or reinterpreting current behavior. Requires full vertical implementation and conformance.

### Deprecation

Keeps behavior temporarily while directing users to a replacement. Requires version, warning/diagnostic, migration, and removal criteria.

### Incompatible change

Invalidates current conforming source, packages, artifacts, ABI consumers, or stable tooling assumptions. Requires an RFC, explicit version decision, migration plan, and release communication.

## 10. Change process

A significant public change follows:

1. problem and evidence;
2. RFC when required;
3. compatibility classification;
4. accepted semantics and alternatives;
5. implementation and migration plan;
6. positive, negative, tooling, backend, and runtime evidence;
7. specification and conformance update;
8. changelog and current-status update;
9. release validation.

See [`../governance/language-evolution.md`](../governance/language-evolution.md).

## 11. Known limitations

A known limitation is implemented current behavior that users may encounter. It is not automatically a promise to add a feature.

Current limitations must be documented in the owning specification or product-status page, with explicit backend and platform classification where relevant.

Examples include:

- unsupported C/debug operations listed in Chapter 14;
- embedding shapes not exposed by the current ABI;
- absence of compile-time proof for undecidable infinite recursion;
- pre-1.0 package/registry details still marked experimental.

Do not copy broad future wish lists into this chapter.

## 12. Documentation rule

The specification describes what the current parser, checker, lowering, backend, runtime, CLI, and tooling accept today.

Future proposals belong in RFCs. Execution sequencing belongs in plans. Historical behavior belongs in the changelog or archive.

When public behavior changes, the delivery updates:

- normative chapters;
- conformance and regression tests;
- examples and maintained user docs;
- diagnostics and migration;
- backend support;
- versioning/ABI where applicable;
- changelog;
- current status.

## 13. Stability review for 1.0

A 1.0 decision should require:

- sustained language compatibility;
- a coherent and documented stdlib;
- reliable project/package formats;
- stable native ABI and runtime distribution;
- reproducible supported-target releases;
- complete deprecation and migration policy;
- strong conformance, quality, security, and performance evidence;
- synchronized documentation and tooling.

Self-hosting may be a maturity milestone, but it is not the only or defining criterion for 1.0.