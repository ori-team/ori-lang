# Implementation Audit and Correction Roadmap

> **Status:** closed audit/correction record (2026-09-04); findings and command
> transcripts below are historical evidence, not current implementation or CI
> requirements. C source emission and generated-C tests were retired on
> 2026-09-05; see [Spec 14](../spec/14-backend-support.md). Current work belongs
> in [BACKLOG.md](BACKLOG.md).
>
> **Audit date:** 2026-08-29 (implementation reconciliation: 2026-09-01)
>
> **Audited baseline:** external audit `ori_lang_auditoria_completa.md` (provided alongside this repository on 2026-08-29), confronted with the current working tree, Ori S3 + workspace `0.3.8-dev`
>
> **Toolchain:** Rust 1.95.0 from `rust-toolchain.toml`
>
> **Owner of open work:** [`BACKLOG.md`](BACKLOG.md)
>
> **Scope:** compiler, HIR optimizers, native/JIT/C backends, runtime, embedded API, package manager, LSP, stdlib, tests, QA, release automation, and implementation-status documentation

This document replaces the earlier 2026-08-17 audit narrative. That narrative
trusted file presence and happy-path tests too much. Several items described as
implemented are scaffolding, no-op passes, partial prototypes, or behavior that
is not connected to the compiler pipeline. This revision records what the code
actually does, the risk, the correction, and the evidence required before an
item can be closed again.

The normative specification still defines intended language behavior. It is
not evidence that the implementation currently satisfies that behavior.

---

## 1. Audit method and confidence model

The audit did not infer completion from names, comments, changelog entries, or
the existence of a test. Each claim was checked through as many of these levels
as the implementation permits:

1. **Definition:** the code or API exists.
2. **Reachability:** a production caller connects it to the active pipeline.
3. **Behavior:** a test observes the promised semantics, including failure paths.
4. **Effect:** an optimizer/performance feature proves an IR, instruction,
   allocation, or timing change rather than only equal program output.
5. **Gate:** CI fails when the property regresses.

Evidence labels used below:

| Label | Meaning |
|---|---|
| **Confirmed** | source path and a reproducer or direct call-graph proof agree |
| **Strong** | direct source proof; no destructive exploit was executed |
| **Gap** | promised behavior has no production integration or adequate proof |
| **Intentional limit** | explicitly unsupported behavior; not a defect unless it crashes or lies |

Priority means:

| Priority | Meaning |
|---|---|
| **P0** | semantic or memory-safety release blocker |
| **P1** | crash, supply-chain, data-integrity, or central-correctness defect |
| **P2** | important incompleteness, performance, or maintainability work |
| **P3** | hygiene, debt, or future capability |

### Validation performed

- `cargo check --workspace --locked` — passed on 2026-08-31.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed on 2026-08-31.
- `python3 tools/qa/rustfmt_scoped.py --check` and `git diff --check` — passed;
  the unrelated historical full-workspace formatting drift remains a scoped
  P2 cleanup.
- `bash tools/qa/daily_fast.sh` — passed on 2026-08-31, including 247
  `ori_spec` tests, diagnostic catalog, memory/security, and residual surface.
- Focused regressions — runtime 100/100, embed 38/38, LSP 31 unit + 13 e2e,
  package 14/14, native backend 48/48, and C ASan+UBSan 2/2.
- `cargo audit` — passed with no reported vulnerability.
- `bash tools/smoke_native_release.sh --skip-build` — passed on Linux,
  including AOT `ori compile`, `ori test`, packaged runtime identity, JIT run,
  `ori doctor`, and the real `ori-lsp` JSON-RPC initialize/shutdown/exit
  handshake. The JIT-only mode also passed after a clean release build.
- Reproducible release archive — passed on 2026-08-31 after making native
  emission order key-stable and pruning `.ori` caches during traversal:
  `tools/qa/archive_repro_smoke.sh` creates two roots with different `.ori`
  cache contents and obtains byte-identical same-epoch archives. A full staged
  package was also archived twice and compared byte-for-byte.
- A full serial workspace run on 2026-08-31 reached all prior suites but first
  exposed an incompatible C `any` function-pointer call under UBSan. The typed
  trampoline fix was then validated by the dedicated 2/2 sanitizer suite;
  rerunning the entire 17-minute matrix is intentionally left to CI/nightly.

Cross-platform Windows/macOS release execution, sanitizer/TSan stress, Miri,
and ignored performance probes remain evidence work under P2 QA rows; they are
not silently represented as passed on this Linux host.

---

## 2. Executive result — implementation truth after external-audit reconciliation

The external report is useful evidence, but it mixes historical snapshots,
normative intent, and current behavior. The current tree has already closed
several of its alleged P0s: enum transferability is cycle-safe, byte exports
are length-aware, duplicate ARC edges have regression coverage, hosted values
have an ownership contract, and the native ABI is implemented. Those items are
not reopened merely because the report predates their fixes.

The report also found real language gaps that were not safe to call “done”:

- direct reads/writes of top-level mutable `var` values from `task.spawn` are
  now rejected by the checker (`concurrency.global_mutable_capture`), including
  same-module and imported named-helper calls through conservative call-graph
  fixed points; receiver calls and `any[Trait]` dispatch now use conservative
  method-name matching. Named functions and local closures with an audited
  capture summary can cross the boundary; unknown function-value environments
  and complete type-level isolation remain open;
- user-defined struct `Equatable` semantics now drive native `==`/`!=`,
  map/set/hash-table membership, and generic graph node/edge lookup. Enum
  structural equality now works for direct comparisons, and explicitly-
  `Equatable` enums can be graph nodes; non-recursive enums with `Hashable` now
  also use structural equality in collections. Custom map/set/hash-table
  collections use generated structural hash callbacks for non-recursive values;
  explicit non-structural equality uses a constant-hash correctness fallback
  when no user `Hashable` method is supplied. A `hash(self) -> int` method now
  drives the generated callback when present. Recursive aggregate admission,
  callback ABI documentation, and collection performance remain open;
- `handle[T]` has syntax and lowering, remains unmanaged/non-transferable, and
  export aggregates containing borrowed handles are now rejected; the safe
  `ori.handle.is_null` sentinel probe and pointer-identity `==`/`!=` are
  implemented, while nullability, lifetime, FFI, and task-transfer contract
  are still not complete;
- async liveness still uses a conservative backend heuristic. A pre-emission
  verifier now checks frame slot bounds, overlap, native layouts, and
  zero-initialized managed await bindings; full HIR data-flow ownership proof
  remains open;
- several stdlib APIs still collapse operational failures into `string`; this
  remains the open typed-error contract. Blocking async file/connect/TLS
  operations now use a shared bounded worker pool, so one thread is no longer
  created per request.

These are language-first P0/P1 work, not tooling polish. The single active
implementation list is [`BACKLOG.md`](BACKLOG.md); the ordered wave below is
the execution plan for the next slices.

### Language-first implementation wave (2026-09-01)

| Order | Priority | Work | Size | Exit condition |
|---:|:---:|---|:---:|---|
| 1 | P0 | Global mutable state at task boundaries | M | Direct, same-module, imported named-helper, receiver/dynamic method, pure named-function, and audited local-closure access checked; unknown function-value environments and full isolation remain |
| 2 | P0 | Coherent `Equatable` + `Hashable` collection dispatch | L | Distinct equal keys behave identically in every native collection; recursive admission and benchmarked performance are explicit |
| 3 | P0 | `handle[T]` ownership and transfer contract | M | Borrowed aggregates rejected; typed null constructor, null probe, and pointer identity are defined; safe accessors, host lifetime, and foreign-thread affinity remain |
| 4 | P0 | Foreign string ownership in exported aggregates | M | Every aggregate boundary copies strings/bytes or validates opaque-handle provenance, payload size, and concrete source-type tag before dereference |
| 5 | P0 | Async ownership/liveness verifier | XL | No managed value can be used after frame cleanup or lost before await; slot/layout guard is present, HIR proof remains |
| 6 | P1 | Generic graph semantics and linked-list contract | L | Generic keys and advertised complexity are true or explicitly rejected |
| 7 | P1 | Typed stdlib errors | XL | Operational failures have stable typed values |
| 8 | P1 | Bounded channels and shared async I/O workers | L | **Done 2026-09-01:** positive bounded channels apply backpressure and shared blocking I/O uses a 4-worker/256-job pool |
| 9 | P1 | Attribute contract and inactive-attribute cleanup | M | **Done 2026-09-01:** unsupported namespaced attributes emit `attr.unknown`; every accepted attribute has a checker schema |
| 10 | P2 | Performance, LSP, release, sanitizer, and future capabilities | L+ | Only after language rows above are green |

The order is deliberately language before tools/QA. QA gates prove each slice,
but they do not outrank unresolved language semantics.

### Current slice evidence (2026-09-01)

- `cargo test -p ori-runtime --lib` — **100/100** passed, including bounded
  channel backpressure/closure and shared I/O worker failure behavior.
- `cargo test -p ori-driver --test concurrency_async` — **75/75** passed,
  including the bounded-channel checker/native regressions and nested closure
  transfer summaries.
- `cargo test -p ori-driver --test multifile_imports check_rejects_namespaced_attribute_until_schema_support_exists`
  — **1/1** passed, proving unsupported namespaced metadata fails closed.
- `cargo test -p ori-driver --test multifile_imports compile_runs_handle_null_constructor_native`
  — **1/1** passed, proving `ori.handle.null()` produces the typed null
  sentinel without ownership or dereference side effects.

- `cargo test -p ori-driver --test concurrency_async` — **75/75** passed,
  including immutable-global acceptance and mutable-global read/write
  rejection, same-module helper-call rejection, imported free-helper rejection,
  imported associated-helper rejection, concrete/imported receiver-method and
  dynamic `any[Trait]` rejection, plus native execution of pure named and
  local-closure function values passed directly to `task.spawn`;
- `cargo test -p ori-driver --test diagnostic_catalog` — **3/3** passed,
  including the new diagnostic row;
- `cargo test -p ori-driver --test multifile_imports compile_updates_top_level_mutable_global`
  — **1/1** passed, proving normal (non-task) global mutation remains valid;
- `cargo test -p ori-driver --test multifile_imports c_export_` — **14/14**
  passed, including a C host that proves valid managed handles round-trip and
  an invalid foreign pointer is rejected before user code;
- `cargo test -p ori-driver --test multifile_imports` — **390/390** passed,
  including the managed-handle ASan/UBSan host fixture when available and the
  typed handle-null native regression;
- `cargo test -p ori-driver --test multifile_imports compile_runs_graph_user_defined_equatable_node_native`
  — **1/1** passed, proving equivalent-by-value struct nodes are found and
  matched by undirected edges through the native graph callback, including
  `clone` and `transitive_closure` copies;
- `cargo test -p ori-driver --test multifile_imports compile_runs_graph_enum_node_structural_equality_native`
  — **1/1** passed, proving direct structural enum equality plus an explicitly-
  `Equatable` enum graph callback for node and edge lookup;
- `cargo test -p ori-runtime --lib graph_custom_nodes_use_equatable_callback_for_lookup_and_edges`
  — **1/1** passed, covering the runtime callback and ARC cleanup directly;
- `cargo test -p ori-driver --test multifile_imports collections::compile_runs_user_defined_equatable_keys_by_value_native`
  — **1/1** passed, proving distinct user values that compare equal work in
  native `map`, `set`, and `hash_table` operations;
- `cargo test -p ori-runtime --lib custom_map_and_set_use_hash_callback_and_repair_dense_slots`
  — **1/1** passed, proving generated-style hash/equality callbacks use the
  open-addressing path and keep dense-index slots correct after removal;
- `cargo clippy -p ori-types -p ori-driver --tests -- -D warnings` — passed.

The full workspace and release scripts have older dated snapshots in §1. They
must be rerun after each language wave; a focused green slice is not a claim
that every open backlog row is complete.

### External-audit confrontation matrix

| Finding from the external report | Evidence in the current tree | Real status | Track |
|---|---|---|---|
| Enum `Transferable` payloads | Cycle-safe `(DefId, args)` recursion in `ori-types`; spawn regression | Closed for the covered contract | Historical fix |
| Bytes containing NUL | Length-aware runtime payloads and embedded-NUL regressions | Closed for managed bytes; foreign pointer provenance remains | `AUD-BYTES-1` |
| Duplicate ARC edges | Slot-preserving edge registry and AOT/JIT cycle tests | Closed | `AUD-ARC-1` |
| Mutable global or resource handle used by a spawned task | Top-level `var` is legal for ordinary code; task-boundary checker guard covers direct, same-module, imported named free/associated-helper calls, receiver/dynamic method names, pure named functions, and local closures with audited captures. OS resource handles are explicitly non-transferable; cancellation tokens remain transferable. | Partial: unknown function-value environments and complete type-level isolation remain open | `CONC-THREADS-1` |
| Async value liveness across `await` | State-machine cleanup and 53 async regressions; pre-emission verifier checks frame slot bounds/overlap/layout and managed binding initialization | Partial: conservative use-based liveness remains and a complete HIR ownership proof is open | `LANG-OWNERSHIP-VERIFY-1` |
| User `Equatable`/`Hashable` in collections | Equality callbacks now drive native map/set/hash-table membership for user-defined structs and non-recursive enums with structural or explicit Equatable; generated structural hash callbacks drive non-recursive probes, and an optional user `hash(self) -> int` method overrides them; generic graph nodes use the same equality callback | Partial: recursive aggregate admission and performance contract open; explicit non-structural equality without `hash` uses constant-hash fallback; structural enum equality is available for direct comparisons | `LANG-COLL-EQHASH-1` |
| Generic graph keys | User-defined struct and non-recursive enum nodes with `Hashable` dispatch structural or explicit `Equatable` for add/find/edge/traversal and preserve the callback through clone/closure; constructor type is resolved on first concrete node operation | Partial: recursive/generic edge cases and linked-list contract remain open | `LANG-GRAPH-LIST-1` |
| `handle[T]` semantics | Syntax/lowering exist as borrowed unmanaged pointer; `ori.handle.null()` constructs a typed null sentinel, `ori.handle.is_null` probes it, `@c_export` aggregates containing handles are rejected, and `==`/`!=` compare identity | Partial: safe access, host lifetime, and foreign-thread affinity remain open | `LANG-HANDLE-1` |
| Foreign string escaping from aggregate returns | Managed aggregates are opaque handles; direct/optional/result strings and bytes are copied, and wrappers reject null/unregistered, wrong-size, or same-size wrong-type handles before dereference for concrete non-generic layouts | Partial P0 boundary: generated generic aggregate exports are rejected and the Linux host fixture is sanitizer-covered; a cross-platform hostile foreign-host matrix remains | `LANG-FFI-1` |
| Structured concurrency | Tokens and cancellation exist; no child-task tree or scope join | Partial | `ASYNC-STRUCT-1` |
| Bounded channels | `channel.create_bounded` returns an optional typed channel; positive capacities apply FIFO backpressure, invalid capacities return `none`, and close wakes blocked senders | Implemented with runtime and native regressions | `LANG-CHANNEL-1` |
| Async filesystem worker pool | Blocking FS/connect/TLS futures share a bounded FIFO queue (256 jobs) and a lazily-created pool capped at four workers; spawn failure and shutdown complete futures deterministically | Implemented; throughput/OS-reactor tuning remains P2 | `LANG-IO-POOL-1` |
| Typed stdlib errors | Process/crypto/FS APIs still expose `string` or sentinel values | Open language/stdlib contract | `LANG-STD-ERRORS-1` |
| Linked-list complexity | Public linked-list names wrap `VecDeque` | Open naming/complexity mismatch | `LANG-GRAPH-LIST-1` |
| Custom attributes | Unsupported namespaced attributes fail closed with `attr.unknown`; only built-in attributes have schemas | Implemented for the reject-unsupported contract; third-party schemas remain future work | `META-ATTR-1` |
| Explicit `self` | Checker and trait spec require explicit `self`; one old function example was corrected | Implemented; documentation drift fixed | `AUD-HYGIENE-1` |
| Host `extern` ABI | Parser, HIR, `ori-embed`, and ABI docs are connected | Implemented for ABI v1 scope | `EMBED-HOST-1` |
| C backend parity | Native AOT/JIT is the reference; C is an intentional sync/debug subset | Intentional partial, not a language blocker | `BACK-C-1` |
| Free-list/SSO/ARC sharding | Measurements show free-list regression; SSO is ABI-sensitive; sharding lacks data | Deferred until a benchmark proves need | P2 performance rows |
| Self-hosting, macros, HKT, specialization | No implementation and no product need in the current surface | Deferred by language decision | M4 / rejected rows |

“Closed” in this table means the specific contract has executable evidence; it
does not erase adjacent gaps. “Partial” and “Open” are deliberately visible in
the backlog so a passing happy-path test cannot silently close them.

---

## 3. Release-blocking correctness and memory safety

### AUD-OPT-1 — DCE removes observable traps

**Priority:** P0 · **Confidence:** Confirmed

`ori-hir/src/optimize/pipeline.rs:38` runs DCE at the default optimization
level. Before the first correction slice, `ori-hir/src/optimize/dce.rs:53`
deleted an unused immutable binding when `expr_may_effect` was false, while
division and indexing inherited only operand effects and did not model the
operation's own trap.

Reproducer: an unused `const x = 1 / 0` aborts with `ORI_OPT=none`, but exits
successfully with `ORI_OPT=default`. The optimizer therefore changed observable
language semantics.

**Implementation status (2026-08-25): done for the P0 correctness contract.** DCE uses `Pure`,
`MayTrap`, and `Effectful` in `dce.rs`. Integer division, remainder, and shift
guards, indexing, allocation-backed expressions (including native `bytes`
literals), interpolation, closures, contracts, and custom destructors are
conservatively retained. Associated-call arguments are included in the
binding-use scan, as are statement-`match` guards. Unit tests prove that
allocation-backed values remain in HIR. AOT and JIT differential regressions
cover unused division, remainder, shift, list indexing, failed field contracts,
and custom destructors at `none`, `default`, and `aggressive`. Deterministic OOM
injection remains useful P1 test infrastructure, but no allocation expression
is classified removable.

**Correction**

- Replace the effect boolean with at least `Pure`, `MayTrap`, and `Effectful`.
- Make unknown calls and operations conservative unless safety is proven in the
  same IR pass.
- Keep trapping expressions even when their value is unused.
- Audit every DCE and constant-folding rule against the normative trap model.

**Acceptance**

- Differential tests across `none`, `default`, and `aggressive`, for AOT and JIT.
- Cases for division/remainder by zero, out-of-bounds indexing, shift overflow,
  failed contracts, propagation/unwrap, allocation-retention paths, and custom
  destructors. Deterministic injected OOM belongs to the P1 runtime test harness.
- An optimizer invariant test asserting equal exit status and observable output.

### AUD-OPT-2 — inlining changes argument evaluation

**Priority:** P0 · **Confidence:** Confirmed

`ori-hir/src/optimize/inline_leafs.rs:15` excludes only a narrow set of candidate
bodies. In the audited baseline, substitution at `inline_leafs.rs:246-249`
cloned an argument into every parameter use and omitted it entirely when the
parameter was unused. Calling an
`ignore(x) -> 1` leaf with `ignore(1 / 0)` aborts without optimization and
returns normally with aggressive optimization. A multiply-used parameter can
duplicate effects and evaluation order can change.

**Implementation status (2026-08-25): done for the P0 correctness contract.**
Until HIR can materialize argument temporaries, the inliner accepts only stable,
scalar, literal-derived arguments and parameters used at most once. Variable
reads, calls, allocations, runtime-managed values, parameter contracts,
closures, `match` binding scopes, propagation, and `await` remain behind the
call boundary. AOT and JIT regressions cover ignored traps, mutable argument
snapshots, source-order/single evaluation, contracts, destructors, and managed
arguments. Explicit temporaries and binding IDs remain a P1 optimization and
maintainability follow-up, not an open semantic correctness path.

**Correction**

- Materialize every argument exactly once, in source order, before the inlined
  body; bind the materialized value to a fresh HIR identity.
- Until that representation exists, inline only arguments proven pure and used
  exactly once.
- Use binding IDs rather than textual names during substitution.

**Acceptance**

- Ignored, once-used, and twice-used arguments with traps and ordered side effects.
- Nested calls, managed temporaries, closures, and destructor-bearing arguments.
- AOT/JIT and optimization-level differential tests.

### AUD-ARC-1 — ownership edges lose multiplicity and permit use-after-free

**Priority:** P0 · **Confidence:** Confirmed by runtime/codegen data flow

`ArcEdges` can store vectors (`ori-runtime/src/lib.rs:115`), but in the audited
baseline `ori_arc_register_edge` at `lib.rs:574-592` explicitly ignored a second
`(owner, child)` edge and does not perform the second retain. Native struct
construction registers every managed field separately
(`ori-codegen/src/native_backend.rs:14441`). Replacing a field calls
`ori_arc_update_edge` (`ori-runtime/src/lib.rs:1968`), which removes by the pair,
not by slot.

For `{ a: child, b: child }`, only one ownership edge exists. Replacing `a`
removes the sole edge and may free `child` while `b` still contains its pointer.
Lists and maps have the same class of bug when the same managed pointer occurs in
multiple slots. This is reachable from safe Ori code.

**Correction**

- Give every managed slot an identity, or maintain an exact multiplicity count
  for `(owner, child)` and decrement exactly one occurrence on mutation/removal.
- Make trial deletion, incoming-edge indexes, owner destruction, and collection
  operations operate on the same multiplicity model.
- Consolidate registration validation and mutation under one ARC-state lock.

**Implementation status (2026-08-25): done for the P0 invariant**

`ArcEdges` now keeps one entry for every registered owner slot. Unregister,
owner destruction, reverse-index cleanup, and trial deletion therefore consume
one occurrence at a time. The runtime no longer silently deduplicates parallel
edges. Redundant map/JSON registrations were removed so one physical map field
still maps to one ARC edge. Runtime tests cover explicit unregister, owner
teardown, and trial deletion of a cycle containing two parallel edges. A native
AOT regression constructs two fields from the same temporary child, replaces one
field after the helper returns, reads the other, and finishes with zero leaks.
Registration and retain now validate the allocation and mutate its reference
count under the same ARC-state lock, closing the lookup-then-free race in the old
fast path.

The native backend also stopped registering map key/value edges after the
runtime had already registered the same physical entry. A shared AOT/JIT matrix
now removes one of two list and map aliases, exercises duplicate managed channel
slots, reads the survivors, and reaches the zero-live-allocation baseline.
Exact multiplicity is the selected model, so debug slot IDs are not required for
correctness. The portable Valgrind probe skips explicitly when unavailable and
becomes required with `ORI_REQUIRE_RUNTIME_VALGRIND=1`; scheduled
sanitizer/TSan coverage remains P2 QA under `AUD-QA-3`.

**Acceptance**

- Struct with two fields referencing one object; replace one, then read the other.
- Duplicate list positions; remove one, then read the other.
- Same pointer as map key/value and duplicate collection entries.
- Cycles containing parallel edges; required-mode external memory checker plus
  live-allocation baseline. This host lacked Valgrind, so the portable probe
  skipped explicitly; `ORI_REQUIRE_RUNTIME_VALGRIND=1` forbids that skip.

### AUD-CHANNEL-1 — channels do not own managed queued values

**Priority:** P0 · **Confidence:** Confirmed by lowering/runtime data flow

`OriChannelState` stores `VecDeque<i64>` (`ori-runtime/src/lib.rs:608`). In the
audited baseline, `ori_channel_send` at `lib.rs:1849` enqueued a pointer without an ARC edge or
retain; receive has no explicit ownership transfer, and channel destruction
drops the queue without releasing managed entries. Call lowering treats send as
a normal borrow and can release a temporary after the call
(`ori-codegen/src/native_backend.rs:14197`).

**Implementation status (2026-08-25): done for the P0 invariant**

Typed HIR lowering now selects `ori_channel_send` for scalars and
`ori_channel_send_managed` for runtime-managed element types. The scalar route
never looks up the raw `i64` in the ARC registry. The managed route rejects an
unregistered payload, registers exactly one queue edge, and stores the ownership
tag used by receive/destruction. Runtime tests cover a scalar numerically equal
to a live pointer, invalid managed payloads, unreceived/duplicate entries, and a
four-sender close race. Native managed-channel programs and the shared AOT/JIT
ownership matrix finish with zero leaks. External sanitizer/TSan scheduling is
P2 QA rather than an unresolved channel ownership rule.

**Correction and acceptance**

- Use typed/specialized ABI paths for managed channel values.
- A queue entry owns one reference; receive transfers that reference; close/drop
  releases pending entries; cycles participate in the ARC graph.
- Test temporary sends, sender scope/thread ending before receive, non-empty
  channel destruction, the same object sent twice, AOT/JIT, and leak checks.
  Scheduled sanitizer/TSan execution is a P2 QA gate.

### AUD-NET-1 — async network work does not retain handles and races with close

**Priority:** P0 · **Confidence:** Confirmed by runtime data flow

TCP/UDP/listener resources store unsynchronized `Option<T>` state around
`ori-runtime/src/lib.rs:9033`. Async operations capture raw handles as `usize`
without retaining the managed object (`lib.rs:9375`), while close mutates the
`Option` and releases the resource (`lib.rs:9753`). Scheduling a read/accept and
closing or dropping the handle before await can use freed memory; concurrent
close/read/write can also race on the object state.

Jobs/futures must retain handles through completion/cancellation. Put resource
state behind a mutex or explicit state machine; close cancels/signals pending
operations and deallocation waits for the last user. Test immediate close after
scheduling, cancellation with pending work, and concurrent close/read/write with
ASan/TSan on supported targets.

**Implementation status (2026-08-25): done**

Readiness jobs now retain managed connection, listener, and UDP handles before
queueing work and release them only after the worker finishes. The native
connection/listener/socket state is mutex-protected, so asynchronous I/O and
explicit close cannot mutate the same transport concurrently. A runtime
regression closes a UDP socket while a receive job is pending; the job reports
an error and all allocations are freed. Existing native TCP/UDP async tests
also pass.

On Unix, each readiness probe now returns an owned duplicate descriptor and is
repeated after a 50 ms poll slice. A close therefore cannot make the reactor
poll a descriptor number that has been reused for another resource. Pending
jobs rotate through the queue, and a job whose future was cancelled releases
its future/resource keepalives without executing the I/O callback. Regressions
cover both close during UDP await and cancel+close with a zero-allocation
baseline. A lower-latency multi-OS reactor and sanitizer contention coverage are
P2 performance/QA follow-ups, not open P0 soundness.

### AUD-EMBED-1 — safe `ori-embed` API permits undefined behavior

**Priority:** P0 · **Confidence:** Confirmed by API construction

`ori-embed/src/lib.rs:241-255` exposes a public, `Copy` `OriValue` whose
`Slice`, `String`, and `Bytes` variants accept arbitrary raw pointers. Safe
methods at `ori-embed/src/lib.rs:306-334` call `CStr::from_ptr`, and safe
`OriEngine::call` at `ori-embed/src/lib.rs:1440-1489` forwards the pointers to
the JIT. The JIT validates value kinds, not pointer provenance or lifetime
(`ori-codegen/src/native_backend/jit.rs:452-479,639-679`). Safe Rust can pass
address `1`, a dangling allocation, or non-NUL memory. `bytes` is additionally
read as a C string, so embedded NUL truncates data despite the runtime knowing
the registered allocation size at `ori-runtime/src/lib.rs:2765-2773`.

**Correction**

- Make raw pointer variants private and remove `Copy` from managed values.
- Separate scalars, owned managed handles, and lifetime-bound borrowed views.
- Store `NonNull`, explicit length, session identity, and module generation in
  internal wrappers; owned handles retain/release through `Drop`.
- Return owned `String`/`Vec<u8>` by default. Expose borrowing only while an
  engine/session guard proves liveness.
- Mark the raw escape hatch `unsafe fn` until a sound safe abstraction exists.

**Implementation status (2026-08-25): done.**

`OriValue` no longer exposes raw managed-pointer constructors. Public string
and bytes values are Rust-owned `String`/`Vec<u8>` payloads; inputs are copied
into the target runtime for each call, returned payloads are copied back before
their ARC reference is released, and safe accessors never dereference a host-
supplied raw pointer. Interior-NUL strings are rejected while bytes preserve
their exact length.

The remaining slice token stores a private `NonNull` pointer, runtime ownership
capability, session ID, module ID, and generation. `OriEngine::call` rejects a
token from another session/module, a stale generation, and an unload/reload
identity before retaining or invoking it. Tests cover all four rejection paths,
embedded NUL, reuse after argument passing, unload, and 32 repeated managed
returns with a zero-allocation delta. A `compile_fail` doctest proves the former
raw constructor is absent. `cargo miri` was attempted on 2026-08-25 but the
stable toolchain has no Miri component; this is recorded as unavailable, not as
a pass. The executable ownership/generation regressions and the C-host
ASan/UBSan gate provide the available dynamic evidence. Callback unwind and
process-wide unload are covered by the completed `AUD-EMBED-2`/`AUD-UNLOAD-1`
lifecycle slices; external sanitizer and foreign-host matrix coverage remains
QA follow-up.

### AUD-FFI-1 — foreign strings can escape `@c_export` without ownership

**Priority:** P0 · **Confidence:** Strong

The report's hypothetical escape assumes that a C host can construct an
aggregate containing a `const char *` and pass it back as an Ori struct. That is
not the ABI emitted by the current compiler: scalar bridge structs contain only
numeric/bool fields, while structs containing `string`, nested structs, or
collections are incomplete opaque handles. Direct, `optional`, and `result`
string/bytes payloads are copied before Ori can retain them.

**Implementation status (2026-09-01): partial — boundary hardened.**

Generated wrappers now validate every opaque managed-handle parameter against
the live ARC registry before retaining or entering user code. For concrete
non-generic payloads they also check the registered payload size and compiler
source-type tag. A foreign, null, wrong-size, or same-size wrong-type pointer
therefore takes the deterministic bounds-failure path instead of being
interpreted as an aggregate with a borrowed string field. The C-host regression
exercises valid handles, foreign pointers, a live wrong-size allocation, and a
same-size handle from a different source type under an ASan/UBSan-instrumented
host when those sanitizers are available. A cross-platform hostile foreign-host
matrix remains; the specific unregistered foreign-string escape described by
the report is no longer reachable through the supported ABI.

---

## 4. Crash resistance and input correctness

### AUD-PARSE-1 — producible lexer token reaches `unreachable!()`

**Priority:** P1 · **Confidence:** Confirmed

`-->` produces `TokenKind::Uninhabited` in `ori-lexer/src/token.rs:269`, but
`display_name()` declares that token unreachable at `token.rs:485`. Parser error
rendering calls `display_name()` through `ori-parser/src/parser.rs:151`.
`using x -->` panics with status 101 instead of emitting a diagnostic.

**Implementation status (2026-08-31): done**

`TokenKind::Uninhabited` now renders as `` `-->` ``. The security robustness
corpus includes the malformed `using value -->` source and confirms that the
parser returns bounded diagnostics instead of panicking.

The lexer token enum now has a complete rendering matrix in the lexer unit
tests. Every producible variant, including comments, literals, operators,
delimiters, and `Uninhabited`, must return a non-empty stable diagnostic name.

Token rendering is total for every producible lexer output and protected by a
generated lexer matrix. Broader CLI/LSP hostile-input coverage remains under
`AUD-QA-3`.

### AUD-PARSE-2 — invalid fixed-arity type syntax indexes missing arguments

**Priority:** P1 · **Confidence:** Confirmed

`ori-parser/src/parse_ty.rs:125-126,151-152` indexes `args[0]`/`args[1]` after a helper
has merely emitted an arity diagnostic and returned the short vector at
`parse_ty.rs:473`. `result[]`, `result[int]`, `map[]`, and `map[int]` panic.

Pattern matching and the fixed-arity regression matrix are complete; future
constructors must reuse the same recovery helper.

**Implementation status (2026-08-31): done**

`parse_type_inner` now checks the argument count after emitting the existing
arity diagnostic and returns parser recovery instead of indexing `args[0]` or
`args[1]`. The robustness corpus covers short `result[int]` and
`map[string]` forms without a panic.

The security suite now iterates zero, one, exact, and extra arguments across
single- and multi-argument constructors, using both canonical brackets and
legacy angle recovery forms. Every case asserts bounded diagnostics and no
parser panic.

### AUD-PARSE-3 — recursion guards do not cover unary expressions or patterns

**Priority:** P1 · **Confidence:** Confirmed

The 128-depth guard in `ori-parser/src/parser.rs:8` is not applied to recursive
`parse_unary` (`parse_expr.rs:195`) or `parse_pattern` (`parse_pat.rs:13`).
Inputs with 50,000 unary operators or nested `some(` patterns terminate with a
stack overflow/status 139.

**Implementation status (2026-08-25): done.**

Every recursive expression, statement, type, and pattern constructor enters
the shared nesting budget. A generated 19-constructor corpus runs through both
the CLI and LSP paths; 512-deep unary, pattern, type, and expression cases stop
with bounded diagnostics and never overflow the process stack.

Use an RAII depth budget on all recursive parser entries or parse unary chains
iteratively. Deep valid/invalid inputs must return one
`parse.nesting_too_deep`, never abort, in CLI and LSP.

### AUD-CT-1 — constant dependency chains overflow the Rust stack

**Priority:** P1 · **Confidence:** Confirmed

`ori-types/src/const_eval.rs:141` tracks a stack for cycle detection but
`evaluate_reference` recursively calls `evaluate_definition` at
`const_eval.rs:266`. A 50,001-element acyclic constant chain overflows the
process stack.

**Implementation status (2026-08-25): done.**

Module-constant dependencies now use iterative DFS with explicit visitation
states and cached terminal values. A 4,096-node acyclic chain is accepted, a
512-node cycle receives a stable diagnostic, and a 10,000-constant performance
guard stays bounded without recursive Rust stack growth.

Replace recursive traversal with an iterative DFS/topological evaluator using
white/gray/black states and memoization, or enforce an explicit diagnostic
budget. Long cycles and long acyclic chains must finish in linear time without
process failure.

### AUD-PARSE-4 — invalid `check` messages are accepted silently

**Priority:** P2 · **Confidence:** Confirmed

After the comma, `ori-parser/src/parse_stmt.rs:520` consumes only a string
literal. Another expression remains in the stream as a separate statement, so
`check true, 42` reports no error although Spec 06 requires a string message.

**Implementation status (2026-08-31): done**

The parser now emits `parse.check_message_literal` for every non-string message
and consumes the expression, so its tokens cannot be reinterpreted as a
following statement. String-literal messages use the shared escape decoder
(`\\n`, `\\t`, `\\0`, Unicode escapes, quotes, and slashes) before lowering.
Dynamic forms such as `item.name()` are rejected instead of being silently
discarded, keeping `CheckStmt`'s typed `Option<SmolStr>` contract. The security
suite covers scalar and dynamic messages followed by another declaration, and
the strict C regression exercises hostile escaped text.

The chosen contract is literal-only. Literal, scalar, and dynamic-expression
regressions verify the diagnostic and recovery boundary.

---

## 5. Package integrity and reproducibility

### AUD-PKG-1 — remote archive extraction is not a contained trust boundary

**Priority:** P1 · **Confidence:** Strong

**Original finding:** the driver accepted HTTP registries, downloaded into
predictable names, and invoked external `tar -xzf`. The completed correction
below replaces that path; the old locations are retained only as audit
history.

**Correction**

- Require HTTPS by default; explicit insecure local development must be opt-in.
- Download into an exclusive `tempfile` directory with byte/time limits.
- Parse the archive structurally. Reject absolute paths, `..`, symlinks,
  hardlinks, devices/FIFOs, excessive depth/count, and entries escaping the
  canonical extraction root.
- Require SHA-256 from registry metadata and verify before extraction.
- Publish the validated cache entry with an atomic rename.

**Implementation status (2026-08-25): done.**

Registry downloads require HTTPS by default, disable redirects, use exclusive
temporary directories, enforce time/compressed/expanded/depth/count limits,
and require a SHA-256 sidecar before extraction. The Rust tar reader rejects
the complete hostile-entry corpus before `unpack_in`, and verified cache trees
are published by same-filesystem atomic rename.

**Acceptance**

A malicious archive corpus must cover traversal, absolute paths, symlink pivot,
hardlink, device/FIFO, truncation, duplicate paths, case collisions, and
decompression limits. No exploit archive was executed during this audit.

### AUD-PKG-2 — `ori.lock` detects drift but does not drive reproducible resolution

**Priority:** P1 · **Confidence:** Confirmed by data flow

**Implementation status (2026-08-25): done.** Lock format v2 records normalized
source identity, exact Git commit, and a SHA-256 package-tree digest for every
transitive dependency. `--locked` and offline resolution restore that graph
from verified cache without re-resolving mutable refs; tests cover a moved Git
ref, changed cache byte, source collision, transitive paths, and v1 migration.

**Original finding:** lock v1 had no content digest, re-resolved current state,
and keyed Git cache entries without verified source/revision identity. The v2
implementation described above replaces that behavior.

**Correction**

- Lock normalized source identity, exact Git commit, and content-tree digest.
- Make `--locked` resolution lock-driven; never re-resolve mutable refs.
- Verify cached trees before use and include source identity in cache keys.
- Define immutable registry versions and atomic cache publication.

**Acceptance**

- Empty-cache installs produce identical trees for the same lock.
- A moved branch restores the locked commit; one changed cache byte is rejected.
- Same name/version from different sources cannot collide.
- `--locked --offline` performs no network request.

User documentation was corrected during this audit. The current lock is
reproducible and lock-driven by the v2 digest contract; documentation must keep
those claims tied to the verified `--locked --offline` tests. “Cryptographically
pinned” remains reserved for the exact SHA-256/source identity guarantees in
Spec 17, not for mutable registry metadata alone.

---

## 6. Features present only as stubs, scaffolding, or narrower behavior

| ID | Current evidence | Real status | Required closure evidence |
|---|---|---|---|
| **GFX-SIMD-1** | The former `ori-hir` vectorizer scaffold was a traversal whose transform always returned `None`; it was removed from the product pipeline on 2026-09-01. The existing E2E checks scalar output only. | **not implemented (scaffold removed)** | HIR/CLIF golden proving vector transformation and scalar remainder, negative alias/effect/overflow cases, target feature policy, and benchmark threshold |
| **GFX-WINDOW-1** | `stdlib/window.orl` always uses ID 1, returns `Event.None`, and does no presentation. Runtime `ori_window_present` is a no-op and the stdlib is not wired to its ABI. | **stub; not a native window** | product-scope decision first; then platform backend, event injection/polling, presentation, lifecycle, headless tests, and per-OS smoke—or remove the public promise |
| **ERR-TRACE-1** | Runtime exports `ori_err_trace_push/format`, but there is no compiler/HIR/stdlib caller. | **ABI symbols only** | codegen integration on error-return propagation, nested-call trace regression, ownership tests, and disabled/enabled overhead benchmark |
| **OPT-TYPE-INTERN-1** | `TyInterner`/`TyId` were isolated from checker/HIR and `get` indexed arbitrary public IDs. | **removed (2026-09-01)** | Reintroduce only as a session-owned, validated interner after a measured hot-path migration and allocation/RSS/time benchmark |
| **OPT-RC-ELISION-1** | The name-only alias pass had no explicit retain/release IR and its escape rule made it effectively inert. | **removed (2026-09-01)** | Reintroduce only with ownership-aware HIR, ARC snapshots, and a measured reduction target |
| **OPT-ACYCLIC-1** | `Ty::is_acyclic()` did not inspect named definitions and had no production caller. | **removed (2026-09-01)** | Reintroduce only through DefMap-backed SCC analysis with collector regressions and a measured benefit |
| **OPT-SSO-1** | Runtime implements a one-allocation `cstring_from_slices` path. No tagged inline-string representation exists, and ABI planning previously blocked it. | **construction fast path, not SSO** | keep the accurate name; real SSO requires an ABI/version decision, layout tests, and benchmarks |
| **CLI-DAEMON-1** | `pipeline/daemon.rs` keeps the process alive and every request still builds a fresh check/JIT pipeline. | **done (2026-09-02)** | persistent `DaemonSession` with SHA-256 source content caching, bounded FIFO eviction, warm check hits (`cached: true`), explicit invalidation (`invalidate`), operational stats reporting (`check_hits`, `check_misses`, `invalidations`), and typed JSON-RPC 2.0 protocol |
| **ASYNC-REACTOR-1** | `ori_reactor_poll/wake` wait on the executor `Condvar`; Unix network readiness separately uses a single `libc::poll` worker and non-Unix falls through to blocking work. | **queue wake + partial Unix readiness** | define scope honestly or implement a shared epoll/kqueue/IOCP abstraction with cancellation, fairness, panic recovery, and target tests |
| **ASYNC-STRUCT-1** | `CancelScope` is a token wrapper without a child-task scope/join tree. | **partial (2026-09-01)** | `defer_cancel` now awaits `task.sleep(millis)` before cancelling (E2E regression); child ownership/join/cancel-on-exit and explicit structured-concurrency semantics remain |
| **CONC-THREADS-1** | `ori.concurrent` helpers return scalars/string or shallow-clone a list; they do not establish a type-level cross-thread transfer model. | **partial — enum payloads, named effects, and local closure summaries checked (2026-09-01)** | transferability now walks enum payload fields with cycle protection, rejects non-transferable nested values, follows same-module plus imported named-helper calls for mutable-global effects, and accepts local function values only when checker-side captures are transferable; define full type-level isolation, richer transitive function-value effects, managed graph policy, and thread stress tests |
| **DX-DOCTEST-1** | The original harness discarded recursive/read/write/temp failures, could fall through from check to JIT, and returned an empty `SourceCache`. | **done (2026-09-02)** | unique per-case temporary directories, deterministic recursive extraction, source-backed cache, expected-output assertions (`-- output:` single and multiline), compile-fail assertions (`-- compile_fail`), richer source-labelled diagnostics, and import extraction |

Parallel per-source type checking is an exception: it is genuinely wired through
`pipeline/frontend.rs:136-174` using Rayon for multiple loaded sources. It still
needs scaling measurements and stable diagnostic ordering, but it should not be
reclassified as missing.

---

## 7. Frontend and LSP correctness gaps

### AUD-LSP-1 — positions use bytes where LSP defaults to UTF-16 code units

**Priority:** P1 · **Confidence:** Confirmed by implementation

**Status (2026-08-25): done.** `ori-lsp` now negotiates UTF-8, UTF-16, or
UTF-32 once during `initialize` (UTF-16 when the client omits the capability),
advertises the selected encoding, and routes every inbound/outbound position
through that codec. Invalid lines, columns beyond the line, and columns inside
a UTF-8 scalar or UTF-16 surrogate pair are rejected before an incremental
edit reaches `String::replace_range`. Unit and process-level LSP regressions
cover accents, CJK, combining marks, emoji, CRLF, diagnostics, incremental
edits, hover, definition, references, and rename.

`ori-lsp/src/utils/position.rs:3` counts bytes in both directions. The server
does not negotiate `positionEncoding` (`main.rs:1532`), and `project.rs:67`
passes the offset to `String::replace_range`. Valid positions after emoji or
non-ASCII characters can shift diagnostics/hover/goto or land inside a UTF-8
scalar and panic.

Create one negotiated position codec, defaulting to the protocol's UTF-16
semantics, with boundary validation. Test incremental edits, diagnostics, hover,
rename, references, and goto with accents, CJK, combining text, and emoji.

### AUD-LSP-2 — symbol operations use textual scans instead of resolved identity

**Priority:** P1 · **Confidence:** Confirmed by implementation

**Status (2026-08-25): done.** Every resolved top-level `Def` now retains its
origin `FileId`; cross-file navigation converts that identity directly through
`SourceCache`. The LSP indexes lexer identifier tokens (therefore excluding
comments and string contents), assigns stable local binding identities by
lexical scope, and resolves project occurrences to `DefId`. Explicit selective
import aliases retain a distinct local alias identity while pointing at their
upstream definition. Regressions cover shadowed locals and closure parameters,
duplicate exported names, multiple imports, aliases, comments/strings, equal
offsets in separate files, and cross-file hover/definition/references/rename.

References are word scans in `ori-lsp/src/index/semantic.rs:94` and
`project_semantic.rs:180`, so strings, comments, and shadowed names can match.
Definitions are selected by simple name, and source files are later guessed by
name/offset because definitions do not retain origin (`project_semantic.rs:214,299`).
`resolve_import_target` ignores the requested name and returns the first import
at `main.rs:1194`.

Carry `FileId`, `DefId`, and a local-binding identity from resolver output into
the index. Test duplicate names, aliases, shadowing, comments/strings, equal
offsets in different files, and multiple imports.

### AUD-LSP-3 — stale validation can overwrite current state

**Priority:** P2 · **Confidence:** Strong · **Status: done (2026-08-30)**

Previously heavy checks ran directly in Tokio futures (`ori-lsp/src/main.rs:51,131`).
Debounce checked freshness before compilation but not after it, then committed an
index and published diagnostics without a version — an older slow check could
publish after a newer edit.

**Correction applied:** `validate_uri` and `schedule_debounced_validate` now capture
an immutable `(uri, version)` snapshot, run `run_check*` via `spawn_blocking` off
the Tokio worker, and re-validate both the debounce instant and document version
immediately before committing the semantic index and publishing diagnostics with
the snapshot version. Unit tests prove surrogate-pair rejection, UTF-16 columns,
version tracking, and a deterministic slow-first staleness discard; `cargo test -p ori-lsp` is 31 unit + 13 e2e verde.

### AUD-LSP-4 — completion and rendering still emit removed syntax

**Priority:** P2 · **Confidence:** Confirmed

**Status: done (2026-09-01).** Completion now exposes only the canonical S3
surface (including contextual `async`, `await`, `try`, and `iter`), and no
longer suggests removed `as`, `only`, `implement`, or `do`. The `apply` snippet
uses the compact `apply Type use Trait` form; `using` inserts one statement and
does not close the enclosing block. Semantic hover renders generic and
optional types with S3 brackets (`Type[Arg]`, `optional[T]`). Unit tests protect
the keyword list, snippets, and generic rendering.

### AUD-LSP-5 — editor linting is textual and scope-insensitive

**Priority:** P2 · **Confidence:** Strong

**Status: partial (2026-09-01).** The LSP no longer scans lines or substrings.
It calls the driver's in-memory AST linter through `run_lint_source`, so
comments, strings, Unicode identifiers, and `check` conditions follow the same
parse/check path as `ori lint`; nested-scope and shadowing regressions are
covered by the LSP unit/E2E suite. The former scanner was also quadratic over
the file and is gone.

The linter now records structured bindings from destructuring, `for`, `while
some`, `match` patterns (including nested optional/result/tuple/variant
patterns), `using`, `repeat`, and `loop`. An integration regression exercises
these forms together and confirms that read bindings do not produce false
unused warnings. LSP linting also has an explicit 1 MiB source-buffer budget;
larger editor buffers are skipped instead of blocking the server on a full
reparse/check. Usage and mutation are now resolved through stable linter-local
binding identities, so nested shadowing cannot hide an outer unused warning.
The remaining hardening is to carry resolver binding identities through every
linter scope. Keep this row open until that resolver-backed evidence exists.

### AUD-FRONT-1 — documented unsupported constructs remain real gaps

These paths diagnose or reject rather than implement the feature:

- cross-module, generic, variadic, and method iterators
  (`ori-types/src/check.rs:193,1017`);
- async iterators (`ori-parser/src/parse_item.rs:693`);
- recursive iterators (`ori-hir/src/lower.rs:3023`);
- symbolic const-generic expressions (`ori-types/src/lower.rs:363`);
- generic newtypes (`ori-parser/src/parse_item.rs:1610`).

They may remain intentional pre-1.0 limits, but must stay visible in the single
backlog and in a `surface × parser × checker × HIR × AOT × JIT × C × test`
matrix. “Language complete” is not an accurate status while these are open.

### AUD-FRONT-2 — invalid internal states use magic `DefId` values

**Priority:** P2 · **Confidence:** Strong

JSON now has a named synthetic ID (`DefId::SYNTHETIC_JSON_VALUE`) that is
resolved by one shared, fail-closed normalization helper used by checker and
HIR, while recovery paths use named
`DefId::INVALID`/`SYNTHETIC_MAIN` constants. Literal recovery, applied type
parameters, and generated closures use disjoint synthetic ranges, so the first
closure cannot collide with `INVALID`. Definitions are allocated with a checked
`usize → u32` conversion. `DefMap::try_get` returns `None` for invalid IDs and
`DefMap::get` fails closed instead of returning a process-global dummy; tests
cover a 10,001-definition arena and closure-range separation. The remaining
work is to replace option-less HIR recovery fields with explicit error nodes and
remove the synthetic JSON ID from public type signatures. Diagnostic rendering
now maps any synthetic/unknown named ID to a stable unresolved-type label rather
than panicking through `DefMap::get`.

---

## 8. Backend/runtime correctness gaps

### AUD-RT-INIT-1 — hosted JIT does not run managed global initialization

**Priority:** P1

**Implementation status (2026-09-01): done.**

The native backend emits a managed global initializer around
`ori-codegen/src/native_backend.rs:6420`; the executable entry path calls it.
`CompiledJitModule::compile_with_host_symbols` invokes the finalized
initializer exactly once before publication and pairs it with teardown on
generation drop. Hosted getters for dynamic globals therefore match AOT.

The C/shared-library lifecycle exports the same explicit order in the generated
header (`ori_rt_init` → `__ori_module_init` → exports → shutdown). Runtime leases
serialize init/shutdown and preserve the cdylib until all managed owners leave.

Tests cover dynamic globals, reload, initialization failure preserving the live
generation, and AOT/JIT parity. The C header documents the same order.

### AUD-LINK-1 — linker validation and shared output are inconsistent

**Priority:** P1

**Implementation status (2026-08-31): done.**

`validate_rust_lld_runs` now probes the platform flavor. Bundled, system,
RustcDriver, and raw-linker paths all honor `NativeLinkOptions.shared`; the
Windows branch no longer feeds GNU-only `-Wl` flags to MSVC/link.exe.

The native backend linker matrix covers executable/shared argument shapes and
the shared artifact path. End-to-end toolchain coverage remains in the release
matrix.

### AUD-JIT-ABI-1 — JIT accepts an arbitrary existing runtime cdylib

**Priority:** P1

**Implementation status (2026-08-31): done.**

Static and dynamic runtime metadata are validated before use. JIT validates
target/version/ABI/name plus a staged SHA-256 digest before `dlopen`, then
queries identity symbols from the loaded library.

Tampered artifacts emit `native.abi_mismatch` before symbol lookup. AOT also
checks the static digest when metadata provides it and packaged mode rejects
missing digests.

### AUD-BYTES-1 — embedded NUL is truncated in byte paths

**Priority:** P1

**Implementation status (2026-08-31): done.**
`ori_bytes_to_list`, synchronous TCP writes, and synchronous/asynchronous UDP
sends now use the allocation-aware `bytes_payload` view. A managed payload
`41 00 42` therefore reaches the peer with all three bytes intact. Legacy
pointer-only calls now fail closed with host error `1002` when a foreign pointer
is not registered; the versioned Host ABI uses an explicit `(ptr, len)` pair.

No byte path infers a length by probing unregistered memory. The generated C
export ABI provides the safe `OriBytes { data, len }` view for host buffers.

### AUD-C-1 — C backend interpolates user text into format strings

**Priority:** P1 within the supported C subset

The audit found that `ori-codegen/src/c_backend.rs:3209` interpolated a user
check message directly into an `fprintf` format literal. Quotes/newlines could
make invalid C; `%s`/`%n` could trigger undefined behavior from missing
arguments. The correction gate is to emit a separately escaped string argument
and a constant `"%s"` format, then compile/run messages containing quotes,
slash, newline, `%s`, and `%n` under ASan/UBSan.

**2026-08-25 correction:** `HirStmt::Check` now emits a constant `%s` format,
and `escape_c_str` handles quotes, slashes, control bytes, NUL, and percent
sequences without placing user text in the format string. A codegen regression
locks the generated shape. The portable `c_backend_sanitizers` integration test
also compiles hostile generated C with strict format checks and runs it under
ASan+UBSan when `clang` or `cc` supports both. Unsupported hosts report an
explicit `SKIP`; `ORI_REQUIRE_C_SANITIZERS=1` converts that condition into a
failure for CI. The Linux native-route matrix enables this required mode, so
the supported CI path cannot silently skip the sanitizer gate.

**Implementation status (2026-08-25): done**

### AUD-C-2 — C `any` boxes keep a pointer to a local vtable

**Priority:** P1 within the supported C subset

**Implementation status (2026-08-31): done.**

Vtables are generated with translation-unit lifetime; managed fields register
ARC edges, boxing transfers its initial owner without an extra retain, and
dynamic dispatch has no unmatched retain. Typed C trampolines adapt boxed
pointer receivers to concrete by-value method signatures, avoiding undefined
function-pointer calls under UBSan. Default trait methods receive the
field-less trait representation expected by their emitted signature, avoiding
an incompatible concrete-struct argument. Codegen, C-compile, and ASan+UBSan
regressions cover both concrete and default methods after the constructing
expression ends.

### AUD-SPAWN-1 — thread creation failure can abort or strand futures

**Priority:** P1

The timer path ignores the `Result` from `Builder::spawn`
(`ori-runtime/src/lib.rs:689-694`), so a timer future can remain pending forever.
Task, filesystem, connect, and other I/O paths use panic-based `thread::spawn`
or `expect` around `lib.rs:1138,8446,8476,9663`; failure can unwind across an
`extern "C"` boundary or leave an operation incomplete.

Use one fallible spawn abstraction that completes the future/job with a typed
resource error. Add deterministic failure injection and prove no panic crosses
FFI, every future reaches a terminal state, and retained inputs are released.

**2026-08-25 correction:** `try_spawn_named` now centralizes fallible thread
creation. Timer and reactor initialization record the failure, blocking and
readiness futures become terminal `Failed`, async filesystem operations reuse
the blocking worker, and task closure ownership is released when creation
fails. Runtime tests inject worker failure and verify both future and task
join contracts. Lazy timer/reactor initialization now retries after a failed
spawn instead of caching a process-lifetime error. Spawn failures also record
host error `1004` (`ORI_HOST_ERROR_THREAD_SPAWN`) with the worker name and
cause. Admission is now serialized with the runtime lifecycle mutex, so a
worker cannot start after shutdown transitions to `Stopping`; OS-level
injection remains a CI follow-up only.

### AUD-TARGET-1 — non-host targets lack native code generation

The C backend remains an intentionally partial synchronous debug/transpile path.
Unsupported async/concurrency/destructor features are not defects when they
produce `backend.c_unsupported`; invalid C, crashes, or wrong semantics within
the declared subset are defects. Cross-target AOT is also not implemented merely
because `ORI_TARGET_TRIPLE` is accepted: codegen still constructs a host ISA.
**Implementation status (2026-09-01): done for the current host-only contract.**
`ensure_native_codegen_target` runs before incremental reuse, AOT/JIT lowering,
and native test linking. A requested `ORI_TARGET_TRIPLE` that differs from the
compiler host now returns `native.target_unsupported` instead of emitting host
ISA with a mismatched runtime. Front-end `@cfg` remains usable for target-aware
source selection; cross-target code generation is still future work.

---

## 9. Rust safety boundary and runtime review

The workspace has roughly 944 textual `unsafe` occurrences; approximately 837
are in `ori-runtime`. A textual count is not a count of independent bugs, but it
shows that unsafe code is a major architecture boundary, not “zero unsafe.”
There is no workspace lint policy and no incremental
`unsafe_op_in_unsafe_fn = "deny"` gate.

### Required unsafe policy

1. Keep raw layouts and pointer conversion in narrowly owned `raw`/`ffi` modules.
2. Expose safe wrappers only after validating length, alignment, provenance,
   lifetime, ownership, and thread rules.
3. Every unsafe block must state the local invariant in an English `SAFETY:`
   comment; `unsafe fn` documentation must state caller obligations.
4. Enable `unsafe_op_in_unsafe_fn` one module at a time; do not hide warnings
   with crate-wide allows.
5. Prevent unwind across C ABI and reactor callback boundaries. Translate panic
   to a trap/error and prove the runtime remains usable.
6. Run Miri on safe wrapper crates and sanitizer/nightly stress jobs on the
   runtime where supported.

### Specific runtime findings

| ID | Evidence and risk | Correction |
|---|---|---|
| **AUD-RT-1** | `ori_alloc` at `ori-runtime/src/lib.rs:481-493` computes the header-inclusive size and several collection/FFI/I/O conversion paths allocate backing storage. | **partial** — `ori_alloc` now uses checked header arithmetic, set/map backing arrays plus graph/heap storage use the shared checked helpers at `lib.rs:2929-2955`, C-facing argument/JSON/hex buffers use fallible `try_reserve`, stream/file reads return typed errors for unrepresentable requests, and string repeat/padding plus stdout/stderr byte lengths validate arithmetic and `isize` limits. Overflow and allocation failure now abort or return controlled errors; the public boundary matrix and injected-failure coverage remain |
| **AUD-RT-2** | `cstr_str` at `lib.rs:2701-2706` silently converts invalid UTF-8 to `""`, conflating malformed input with valid empty text. | **partial (2026-09-01)** — `cstr_str_result` and `bounded_cstr_str` validate both NUL-terminated and explicit-length string ingress; invalid data sets host error `ORI_HOST_ERROR_INVALID_UTF8` (`1003`) before legacy APIs return the compatibility empty value, and explicit lengths are checked against `isize`. A full typed `Result` for every pointer-returning ABI entry and pointer-provenance validation remain |
| **AUD-RT-3** | Most ARC retain/release/edge operations acquire one global `ARC_STATE` mutex. Edge registration performs multiple lock cycles before the final mutation. | **partial (2026-09-01)** — all runtime ARC paths now use one poison-safe `lock_arc_state` helper, and a lock-free atomic counter records only contended acquisitions for benchmark runs. Sharding or thread-local ownership remains gated on measured contention and cycle-semantics evidence |
| **AUD-RT-4** | I/O reactor setup/work uses `expect`/`unwrap` (`lib.rs:9498-9525,9635-9647`). A panicking work item can kill the only reactor thread and strand futures. | **done (2026-08-31)** — readiness and blocking workers catch unwind, fail only the affected future, release keepalives, drain queued jobs on shutdown, recover poisoned locks, and reap dead persistent workers before restart. |
| **AUD-SPAWN-1** | Timer, task, filesystem, and I/O worker creation previously ignored `spawn` errors or used `expect`, leaving pending futures or crossing the C ABI on failure. | **done (2026-08-31)** — `try_spawn_named` returns a typed `Result`; admission and shutdown share a lifecycle mutex, timer/reactor startup retries, worker futures transition to `Failed`, task closure ownership is released on failure, and host error `1004` records the cause. |
| **AUD-RT-5** | Timer scheduling sorts a vector on every wake, creating avoidable repeated `O(n log n)` work under many timers. | **done (2026-09-01)** — timer entries use a min-heap with monotonic tie-breaking, so insertion is `O(log n)` and wake-up only pops due entries. Periodic heap compaction removes terminal futures and balances their timer-owned ARC references. Long-lived cancellation tokens shrink burst capacity after removals. Runtime tests cover ordering, stale-owner extraction, and capacity recovery; `tools/bench/run_timer_heap_churn.sh` provides a reproducible 128-concurrent-sleep workload with a completion canary. |
| **AUD-CANCEL-2** | Cancellation associates a future after checking `cancelled` outside the association lock; cancellation can race between the check and insert. Cancel also removes retained futures without balancing every association reference. | **done (2026-08-25)** — association rechecks cancellation while holding the token list lock, clears tentative links before cancelling, and cancellation releases the future retain for every entry removed in bulk. Runtime regressions cover normal cancel and association-after-cancel paths. |
| **AUD-UNLOAD-1** | Loading through `.init_array` installs a process-wide signal handler. | **done (2026-08-31)** — runtime leases keep code loaded through managed values; shutdown stops/joins workers with a deadline, restores handlers/alt stacks, rejects attached foreign threads, and context destruction drains callbacks/handles before unload. |
| **AUD-UNICODE-1** | `ori_string_case_fold` at `lib.rs:2623-2625` delegates to `to_lowercase()`. Unicode case folding is not equivalent to lowercasing (for example, multi-scalar folds), so the public name/status overstates behavior. | **partial (2026-09-01)** — native runtime uses `unicode-casefold` full/default non-Turkic data (including multi-scalar `ß → ss`) with a regression test; C/debug still needs an equivalent table or an explicit reduced-parity diagnostic and generated conformance vectors |
| **AUD-EMBED-2** | Hosted callback registries use poisoning `expect` paths; a callback panic can make later public Rust/C calls panic or abort. | **done (2026-08-31)** — callback and dispatcher types use `C-unwind`; trampolines translate panics to structured traps, registry locks recover poisoning, unregister waits for active calls, and affinity dispatch is synchronous by contract. |

### AUD-ABI-QA-1 — ABI knowledge and validation are duplicated

The runtime and generated code duplicate ABI knowledge across
`ori-types/src/stdlib.rs`, `ori-hir/src/lower.rs`, codegen declarations, runtime
exports, and C header strings. Existing tests prove that metadata exists, not
that every Rust export, compiler signature, layout, ownership flag, and generated
header agree. Replace this with one declarative typed ABI schema that generates
or validates all consumers and add dynamic symbol/layout/ownership smoke tests.
**Implementation status (2026-09-01): partial.** `tools/qa/abi_exports.sh` now
builds `ori-runtime`, validates `runtime-link.json` with
`tools/qa/validate_runtime_link.py`, checks every manifest/native-backend symbol
against the static archive, and checks lifecycle symbols in the shared
artifact. The metadata validator rejects malformed target/profile/version
fields, unsafe artifact paths, missing staged files, and mismatched SHA-256
digests. It is a required `daily_fast` stage. The PowerShell checker now
resolves the actual `compiler/target` output and validates the cdylib when
present. A declarative schema that also proves C layouts, ownership flags, and
generated headers is still open. **The metadata shape is now declarative
(2026-09-01):** `tools/qa/runtime-link.schema.json` is consumed by the
validator before its artifact and digest checks; the remaining gap is the
cross-layer ABI layout/ownership contract.

---

## 10. Performance findings: measure before changing representation

### AUD-CHECKER-1 — checker lookup indexes and clone budget

**Implementation status (2026-08-25): done.** `CheckerIndexes` records values,
types, traits, implementations, methods, and fields once in `ResolvedModule`.
Every file checker borrows that immutable snapshot, so multi-file checking no
longer rebuilds the indexes per file. Synthetic guards cover 1,000 declaration
families, 10,000 constants, and a large import graph; further interning or
parallelism remains gated on measured benefit.

- `ori-types/src/check.rs` is 8,662 lines. Only functions have an effective
  index; values, structs, enums, traits, and implementations are repeatedly
  scanned and signatures cloned (`check.rs:265,385`). Build immutable indexes
  once by `DefId`, `(type, trait)`, `(type, method)`, and field before attempting
  more parallelism.
- Recursive `Ty` clones remain common. `TyId` is not currently connected. Select
  one hot path, record allocations/RSS/time, migrate it behind stable APIs, then
  decide whether an end-to-end interner pays for its complexity.
- Parallel source checking is real, but small projects may pay Rayon overhead.
  Benchmark thresholds by source count/size and preserve deterministic diagnostic
  order.

### AUD-TEST-1 — `ori test` recompiles the program for every test

**Implementation status (2026-09-01): done for native test execution.**
`pipeline/native.rs` now clones/lower/codegens/links one suite HIR, injects an
`ori_test_selected(index)` dispatcher, and runs each selected case in its own
fresh process via `ORI_TEST_INDEX`. Multi-test, filtering, and failure tests
prove result isolation; a future benchmark should record compile/link counts
for large suites.

### Related compiler performance work

- The daemon currently provides process-startup savings only. A meaningful
  service needs syntax/resolution/type/codegen caches with dependency-aware
  invalidation and cold/warm benchmarks.

### Runtime hot paths

- The global ARC registry lock is the primary scalability hypothesis. Instrument
  acquisitions, contention, live allocations, and edge degree before changing
  ownership architecture.
- The executor creates an OS thread per spawned task and for several blocking
  operations; one readiness reactor serializes work, and Windows readiness is a
  no-op before blocking work. Test 10,000 tasks/I/Os with thread and memory caps,
  fairness, and injected spawn failure before selecting a bounded pool design.
- C-string representation causes repeated scans and makes `bytes` semantics
  fragile. A length-carrying internal representation can improve correctness
  and performance, but public ABI changes require versioning/migration.
- Use a heap for timers only after a workload with many scheduled sleeps proves
  the current sort cost.
- Actual SIMD work begins only after a scalar benchmark, transformation proof,
  alias/effect analysis, target feature detection, and a scalar remainder path.

### Benchmark acceptance template

Every performance PR must record toolchain, target, CPU, command, corpus, warmup,
sample count, median and tail, allocations/RSS when relevant, binary size, and
correctness gates. Reject optimizations that only move cost, regress small
programs materially, or cannot be disabled/rolled back.

---

## 11. Redundant, dead, stale, and oversized code

### Exact or near duplication

- Compatibility modules `stdlib/concurrent/utils.orl` and
  `stdlib/process/utils.orl` now forward to their canonical parent modules;
  they no longer duplicate implementation bodies. Other compatibility modules
  still need the same audit.
- Stdlib signature/ABI data is repeated in `ori-types/src/stdlib.rs` and
  `ori-hir/src/lower.rs`, then repeated again in codegen/runtime declarations.
- Compatibility modules should have one canonical implementation with explicit
  forwarding/re-export or generated aliases, not copied bodies.

### AUD-HYGIENE-1 — dead or misleading scaffolding

**Implementation status (2026-09-01): partial.** Exhaustiveness duplicate and
missing-case keys, plus `ori explain` guidance, now use the canonical `ok`/`err`
constructors. The AST/HIR crate-level docs no longer claim implementation is
pending, and the LSP's unused workspace-root storage/accessors were removed.
Legacy angle-generic lookahead/recovery is intentionally retained so removed
syntax produces a bounded `parse.removed_angle_type` diagnostic; it is not a
live language feature. The remaining stubs and compatibility modules need an
explicit remove-or-implement decision before this row can close.

- Error-trace exports and window runtime code remain unused, unreachable, no-op,
  or materially narrower than their names/docs. The inert vectorizer scaffold
  was removed from the optimization pipeline on 2026-09-01. The
  unconsumed type interner, acyclic helper, and name-only RC pass were removed
  on 2026-09-01.
- `OriWindow` and some LSP helpers use `#[allow(dead_code)]`; removed syntax is
  still modeled in completion/helpers.
- Exhaustiveness diagnostics and `ori explain` now use `ok(...)`/`err(...)`.
- `ori-ast/src/lib.rs` and `ori-hir/src/lib.rs` now describe the implemented
  AST/HIR modules.
- `ori run` sets custom arguments in the statically linked runtime copy while
  JIT code uses a separate cdylib; this call is redundant/misleading.
- Native runtime discovery caches its first success or failure in an unkeyed
  `OnceLock`, so target/path/environment changes in one process cannot recover.

### Files needing domain extraction after correctness gates

| File | Approximate lines | Extraction boundary |
|---|---:|---|
| `ori-codegen/src/native_backend.rs` | 19,169 | ABI declarations, layouts, expression/statement emission, ARC, async, debug, linking |
| `ori-runtime/src/lib.rs` | 10,253 | allocation/ARC, collections, strings/bytes, executor, I/O/network, crypto, raw exports |
| `ori-types/src/check.rs` | 8,662 | declarations, expressions, calls/methods, traits, patterns, async, diagnostics |
| `ori-hir/src/lower.rs` | 6,019 | phase/context-oriented lowering and stdlib ABI boundary |
| `ori-codegen/src/c_backend.rs` | 5,915 | C runtime template, type/layout, emit phases |
| `ori-driver/src/package.rs` | 2,581 | source resolution, lock model, cache, archive, transport, publish |

Extraction must preserve behavior and land in small domain slices. Do not start
with a mechanical “split the file” change: first add tests around the invariant
being moved, then move one owner at a time.

Repository hygiene also needs review: six tracked Linux debug ELFs under
`tools/bench/graphics/bin/` total about 66.8 MB although the runner regenerates
them; `_archive` contains large patch/scratch artifacts. Confirm provenance,
then remove generated binaries from source history going forward and publish
them as CI artifacts. Do not delete historical material without the owner's
explicit decision.

---

## 12. QA, CI, release, and diagnostic gates

### AUD-QA-1 — required quality status is false

**Priority:** P1

**Implementation status (2026-08-31): done.** Required daily/native-route
stages now fail closed and use `--locked`: workspace check, full strict Clippy,
tests, and a documented scoped rustfmt ratchet. Existing unrelated formatting
debt is not rewritten in one unsafe bulk change. Full observational stages say
`INCOMPLETE` when their optional host tooling is unavailable and never print a
false success; masking `|| true` paths were removed. The Linux daily gate passes
with these rules.

**Original finding:** the workspace count was stale, required unit stages were
suppressed with `|| true`, observational stages printed `OK` after suppressed
failures, and full strict Clippy was not required by CI. The scoped rustfmt
ratchet deliberately avoids rewriting unrelated historical drift while making
newly owned Rust files fail closed.

The required baseline on the pinned project toolchain is:

```sh
python3 tools/qa/rustfmt_scoped.py --check
cargo test -p ori-driver --test c_backend_sanitizers --locked -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo audit
```

`cargo fmt --all -- --check` is an explicit cleanup target while the scoped
ratchet is the enforceable gate; it currently reports historical drift outside
the files owned by this correction slice.

Mandatory stages must fail. Observational/performance stages may warn, but must
report `warning/incomplete`, never `OK`. Add nightly/weekly fuzz, Miri, sanitizer,
ignored stress tests, and coverage-trend jobs without making unsupported targets
block unrelated platforms.

### AUD-QA-2 — presence tests are not behavior tests

- `examples_smoke.sh` now walks root and nested examples and checks all 25
  current entrypoints. `ORI_EXAMPLES_COMPILE=1` adds a native build tier with
  isolated temporary outputs; the default remains check-only for fast local
  feedback.
- `docs_examples.sh` may skip compiler-backed doc checks and still succeed.
- `docs_coverage.sh` parses a narrow one-line YAML shape and verifies paths, not
  schema, behavior, or status truth.
- `diagnostic_catalog.rs` finds code strings in source, so dead strings count as
  implementation. It does not validate severity, primary span, message, or
  action required by Spec 13.

**Implementation status (2026-09-01): partial.** `tools/qa/validate_atlas.py`
validates the repository's intentionally small Atlas YAML subset: schema
version, required feature fields, allowed status values, duplicate IDs, safe
relative paths, and path existence. `diagnostic_catalog` also validates every
emitted row's severity/description and exercises a real `parse.module_missing`
diagnostic for severity, primary message, and bounded span. `daily_full.sh` now
invokes the isolated L2 native build tier for all examples and runs the
curated `hello`, `language_features`, and `native_showcase` binaries at L3
(overridable with `ORI_EXAMPLES_RUN`). The remaining work is broader behavior:
expand the safe run set and represent diagnostics as typed catalog entries with
snapshots for action text and richer message/span cases.

### AUD-QA-3 — security and robustness depth is too small

**Implementation status (2026-09-01): partial.** The dependency-free
`tools/qa/fuzz_smoke.py` gate now exercises deterministic malformed bytes,
truncated blocks, invalid `check` expressions, and 512-level nesting through
the real `ori check` process. Each case has a timeout and panic/stack-overflow
output detection; `daily_full.sh` treats a missing compiler binary as an
explicit observational `INCOMPLETE` result. This is a smoke gate, not
coverage-guided fuzzing. Parser/package/ABI/ARC/embedding/LSP generators,
Miri, sanitizer scheduling, and coverage-trend reporting remain open.

No coverage-guided property/fuzz framework, Miri, sanitizer, or coverage job was
found. `security_robustness.rs` has five tests and three heavy ARC/performance
tests are ignored; the new smoke gate does not replace those deeper jobs.

Initial targets:

- lexer/parser arbitrary bytes and nesting budgets;
- parse/format idempotence and migration;
- manifest, lock, registry response, JSON-RPC, and archive parsers;
- bindgen/C header inputs and ABI schema;
- ARC edge mutations, cycles, destructors, panic/unwind, and concurrency;
- LSP position conversion and incremental edits.

Keep Cranelift verification enabled in CI/dev/fuzz even if release builds disable
it for measured performance.

### AUD-REL-1 — release workflow can publish incomplete artifacts

**Implementation status (2026-08-31): done.** Linux GNU, Windows MSVC, macOS
ARM64, and macOS x86_64 are required builders and each package script exercises
JIT plus AOT. Publication depends on the complete matrix, validates every
artifact, pins actions/toolchain, limits write/OIDC permissions to the publish
job, and emits SHA-256 checksums, a commit-timestamped deterministic SPDX SBOM,
build-provenance attestations, and a deterministic tar.gz archive. Native
emission now sorts string/global data and function-reference/wrapper snapshots
by semantic key, avoiding per-process `HashMap` order in generated machine
code; archive traversal prunes `.ori` caches before reading them. Actual
publication remains a tag-triggered release operation and was intentionally
not simulated locally.

**Original finding:** Windows and macOS ARM builders could fail, smoke forced a
JIT-only route, publishing used `always()` with an incomplete artifact set, and
no checksum, SBOM, or provenance was produced. The required matrix and release
artifacts above replace that workflow.

Define the actually supported matrix. Require package + JIT + AOT compile/test
for every published target, make publishing depend on all required builders,
pin actions by commit SHA, and produce checksums/SBOM/provenance. Optional
platforms must be labeled preview and excluded from the required release set.

---

## 13. Dependencies and Rust practices

- Add workspace lint policy and inherit it in every crate. Start with warnings
  already clean, then introduce `unsafe_op_in_unsafe_fn` per module.
- Add an explicit `rust-version` matching the pinned toolchain if Rust 1.95 is
  the supported compiler contract.
- `tokio` currently enables `full`; select only used features after
  `cargo tree -e features`.
- Align direct `lsp-types` with the version used by `tower-lsp` when compatible.
- Review three `hashbrown` versions and duplicate `webpki-roots` chains; do not
  force a unification that violates upstream constraints.
- `compiler/Cargo.toml` repository metadata points to `ori-lang/ori`, while
  release/user docs point to `raillen/ori-lang`; choose the canonical URL.
- Keep `Cargo.lock` tracked for the compiler application. Fix `.gitignore`
  ordering where `*.lock` currently overrides the earlier Cargo exception.
- Add `cargo audit`/license policy to CI and monitor the allowed transitive
  RustSec warning until the Cranelift chain can be updated.

Primary references for the policy are the Rust lint documentation for
[`unsafe_op_in_unsafe_fn`](https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#unsafe-op-in-unsafe-fn),
the safety contract of
[`CStr::from_ptr`](https://doc.rust-lang.org/std/ffi/c_str/struct.CStr.html#method.from_ptr),
the Cargo guidance on
[`Cargo.toml` and `Cargo.lock`](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html),
and the LSP 3.17
[`Position` contract](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#position).

---

## 14. Correction program

Each row should normally be one independently reviewable PR. Split rows further
when production changes exceed roughly 100-600 lines or cross more than one
ownership boundary.

### Wave 0 — containment and truthful state

| Order | Slice | Exit gate |
|---:|---|---|
| 0.1 | Fix DCE trap classification | differential AOT/JIT optimizer matrix |
| 0.2 | Restrict/fix inliner argument evaluation | single-evaluation and order regressions |
| 0.3 | Fix ARC edge multiplicity and managed channel ownership | parallel-edge/channel ASan + leak suite |
| 0.4 | Retain/synchronize async network handles | close/cancel/race sanitizer suite |
| 0.5 | Make `ori-embed` public values and foreign-string ingress sound | compile-fail + Miri + stale-session/foreign-buffer tests |
| 0.6 | Reclassify false `done` claims and public promises | backlog, changelog, EN/PT docs, and Atlas agree |
| 0.7 | Restore format/full Clippy gates and remove false-success QA paths | pinned baseline commands green |

### Wave 1 — hostile input and supply chain

| Order | Slice | Exit gate |
|---:|---|---|
| 1.1 | Total lexer token diagnostics + fixed-arity type recovery | no-panic corpus |
| 1.2 | Parser recursion budget + iterative const evaluation | 50k-depth/chain bounded diagnostics |
| 1.3 | Structured archive extraction with limits/digests | malicious archive corpus |
| 1.4 | Lock-driven source resolution and cache identity | empty-cache/offline/moved-ref reproducibility |
| 1.5 | Typed daemon JSON-RPC parser with limits | invalid/malicious request matrix |
| 1.6 | C format/vtable lifetime fixes in supported subset | C compile/run under ASan/UBSan |

### Wave 2 — semantic tooling and runtime reliability

| Order | Slice | Exit gate |
|---:|---|---|
| 2.1 | UTF-16/negotiated LSP position codec | Unicode edit/diagnostic matrix |
| 2.2 | Resolver-identity LSP index | shadow/alias/multi-file rename/goto matrix |
| 2.3 | Generation-safe background validation | stale result cannot publish |
| 2.4 | JIT runtime ABI validation + hosted module init | old cdylib rejected; global parity |
| 2.5 | Reactor panic/poison recovery and runtime unload lifecycle | later jobs finish; shutdown+dlclose host survives |
| 2.6 | Doctest error propagation and temp ownership | failure-path regression suite |
| 2.7 | Correct delayed cancellation or remove it | timing and cleanup tests |
| 2.8 | Implement or accurately rename Unicode case folding | Unicode conformance cases and EN/PT docs |
| 2.9 | Make all thread creation fallible and terminal | injected failure never panics or strands a future |

### Wave 3 — implementation truth and measured performance

| Order | Slice | Exit gate |
|---:|---|---|
| 3.1 | Checker indexes before deeper parallelism | 10× definition corpus and clone/lookup counters |
| 3.2 | Decide `TyInterner`: integrate one hot path or delete | **done 2026-09-01:** unused public scaffold removed; only reintroduce with measured migration |
| 3.3 | Rename/remove RC pass or introduce ownership IR | **done 2026-09-01:** inert name-only pass removed; ownership-aware IR remains a future optimization |
| 3.4 | Remove/rebuild acyclic analysis through DefMap SCCs | **done 2026-09-01:** unsound unused helper removed; SCC analysis remains future work |
| 3.5 | Integrate error traces or remove active claim | nested propagation trace + overhead budget |
| 3.6 | Real daemon caching | warm check/eval is measurably faster with correct invalidation |
| 3.7 | One-build native test dispatcher | **done 2026-09-01:** one suite emits/links once; each test runs in an isolated process |
| 3.8 | Actual SIMD | vector IR/instruction proof + scalar parity + threshold |

### Wave 4 — architecture and optional product features

| Order | Slice | Exit gate |
|---:|---|---|
| 4.1 | Declarative stdlib/runtime ABI schema | generated consumers + symbol/layout/ownership validation |
| 4.2 | Extract runtime/codegen/checker modules by domain | behavior gates remain green after each slice |
| 4.3 | ARC contention and bounded-executor experiments | reproducible improvement without semantic regression |
| 4.4 | OS reactor architecture, only if product scope requires it | epoll/kqueue/IOCP target tests |
| 4.5 | Native windowing, only after a product ADR | platform implementation and real presentation/event smoke |

`ori.window` should not displace language correctness, documentation, or local DX.
The current product policy excludes bringing game/editor packages back into core;
removing or shelving the stub is lower-risk than expanding it without a product
decision.

### Wave 5 — High-performance native systems & engine foundation (2026-09-03)

Derived from the deep architectural confrontation with `/home/raillen/Documentos/ori_game_engine_2d_3d_roadmap.md` and the 35 packages in `game-engine-full`.
These features remain strictly general-purpose low-level systems capabilities (Layer A/B/C); no game-specific syntax, engine types, or monorepo packages are introduced.

| Order | Slice | Exit gate |
|---:|---|---|
| 5.1 | Declarative native dependencies (`PKG-NATIVE-1`) | `[native.dependencies]` with `pkg-config`, per-platform library lists, and link paths in `ori.pkg.toml` without manual `.a` linker script workarounds |
| 5.2 | Static `@noalloc` verification (`LANG-NOALLOC-1`) | Static checker/HIR rejection of heap allocations, collection growth, formatting, or boxing in marked functions with `perf.allocation_in_noalloc` |
| 5.3 | Explicit struct/field alignment (`LANG-ALIGN-1`) | `@align(N)` attribute lowered to Cranelift layout and C headers (`alignas`) for std140/std430 GPU uniform buffers and SIMD |
| 5.4 | Scoped memory arenas/regions (`MEM-REGION-1`) | `using region = mem.region()` with compile-time escape analysis and single O(1) bulk deallocation at frame exit |
| 5.5 | Portable SIMD vectors (`LANG-SIMD-1`) | `simd[float32, 4]` lowered to Cranelift vector IR with vector arithmetic and lane operations |

---

## 15. Definition of done for every reopened item

An item is not `done` because a file, type, export, pass, or smoke test exists.
Closure requires:

- production reachability demonstrated;
- intended semantics and failure semantics documented;
- regression at the correct Ori test levels: check → compile → run where applicable;
- diagnostic catalog entry with code, severity, primary span, message, and action;
- native AOT/JIT parity, and explicit C-backend support or rejection;
- unit/property/fuzz coverage proportional to parser/FFI/security risk;
- performance effect measured when performance is the claim;
- no new broad `allow`, undocumented unsafe operation, panic-based input handling,
  or magic sentinel;
- `cargo fmt`, full workspace Clippy, workspace tests, diagnostic catalog, and
  relevant QA stages green on the pinned toolchain;
- spec, EN/PT user docs, Atlas, backlog, and `[Unreleased]` changelog updated in
  the same slice;
- rollback/compatibility impact stated for ABI, package, lock, cache, and surface
  changes.

This audit should be re-run after Waves 0 and 1. Findings move to historical
records only when their acceptance evidence is linked from the single backlog.
