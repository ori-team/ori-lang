# Ori Language Specification — Chapter 18: Stability and Compatibility

> Status: normative for the **S3 / `0.3.0`** surface cycle
> Audience: maintainers, contributors, users maintaining a project

---

Ori is still before `1.0`. Even so, the project must separate what is a
**public contract** from what is an **experiment**. That separation is what
keeps upgrades from surprising someone who is learning the language or
maintaining a small project.

---

## Stable contract of the current cycle

During the **S3 (`0.3.0` surface)** cycle, these points are public contract:

- `.orl` files in UTF-8;
- blocks terminated by `end` (labels optional: `end if`, `end match`, …);
- `module` required at the top of the file (`namespace` is an error);
- explicit imports in the three S3 forms: `import path (A)`,
  `import path = alias`, `import path` (no import `as` / `only`);
- explicit types on bindings, parameters, and public return types;
- composite types with `[]` (`optional[T]`, `result[T, E]`, `list[T]`, …);
- absence through `optional[T]`;
- failure through `result[T, E]`;
- propagation **only** through `try expr` (postfix `expr?` is an error);
- struct construction with `Type { field: value }`, or `{ field: value }` when
  the expected type is known;
- enum construction with `Enum.Variant(...)`, or `.Variant(...)` when the
  expected type is known;
- traits through `apply Type` + `use Trait`;
- the native backend as the semantic reference.

Changes to these points must be documented in `CHANGELOG.md` and need a
regression test.

### Added after the S3 cut

These forms are also public contract. They were added inside the `0.3.x` line
under FREEZE-1 as additive surface, each with regression tests:

| Form | Chapter |
|---|---|
| `newtype Name = Type` — nominal type, erased at lowering | [04](04-types.md) |
| `match` as an expression | [05](05-expressions.md) |
| `case a or b:` — or-patterns | [06](06-statements.md) |
| `if ok(v) = expr` / `if err(e) = expr` | [06](06-statements.md) |
| `const { field } = value` — destructuring bindings | [06](06-statements.md) |
| `alias Name = Type` inside a `use` section | [08](08-traits.md) |
| `Buffer[size: 8]` — const generics with named arguments | [11](11-generics.md) |

---

## Native binary contract (M3 + ABI-1)

The native backend documents its ABI in [`19-abi.md`](19-abi.md) under the tag
**`ori-native-abi-1`** (`ORI_ABI_VERSION` in `ori-runtime`). **ABI-1 is in
force** together with the FREEZE-1 window (see
`docs/planning/freeze-and-abi-gates.md`). It covers:

- layouts of primitives, structs, enums (tag `i32`), `optional` / `result` /
  tuples;
- the ARC header (`OriHeapHeader`) and the `ori_alloc` / `ori_arc_*` symbols;
- layouts of runtime collections (`OriList`, `OriMap`, `OriSet`, …);
- `ORI__*` mangling and the export of `main`;
- the `abi_version` check in `runtime-link.json`.

A change **incompatible** with already-staged runtimes requires bumping to
`ori-native-abi-N`, updating chapter 19, and re-staging staticlib + cdylib.
**Additive** `ori_*` symbols do not require a bump, but must be listed in the
stdlib manifest.

The C / debug backend is **not** an ABI reference.

---

## Experimental contract

These points may change before `1.0`:

- the final package and lockfile format;
- the hosted registry;
- REPL limits;
- stdlib APIs marked experimental;
- generic optimization and binary-size details;
- the public surface of the C / debug backend;
- fine details of local inference beyond the documented option B
  (literals + field / index / call / pipe); **global HM stays out**;
- package / registry formats not yet stabilized;
- final names of stdlib domain aliases beyond those already documented.

> **Already stabilized on the pre-1.0 surface (but still pre-1.0):** pipe `|>`
> (kept in Ori), Nim-style local inference + option B (`0.3.1`+), the native
> ABI `ori-native-abi-1` (M3), and the Rust-free install path (M1).

---

## Known gaps

Documented here so that nobody mistakes them for contract. Each is a real
limitation of the current compiler, not a planned removal:

| Gap | Detail |
|---|---|
| Higher-kinded types | Declaration parses, no implementation can satisfy it; **out of scope** by decision ([11](11-generics.md)) |
| `@c_export` aggregates | Structs, `list`, `map`, `optional`, `result` have no stable C layout; scalars and `string` do cross ([19](19-abi.md) §8.3b) |
| Infinite recursion | Not detected at compile time (undecidable). At runtime the stack guard reports `stack overflow` instead of dying on a bare signal |
| `Default` | A registered core trait with no methods: `default() -> Self` needs a receiver-less trait method, which is unsupported ([12](12-stdlib.md)) |
| Type names in backend errors | Codegen messages still print `<def DefId(N)>`; the checker no longer does |

---

## Documentation rule

The spec must describe what the parser, checker, runtime, and tooling accept
**today**. Future ideas belong in `docs/planning/`.

When a feature moves from planning to implementation, the same delivery must
update:

- the normative spec;
- examples or fixtures;
- tests;
- `CHANGELOG.md`;
- the planning docs, marking the item as delivered or changed.
