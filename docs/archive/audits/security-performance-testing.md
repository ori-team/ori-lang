# Testes de seguranca e performance

> Status: **archived**  
> Original path: `docs/planning/historico/security-performance-testing.md`  
> Original date: before 2026-07-27  
> Archived on: 2026-07-27  
> Category: `audits`  
> Current replacement: `docs/ATLAS.md`  
> Warning: versions, syntax, commands, paths, and priorities below may be obsolete.


Este documento registra a estrategia atual para testar seguranca e performance da linguagem Ori.

O foco e simples:

- o compilador nao deve entrar em panic com entrada malformada;
- diagnosticos devem ter codigos estaveis e spans validos;
- HTML gerado por `ori doc` deve escapar conteudo de comentarios;
- o runtime nativo deve manter zero vazamento nos cenarios cobertos;
- performance deve ter medicao repetivel sem deixar o `cargo test` normal fragil.

## Onde ficam os testes

| Arquivo | Papel |
| --- | --- |
| `compiler/crates/ori-driver/tests/security_robustness.rs` | Corpus adversarial de lexer/parser/checker, regras de seguranca semantica, escaping de HTML e smoke de runtime com leak-check. |
| `compiler/crates/ori-driver/tests/performance_guard.rs` | Guardas de performance para `ori check`, grafo de imports, `ori fmt`, `ori doc` e probe pesado de runtime gerado. |
| `compiler/crates/ori-driver/tests/common/mod.rs` | Helpers compartilhados: diretorio temporario, spans de diagnostico, path do binario `ori`. |

## Como rodar

Coleta automatica de metricas em CSV/TXT:

```bash
cargo run -p ori-driver -- run tools/quality_metrics.orl
```

Saida:

```text
target/ori-quality-metrics/
```

Suite de seguranca e robustez:

```bash
cargo test -p ori-driver --test security_robustness
```

Suite de performance leve:

```bash
cargo test -p ori-driver --test performance_guard
```

Modo estrito de performance:

```bash
ORI_PERF_STRICT=1 cargo test -p ori-driver --test performance_guard
```

Probe pesado de runtime:

```bash
ORI_PERF_STRICT=1 cargo test -p ori-driver --test performance_guard -- --ignored
```

## Budgets configuraveis

Os budgets abaixo so sao aplicados quando `ORI_PERF_STRICT=1`.

| Variavel | Default |
| --- | ---: |
| `ORI_PERF_CHECK_SINGLE_FILE_BUDGET_MS` | `2000` |
| `ORI_PERF_CHECK_IMPORT_GRAPH_BUDGET_MS` | `2500` |
| `ORI_PERF_FMT_SURFACE_BUDGET_MS` | `1500` |
| `ORI_PERF_DOC_SURFACE_BUDGET_MS` | `1500` |
| `ORI_PERF_RUNTIME_PROBE_BUDGET_MS` | `3500` |
| `ORI_PERF_CHECK_LARGE_SCALE_BUDGET_MS` | `10000` |
| `ORI_PERF_COMPILE_LARGE_SCALE_BUDGET_MS` | `60000` |

O probe de escala fica ignorado na suíte normal. Ele aceita
`ORI_PERF_LARGE_FUNCTION_COUNT` (10.000 por padrão),
`ORI_PERF_LARGE_COMPILE=1` para medir AOT e
`ORI_INTERNAL_PIPELINE_TIMINGS=1` para separar lexer/parser, resolução,
checagem, HIR, codegen e link.

## CSV de metricas

O script `tools/quality_metrics.orl` grava um arquivo `metrics-<run_id>.csv`.

Campos:

| Campo | Significado |
| --- | --- |
| `category` | Grupo: `baseline`, `security` ou `performance`. |
| `name` | Nome curto da suite. |
| `exit_code` | Codigo de saida do comando. |
| `passed` | `true` quando `exit_code == 0`. |
| `duration_ms` | Duracao total do processo em milissegundos. |
| `log_bytes` | Tamanho do log redirecionado para disco. |
| `log_path` | Caminho do log bruto. |

## Comparacao externa de performance

A comparacao com outras linguagens fica separada das suites internas.

Runner:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 5
```

Workloads:

```text
benchmarks/language-comparison/
```

Relatorio:

```text
docs/guides/language-comparison.md
```

O runner compara Ori, Rust, C, Python e Node.js em funcoes equivalentes:

- `fib`;
- `fib_work`;
- `sum_squares`;
- `list_push_sum`.

Regra de validade: uma execucao so entra no resumo comparativo quando a saida bate exatamente com a saida esperada.

## Regra de manutencao

Quando um crash, panic, vazamento, regressao de escape HTML ou lentidao real aparecer:

1. reduza o caso para um arquivo `.orl` pequeno;
2. adicione o caso em `security_robustness.rs` ou `performance_guard.rs`;
3. mantenha o diagnostico esperado explicito;
4. atualize este documento se o comando ou o budget mudar.
