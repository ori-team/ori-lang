# Interoperabilidade e ABI C

> **English:** [interop.md](interop.md)
> **Referência normativa:** [16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md) e [19-abi.md](../spec/19-abi.md)

Há duas direções de FFI. `extern` importa um símbolo nativo para o código Ori;
`@c_export` publica uma função `public` escolhida para que um host C a chame.

## Exportar uma biblioteca

```ori
module app.embed_add

@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end
```

Compile com:

```bash
ori compile --lib examples/embed/add_scores.orl -o libadd_scores.so
```

O compilador grava a biblioteca e um header C irmão. O host deve chamar
`ori_rt_init()` antes de usar a biblioteca e `ori_rt_shutdown()` ao terminar.

## Tipos aceitos

ABI-1 aceita escalares, `bool`, `void`, `string`, structs escalares não vazias e
não genéricas por wrappers pointer/out, structs gerenciadas por handles ARC
opacos e bridges diretos de `optional`/`result` sobre esses payloads.

`list`, `map`, `set`, `tuple`, unions aninhadas, structs genéricas e structs
vazias diretas são rejeitadas. Uma collection pode ficar dentro de uma struct
gerenciada, pois seu layout permanece privado.

Parâmetros gerenciados são emprestados. Retornos transferem uma referência ao
host, que deve liberá-la com `ori_arc_release`.

## Limites atuais

`@c_export` pertence ao backend nativo; o backend C/debug não é a referência de
ABI. Callbacks host→Ori e layouts diretos de collections continuam fora da
ABI-1. Os nomes exportados precisam ser identificadores portáveis de C/C++.

O caminho completo está em [`examples/embed`](../../examples/embed).
