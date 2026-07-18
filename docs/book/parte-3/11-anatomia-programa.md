# Cap. 11 — Anatomia de um programa

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Todo arquivo começa com `module`. A entrada é `main()` **sem** keyword `func`.
Blocos terminam com `end`. Imports usam `import path = alias` (path à esquerda).

## Exemplo

```ori
module app.hello

import ori.io = io

main()
    io.print("Hello, Ori Language!")
    const name: string = "Developer"
    io.print(f"Welcome, {name}!")
end
```

(Fonte: [`examples/hello/main.orl`](../../../examples/hello/main.orl).)

## Como funciona

| Peça | Forma canônica |
|------|----------------|
| Cabeçalho | `module app.hello` |
| Import com apelido | `import ori.io = io` |
| Import seletivo | `import ori.fs (read_text)` |
| Função | `name(params) -> T` / `main()` |
| Bloco | indentação + `end` |
| Constante / variável | `const x: T = …` / `var x: T = …` |
| Comentário | `-- …` |

### Inferência local (opção B)

Em `const`/`var` **locais**, o tipo pode ser omitido se o lado direito for
literal óbvio, campo, índice, chamada com tipo conhecido ou pipe `|>`.

Na API pública (assinaturas), prefira tipos explícitos — o leitor agradece.

### Formas que **não** existem mais (pré-S3)

`func`, `namespace`, `else if`, `?`, `import as` antigo, etc. → erro duro.  
Migração mecânica: `ori migrate-syntax`.

## O que memorizar

- `module` na primeira linha.
- Sem `func`; blocos com `end`.
- `import path = alias` — path à esquerda.

## Ir mais fundo

- Tour: [`../../language/tour.pt-BR.md`](../../language/tour.pt-BR.md)
- Spec statements: [`../../spec/06-statements.md`](../../spec/06-statements.md)
- Cap. 15 — módulos e projetos
