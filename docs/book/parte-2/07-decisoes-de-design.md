# Cap. 7 — Decisões de design

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

Cada decisão grande da Ori responde a uma pergunta do leitor. Uma forma
canônica por conceito. Sem inferência global (HM). Traits e recursos explícitos.

## Exemplo

Checklist do manifesto antes de aceitar feature nova:

1. É visível na fonte onde o leitor precisa?
2. O contrato de tipo está legível no ponto de uso?
3. O erro ensina (código + ação)?
4. Há uma forma canônica simples?

## Como funciona

| Decisão | Por quê |
|---------|---------|
| `module` no topo | O arquivo declara onde mora |
| Tipos com `[]` | Uma gramática de tipos |
| `optional` / `result` | Ausência ≠ falha; ambos explícitos |
| Só `try` | Uma forma de propagar |
| `apply` + `use` | Origem do comportamento de trait visível |
| `using` | Cleanup determinístico e legível |
| Inferência **local** B | Menos ruído sem HM global |
| Pipe `\|\>` | Composição legível da esquerda para a direita |
| Diagnostics com código | Erros como mapa, não como xingamento |

O que **não** muda com a pele S3: poder do async, semântica forte de traits,
ARC, capacidade da stdlib, rejeição de HM global, propósito do manifesto.

## O que memorizar

- Uma forma canônica por conceito.
- Visível > mágico.
- Inferência local sim; HM global não.

## Ir mais fundo

- Manifesto checklist: [`../../spec/00-manifesto.md`](../../spec/00-manifesto.md)
- Specs `04`–`09`, `08-traits`
