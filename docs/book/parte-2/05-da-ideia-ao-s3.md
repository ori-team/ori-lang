# Cap. 5 — Da ideia à superfície S3

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

A Ori manteve o **motor** (async, traits, ARC, nativo) e trocou a **pele** para
S3 (ritmo Auk9). Em `0.3.0` o corte foi seco: pré-S3 morre. Em `0.3.1` veio
inferência local; o pipe `|>` ficou. Auk9 como produto foi arquivada.

## Exemplo

Norte em uma frase (registro de decisões):

> Motor e features da Ori; pele e ritmo da Auk9; superfície vivente na Ori.

## Como funciona

| Marco | O que fechou |
|-------|----------------|
| S3 / `0.3.0` | Sintaxe canônica; rejeição dura do pré-S3 |
| `0.3.1` | Inferência local Nim-style + opção B (campo/index/call/pipe) |
| Auk9 | Lab/referência — não é mais produto paralelo |
| Pipe `\|\>` | Mantido na Ori (Auk9 havia rejeitado) |

Exceções conscientes Ori vs Auk9 pura (resumo):

- Alias: `import ori.io = io` (path → apelido)
- If-expressão: `if cond then a else b`
- Closures: `(u) => …`
- Inferência local entregue; Auk9 v1 era mais rígida

Decisões detalhadas: blocos 0–9 em
[`../../planning/ori-surface-s3-auk9.md`](../../planning/ori-surface-s3-auk9.md).

## O que memorizar

- S3 = pele; Ori = motor.
- Corte seco em `0.3.0` — sem dual longo.
- Pipe e inferência local B fazem parte da Ori vivente.

## Ir mais fundo

- ADR: [`../../planning/adr-ori-surface-s3-auk9.md`](../../planning/adr-ori-surface-s3-auk9.md)
- CHANGELOG `[0.3.0]` / `[0.3.1]`: [`../../../CHANGELOG.md`](../../../CHANGELOG.md)
