---
id: selfhost-remaining-gap-list
title: Self-Hosted Compiler — Lista exaustiva de implementação restante
status: in_progress
adr: docs/decisions/adr/0006-selfhost-modular-architecture.md
target_version: 0.4.0
created: 2026-09-09
---

# Compilador Self-Hosted Ori — Tudo que ainda falta implementar

> **Método:** comparação direta entre o compilador de referência Rust
> (`compiler/crates/*`, ~48.8k linhas) e o compilador em Ori
> (`selfhost/compiler/**/*`). Cada item existe no Rust e está ausente ou
> parcial no Ori. Nada aqui é aspiracional — é débito de paridade medido.

## F1 — Frontend: lexer

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| F1-01 | P1 | S | Literais numéricos completos: prefixos `0x`/`0o`/`0b`, separador `_`, sufixos `i8`–`u64`/`f32`/`f64`, validação de overflow | `0xFF`, `1_000_000`, `42u8` lexam como um token com valor certo; overflow rejeitado | `done` |
| F1-02 | P1 | M | Strings interpoladas `f"..."`/`f"""..."""`: parsing de `{expr}`, `{{`/`}}`, erros `fstring_*` | Golden com interpolação aninhada + 3 negativas | `todo` |
| F1-03 | P1 | S | Triple-quoted strings: normalização CRLF, strip de newline inicial, dedent de baseline | Golden de bloco multi-linha idêntico ao Rust | `todo` |
| F1-04 | P1 | S | Bytes `b"..."`: escapes `\xNN`, rejeição de `\u` (`parse.byte_unicode_escape`) | Golden + negativa de unicode-escape | `todo` |
| F1-05 | P1 | S | Operadores bitwise/shift no lexer: `&`, `\|`, `^`, `~`, `<<`, `>>` (símbolos, não keywords) | `a << 2 \| b` tokeniza como 5 tokens de op | `todo` |
| F1-06 | P1 | S | Range `..` vs `.` vs número `1.0`: desambiguação completa | `0..9`, `a.b`, `3.14` sem colisão | `todo` |
| F1-07 | P2 | S | Comentários bloco/doc `--| … |--` emitidos (p/ `ori doc`); linha `--` ignorado | Doc-comment preservado no stream | `todo` |

## F2 — Frontend: parser de expressões

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| F2-01 | P1 | L | Precedência total: `\|>` < `or` < `\|` < `^` < `&` < `and` < `<<`/`>>` < comparação < `+`/`-` < `*`/`/`/`%`, com encadeamento à esquerda | Golden `1 + 2 * 3`, `a \| b ^ c & d`, `x \|> f` | `done` (parcial: sem `\| & ^ << >>`) |
| F2-02 | P1 | S | `is Type` (`IsCheck`) com precedência própria | Golden `x is Circle` | `todo` |
| F2-03 | P1 | S | Range `a..b` como expressão (inclusive, descendente ok) | Golden `0..9`, `5..3` | `todo` |
| F2-04 | P1 | L | Unários completos: `-`, `not`, `~`, `await`, `try` (contextual) | Golden de cada unário + `try f()` | `todo` |
| F2-05 | P1 | L | Cadeia pós-fixa: `.campo`, `.0` (tuple index), `f(args)`, `Type{...}`, `[i]`/`[a..b]`, sem `?` (erro `parse.question_propagate_removed`) | Golden de acesso encadeado `a.b[0](x)` | `todo` |
| F2-06 | P1 | M | Argumentos de chamada: posicionais, nomeados `n: e`, spread `..e`; valores são exprs completas | Golden `f(1, y: g(x), ..rest)` | `todo` |
| F2-07 | P1 | M | Closures completas: 0–N params tipados, `-> R`, corpo `=> expr` ou bloco `... end closure`; desambiguação `(a)` vs closure | Golden dos 3 formatos + negativa `(a)` não-closure | `done` (parcial: só 1 param sem tipo) |
| F2-08 | P1 | M | Struct literals `Type { f: v }`, anônimos `{ f: v }`, update `p with { x: v }`; rejeitar `Type(...)` legado | Golden dos 3 + negativa legada | `done` (parcial: só 1 forma) |
| F2-09 | P1 | M | Literais compostos: `[a,b]`, `{}` map vazio, `{k:v}` map vs `{f:v}` anon-struct, `set{...}`, `tuple(a,b)`, `()` unit, bytes | Golden de cada + aridade de tupla | `todo` |
| F2-10 | P1 | M | `if cond then a else b` inline (else obrigatório); `match` como expressão com guards | Golden + negativa sem `else` | `todo` |
| F2-11 | P2 | L | Chamada poética `callee arg` (mesma linha, 1 arg, sem aninhar) + erro `parse.poetic_call_nested` | Golden + negativa aninhada | `todo` |

## F3 — Frontend: statements

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| F3-01 | P1 | M | `var`, `break`, `continue`, `return [e]` nu; atribuição `=` e composta `+=`/`-=`/`*=`/`/=`; lvalue `a.b[i]`; erro `parse.invalid_lvalue` | Golden de cada + negativa de lvalue | `todo` |
| F3-02 | P1 | M | Destruturação `const/var {a, b:c} = e` + `parse.empty_destructure`; inferência local sem anotação (opção B) | Golden destrut + golden sem anotação | `todo` |
| F3-03 | P1 | M | `if/elif/else`, `if some/ok/err(x) = e`, `else if` rejeitado (use `elif`), `end if` validado | Golden de cada forma + 2 negativas | `todo` |
| F3-04 | P1 | L | `while [some]`, `for x[,y] in it`, `repeat e [times]`, `loop`, todos com `end <label>` | Golden de cada loop | `todo` |
| F3-05 | P1 | M | `match` stmt: `case p [if g]:`, `case else:` por último, `end match`; `using x: T = e`; `check c[, "msg"]` (msg deve ser literal) | Golden + 3 negativas | `todo` |
| F3-06 | P1 | S | `suspend e`, terminadores `Else/Elif/Case`, sincronização de erro por bloco | Recuperação sem abortar arquivo | `todo` |

## F4 — Frontend: patterns e tipos

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| F4-01 | P1 | M | Patterns: `none`, `some(p)`, `ok(p)`/`err(p)`, `tuple(p,..)`, literais ±, `Variant(f, f:p)` + shorthand, `or` n-ário; distinção `Bind` vs variante unitária; erro `.Variant` | Golden de cada + 2 negativas | `todo` |
| F4-02 | P1 | L | Tipos primitivos como keywords + compostos: `set/range/lazy/handle/map/any/tuple/func/generic/buffer/slice/array/simd`, `Name[T]`, const-args `name: expr` | Golden de cada forma; rejeitar `<T>`, `of`, `<>` legados | `done` (parcial: só `list/optional/result`) |
| F4-03 | P2 | M | Recuperações legadas com diagnóstico: `<>`, `of`, `where`, `do`, `.{...}`, `success/error`, `Type(...)` | 1 negativa por forma legada | `todo` |

## F5 — Frontend: itens, imports, attrs, recovery

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| F5-01 | P1 | M | `module a.b.c` pontilhado primeiro; `namespace` rejeitado; `module_not_first`/`import_after_declaration` | Golden + 2 negativas | `done` (parcial: só 1 segmento) |
| F5-02 | P1 | M | Imports: `public`, `path as/= alias`, seletiva `path(A, B=C)`, bloco `imports ... end`, rejeitar `only` | Golden de cada forma | `done` (parcial: só `as`) |
| F5-03 | P1 | M | Funções S3: `async/iter/mut`, genéricos `[T]` + `for` bounds, `self`, `n:T...`, default `=e`, contrato `if e`, corpo `=>e` ou bloco; validações (`duplicate_param`, `variadic_not_last`, …) | Golden full + 3 negativas | `todo` |
| F5-04 | P1 | M | Struct com métodos/contratos, enum com payloads `V(n:T)`, trait (assoc `type`, required vs default, disambig call-vs-sig), `apply` (colon/multi/compact/`use Tr[T]`/binds/assoc alias), `alias`/`newtype`/`const`/`var` top-level, `extern "c"` | Golden de cada item | `todo` |
| F5-05 | P1 | M | Atributos `@a.b(...)` + `@cfg(...)` com predicados aninhados; profundidade máxima 128 | Golden + negativa de nesting | `todo` |
| F5-06 | P2 | M | Recovery: `end <label>` validado (`unterminated_block`, `end_label_mismatch`), `synchronize` por conjuntos, todos `removed_*` | Arquivo com 3 erros emite 3 diagnósticos com spans | `todo` |

## T1 — Tipos: núcleo e unificação

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| T1-01 | P1 | XL | Inventário `Ty` completo: 16 primitivas dimensionadas, `Never`/`Error`, `Buffer/Slice/Array/Simd/Map/Set/Range/Tuple/Lazy/Handle/Future/TaskJob/Channel/AtomicInt` + erros, 17 `OpaqueTy`, `Any/Func/Named/Param/Infer/ConstInt` | Teste de construção de cada variante | `todo` |
| T1-02 | P1 | L | Unificação real: occurs-check, binding de `Infer(id)` em tabela de substituição, coerção `Never`, promoção numérica, veneno `Error` | 6 testes (incl. ciclo `T = list[T]` rejeitado) | `done` (parcial: igualdade estrutural rasa) |
| T1-03 | P1 | M | Literais: radices, `_`, sufixos, overflow por largura; floats científicos/`f32`; `if some/ok/err` narrowing; `@cfg` filtering | Golden + 4 negativas | `todo` |

## T2 — Tipos: inferência, traits, generics, stdlib, const

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| T2-01 | P1 | XL | `infer_expr` bidirecional em 30+ formas + join de branches + coleções vazias + `return` vs declarado + args nomeados/default/variádicos + chamadas de método | Suíte de 15 casos | `todo` |
| T2-02 | P1 | L | Traits: conformidade de assinatura (params+ret+`self`), where-clauses (`is`/`is not`), assoc types, defaults, `T.m()` estático, `any[Trait]` + vtable real | Suíte de 10 casos | `done` (parcial: só nome de método) |
| T2-03 | P1 | XL | Generics: substituição em AST/HIR, const-generics, monomorfização com clone de corpo + reescrita de callsites até ponto fixo, remoção de templates | Golden `id[T]` com 2 instanciações | `done` (parcial: só chave de nome) |
| T2-04 | P1 | XL | Stdlib: 436 entradas / 459 assinaturas / 13 raízes + ABI nativa (`Ptr/I64/I32/I8/F64`) em vez de 10–14 mocks | `lookup` de 30 paths reais | `todo` |
| T2-05 | P2 | M | Const-eval CT-0: bitwise/bool/mod/comparações, floats/strings, `if` const, refs cruzadas, ciclos, overflow | Suíte de 8 casos | `done` (parcial: só `+-*/` em int) |
| T2-06 | P1 | M | Exhaustiveness real: decomposição aninhada/tupla/literal, or-patterns, guards, redundância (`match.unreachable_case`) | Suíte de 8 casos | `done` (parcial: só lista plana) |

## R1 — Resolução: DefMap, imports, visibilidade

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| R1-01 | P1 | L | `DefId` arena + espaços sintéticos + `DefKind` (9: Struct/Enum/Trait/Func/Const/Var/TypeAlias/Newtype/Extern) | IDs únicos estáveis por sessão | `todo` |
| R1-02 | P1 | M | Imports: relativos/absolutos, wildcard, item, re-export, rename; visibilidade `public` em itens e campos (`name.private`) | Golden multi-arquivo + 2 negativas | `done` (parcial: só alias plano) |
| R1-03 | P1 | M | Escopos: shadowing, bindings `let/var/const`, escopos de loop (`break`/`continue`), fronteira de captura de closure | Golden de shadowing + 2 negativas | `done` (parcial: só 1 nível global) |

## H1 — HIR: nós, lowering, ARC, closures, async

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| H1-01 | P1 | L | Nós HIR ausentes: 8 stmts (`If/While/For/Loop/Repeat/Match/Using/Check`), 10 patterns, `Field`/`Index` lvalues, 23 exprs (literais compostos, `Field/Index/TupleIndex`, `MethodCall/AssociatedCall`, `StructLit/EnumVariant/ListLit/ArrayLit/SimdLit/TupleLit/StructUpdate`, `Some/None/Ok/Err/try/await/IfExpr/MatchExpr`, `Range/MapLit/SetLit`, `Closure/IsCheck`) | Construção de cada nó com teste | `todo` |
| H1-02 | P1 | XL | Lowering real: expr/stmt recursivo, `\|>`/`try`/f-string/`using`/iter-inline/default-args/overload-op/newtype-erase/destructure; itens struct/enum/trait/const/extern (hoje só nome de func) | Golden arquivo→HIR | `done` (parcial: só nomes) |
| H1-03 | P1 | L | ARC: `is_runtime_managed()` por tipo + retain/release/edge-register nos pontos de lowering (hoje só rastreador mock) | `memory_arc`-like verde | `todo` |
| H1-04 | P1 | M | Closures: coleta de livres, func sintética, env struct + `Closure{func,captures}` (hoje só push de strings) | Golden com captura real | `todo` |
| H1-05 | P1 | M | Async: flag `is_async`, `Future[T]` no ret, nó `Await` propagado (state machine fica no backend) | Golden `async/await` | `todo` |
| H1-06 | P2 | M | Verifier: unicidade func/struct/enum, campos/variantes dup, integridade de DefId em literais, arms não-vazios, recursão de exprs | 6 negativas | `done` (parcial: só bloco não-vazio + terminador) |
| H1-07 | P2 | M | Otimizador: driver `OptLevel` + ponto fixo, const-fold em HIR, DCE, strength-reduce, inline de folhas | 4 goldens de opt | `todo` |

## B1 — Backend, bridge IPC e driver

| ID | Pri | Esforço | Item (Rust → Ori) | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| B1-01 | P1 | XL | Bridge schema total: todos os `HirStmt/HirExpr/HirPattern/HirTy` serializáveis (hoje: 5 tys, 6 exprs, 3 stmts) | Round-trip de módulo real | `todo` |
| B1-02 | P1 | M | Transporte IPC real no Ori: socket Unix/stdio subprocesso → `ori-bridge-server` (hoje só imprime tamanho do JSON) | `ori-stage1 compile` gera `.o` real | `todo` |
| B1-03 | P1 | L | Vocabulário: unidades incrementais, comando link, streaming de diagnósticos, `repl-eval` | 1 teste por comando | `todo` |
| B1-04 | P2 | M | JSON com escaping total (hoje concatenação manual) | Fuzz de strings com `"`/`\`/unicode | `todo` |
| B1-05 | P1 | XL | Codegen Cranelift: structs/enums/tuplas/coleções, closures, vtables, ARC, async frames, convenções SystemV/Fastcall, `@c_export` + `.h`, DWARF via bridge | Suíte AOT verde via bridge | `todo` |
| D1-01 | P1 | XL | CLI: 18 subcomandos ausentes (`build/run/test/fmt/lint/doc/install/get/lock/publish/debug/repl/doctor/update/explain/summary/migrate-syntax/daemon/bindgen/lex/parse`) + flags globais + `--help/--version` | `--help` de cada + 1 teste | `todo` |
| D1-02 | P1 | M | Pipeline multi-arquivo: grafo de imports, `@cfg`, cache incremental `.ori/`, split de módulos, link com `libori_runtime.a` + CRT + ABI-check + DWARF/`objcopy` | `compile` de projeto real gera ELF | `todo` |
| D1-03 | P1 | L | Packaging: parsers `ori.pkg.toml`/`ori.proj`, `ori.lock` (`--locked/--offline`), SemVer, deps path/git/registry, cache `~/.ori/packages`, `native_deps` | `install`+`lock`+`build` offline | `todo` |

## M6 — Runtime (correções validadas no Módulo 6)

| ID | Pri | Esforço | Item | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| M6-01 | P1 | L | FIX-JIT-STRUCT: regressão AOT decidida; SOA no self-host | Teste `nested_struct_newtypes` verde | `done` |
| M6-02 | P1 | M | FIX-FLAKY-EMBED: 38/38 `ori-embed` determinísticos | Suíte verde | `done` |
| M6-03 | P1 | M | FIX-STR-INTERP: regressão de interpolações encadeadas | Teste verde | `done` |

## M7 — Prova final

| ID | Pri | Esforço | Item | Aceite | Status |
|---|:---:|:---:|---|---|:---:|
| M7-01 | P1 | L | `bin/ori-stage1` ELF autônomo a partir de `main.orl` | Executa sem Rust | `done` (demonstração; CLI é demo) |
| M7-02 | P1 | XL | Stage 2/3 + `diff` + 251 testes via stage2 + aposentadoria do frontend Rust | Critérios do plano | `todo` |

## Ordem de execução proposta (dependências)

```text
F1 → F2+F4-02 → F3+F4-01 → F5 → R1 → T1 → T2 → H1 → B1 → D1 → M7
```

**Regra de avanço:** cada ID só vira `done` com `ori check` limpo +
harness `ori run` verde no módulo + teste diferencial contra o stage0
quando houver semântica executável envolvida.
