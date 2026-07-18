# Cap. 3 — Para que existe (e para que não)

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

Ori existe para **estudar compiladores**, **explorar programação com IA** no
mesmo código e **reduzir carga cognitiva na leitura**. Não existe para
substituir Rust, Go ou TypeScript no mercado.

## Exemplo

Três perguntas. Se a resposta for “sim” nas três, você está no público certo:

1. Dá para ensinar um pipeline real de compilador com este repo?
2. Dá para um humano e um agente trabalharem no mesmo codebase com regras claras?
3. O código deixa contratos (falha, ausência, módulo) visíveis ao leitor?

## Como funciona

### Os três pilares

| Pilar | O que significa na prática |
|-------|----------------------------|
| **Estudo** | Lexer, tipos, codegen, runtime e stdlib no mesmo monólito — lab sério, não toy |
| **Humano + IA** | Skills, `AGENTS.md`, testes e CHANGELOG como contrato de sessão |
| **Legibilidade ND** | Menos regras ocultas; diagnostics com código; contratos na fonte |

*ori* (אוֹרִי) — hebraico para “minha luz.” O nome aponta para clareza, não para marketing.

### Três leitores típicos

1. **Estudante de compiladores** — quer ver AOT de verdade (Cranelift, link, ARC), não só AST de brinquedo.
2. **Quem programa com agentes** — quer limites explícitos: o que a IA pode mudar e o que é norma.
3. **Quem se cansa de código opaco** — quer `optional`/`result`/`module` visíveis sem caçar convenções.

### Para que *não* serve

| Não prometa | Por quê |
|-------------|---------|
| “Substitui Rust/Go/TS” | Fora do manifesto |
| Estabilidade 1.0 hoje | Ainda é `0.3.x` (FREEZE-1 com disciplina) |
| Foco em loja de extensão / marketing multi-OS | Produto agora: linguagem, stdlib, docs, performance, DX local |
| Self-hosting como próximo passo | M4 é a **última** discussão tática |

Uso em projetos **pequenos e médios** é bem-vindo como laboratório — não como campanha comercial.

### Como o livro usa esses pilares

- Parte I = por quê  
- Parte II = como o lab é construído (inclui Cap. 9 sobre IA)  
- Parte III = escrever Ori legível  
- Parte IV = consultar sem reabrir a spec inteira  

## O que memorizar

- Três pilares: estudo · IA · legibilidade ND.
- Anti-pitch: sem guerra de market share.
- Nome = luz = clareza na leitura.

## Ir mais fundo

- Manifesto: [`../../spec/00-manifesto.md`](../../spec/00-manifesto.md)
- Cap. 1 — problema da leitura  
- Cap. 9 — IA no desenvolvimento  
- Cap. 22 — estabilidade e limites  
