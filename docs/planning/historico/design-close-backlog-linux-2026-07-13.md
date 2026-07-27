# Design: Close remaining backlog (Linux-only product path)

> **PLAN SCOPE (user decision 2026-07-13)**  
> Implement remaining BACKLOG items **except**: M4 self-host, ECO-1, ECO-2, TOOL-1, **Windows package (DIST-1)**, **macOS packages (DIST-2)**.  
> Distribution remains **Linux-first** (`x86_64-unknown-linux-gnu` only) until further notice.

Surface baseline: S3 `0.3.0` + inference B `0.3.1` + package `0.3.2`.

---

## Goals

1. Make the **Linux** install/release story honest and green (smoke-no-rust CI, docs).
2. **Freeze** package manifests (PKG-4) after git/registry landed.
3. Raise **C/debug sync parity** for the known failing / high-value paths (LANG-2).
4. Deliver an honest **STDLIB-4** slice: usable async-friendly I/O without claiming full non-blocking runtime.
5. Optional maintainability **STDLIB-5** only if low-risk; otherwise document Layer-1 permanence.
6. Document **FREEZE-1** / **ABI-1** process criteria (not a calendar freeze of the language today).
7. Update **BACKLOG.md** so deferred multi-OS items are explicit, not silent debt.

## Non-goals

- Windows/macOS release packages and CI matrices for those OSes.
- Self-hosting compiler.
- External demos (raylib/sqlite).
- VS Code Marketplace publish.
- Full industrial non-blocking kernel I/O (io_uring / IOCP) in this plan — STDLIB-4 MVP uses the executor + `run_blocking` bridge with a clear upgrade path.

## Key Decisions

| ID | Decision | Rationale |
|----|----------|-----------|
| **D1** | **Linux-only distribution** until user re-opens DIST-1/2 | User explicit; Windows job already fails; avoid false multi-OS promises |
| **D2** | release.yml: **drop or skip Windows matrix** for now; keep Linux package + publish | Matches D1; CI green is product integrity |
| **D3** | STDLIB-4 MVP = **async wrappers** over existing sync L1 via `task.run_blocking` (+ docs), not new kernel async | Honest; unblocks async programs without XL runtime rewrite |
| **D4** | LANG-2 = fix **known red** `build_c_backend_*` and document remaining partials in spec 14 | Reference backend is native; C is debug |
| **D5** | STDLIB-5 = **skip mass port**; optionally port 1–2 pure helpers if already trivial; mark rest optional wontfix-for-now | L1 Rust is permanent design |
| **D6** | FREEZE-1/ABI-1 = **process docs + BACKLOG criteria**, not hard freeze today | Pre-1.0 still; document gates |
| **D7** | PKG-4 = normative **manifest schema doc** + negative/positive edge tests | Path/git/registry shapes now known |

## Architecture / workstreams

### A. Distribution (Linux)

- `.github/workflows/release.yml`: Linux-only matrix; Windows job removed or `if: false` with comment pointing to BACKLOG DIST-1 deferred.
- Add/ensure CI job for `smoke_no_rust` on packaged Linux artifact (DIST-3 Linux).
- `docs/install.md`: Linux primary; Windows/macOS as “supported for build from source / future packages”.

### B. PKG-4 Manifest freeze

- New or expand `docs/spec/` or `docs/planning/manifest-schema.md` (prefer under `docs/spec/` if user-facing, else planning):
  - `ori.proj` fields, sections, dependency table shapes (path, version, git).
  - `ori.pkg.toml` fields, native_libs, dependencies.
- Edge-case tests in `ori_spec.rs` (invalid name, missing entry, git+path reject, version format).

### C. LANG-2 C backend

- Fix failures: `build_c_backend_compiles_any_trait_dynamic_dispatch`, `build_c_backend_displayable_string_conversion` (and any easy adjacent fails).
- Update `docs/spec/14-backend-support.md` rows if parity improved.

### D. STDLIB-4 MVP

- For hot I/O (`ori.fs.read_text`, net connect helpers, or thin stdlib helpers):
  - `*_async` or `*_in_background` style that returns `task.Job` / future via `task.run_blocking`.
- Spec note: true non-blocking = future STDLIB-4b.
- Tests: compile_runs at least one async main using the helper.

### E. Process freeze docs

- Short `docs/planning/freeze-and-abi-gates.md`: criteria for FREEZE-1 and ABI-1.
- BACKLOG status updates.

## PR Plan

### PR 1: Linux-only product policy + BACKLOG + freeze gates docs

- **Description:** Codify user decision: distribution is Linux-only for packages/CI. Update BACKLOG (DIST-1/2 deferred/wontfix-for-now; DIST-4 deferred; TOOL-1/ECO/M4 excluded). Add FREEZE-1/ABI-1 process doc. Align release.yml to Linux-only packaging.
- **Files/components affected:** `docs/planning/BACKLOG.md`, `docs/planning/freeze-and-abi-gates.md` (new), `.github/workflows/release.yml`, optionally `docs/install.md` intro
- **Dependencies:** None

### PR 2: PKG-4 manifest schema freeze

- **Description:** Document frozen `ori.proj` / `ori.pkg.toml` schemas (including path/git/version deps and registry pins). Add regression tests for parse edge cases and dependency table validation.
- **Files/components affected:** `docs/planning/manifest-schema.md` (new) or `docs/spec/…`, `compiler/crates/ori-driver/src/package.rs`, `compiler/crates/ori-driver/src/pipeline.rs`, `compiler/crates/ori-driver/tests/ori_spec.rs`, `CHANGELOG.md`
- **Dependencies:** None (can parallel PR 1)

### PR 3: DIST-3 Linux smoke-no-rust CI + DOC-1

- **Description:** Ensure CI runs package + `tools/smoke_no_rust.sh` for Linux without Rust on PATH (or document isolated PATH). Update `docs/install.md` for Linux package-first story; note multi-OS deferred.
- **Files/components affected:** `.github/workflows/` (release and/or new smoke workflow), `tools/smoke_no_rust.sh`, `docs/install.md`, `README.md` install section, `CHANGELOG.md`
- **Dependencies:** PR 1 (release.yml policy)

### PR 4: LANG-2 C/debug sync parity slice

- **Description:** Fix failing C-backend tests for trait dynamic dispatch and Displayable string conversion; add or extend tests so green. Update backend matrix notes.
- **Files/components affected:** `compiler/crates/ori-codegen/src/c_backend.rs`, `compiler/crates/ori-driver/tests/multifile_imports.rs`, `docs/spec/14-backend-support.md`, `CHANGELOG.md`
- **Dependencies:** None

### PR 5: STDLIB-4 async I/O MVP (run_blocking bridge)

- **Description:** Add stdlib async-friendly helpers for file and/or net I/O using `task.run_blocking` (or equivalent Job API). Document permanent vs interim in spec 12/14. Tests: async program that awaits file or net helper.
- **Files/components affected:** `stdlib/fs.orl` and/or `stdlib/io.orl` and/or `stdlib/net.orl`, `stdlib/*.oridoc` if present, `compiler/crates/ori-driver/tests/concurrency_async.rs` or multifile, `docs/spec/12-stdlib.md`, `docs/spec/14-backend-support.md`, `docs/planning/BACKLOG.md`, `CHANGELOG.md`
- **Dependencies:** None (LANG-1 already done)

### PR 6: STDLIB-5 policy + BACKLOG closeout

- **Description:** Do **not** mass-port L1. Optionally add zero or one pure-`.orl` helper if already half-done. Mark STDLIB-5 as `wontfix for now` / optional with policy text in stdlib README + BACKLOG. Final BACKLOG snapshot for this plan: all in-scope items done or explicitly deferred.
- **Files/components affected:** `stdlib/README.md`, `docs/planning/BACKLOG.md`, `docs/planning/stdlib-merge-policy.md`, `CHANGELOG.md`
- **Dependencies:** PR 5 (if STDLIB-4 notes interact); else PR 1

## Open Questions

None — user decisions are final for this plan.

## Success criteria

- `cargo test -p ori-driver --test ori_spec package_` and `cargo test -p ori-driver --test multifile_imports build_c_backend` improve (no new red for LANG-2 targets).
- Linux release workflow packages without Windows matrix requirement.
- BACKLOG reflects: in-scope done; DIST-1/2/TOOL-1/ECO/M4 excluded or deferred explicitly.
- STDLIB-4 MVP has at least one green native async test using I/O helper.
- Manifest schema doc exists and edge tests pass.

## Out of scope forever for this plan

M4, ECO-1, ECO-2, TOOL-1, Windows package, macOS package, full non-blocking OS I/O.
