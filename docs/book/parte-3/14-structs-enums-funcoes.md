# Cap. 14 — Structs, enums e funções

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Structs e enums declaram dados; literais usam `Type { campo: valor }`. Funções
não usam `func`. Corpo curto pode usar `=>`. Generics usam `[]`.

## Exemplo

```ori
module app.points

import ori.io = io

struct Point
    x: int
    y: int
end

add(a: int, b: int) -> int
    return a + b
end

double(n: int) -> int => n * 2

main()
    const p: Point = Point { x: 1, y: 2 }
    io.println(f"{p.x},{p.y} sum={add(p.x, p.y)} dup={double(p.x)}")
end
```

## Como funciona

### Structs

- Campos tipados; literal nomeado `Type { … }`.
- Não há “struct call” estilo construtor posicional genérico pré-S3.

### Enums

- Variantes; campos nomeados usam vírgulas dentro de parens (ver spec).
- `match` sem `.` nas variantes no padrão S3 do tour.

### Funções

| Forma | Uso |
|-------|-----|
| `name(params) -> T` + `end` | Corpo normal |
| `name(params) -> T => expr` | Corpo expressão |
| `main()` | Entrada |

Generics: ver [`../../spec/11-generics.md`](../../spec/11-generics.md) — tipos com `[]`.

Coleções do dia a dia: `list[T]`, `map[K, V]`, `set[T]` — ver
[`examples/collections_demo`](../../../examples/collections_demo/).

## O que memorizar

- Literal: `Type { f: v }`.
- Sem keyword `func`.
- `=>` para corpo curto.

## Ir mais fundo

- Spec funções: [`../../spec/07-functions.md`](../../spec/07-functions.md)
- Tour §6–7: [`../../language/tour.pt-BR.md`](../../language/tour.pt-BR.md)
