# ADR — Ori Surface S3 (sintaxe Auk9-inspired)

**Status:** Aceito  
**Data:** 2026-07-12  
**Versão de quebra:** `0.3.0` (superfície + docs) · `0.3.1` (inferência local) · opção B (campo/index/call/pipe) pós-0.3.1  
**Registro vivo de decisões:** [`ori-surface-s3-auk9.md`](ori-surface-s3-auk9.md)  
**Plano de implementação /execute-plan:** [`pr-plan-ori-surface-s3.md`](historico/pr-plan-ori-surface-s3.md)

---

## Contexto

A Ori (`0.2.x`) tem **features e maturidade** de linguagem compilada AOT (async,
traits, stdlib, LSP, runtime). A Auk9 explorou uma **superfície de leitura**
(ritmo visual / “poema”) que queremos na Ori, **sem** reimplementar o motor na
Auk9 e **sem** competir como produto de mercado.

Decisão de produto: **aposentar a Auk9 como linguagem/produto**; absorver a
pele na Ori; Auk9 permanece lab de referência até o corte.

---

## Decisão

Adotar a **superfície S3** na Ori:

| Camada | Ação |
|--------|------|
| Features / runtime / async / traits **semântica** | **Manter caminho Ori** |
| Gramática e ritmo de leitura | **O mais próximo possível da Auk9**, com exceções documentadas |
| Release | **Corte seco** no `0.3.0` publicado (forma antiga = erro) |
| Dev interno | Fases + script de migração; dual **apenas** em branch de trabalho se útil |
| Identidade | `.orl`, CLI `ori`, stdlib `ori.*` |

Exceções conscientes vs Auk9 pura:

| Tópico | Ori-S3 | Auk9 |
|--------|--------|------|
| Alias de import | `import ori.io = io` (path → apelido) | `import io = ori.io` |
| If-expressão | `if cond then a else b` (Ori) | if-expr só em `=>` |
| Default em trait | corpo = default (sem keyword `default`) | keyword `default` |
| Closure | `(u) => …` | `do(u) => …` |
| Inferência | Nim-local + **opção B** (`0.3.1`+) | anotações rígidas na v1 Auk9 |
| Pipe `\|>` | **mantido** (feature Ori; tipado como call; teste nativo) | rejeitado na Auk9 |

---

## Propósito da linguagem (normativo para manifesto)

A Ori **não** visa competir com linguagens de mercado. Existe para:

1. Estudo de compiladores e design de linguagens  
2. Explorar limites da programação assistida por IA  
3. Legibilidade / neurodivergência (carga cognitiva baixa)

---

## Consequências

### Positivas

- Uma linguagem só (Ori) com cara de poema e motor maduro  
- Documentação e exemplos alinhados a **uma forma canônica por conceito**  
- Auk9 deixa de consumir esforço de paridade de features  

### Negativas / custos

- Breaking change grande (`0.3.0`)  
- Migração de stdlib, testes, examples (game/imgui **cancelados** / fora do produto)  
- Risco de bugs de parser em poetic call / struct-vs-map  

### Mitigações

- Script `ori migrate-syntax` (melhor esforço)  
- PR plan fatiado para `/execute-plan`  
- Inferência local **fora** do big-bang (`0.3.1`); opção B formalizada depois  
- Checklist de pronto antes da tag  

---

## Fora de escopo deste ADR

- Self-hosting, registry remoto  
- Inferência global HM (continua proibida); opções C/D de inferência “pelo uso”  
- Migrar `ori-game` / `ori-imgui` (plano antigo): **cancelado** — fora do produto  
- Reimplementar features Ori na Auk9  
- Package de distribuição (adiado até fechar pendências de runtime/stdlib/LSP)

---

## Referências

- `docs/planning/ori-surface-s3-auk9.md` — tabela completa de decisões 0–9  
- `docs/planning/historico/pr-plan-ori-surface-s3.md` — DAG de PRs  
- `auk9-lang` — lab de sintaxe (read-only)  
- Spec Ori atual: `docs/spec/*` (será reescrita no 0.3.0)  
