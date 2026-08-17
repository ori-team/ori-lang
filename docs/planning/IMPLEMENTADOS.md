# Recursos Implementados e Resolvidos — Ori Language

> **Registro histórico:** esta é uma cronologia de entregas, não a fonte atual
> de status. Para o inventário vigente, consulte [`../ATLAS.md`](../ATLAS.md) e
> [`BACKLOG.md`](BACKLOG.md).

Este documento registra o estado atual de implementação da linguagem Ori (conforme verificado pelas suítes de testes automatizados e especificações normativas).

---

## 1. Estrutura Léxica e Sintática (Lexer & Parser)

- **Comentários**: Suporte a comentários de linha (`--`) e blocos de comentários reais (`--| ... |--`), com tratamento robusto para detectar blocos não fechados (`lex.unclosed_block_comment`).
- **Literais e Strings**:
  - Inteiros e floats com e sem sufixos específicos.
  - Strings simples (`"..."`), byte strings (`b"..."`), f-strings interpoladas (`f"..."`).
  - Triple strings multi-linha (`"""..."""`) com desindentação automática baseada na margem esquerda (*baseline stripping*) em runtime, tanto para strings comuns quanto para f-strings interpoladas.
  - Validação de escapes unicode em bytes string (rejeição de `\u{...}` no parser).
- **Identificadores**: Permite identificadores unicode e palavras contextualizadas (como o uso de `times` como identificador local fora de contextos especiais).
- **Atributos**: Parsing e validação de atributos em declarações de alto nível (ex. `@deprecated`, `@test`). `@cfg` possui predicados estruturados, features de manifesto e filtra itens antes da resolução (LANG-CFG-1, 2026-08-10).

---

## 2. Namespaces e Importações (Imports)

- **Carregamento de Múltiplos Arquivos**: Resolução de módulos locais recursivos e transitivos a partir do caminho raiz descoberto (`DefMap`).
- **Aliases de Importação**:
  - Alias explícito: `import app.util as util`.
  - Alias implícito (último segmento): `import app.util` (disponibiliza `util`).
- **Resolução de Conflitos**: Detecção e diagnóstico claro para imports duplicados, conflitos de nomes locais vs. importados e caminhos circulares (`project.circular_import`).
- **Visibilidade**: Controle de visibilidade `pub` (público) e `priv` (privado) em imports de símbolos entre arquivos.

---

## 3. Sistema de Tipos e Verificação Semântica (Type Checker)

- **Tipos Primitivos**: `int`, `float`, `bool`, `string`, `bytes`, `void`, `never`.
- **Tipos Estruturados**: Structs, enums (com payloads nomeados), tuplas, tipo `optional<T>` e `result<T, E>`.
- **Inferência e Unificação de Tipos**: Algoritmo completo de inferência com unificação para tipos genéricos e resolução monomórfica.
- **Checagem de Funções e Métodos**:
  - Parâmetros posicionais, parâmetros com valores default (inseridos no call-site) e parâmetros variádicos (`Type...` posicionados no final da assinatura).
  - Parâmetros nomeados/rotulados.
  - Contratos em parâmetros de funções e campos de structs (`if it > 0`).
  - Métodos inerentes e métodos de traits, suportando o receptor implícito `self` e explícito.
  - Detecção e erro em recursões infinitas de instanciações genéricas (`generic.circular_instantiation`).

---

## 4. Traits, Operadores e Resolução de Métodos

- **Traits**: Definição e implementação de traits, incluindo métodos default e constraints genéricas (`where T is Trait`).
- **Restrições Negativas**: Validação de constraints negativas no type-checker (`where T is not Trait`).
- **Resolução de Métodos e Ambiguidade**:
  - Múltiplas implementações do mesmo nome de método para traits distintos são aceitas.
  - Chamadas de método ambíguas e diretas são rejeitadas (`type.ambiguous_method`).
  - Suporte a chamadas qualificadas explícitas (`Trait.metodo(valor)`).
- **Sobrecarga de Operadores**: Sobrecarga via traits nativos (`ori.core`) para operadores aritméticos e relacionais (`+`, `-`, `==`, `!=`, `<`, `<=`, `>`, `>=`).
- **Igualdade Estrutural (== / !=)**:
  - Implementado para tipos primitivos, `bytes`, `list<T>`, `optional<T>`, `result<T, E>`, tuplas, structs concretas e structs genéricas (com substituição correta de parâmetros genéricos nos campos em tempo de compilação).
  - Implementado suporte para comparação estrutural avançada em mapas (`map<K,V>`) e conjuntos (`set<T>`) cujos elementos/chaves suportam igualdade (seja via trait `Equatable` ou via igualdade estrutural).
- **Displayable**: Conversão dirigida por trait para strings (`string(value)`) e f-strings.

---

## 5. Lowering (HIR) e Compilação

- **HIR Lowering**: Desaçucaramento completo de construções como operador pipe (`|>`), atualização de structs (`x with { field: v } end`), expressões `is`, indexadores e f-strings em chamadas de runtime correspondentes.
- **using**: Lowering de blocos `using` para injeção automática de chamadas `Disposable.dispose()` na saída do escopo, incluindo caminhos de retorno normal, retornos antecipados (`?`) e panics.
- **Backend Nativo (Cranelift)**: Rota principal de compilação sem dependência direta de toolchain C. Gera binários nativos executando linkagem própria via `NativeLinker` e empacotamento do runtime para Linux e Windows.
- **Backend C (Debug/Transpile)**: Mantido apenas como rota de depuração e transpilação com paridade parcial. Rejeita ativamente features não suportadas de concorrência ou async com diagnósticos claros.

---

## 6. Runtime Nativo e Biblioteca Padrão (Stdlib)

- **Runtime de Referência**: `ori-runtime` implementado em Rust. Gerencia o modelo de memória de contagem de referências atômica (ARC) e bounds-checking para coleções, strings e bytes.
- **Módulos Centrais da Stdlib**:
  - `ori.core`: Declaração de traits essenciais (`Displayable`, `Equatable`, `Comparable`, `Hashable`, `Disposable`, `Default`, `Error`, `Cloneable`).
  - `ori.io`: Operações de entrada e saída, incluindo `print` e `read_line`.
  - `ori.fs`: Leitura e escrita de texto/bytes síncronas e assíncronas (`read_text_async`, `write_text_async`), além de utilitários como `exists`, `delete`, `list_dir`, `create_dir`, etc.
  - `ori.string`: Manipulação e conversão de strings (`trim_start`, `trim_end`, `index_of`, `join`, `repeat`, `pad_left`, `pad_right`, `parse_int`, `parse_float`).
  - `ori.bytes`: Manipulação de bytes crus.
  - `ori.math`: Funções trigonométricas, exponenciais e constantes matemáticas (`pi`, `e`, `nan`, etc.).
  - `ori.convert`: Conversão explícita entre floats, strings, ints e bools.
  - `ori.time` / `ori.os` / `ori.random` / `ori.lazy` / `ori.test`.
  - `ori.iter`: Operações *eager* sobre listas (`map`, `filter`, `any`, `all`, `reduce`, `find`, `zip`, etc.).

---

## 7. Estruturas de Dados Avançadas (Collections v1)

Todas as coleções listadas abaixo estão integradas com o type-checker, possuem ABI estável no runtime nativo e tratamento seguro de valores vazios/nulos com `optional`:

- `ori.list`: Operações de ordenação, fatiamento e manipulação direta.
- `ori.map` / `ori.set`: Implementações de tabelas hash com chaves do tipo `int`, `string` ou customizados com `Hashable + Equatable`.
- `ori.deque` / `ori.queue` / `ori.stack`: Estruturas de dados lineares otimizadas e implementadas sobre buffers circulares no runtime nativo (via `VecDeque`).
- `ori.linked_list` / `ori.doubly_linked_list`: Listas encadeadas expostas através de cursores por posição (`insert_after`, `insert_before`, `remove_at`), evitando vazamentos de ciclos ARC internos.
- `ori.tree`: Árvores gerais baseadas em arenas com `NodeId` e suporte a travessias e reparenting de subárvores.
- `ori.hash_table`: API de tabela hash explícita voltada a performance (capacidade e reservas de memória).
- `ori.graph`: Implementação de grafos direcionados e não-direcionados, cobrindo caminhos sem peso (BFS/DFS), arestas ponderadas, caminhos de custo mínimo (Dijkstra) e ordenação topológica.
- `ori.heap`: Min-heap com ordenação dirigida por `Comparable`.

---

## 8. Concorrência e Async (v1)

- **Concorrência**:
  - Trait marcador `Transferable` validado no checker para tipos que podem cruzar tasks/canais.
  - Spawning e joining de tarefas em threads nativas (`ori.task`).
  - Canais de comunicação sincronizados e seguros para concorrência (`ori.channel`).
  - Inteiros atômicos (`ori.atomic`).
- **Async/Await**:
  - Tipo primitivo `future<T>` integrado.
  - Executor nativo em thread dedicada com timers não-bloqueantes (`task.sleep`).
  - Sintaxe de funções `async` e expressão `await` controladas no parser, HIR e type checker.
  - Entrada assíncrona principal: `async main()`.
  - **State Machine Async (v1)**: Geração nativa de frame e state machine com despacho por estado para fluxos sequenciais. Preserva a contagem de referências ARC para locals vivos através de suspensões sequenciais simples.

---

## 9. Ferramentas, CLI e LSP

- **Comandos do CLI**:
  - `ori check` (verificação estática rápida).
  - `ori compile` (compilação para executável nativo).
  - `ori run` (atalho para compilar e rodar arquivo temporário).
  - `ori test` (execução automática de suítes de testes `@test`).
  - `ori fmt` (preserva indentação e regras de formatação).
  - `ori doc` (extração de documentação).
- **LSP (`ori-lsp`)**:
  - Emissão de diagnósticos de parser e checker por arquivo.
  - Preenchimento automático (*autocomplete*) para stdlib.
  - Hover semântico detalhado mostrando assinaturas de funções, tipos de variáveis, campos de structs e restrições de contratos.
  - Navegação de ir para a definição (*go-to-definition*) para escopos locais.

---

## 11. Etapa 2 — Sistema de Tipos Avançado (2026-06)

- **Igualdade dinâmica (`any<Trait>`)**: `==` / `!=` via vtable lookup no runtime nativo quando o trait constraint permite igualdade.
- **Associated types**: declaração `type Item` em traits; validação no checker e resolução na monomorfização.
- **Const generics**: parâmetros genéricos de constante (ex.: tamanhos fixos) aceitos pelo parser e checker.
- **Higher-kinded types (HKT)**: tipos genéricos parametrizados por construtores de tipo, com constraints avançadas.
- **Igualdade em coleções opacas**: `deque`, `queue`, `stack`, `linked_list`, etc. comparáveis quando elementos suportam `Equatable`.
- **Propagação estática de traits**: `list<T> is Equatable` somente se `T is Equatable`.
- **Iteradores lazy**: interface lazy para estruturas opacas sem snapshot completo (`to_list()`).
- **JSON estruturado**: `json.Value` como enum recursivo (`Null`, `Bool`, `Number`, `String`, `Array`, `Object`) com parse/stringify nativos.

---

## 12. Bugs e Ajustes Críticos Resolvidos (Fase 0)

- **Bug 0.1 (Heap)**: Resolvido o problema de ordenação incorreta de elementos no custom min-heap devido ao registro tardio de arestas ARC (`ori_arc_register_edge` invocado corretamente antes de `heap_push_raw`).
- **Bug 0.2 (Iterable)**: Corrigido o avanço incorreto de iteradores customizados em loops `for` nativos (injetando retenção e liberação corretas no cabeçalho do loop).
- **Bug 0.3 (Git)**: Limpeza de objetos inacessíveis no repositório executada com sucesso.

---

## 13. Release v0.2.0 — Snapshot (2026-06-29)

Snapshot completo do que entra na release `v0.2.0` (Etapa 9 do `PLANO-MATURIDADE-COMPLETO.md`). Esta release consolida as Etapas 0–8 (estabilização do workspace, features bloqueadoras, sistema de tipos avançado, sync documental normativa, dívida técnica do compilador, runtime/ARC, LSP semântico cross-file, catálogo de diagnósticos auditado, organização/infra/qualidade).

### Componentes versionados

- **Workspace version:** `0.2.0` (`Cargo.toml [workspace.package]`); todos os 10 crates usam `version.workspace = true`.
- **Runtime ABI (M3):** `ori-native-abi-1` (`ORI_ABI_VERSION` em `ori-runtime/src/lib.rs`); contrato normativo em `docs/spec/19-abi.md` (layouts, ARC header, mangling `ORI__*`, link versioning).
- **Rust toolchain:** `1.95.0` via `rust-toolchain.toml` (canal + `rustfmt` + `clippy`).

### Binários de release (Windows MSVC, build desta release)

| Artefato | Profile | Tamanho |
|----------|---------|---------|
| `ori.exe` (compilador) | release | ~9.65 MB |
| `ori-lsp.exe` (LSP server) | release | ~11.83 MB |
| `ori_runtime.lib` (Win MSVC) | release | ~12.76 MB |

### Validação de release

- **Smoke de package:** `tools/smoke_native_release.ps1 -SkipBuild` passa com `ORI_REQUIRE_PACKAGED_RUNTIME=1` — `ori compile` (hello_world.orl + async_demo.orl) e `ori test` (package_smoke_test.orl com `@test` sync + async) validados em package isolado.
- **Testes workspace:** `cargo test --workspace` verde no snapshot de release. Em `[Unreleased]`, o ignore de `await` em loops aninhados foi removido; permanecem apenas probes pesados intencionais (`cycle_stress` e `performance_guard` estrito).
- **Catálogo de diagnósticos:** `cargo test -p ori-driver --test diagnostic_catalog` verde (consistência bidirecional emitted×catalog + guarda contra reintrodução de códigos removidos).
- **LSP E2E:** `cargo test -p ori-lsp` verde — 8 testes E2E + testes unitários (cross-file goto-def, type-aware dot completion, cross-file find-references, circular import diagnostic, formatting idempotency).

### CI

- **Pipeline:** `native-route.yml` define jobs para 5 triples: windows-msvc, windows-gnu, linux-gnu, macos-x86_64, macos-aarch64. Execução no CI remoto pendente de push (não executado automaticamente nesta release).

### Known Issues (não bloqueadoras, documentadas)

1. **Async em loops aninhados:** resolvido em `[Unreleased]`; `compile_runs_async_await_in_deeply_nested_bodies_native` roda na suite normal.
2. **Formatter:** bug de `trait` corrigido em `[Unreleased]`; assinaturas obrigatórias não abrem corpo e métodos default continuam indentados.

### Backlog v2 (não entra nesta release)

- Stdlib escrita em `.orl` com bootstrap gradual (hoje embutida em Rust manifesto `STDLIB_RUNTIME_FUNCTIONS`).
- Backend C mantido como rota síncrona parcial de debug, sem paridade
  async/concurrency por decisão de 2026-07-23.
- Suporte a triples adicionais além dos 5 do CI.
- Publicação em registry / instalador.
- `ori doc` com output HTML estático.
- Bundle `rust-lld` no release package para eliminar dependência de Rust toolchain no `ori compile` (ver discussão em `PLANO-MATURIDADE-COMPLETO.md` Etapa 9 + resposta de arquitetura sobre "transpiler vs compilador").
