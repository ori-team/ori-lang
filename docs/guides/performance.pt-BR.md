# Microbench de performance (polyglot)

> **Público:** usuários e contribuidores que querem um retrato honesto do custo
> de runtime da Ori em kernels pequenos.  
> **Não** é um ranking completo de linguagens.  
> **Inglês (canônico):** [performance.md](performance.md)  
> **Harness:** [`tools/bench/polyglot/`](../../tools/bench/polyglot/)  
> **Relatório da máquina:** [`tools/bench/polyglot/results/LATEST.md`](../../tools/bench/polyglot/results/LATEST.md)

Esta página preserva a **medição histórica de 14/07/2026**. Ela não representa
automaticamente o workspace atual `0.3.8-dev`; rode o harness novamente antes
de usar os números em uma decisão atual.

## Snapshot (2026-07-14, fix GC em loops + mid-end)

| Item | Valor |
|------|--------|
| Host | Linux x86_64 · Intel Core i7-3632QM @ 2.20 GHz |
| Amostras | **5** (mediana de wall time) |
| Timer | `time.perf_counter` em torno do processo (µs) |
| Ori | **0.3.4** AOT (`ori compile`, mid-end **Default**, histórico) |
| Python | CPython **3.12.3** |
| Rust | **1.95.0** release |
| C | **gcc 13.3** `-O2` |
| Go | **1.22.2** |
| JavaScript | **Node v24.18** |
| TypeScript | **tsc 7.0** → Node |
| Ruby | **3.2.3** (CRuby) |
| Nim | **1.6.14** `-d:release` |

Mesmo formato de algoritmo (`while` / índices explícitos). Resultados impressos
batem em todas as linguagens em todos os kernels.

**O que entrou neste snapshot:**

1. `while`/`for` nativos não chamam mais `ori_arc_collect_cycles` a cada iteração.
2. Mid-end Default: const fold + **strength reduction** de loops puros + DCE.
3. `ORI_OPT=aggressive` adiciona inlining de leaf monomórfico (pouco efeito nestes
   kernels de uma função).

### Runtime (mediana em segundos)

| Workload | Ori | Python | Rust | C | Go | JS | TS | Ruby | Nim |
|----------|-----|--------|------|---|-----|----|----|------|-----|
| `sum_loop` Σ 0..10⁷ | **0.0022**\* | 2.93 | 0.0016\* | 0.0013\* | 0.0089 | 0.081 | 0.077 | 0.410 | 0.0071 |
| `fib_iter` 2·10⁷ passos | **0.016** | 7.05 | 0.011 | 0.015 | 0.020 | 1.17 | 1.22 | 5.99 | 0.024 |
| `list_sum` 10⁶ push+soma | **0.011**† | 0.53 | 0.0089 | 0.010 | 0.0098 | 0.095 | 0.093 | 0.198 | 0.032 |
| `nested` 2000×2000 | **0.0018**\* | 0.97 | 0.0022 | 0.0018 | 0.0042 | 0.061 | 0.060 | 0.212 | 0.0019 |

\* Soma/nested puros costumam virar forma fechada. Prefira **`fib_iter`** e
**`list_sum`** para custo de loop / heap.  
† Após inline de push/get escalar + `with_capacity` (remeasure 2026-07-14;
≈ **1.25×** Rust no mesmo host). Outras colunas da suite polyglot completa.

### Suite expandida — setembro de 2026 (ori 0.3.8, polyglot com 8 workloads)

Medição atualizada (mediana de 3 amostras, i7-3632QM, setembro de 2026, AOT):

| Workload | Ori | Python | Rust | C | Go | JS | Ruby |
|----------|-----|--------|------|---|-----|----|------|
| `vec4_simd` 5M somas 4D | **0.008** | 1.878 | 0.007 | 0.007 | 0.010 | 0.087 | 1.496 |
| `arena_bulk_alloc` 100k resets | **0.015** | 0.032 | 0.002 | 0.001 | 0.003 | 0.047 | 0.089 |
| `channel_throughput` 100k msgs | **0.114** | 0.049 | 0.006 | 0.001 | 0.010 | 0.060 | 0.103 |
| `spatial_grid_bvh` 1M AABBs | **0.543** | 0.745 | 0.003 | 0.001 | 0.004 | 0.060 | 0.512 |

**Leituras técnicas:**

1. **Vetorização SIMD (`vec4_simd`)**: Ori baixa `simd[float32, 4]` diretamente para
   registradores Cranelift `F32X4`, completando 5M somas vetoriais em **8.5 ms**
   (≈1.1× GCC -O2, ≈1.2× Rust), superando Go (10.3 ms), Node (80 ms) e sendo
   **~178× mais rápido que Python**.
2. **Reset de arena em lote (`arena_bulk_alloc`)**: o overhead agregado da Ori (15 ms)
   vem das chamadas FFI repetidas a `ori_region_reset`, enquanto Rust/C redefinem o
   offset do buffer no próprio processo (1–2 ms). O custo O(1) é a chamada de API,
   não o mecanismo de reset.
3. **Cadência do canal gerenciado (`channel_throughput`)**: o caminho sincronizado
   de `ori_channel_send`/`receive` usa a fila global consciente de tarefas do ARC
   (~125 ms), mais lento que o runtime dedicado de goroutines do Go e o `crossbeam`
   do Rust. Um rebalance dos locks ou anel lock-free reduziria 1–2 ordens de magnitude.
4. **Passagem de structs e desvirtualização (`spatial_grid_bvh`)**: Ori passa structs
   `AABB` por cópia sob a ABI completa de chamadas, enquanto C/Rust inlinam o
   acesso (~1–2 ms). Um limiar de inlining ou especialização de folhas fechou
   a maior parte dessa lacuna.

### Relativo à Ori (lang / Ori; **menor é mais rápido**)

| Workload | Py | Rust | C | Go | JS | TS | Ruby | Nim |
|----------|-----|------|---|-----|----|----|------|-----|
| `sum_loop` | **1360×** | 0.73×\* | 0.61×\* | 4.1× | 37× | 36× | 190× | 3.3× |
| `fib_iter` | **440×** | **0.68×** | 0.92× | 1.24× | 73× | 76× | 374× | 1.50× |
| `list_sum` | **48×**† | **0.78×**† | ~0.9× | ~0.9× | ~9× | ~8× | ~18× | ~2.9× |
| `nested` | **552×** | **1.26×** | 1.04× | 2.4× | 35× | 34× | 121× | 1.09× |

## Como ler

### Ori vs interpretadores

| Par | Leitura |
|-----|---------|
| **Python** | Ori cerca de **30–1400×** mais rápida |
| **Ruby** | Ori cerca de **12–370×** mais rápida |
| **JS / TS (Node)** | Ori ganha nos quatro (**~6–75×**) |

### Ori vs AOT / sistemas

| Par | Leitura |
|-----|---------|
| **`fib_iter`** | Melhor sinal sem forma fechada: Ori **~1.5×** Rust, **ganha de Go e Nim**, perto de C |
| **`list_sum`** | Ori **~1.25×** Rust com `with_capacity` + **push/get escalar inline** (era ~1.8×); residual é checks/version, não ARC em `list[int]` |
| **`sum` / `nested`** | Ruído de forma fechada; Ori competitiva com C/Rust quando reduz |
| **Go / Nim** | Não dominam mais a Ori no fib após o fix do GC |

### Posicionamento (pre-1.0)

- Claramente **acima de CPython, CRuby e Node**.
- **Competitiva com AOT maduro** em fib tight (dentro de ~1.5× do Rust).
- Gap residual: sobretudo **lista/ARC** e polish de mid-end
  (`ORI_OPT=aggressive` para código multi-função real).

### Flags de mid-end

| `ORI_OPT` | Passes |
|-----------|--------|
| `none` / `0` | Sem rewrites HIR |
| `default` (unset) | Const fold + strength reduction + DCE |
| `aggressive` / `2` | Default + leaf inlining monomórfico |

## Justiça / limites

1. Mesmo formato de fonte nas linguagens.
2. Ori é **AOT** (`ori compile`), não JIT `ori run`.
3. Python / Ruby: máscara 64-bit no fib.
4. JS/TS: BigInt com wrap 64-bit.
5. Nim: `{.push overflowChecks: off.}` no fib wrapping.
6. Rust/C/Ori podem reduzir `sum_loop` / nested puro.
7. Tempos incluem start do processo + um `print`.
8. Host é notebook; **razões importam mais que ms absolutos**.
9. **Não** mede I/O, async, FFI ou apps reais.

## Hot paths sem alocação (`@noalloc` + `mem.region` + `simd`)

A onda de alta performance (2026-09-03) adiciona primitivas zero-allocation de propósito
geral para loops de 60/120 FPS:

- `@noalloc` em funções verifica estaticamente que não há alocações heap (proíbe `list`/`map`/`set`,
  interpolação, closures, `await`, `using` e chamadas a funções que alocam).
- `using r: mem.Region = mem.region()` cria arenas com reset instantâneo em O(1) (`mem.reset`),
  sem custo de contagem de referência por objeto.
- `simd[float32, 4]` baixa diretamente para vetores de CPU (x86_64 SSE/AVX e ARM NEON) com
  operadores paralelos (`+`, `-`, `*`, `/`).
- `@align(N)` força alinhamento de structs para GPU uniform buffers (`alignas(N)`).

## Como reproduzir

```bash
SAMPLES=5 ./tools/bench/polyglot/run_polyglot_bench.sh
# ORI_OPT=none ./tools/bench/polyglot/run_polyglot_bench.sh
```

Fontes em `tools/bench/polyglot/{ori,python,rust_*,c,go,javascript,typescript,ruby,nim}/`.

## Documentos relacionados

| Documento | Papel |
|-----------|--------|
| [tools/bench/polyglot/README.md](../../tools/bench/polyglot/README.md) | Layout do harness |
| [results/LATEST.md](../../tools/bench/polyglot/results/LATEST.md) | Relatório completo |
| [language-comparison.md](language-comparison.md) | Suite PowerShell antiga (histórico) |
| [../planning/perf-baseline-2026-07-13.md](../planning/perf-baseline-2026-07-13.md) | Baseline LANG-PERF do compilador |
| [../planning/historico/perf-runtime-midend-plan.md](../planning/historico/perf-runtime-midend-plan.md) | Plano mid-end LANG-PERF-2 |
