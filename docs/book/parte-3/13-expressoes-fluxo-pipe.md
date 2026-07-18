# Cap. 13 — Expressões, fluxo e pipe

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Fluxo: `if` / `elif` / `else`, `while`, `for … in`, `match`. If-expressão:
`if cond then a else b`. Pipe `|>` tipa como `f(value)` e encadeia da esquerda
para a direita.

## Exemplo — fluxo + pipe

```ori
module app.flow

import ori.io = io

double(n: int) -> int => n * 2

main()
    const n: int = 3
    if n > 2
        io.println("big")
    elif n == 2
        io.println("two")
    else
        io.println("small")
    end

    const label: string = if n > 0 then "pos" else "non-pos"
    const doubled: int = n |> double
    io.println(f"{label} {doubled}")
end
```

## Exemplo — encadeamento

Pipe aceita várias etapas (como em [`examples/language_features`](../../../examples/language_features/)):

```ori
-- ideia: 5 |> double |> add_ten |> to_label
const label: string = 5 |> double |> add_ten |> to_label
```

Cada etapa é uma função que recebe o valor à esquerda como **primeiro** argumento.

## Como funciona

| Forma | Notas |
|-------|--------|
| `if` / `elif` / `else` | Não use `else if` |
| `while cond` … `end` | Loop condicional |
| `for x in xs` … `end` | Iteração |
| `match` | `case ok(x):` / `case err(m):` / enums |
| `if c then a else b` | Expressão (valor) |
| `a \|> f` | Equivale a `f(a)` |
| `a \|> f \|> g` | `g(f(a))` |

### Inferência local B

Campo, índice, chamada e pipe no RHS de `const`/`var` local permitem omitir o tipo
quando o retorno já é conhecido.

### O que evitar

| Evite | Use |
|-------|-----|
| `else if` | `elif` |
| Pipe com closure inventada sem testar | Função nomeada (`double`) |
| Confundir pipe com método encadeado estilo OO | É só açúcar de chamada |

## O que memorizar

- `elif`, nunca `else if`.
- Pipe = primeiro argumento à esquerda; encadeia com vários `|>`.
- Preferir `match` para `result`/`optional` ramificados.

## Ir mais fundo

- Spec: [`../../spec/05-expressions.md`](../../spec/05-expressions.md)
- Exemplo: [`../../../examples/language_features`](../../../examples/language_features/)
