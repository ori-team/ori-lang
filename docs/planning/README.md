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
| [plano-arc-nim-2026-07-16.md](plano-arc-nim-2026-07-16.md) | **LANG-MEM-0…9** — plano ARC/ORC do estudo Nim (**concluído 2026-07-18**: dono único da cascata, collector incremental, elisão, wrappers do runtime, spec + ADR COW) |
| [prompt-analisar-nim-para-ori.md](prompt-analisar-nim-para-ori.md) | Prompt mestre do programa de estudo Nim→Ori (campanhas C0–C7; requer clone local do Nim em `_references/nim-lang/`, gitignored) |
| [PLANO-CDYLIB-EMBED.md](PLANO-CDYLIB-EMBED.md) | `ori compile --lib` / embed (P1 done; P2–P5 open) |
| [eco-game-imgui-raylib3d-plan.md](eco-game-imgui-raylib3d-plan.md) | External packages plan (ori-game / ori-imgui / raylib3d …) |
| [freeze-and-abi-gates.md](freeze-and-abi-gates.md) | FREEZE-1 / ABI-1 gates **+ 1.0 readiness checklist** (merged) |
| [stdlib-merge-policy.md](stdlib-merge-policy.md) | Stdlib API merge policy (M2) |
| [repo-and-project-layout.md](repo-and-project-layout.md) | Monorepo + root-first projects |
| [ori-surface-s3-auk9.md](ori-surface-s3-auk9.md) | S3 surface decisions (living record) |
| [adr-ori-surface-s3-auk9.md](adr-ori-surface-s3-auk9.md) | ADR accepted for S3 |
| [registry-v1.md](registry-v1.md) | Package registry v1 (living contract) |
| [manifest-schema.md](manifest-schema.md) | Manifest schema freeze (PKG-4) |
| [package-ecosystem-guidelines.md](package-ecosystem-guidelines.md) | Package conventions |
| [roadtov1.md](roadtov1.md) | Long-horizon 1.0 sketch |
| [perf-baseline-2026-07-13.md](perf-baseline-2026-07-13.md) | LANG-PERF baselines + polyglot multi-lang snapshot |
| [qa/test-matrix-ori.md](qa/test-matrix-ori.md) | Product-mapped compiler test matrix |
| [qa/residual-cleanup-2026-07-13.md](qa/residual-cleanup-2026-07-13.md) | Residual surface cleanup snapshot |
| [web-templates-discussion-roadmap.md](web-templates-discussion-roadmap.md) | **ori-templates / ori-web / HTML-first** — discussion roadmap (syntax, SEC, packages) |
| [web-framework-learning-course.md](web-framework-learning-course.md) | **Curso** (pt-BR): conceitos web/security/htmx + decisões Ori (estudo/revisão) |
| (mesmo roadmap §12) | Futuro **ori-web-app** Rails-like (D21); convenções APP* |

## Historical / archive

| Path | Role |
|------|------|
| [IMPLEMENTADOS.md](IMPLEMENTADOS.md) | Chronological “done” log |
| [historico/](historico/) | Finished designs and closed plans (see below) |
| [language-direction-decisions-2026-06-30.md](language-direction-decisions-2026-06-30.md) | Older language-direction ADR (still cited by the Nim study) |
| [historico/nim-study-2026-07-16-c0.md](historico/nim-study-2026-07-16-c0.md) | Nim→Ori study note C0 (glossary, destroy paths, open questions) |
| [historico/sessao-nim-arc-2026-07-16.md](historico/sessao-nim-arc-2026-07-16.md) | Session log — resume point after machine switch |
| [historico/issue-ffi-dispatch-large-binary-2026-07-16.md](historico/issue-ffi-dispatch-large-binary-2026-07-16.md) | **LANG-PERF-3** issue (resolved: ARC registry linear → HashMap) |
| [historico/perf-runtime-midend-plan.md](historico/perf-runtime-midend-plan.md) | LANG-PERF-2 plan (**done**, waves 0–6) |
| [historico/pr-plan-ori-surface-s3.md](historico/pr-plan-ori-surface-s3.md) | S3 PR plan (completed, PRs 1–11 + option B) |
| [historico/result-ctors-ok-err.md](historico/result-ctors-ok-err.md) | `ok`/`err` rename (delivered 2026-07-13) |
| [historico/lang-res-closure.md](historico/lang-res-closure.md) | LANG-RES closure (normative inventory now in Spec 14) |
| [historico/design-close-backlog-linux-2026-07-13.md](historico/design-close-backlog-linux-2026-07-13.md) | Close-backlog design (executed) |
| [historico/porting-raylib-sqlite-cimgui.md](historico/porting-raylib-sqlite-cimgui.md) | **Archived** community-port ideas — not core backlog |
| [historico/ideias-programas-avancados.md](historico/ideias-programas-avancados.md) | Scratch ideas (validation programs) |

Do **not** treat `historico/` or closed PR plans as the current language surface.
Normative syntax: [../spec/](../spec/README.md).
