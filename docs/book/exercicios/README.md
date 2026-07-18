# Soluções sugeridas dos exercícios

> Enunciados: [Apêndice C](../apendices/C-exercicios.md).  
> Todos os trechos abaixo passaram por revisão com a superfície S3 em mente;
> rode `ori check` no seu ambiente ao adaptar.

## 1 — Hello pessoal

```ori
module app.ex1

import ori.io = io

main()
    const name: string = "Raillen"
    io.println(f"Hello, {name}!")
end
```

## 2 — Soma tipada

```ori
module app.ex2

import ori.io = io

add(a: int, b: int) -> int
    return a + b
end

main()
    io.println(f"{add(2, 3)}")
end
```

## 3 — Divisão segura

```ori
module app.ex3

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("zero")
    end
    return ok(a / b)
end

main()
    match divide(10, 0)
        case ok(v):
            io.println(f"{v}")
        case err(m):
            io.println(m)
    end
end
```

## 4 — Optional

```ori
module app.ex4

import ori.io = io

find(id: int) -> optional[string]
    if id == 1
        return some("ok")
    end
    return none
end

main()
    if some(name) = find(1)
        io.println(name)
    else
        io.println("missing")
    end
end
```

## 5 — Pipe

```ori
module app.ex5

import ori.io = io

double(n: int) -> int => n * 2
inc(n: int) -> int => n + 1

main()
    const v: int = 3 |> double |> inc
    io.println(f"{v}")
end
```

## 6 — Struct Point

```ori
module app.ex6

import ori.io = io

struct Point
    x: int
    y: int
end

main()
    const p: Point = Point { x: 1, y: 2 }
    io.println(f"{p.x},{p.y}")
end
```

## 7 — Dois módulos

Use o esqueleto do [Cap. 15](../parte-3/15-modulos-projetos.md) (`ori.proj` +
`greeter.orl` + `main.orl`) ou copie [`examples/multi_module`](../../../examples/multi_module/).

## 8 — Displayable

Espelhe o exemplo completo do [Cap. 16](../parte-3/16-traits.md).

## 9 — Ler arquivo

Espelhe o exemplo FS do [Cap. 17](../parte-3/17-stdlib-io-async-testes.md)
(crie `notes.txt` ao lado ou trate o `err`).

## 10 — Teste

Siga [`examples/tests_demo`](../../../examples/tests_demo/) e
[`../../guides/testing.pt-BR.md`](../../guides/testing.pt-BR.md).

## 11 — CLI args

Siga [`examples/cli_args`](../../../examples/cli_args/) (usa bloco `imports … end`
e `ori.args`).

## 12 — Diagnostic

```ori
module app.ex12

main()
    io.println("boom")
end
```

`ori check` deve apontar algo na família `name.*` (nome `io` indefinido).  
Depois corrija com `import ori.io = io`.
