# PR Plan — Ori Surface S3 (Auk9-inspired)

**Origem (ADR):** `docs/planning/adr-ori-surface-s3-auk9.md`  
**Registro de decisões:** `docs/planning/ori-surface-s3-auk9.md`  
**Data:** 2026-07-12  
**Status:** PRs 1–10 ✅ (`0.3.0`) · PR 11 ✅ (`0.3.1`) · **opção B** ✅ (campo/index/call/pipe)  
**Release alvo:** `0.3.0` · `0.3.1` · B em `[Unreleased]` / superfície viva

---

## Resumo

Implementar a superfície S3 da Ori com **corte seco** no artefato `0.3.0`:
lexer/parser/checker/fmt/LSP grammar, migração de fontes do **repo ori-lang**
(stdlib, tests, examples), reforma documental, script de migração.

**Pós-0.3.0 (entregue):** inferência Nim-local (`0.3.1`) + opção B; pipe `|>`
**mantido** na Ori. **`ori-game` / `ori-imgui`:** cancelados / fora do produto.

**Skills:** `compiler-dev`, `lang-compiled` (se tocar codegen só por desugar),
`ori-testing`, `living-docs`, `clean-code`, `rust`.

**Fonte de verdade de sintaxe:** `ori-surface-s3-auk9.md` + ADR. Em dúvida,
prevalece o registro vivo.

---

## PR Plan

### PR 1: Lock-in docs — ADR, manifesto skeleton, catálogo de erros S3

**Description:** Congela o desenho no repo. Cria/atualiza
`docs/spec/00-manifesto.md` (propósito ND + estudo + IA; identidade S3).
Garante ADR e `ori-surface-s3-auk9.md` referenciados no `docs/planning/README.md`.
Registra no `docs/spec/13-error-catalog.md` códigos novos/alterados previstos
(ex.: `parse.poetic_call_nested`, `parse.end_label_mismatch`, erros de forma
removida se ainda não existirem). **Não** remove ainda a sintaxe antiga do
compiler — só docs/catálogo.

**Files/components affected:** `docs/planning/adr-ori-surface-s3-auk9.md`, `docs/planning/ori-surface-s3-auk9.md`, `docs/planning/pr-plan-ori-surface-s3.md`, `docs/planning/README.md`, `docs/spec/00-manifesto.md`, `docs/spec/13-error-catalog.md`, `docs/spec/README.md`

**Dependencies:** None

---

### PR 2: Superfície de arquivo — `module`, sem `func`

**Description:** Lexer/parser: `module` substitui `namespace` (erro em
`namespace`). Remover keyword `func` de declarações (função, trait method,
apply, async: `async name(...)`). Manter regras de omissão de `->` como a Ori
hoje (decisão 1.3). Atualizar testes de parser/`ori_spec` mínimos para a nova
forma; fixtures que ainda usam forma antiga podem ser migradas parcialmente
neste PR se bloquearem o compile dos testes do crate. Formatter/LSP keywords
básicas.

**Files/components affected:** `compiler/crates/ori-lexer/`, `compiler/crates/ori-parser/`, `compiler/crates/ori-ast/` (se necessário), `compiler/crates/ori-driver/src/pipeline/fmt.rs` (ou fmt module), `compiler/crates/ori-driver/tests/ori_spec.rs`, `extensions/vscode-orl/` (keywords)

**Dependencies:** None

---

### PR 3: Gramática de tipos — só `[]`, remover `of` / `<>`

**Description:** Tipos compostos canônicos: `list[T]`, `map[K,V]`,
`optional[T]`, `result[T,E]`, genéricos de usuário `Nome[T]`. Remover `of` /
`map of K to V` / angulares `<>` na superfície. Bounds de genéricos na forma
Auk9: `for T: Trait` (em vez de `where T is` / `func foo<T>` canônico).
Testes de parse/type negativos para formas antigas.

**Files/components affected:** `compiler/crates/ori-parser/src/parse_ty.rs`, `compiler/crates/ori-lexer/`, `compiler/crates/ori-types/`, `compiler/crates/ori-driver/tests/`

**Dependencies:** PR 2

---

### PR 4: Fluxo — só `try`, `elif`, match enum sem ponto no case

**Description:** Remover `expr?` (erro). Aceitar só `try expr`. Substituir
`else if` por `elif` (erro em `else if`). Patterns de enum no `match`:
`case Variant:` / `case Variant(...):` sem ponto obrigatório (3.4). Manter
if-expressão Ori `if cond then a else b` (3.3 B). Testes de regressão.

**Files/components affected:** `compiler/crates/ori-parser/`, `compiler/crates/ori-lexer/`, `compiler/crates/ori-types/`, `compiler/crates/ori-driver/tests/`

**Dependencies:** PR 2

---

### PR 5: Literais — struct `{…}` / `Type {…}`, map, enum, list

**Description:** Struct: apenas `{ field: v }` e `Type { field: v }`; remover
`Type(...)`, `.{…}`, construção guiada `(…)`. Map: `{ "k": v }`; disambiguação
ident vs literal antes de `:`. Enum fora do match: `Enum.Var` / `.Var(...)`.
List: manter `[…]`. Atualizar lower/checker se AST de literal mudar.
Testes de parse e compile+run mínimos.

**Files/components affected:** `compiler/crates/ori-parser/src/parse_expr.rs`, `compiler/crates/ori-ast/`, `compiler/crates/ori-hir/`, `compiler/crates/ori-types/`, `compiler/crates/ori-driver/tests/`

**Dependencies:** PR 3

---

### PR 6: Imports S3 — três formas, bloco com vírgulas, `pub import`

**Description:** Formas: (1) `import path (A, B)` seletivo; (2)
`import path = alias` (path à esquerda — **não** ordem Auk9); (3) `import path`
só caminho completo. Remover `only` e `as`. Bloco `imports … end` com as três
formas; múltiplas entradas por linha separadas por vírgula **somente** no
bloco. Manter `pub import`. Atualizar resolver/pipeline de imports e testes
`multifile_imports`.

**Files/components affected:** `compiler/crates/ori-parser/`, `compiler/crates/ori-types/src/resolve.rs`, `compiler/crates/ori-driver/src/pipeline.rs`, `compiler/crates/ori-driver/tests/multifile_imports.rs`

**Dependencies:** PR 2

---

### PR 7: Traits — `apply Type` + `use Trait` + bind `slot = fn`

**Description:** Substituir `apply Trait to Type` por `apply Type` com seções
`use Trait`. Permitir corpo inline e bind `compare = comparePoints`. Defaults
de trait = método **com corpo** (sem keyword `default`). Ordem fixa no apply:
soltos → `use` sections. `self` sem tipo ok. Remover forma antiga (erro).
Atualizar method resolution / checker / testes `method_resolution`.

**Files/components affected:** `compiler/crates/ori-parser/`, `compiler/crates/ori-ast/`, `compiler/crates/ori-types/`, `compiler/crates/ori-hir/`, `compiler/crates/ori-driver/tests/method_resolution.rs`

**Dependencies:** PR 2, PR 3

---

### PR 8: Ritmo — `=>`, poetic call, `end` rotulado, closures `(u)=>`

**Description:** Corpos `nome(params) -> T => expr`. Closures canônicas
`(params) => expr` e bloco `(params) … end` (sem `do`/`fn`/`given`). Chamada
poética: um argumento na mesma linha; válido `print greet("hello")`; erro
`parse.poetic_call_nested` em poetic aninhada. `end if` / `end match` / …
opcionais com `parse.end_label_mismatch`. Formatter + grammar VS Code.
Testes ori_spec + concurrency se tocar async surface só na casca.

**Files/components affected:** `compiler/crates/ori-parser/`, `compiler/crates/ori-ast/`, `compiler/crates/ori-types/`, `compiler/crates/ori-hir/` (se necessário), `compiler/crates/ori-driver/`, `extensions/vscode-orl/`

**Dependencies:** PR 2, PR 4

---

### PR 9: Script `ori migrate-syntax` + migração em massa `.orl` do repo

**Description:** Implementar ferramenta de migração melhor-esforço (CLI
`ori migrate-syntax` ou `tools/migrate_syntax.*`) cobrindo rewrites mecânicos:
`namespace`→`module`, strip `func`, `as`→`path = alias`, `only`→`(…)`,
`<>`→`[]`, `else if`→`elif`, `?`→`try` onde seguro, etc. Aplicar ao
`stdlib/**/*.orl`, `examples/**/*.orl`, fixtures em
`compiler/crates/ori-driver/tests/`, `tests/*.orl` se houver. Ajustes manuais
para `apply Trait to Type` → `apply Type`/`use` onde o script não der conta.
`cargo test -p ori-driver` e subset workspace relevantes verdes.
`ori-game`/`ori-imgui` **fora do produto** (não migrar; removidos).

**Files/components affected:** `compiler/crates/ori-driver/` (CLI migrate), `tools/`, `stdlib/`, `examples/`, `compiler/crates/ori-driver/tests/`, `tests/`

**Dependencies:** PR 3, PR 4, PR 5, PR 6, PR 7, PR 8

---

### PR 10: Reforma documental 0.3.0 + CHANGELOG breaking

**Description:** Adequar documentação ativa ao S3: `docs/spec/01-overview.md`
e capítulos de sintaxe/tipos/traits/errors afetados; merge/apagar docs
depreciados; guias de migração 0.2→0.3; atualizar README(s); entrada
`CHANGELOG.md` `[0.3.0]` com lista breaking. Checklist 9.5. Nota sobre Auk9
lab. **Não** bump de versão Cargo se a política for só documentar até o
release tag — ou bump workspace para `0.3.0` se o projeto já versiona junto
(seguir convenção do repo / AGENTS).

**Files/components affected:** `docs/`, `README.md`, `README.pt-BR.md`, `README.ja.md`, `CHANGELOG.md`, `AGENTS.md` (status), `stdlib/README.md`

**Dependencies:** PR 9

---

### PR 11: Inferência local Nim-style (`0.3.1`) — ✅

**Description:** Omissão de tipo em bindings locais quando o RHS fixa o tipo na
mesma linha (literais; `User {…}`; regras do bloco 8b). API `pub`/params/retornos
continuam anotados. Entregue como `0.3.1`.

**Files/components affected:** `compiler/crates/ori-types/`, `compiler/crates/ori-parser/`, `compiler/crates/ori-driver/tests/`, `docs/spec/`

**Dependencies:** PR 10

### PR 11b: Inferência local opção B — ✅

**Description:** Estender a omissão local a **campo**, **index**, **chamada com
retorno conhecido** e **pipe `|>`** (tipado como `f(value)`). Rejeitar `void`.
Sem C/D (tipo pelo uso) e sem HM. Testes + spec 04/05/06 + catálogo.

**Files/components affected:** `compiler/crates/ori-types/src/check.rs`, tests, docs

**Dependencies:** PR 11

---

## Ordem de execução

```
PR1 (docs lock) ──┐
PR2 (module/func) ┼──▶ PR3 (types []) ──▶ PR5 (literals) ──┐
                  ├──▶ PR4 (try/elif/match) ───────────────┼──▶ PR8 (ritmo) ──┐
                  ├──▶ PR6 (imports) ──────────────────────┤                 │
                  └──▶ PR7 (apply/use) [needs PR3] ────────┘                 │
                                                                              ▼
                                                         PR9 (migrate sources) ──▶ PR10 (docs 0.3.0)
                                                                                      │
                                                                                      ▼
                                                                                 PR11 (0.3.1 infer) 
```

| Nível | PRs (paralelos) |
|-------|-----------------|
| 0 | PR1, PR2 |
| 1 | PR3, PR4, PR6 (após PR2); PR7 após PR2+PR3 |
| 2 | PR5 (após PR3); PR8 (após PR2+PR4) |
| 3 | PR9 (após PR3–PR8) |
| 4 | PR10 (após PR9) |
| 5 | PR11 — **release 0.3.1** ✅ |
| 6 | PR11b — **opção B** ✅ |

---

## Notas de implementação

1. **Corte seco no produto:** formas antigas emitem **erro**, não warning, ao
   final de cada PR de superfície (ou no PR9 se dual interno for necessário
   entre PRs — preferir erro cedo + migrar testes no mesmo PR).
2. **Dual só em dev:** se um PR intermediário precisar aceitar as duas formas
   temporariamente, remover dual **antes** do PR10.
3. **Testes:** `ori-testing` — L1 check → L2 compile → L3 run nos E2E
   tocados; `diagnostic_catalog` após novos códigos.
4. **Não copiar gramática Auk9 cegamente** nos pontos de exceção do ADR
   (import `path = alias`, if-then, closures `(u)=>`, sem `default` keyword).
5. **Codegen/runtime:** preferir desugar para AST/HIR existente; evitar
   redesign de async/ARC neste plano.
6. **Pacotes game/imgui:** **cancelados** — fora do produto
   é backlog pós-0.3.0 (decisão 9.4 B).
7. **Script de migração:** não precisa ser perfeito; deve ser idempotente o
   bastante para re-rodar em CI local.

---

## Critério de pronto `0.3.0` (após PR10)

- [ ] `cargo test --workspace` verde (ou subset documentado se LSP flaky)  
- [ ] `cargo test -p ori-driver --test diagnostic_catalog`  
- [ ] examples + stdlib na sintaxe S3  
- [ ] manifesto `00` + specs alinhadas  
- [ ] CHANGELOG `[0.3.0]` com breaking list  
- [ ] Forma antiga: erro  

## Critério de pronto `0.3.1` (após PR11)

- [x] Omissão local documentada e testada  
- [x] API pública ainda anotada  
- [x] Sem HM global  

## Critério de pronto opção B (após PR11b)

- [x] Campo / index / call / pipe com tipo concreto  
- [x] Rejeita `void` / `try` / empty / bare `none`  
- [x] Spec + CHANGELOG + testes  
- [x] Pipe tipado como call no checker  

