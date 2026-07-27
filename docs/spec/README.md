# Ori language specification

> Status: **normative for the implemented 0.3.8 language and native runtime contracts**  
> Language surface: **S3**  
> Native ABI: **`ori-native-abi-1`**  
> Audience: language designers, compiler/runtime implementers, tooling contributors

This directory defines behavior accepted by the current Ori implementation. It must not contain aspirational features presented as current behavior.

## Normative role

The specification owns:

- source syntax and semantics;
- modules, visibility, and declarations;
- types, expressions, statements, traits, and generics;
- errors, memory, and standard-library contracts;
- backend support boundaries;
- runtime FFI safety;
- project and documentation formats;
- stability and compatibility;
- native ABI layouts and symbols.

Architecture explains how these contracts are implemented. RFCs describe proposals. Planning documents sequence accepted work.

## Chapters

| Chapter | Subject |
|---|---|
| [`00-manifesto.md`](00-manifesto.md) | Identity and purpose |
| [`01-overview.md`](01-overview.md) | Language overview and core model |
| `02-*` | Modules, declarations, and grammar foundations |
| [`04-types.md`](04-types.md) | Type system and representations visible to the language |
| [`05-expressions.md`](05-expressions.md) | Expressions and evaluation |
| [`06-statements.md`](06-statements.md) | Statements and control flow |
| [`07-functions.md`](07-functions.md) | Functions and callable behavior |
| [`08-traits.md`](08-traits.md) | Traits, applications, and dispatch |
| [`09-errors.md`](09-errors.md) | Error, optional, result, and propagation model |
| [`10-memory.md`](10-memory.md) | ARC, cycle collection, cleanup, and destruction |
| [`11-generics.md`](11-generics.md) | Generics, constraints, and const arguments |
| [`12-stdlib.md`](12-stdlib.md) | Public standard-library contract |
| [`13-error-catalog.md`](13-error-catalog.md) | Emitted diagnostic-code catalog |
| [`14-backend-support.md`](14-backend-support.md) | Feature support by native, JIT, and C/debug routes |
| [`15-stdlib-maintenance.md`](15-stdlib-maintenance.md) | Normative maintenance and parity requirements |
| [`16-runtime-ffi-safety.md`](16-runtime-ffi-safety.md) | Runtime FFI safety contract |
| [`17-project-and-docs.md`](17-project-and-docs.md) | Project, package-adjacent, and documentation formats |
| [`18-stability-and-compatibility.md`](18-stability-and-compatibility.md) | Public versus experimental contracts |
| [`19-abi.md`](19-abi.md) | Native ABI `ori-native-abi-1` |

There is no standalone Chapter 03. Grammar is currently distributed across the syntax chapters. A future consolidated grammar must be generated or validated against those chapters rather than becoming a competing source.

## Current product facts

| Fact | Current contract |
|---|---|
| Project/compiler version | `0.3.8` |
| Surface | S3 |
| Function/module naming | `snake_case` |
| Type naming | `PascalCase` |
| Visibility | `public` |
| Absence | `optional[T]` |
| Recoverable failure | `result[T, E]` |
| Propagation | `try expression` |
| Traits | `apply Type` + `use Trait` |
| Memory | ARC plus cooperative cycle collection |
| Semantic reference | Cranelift native backend |
| Run route | JIT when compatible runtime cdylib is available; AOT otherwise/when selected |
| Native ABI | `ori-native-abi-1` |

Historical introduction versions belong in the changelog or an explicit “introduced in” note. They must not replace `0.3.8` as the current version.

## Specification writing rules

1. Describe implemented behavior today.
2. Use current S3 syntax in active examples.
3. Separate normative requirements from explanatory notes.
4. Name invalid and unsupported behavior explicitly.
5. Link diagnostic codes to Chapter 13.
6. Link backend limitations to Chapter 14.
7. Link memory and FFI requirements to Chapters 10, 16, and 19.
8. Include compatibility and migration consequences for changed contracts.
9. Add conformance evidence for significant normative rules.
10. Keep the specification English-only as the single normative language source.

## Change requirements

A normative behavior change must update, as applicable:

- specification chapter;
- conformance mapping and tests;
- diagnostic catalog;
- formatter and LSP behavior;
- AOT/JIT and C/debug support classification;
- runtime/ABI contract;
- user examples and maintained translations;
- compatibility/versioning policy;
- changelog;
- accepted RFC/ADR and implementation plan.

## Related navigation

- Documentation ATLAS: [`../ATLAS.md`](../ATLAS.md)
- Product status: [`../product/status.md`](../product/status.md)
- Compiler architecture: [`../architecture/compiler-pipeline.md`](../architecture/compiler-pipeline.md)
- Implementation standards: [`../implementation/standards.md`](../implementation/standards.md)
- Conformance: [`../quality/language-conformance.md`](../quality/language-conformance.md)
- Language evolution: [`../governance/language-evolution.md`](../governance/language-evolution.md)