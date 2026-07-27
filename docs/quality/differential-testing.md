# Differential testing

Differential testing compares two implementations or configurations that should produce the same observable semantics.

## Primary comparisons

### AOT versus JIT

For the shared support surface, compare:

- accepted/rejected source;
- stdout and stderr;
- exit status;
- returned values;
- panic/trap behavior;
- deterministic cleanup and destructor effects;
- managed allocation/leak observations;
- platform-independent debug-visible results where practical.

A difference is a defect unless the support matrix explicitly documents it.

### Optimized versus unoptimized HIR

Compare `ORI_OPT=none` with default/aggressive modes for programs whose semantics are supported by both.

Validate:

- output and exit behavior;
- side-effect order;
- overflow/trap behavior;
- cleanup and ownership;
- async/iterator/control-flow results;
- debug source mapping where the optimization promises preservation.

### Native versus C/debug subset

For the documented C/debug subset, compare behavior. For unsupported features, assert the explicit diagnostic rather than attempting comparison.

### Compiler versus reference model

Useful focused models include:

- constant-expression evaluator;
- collection operations;
- pattern exhaustiveness cases;
- package resolution/lockfile ordering;
- ARC ownership graph;
- formatter structural normalization.

The model should be simpler and independently implemented enough to catch shared-assumption bugs.

## Fixture design

Differential fixtures should:

- be deterministic;
- avoid environment-dependent output unless normalized;
- consume computed results;
- include managed and unmanaged values;
- include failure paths;
- expose cleanup effects through test hooks when required;
- remain small enough to diagnose.

Random generators may produce fixtures, but minimized deterministic cases become permanent regressions.

## Normalization

Normalize only non-semantic differences such as:

- temporary paths;
- platform line endings where the contract permits;
- process-specific IDs;
- unordered debug-only metadata not promised by the contract.

Do not normalize away:

- ordering that is language-visible;
- error codes;
- exit status;
- missing output;
- ownership/leak differences;
- target-independent numeric differences.

## Execution harness

A shared harness should record:

- source/project fixture;
- compiler version and commit;
- target;
- route/configuration;
- environment flags;
- runtime artifact and ABI version;
- timeout;
- stdout/stderr/status;
- optional leak/ARC counters;
- generated artifact hashes where relevant.

When a route fails to build, distinguish compilation failure from runtime difference.

## Property families

- AOT result equals JIT result.
- Optimized result equals unoptimized result.
- Incremental rebuild result equals clean rebuild result.
- Package resolution with a valid lock equals locked validation.
- Documentation/stdlib export is deterministic.
- Formatter output produces the same semantic result as original valid source.
- Generated C host bridge agrees with direct native call for supported ABI types.

## Test selection

Start with:

- arithmetic and control flow;
- structs/enums/patterns;
- result/optional/try;
- traits and generics;
- strings/bytes and collections;
- closures and iterators;
- async cleanup and tasks;
- filesystem/process fixtures using temporary roots;
- embedding aggregate cases.

Expand from real regressions and high-risk code changes.

## Failure workflow

1. Confirm the comparison is required by the support matrix.
2. Reproduce each route independently.
3. Minimize source and environment.
4. Identify the earliest differing phase or runtime operation.
5. Add a stable regression.
6. Fix the non-reference route—or correct the contract through governance if the reference behavior was wrong.
7. Update support/documentation if a route is deliberately excluded.

## CI strategy

- Run a focused deterministic differential set on PRs touching HIR, codegen, runtime, incremental, or JIT.
- Run a broader generated matrix on scheduled CI.
- Store failing source, route metadata, and outputs as artifacts.
- Use timeouts and isolated temporary directories.
- Track comparison count and uncovered feature families.

## Completion criteria for QA-DIFF-1

- reusable AOT/JIT harness;
- optimized/unoptimized comparison;
- clean/incremental comparison for representative projects;
- explicit C/debug supported-subset route;
- managed cleanup observations for high-risk fixtures;
- scheduled broader matrix;
- minimized-regression workflow.