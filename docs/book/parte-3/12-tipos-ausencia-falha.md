# Cap. 12 — Tipos, ausência e falha

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Não há null. Ausência é `optional[T]` (`some`/`none`). Falha é `result[T, E]`
(`ok`/`err`). Propagação só com `try expr`. Trate com `match` (ou `if some`).

## Exemplo — `result` + `match`

```ori
module app.errors

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("Division by zero")
    end
    return ok(a / b)
end

main()
    match divide(10, 2)
        case ok(val):
            io.print(f"ok: {val}")
        case err(msg):
            io.print(f"err: {msg}")
    end
end
```

(Ideia de [`examples/error_handling`](../../../examples/error_handling/).)

## Exemplo — `optional` + `try`

```ori
module app.opt

import ori.io = io

find_user(id: int) -> optional[string]
    if id == 0
        return none
    end
    return some("alice")
end

greet(id: int) -> optional[string]
    const name: string = try find_user(id)
    return some(f"hi {name}")
end

main()
    match greet(1)
        case some(msg):
            io.println(msg)
        case none:
            io.println("missing")
    end
end
```

`try` em `optional` propaga `none`. Em `result`, propaga `err`.

## Como funciona

| Conceito | Papel | Construtores |
|----------|--------|----------------|
| `void` | Sem valor útil | — |
| `optional[T]` | Pode faltar | `some` / `none` |
| `result[T, E]` | Sucesso ou falha | `ok` / `err` |
| `try expr` | Propaga `none` ou `err` | única forma (sem `?`) |
| `check` | Invariante em runtime | aborta se falhar |

Tipos compostos usam **`[]`**: `list[int]`, `map[string, int]`, `result[T, E]`.

### Tratar vs propagar

| Objetivo | Forma |
|----------|--------|
| Tratar no lugar | `match` · `if some(x) = expr` |
| Subir para o caller | `try expr` (função deve retornar `optional` ou `result` compatível) |
| Invariante “nunca deveria falhar” | `check cond, "msg"` |

### Escolha rápida

| Situação | Use |
|----------|-----|
| Busca / EOF | `optional[T]` |
| I/O, validação | `result[T, E]` |
| Efeito sem valor | `void` |
| “Isso nunca pode acontecer” | `check` |

### Armadilhas comuns

- Usar `result` para “não achei” e `optional` para I/O — inverte os significados.
- Esperar `?` pós-fix — removido no S3.
- Esquecer que `main() -> result[…, …]` é necessário se `try` sobe até a entrada.

## O que memorizar

- Sem null → `optional` ou `result`.
- Construtores: `ok`/`err`, `some`/`none`.
- Só `try` para propagar; `match` (ou `if some`) para tratar.

## Ir mais fundo

- Guia: [`../../guides/errors-null-void.pt-BR.md`](../../guides/errors-null-void.pt-BR.md)
- Spec: [`../../spec/09-errors.md`](../../spec/09-errors.md), [`../../spec/04-types.md`](../../spec/04-types.md)
- Exemplo: [`../../../examples/error_handling`](../../../examples/error_handling/)
