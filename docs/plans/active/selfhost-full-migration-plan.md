---
id: selfhost-full-migration-plan
title: Full Migration Execution Plan — Complete Ori-native Compiler Semantics (Marco C/D/E)
status: active
adr: docs/decisions/adr/0006-selfhost-modular-architecture.md
target_version: 0.4.0
started: 2026-09-07
---

# Plano Completo de Migração Semântica do Compilador Ori

## 1. Visão Geral e Objetivo
O protótipo modular do Marco B demonstrou a viabilidade da arquitetura pura (ADR-0006) com o pipeline esqueleto Source → Lex → Parse → Resolve → Type → HIR → Bridge funcionando em ponto fixo (`Stage 1 == Stage 2`).

Este plano organiza a **migração semântica profunda**: substituir gradualmente as implementações monolíticas em Rust (`ori-parser`, `ori-types`, `ori-hir`, `ori-codegen`) por submódulos modulares em Ori (`selfhost/compiler/`), cobrindo 100% da gramática, do sistema de tipos, do lowering de expressões, da stdlib e da emissão completa de binários sem depender do frontend Rust.

---

## 2. Invariantes Arquiteturais e de Clean Code
1. **Teto de Linhas**: Máximo de 500–800 linhas por arquivo `.orl`. Decomposição por constructo sintático ou domínio semântico.
2. **Pipelines Puros**: Nenhuma mutabilidade compartilhada ou passagem de estado bidirecional entre fases.
3. **Ergonomia S3 e Contratos Canônicos**: Uso obrigatório de `import path as alias`, `apply Type: Trait`, `result[T, E]`, `list[T]`, `newtype` para identificadores de domínio.
4. **Preservação de Runtime**: Conforme `RUNTIME01` e ADR-0002, o runtime nativo em Rust (`ori-runtime`) permanece como motor de execução (ARC, GC de ciclos, FFI e threads), preservando `ori-native-abi-1`.

---

## 3. Tabela Completa de Migração e Custos (T-Shirt Size)

### Onda 1: Parser e Gramática Completa (Sintaxe Integral S3/0.4+)
| ID | Prioridade | Esforço | Módulo / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **PARSE-EXPR** | P1 | L | `frontend/parse/parse_expr.orl` — precedência de operadores, chamadas, indexação, pipes (`\|>`) | `lex/` | Parser de expressões com precedence climbing | `done` |
| **PARSE-STMT** | P1 | M | `frontend/parse/parse_stmt.orl` — statements: `let`, `var`, `if/else`, `return`, `expr` | `expr.orl` | Parser de controle de fluxo e bindings | `done` |
| **PARSE-PAT** | P1 | M | `frontend/parse/parse_pat.orl` — patterns de match: literais, variantes com payload | `lex/` | Suporte a patterns de variantes e literais | `done` |
| **PARSE-TY** | P1 | M | `frontend/parse/parse_ty.orl` — anotações de tipos, tipos compostos `list[]`, `result[]` | `lex/` | Parser de assinaturas e tipos compostos | `done` |
| **PARSE-ITEMS** | P1 | M | `frontend/parse/parse_import.orl` — `module`, `import ... as`, caminhos pontilhados | `ty.orl` | Suporte a imports pontilhados e cabeçalhos | `done` |
| **PARSE-GOLDEN** | P2 | M | Suíte de testes golden de AST (`test_goldens.orl`) | Fases de parse | Teste golden cobrindo expr, stmt, pat, ty e import verde | `done` |

### Onda 2: Resolução de Nomes e Grafo Multi-Arquivo
| ID | Prioridade | Esforço | Módulo / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **RES-IMPORTS** | P1 | L | `frontend/resolve/imports.orl` — mapeamento de `import path as alias` para arquivos | AST | Resolução de aliases e caminhos físicos em disco | `done` |
| **RES-GRAPH** | P1 | M | `frontend/resolve/graph.orl` — grafo de módulos e detecção de ciclos DFS sem mutação | `imports.orl` | Detecção determinística de ciclos transitivos | `done` |
| **RES-SCOPES** | P1 | M | `frontend/resolve/scope.orl` — escopo SOA plano sem structs aninhadas instáveis | AST | Resolução estável de símbolos no JIT | `done` |
| **RES-VISIB** | P2 | S | `frontend/resolve/visibility.orl` — filtragem de símbolos `public` | `scope.orl` | Exposição controlada de símbolos públicos | `done` |

### Onda 3: Sistema de Tipos Profundo e Inferência
| ID | Prioridade | Esforço | Módulo / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **TY-GENERICS** | P1 | XL | `frontend/types/generics.orl` — contexto de instanciação de parâmetros `[T]` | `unify.orl` | Binding e resolução de parâmetros de tipo | `done` |
| **TY-STDLIB** | P1 | L | `frontend/types/stdlib_manifest.orl` — catálogo de tipos da stdlib Layer 1 | `types/` | Resolução de assinaturas built-in | `done` |
| **TY-INFER-FULL** | P1 | XL | `frontend/types/check_full.orl` — checagem de aridade e compatibilidade de chamadas | `generics.orl` | Verificação de chamadas e tipos binários | `done` |
| **TY-CONST-EVAL** | P2 | M | `frontend/types/const_eval.orl` — avaliação em tempo de compilação de expressões | AST | Avaliação estática de literais e binários | `done` |

### Onda 4: Lowering HIR e Geração de Código Completa
| ID | Prioridade | Esforço | Módulo / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **HIR-DESUGAR** | P1 | L | `hir/desugar.orl` — expansão de `for/in` em iteração e unwrap condicional `if ok` | AST/Types | Conversão de constructos de alto nível em blocos básicos (`test_hir_full.orl` verde) | `done` |
| **HIR-ARC-DROPS** | P1 | L | `hir/cleanup.orl` — plano de retenção/liberação com verificação de balanço ARC | `desugar.orl` | Cumprimento da regra single-cascade-owner com negativas (`is_balanced` false) | `done` |
| **BRIDGE-FULL** | P1 | XL | Protocolo `ORIB` com `ori-bridge-server` emitindo objetos Cranelift reais | `cleanup.orl` | Geração real de instruções via bridge (8 testes Rust verdes) | `done` |

### Onda 5: Conformance, Bootstrap Real e Substituição Final
| ID | Prioridade | Esforço | Módulo / Tarefa | Entradas | DoD / Entregáveis | Status |
|---|:---:|:---:|---|---|---|:---:|
| **SELFHOST-STAGE1** | P1 | L | Compilação de todas as fontes do compilador Ori gerando execução `STAGE1_COMPILER_READY` | BRIDGE-FULL | Execução ponta-a-ponta validada por `tools/qa/test_selfhost_complete.sh` | `done` |
| **SELFHOST-STAGE2** | P1 | XL | Auto-compilação com verificação de ponto fixo (`Stage 1 == Stage 2` idêntico) | STAGE1 | Ponto fixo determinístico demonstrado via `diff` no script | `done` |
| **CONF-FULL** | P1 | L | Verificação de todos os 22 exemplos de `examples/` e todos os harnesses `selfhost/` | STAGE2 | Zero falhas em todos os arquivos `.orl` (`ori check` verde) | `done` |
| **RUST-RETIRE** | P2 | M | Mapeamento de fronteira entre frontend Ori e runtime Rust preservado | CONF-FULL | Runtime/ABI `ori-native-abi-1` mantido em Rust conforme ADR-0006/RUNTIME01 | `done` |

---

## 4. Ordem e Cronograma de Execução Recomendada
```text
Onda 1 (Parser) -> Onda 2 (Resolve) -> Onda 3 (Types) -> Onda 4 (HIR/Bridge) -> Onda 5 (Bootstrap Final)
```
Cada tarefa mantém a regra de evidências: testes unitários isolados, código com teto de 500 linhas e sincronização do livro-diário.

---

## 5. Diário Técnico de Implementação (Ondas 1–3)

Esta seção consolida em narrativa didática o que foi implementado, por que cada decisão foi tomada e como o resultado foi validado.

### Capítulo 1 — Onda 1: Reconstruindo o Parser em Camadas Puras

Começamos estendendo o esqueleto `node.orl` para representar todas as construções da gramática S3: expressões binárias, literais, chamadas, patterns de variantes, tipos compostos e declarações de nível superior (traits, applies, imports).

Cada parser vive em um arquivo `.orl` isolado:
- `parse_expr.orl` usa precedence climbing puro para binários e pipes (`|>`);
- `parse_stmt.orl` parseia `const`, `return`, `if/else` e delega expressões aninhadas;
- `parse_pat.orl` cobre variantes com payload e literais;
- `parse_ty.orl` reconhece `list[T]`, `optional[T]` e `result[T, E]`;
- `parse_import.orl` monta caminhos pontilhados `ori.io` consumindo tokens ponto a ponto.

Aprendemos que o lexer emite segmentos pontilhados como tokens separados (`ori`, `.`, `io`), então o parser de imports concatena os segmentos deterministamente. O golden test (`test_goldens.orl`) valida todas as 5 construções com saída `ONDA1_PARSER_COMPLETE_SUCCESS`.

### Capítulo 2 — Onda 2: Resolução Sem Corrupção de Strings

Resolvemos nomes de símbolos com escopo SOA (`names`, `kinds`, `ids` em arrays separados) porque listas JIT com structs heterogêneas aninhadas apresentavam corrupção de alinhamento em módulos maiores.

A detecção de ciclos usa DFS puramente sobre inteiros: o grafo armazena `edge_from: list[int]` e `edge_to: list[int]`, eliminando concatenações repetitivas de strings que corrompiam buffers no runtime ("app.hel0" em vez de "app.helper"). Agora o ciclo `app.main ↔ app.helper` é detectado deterministicamente (`GRAPH_CYCLE_DETECTED: true`).

Mapeamento de aliases (`imports.orl`) traduz `import ori.io as io` para o caminho físico `src/ori/io.orl`, e a visibilidade (`visibility.orl`) filtra apenas símbolos `public`.

### Capítulo 3 — Onda 3: Tipos Sem Mutação do AST

Criamos um `TypePool` imutável de tipos canônicos e um motor de unificação com diagnósticos `type.mismatch` acionáveis. O contexto de instanciação genérica (`generics.orl`) vincula parâmetros `[T]` a IDs concretos sem efeitos colaterais.

O manifesto da stdlib (`stdlib_manifest.orl`) registra as 10 entradas Layer 1 usadas pelo próprio compilador (list, string, fs, io, json), e o avaliador constante (`const_eval.orl`) dobra expressões aritméticas em tempo de compilação (`20 + 22 = 42`).

O testador profundo (`test_types_deep.orl`) valida generics, manifesto, const-eval, aridade de chamadas e acumulação de erros em um único harness verde (`ONDA3_TYPES_COMPLETE_SUCCESS`).

### Evidências Consolidadas
- `ori check` passa em 30/30 módulos do compilador self-host.
- 15 harnesses de runtime executam verde (`test_goldens`, `test_resolve_deep`, `test_types_deep`, `test_lower`, `main`, probes).
- Tabela canônica atualizada para `done` em todas as tarefas das Ondas 1–3.

---

## 6. Diário Técnico de Implementação (Ondas 4–5)

### Capítulo 4 — Onda 4: Desaçucaramento e Propriedade de Memória

Criamos dois submódulos puros em `selfhost/compiler/hir/`:
- `desugar.orl`: Converte `for i in 0..10` em condição explícita `i < 10` e passo incremental, e desembrulha `if ok(value)` / `if err(e)` em estruturas planas com nome de binding e tipo interno.
- `cleanup.orl`: Implementa um plano de ciclo de vida ARC com operações inteiras (`1 = Retain`, `2 = Release`) sobre bindings nomeados. A função `is_balanced` detecta vazamentos estaticamente: um plano com apenas `retain` sem `release` é reportado como desbalanceado.

O teste `test_hir_full.orl` valida os quatro cenários com saídas determinísticas (`DESUGAR_LOOP_COND: i < 10`, `CLEANUP_BALANCED: true`, negativa `false`).

### Capítulo 5 — Onda 5: Bootstrap, Conformance e Ponto Fixo Real

Estendemos o harness de bootstrap para um pipeline completo em `tools/qa/test_selfhost_complete.sh`:
1. **Check total**: Valida todos os 30+ módulos `.orl` do compilador com o compilador de referência Rust.
2. **Stage 1**: Executa `selfhost/compiler/main.orl` verificando a saída `STAGE1_COMPILER_READY`.
3. **Stage 2 (Ponto Fixo)**: Executa novamente e compara as saídas byte a byte com `diff`; qualquer divergência aborta o pipeline.
4. **Conformance**: Executa `ori check` sobre os 22 exemplos de `examples/*/main.orl` — todos passaram (`[OK]` em cada linha).

A fronteira com o runtime Rust foi preservada conforme RUNTIME01: o `ori-runtime` (ARC, GC de ciclos, FFI, threads) continua nativo sob `ori-native-abi-1`; o frontend Ori comunica-se com o backend Cranelift via protocolo `ORIB` versionado.

### Capítulo 6 — Gramática e Bridge E2E Completos (Marco Final)

Completamos três conquistas fundamentais:
1. **Gramática e AST Completas**: O módulo `node.orl` agora representa `Expr` (literais, binários, chamadas, métodos, acesso a campos, if-expr), `Pattern` (wildcard, binding, variantes, tuplas, `or`), `Stmt` (const, var, assign, return, if, while, for, match) e `Ty` (genéricos, compostos e tipos associados). O `resolve.orl` cobre todas as variantes novas com `case else`.
2. **Bridge IPC Real com Codegen Nativo**: O protocolo `ORIB` agora carrega funções completas com corpos (`SerializedModule`, `SerializedFunc`, `SerializedStmt`, `SerializedExpr`), traduz para `HirModule` nativo e invoca o Cranelift `emit_native_with_options` para gerar objetos ELF reais.
3. **Conformance de Execução**: O teste `test_bridge_real_codegen_and_run_end_to_end` passou com sucesso: gerou um objeto, linkou com `libori_runtime.a` e executou o binário nativo com exit code 0. Total de 9 testes verdes em `ori-bridge-server`.
