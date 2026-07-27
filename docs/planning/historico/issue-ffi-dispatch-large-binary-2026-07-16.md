# Issue LANG-PERF-3 — custo de chamada FFI explode em binário grande

> **Aberto:** 2026-07-16 · **Origem:** pivot do Studio (game-engine-full, 2026-07-15)  
> **Status:** **corrigido em 2026-07-16** (root cause: registro ARC linear, não dispatch) · **Backlog:** `BACKLOG.md` §2 → LANG-PERF-3

## Resolução (2026-07-16)

**Root cause:** não era dispatch de símbolo. O registro ARC do runtime
(`ori-runtime/src/lib.rs`) guardava **todas** as alocações vivas em um `Vec`
e cada `ori_arc_retain` / `ori_arc_release` / registro de edge fazia
**varredura linear** desse vetor sob mutex global. Custo por operação ARC =
O(alocações vivas). Binário grande (Studio) = heap grande no boot = cada
temporário gerenciado na fronteira FFI custava ~1,5ms. O nº de
funções/símbolos era irrelevante (hipóteses 1, 3 e 4 descartadas por medição).

**Fix:** alocações agora são um `HashMap` chaveado pelo endereço do payload;
edges de ownership são indexados por owner **e** por child. Retain/release e
edges viram O(1); `ori_arc_collect_cycles` caiu de O(n²) para O(n + e).
Semântica preservada (mesma API, mesmos testes).

**Medição (repro sintético — 1 chamada extern + 1 temp gerenciado por iteração):**

| Alocações vivas | Antes (por iteração) | Depois (por iteração) |
|-----------------|----------------------|----------------------|
| 1 000 | ~28µs | ~1,5µs |
| 10 000 | ~226µs | ~1,5µs |
| 100 000 | (extrapolado ~2,3ms) | ~1,6µs |

Custo agora **constante** em relação ao heap vivo (paridade com binário
pequeno, ordem de µs — critério de fechamento atendido no repro).

**Regressão:** `compiler/crates/ori-driver/tests/performance_guard.rs` →
`run_ffi_boundary_cost_stays_flat_with_many_live_allocations`
(20k strings vivas + 20k iterações FFI; budget estrito via `ORI_PERF_STRICT=1`,
`ORI_PERF_FFI_ARC_REGISTRY_BUDGET_MS`).

### Validação externa (2026-07-17, lab `game-engine-full`)

**Runtime comparado:** instalado (pré-fix, release) vs `ori-lang` staged
**release** (`cargo build -p ori-runtime --release` +
`runtime/x86_64-unknown-linux-gnu/`).

| Cenário | Pré-fix | Pós-fix (release) |
|---------|----------|---------------------|
| Sintético headless: 50k strings vivas + 10k iterações (extern `labs` + temp gerenciado) | ~3800µs/iter (JIT) | **~5µs/iter** (JIT e AOT) |
| `perf_probe` ImGui `frame`, **sem** ballast | (baseline lab ~60fps) | **~90–400 fps** (média raylib nos primeiros 30 frames) |
| `perf_probe` + **50k** strings vivas no heap | ~2fps (sintoma original) | **ainda ~2fps** |

**Conclusão:** o critério de **paridade µs na fronteira FFI** (repro sintético)
está fechado. O FPS do probe com heap grande **não** volta a 60fps só com o
fix do registry: residual é o **cycle collector em todo return de função**
(`native_backend` emite `ori_arc_collect_cycles` quando
`managed_start == 0 && loop_stack.is_empty()`), O(n+e) sobre o heap vivo —
`on_update`/`on_draw` e helpers disparam full-heap scan com dezenas de milhares
de alocações. Desligar o collect cooperativo
(`ORI_COOPERATIVE_COLLECT_THRESHOLD` alto) **não** muda o FPS; mode `none` vs
`frame` com 50k ballast também ~2fps → não é ImGui, é o scan no root.

**Mitigação 2026-07-17 (LANG-MEM-3 parcial):** function roots e post-await
passam a chamar `ori_arc_maybe_collect_cycles` (threshold 256 alocações), não
full scan em todo return. Residual de collector **completo** (buffer de
suspeitos / passe O(suspeitos)) continua em F3 do plano Nim.

### Re-medida shell Studio (2026-07-17, lab `game-engine-full`)

Binário: `ori-imgui/demos/studio_shell` AOT com compiler local
(`compiler/target/debug/ori` 0.3.5 + commits LANG-PERF-3 / maybe_collect) e
runtime **release** staged (`runtime/x86_64-unknown-linux-gnu/libori_runtime.a`
com `ori_arc_maybe_collect_cycles`). Host: Intel HD Graphics 4000, DISPLAY X11.
Compile wall ≈52s; run ~75s (`timeout`).

| Métrica | Antes (issue) | Depois (esta sessão) |
|---------|---------------|----------------------|
| Shell UI interativo | ~**2fps** | **~48–60fps** (36 amostras `STUDIO-PERF`, média **58**) |
| `DIAG-FFI: 100k app.fps()` | ~ms/call × 100k (shell morto) | **5 ms** total (~50 ns/call) |
| Critério “~60fps edit lite” | falha | **atendido** (vsync 60 no host) |

Amostras `STUDIO-PERF` (cada 120 frames): quase todas em 54–60; mínimo 48.
Timers por seção no log (`update`/`view3d`/`imgui` ms) ficaram 0–1 (resolução
inteira ms — frame sob orçamento).

**Nota separada (não bloqueia):** `ori compile` de fonte com 10k funções levou
~4min (provável custo quadrático no front/mid-end) — candidato a novo item de
backlog LANG-PERF.

## TL;DR

Em binário Ori **grande**, cada chamada FFI custa **~1,5ms**.
Em binário **pequeno**, a mesma chamada custa **~0,55µs** — **~3000× de diferença**,
com as **mesmas libs e o mesmo host**. Isso derrubou o shell ImGui do Studio
para 2fps e afeta **qualquer app Ori grande**, não só editor.

## Sintoma medido (2026-07-15)

| Cenário | Custo por chamada FFI | FPS |
|---------|----------------------|-----|
| Binário pequeno (probe isolado) | ~0,55µs | 60fps com ImGui completo |
| Binário grande (shell Studio) | ~1,5ms | ~2fps |

- ImGui, OpenGL e raylib foram **inocentados** por probes isolados:
  `game-engine-full/ori-imgui/demos/perf_probe` roda a 60fps com ImGui completo.
- O custo cresce com o **tamanho do binário Ori**, não com a lib chamada.

## Hipótese

Bug de **dispatch** no ori-lang proporcional ao tamanho do binário
(ou ao número de símbolos/funções/globals). Candidatos a verificar:

1. Lookup de função extern por varredura linear (tabela de símbolos/registro do runtime).
2. Trabalho de ARC / cycle collector na fronteira FFI proporcional ao nº de globals.
3. Lazy binding / PLT do loader (teste rápido: `LD_BIND_NOW=1` — se mudar, não é bug nosso).
4. Diferença SystemLinker vs RustcDriver na resolução de chamadas.

## Repro

O repro vive no lab `game-engine-full` (fora deste repo):

```bash
cd /home/raillen/Documentos/Projetos/game-engine-full

# Caso rápido (binário pequeno, 60fps — controle):
#   ori-imgui/demos/perf_probe  → tools/stage_libs.sh + ori run
#   modos: sem arg = raylib puro · "init" = ui.init_raylib() · "frame" = frame completo

# Caso lento (binário grande, 2fps):
#   ori-studio/tools/run.sh  (shell ImGui, hoje laboratório)
#   F6 alterna UI/MINIMAL e loga "PERF A/B: ... avg fps≈N"
```

Repro sintético sugerido (dentro do ori-lang, sem deps de jogo):

1. Gerar programa com **1 chamada extern em loop** + **N funções/globals dummy**.
2. Medir custo/chamada para N = 10, 100, 1k, 10k.
3. Se o custo escalar com N → confirmado dispatch proporcional ao binário.

## Plano de investigação

1. Repro sintético acima (isola o compiler; sem raylib/ImGui).
2. `perf record` no caso lento → onde o tempo vai (símbolo exato).
3. Corrigir no ponto encontrado; re-medir probe + shell.

## Critério de fechamento

- Custo por chamada FFI em binário grande volta à ordem de **µs** (paridade com binário pequeno).
- Shell ImGui do lab (`ori-studio`) interativo (~60fps edit lite).
- Teste de regressão no repo (microbench FFI × N símbolos) + CHANGELOG.

## Referências

- Diagnóstico original: `game-engine-full` commit `89ee961`
  ("pivot Studio destino = Tauri + diagnóstico perf ImGui shell"), seção
  "Pivot 2026-07-15 (noite)" do DEV-HANDOFF da época.
- Probe: `game-engine-full/ori-imgui/demos/perf_probe/`
- Estudo de frame loop (não confundir — outro problema, já resolvido):
  `game-engine-full/docs/planning/STUDY-TO-ORI-PERF.md`
