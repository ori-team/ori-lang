# Cap. 17 — Stdlib, I/O, async e testes

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Stdlib canônica: `ori.X`. Layer 1 (Rust/runtime) para hot path; Layer 2/3 em
`.orl`. I/O e FS via imports explícitos. Async no backend nativo. Testes com
`ori test`.

## Exemplo — FS + `result`

```ori
module app.main

import ori.fs = fs
import ori.io = io

main() -> result[void, string]
    const text: string = try fs.read_text("notes.txt")
    io.println(text)
    return ok()
end
```

## Exemplo — teste mínimo

```ori
module app.main

import ori.test = test

@test
math_is_stable()
    test.assert(1 + 1 == 2, "math should work")
end
```

```bash
ori test
```

(Com `ori.proj` + `entry`. Ver também [`examples/tests_demo`](../../../examples/tests_demo/).)

## Como funciona

### Camadas

| Layer | Onde | Papel |
|-------|------|--------|
| 1 | Runtime Rust + manifest | FFI, ARC, FS/net quentes |
| 2/3 | `stdlib/*.orl` | Ergonomia e algoritmos em Ori |

API pública nova: só em `ori.X`. Paths `ori.X.utils` são compat silenciosa —
não ensinar como API nova.

### Domínios úteis

`ori.io`, `ori.fs`, `ori.string`, `ori.bytes`, `ori.net`, `ori.os`, `ori.process`,
`ori.json`, `ori.test`, … — índice no Cap. 20.

### Async (nativo)

- Entrada: `async main()` quando o programa é assíncrono.
- APIs awaitable: sufixos `*_async` / futures da stdlib (FS/net conforme spec).
- Backend C/debug **rejeita** async — use o caminho nativo.
- Rede (`examples/http_get`) precisa de ambiente/rede real; não trate como unitário puro.

### `using`

Para recursos com cleanup determinístico e visível — ver
[`examples/using_fs`](../../../examples/using_fs/) e a spec de memória/erros.

### Exemplos no repo

| Exemplo | Tema |
|---------|------|
| `using_fs` / `path_time_io` | FS e tempo |
| `async_demo` / `concurrency` | Async / concorrência |
| `tests_demo` | Testes |
| `http_get` | Rede (avançado) |

### Mantenedor vs usuário

| Você é… | Como testar |
|---------|-------------|
| App Ori | `ori test` no projeto |
| Compilador | `cargo test` + testes `ori-driver` |

## O que memorizar

- `import ori.X = alias` — pais canônicos.
- Layer 1 Rust é design permanente, não “dívida”.
- Async = backend nativo; rede = ambiente real.

## Ir mais fundo

- Spec stdlib: [`../../spec/12-stdlib.md`](../../spec/12-stdlib.md)
- [`../../../stdlib/README.md`](../../../stdlib/README.md)
- Testing: [`../../guides/testing.pt-BR.md`](../../guides/testing.pt-BR.md)
- Cap. 20 — índice de consulta
