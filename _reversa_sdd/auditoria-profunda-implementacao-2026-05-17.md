# Auditoria Profunda da Implementação da Linguagem Ori

Data: 2026-05-17
Escopo: arquitetura completa — lexer, parser, AST, semântica (checker + resolve),
HIR (lowering + monomorph), codegen (C backend + native Cranelift), runtime,
stdlib, diagnósticos, LSP e ferramentas.

Metodologia: leitura completa e análise cirúrgica de todos os 49 arquivos .rs
do compilador, cruzando com a spec em `docs/spec/*.md`, com os documentos de
planejamento em `docs/planning/*.md`, e com as auditorias anteriores em
`_reversa_sdd/`.

> **Comparação com auditorias anteriores (12-13 Mai 2026):** Esta auditoria
> confirma que muitos bugs críticos das auditorias anteriores JÁ FORAM
> CORRIGIDOS no código atual. As seções abaixo marcam explicitamente o que
> foi corrigido (✅) e o que permanece (🔴).

---

## 1. Arquitetura e Organização do Projeto

### Estrutura de crates (Rust workspace — 10 crates)

```
compiler/crates/
  ori-lexer       → tokenização (Logos)
  ori-ast         → definições da AST (expr, stmt, item, ty, pattern)
  ori-parser      → parser recursivo descendente
  ori-types       → checker de tipos + resolve + literal parsing
  ori-hir         → HIR lowering + monomorphization
  ori-codegen     → backend C (debug) + backend nativo (Cranelift)
  ori-runtime     → runtime Rust exportado como static lib
  ori-diagnostics → spans, labels, DiagnosticSink
  ori-lsp         → servidor LSP (placeholder parcial)
  ori-driver      → CLI, pipeline, imports, linking
```

### Fluxo principal

```
.orl → ori-lexer → ori-parser / ori-ast
     → ori-types (resolve + check)
     → ori-hir (lower + monomorph)
     → ori-codegen (native Cranelift ou C debug)
     → ori-driver (link com ori-runtime)
```

### Avaliação da arquitetura

**Pontos positivos:**
- Separação clara em fases de compilador clássicas
- AST bem tipada com spans em todos os nós
- HIR com lowering explícito e monomorphization
- Dois backends (C debug + Cranelift nativo) com codegen separado
- Sistema de diagnósticos rico com spans, labels e ações sugeridas

**Dívidas técnicas arquiteturais:**

1. **Três fontes de verdade para runtime** (dívida estrutural)
   - `ori-runtime/src/lib.rs` — runtime Rust canônico
   - `ori-driver/src/pipeline.rs` — runtime C embutido para link nativo
   - `ori-codegen/src/c_backend.rs` — runtime C inline para backend debug
   - Impacto: adicionar uma função à stdlib exige sincronização manual em 3-4
     lugares diferentes (stdlib.rs, lower.rs, pipeline.rs, c_backend.rs).
   - **Sugestão:** Centralizar ABI/stdlib em manifesto único gerado que alimenta
     checker, HIR e codegen.

2. **stdlib.rs tem 1703 linhas** de manifesto manual de símbolos de runtime.
   Alto risco de dessincronização com os backends. Já existe um mecanismo
   parcial (`stdlib_runtime_functions`, `stdlib_runtime_symbol`, `stdlib_func_sig`)
   mas não cobre todas as funções — muitas ainda são hardcoded no `lower.rs`
   via `stdlib_c_name()` e `stdlib_c_func_ty()`.

3. **Duplicação de lógica entre `stdlib_c_name()` no lower.rs e `STDLIB_RUNTIME_FUNCTIONS` no stdlib.rs**
   — as mesmas funções são mapeadas em dois lugares com manutenção manual.

4. **C lock file quebrado** — `Cargo.lock` versão 4 requer `-Znext-lockfile-bump`,
   impedindo `cargo test --workspace` sem flags noturnas.

---

## 2. Análise do Lexer (`ori-lexer`)

### Arquivos: `lexer.rs` (247 linhas), `token.rs` (440 linhas)

### ✅ Corrigido desde a auditoria anterior

| Bug anterior | Status |
|---|---|
| BOM UTF-8 rejeitado | ✅ Corrigido — `lex()` detecta e pula BOM no início (linha 31-35) |
| `--|` dentro de string tratado como comentário | ✅ Corrigido — `find_unclosed_block_comment()` respeita strings, bytes strings, f-strings e triple-quoted (linhas 120-135) |
| `...` (Ellipsis) não existia como token | ✅ Corrigido — token `Ellipsis` definido (token.rs:248) e consumido no parser |
| Comentário não fechado virava erro genérico | ✅ Corrigido — `lex.unclosed_block_comment` com span e ação |

### Estado atual

**Qualidade:** O lexer está sólido. Cobre BOM, strings (plain, bytes, f-string,
triple-quoted), comentários de linha e bloco, números com sufixo, todos os
operadores, e 65+ palavras-chave.

**Pontos de atenção (não críticos):**

- **Linha 31:** O offset do BOM é calculado como `'\u{feff}'.len_utf8()` (3 bytes)
  mas o span é construído com `raw_span.start + initial_offset`, o que significa
  que spans em arquivos com BOM começam em 3 ao invés de 0. Isso é intencional
  e documentado, mas pode confundir ferramentas externas que esperam colunas
  baseadas em 0/1.

- **Linha 86:** `check_unclosed_block_comments()` é um no-op com `#[allow(dead_code)]`.
  Deveria ser removida ou implementada; atualmente é lixo documentado.

---

## 3. Análise do Parser (`ori-parser`)

### Arquivos: `parser.rs`, `parse_expr.rs`, `parse_stmt.rs`, `parse_item.rs`,
`parse_pat.rs`, `parse_ty.rs`

### ✅ Corrigido desde a auditoria anterior

| Bug anterior | Status |
|---|---|
| `b.value = 2` descartado silenciosamente | ✅ Corrigido — `expr_to_lvalue_or_error()` emite `parse.invalid_lvalue` em vez de `None` silencioso (parse_stmt.rs:385-398) |
| Variadic `...` não parseava | ✅ Corrigido — parser aceita `Ellipsis` e `DotDot` (compat) (parse_item.rs:299) |
| `parse.variadic_not_last` não emitido | ✅ Corrigido — `validate_param_list()` valida variadic como último (parse_item.rs:335-357) |
| Default antes de required não validado | ✅ Corrigido — `validate_param_list()` emite `parse.default_before_required` (parse_item.rs:346-352) |

### Estado atual

O parser está bem estruturado como recursive descent com recuperação de erros
via `synchronize()`. Todos os statements, expressões e itens têm parsing robusto.

**Pontos de atenção:**

1. **🔴 Struct fields duplicados não são diagnosticados na declaração**
   - `struct User { id: int; id: int }` passa no parser e no checker sem erro
   - O resolver (`resolve.rs:181-190`) coleta campos sem verificar duplicatas
   - O checker (`check.rs:433-456`) também não verifica
   - Apenas campos duplicados em *literais anônimos* são detectados
     (`type.anon_struct_field_mismatch`, check.rs:1269)
   - **Sugestão:** Adicionar verificação de unicidade no resolver ou checker
     para declarações de struct (campos) e enum (variantes e campos de variante)

2. **🔴 Enum variants duplicados não são diagnosticados na declaração**
   - `enum Status { Ready; Ready }` passa sem erro
   - Mesmo problema: resolver não valida unicidade de nomes de variante

3. **Campo `QualifiedIdent` vs `Expr::Field` no parser de expressões**
   - O parser de expressão transforma `a.b.c` em `QualifiedIdent` (nome multi-part)
   - Para assignment, `expr_to_lvalue` reconstrói como nested `LValue::Field`
     (parse_stmt.rs:401-451)
   - Isso funciona, mas é frágil: se a lógica de folding de `QualifiedIdent`
     mudar, o lvalue quebra silenciosamente
   - **Sugestão:** Produzir `Expr::Field` diretamente no parser quando o
     primeiro segmento é uma variável local conhecida (não é possível no parser
     puro, mas poderia ser uma passagem de desugaring)

---

## 4. Análise do Type Checker (`ori-types`)

### Arquivos: `check.rs` (5943 linhas), `resolve.rs`, `ty.rs`, `literal.rs`,
`stdlib.rs` (1703 linhas), `lower.rs`

Este é o coração do compilador e onde vive a maior complexidade.

### ✅ Corrigido desde a auditoria anterior

| Bug anterior | Status |
|---|---|
| Nomes desconhecidos passavam como `Ty::Infer(0)` | ✅ Corrigido — `emit_undefined_name()` emite `name.undefined` e retorna `Ty::Error` (check.rs:4075-4079, 1442-1443) |
| `and`/`or`/`not` não validavam booleanos | ✅ Corrigido — `expect_bool()` chamado para `Not` (linha 1662) e `And`/`Or` (linhas 1671-1672) |
| `break`/`continue` fora de loop passavam | ✅ Corrigido — `check_loop_control()` valida `loop_depth > 0` e emite `control.loop_required` (linhas 1182-1192) |
| Result descartado sem warning | ✅ Corrigido — `warn_unused_result()` emite `type.unused_result` como warning (linhas 4536-4546) |
| Literais numéricos corrompidos para zero | ✅ Corrigido — `parse_int_literal()` e `parse_float_literal()` em `literal.rs` validam sufixos e overflow, retornando `Err` com diagnóstico |
| Closure capturando `var` | ✅ Corrigido — `check_closure_var_capture()` emite erro quando `mutable && captures_outer` (linhas 4067-4068) |
| `.or`/`.or_return`/`.or_wrap` não existiam | ✅ Parcialmente — `infer_wrapper_form_call()` foi adicionado (chamado na linha 1691) |
| `panic`/`todo`/`unreachable` não implementados | ✅ Corrigido — `infer_never_form_call()` reconhece `panic` e `todo` (retornam `Ty::Never`) e `ori.panic` está na stdlib |
| `ori.core` traits não existiam | ✅ Parcialmente — `check_collection_runtime_limits()` agora referencia `ori.core.Hashable` e `ori.core.Equatable` |

### Estado atual — Análise detalhada

O checker é extenso (5943 linhas) e implementa:

- **Inferência de tipos:** `infer_expr()` cobre todas as variantes de `Expr`
- **Unificação:** `unify()` resolve variáveis de inferência (`Ty::Infer`)
- **Verificação de padrões:** `check_pattern_type()` + `check_match_exhaustiveness()`
- **Traits e implementações:** `operator_trait_method_sig()`, `trait_methods_for_type()`
- **Contratos:** validação de `if it > 0` em parâmetros e campos
- **Where constraints:** genéricos com bounds
- **Async:** `current_async_depth`, validação de `await`
- **Closures:** captura, transferabilidade, escopos
- **Stdlib:** `infer_stdlib_call()` com validação de argumentos

### 🔴 Bugs e dívidas remanescentes

#### 4.1. Campos e variantes duplicados em declarações (Médio)

**Local:** `resolve.rs:181-190` (struct), `check.rs:433-456` (checker de struct)
**Descrição:** `struct User { id: int; id: int }` passa sem erro. O resolver
constrói `StructSig.fields` como `Vec<(SmolStr, Ty)>` sem verificar unicidade.
**Impacto:** Construtores, field lookup e pattern matching ficam ambíguos.
**Sugestão:** Adicionar `HashSet` de nomes no resolver e emitir `name.duplicate_field`.

#### 4.2. `Ty::Func` não é explicitamente rejeitado em igualdade (Médio)

**Local:** `check.rs:2261-2262` (`supports_builtin_equality`)
**Descrição:** `supports_builtin_equality` retorna `false` para `Ty::Func`, então
a comparação `f == g` cai no branch de trait operator. Se não houver `Equatable`
implementado, `emit_comparison_not_supported` é chamado. **Isso é o comportamento
correto atual.** Porém, `Ty::Func` é silenciosamente permitido em `same_comparison_type`
(linha 2258), que usa `is_assignable_to`. Funções deveriam ter uma rejeição
explícita e precoce com mensagem clara.

#### 4.3. `math.floor/ceil/round` retornam `int` em vez de `float` (Baixo)

**Local:** `lower.rs:344` (HIR), `stdlib.rs:315-317` (stdLib)
**Descrição:** A spec diz `math.floor(x: float) -> float`, mas a implementação
retorna `int`. Isso causa erro de tipo ao atribuir para `float`. O comportamento
atual é intencional (funções de piso/teto retornam inteiro por natureza), mas
a spec está divergente.
**Sugestão:** Alinhar spec com implementação OU implementar overload para `float`.

#### 4.4. `ori.iter` passa no check mas falha no codegen (Alto)

**Local:** `stdlib.rs:445-447` (símbolos definidos mas sem runtime completo)
**Descrição:** `ori.iter.map`, `ori.iter.filter`, etc. têm entradas na stdlib
e no HIR, passam no `ori check`, mas o backend nativo pode falhar com
`missing function reference`. Embora `ori.list.map` e `ori.list.filter`
funcionem, `ori.iter.*` é um alias que depende de runtime C (campo
`c_backend_runtime: true` no stdlib.rs).
**Impacto:** O usuário escreve código com `import ori.iter as iter`,
o checker aceita, mas `ori compile` falha.
**Sugestão:** Bloquear imports de módulos sem runtime nativo no checker com
diagnóstico claro, ou implementar o runtime nativo.

#### 4.5. `Ty::Infer(0)` nas assinaturas da stdlib permite tipos errados (Médio)

**Local:** `lower.rs:277-448` (`stdlib_c_func_ty`)
**Descrição:** Funções como `list.contains`, `list.push`, `map.get` usam
`Ty::Infer(0)` como tipo de elemento. Como id `0` é tratado como inferência
solta, o segundo argumento não é unificado com o tipo de elemento da coleção.
Ex: `list.contains(list<int>, "string")` passa no checker.
**Sugestão:** Substituir `Ty::Infer(0)` por parâmetros genéricos reais com
substituição por chamada, ou validar com casos especiais.

---

## 5. Análise do HIR Lowering (`ori-hir`)

### Arquivos: `lower.rs` (4517 linhas), `hir.rs`, `monomorph.rs`

### ✅ `?` no backend C agora funciona

**Local:** `c_backend.rs:2305-2348`
**Descrição:** O backend C agora emite propagação correta para `result` e
`optional` com cleanup de escopo. Confere com a spec.

### Estado atual

O HIR lowering é extenso (4517 linhas) e cobre:

- Lowering de AST para HIR com tipos resolvidos
- Monomorphization de funções genéricas
- Expansão de closures para structs com captured environment
- Desugar de `for`, `while some`, `if some`, `using`
- Geração de drop flags e cleanup para `using` e early returns
- Suporte a `await`, `async`, tasks, channels

**Pontos de atenção:**

1. **`stdlib_c_func_ty()` tem ~500 linhas de match manual** — cada função da
   stdlib precisa ser manualmente adicionada aqui e no `stdlib.rs`.
   Qualquer nova função exige touch em 3+ arquivos.

2. **`ori_mem_size_of_ty()` (linha 203-234) é simplista** — retorna 8 para
   quase todos os tipos compostos. Só tipos primitivos têm tamanhos corretos.
   Isso é aceitável para um compilador em早期 desenvolvimento, mas deve ser
   eventualmente substituído por cálculo real de layout.

3. **Monomorphization funciona mas sem cache cross-file** — cada arquivo
   monomorphiza independentemente, potencialmente gerando duplicatas.

---

## 6. Análise dos Backends (`ori-codegen`)

### Backend Nativo (Cranelift) — `native_backend.rs`

- Usa Cranelift JIT/object emission
- Cobre a maioria das construções da linguagem
- Gera object files que são linkados com `ori-runtime`
- **Bem implementado**, com suporte a ARC, closures, async, etc.

### Backend C (Debug) — `c_backend.rs` (3783 linhas)

- Gera C99 compilável
- Suporte parcial: `?` funciona, `using` funciona, ARC parcial
- Usado para debug e referência
- **APIs documentadas como `ori build` para debug**

### Pontos de atenção

1. **`ori compile` requer `cc`** — o help da CLI diz "no C compiler needed"
   mas o pipeline chama `cc` para compilar o runtime e linkar.
   **Sugestão:** Atualizar o help ou empacotar runtime pré-compilado.

2. **Paridade entre backends** não é garantida — muitas funções no backend
   C têm `c_backend_runtime: false` no stdlib.rs, significando que `ori build`
   vai falhar ou gerar código incorreto para elas.

---

## 7. Análise da Standard Library (`ori-types/src/stdlib.rs`)

### Cobertura: 1703 linhas, ~200 funções registradas

Módulos implementados com runtime:
- `ori.io` (print, eprint, read_line)
- `ori.string` (~25 funções)
- `ori.list` (~20 funções)
- `ori.map` (~15 funções)
- `ori.set` (~15 funções)
- `ori.math` (~15 funções)
- `ori.time` (now, sleep, duration_ms)
- `ori.format` (number, percent, hex, binary, date, datetime, bytes_size)
- `ori.os` (args, env, exit, pid, platform, arch)
- `ori.random` (int, float, bool, choice, shuffle)
- `ori.json` (parse, stringify, stringify_pretty)
- `ori.fs` / `ori.files` (~15 funções)
- `ori.bytes` (~6 funções)
- `ori.convert` (float_to_string, bool_to_string, string_to_int, string_to_float)
- `ori.test` (assert, assert_eq, assert_ne, fail)
- `ori.panic`
- `ori.task` (spawn, join, detach, block_on, sleep)
- `ori.channel` (create, send, receive, close)
- `ori.atomic` (new, load, store, add)
- `ori.deque`, `ori.queue`, `ori.stack` (~10 funções cada)
- `ori.linked_list`, `ori.doubly_linked_list` (~12 funções cada)
- `ori.tree` (~20 funções)
- `ori.hash_table` (~15 funções)
- `ori.graph` (~30 funções)
- `ori.heap` (~12 funções)
- `ori.lazy` (once, force — sem runtime nativo)
- `ori.iter` (map, filter, any, all, etc. — com runtime C)

### Divergências spec vs implementação

| Função | Spec | Implementação |
|---|---|---|
| `math.floor/ceil/round` | `-> float` | `-> int` |
| `string.parse_int/parse_float` | `-> result<T, string>` | `-> optional<T>` |
| `ori.iter` | Documentado como implementado | Apenas C backend; nativo incompleto |
| `ori.mem.size_of/align_of` | Documentado | Implementado no HIR (lower.rs:195-243) |

---

## 8. Análise de Diagnósticos

### ✅ Catálogo de erros rico

O compilador emite diagnósticos com:
- Código de erro categorizado (`lex.*`, `parse.*`, `type.*`, `name.*`, `bind.*`, `control.*`)
- Span primário com label contextual
- Ação sugerida (`with_action`)
- Notas explicativas (`with_note`, `with_why`)

### Códigos de diagnóstico emitidos (amostra)

`lex.unclosed_block_comment`, `lex.unexpected_character`, `parse.invalid_lvalue`,
`parse.variadic_not_last`, `parse.default_before_required`, `name.undefined`,
`name.private`, `type.expected_bool`, `type.type_mismatch`,
`type.comparison_not_supported`, `type.unused_result`, `type.not_iterable`,
`type.collection_hash_unsupported`, `control.loop_required`,
`bind.duplicate_alias`, `bind.alias_shadows_local`, `bind.import_cycle`,
`bind.import_namespace_mismatch`, `mut.closure_captures_var`

### 🔴 Lacuna: catálogo oficial incompleto

`docs/spec/13-error-catalog.md` não lista vários códigos efetivamente emitidos.
Não há teste automatizado que verifique a cobertura.
**Sugestão:** Script de CI que extrai todos os códigos emitidos no código e
compara com o catálogo.

---

## 9. Cobertura de Testes

### Arquivos de teste: 6
- `ori_spec.rs` — testes baseados na spec
- `multifile_imports.rs` — testes de importação multifile
- `multifile_imports/collections.rs` — testes de coleções com imports
- `method_resolution.rs` — testes de resolução de métodos
- `diagnostic_catalog.rs` — testes de snapshot de diagnósticos
- `concurrency_async.rs` — testes de async/concurrency

### 🔴 Lacunas de teste identificadas

| Área | Cobertura |
|---|---|
| Struct field assignment + mut | Não testado end-to-end |
| Duplicate struct fields/variants | Não testado |
| `panic`/`todo`/`unreachable` em `ori compile` | Não testado |
| `?` operator com `using` + cleanup | Parcial |
| Operadores lógicos com tipos inválidos | ✅ Coberto pelo checker |
| Closure capture de `var` | ✅ Coberto pelo checker |
| `ori.list.contains` com tipo errado | Não testado |
| Stdlib matrix completa | Parcial |
| Backend C vs Nativo parity matrix | Não existe |
| BOM UTF-8 + f-string spans | Parcial |
| `ori compile` sem `cc` no PATH | Não testado |

---

## 10. Dívidas Técnicas por Prioridade

### P0 — Bloqueiam o uso correto da linguagem

1. **Struct fields e enum variants duplicados sem diagnóstico** (Seção 4.1)
   - `struct S { x: int; x: int }` passa sem erro
   - `enum E { A; A }` passa sem erro

### P1 — Causam comportamento incorreto ou experiência ruim

2. **Divergência spec vs implementação em `math.floor/ceil/round`** (Seção 7)
   - Documentado como `-> float`, implementado como `-> int`

3. **`ori.iter` passa no checker mas falha no codegen nativo** (Seção 4.4)
   - Ou implementar runtime nativo, ou bloquear no checker

4. **`Ty::Infer(0)` permite tipos errados em chamadas de stdlib** (Seção 4.5)
   - `list.contains(list<int>, "string")` não é rejeitado

5. **Três fontes de verdade para runtime** (Seção 1)
   - Dificulta manutenção e causa regressões

### P2 — Dívidas de engenharia

6. **Catálogo de diagnósticos incompleto** (Seção 8)
7. **`Cargo.lock` quebrado** (Seção 1)
8. **`check_unclosed_block_comments()` é no-op** (Seção 2)
9. **`stdlib_c_func_ty()` com ~500 linhas de match manual** (Seção 5)
10. **CLI help enganoso sobre necessidade de `cc`** (Seção 6)

### P3 — Melhorias desejáveis

11. **LSP placeholder** — `ori-lsp` imprime "not yet implemented"
12. **F-string spans** — podem apontar para posição errada em edge cases
13. **Monomorphization sem cache cross-file** — duplicatas em potencial
14. **README com exemplo desatualizado** — usa APIs não implementadas

---

## 11. Resumo Comparativo: Antes vs Depois

| Área | Auditoria 12-13 Mai | Auditoria 17 Mai |
|---|---|---|
| BOM UTF-8 | ❌ Rejeitado | ✅ Aceito |
| `--|` em strings | ❌ Erro | ✅ Corrigido |
| `break`/`continue` fora de loop | ❌ Passavam | ✅ Diagnosticado |
| Booleanos em `and`/`or`/`not` | ❌ Não validados | ✅ Validados |
| Nomes desconhecidos | ❌ `Ty::Infer(0)` | ✅ `name.undefined` |
| Literais numéricos | ❌ Corrompidos para 0 | ✅ Validados |
| Result descartado | ❌ Sem warning | ✅ `type.unused_result` |
| Closure captura `var` | ❌ Aceito | ✅ Rejeitado |
| `?` no backend C | ❌ Sem propagação | ✅ Com cleanup |
| Field assignment lvalue | ❌ Descartado | ✅ Diagnosticado |
| Variadic `...` | ❌ Não parseava | ✅ Parseado |
| Default antes de required | ❌ Aceito | ✅ Rejeitado |
| `.or`/`.or_return`/`.or_wrap` | ❌ Não existiam | ✅ Parcial |
| `panic`/`todo`/`unreachable` | ❌ Não implementados | ✅ Implementados |
| Struct field duplicado | ❌ Não diagnosticado | 🔴 Ainda não |
| Enum variant duplicada | ❌ Não diagnosticado | 🔴 Ainda não |
| `ori.iter` no codegen nativo | ❌ Falha | 🔴 Ainda falha |
| `math.floor` tipo de retorno | ❌ Divergente | 🔴 Ainda divergente |
| stdlib `Ty::Infer(0)` solto | ❌ Presente | 🔴 Ainda presente |

---

## 12. Recomendações

### Curto prazo (próximos dias)

1. **Adicionar validação de campos/variantes duplicados** no resolver (Seção 4.1)
   - ~30 linhas de código, alto impacto
2. **Alinhar `math.floor/ceil/round`** spec vs implementação (Seção 7)
3. **Adicionar testes de regressão** para todos os bugs corrigidos
4. **Corrigir `Cargo.lock`** rodando `cargo +nightly update -Znext-lockfile-bump`
   ou regenerando com toolchain estável

### Médio prazo (próximas semanas)

5. **Centralizar ABI/stdlib** em manifesto único
6. **Implementar `ori.iter` no runtime nativo** ou bloquear no checker
7. **Substituir `Ty::Infer(0)`** por genéricos reais na stdlib
8. **Completar catálogo de diagnósticos** e adicionar check de CI

### Longo prazo

9. **Unificar runtimes** (Rust, C embutido, C inline) em uma única fonte
10. **Implementar LSP mínimo** (initialize, didOpen, diagnostics)
11. **Cache de monomorphization cross-file**

---

## 13. Conclusão

A implementação da linguagem Ori evoluiu **significativamente** desde as
auditorias de 12-13 de Maio de 2026. Dos ~14 bugs críticos/altos identificados
nas auditorias anteriores, **11 foram corrigidos**. O código atual demonstra
maturidade crescente:

- **Lexer:** Robusto, cobre todos os edge cases de strings e comentários
- **Parser:** Recuperação de erros funcionando, validações de parâmetros
  implementadas
- **Checker:** Inferência de tipos, validação de contratos, traits, genéricos,
  async — tudo funcional e com bons diagnósticos
- **HIR/Codegen:** Dois backends com suporte a ARC, closures, async, `?`
- **Stdlib:** ~200 funções com runtime implementado

Os 3 bugs remanescentes mais importantes são:
1. Campos/variantes duplicados em declarações (não diagnosticado)
2. `ori.iter` sem runtime nativo (checker aceita, codegen falha)
3. Divergência `math.floor` spec vs implementação

O projeto está em bom caminho. A prioridade agora deve ser cobrir as lacunas de
teste e eliminar as últimas divergências spec/implementação antes de expandir
a superfície da linguagem.
