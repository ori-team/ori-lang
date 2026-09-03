# Freeze and ABI gates (FREEZE-1 / ABI-1)

> **Status (2026-08-09):** **FREEZE-1 closed**; the workspace remains
> **`0.3.8-dev`** and no `0.4.0` cut has occurred. **ABI-1 remains in force** —
> `ori-native-abi-1` still holds and its rules below still apply.
> This file also carries the **1.0 readiness checklist** (merged 2026-07-17 from
> the former `freeze-1-0-readiness.md`; the FREEZE-1/ABI-1 rules were duplicated).

---

## FREEZE-1 — **CLOSED 2026-07-19** (window ran 2026-07-13 → 2026-07-19)

**Opened:** 2026-07-13  
**Closed:** 2026-07-19 · current workspace: **`0.3.8-dev`**. A future `0.4`
line requires an explicit breaking or major milestone decision; closing the
freeze did not perform that version cut.

**Why now:** the freeze existed to stabilize the S3 surface, and it did its
job — the `0.3.x` series shipped the ARC campaign, the incremental cycle
collector, `ori update`, the CI native-route fix and the silent match-guard
bug fix, with **no** intentional surface break. Closing the timed gate permits
reviewed additive work, but does not by itself authorize a breaking surface or
declare a new released version line.

**What the `0.3.x` rules below still bought us (kept as the model for the
next freeze):** every entry in "Not allowed" stayed unviolated for the whole
window.

### Post-FREEZE-1 rules (current `0.3.8-dev`)

- Small compatible surface **additions** may use `0.3.x`; each lands with spec,
  guide/reference updates, tests, and CHANGELOG, never syntax-only.
- Surface **removals/renames** still need an explicit decision recorded in
  the roadmap first and normally require a `0.4+` cut (the S3 "one canonical
  form" norm did not relax).
- **ABI-1 is untouched:** breaking `ori-native-abi-1` still requires a
  version bump per the ABI section below.
- Documents must not present `0.4` as current until the workspace, changelog,
  package metadata, and release process actually move together.

### Scope of the freeze that just closed (kept for the record)

### Scope of freeze (no intentional breaking)

- Language surface S3 (`0.3.x` syntax/semantics)
- Canonical stdlib paths `ori.X` (nested utils/algorithms remain silent compat)
- Package manifests: `ori.proj` / `ori.pkg.toml` (see `manifest-schema.md`)
- Native ABI tag **`ori-native-abi-1`** (spec 19) — treat as stable; bump only for breaks

### Allowed during freeze

- Patch releases (`0.3.x`): bug fixes, docs, diagnostics, tests
- Additive APIs (new stdlib functions, new optional CLI flags)
- Performance and reliability of existing paths

### Not allowed without bumping to `0.4+` and explicit exit

- Breaking language syntax/semantics
- Removing or renaming public `ori.X` APIs
- Changing `ori.proj` / `ori.pkg.toml` required fields incompatibly
- Breaking `ori-native-abi-1` without ABI version bump

### How to exit freeze — **executed 2026-07-19**

1. ~~Decide end date~~ → closed 2026-07-19; no automatic version cut.
2. ~~Document in CHANGELOG~~ → recorded under `[Unreleased]`.
3. ~~Update BACKLOG~~ → gate row updated.

Reuse this same 3-step process for any future freeze window.

---

## ABI-1 — **DONE** (enforcement in force)

After FREEZE-1 closure, **ABI-1 remains enforced**:

| Gate | Rule |
|------|------|
| Spec | `docs/spec/19-abi.md` is normative for native layouts / mangling / runtime symbols |
| Version | Tag **`ori-native-abi-1`** (`ORI_ABI_VERSION` in `ori-runtime`) |
| Link | `runtime-link.json` `abi_version` must match staged runtime |
| Break | Incompatible layout/symbol change → bump to `ori-native-abi-N`, update cap. 19, re-stage staticlib **and** cdylib |
| Additive | New `ori_*` symbols OK without bump; list in stdlib manifest |

### Enforcement checklist (maintainers)

- [x] Spec 19 published (`ori-native-abi-1`)
- [x] Freeze window open (FREEZE-1)
- [x] CHANGELOG documents ABI-1 in force
- [x] No silent layout changes on `0.3.x` without bump

## Explicitly out of freeze scope (not language work)

- Multi-OS packaging / marketplace / demos — **shelved** until language + docs +
  performance are done (see BACKLOG priority policy)
- Self-host (M4) — last
- C/debug async (LANG-3 wontfix for v1)

---

## Language-first implementation (pre-1.0 content) — **closed**

| Area | Status |
|------|--------|
| S3 + inference B | done |
| M1 install path / M2 stdlib / M3 ABI | done |
| STDLIB async + net/fs streams | done |
| LANG-DOC + examples | done |
| LANG-PERF | done (living residual only) |
| LANG-RES | done (reopen on concrete blocker) |
| Active language backlog | see [`BACKLOG.md`](BACKLOG.md) §2 (LANG-MEM wave) |

---

## Path to **1.0** (not started as a gate)

1.0 is a **maturity** call, not a feature checklist:

1. Keep FREEZE-1 discipline through prolonged `0.3.x` use.
2. Real programs / feedback without prolonged intentional breaking.
3. Install path remains Rust-free for end users (M1).
4. Self-host (M4) is **optional and last** — not required for 1.0 utility.
5. Explicit coordinated version, ABI, and CHANGELOG decision before any
   `0.4` or `1.0` declaration.

Until then: ship **0.3.x** packages under the post-freeze compatibility rules.

---

## Packaging under freeze (Linux)

| Artifact | Role |
|----------|------|
| `ori-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Portable tree |
| `ori_X.Y.Z_amd64.deb` | Debian/Ubuntu install (`tools/package_deb.sh`) |

Windows/macOS packages remain deferred (DIST multi-OS).
