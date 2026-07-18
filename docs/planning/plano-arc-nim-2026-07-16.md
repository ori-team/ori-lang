# Plano ARC/ORC — implementação e correção a partir do estudo Nim

> **Status:** **CONCLUÍDO 2026-07-18** — F0–F7 entregues (LANG-MEM-0…9
> done no BACKLOG; F6 fechado como ADR de deferimento). Criado 2026-07-16
> **Base:** nota de estudo C0 [`historico/nim-study-2026-07-16-c0.md`](historico/nim-study-2026-07-16-c0.md)
> (Nim `devel` @ `3bb46d3`) e direção
> [`language-direction-decisions-2026-06-30.md`](language-direction-decisions-2026-06-30.md)
> ("aproximar de Nim ORC, não de Pony ORCA").
> **IDs de backlog:** `LANG-MEM-0…7` (ver BACKLOG.md §2).

## TL;DR

1. **Corrigir primeiro** (F0–F2): comentário/spec do layout do header,
   auditoria da sobreposição dtor × edges, e completude de edges — tudo
   guiado por testes que falham antes do fix.
2. **Evoluir depois** (F3–F4): collector incremental por suspeitos com
   threshold adaptativo, e elisão de pares retain/release.
3. **Documentar e decidir** (F5–F7): regras de safe point async, ADR de
   COW, flag DX `expand-arc`.

Gate do prompt de estudo: **só implementar F3+ depois que F1–F2 fecharem**.
Nenhuma fase copia código Nim; tudo é reimplementação Rust/Cranelift com
ABI `ori_*`.

---

## Pré-requisito: código-fonte do Nim na máquina

As fases F1+ exigem consulta ao fonte do Nim, que **não é versionado**
neste repo (é referência de estudo de terceiros — nunca copiar código).
Antes de executar o plano numa máquina nova:

```bash
cd <raiz do repo ori-lang>
git clone --depth 1 --branch devel https://github.com/nim-lang/Nim.git _references/nim-lang
```

- `_references/` está no `.gitignore` (só leitura/estudo; licença do Nim em
  `nim-lang/copying.txt`).
- Commit estudado na nota C0: `3bb46d3`. O `devel` avança; toda nota nova
  cita o commit atual via `git -C _references/nim-lang rev-parse --short HEAD`.
- Arquivos-chave do estudo: `doc/mm.md`, `doc/destructors.md`,
  `lib/system/arc.nim`, `lib/system/orc.nim`,
  `compiler/injectdestructors.nim`, `compiler/liftdestructors.nim`.
- O prompt mestre do programa de estudo está preservado em
  [`prompt-analisar-nim-para-ori.md`](prompt-analisar-nim-para-ori.md)
  (os caminhos `_references/...` dele valem a partir da raiz deste repo).

## Regras de execução (todas as fases)

- Uma fase por sessão de trabalho; máx. 3 mudanças por PR/slice.
- Teste que reproduz o problema **antes** da correção (prova por teste).
- Validação padrão de cada slice:
  1. `cargo test -p ori-runtime` (workspace em `compiler/`)
  2. `cargo test -p ori-driver` (testes de contrato do native backend)
  3. `ORI_TEST_LEAK_CHECK=1` nos testes de memória
  4. Se `ori-runtime` mudou: `bash tools/stage_native_runtime.sh` antes de
     testar `ori compile` (o link usa o staticlib stageado)
  5. `performance_guard` (não regredir LANG-PERF-3: retain/release O(1))
- Se o contrato mudar: atualizar `docs/spec/10-memory.md` (+ `19-abi.md` se
  ABI) e `CHANGELOG.md` quando user-facing.
- Comentários de decisão em código: inglês. Este doc e notas: PT-BR.

---

## F0 — Correções imediatas de verdade documental (LANG-MEM-0)

**Tipo:** correção · **Esforço:** S · **Prioridade:** 1 (fazer já)

**Problema (achado C0, pergunta 4):** o comentário de topo em
`compiler/crates/ori-runtime/src/lib.rs` descreve o header como
`[ref_count: u32][type_tag: u32]`, mas o struct real é
`OriHeapHeader { refcount: AtomicI64, destructor: Option<fn> }` (16 bytes).
Documentação errada sobre layout de memória é bug latente para quem mexer
no codegen.

**Mudanças:**
1. Corrigir o comentário em `lib.rs` para o layout real.
2. Conferir `docs/spec/19-abi.md` e `docs/spec/10-memory.md`: se citarem
   layout de header, alinhar com o struct real.
3. Adicionar teste em `ori-runtime` que fixa `size_of::<OriHeapHeader>()`
   e os offsets (guard de ABI contra drift futuro).

**Aceite:** teste de layout passa; nenhuma menção a `type_tag` sobra na
documentação do header.

**Não fazer:** mudar o layout em si (isso é decisão do Eixo 1, só depois
de F3 mostrar se precisamos de bits de cor/rootIdx).

---

## F1 — Paridade dtor/cascata: auditoria dtor × edges (LANG-MEM-1)

**Tipo:** correção · **Esforço:** M · **Prioridade:** 1
**Campanha de estudo:** C1 (Eixo 2, cenários S1–S4)

**Problema (perguntas 1–3 da C0):** a Ori tem duas cascatas de liberação —
`__dtor_*` (campos de struct/enum/tuple) e edges (optional/result, frames
async). Se algum caminho do codegen registrar **os dois** para o mesmo
campo, o campo é retained 2× e released 2× (risco de double-free ou leak
conforme o desbalanceio). Além disso, a Ori libera children de edge
**depois** do `libc::free` do dono — ordem diferente do Nim (campos antes
do free) e sem proteção explícita contra dtor reentrante.

**Passos:**

1. **Auditoria estática** em `compiler/crates/ori-codegen/src/native_backend/`:
   mapear, por tipo managed (string, bytes, list, map, set, struct, enum,
   tuple, optional, result, closure, frame async), qual mecanismo de
   cascata é usado em cada site de store. Saída: tabela no relatório da
   sessão (template da nota Nim, cenário por cenário).
2. **Testes S1–S4** em `native_backend/tests.rs` (+ driver quando fizer
   sentido), todos com `ORI_TEST_LEAK_CHECK=1`:
   - S1: bind local managed sai de escopo (`const s = f()`).
   - S2: assign overwrite `x = y` — inclusive **enum rebind para variante
     diferente** (o `__dtor_enum` deve soltar a variante antiga, com a tag
     lida antes do overwrite).
   - S3: return managed (retain do retorno vs cleanup do frame).
   - S4: struct com campo managed — literal, update de campo, nested.
3. **Teste de reentrância:** dtor de child que aloca/registra edge durante
   `free_registered_object` e durante `ori_arc_collect_cycles` (análogo ao
   bug Nim #22927). Verificar deadlock (mutex global) e consistência do
   snapshot de marks.
4. **Corrigir o que falhar.** Correções esperadas (se confirmadas):
   invariante "um mecanismo por tipo" imposta no codegen; ordem de
   liberação documentada no spec; guarda de reentrância no runtime.

**Aceite:**
- Tabela de auditoria completa (todos os tipos managed × mecanismo).
- Testes S1–S4 verdes com leak check; enum rebind coberto.
- Reentrância de dtor não trava nem corrompe.
- `docs/spec/10-memory.md` atualizado se a ordem/invariante mudar.

**Não fazer:** introduzir `=wasMoved`/move semantics (é F4); mexer no
collector (é F3).

---

## F2 — Completude de edges nos tipos managed (LANG-MEM-2)

**Tipo:** correção · **Esforço:** M · **Prioridade:** 1
**Campanha de estudo:** C2 (Eixo 3, cenários S4–S5, S8–S10)

**Problema:** no modelo Ori, edge faltando = ciclo vira **leak**; edge
sobrando = **double release** (UAF). O grafo registrado precisa ser
completo exatamente onde há posse de managed por managed. O Nim resolve
com `=trace` gerado por tipo; a Ori registra no codegen — então a
completude é responsabilidade nossa, store a store.

**Passos:**

1. Estender a auditoria de F1 com a coluna "registra edge em todo store?":
   closures capturando managed (S8), frames async pré/pós-await (S9),
   optional/result com payload managed (S10), collections aninhadas
   (list de list, map com valores managed) (S4), ciclo direto A→B→A via
   closures/estruturas (S5).
2. Para cada gap: teste que falha primeiro —
   `ori.test.assert_no_leaks` para edge faltante; ASAN/double-free para
   edge duplicada (rodar suíte de memória com sanitizer quando possível).
3. Corrigir no codegen (`emit_arc_*` / registro de edges) — nunca com
   trace genérico no runtime (decisão de direção: edges são do codegen).
4. Registrar no spec a lista canônica: quais tipos usam dtor, quais usam
   edges, e o porquê.

**Aceite:** matriz "tipo managed × registra edges em todos os stores"
sem células vermelhas; testes de ciclo S5 reclamados pelo collector;
leak check verde na suíte.

**Não fazer:** `=trace` dinâmico por tipo no runtime (contraria a decisão
de manter o grafo no codegen; reavaliar só se a matriz mostrar custo
inviável).

---

## F3 — Collector incremental: suspeitos + threshold adaptativo (LANG-MEM-3)

**Tipo:** evolução (perf) · **Esforço:** L · **Prioridade:** 2
**Campanha de estudo:** C3 (Eixo 4) · **Gate:** F1 e F2 fechadas

**Problema:** `ori_arc_collect_cycles` escaneia o heap inteiro
(O(alocações vivas)) sob o lock global, a cada passe. O Nim só examina um
**buffer de suspeitos** (decrefs que não zeraram) e adapta o threshold
pela eficácia do passe. Em heaps grandes, o full scan vira pausa
perceptível nos safe points.

**Mudanças (ideias a validar, na ordem):**

1. **Buffer de suspeitos:** registrar candidato quando um release não zera
   o refcount de um objeto que participa de edges (aproximação Ori do
   `rememberCycle`). Guardar índice no próprio registro de alocação
   (análogo do `rootIdx`, remoção O(1)) — sem bit-packing no refcount.
2. **Passe restrito:** trial deletion apenas sobre o subgrafo alcançável
   a partir dos suspeitos, não sobre `allocations` inteiro.
3. **Threshold adaptativo:** manter o contador cooperativo (256) como
   fallback, mas ajustar pela eficácia (`freed*2 >= touched` → encolhe;
   senão cresce), como o Nim. Atalho `rcSum == edges` ("tudo é lixo")
   se a contabilidade permitir.

**Métricas/aceite:**
- Bench antes/depois em `benchmarks/` (ou bench sintético no
  `performance_guard`): tempo de passe com 100k alocações vivas e 0
  ciclos deve cair de O(n) para ~O(suspeitos).
- Nenhuma regressão em `run_ffi_boundary_cost_stays_flat_with_many_live_allocations`.
- Todos os testes de ciclo de F2 continuam passando (mesmos objetos
  reclamados, mesma contagem).
- Spec 10 atualizado (gatilhos e complexidade do collector).

**Não fazer:** cores black/gray/white empacotadas no refcount word (só
reconsiderar se o buffer de suspeitos não bastar); jump stack/critical
links (o próprio Nim desligou).

---

## F4 — Elisão de RC no codegen (LANG-MEM-4)

**Tipo:** evolução (perf) · **Esforço:** L · **Prioridade:** 3
**Campanha de estudo:** C4 (Eixo 5, cenários S6–S7) · **Gate:** F3 medida

**Problema:** a Ori insere retain/release por escopo léxico, sem análise
de último uso. O Nim elide pares via DFA (`lastReadOf`), sink e move.
Cada par elidido economiza **duas** operações que na Ori custam lock +
lookup.

**Pré-requisito de medição:** microbench isolando custo de retain/release
por iteração (S6: temporários em `g(f(), f())`; S7: rebind em loop).
Sem baseline não há ROI mensurável — regra do prompt de estudo.

**Mudanças candidatas (máx. 3, cada uma com métrica):**
1. Elisão de pares retain/release para temporários que morrem na mesma
   expressão (S6).
2. Último uso em rebind de loop (S7): release do valor antigo sem retain
   redundante do novo quando a fonte morre ali.
3. Transferência de ownership no return (S3): não retain + release no
   frame; mover a referência.

**Aceite:** bench S6/S7 melhora mensurável; suíte de memória + leak check
verdes; contagem de calls `ori_arc_*` num programa de referência cai
(teste de contrato pode contar símbolos emitidos, estilo `expandArc`).

**Não fazer:** move semantics de superfície (sem `sink`/`move` na
linguagem — decisão S3 reading-first); mudanças que exijam anotação do
usuário.

---

## F5 — Regras de safe point async/threads documentadas (LANG-MEM-5)

**Tipo:** documentação normativa · **Esforço:** S · **Prioridade:** 2
**Campanha de estudo:** C5 (Eixo 6, cenário S9)

O runtime já coleta pós-await e nos safe points do executor, e o RC é
atômico (diferente do Nim, que move subgrafos). Falta o contrato escrito:

1. Enumerar em `docs/spec/10-memory.md` todos os safe points atuais
   (fim de fn top-level, pós-await com release de frame values, drain do
   executor, contador cooperativo) — hoje o texto diz que "não há
   coleta periódica", o que já divergiu do contador de 256 allocs.
2. Documentar a decisão "RC atômico + lock global" vs "RC não-atômico +
   move de subgrafos" (Nim) como trade-off registrado, com ponte para o
   modelo de threads da Ori.
3. Cancel tokens + frames managed: descrever o caminho de cleanup em
   cancelamento (ligar com o TODO residual de `using` async do spec).

**Aceite:** Spec 10 sem afirmações desatualizadas sobre gatilhos; decisão
de atomicidade registrada (ADR curto ou seção no doc de direção).

---

## F6 — ADR: COW para collections (LANG-MEM-6)

**Tipo:** decisão · **Esforço:** S (ADR) · **Prioridade:** 3
**Campanha de estudo:** C6 (Eixo 7)

A Ori cria listas novas a cada mutação aparente; o Nim muta in place
quando `isUniqueRef`. Um `is_unique` na Ori (refcount == 1 sem edges de
entrada) permitiria mutação in place invisível ao leitor S3.

**Entregável:** ADR aceitar/rejeitar com: bench de custo atual (append em
loop, S7), esboço do check de unicidade no runtime, análise de risco
(aliasing via edges) e impacto zero na superfície da linguagem. Sem
implementação até o ADR ser aceito.

---

## F7 — DX: flag `expand-arc` (LANG-MEM-7)

**Tipo:** DX · **Esforço:** S · **Prioridade:** 3
**Campanha de estudo:** C7 (Eixo 8)

Análogo barato do `--expandArc` do Nim: dump textual dos pontos onde o
codegen inseriu `ori_arc_retain/release/register_edge` por função
(ex.: `ORI_DUMP_ARC=fn_name`, no molde do `ORI_DUMP_CLIF` existente).
Serve de ferramenta de verificação para F4 (contar pares elididos).

**Aceite:** flag documentada (docs internos, não superfície da
linguagem); teste de driver que confere o dump para um caso S1 simples.

---

## Ordem e gates (resumo)

| Fase | ID | Tipo | Esforço | Gate |
|------|----|------|---------|------|
| F0 | LANG-MEM-0 | correção doc/ABI | S | — |
| F1 | LANG-MEM-1 | correção (testes primeiro) | M | — |
| F2 | LANG-MEM-2 | correção (testes primeiro) | M | F1 |
| F3 | LANG-MEM-3 | perf collector | L | F1+F2 |
| F4 | LANG-MEM-4 | perf elisão | L | F3 + baseline |
| F5 | LANG-MEM-5 | spec/contrato | S | paralelo (após F1) |
| F6 | LANG-MEM-6 | ADR COW | S | após F3 |
| F7 | LANG-MEM-7 | DX dump | S | antes ou junto de F4 |

**Recomendação de próxima sessão:** F0 + início de F1 (auditoria +
primeiros testes S1–S2). F0 é pequena e destrava a confiança no layout;
F1 é onde moram os bugs potenciais.

## Fora de escopo do plano inteiro

- Portar código Nim (qualquer arquivo) para o tree da Ori.
- Multi-modos de GC, RC não-atômico sem modelo de threads, macros/effects.
- Anotações de usuário (`weak`, `acyclic`) sem ADR aprovado contra o
  manifesto reading-first.
- Self-host / bootstrap (M4).
