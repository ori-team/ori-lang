# Property-based testing

Property-based tests generate many structured inputs and verify invariants rather than enumerating only hand-written cases.

## Appropriate uses

- lexer and parser structural invariants;
- formatter round trips;
- type substitution and generic argument handling;
- constant evaluation;
- collection models;
- manifest/lockfile serialization;
- ARC ownership graphs;
- deterministic catalogs and generated metadata.

Property tests complement examples and regressions. They do not replace precise tests for a known diagnostic or language rule.

## Core properties

### Formatter

For accepted source:

```text
format(format(source)) == format(source)
```

And:

```text
parse(format(source)) preserves the relevant AST/semantic structure
```

### Parser and spans

- Every span is within source bounds.
- Token/AST traversal terminates.
- Parsing the same source is deterministic.
- Pretty-printed valid generated AST parses.

### Type system

- Substituting no parameters is identity.
- Substitution preserves non-target definitions.
- Equivalent normalized types compare consistently.
- Alias expansion terminates or diagnoses cycles.
- Displayed types remain stable for equal semantic types.

### Constant evaluator

For the supported scalar subset, compare against a small reference evaluator.

- deterministic result;
- checked overflow;
- division-by-zero diagnostic;
- cycle detection;
- no runtime-only effects.

### Collections

Compare generated operation sequences with a simple Rust model.

- length and content agree;
- get/set/insert/remove behavior agrees;
- iteration invalidation follows contract;
- equality/hash/order preconditions are respected;
- clone and aliasing behavior agrees with documented semantics.

### Manifests and lockfiles

- serialize → parse preserves canonical data;
- canonical serialization is deterministic;
- dependency order does not change canonical output;
- unknown schema versions reject clearly;
- paths remain within allowed roots.

### ARC model

For generated ownership graphs:

- reachable objects survive collection;
- unreachable acyclic objects free at zero;
- unreachable cycles collect;
- finalizers run at most once;
- edge indexes match model relationships;
- no negative count or stale allocation remains;
- fresh/borrowed store rules balance ownership.

## Generator design

Generators should produce valid structured values with controlled complexity.

Use:

- bounded depth;
- weighted common and edge cases;
- shrinkable components;
- explicit invalid generators for diagnostic properties;
- seed recording;
- realistic identifiers, types, and source shapes.

Avoid generators that produce almost only meaningless rejected input. Fuzzing is better for broad malformed exploration; property testing is strongest with structured domains.

## Shrinking

A failure is useful when it reduces to a small explanation.

Generators should shrink:

- source blocks and declarations;
- type argument lists;
- operation sequences;
- ownership graphs;
- manifest dependency sets;
- strings/bytes and numeric values.

Preserve the seed and minimized case as a deterministic regression.

## Determinism

Every CI failure records:

- seed;
- generated input or operation sequence;
- property name;
- project commit/version;
- target and relevant environment.

PR checks use fixed or replayable seeds. Scheduled runs may vary seeds while publishing them on failure.

## Boundaries

Property tests must not:

- execute unbounded generated native programs without timeout;
- write outside temporary roots;
- contact real registries/services;
- rely on wall-clock timing as a semantic assertion;
- accept normalization that hides real output differences;
- assert private structure that refactoring should be free to change.

## Tooling

Rust crates such as `proptest` or `quickcheck` may be used after evaluating compile-time and maintenance cost. Some properties can use custom deterministic generators without a new dependency.

The selected framework should support:

- shrinking;
- reproducible seeds;
- bounded case counts;
- readable failure output;
- integration with the existing test runner.

## Adoption sequence

1. Formatter idempotence and valid-source round trip.
2. Type substitution/alias properties.
3. Manifest/lockfile canonical round trip.
4. Collection reference-model sequences.
5. ARC ownership-graph model.
6. Generated semantic/differential programs.

## Completion criteria

A property-test family is mature when:

- its invariant is documented;
- generator and shrinker target meaningful cases;
- failures are reproducible;
- runtime is suitable for its CI tier;
- at least one known bug class is protected;
- minimized cases become ordinary regressions.