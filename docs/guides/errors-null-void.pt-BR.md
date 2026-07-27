# Erros, optional e void — mapa mental

> Guia pedagógico (**S3 / 0.3.2**).  
> **English:** [errors-null-void.md](errors-null-void.md)  
> Normativo: [09-errors](../spec/09-errors.md), [04-types](../spec/04-types.md)

## Quatro conceitos

| Conceito | Papel | Quando |
|----------|--------|--------|
| **`void`** | Sem valor útil de retorno | Funções de efeito |
| **`optional[T]`** | Valor pode faltar | Busca, EOF — ausência ≠ falha |
| **`result[T, E]`** | Sucesso ou falha com motivo | I/O, validação |
| **`check`** | Pré-condição em runtime | Invariantes |

Ori **não tem null**. Use `none` ou `err(...)`.

## `void`

```ori
module app.main

import ori.io = io

greet() -> void
    io.println("olá")
end

main()
    greet()
end
```

## `optional[T]`

```ori
module app.main

find_user(id: int) -> optional[string]
    if id == 0
        return none
    end
    return some("alice")
end

main()
    match find_user(1)
        case some(name):
            -- use name
        case none:
    end
end
```

- Desempacote com `if some(x) = expr` ou `match`.
- `try` em optional propaga `none`.
- Postfix `?` foi **removido** no S3.

## `result[T, E]`

```ori
module app.main

import ori.fs = fs
import ori.io = io

read_config(path: string) -> result[string, string]
    return fs.read_text(path)
end

main()
    match read_config("app.conf")
        case ok(text):
            io.println(text)
        case err(msg):
            io.eprintln(msg)
    end
end
```

- Construtores: **`ok` / `err`** (não `success` / `error`).
- Trate com `match` ou **`try expr`**.

## `check`

```ori
module app.main

divide(a: int, b: int) -> int
    check b != 0, "division by zero"
    return a / b
end
```

Quebra o processo se o contrato falhar; não é um `result`.

## Mapa rápido

| Situação | Use |
|----------|-----|
| Só efeito | `-> void` |
| “Não achou”, sem erro | `optional[T]` |
| Falha com mensagem | `result[T, string]` |
| Sempre deve ser verdade | `check` |

```bash
ori explain name.undefined
ori doctor
```

Catálogo: [13-error-catalog.md](../spec/13-error-catalog.md).
