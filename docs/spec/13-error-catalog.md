# Ori Language Specification - Chapter 13: Diagnostic Error Catalog

> Status: normative for emitted diagnostics; informative for planned diagnostics  
> Audience: compiler implementers, tool authors  
> Surface: **S3** (`0.3.0`) + living **0.3.x** under FREEZE-1  
> Quality guide: `docs/planning/qa/` + skill `ori-lang-qa` / `references/diagnostics-quality.md`

---

## Message quality (normative intent)

Every **emitted** diagnostic should allow a reader to answer:

1. **What** failed (primary message).  
2. **Where** (primary span on the offending construct).  
3. **What to do** when the fix is mechanical (`action` / help note).  

Under **S3**, messages and actions **must not** recommend removed forms
(`namespace`, declaration `func`, `else if`, postfix `?`, `import as`, …).
Prefer the canonical form (`module`, `name(...)`, `elif`, `try expr`,
`import path = alias`).

Consistency gate: `cargo test -p ori-driver --test diagnostic_catalog`.

---

## Overview

Every diagnostic emitted by the Ori compiler has a unique code:

```text
category.specific_name
```

The tables below are split into two groups:

- **Emitted diagnostics**: codes that the compiler currently emits.
- **Planned or reserved diagnostics**: codes kept for docs, tools, or future implementation.

The compiler test suite checks that every emitted code appears in the emitted
section. If a code is documented as planned, it must move to the emitted section
when the compiler starts producing it.

---

## Emitted Diagnostics

### `lex`

| Code | Severity | Description |
|---|---|---|
| `lex.unexpected_character` | error | Lexer found a character that is not valid in Ori source |
| `lex.unclosed_block_comment` | error | Block comment starts with `--|` but is not closed with `|--` |

### `parse`

| Code | Severity | Description |
|---|---|---|
| `parse.byte_unicode_escape` | error | Byte string contains a Unicode escape; byte strings accept byte escapes only |
| `parse.chained_comparison` | error | Comparison chaining is not allowed |
| `parse.default_before_required` | error | Required parameter appears after a default parameter |
| `parse.expected_declaration` | error | Parser expected a top-level declaration |
| `parse.expected_expression` | error | Parser expected an expression |
| `parse.expected_extern_member` | error | Parser expected a member inside an `extern` block |
| `parse.expected_identifier` | error | Parser expected an identifier |
| `parse.expected_pattern` | error | Parser expected a pattern |
| `parse.expected_type` | error | Parser expected a type |
| `parse.fstring_empty_expr` | error | Interpolated string contains an empty expression |
| `parse.fstring_expr_trailing_tokens` | error | Interpolated string expression has extra tokens |
| `parse.fstring_unclosed_expr` | error | Interpolated string expression is not closed |
| `parse.fstring_unmatched_brace` | error | Interpolated string contains an unmatched brace |
| `parse.import_after_declaration` | error | Import appears after a top-level declaration |
| `parse.import_as_removed` | error | Source used removed `import path as alias`; use `import path = alias` |
| `parse.import_only_removed` | error | Source used removed `import path only (…)`; use `import path (…)` |
| `parse.invalid_escape` | error | String or byte literal contains an invalid escape |
| `parse.invalid_lvalue` | error | Assignment target is not assignable |
| `parse.invalid_range` | error | Range expression has a non-integer boundary |
| `parse.missing_else_in_if_expr` | error | Inline `if` expression is missing the required `else` branch |
| `parse.module_missing` | error | Source file is missing the leading `module` declaration |
| `parse.module_not_first` | error | `module` declaration appears after another declaration |
| `parse.namespace_removed` | error | Source used removed `namespace` keyword; use `module` |
| `parse.func_removed` | error | Source used removed `func` on a declaration; write `name(...)` directly |
| `parse.result_ctor_renamed` | error | `success`/`error` result constructors were renamed to `ok`/`err` |
| `parse.removed_angle_type` | error | Angle-bracket type arguments `Type<…>` are removed; use `Type[…]` |
| `parse.removed_of_type` | error | `of` / `map of K to V` type forms are removed; use `list[T]`, `map[K, V]`, … |
| `parse.removed_struct_call_literal` | error | Struct construction via `Type(...)`, `.{…}`, or guided `(…)` is removed; use `Type { … }` or `{ … }` |
| `parse.removed_where_bound` | error | `where T is Trait` / `where T is not Trait` bounds are removed; use `for T: Trait` |
| `parse.question_propagate_removed` | error | Source used removed postfix `?` propagation; use `try expr` |
| `parse.else_if_removed` | error | Source used removed `else if`; use `elif` for chained conditionals |
| `parse.case_dot_variant_removed` | error | Source used removed leading `.` on a match enum variant; write `case Variant` / `case Variant(...)` |
| `parse.implement_removed` | error | `implement Trait for Type` was removed; use `apply Type` with `use Trait` |
| `parse.apply_trait_to_removed` | error | `apply Trait to Type` / `apply Trait for Type` was removed; use `apply Type` with `use Trait` |
| `parse.apply_member_after_use` | error | Free methods/binds appear after a `use Trait` section (order is free members, then `use`) |
| `parse.associated_type_keyword_removed` | error | `type Name = …` for an associated type was removed; write `alias Name = …` |
| `apply.redundant_use_block` | error | An `apply` block whose whole body is one `use` section; write the compact header `apply Type use Trait` |
| `parse.poetic_call_nested` | error | Nested poetic call (juxtaposition of calls without parentheses) is not allowed; at most one poetic verb per expression |
| `parse.end_label_mismatch` | error | Optional labeled `end` (`end if`, `end match`, …) does not match the opening construct |
| `parse.do_removed` | error | Source used removed `do` closure keyword; write `(params) => expr` or `(params) … end` |
| `parse.tuple_arity` | error | Tuple type or expression has invalid arity |
| `parse.unterminated_block` | error | End-delimited block reaches end of file before `end` |
| `parse.unterminated_string` | error | String literal starts but is not closed |
| `parse.unexpected_token` | error | Parser found a token that is not valid here |
| `parse.variadic_not_last` | error | Variadic parameter is not the last parameter |

### `type`

| Code | Severity | Description |
|---|---|---|
| `type.ambiguous_method` | error | Method call matches more than one trait method for the receiver type |
| `type.anon_struct_field_mismatch` | error | Anonymous struct literal fields do not match the expected struct type |
| `type.anon_struct_type_unknown` | error | Anonymous struct literal is used without an expected struct type |
| `type.arg_count_mismatch` | error | Function call has the wrong number of arguments |
| `type.arg_type_mismatch` | error | Function call argument type does not match the parameter type |
| `type.arithmetic_type_mismatch` | error | Arithmetic operator received incompatible operand types |
| `type.comparison_not_supported` | error | Comparison operator is not supported for this type |
| `type.comparison_type_mismatch` | error | Comparison operands have incompatible types |
| `type.collection_comparable_unsupported` | error | Current ordered collection runtime does not support this element type without `Comparable` |
| `type.collection_hash_unsupported` | error | Current `map`/`set` runtime does not support this key or element type yet |
| `type.duplicate_arg_label` | error | Named argument is passed more than once |
| `type.enum_variant_named_fields_required` | error | Enum variant construction requires named fields |
| `type.expected_bool` | error | Expression must have type `bool` |
| `type.equality_unsupported_field` | error | Struct equality has a field whose type cannot be compared |
| `type.field_on_non_struct` | error | Field access was used on a non-struct value |
| `type.field_on_tuple_not_int` | error | Tuple field access must use an integer index |
| `type.hash_key_not_supported` | error | Computed hash key type is not supported by the current runtime |
| `type.if_branch_mismatch` | error | `if` branches produce incompatible types |
| `type.match_arm_mismatch` | error | `match` used as an expression has arms producing incompatible types |
| `type.ifsome_not_optional` | error | `if some` was used on a non-optional value |
| `type.destructure_not_struct` | error | A destructuring binding was applied to a value that is not a struct |
| `parse.empty_destructure` | error | A destructuring binding names no fields (`const { } = …`) |
| `type.ifok_not_result` | error | `if ok` was used on a value that is not a `result[T, E]` |
| `type.iferr_not_result` | error | `if err` was used on a value that is not a `result[T, E]` |
| `type.index_not_int` | error | Index expression must have type `int` |
| `type.iterable_next_missing` | error | A type implements `Iterable` but does not provide `next` |
| `type.iterable_next_signature` | error | `Iterable.next` does not match `mut next() -> optional[T]` |
| `type.is_target_not_type` | error | `is` target is not a valid type |
| `type.list_element_mismatch` | error | List literal elements have incompatible types |
| `type.local_inference_failed` | error | Local `const`/`var` omitted its type but the RHS is not obvious enough (`0.3.1` + option B: not literal/field/index/call/pipe with concrete type; or `try` / empty / `none` / `void`) |
| `type.map_key_mismatch` | error | Map literal keys have incompatible types |
| `type.map_value_mismatch` | error | Map literal values have incompatible types |
| `type.missing_return` | error | Non-void function may finish without returning a value |
| `type.missing_struct_field` | error | Struct literal omits a required field |
| `type.no_such_field` | error | Struct field does not exist |
| `type.no_such_method` | error | Method does not exist for this receiver type |
| `type.not_indexable` | error | Value cannot be indexed |
| `type.not_iterable` | error | `for` loop received a value that is not iterable |
| `type.not_sliceable` | error | Value cannot be sliced |
| `type.numeric_literal_invalid` | error | Numeric literal syntax or suffix is invalid |
| `type.numeric_literal_out_of_range` | error | Numeric literal does not fit its target type |
| `type.pattern_mismatch` | error | Pattern is incompatible with the matched value type |
| `type.positional_after_named_arg` | error | Positional argument appears after a named argument |
| `type.propagate_err_mismatch` | error | `try` would propagate an incompatible result error type |
| `type.propagate_not_result_or_optional` | error | `try` was used on a value that is not `result` or `optional` |
| `type.propagate_return_mismatch` | error | `try` was used in a function with an incompatible return type |
| `type.repeat_count_not_int` | error | `repeat` count must be an `int` |
| `type.return_mismatch` | error | Returned value type does not match function return type |
| `type.spread_non_list` | error | Spread argument is not a list |
| `type.spread_non_variadic` | error | Spread argument is used outside a variadic parameter |
| `type.set_element_mismatch` | error | Set literal elements have incompatible types |
| `type.struct_literal_named_fields_required` | error | Struct construction requires named fields |
| `type.tuple_index_on_non_tuple` | error | Tuple index access was used on a non-tuple value |
| `type.tuple_index_out_of_bounds` | error | Tuple index is outside the tuple arity |
| `type.type_mismatch` | error | Value type does not match the expected type |
| `type.unused_result` | warning | `result[T, E]` expression value is discarded |
| `type.unary_neg_non_numeric` | error | Unary `-` was used on a non-numeric value |
| `type.undefined_name` | error | Type name is not defined |
| `type.unknown_field` | error | A destructuring binding names a field the struct does not have |
| `type.unknown_arg_label` | error | Named argument does not match any parameter |
| `type.unknown_enum_variant` | error | Enum variant does not exist |
| `type.whilesome_not_optional` | error | `while some` was used on a non-optional value |

### `concurrency`

| Code | Severity | Description |
|---|---|---|
| `concurrency.not_transferable` | error | Value cannot cross a task or channel boundary because it is not `Transferable` |

### `contract`

| Code | Severity | Description |
|---|---|---|
| `contract.ok_void_mismatch` | error | `ok()` without a payload is used where the result success type is not `void` |

### `async`

| Code | Severity | Description |
|---|---|---|
| `async.capture_not_transferable` | error | Closure passed to `task.spawn` captures a value that is not `Transferable` |
| `async.await_outside_async` | error | `await` was used outside an `async` function (`async name(...)`) |
| `async.await_non_future` | error | `await` was used on a value that is not `future[T]` |

### `backend`

| Code | Severity | Description |
|---|---|---|
| `backend.c_unsupported` | error | C debug backend cannot generate code for a feature supported by the native route |
| `backend.native_unsupported` | error | Native backend rejected a typed HIR shape before Cranelift because that codegen path is not implemented yet |

### `native`

| Code | Severity | Description |
|---|---|---|
| `native.abi_mismatch` | error | Packaged native runtime metadata uses an ABI version that does not match the driver |
| `native.link_failed` | error | Native linker ran but failed to produce an executable |
| `native.linker_missing` | error | Native linker driver or configured native linker could not be started |
| `native.runtime_metadata_invalid` | error | Native runtime metadata is missing or malformed |
| `native.runtime_metadata_mismatch` | error | Native runtime metadata targets a different compiler version, target, or artifact name |
| `native.runtime_missing` | error | Native runtime library could not be found or built |
| `native.runtime_symbol_missing` | error | Native linker reported an unresolved runtime/backend symbol |

### `bind`

| Code | Severity | Description |
|---|---|---|
| `bind.alias_shadows_local` | error | Import alias shadows a local definition |
| `bind.alias_shadows_builtin_type` | error | Import alias shadows a built-in type name |
| `bind.const_reassignment` | error | Code tries to reassign a `const` binding |
| `bind.duplicate_alias` | error | More than one import uses the same alias |
| `bind.duplicate_field` | error | Struct or enum variant field is declared more than once |
| `bind.duplicate_implement` | error | Same trait/type implementation pair is declared twice |
| `bind.duplicate_param` | error | Function, method, or signature parameter is declared more than once |
| `bind.duplicate_variant` | error | Enum variant is declared more than once |
| `bind.import_ambiguous` | error | Import path matches more than one file |
| `bind.import_member_unknown` | error | Selective import names a member that the source module does not export |
| `bind.import_not_found` | error | Imported module could not be resolved to a file |
| `bind.self_outside_method` | error | `self` is used outside method scope |
| `bind.shadowing` | error | Binding shadows another binding in the same scope |
| `bind.stdlib_module_unknown` | error | Standard library module name is unknown |
| `bind.stdlib_module_unavailable` | warning | Standard library function is not yet available in the native runtime |
| `bind.unused_import` | warning | Private import is not used |

### `attr`

| Code | Severity | Description |
|---|---|---|
| `attr.deprecated` | warning | Deprecated declaration is used |
| `attr.duplicate` | warning | Attribute is repeated on the same declaration |
| `attr.invalid_arg` | error | Attribute arguments do not match the supported form |
| `attr.invalid_test_signature` | error | `@test` function has parameters, type parameters, or a return value |
| `attr.invalid_target` | error | Attribute is applied to a declaration kind that does not support it |
| `attr.unknown` | error | Attribute name is not part of the current built-in attribute set |
| `attr.c_export_not_public` | error | `@c_export` used on a non-`public` function |
| `attr.c_export_async` | error | `@c_export` used on an async function |
| `attr.c_export_generic` | error | `@c_export` used on a generic function |
| `attr.c_export_bad_type` | error | `@c_export` parameter or return type is not FFI-safe for phase 1 |

### `doc`

| Code | Severity | Description |
|---|---|---|
| `doc.missing_public` | warning/error | Public symbol is undocumented while `docs.require_public` is configured as `warn` or `error` |
| `doc.missing_return` | warning | Documentation for a non-void function is missing `@return` or `@returns` |
| `doc.param_name_mismatch` | warning | Documentation `@param` tag names a parameter that does not exist on the documented function |
| `doc.symbol_not_found` | error | `.oridoc` entry targets a symbol that does not exist in the loaded project |
| `doc.syntax` | error | `.oridoc` file is malformed |

### `name`

Name-resolution diagnostics. The `name.*` prefix covers undefined, private, and
top-level duplicate names. Binding-specific duplicates (fields, params, variants,
aliases) use the `bind.*` prefix instead — see the `bind` section below.

| Code | Severity | Description |
|---|---|---|
| `name.duplicate` | error | Name is already defined in this module |
| `name.private` | error | Code tries to access a non-public imported item |
| `name.undefined` | error | Value name is not defined |

### `control`

| Code | Severity | Description |
|---|---|---|
| `control.loop_required` | error | `break` or `continue` is used outside a loop |

### `match`

| Code | Severity | Description |
|---|---|---|
| `match.duplicate_case` | warning | Match arm repeats an earlier unguarded pattern |
| `match.non_exhaustive` | error | `match` does not cover all possible cases |
| `match.or_pattern_binding` | error | an alternative in an `or` pattern binds a value; alternatives must be binding-free |
| `match.unreachable_case` | warning | Match arm appears after an unguarded catch-all case |

### `mut`

| Code | Severity | Description |
|---|---|---|
| `mut.closure_captures_var` | error | Closure captures a mutable binding |
| `mut.const_method_call` | error | `mut func` is called on a `const` receiver |
| `mut.const_mutation` | error | Code tries to mutate a `const` value |
| `mut.field_mutation_in_func` | error | Non-`mut` method mutates `self` state |
| `mut.using_binding_mutated` | error | `using` binding is reassigned |

### `generic`

| Code | Severity | Description |
|---|---|---|
| `generic.constraint_not_satisfied` | error | Type does not satisfy a generic constraint |
| `generic.constraint_not_trait` | error | Generic constraint target is not a trait |
| `generic.circular_instantiation` | error | Generic function recursively instantiates itself without a concrete type |
| `generic.negative_constraint_violated` | error | Type violates a negative generic constraint |
| `generic.unknown_type_param` | error | Generic constraint references an unknown type parameter |

### `impl`

| Code | Severity | Description |
|---|---|---|
| `impl.missing_method` | error | `apply Type` / `use Trait` omits a required trait method |
| `impl.mut_mismatch` | error | Trait method mutability does not match implementation |
| `impl.trait_not_found` | error | Trait named in a `use Trait` section does not exist |
| `impl.type_not_found` | error | Type named in an `apply Type` block does not exist |
| `impl.wrong_signature` | error | Implemented method signature does not match the trait |

### `using`

| Code | Severity | Description |
|---|---|---|
| `using.not_disposable` | error | `using` value does not satisfy the disposable contract |

### `extern`

| Code | Severity | Description |
|---|---|---|
| `extern.managed_type_in_ffi` | error | `extern` member uses an Ori-managed type at the raw FFI boundary |
| `extern.unknown_abi` | error | `extern` block names an unsupported ABI |

### `project`

Project-level diagnostics emitted while loading the entry file and its
transitive imports. The `project.*` module consolidates configuration and
import-graph failures that span more than one source file.

| Code | Severity | Description |
|---|---|---|
| `project.circular_import` | error | Local imports form a cycle |
| `project.namespace_file_mismatch` | error | Imported file declares a different module |
| `project.entry_not_found` | error | Project entrypoint declared in `ori.proj` does not exist, or the manifest is missing an `entry` key |
| `project.no_proj_file` | error | Project manifest (`ori.proj`) was not found at the workspace root |

---

## Planned Or Reserved Diagnostics

These codes are documented for future work or for runtime/tooling contracts.
They are not emitted by the compiler today.

### Reserved aliases

Each code below is a reserved alias. The compiler emits the more specific
code listed in the "Emitted as" column instead. Tools that match on
diagnostic codes should accept the alias for compatibility, but the
compiler will not produce it.

| Code | Intended severity | Emitted as |
|---|---|---|
| `bind.undefined` | error | `name.undefined` |
| `type.callable_mismatch` | error | `type.arg_count_mismatch` / `type.arg_type_mismatch` |
| `type.constraint_not_satisfied` | error | `generic.constraint_not_satisfied` |
| `type.incompatible_result_error` | error | `type.propagate_err_mismatch` |
| `type.index_non_indexable` | error | `type.not_indexable` |
| `type.invalid_is_check` | error | `type.is_target_not_type` |
| `type.mismatch` | error | `type.type_mismatch` |
| `type.propagation_context` | error | `type.propagate_*` (more specific) |
| `type.undefined` | error | `type.undefined_name` |

The code `async.using_unsupported` is **obsolete** — `using` inside an `async` function
is now allowed. Do not emit this diagnostic.

---

## Removed From v1 Catalog (Audited 2026-06-29)

The following codes were listed as planned in earlier catalog revisions.
The Etapa 7 nomenclature audit determined that each is either redundant
with an existing emitted code, not applicable to Ori's explicitly-typed
design, or deferred to v2. They are no longer tracked as planned.

| Code | Reason for removal |
|---|---|
| `contract.check_failure` | Runtime contract checking is not implemented in v1; deferred to v2. Compiler does not emit; runtime does not use the code string. |
| `contract.field_violation` | Same as `contract.check_failure` — runtime-only, deferred to v2. |
| `contract.param_violation` | Same as `contract.check_failure` — runtime-only, deferred to v2. |
| `doc.unclosed_block` | Redundant with `lex.unclosed_block_comment`, which already covers unclosed `--| ... |--` block comments. Doc comments use the same delimiter syntax. |
| `generic.ambiguous_type_arg` | Ambiguous type argument inference is reported via `type.type_mismatch` when inference fails; a dedicated code is deferred to v2. |
| `match.guard_not_exhaustive` | Guard exhaustiveness analysis is not implemented in v1; `match.non_exhaustive` covers unguarded cases. Guarded exhaustiveness deferred to v2. |
| `type.ambiguous_generic` | Alias for existing generic diagnostics (`type.type_mismatch`, `generic.constraint_not_satisfied`); no separate emission needed. |
| `type.annotation_required` | Superseded for locals by `type.local_inference_failed` (`0.3.1`). Top-level and API positions still require annotations via the grammar. |
| `using.non_result_init` | `using` accepts any `Disposable` value; non-disposable inits are reported by `using.not_disposable`. The `result`-specific variant is not part of the v1 `using` contract. |

---

## Diagnostic Format Contract

Every diagnostic should provide:

1. **Code**: machine-readable identifier.
2. **Severity**: `error` or `warning`.
3. **Short message**: one line, present tense, no period.
4. **Span**: file, line, column of the primary location.
5. **`= why:`**: explanation of the rule, when useful.
6. **`= action:`**: what the programmer should do, when useful.

Optional:

- Secondary spans.
- Notes with migration hints or related rules.

Example:

```text
error[generic.constraint_not_satisfied]: type does not satisfy constraint
  --> src/app/sort.orl:8:14
   |
8  |    const sorted: list[User] = list.sort(users)
   |                               ^^^^^^^^^
   |
   = action: implement the required trait or use a value with a supported type
```
