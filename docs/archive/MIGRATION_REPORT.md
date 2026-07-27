# Historical documentation migration report

> Status: **completed**  
> Completed on: 2026-07-27  
> ExecPlan: [`plans/documentation-history-migration.md`](plans/documentation-history-migration.md)

DOC-MIGRATE-1 classified and moved historical planning and loose archive documents into canonical archive categories.

- Inventory candidates migrated: **51**
- Completed ExecPlan archived: **1**
- Transitional `docs/planning/historico/` root: **removed**
- Relative inbound links: **rewritten by resolved target**
- Temporary migration tooling/workflows: **removed**

## Moves

| Original path | Archive path | Category |
|---|---|---|
| `docs/archive/analise-profunda-implementacao-linguagem.md` | [`docs/archive/audits/analise-profunda-implementacao-linguagem.md`](../archive/audits/analise-profunda-implementacao-linguagem.md) | `audits` |
| `docs/archive/auditoria-profunda-implementacao-2026-05-17.md` | [`docs/archive/audits/auditoria-profunda-implementacao-2026-05-17.md`](../archive/audits/auditoria-profunda-implementacao-2026-05-17.md) | `audits` |
| `docs/archive/auditoria-profunda-implementacao-linguagem-2026-05-13.md` | [`docs/archive/audits/auditoria-profunda-implementacao-linguagem-2026-05-13.md`](../archive/audits/auditoria-profunda-implementacao-linguagem-2026-05-13.md) | `audits` |
| `docs/archive/plano-correcao-implementacao-linguagem.md` | [`docs/archive/plans/plano-correcao-implementacao-linguagem.md`](../archive/plans/plano-correcao-implementacao-linguagem.md) | `plans` |
| `docs/archive/relatorio-fechamento-correcao-implementacao-linguagem.md` | [`docs/archive/audits/relatorio-fechamento-correcao-implementacao-linguagem.md`](../archive/audits/relatorio-fechamento-correcao-implementacao-linguagem.md) | `audits` |
| `docs/archive/relatorio-fechamento-nova-rodada.md` | [`docs/archive/audits/relatorio-fechamento-nova-rodada.md`](../archive/audits/relatorio-fechamento-nova-rodada.md) | `audits` |
| `docs/planning/IMPLEMENTADOS.md` | [`docs/archive/audits/IMPLEMENTADOS.md`](../archive/audits/IMPLEMENTADOS.md) | `audits` |
| `docs/planning/PLANO-CDYLIB-EMBED.md` | [`docs/archive/plans/PLANO-CDYLIB-EMBED.md`](../archive/plans/PLANO-CDYLIB-EMBED.md) | `plans` |
| `docs/planning/eco-game-imgui-raylib3d-plan.md` | [`docs/archive/plans/eco-game-imgui-raylib3d-plan.md`](../archive/plans/eco-game-imgui-raylib3d-plan.md) | `plans` |
| `docs/planning/freeze-and-abi-gates.md` | [`docs/archive/plans/freeze-and-abi-gates.md`](../archive/plans/freeze-and-abi-gates.md) | `plans` |
| `docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md` | [`docs/archive/plans/maturity-plan-2026-06.md`](../archive/plans/maturity-plan-2026-06.md) | `plans` |
| `docs/planning/historico/bugcheck-native-ori-ide-2026-07-18.md` | [`docs/archive/investigations/bugcheck-native-ori-ide-2026-07-18.md`](../archive/investigations/bugcheck-native-ori-ide-2026-07-18.md) | `investigations` |
| `docs/planning/historico/c-backend-redefinition.md` | [`docs/archive/investigations/c-backend-redefinition.md`](../archive/investigations/c-backend-redefinition.md) | `investigations` |
| `docs/planning/historico/design-close-backlog-linux-2026-07-13.md` | [`docs/archive/plans/design-close-backlog-linux-2026-07-13.md`](../archive/plans/design-close-backlog-linux-2026-07-13.md) | `plans` |
| `docs/planning/historico/ideias-programas-avancados.md` | [`docs/archive/investigations/ideias-programas-avancados.md`](../archive/investigations/ideias-programas-avancados.md) | `investigations` |
| `docs/planning/historico/io-streams-design.md` | [`docs/archive/investigations/io-streams-design.md`](../archive/investigations/io-streams-design.md) | `investigations` |
| `docs/planning/historico/issue-ffi-dispatch-large-binary-2026-07-16.md` | [`docs/archive/investigations/issue-ffi-dispatch-large-binary-2026-07-16.md`](../archive/investigations/issue-ffi-dispatch-large-binary-2026-07-16.md) | `investigations` |
| `docs/planning/historico/lang-mem-9-runtime-wrappers-2026-07-18.md` | [`docs/archive/investigations/lang-mem-9-runtime-wrappers-2026-07-18.md`](../archive/investigations/lang-mem-9-runtime-wrappers-2026-07-18.md) | `investigations` |
| `docs/planning/historico/lang-res-closure.md` | [`docs/archive/audits/lang-res-closure.md`](../archive/audits/lang-res-closure.md) | `audits` |
| `docs/planning/historico/net-v2-design.md` | [`docs/archive/investigations/net-v2-design.md`](../archive/investigations/net-v2-design.md) | `investigations` |
| `docs/planning/historico/nim-study-2026-07-16-c0.md` | [`docs/archive/investigations/nim-study-2026-07-16-c0.md`](../archive/investigations/nim-study-2026-07-16-c0.md) | `investigations` |
| `docs/planning/historico/nim-study-2026-07-17-c1.md` | [`docs/archive/investigations/nim-study-2026-07-17-c1.md`](../archive/investigations/nim-study-2026-07-17-c1.md) | `investigations` |
| `docs/planning/historico/nim-study-2026-07-17-c2.md` | [`docs/archive/investigations/nim-study-2026-07-17-c2.md`](../archive/investigations/nim-study-2026-07-17-c2.md) | `investigations` |
| `docs/planning/historico/nim-study-2026-07-17-c3.md` | [`docs/archive/investigations/nim-study-2026-07-17-c3.md`](../archive/investigations/nim-study-2026-07-17-c3.md) | `investigations` |
| `docs/planning/historico/nim-study-2026-07-17-c4-c7.md` | [`docs/archive/investigations/nim-study-2026-07-17-c4-c7.md`](../archive/investigations/nim-study-2026-07-17-c4-c7.md) | `investigations` |
| `docs/planning/historico/perf-runtime-midend-plan.md` | [`docs/archive/plans/perf-runtime-midend-plan.md`](../archive/plans/perf-runtime-midend-plan.md) | `plans` |
| `docs/planning/historico/plano-correcao-bugs-2026-05-17.md` | [`docs/archive/plans/plano-correcao-bugs-2026-05-17.md`](../archive/plans/plano-correcao-bugs-2026-05-17.md) | `plans` |
| `docs/planning/historico/plano-implementacao-lsp-avancado.md` | [`docs/archive/plans/plano-implementacao-lsp-avancado.md`](../archive/plans/plano-implementacao-lsp-avancado.md) | `plans` |
| `docs/planning/historico/porting-raylib-sqlite-cimgui.md` | [`docs/archive/plans/porting-raylib-sqlite-cimgui.md`](../archive/plans/porting-raylib-sqlite-cimgui.md) | `plans` |
| `docs/planning/historico/pr-plan-ori-surface-s3.md` | [`docs/archive/plans/pr-plan-ori-surface-s3.md`](../archive/plans/pr-plan-ori-surface-s3.md) | `plans` |
| `docs/planning/historico/registry-v2.md` | [`docs/archive/plans/registry-v2.md`](../archive/plans/registry-v2.md) | `plans` |
| `docs/planning/historico/result-ctors-ok-err.md` | [`docs/archive/plans/result-ctors-ok-err.md`](../archive/plans/result-ctors-ok-err.md) | `plans` |
| `docs/planning/historico/rust-independence.md` | [`docs/archive/plans/rust-independence.md`](../archive/plans/rust-independence.md) | `plans` |
| `docs/planning/historico/security-performance-testing.md` | [`docs/archive/audits/security-performance-testing.md`](../archive/audits/security-performance-testing.md) | `audits` |
| `docs/planning/historico/sessao-nim-arc-2026-07-16.md` | [`docs/archive/sessions/sessao-nim-arc-2026-07-16.md`](../archive/sessions/sessao-nim-arc-2026-07-16.md) | `sessions` |
| `docs/planning/historico/stdlib-gap-parity.md` | [`docs/archive/audits/stdlib-gap-parity.md`](../archive/audits/stdlib-gap-parity.md) | `audits` |
| `docs/planning/language-direction-decisions-2026-06-30.md` | [`docs/archive/legacy/language-direction-decisions-2026-06-30.md`](../archive/legacy/language-direction-decisions-2026-06-30.md) | `legacy` |
| `docs/planning/manifest-schema.md` | [`docs/archive/plans/manifest-schema.md`](../archive/plans/manifest-schema.md) | `plans` |
| `docs/planning/ori-surface-s3-auk9.md` | [`docs/archive/legacy/ori-surface-s3-auk9.md`](../archive/legacy/ori-surface-s3-auk9.md) | `legacy` |
| `docs/planning/package-ecosystem-guidelines.md` | [`docs/archive/legacy/package-ecosystem-guidelines.md`](../archive/legacy/package-ecosystem-guidelines.md) | `legacy` |
| `docs/planning/perf-baseline-2026-07-13.md` | [`docs/archive/audits/perf-baseline-2026-07-13.md`](../archive/audits/perf-baseline-2026-07-13.md) | `audits` |
| `docs/planning/plano-arc-nim-2026-07-16.md` | [`docs/archive/plans/plano-arc-nim-2026-07-16.md`](../archive/plans/plano-arc-nim-2026-07-16.md) | `plans` |
| `docs/planning/prompt-analisar-nim-para-ori.md` | [`docs/archive/investigations/prompt-analisar-nim-para-ori.md`](../archive/investigations/prompt-analisar-nim-para-ori.md) | `investigations` |
| `docs/planning/qa/residual-cleanup-2026-07-13.md` | [`docs/archive/audits/residual-cleanup-2026-07-13.md`](../archive/audits/residual-cleanup-2026-07-13.md) | `audits` |
| `docs/planning/registry-v1.md` | [`docs/archive/plans/registry-v1.md`](../archive/plans/registry-v1.md) | `plans` |
| `docs/planning/roadmap-maturidade-v0.4-v0.5.md` | [`docs/archive/plans/roadmap-maturidade-v0.4-v0.5.md`](../archive/plans/roadmap-maturidade-v0.4-v0.5.md) | `plans` |
| `docs/planning/roadtov1.md` | [`docs/archive/plans/roadtov1.md`](../archive/plans/roadtov1.md) | `plans` |
| `docs/planning/stdlib-merge-policy.md` | [`docs/archive/legacy/stdlib-merge-policy.md`](../archive/legacy/stdlib-merge-policy.md) | `legacy` |
| `docs/planning/uso-real-pequeno-medio.md` | [`docs/archive/plans/uso-real-pequeno-medio.md`](../archive/plans/uso-real-pequeno-medio.md) | `plans` |
| `docs/planning/web-framework-learning-course.md` | [`docs/archive/investigations/web-framework-learning-course.md`](../archive/investigations/web-framework-learning-course.md) | `investigations` |
| `docs/planning/web-templates-discussion-roadmap.md` | [`docs/archive/investigations/web-templates-discussion-roadmap.md`](../archive/investigations/web-templates-discussion-roadmap.md) | `investigations` |
| `docs/plans/active/documentation-history-migration.md` | [`docs/archive/plans/documentation-history-migration.md`](../archive/plans/documentation-history-migration.md) | `plans` |

## Validation

- canonical documentation validator passes after the moves;
- archive category indexes are generated from final files;
- active planning no longer contains completed plans or the historical root;
- the catalog no longer declares `docs/planning/historico` as a historical root;
- CI rejects any future Markdown file added under the retired root;
- Git history retains the original content and paths.

## Intentional compatibility pointers

The migrated ADR and repository-layout paths under `docs/planning/` remain concise compatibility pointers because they are likely external entry points. They are not historical content roots and are excluded from the archive migration.
