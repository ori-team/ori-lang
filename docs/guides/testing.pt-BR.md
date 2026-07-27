> Manual de mantenedor (PT). **English (user + contributor summary):** [testing.md](testing.md)

# Manual de testes do Ori

Este manual mostra como executar os testes do projeto Ori.

## Como usuário da linguagem

```ori
module app.main

import ori.test = test

@test
adds()
    test.assert(1 + 1 == 2, "add")
end

main()
end
```

```bash
ori test main.orl
ori test main.orl --filter adds
```

## Como contribuinte do compilador

Use a raiz do repositorio e o workspace em `compiler/`:

```bash
cd /path/to/ori-lang/compiler
```

## Leitura rapida

Para validar o projeto inteiro:

```bash
cd compiler && cargo check --workspace
cargo test --workspace
```

Para coletar metricas de seguranca e performance:

```bash
cargo run -p ori-driver -- run tools/quality_metrics.orl
```

Os relatorios ficam em:

```text
target/ori-quality-metrics/
```

Para comparar Ori com Rust, C, Python e Node.js em workloads equivalentes:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 5
```

Os relatorios ficam em:

```text
target/language-comparison/
```

## Metricas em CSV e TXT

O script `tools/quality_metrics.orl` executa uma suite curada e salva:

- um CSV com status, tempo, exit code e tamanho dos logs;
- um TXT de resumo;
- um TXT bruto para cada comando executado.

Comando normal:

```bash
cargo run -p ori-driver -- run tools/quality_metrics.orl
```

Com budgets estritos de performance:

```bash
$env:ORI_PERF_STRICT="1"
cargo run -p ori-driver -- run tools/quality_metrics.orl
```

No Linux/macOS:

```bash
ORI_PERF_STRICT=1 cargo run -p ori-driver -- run tools/quality_metrics.orl
```

Campos do CSV:

| Campo | Significado |
| --- | --- |
| `category` | Grupo do teste: `baseline`, `security` ou `performance`. |
| `name` | Nome curto da suite. |
| `exit_code` | Codigo de saida do comando. `0` significa sucesso. |
| `passed` | `true` quando `exit_code == 0`. |
| `duration_ms` | Tempo total do processo em milissegundos. |
| `log_bytes` | Tamanho do log bruto redirecionado para disco. |
| `log_path` | Arquivo TXT com log bruto. |

## Gates principais

Use estes comandos antes de considerar uma mudanca pronta:

```bash
cd compiler && cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-driver --test security_robustness
cargo test -p ori-driver --test performance_guard
```

Para validar o probe pesado de runtime gerado:

```bash
cargo test -p ori-driver --test performance_guard -- --ignored
```

## Suites por crate

### Workspace completo

```bash
cargo test --workspace
```

Roda testes unitarios e integracao de todos os crates.

### Lexer, AST, Parser, HIR e Diagnostics

```bash
cargo test -p ori-lexer
cargo test -p ori-ast
cargo test -p ori-parser
cargo test -p ori-hir
cargo test -p ori-diagnostics
```

Use quando mudar tokens, AST, parser, HIR ou spans/diagnosticos.

### Type checker

```bash
cargo test -p ori-types
cargo test -p ori-types --lib stdlib
```

Use quando mudar tipos, stdlib manifest, constraints, generics ou diagnosticos semanticos.

### Codegen nativo e C debug

```bash
cargo test -p ori-codegen
```

Use quando mudar Cranelift, linker, JIT, C debug backend ou runtime symbols.

### Runtime

```bash
cargo test -p ori-runtime
```

Use quando mudar ARC, cycle collector, FFI, collections, filesystem, process, async executor ou test harness.

### Driver CLI

```bash
cargo test -p ori-driver
```

Use quando mudar pipeline, CLI, imports, doc export, fmt, summary, doctor ou execucao E2E.

Filtro de teste no binario `ori`:

```bash
ori test tests/minha_suite.orl --filter nome_do_teste
```

O filtro compara texto contra o nome completo (`app.modulo.test_nome`) e contra
o nome curto (`test_nome`). A saida mostra quantos testes foram descobertos e
quantos foram selecionados.

### LSP

```bash
cargo test -p ori-lsp
cargo test -p ori-lsp --test e2e
```

Use quando mudar hover, completion, diagnostics, goto definition, sync incremental ou extension protocol.

## Testes de integracao do `ori-driver`

Rode uma suite isolada quando a mudanca for focada.

| Suite | Comando | Quando usar |
| --- | --- | --- |
| `ori_spec` | `cargo test -p ori-driver --test ori_spec` | Sintaxe, tipos, statements, generics e contrato geral da linguagem. |
| `multifile_imports` | `cargo test -p ori-driver --test multifile_imports` | Imports, stdlib `.orl`, projetos multi-arquivo, C backend e features transversais. |
| `concurrency_async` | `cargo test -p ori-driver --test concurrency_async` | `async`, `await`, task, channel, atomic, cancelamento e formatter async. |
| `memory_arc` | `cargo test -p ori-driver --test memory_arc` | ARC, destrutores, cycle collector e leak-check. |
| `jit_run` | `cargo test -p ori-driver --test jit_run` | `ori run` via JIT e fallback default. |
| `diagnostic_catalog` | `cargo test -p ori-driver --test diagnostic_catalog` | Consistencia entre codigos emitidos e `docs/spec/13-error-catalog.md`. |
| `security_robustness` | `cargo test -p ori-driver --test security_robustness` | Corpus adversarial, spans validos, escaping HTML e leak smoke. |
| `performance_guard` | `cargo test -p ori-driver --test performance_guard` | Guardas leves de performance para check/fmt/doc/import graph. |
| `doctor` | `cargo test -p ori-driver --test doctor` | `ori doctor` e descoberta de runtime/stdlib/linker. |
| `doc_export` | `cargo test -p ori-driver --test doc_export` | `ori doc export` e JSON de referencia. |
| `explain` | `cargo test -p ori-driver --test explain` | `ori explain <code>`. |
| `summary` | `cargo test -p ori-driver --test summary` | `ori summary`. |
| `method_resolution` | `cargo test -p ori-driver --test method_resolution` | Resolucao de metodos. |
| `stdlib_fallback` | `cargo test -p ori-driver --test stdlib_fallback` | Descoberta de stdlib fora do layout principal. |

## Performance

Modo leve:

```bash
cargo test -p ori-driver --test performance_guard
```

Modo estrito:

```bash
$env:ORI_PERF_STRICT="1"
cargo test -p ori-driver --test performance_guard
```

Probe pesado:

```bash
$env:ORI_PERF_STRICT="1"
cargo test -p ori-driver --test performance_guard -- --ignored
```

Budgets configuraveis:

| Variavel | Default |
| --- | ---: |
| `ORI_PERF_CHECK_SINGLE_FILE_BUDGET_MS` | `2000` |
| `ORI_PERF_CHECK_IMPORT_GRAPH_BUDGET_MS` | `2500` |
| `ORI_PERF_FMT_SURFACE_BUDGET_MS` | `1500` |
| `ORI_PERF_DOC_SURFACE_BUDGET_MS` | `1500` |
| `ORI_PERF_RUNTIME_PROBE_BUDGET_MS` | `3500` |

## Comparacao com outras linguagens

Use o runner abaixo para executar funcoes equivalentes em Ori, Rust, C, Python e Node.js:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 5
```

Fontes dos workloads:

```text
benchmarks/language-comparison/
```

Relatorio de metodologia e resultados atuais:

```text
docs/guides/language-comparison.md
```

O runner valida a saida antes de comparar tempos. Se uma linguagem imprimir resultado diferente, a execucao fica registrada no CSV, mas nao entra como execucao valida.

## Segurança

Suite principal:

```bash
cargo test -p ori-driver --test security_robustness
```

Ela cobre:

- entradas malformadas que nao podem gerar panic;
- spans de diagnostico dentro dos arquivos fonte;
- codigos de diagnostico estaveis para regras semanticas;
- escaping de HTML gerado por `ori doc`;
- runtime nativo com `ORI_TEST_LEAK_CHECK=1`.

Para memoria/ARC em profundidade:

```bash
cargo test -p ori-driver --test memory_arc
```

Para async/concurrency:

```bash
cargo test -p ori-driver --test concurrency_async
```

## Runtime e release smoke

Windows:

```powershell
.\tools\stage_native_runtime.ps1
.\tools\smoke_native_release.ps1
```

Linux/macOS:

```bash
sh tools/stage_native_runtime.sh
sh tools/smoke_native_release.sh
```

Smoke sem rebuild, quando os artefatos ja foram gerados:

```powershell
.\tools\smoke_native_release.ps1 -SkipBuild
```

Gerar pacote depois do smoke:

```powershell
.\tools\package_native_release.ps1
```

Linux/macOS:

```bash
sh tools/package_native_release.sh
```

Os scripts de pacote chamam o smoke primeiro. Se `compile`, `test`, stdlib,
LSP ou JIT falharem no pacote isolado, o arquivo `.zip`/`.tar.gz` nao e gerado.

## VS Code extension

Smoke completo a partir da raiz:

```powershell
.\tools\smoke_vscode_extension.ps1
```

Esse smoke compila a extensao, valida os JSONs, roda o E2E do LSP e cria um
projeto temporario fora do repo para executar `check`, `run`, `test`, `fmt`,
`doc check` e `summary`.

Para validar apenas TypeScript:

```bash
cd extensions/vscode-orl
npm install
npm run compile
```

Volte para a raiz depois:

```bash
cd ..\..
```

## Testes de arquivos `.orl`

Checar um arquivo:

```bash
cargo run -p ori-driver -- check examples/hello
```

Rodar via `ori run`:

```bash
cargo run -p ori-driver -- run examples/hello
```

Compilar binario:

```bash
cargo run -p ori-driver -- compile examples/hello
```

Rodar testes declarados em `.orl`:

```bash
cargo run -p ori-driver -- test caminho/do/arquivo.orl
```

## Ordem recomendada por tipo de mudanca

Mudanca pequena de parser/checker:

```bash
cd compiler && cargo check --workspace
cargo test -p ori-driver --test ori_spec
cargo test -p ori-driver --test diagnostic_catalog
```

Mudanca em stdlib:

```bash
cargo test -p ori-types --lib stdlib
cargo test -p ori-driver --test multifile_imports
cargo test -p ori-driver --test diagnostic_catalog
```

Mudanca em runtime:

```bash
cargo test -p ori-runtime
cargo test -p ori-driver --test memory_arc
cargo test -p ori-driver --test concurrency_async
```

Mudanca em LSP:

```bash
cargo test -p ori-lsp --test e2e
cargo test -p ori-driver --test diagnostic_catalog
```

Mudanca antes de release:

```bash
cd compiler && cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- run tools/quality_metrics.orl
```

## Como ler falhas

1. Leia o primeiro erro real, nao a cascata inteira.
2. Se falhar `native.link_failed`, gere novamente o runtime com `tools/stage_native_runtime.ps1` ou `.sh`.
3. Se falhar catalogo, confira `docs/spec/13-error-catalog.md`.
4. Se falhar performance estrita, rode sem `ORI_PERF_STRICT` para separar lentidao real de variacao da maquina.
5. Se falhar JIT, rode tambem com `ORI_USE_AOT=1` para comparar JIT e AOT.

## Registro no changelog

Quando adicionar ou alterar testes relevantes:

1. atualize `CHANGELOG.md`;
2. atualize este manual se mudar comando, suite ou variavel;
3. atualize `docs/planning/security-performance-testing.md` se mudar metricas ou budgets.
4. atualize `docs/guides/language-comparison.md` se mudar workloads comparativos, linguagens ou metodologia.
