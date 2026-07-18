# Cap. 2 — O que é Ori

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

Ori é uma linguagem **compilada AOT**, tipada, com superfície de leitura **S3**,
runtime com **ARC**, codegen nativo (Cranelift) e **JIT** opcional em `ori run`.
Arquivos `.orl`, CLI `ori`, stdlib `ori.*`.

## Exemplo

```ori
module app.hello

import ori.io = io

main()
    io.println("Hello, Ori!")
    const answer: int = 21 * 2
    io.println(f"The answer is {answer}")
end
```

```bash
ori run main.orl
```

## Como funciona

| Camada | O que é |
|--------|---------|
| **Pele (S3)** | Ritmo de leitura: `module`, blocos `end`, tipos com `[]`, traits `apply`/`use` |
| **Motor** | Async, traits semânticos, ARC, backends nativo (+ C debug), stdlib |
| **Ferramenta** | Compilador em Rust; package de release sem exigir Rust no PATH do usuário |

Pipeline resumido:

```text
.orl → lexer → parser → resolver → types → HIR → Cranelift → binário ou JIT
```

Pré-1.0: a superfície **pode** mudar com disciplina; sintaxe pré-S3 é **erro duro**.

## O que memorizar

- Extensão `.orl`, comando `ori`, módulos stdlib `ori.X`.
- AOT para `compile`/`test`; JIT por padrão em `run` quando há cdylib.
- S3 desde `0.3.0`; inferência local opção B desde `0.3.1`.

## Ir mais fundo

- Overview normativo: [`../../spec/01-overview.md`](../../spec/01-overview.md)
- README do repo: [`../../../README.md`](../../../README.md)
- Cap. 6 — pipeline completo
