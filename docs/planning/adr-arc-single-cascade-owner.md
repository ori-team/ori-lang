# ADR — ARC cascade has a single owner: registered edges

> **Status:** accepted (2026-07-17)
> **Context:** LANG-MEM-1 / plan F1 ([`plano-arc-nim-2026-07-16.md`](plano-arc-nim-2026-07-16.md));
> study note [`historico/nim-study-2026-07-17-c1.md`](historico/nim-study-2026-07-17-c1.md).

## Problem

Composite owners (structs, enums, tuples) released their managed fields
**twice** on free: once through the generated `__dtor_*` destructor hook
installed at `ori_alloc`, and once through the ARC edges the codegen
registers for the same fields (each edge holds a +1 on the child).

Consequences (all reproduced by tests before the fix):

- A child shared between a live binding and a dying owner was freed while
  the binding was still in scope (use-after-free; `memory_arc.rs`
  `shared_child_*` tests).
- Element/field slots whose stores never released the owned temporary's
  +1 leaked — masked in composites by the double release, **unmasked** in
  lists (nested list literals and `lists.push` leaked in real programs).

## Decision

**Registered ARC edges are the only owner of a stored managed child.**

1. Composite allocations (struct/enum/tuple literals) install **no**
   destructor hook. Cascading release happens exclusively through
   `free_registered_object` / `ori_arc_collect_cycles` releasing the
   owner's registered edges.
2. Uniform store rule in codegen: **store → register/update edge →
   release the temporary's own +1 if the value expression produced an
   owned reference.** Borrowed references (loads from bindings/fields)
   keep their existing +1 untouched.
3. `ori_arc_register_edge` keeps retaining the child (+1); "transfer of
   ownership" is expressed by the paired release of the owned temporary,
   not by skipping the retain (borrowed sources need the retain).

## Alternative rejected

**Nim-style: destructor is the owner, edges become trace-only.** Nim's
`=destroy` releases fields; `=trace` only walks the graph and holds no
reference. Adopting that in Ori would require adding destructors to every
managed container that today relies on owning edges (optional, result,
closures env, async frames, lists) and stripping the +1 from
`ori_arc_register_edge` — a much larger surgery with the same end state.
The direction remains worth revisiting if the collector ever moves to
type-driven tracing (plan F3+ may reopen this with an explicit ADR).

## Consequences

- `native_backend.rs` no longer declares/defines `__dtor_struct_*`,
  `__dtor_enum_*`, `__dtor_tuple_*` (dead code removed; smaller binaries).
- The `destructor` hook in `OriHeapHeader` remains for **runtime-internal**
  allocations (lists' internal storage etc.) — it must not release
  compiler-registered children.
- Edge completeness becomes load-bearing for cascade correctness, not just
  for cycle collection: a missing edge is now a leak instead of being
  hidden by the dtor. That is exactly the audit plan F2 (LANG-MEM-2)
  performs; C backend keeps its own inline ARC model (debug backend only).
- Spec 10's "Type-specific destructors" section is superseded by the
  "single cascade owner" contract.
