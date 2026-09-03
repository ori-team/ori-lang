# Ori — Manifesto

> Status: normativo (identidade e propósito)  
> Audience: mantenedores, contribuidores, leitores da linguagem  
> Superfície canônica: **S3** (Auk9-inspired) a partir de `0.3.0`  
> Registro de decisões: [`docs/planning/ori-surface-s3-auk9.md`](../planning/ori-surface-s3-auk9.md)  
> ADR: [`docs/planning/adr-ori-surface-s3-auk9.md`](../planning/adr-ori-surface-s3-auk9.md)

---

## Em uma frase

Ori é uma linguagem **compilada AOT**, tipada e legível, com features de linguagem
“de verdade”, superfície de leitura no estilo poema (**S3**), feita para **estudar
compiladores**, **testar programação assistida por IA** e **ler código com menos
carga cognitiva** — **não** para disputar o mercado de linguagens.

---

## O que a Ori não é

A Ori **não** visa competir com linguagens de mercado como produto industrial.
Não há promessa de “substituir Rust, Go, TypeScript, …” nem de capturar market share.

Uso real em projetos **pequenos e médios** e maturidade de features existem como
**laboratório sério** — não como pitch comercial.

---

## Para que a Ori existe

1. **Estudo** de compiladores e design de linguagens  
2. **Explorar limites** da programação assistida por IA (humano + agente no mesmo código)  
3. **Legibilidade** e acessibilidade — em especial neurodivergência (TDAH, dislexia, autismo, etc.)

*ori* (אוֹרִי) — hebraico para “minha luz.”

---

## Identidade de superfície (S3)

| Camada | Decisão |
|--------|---------|
| **Motor / features** | Caminho Ori: async, traits semânticos, runtime ARC, codegen nativo, JIT, stdlib |
| **Pele / ritmo de leitura** | Superfície **S3**, o mais próximo possível da Auk9 (lab), com exceções documentadas |
| **Arquivos / CLI** | Extensão **`.orl`**, CLI **`ori`**, stdlib **`ori.*`** |
| **Corte** | **Seco** no artefato `0.3.0` — forma antiga deixa de ser aceita |
| **Inferência local (Nim-style + opção B)** | **Entregue** — omissão em `const`/`var` locais em RHS óbvio (literal, campo, index, call, pipe); **sem** HM global nem inferência “pelo uso depois” |
| **Pipe `\|\>`** | **Mantido** na Ori (feature de primeira classe; Auk9 rejeitou, Ori não) |
| **Auk9** | **Produto arquivado** — lab/referência de sintaxe; superfície vivente na Ori |

Exceções conscientes vs Auk9 pura (ADR):

| Tópico | Ori-S3 | Auk9 |
|--------|--------|------|
| Alias de import | `import ori.io = io` (path → apelido) | `import io = ori.io` |
| If-expressão | `if cond then a else b` | if-expr só em `=>` |
| Default em trait | corpo = default (sem keyword `default`) | keyword `default` |
| Closure | `(u) => …` | `do(u) => …` |
| Inferência | Nim-local + **opção B** (`0.3.1`+) | anotações rígidas na v1 Auk9 |
| Pipe `\|\>` | **mantido** | rejeitado na Auk9 |

---

## O que a Ori otimiza

Ori otimiza para **leitura**, não para digitação máxima.

Um programa é lido muitas vezes mais do que é escrito. Cada leitura deve custar menos:

- **Onde o arquivo pertence** — `module path` no topo de cada arquivo  
- **O que cada valor é** — tipos explícitos onde o contrato de leitura importa  
- **Onde a ausência pode acontecer** — `optional[T]`  
- **Onde a falha pode acontecer** — `result[T, E]`  
- **Quando o recurso é liberado** — `using` visível e determinístico  
- **De onde vem o comportamento de trait** — `apply Type` + `use Trait` explícitos  

---

## Checklist de feature (norma)

Antes de aceitar sintaxe ou feature nova, responder:

1. **É visível na fonte** onde o leitor precisa?  
2. **O tipo / contrato** está legível no ponto de uso (ou via alias honesto)?  
3. **O erro ensina** — código de diagnóstico + ação clara?  
4. **Há uma forma canônica** — a mais simples que cobre o caso, sem dual longo?

Uma forma canônica por conceito é **norma** (superfície S3 + reforma documental).

---

## O que não muda com o S3 (fora de escopo de “pele”)

- Async / await, channels, cancel tokens  
- Poder semântico dos traits Ori (bounds, monomorph, defaults por corpo)  
- Runtime ARC, backends nativo/C, capacidade da stdlib  
- Rejeição de inferência global (HM)  
- Propósito acima: estudo, IA, legibilidade ND — **não** competição de mercado  

---

## Migração 0.2 → 0.3

- Script melhor-esforço: `ori migrate-syntax` (wrapper `tools/migrate_syntax.sh`)  
- Escopo de produto: **linguagem + stdlib + docs + performance + DX local** (editores). Distribuição multi-OS e lojas de extensão ficam **depois**.
- Docs de produto: **inglês canônico no GitHub** + **português em paralelo**
  (`docs/README.md`)  
- Lista breaking completa: [`CHANGELOG.md`](../../CHANGELOG.md) seção `[0.3.0]`  
- Catálogo de erros de forma removida: [`13-error-catalog.md`](13-error-catalog.md)  

---

## Relação com a especificação normativa

| Documento | Papel |
|-----------|--------|
| **Este manifesto** | Identidade, propósito, norte de superfície S3 |
| `01-overview.md` … | Contrato de linguagem (superfície **S3** em `0.3.0`) |
| `13-error-catalog.md` | Códigos **emitidos** (incl. rejeições de forma pré-S3) |
| `docs/planning/ori-surface-s3-auk9.md` | Tabela completa de decisões 0–9 |
| `docs/planning/historico/pr-plan-ori-surface-s3.md` | DAG de implementação (PRs 1–10 = 0.3.0; PR 11 = 0.3.1; opção B pós-PR11) |

Em conflito de **propósito / identidade**, prevalece este manifesto + o registro S3 + o ADR.
Em conflito de **sintaxe canônica**, prevalecem os capítulos `01`–`11` alinhados ao S3 e o compilador.
