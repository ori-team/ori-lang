---
name: ori-lang-qa
description: >
  Qualidade da linguagem Ori (compilador AOT S3/0.3.x): matriz de testes por
  estágio (lexer→link), performance, diagnostics/catálogo, residuals nativos,
  specs vivas e FREEZE-1. Use em QA diário, regressão, fuzz leve, catalog,
  Spec 14, BACKLOG language. Use com /ori-lang-qa. Complements ori-testing,
  compiler-dev, lang-compiled, living-docs, rust.
when-to-use: >
  ori-lang, QA diário, test matrix, diagnostic catalog, residual backend,
  performance bench, conformance, freeze-1, spec update, language quality.
metadata:
  short-description: "QA/maturidade do compilador Ori"
  author: "raillen"
  source: "ori-lang project + grok-memory"
---

# Ori Language QA & Maturity

Skill **profunda** para manter Ori (monorepo `ori-lang`) sob **FREEZE-1 / 0.3.x**:
testes em estágios, performance, catálogo de erros, residuals, specs.

**Precedência:** `AGENTS.md` do repo > esta skill > `ori-testing` / `compiler-dev`.

**Não é Engine A/B nem ECO packages** — só **linguagem + compiler + stdlib + docs normativas**.

## Pilares

1. **Matriz mapeada ao produto Ori** — nem todo teste de “compilador genérico” se aplica (sem borrow checker, sem VM produto, ARC ≠ GC clássico).
2. **Estágios diários** — scripts em `tools/qa/` (fast → full → perf).
3. **Diagnostics** — código estável + mensagem acionável + catálogo `docs/spec/13-error-catalog.md`.
4. **Residuals** — Spec 14 + `lang-res-closure.md`; reabrir só com repro real.
5. **Specs vivas** — `docs/spec/` inglês normativo alinhado a S3 + inference B + package 0.3.x.

## Skills combinadas

| Skill | Papel |
|-------|--------|
| `ori-testing` | L1 check → L2 compile → L3 run + regressão driver |
| `compiler-dev` | Fase correta da mudança + checklist |
| `lang-compiled` | IR/SSA, native, link, JIT-as-run |
| `living-docs` | CHANGELOG, spec vs planning |
| `rust` | crates do compiler |
| `check-work` | fechar fatia grande |

## Agents (personas de sessão)

| Agent | Foco | Ver |
|-------|------|-----|
| `ori-lang-frontend` | lexer, parser, resolve, types, diagnostics | `.grok/agents/ori-lang-frontend.md` |
| `ori-lang-backend` | HIR, codegen, runtime, residuals | `.grok/agents/ori-lang-backend.md` |
| `ori-lang-diagnostics` | catalog, mensagens, recovery | `.grok/agents/ori-lang-diagnostics.md` |
| `ori-lang-qa-daily` | stages diários, perf, matrix | `.grok/agents/ori-lang-qa-daily.md` |
| `ori-lang-docs` | specs/guides/CHANGELOG sob freeze | `.grok/agents/ori-lang-docs.md` |

## Matriz de testes (resumo)

Documento completo: `docs/planning/qa/test-matrix-ori.md`  
e `references/test-matrix.md` (cópia skill).

| Stage | Nome | Comando típico |
|-------|------|----------------|
| **S0** | Workspace compile + compiler quality | `cargo check --workspace` + strict Clippy for runtime/codegen/driver |
| **S1** | Unit crates (lexer→types) | `cargo test -p ori-lexer -p ori-parser -p ori-types -p ori-hir` |
| **S2** | Spec + diagnostics | `cargo test -p ori-driver --test ori_spec --test diagnostic_catalog` |
| **S3** | Memory/security/async | `cargo test -p ori-driver --test memory_arc --test security_robustness --test concurrency_async` |
| **S4** | Multifile / stdlib / packages | `cargo test -p ori-driver --test multifile_imports` |
| **S5** | Full workspace | `cargo test --workspace` |
| **S6** | Examples product surface | `tools/qa/examples_smoke.sh` |
| **S7** | Perf micro | `tools/microbench_lang_perf.sh` / `tools/qa/perf_daily.sh` |
| **S8** | Residual gate | `cargo test -p ori-driver --test concurrency_async compile_runs_lang_res` |

Daily default: **S0→S4 + S8** (`tools/qa/daily_fast.sh`).  
Weekly: **S0→S7** (`tools/qa/daily_full.sh`).

### Categorias da tabela do usuário → Ori

Ver `references/test-matrix.md` para cada linha: **APLICA / PARCIAL / N/A**.

**N/A no produto Ori (não inventar suíte fake):**  
Borrow checker, lifetime refs Rust-like, bytecode/VM product, double-free/use-after-free clássicos (ARC), GC genérico (há cycle collector + ARC).

**Aplica com outro nome:**  
Null safety → `optional` / `result`; ownership → ARC/value semantics; memory leak → ARC + `ORI_TEST_LEAK_CHECK` / cycle collect.

## Protocolo de mudança de linguagem

1. Spec (se normativo) em `docs/spec/`.  
2. Implementar na **fase correta**.  
3. Diagnostic code no **13-error-catalog** (Emitted).  
4. Teste driver (`check_fails` / `compile_runs`).  
5. `cargo test -p ori-driver --test diagnostic_catalog`.  
6. CHANGELOG se user-facing.  
7. FREEZE-1: **sem breaking** em 0.3.x sem bump + nota de freeze exit.

## Mensagens de erro (qualidade)

Toda mensagem user-facing deve:

1. **Código** estável (`category.name`)  
2. **O quê** falhou (em inglês nas specs; render pode i18n depois)  
3. **Onde** (span honesto)  
4. **Ação** (`action:` / help) quando o fix for mecânico  
5. **Não mentir** sobre o estado da linguagem (ex. não sugerir `func` em S3)

Checklist: `references/diagnostics-quality.md`.

## Residuais (limpeza)

| Tipo | Ação |
|------|------|
| Product-blocking `native_unsupported` | **Fix** + teste positivo |
| Documented intentional (Spec 14) | Manter inventário; teste negativo se útil |
| C/debug gap | Não expandir como “bug native” |
| Synthetic residual | **Não** reabrir LANG-RES |

Audit: `tools/qa/residual_audit.sh`.

## Comandos rápidos

```bash
cd compiler   # ou repo root se workspace na raiz

# Daily fast
../tools/qa/daily_fast.sh

# Full + perf
../tools/qa/daily_full.sh

# Catalog
cargo test -p ori-driver --test diagnostic_catalog

# Residual product surface
cargo test -p ori-driver --test concurrency_async compile_runs_lang_res_product_surface_native
```

## Referências

- `references/test-matrix.md` — tabela completa mapeada  
- `references/diagnostics-quality.md`  
- `references/daily-stages.md`  
- `references/residual-policy.md`  
- Repo: `docs/planning/qa/test-matrix-ori.md`  
- Repo: `docs/planning/historico/lang-res-closure.md`  
- Repo: `docs/spec/13-error-catalog.md`, `14-backend-support.md`  
- Repo: `docs/planning/BACKLOG.md` (fila language = empty; maintenance only)
