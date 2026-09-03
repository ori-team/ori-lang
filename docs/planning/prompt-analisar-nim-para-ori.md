# Prompt: analisar Nim para evoluir a Ori

> Cole este documento inteiro (ou a seção **PROMPT PARA O AGENTE**) em uma sessão
> de estudo/implementação. Caminhos relativos à **raiz do repositório Ori**.
>
> Fontes Nim: `_references/nim-lang/` (clone `devel`).
> Contrato Ori: `docs/spec/00-manifesto.md`, `docs/spec/10-memory.md`,
> `docs/planning/language-direction-decisions-2026-06-30.md`.

---

## Contexto fixo (não negociar)

### O que a Ori é

- Linguagem **AOT nativa** (Cranelift) + JIT opcional em `ori run`.
- Compilador em **Rust** (`compiler/crates/`), runtime Layer 1 em **Rust**
  (`ori-runtime`), stdlib em `.orl` (`stdlib/`).
- **Reading-first**, tipagem **explícita** (inferência só local, sem HM global).
- Memória: **value semantics** + **ARC** em tipos managed + **cycle collector**
  (trial-deletion), pontos cooperativos — **não** borrow checker, **não** GC
  stop-the-world, **não** multi-MM modes como o Nim histórico.
- Superfície S3: uma forma canônica; erros `result`/`optional` + `try`; traits
  `apply`/`use`.
- Propósito: lab de compilador + IA + legibilidade ND — **não** copiar Nim
  como produto.

### O que a Ori **já decidiu** sobre Nim

| Trazer | Evitar |
|--------|--------|
| Ideias de **ARC + ORC** (ciclos, trial deletion) | Herdar **vários** modos de GC (`refc`, boehm, …) |
| Otimizar RC ops / entender move semantics *conceitualmente* | Sintaxe Nim, macros, effect system inteiro |
| `=destroy` / `=trace` como **modelo mental** | Expor hooks de destrutor ao usuário Ori agora |
| Codegen que **insere** retain/release | Codegen C do Nim como backend da Ori |
| Documentação de trade-offs (`doc/mm.md`, `doc/destructors.md`) | Complexidade histórica de `refc` |

Direção explícita: *aproximar mais de Nim ORC do que de Pony ORCA*
(`docs/planning/language-direction-decisions-2026-06-30.md`).

### Regras de trabalho (obrigatórias)

1. **Estudar → extrair padrão → mapear para Ori → propor mudança mínima.**
2. **Não copiar código Nim** para o repo Ori. Reimplementar em Rust/Cranelift
   com ABI `ori_*` e spec em inglês.
3. Toda proposta deve responder: **o que o leitor Ori vê na fonte?** (manifesto).
4. Skills: `clean-code`, `rust`, `compiler-dev`, `lang-compiled`, `ori-testing`,
   `living-docs`. Comentários de decisão em código Ori: inglês; docs de planning:
   PT ou EN do arquivo.
5. Validar com: `cargo test -p ori-runtime`, testes ARC/ciclo em driver, e
   quando mudar memória: `ORI_TEST_LEAK_CHECK=1` onde aplicável.
6. Citar sempre: **caminho Nim + commit**  
   `git -C _references/nim-lang rev-parse --short HEAD`.
7. Entregar **notas estruturadas** (template no final), não resumo vago.

---

## Mapa de arquivos (estudo)

### Nim — ordem de leitura

#### Fase A — Contrato e vocabulário (ler antes de código)

| # | Arquivo | Por quê |
|---|---------|---------|
| A1 | `_references/nim-lang/doc/mm.md` | Estratégias MM; ORC default; trial deletion; atomic vs não |
| A2 | `_references/nim-lang/doc/destructors.md` | `=destroy`, `=wasMoved`, `=trace`, `=copy`, `=sink`; move |
| A3 | `_references/nim-lang/doc/intern.md` | Pipeline do compilador (visão interna) |

#### Fase B — Runtime ARC/ORC (coração)

| # | Arquivo | Foco |
|---|---------|------|
| B1 | `lib/system/arc.nim` | Inc/dec ref, destroy path, API “rtl” do compiler |
| B2 | `lib/system/orc.nim` | Cores (black/gray/white), trial deletion, `trace`, thresholds |
| B3 | `lib/system/cyclebreaker.nim` | Quebra / casos de ciclo |
| B4 | `lib/system/cellseqs_v2.nim` (e v1 se referenciado) | Estruturas auxiliares do collector |
| B5 | `lib/system/gc_interface.nim`, `gc_common.nim` | Fronteira histórica — **só** o que ORC ainda usa |
| B6 | `lib/system/alloc.nim`, `memalloc.nim`, `osalloc.nim` | Alocação vs RC |
| B7 | `lib/system/seqs_v2.nim`, `strs_v2.nim`, `assign.nim` | Coleções/strings sob ARC |

#### Fase C — Compilador: inserir lifetimes (o que Ori faz no codegen)

| # | Arquivo | Foco |
|---|---------|------|
| C1 | `compiler/injectdestructors.nim` | Pass que injeta destrutores / RC |
| C2 | `compiler/liftdestructors.nim` | Lift de hooks `=destroy` / `=trace` |
| C3 | `compiler/dfa.nim`, `compiler/aliasanalysis.nim` | Análise para elidir RC |
| C4 | `compiler/cgen.nim`, `ccgexprs.nim`, `ccgstmts.nim`, `ccgtypes.nim` | Emissão de calls rtl (inc/dec/destroy) |
| C5 | `compiler/lambdalifting.nim` | Closures + capturas (analogia Ori closures ARC) |
| C6 | `compiler/sem*.nim` (pontual) | Só se tipos/ref semântica forem ambíguas |

#### Fase D — Testes Nim como oráculo de comportamento

| # | Onde | Uso |
|---|------|-----|
| D1 | `_references/nim-lang/tests/arc/` (e vizinhos) | Casos de destroy, ciclo, loop, async |
| D2 | Buscar `expandArc` / flags `--mm:orc` nos testes | O que o Nim considera correto |

### Ori — espelho (comparar sempre)

| Tema | Onde na Ori |
|------|-------------|
| Spec memória | `docs/spec/10-memory.md` |
| Runtime ARC / ciclos | `compiler/crates/ori-runtime/src/lib.rs` (+ `tests.rs`) |
| Inserção retain/release / edges | `compiler/crates/ori-codegen/src/native_backend/` |
| Dtors de struct/enum | codegen `__dtor_*` + registro no alloc |
| Edges / cycle graph | `ori_arc_register_edge`, `unregister`, `update_edge`, `collect_cycles` |
| Pontos cooperativos | fim de função top-level; pós-`await`; `ori.test.collect_cycles` |
| ABI | `docs/spec/19-abi.md` |
| Leak check | `ori.test.assert_no_leaks`, env `ORI_TEST_LEAK_CHECK` |
| Direção Nim | `docs/planning/language-direction-decisions-2026-06-30.md` |

---

## O que analisar (checklist por eixo)

Para **cada eixo**, preencher o template de nota (final do doc).

### Eixo 1 — Modelo de contagem de referências

- [ ] O que é um “cell” / header no Nim vs header ARC na Ori?
- [ ] Inc/dec: quando, com que atomicidade? (Nim ORC: RC **não-atômico** + move entre threads; Ori: **atômico** no runtime atual — documentar trade-off)
- [ ] Nil / null object handling
- [ ] Overflow / sticky bits (Nim: `maybeCycle`, cores no mesmo word que RC)
- [ ] O que a Ori **não** deve copiar (bits packed demais sem necessidade)

**Pergunta de aceite:** “Consigo desenhar o layout de header Ori em 10 linhas e dizer o que mudaria se adotássemos bit de cor estilo ORC?”

### Eixo 2 — Destrutores e cascata

- [ ] Como `=destroy` desce campos / seq / string
- [ ] `=wasMoved` e por que Ori (ainda) não tem move semântico de primeira classe
- [ ] Geração de dtor por tipo (Nim lift vs Ori `__dtor_struct_*`)
- [ ] Self-assignment e copy (`=copy`)
- [ ] Ordem: release de campos **antes** de free do header

**Pergunta de aceite:** “Há bug class na Ori se o dtor do enum soltar variante errada? O Nim evita como?”

### Eixo 3 — Grafo de objetos e `=trace` / edges

- [ ] Nim: `=trace` + `traceImpl` no RTTI v2
- [ ] Ori: edges **registradas pelo codegen** (não trace genérico no runtime)
- [ ] Completude do grafo: closures, futures, collections, optional/result payload
- [ ] Custo: registrar edge em todo store vs trace sob demanda
- [ ] Falhas se edge faltar (ciclo vira leak) vs edge sobrar (UAF / double free)

**Pergunta de aceite:** “Lista de tipos Ori managed e se cada um registra edges em todos os stores.”

### Eixo 4 — Cycle collector (trial deletion)

- [ ] Paper/refs no cabeçalho de `orc.nim` (Bacon, Lins)
- [ ] Estados de cor e transições
- [ ] Quando um objeto entra no “suspeito de ciclo”
- [ ] Threshold / quantos toques antes de coletar
- [ ] O que é liberado e em que ordem
- [ ] Interação com async (Nim async **precisa** ORC; Ori já coleta pós-await)

**Pergunta de aceite:** “Diagrama de 1 ciclo A→B→A: passos do ORC vs passos do `ori_arc_collect_cycles`.”

### Eixo 5 — Onde o **compilador** decide RC (elisão e inserção)

- [ ] Passes: inject / lift / DFA
- [ ] Casos de elisão: última use, sink, transfer de ownership
- [ ] Assignment overwrite: retain new **antes** release old? (Ori testa isso)
- [ ] Return: retain do valor de retorno vs cleanup do frame
- [ ] Loops e temporários (fonte clássica de double free / leak)

**Pergunta de aceite:** “3 regras de inserção RC da Ori reescritas ao lado das do Nim; gaps numerados.”

### Eixo 6 — Threads, async, shared heap

- [ ] Nim: heaps / move de subgraph entre threads; RC não atômico
- [ ] Ori: atomic RC; async cooperativo; channels
- [ ] O que Pony/ORCA faria e por que **não** agora
- [ ] Cancel tokens + frames managed

**Pergunta de aceite:** “Podemos manter atomic RC e ainda assim roubar ideias de elisão do Nim?”

### Eixo 7 — Coleções e COW (Swift também, mas Nim tem prática)

- [ ] seq/string: unique ref → mutate in place?
- [ ] Ori: imutabilidade aparente / novas listas — custo vs Nim
- [ ] Onde COW caberia sem quebrar leitura S3

### Eixo 8 — Observabilidade e DX

- [ ] Nim `--expandArc` (como “ver” RC inserido)
- [ ] Ori: diagnostics, leak check, testes em `native_backend/tests.rs`
- [ ] Proposta: flag de debug Ori análoga (só se barato)

### Eixo 9 — O que **não** estudar agora (backlog consciente)

- Macros, templates, concept/typeclass Nim
- MM modes legados (`refc`, boehm, go)
- JS backend / VM Nim (exceto curiosidade)
- Self-host koch bootstrap (salvo M4 da Ori no futuro)
- Copia de stdlib Nim

---

## Como analisar (método)

### Método em 5 passos (repetir por eixo)

```text
1. LER     docs Nim (mm/destructors) → 1 parágrafo com suas palavras
2. RASTREAR 1 símbolo (ex.: nimIncRefCyclic) de definição → call sites no compiler
3. ESPELHO  achar o análogo Ori (ou marcar AUSENTE)
4. DELTA    tabela: Nim | Ori hoje | Gap | Risco | Esforço
5. AÇÃO     0..3 mudanças possíveis, ordenadas por ROI; cada uma com teste
```

### Técnicas concretas

1. **Leitura dirigida por símbolo**  
   No Nim: `nimIncRef`, `nimDecRef`, `nimDestroy`, `nimIncRefCyclic`, `=trace`,  
   `injectdestructors`.  
   Na Ori: `ori_arc_retain`, `ori_arc_release`, `ori_arc_register_edge`,
   `ori_arc_collect_cycles`, `emit_arc_*` no codegen.

2. **Diferença por cenário** (sempre o mesmo conjunto):

   | ID | Cenário | Código mental |
   |----|---------|----------------|
   | S1 | Bind local managed e sai de escopo | `const s = f()` |
   | S2 | Assign overwrite | `x = y` com ambos managed |
   | S3 | Return managed | `return list` |
   | S4 | Campo de struct managed | `S { a: list }` |
   | S5 | Ciclo de 2 refs | A.ref→B, B.ref→A (se Ori tiver ref; senão ciclo via closures/async) |
   | S6 | Temp em expressão | `g(f(), f())` |
   | S7 | Loop rebind | `while …: x = next()` |
   | S8 | Closure captura managed | |
   | S9 | Async suspend/resume | frame values mortos |
   | S10 | optional/result com payload managed | |

3. **Prova por teste**  
   Para cada gap “devemos copiar”: escrever **antes** o teste Ori que falha
   (ou assert de leak), depois implementar.

4. **Custo cognitivo**  
   Se a mudança exigir anotação do usuário Ori (`weak`, `acyclic`, …),
   avaliar contra o manifesto (leitura ND). Preferir default seguro no runtime.

5. **Limite de escopo por sessão**  
   Uma sessão = **um eixo** (ou um cenário S1–S10). Não “estudar Nim inteiro”.

### Anti-padrões

- “Vamos portar `orc.nim` linha a linha para Rust.”
- Misturar frontend S3 com mudança de MM.
- Aceitar RC não-atômico sem desenhar modelo de threads da Ori.
- Ignorar edges e só olhar inc/dec.
- Propor 5 features; máximo **3** ações por relatório.

---

## Ordem de campanhas (roadmap de estudo → implementação)

| Campanha | Eixos | Saída esperada |
|----------|-------|----------------|
| **C0** Setup | A1–A3 | Glossário Nim↔Ori (1 página) |
| **C1** Paridade de dtor/cascata | 2, S1–S4 | Lista de bugs potenciais + testes |
| **C2** Completude de edges | 3, S4–S5,S8–S10 | Auditoria tipos managed |
| **C3** Collector | 4 | Diff trial-deletion Nim vs Ori; gaps |
| **C4** Elisão RC | 5, S6–S7 | 3 opts de elisão com métrica |
| **C5** Async/threads | 6, S9 | Regras de safe point documentadas |
| **C6** COW/coleções | 7 | ADR aceitar/rejeitar COW |
| **C7** DX | 8 | Proposta `expand-arc` ou equivalente |

Só **implementar** após C1–C3 se o objetivo é “corrigir memória”.  
C4+ é evolução/performance.

---

## PROMPT PARA O AGENTE

Copie o bloco abaixo.

```text
Você é um engenheiro de compiladores trabalhando no projeto Ori (linguagem AOT,
reading-first, ARC + cycle collector, compilador Rust, runtime ori-runtime).

OBJETIVO
Estudar o código-fonte do Nim em `_references/nim-lang/` para extrair padrões
aplicáveis à Ori — corrigir bugs de memória e evoluir ARC/ORC — SEM copiar
código Nim e SEM herdar multi-GC ou sintaxe Nim.

CONTRATO ORI (leia antes de propor mudanças)
- docs/spec/00-manifesto.md
- docs/spec/10-memory.md
- docs/spec/19-abi.md
- docs/planning/language-direction-decisions-2026-06-30.md (Nim ORC > Pony)
- AGENTS.md (skills: clean-code, rust, compiler-dev, lang-compiled, ori-testing)

FONTES NIM (ordem)
1) doc/mm.md, doc/destructors.md
2) lib/system/arc.nim, orc.nim, cyclebreaker.nim
3) compiler/injectdestructors.nim, liftdestructors.nim
4) compiler/dfa.nim + cgen/ccg*.nim (só pontos de emissão de RC)
Cite commit: `git -C _references/nim-lang rev-parse --short HEAD`

ESPELHO ORI
- compiler/crates/ori-runtime/src/lib.rs (+ tests)
- compiler/crates/ori-codegen/src/native_backend/ (emit_arc_*)
- testes de contrato em native_backend/tests.rs (managed assignment, edges, async)

MÉTODO
Para o EIXO ou CENÁRIO que eu indicar:
1. Resumo Nim em linguagem simples (≤10 linhas)
2. Tabela Nim | Ori hoje | Gap
3. Riscos (leak, double-free, data race, regressão perf, carga cognitiva S3)
4. No máximo 3 ações, cada uma: mudança, arquivos Ori, teste, esforço S/M/L
5. O que NÃO fazer e por quê

REGRAS
- Não editar _references/nim-lang
- Não portar arquivos Nim para o tree Ori
- Preferir mudanças mínimas e testes de regressão
- Se for implementar: ori-testing (check/compile/run + cargo test)
- Saída no template de nota do arquivo
  `_references/PROMPT-analisar-nim-para-ori.md`

ESCOPO DESTA SESSÃO
(substitua) Eixo N / Cenário S# / Campanha C#
```

---

## Template de nota de estudo (preencher)

```markdown
# Nota Nim → Ori

- Data:
- Commit Nim:
- Eixo / Campanha:
- Autor/agente:

## 1. TL;DR (3 linhas)

## 2. O que o Nim faz (com trechos/caminhos)

## 3. O que a Ori faz hoje (caminhos)

## 4. Tabela delta

| Aspecto | Nim | Ori | Gap | Risco | Esforço |
|---------|-----|-----|-----|-------|---------|
| | | | | | |

## 5. Cenários (S1–S10) exercitados

| ID | Resultado (ok / leak / gap) | Evidência |
|----|-----------------------------|-----------|
| | | |

## 6. Ações propostas (máx 3)

### Ação 1 — título
- Por quê:
- Arquivos Ori:
- Teste:
- Fora de escopo:

## 7. Explicitamente NÃO fazer

## 8. Próxima campanha
```

Salvar notas em:

`docs/planning/historico/nim-study-YYYY-MM-DD-eixoN.md`

(ou continuar no mesmo arquivo se for série).

---

## Exemplo de primeira sessão (C0 + começo C1)

```text
ESCOPO DESTA SESSÃO: Campanha C0 + esboço Eixo 2 (destrutores).

Entregáveis:
1) Glossário de 15 termos Nim→Ori
2) Diagrama textual do caminho de destroy no Nim (arc.nim) vs Ori (__dtor + release)
3) Lista de 5 perguntas abertas para C1
Sem código ainda.
```

## Exemplo de sessão de implementação (depois do estudo)

```text
ESCOPO: implementar Ação 1 da nota docs/planning/historico/nim-study-….md
- Só essa ação
- Teste de regressão em ori-driver ou ori-runtime
- Atualizar docs/spec/10-memory.md se o contrato mudar
- CHANGELOG se user-facing/runtime behavior
```

---

## Glossário mínimo (preencher na C0)

| Termo Nim | Significado | Análogo Ori |
|-----------|-------------|-------------|
| cell / Refheader | header RC do objeto | header `ori_arc` |
| nimIncRef / DecRef | ±RC | `ori_arc_retain` / `release` |
| =destroy | dtor de tipo | `__dtor_*` + free |
| =trace | visita filhos p/ ORC | edges registradas |
| =wasMoved | zera após move | (gap / futuro) |
| =sink / move | transfer sem inc | (gap / elisão) |
| ORC | ARC + trial deletion | ARC + `collect_cycles` |
| injectdestructors | pass de inserção | emits em native_backend |
| expandArc | dump RC inserido | (gap DX) |
| acyclic | anotação otimiza ORC | (avaliar vs manifesto) |
| shared heap | … | … |
| critical link | … | … |

---

## Critérios de sucesso do programa de estudo

Você está “pronto para evoluir a Ori a partir do Nim” quando:

1. Consegue explicar ORC vs `ori_arc_collect_cycles` sem olhar o código.
2. Tem auditoria de edges para todos os tipos managed.
3. Tem pelo menos **3** gaps priorizados com teste reproduzível.
4. Nenhuma proposta exige multi-MM ou anotação pesada no usuário sem ADR.
5. Cada mudança implementada passa workspace tests relevantes e leak-check nos
   casos de memória.

---

## Referências rápidas

| Recurso | Path / URL |
|---------|------------|
| Clone local | `_references/nim-lang/` |
| README referências | `_references/README.md` |
| Nim mm | `_references/nim-lang/doc/mm.md` |
| Nim destructors | `_references/nim-lang/doc/destructors.md` |
| Ori memory | `docs/spec/10-memory.md` |
| Direção Nim ORC | `docs/planning/language-direction-decisions-2026-06-30.md` |
| Paper citado no orc.nim | Bacon concurrent RC / trial deletion (ver header do arquivo) |
