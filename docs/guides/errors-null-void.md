# Errors, Null, Void — mapa mental Ori

> Guia pedagógico. Normativo: caps. [04-expressions](../spec/04-expressions.md) e [09-types](../spec/09-types.md).

## Quatro conceitos, quatro papéis

| Conceito | Papel | Quando usar |
|----------|--------|-------------|
| **`void`** | “Nada útil retornado” | Tipo de retorno de `func main() -> void`, side effects only |
| **`optional<T>`** | “Pode não haver valor” | Parsing, lookup, EOF — ausência **não** é erro |
| **`result<T, E>`** | “Sucesso ou falha explícita” | I/O, validação, APIs que podem falhar com motivo |
| **`check`** | Pré-condição / contrato | Invariantes em runtime (`check cond, "msg"`) |

Ori **não tem null**. Use `none` dentro de `optional<T>` ou `error(...)` dentro de `result<T, E>`.

## `void`

```lua
func greet() -> void
    io.println("hello")
end
```

- Não confunda com `optional`: `void` não é um valor que você armazena.
- Descartar `result` sem tratar emite `type.unused_result` (warning).

## `optional<T>`

```lua
func find_user(id: int) -> optional<string>
    if id == 0
        return none
    end
    return success("alice")
end
```

- `none` = ausência esperada.
- `if some x in expr` desempacota com segurança.
- `try valor` em `optional` propaga `none` (ver cap. 09).
- `valor?` existe como forma compacta de `try valor`.

## `result<T, E>`

```lua
func read_config(path: string) -> result<string, string>
    return fs.read_text(path)
end
```

- `success(value)` ou `error(reason)`.
- Trate com `match`, `try`, `?`, ou Layer 2 helpers (`parse_int_or`, `get_or`).
- Preferir `result` a `bool` para operações que falham (migração FS em andamento — ver `PENDENTES.md`).

## `check`

```lua
func divide(a: int, b: int) -> int
    check b != 0, "division by zero"
    return a / b
end
```

- Falha de contrato aborta com diagnóstico runtime (não é `result`).
- Diferente de `if`: `check` documenta invariantes.

## Comparativo rápido

| Situação | Tipo / construção |
|----------|-------------------|
| Função só imprime | `-> void` |
| “Não achei” sem erro | `optional<T>` + `none` |
| Falha com mensagem | `result<T, string>` |
| Pré-condição que deve ser verdade | `check` |

## Ferramentas DX

```bash
ori explain name.undefined
ori explain type.type_mismatch
ori doctor
ori summary path/to/main.orl
```

Ver catálogo completo: [`13-error-catalog.md`](../spec/13-error-catalog.md).
