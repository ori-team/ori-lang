# Planning and backlog — Ori

> **Audience:** maintainers and contributors.  
> **Not** end-user tutorials — those live under [../guides/](../guides/) and
> [../language/](../language/).  
> Product docs policy: [../README.md](../README.md) (EN primary, PT parallel).

## Active

| Document | Role |
|----------|------|
| **[BACKLOG.md](BACKLOG.md)** | **Only** open implementation list (priority · difficulty · deps · waves) |
| [PENDENTES.md](PENDENTES.md) | History + pointer to BACKLOG |
| [uso-real-pequeno-medio.md](uso-real-pequeno-medio.md) | Small/medium real-use narrative (open items → BACKLOG) |
| [stdlib-merge-policy.md](stdlib-merge-policy.md) | Stdlib API merge policy (M2) |
| [repo-and-project-layout.md](repo-and-project-layout.md) | Monorepo + root-first projects |
| [result-ctors-ok-err.md](result-ctors-ok-err.md) | `ok`/`err` rename notes |
| [ori-surface-s3-auk9.md](ori-surface-s3-auk9.md) | S3 surface decisions (living record) |
| [adr-ori-surface-s3-auk9.md](adr-ori-surface-s3-auk9.md) | ADR accepted for S3 |
| [pr-plan-ori-surface-s3.md](pr-plan-ori-surface-s3.md) | PR plan (completed) |
| [package-ecosystem-guidelines.md](package-ecosystem-guidelines.md) | Package conventions |
| [eco-packages-status.md](eco-packages-status.md) | ECO inventory (`game-engine-full/`) — all packages **5 (Linux)**; maturity-5 plan complete |
| [eco-library-ports-catalog.md](eco-library-ports-catalog.md) | **Canonical** — all ports **5 (Linux)** incl. U1–U15 @ 0.2.0; alta/média **fechadas** |
| [pr-plan-eco-ports-e2e.md](pr-plan-eco-ports-e2e.md) | Execute-plan DAG — medium ports 0.1.0 (**complete**) |
| [pr-plan-eco-maturity-5.md](pr-plan-eco-maturity-5.md) | Engine packages → 5 (Linux) — **PRs 1–19 complete** (W10) |
| [pr-plan-imgui-tools-maturity-5.md](pr-plan-imgui-tools-maturity-5.md) | **PRs 1–14 done** (`a68f7529`) — stages A–D + catalog; **PR15 Phase OS last** |
| [PHASE-OS.md](PHASE-OS.md) | **Phase OS** Windows/mac ECO staging (**last**, non-blocking) |
| [game-ports-maturity-matrix.md](game-ports-maturity-matrix.md) | **Consult** — Table A (all **5 Linux**) · A2 historical U-ids · W10 **done** |
| [roadtov1.md](roadtov1.md) | Long-horizon 1.0 sketch |
| [perf-baseline-2026-07-13.md](perf-baseline-2026-07-13.md) | LANG-PERF baselines + polyglot Ori/Py/Rust snapshot |

## Historical / archive

| Path | Role |
|------|------|
| [IMPLEMENTADOS.md](IMPLEMENTADOS.md) | Chronological “done” log |
| [historico/](historico/) | Finished designs (net-v2, rust-independence, maturity plan, …) |
| [language-direction-decisions-2026-06-30.md](language-direction-decisions-2026-06-30.md) | Older language-direction ADR |
| [porting-raylib-sqlite-cimgui.md](porting-raylib-sqlite-cimgui.md) | Historical — **superseded** by `eco-library-ports-catalog.md` |
| [PHASE-OS.md](PHASE-OS.md) | **Phase OS** Windows/mac ECO staging |
| [ideias-programas-avancados.md](ideias-programas-avancados.md) | Scratch ideas |

Do **not** treat `historico/` or closed PR plans as the current language surface.
Normative syntax: [../spec/](../spec/README.md).
