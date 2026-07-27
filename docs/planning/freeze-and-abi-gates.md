# Freeze and ABI gates (FREEZE-1 / ABI-1)

> **Status (2026-07-13):** both gates **done / in force** — process **finalized**.  
> Calendar window: opened **2026-07-13** · remains open through pre-1.0 `0.3.x`
> until an explicit exit is recorded in CHANGELOG + this file.  
> This file also carries the **1.0 readiness checklist** (merged 2026-07-17 from
> the former `freeze-1-0-readiness.md`; the FREEZE-1/ABI-1 rules were duplicated).

---

## FREEZE-1 — **DONE** (window open)

**Opened:** 2026-07-13  
**Exit:** not closed yet (intentional long pre-1.0 window on `0.3.x`)

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

### How to exit freeze

1. Decide end date and next version line (`0.4+` or approach to 1.0).
2. Document in CHANGELOG (`FREEZE-1 closed YYYY-MM-DD`).
3. Update BACKLOG (already **done** for opening the gate).

---

## ABI-1 — **DONE** (enforcement in force)

With FREEZE-1 open, **ABI-1 is enforced**:

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
5. Explicit CHANGELOG entry when declaring 1.0 (or when closing FREEZE-1 into 0.4/1.0).

Until then: ship **0.3.x** packages under freeze.

---

## Packaging under freeze (Linux)

| Artifact | Role |
|----------|------|
| `ori-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Portable tree |
| `ori_X.Y.Z_amd64.deb` | Debian/Ubuntu install (`tools/package_deb.sh`) |

Windows/macOS packages remain deferred (DIST multi-OS).
