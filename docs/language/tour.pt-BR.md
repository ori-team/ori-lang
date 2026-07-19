# Tour da linguagem Ori (S3)

> **Público:** quem quer aprender a ler e escrever Ori  
> **English:** [tour.md](tour.md)  
> **Detalhe normativo:** [../spec/01-overview.md](../spec/01-overview.md)  
> **Superfície:** S3 `0.3.0` + inferência B `0.3.1` · construtores `ok`/`err`

Este tour reflete **o que o compilador aceita hoje**. Formas pré-S3 são erros
duros (`ori migrate-syntax` reescreve várias delas).

---

## 1. Programa completo e pequeno

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

| Ideia | Forma |
|-------|--------|
| Arquivo em um namespace | `module app.hello` na primeira linha |
| Import com nome curto | `import ori.io = io` (path **à esquerda**) |
| Entrada | `main()` — sem keyword `func` |
| Blocos | terminam com `end` |
| Tipos | explícitos na API pública / quando a inferência não basta |

---

## 2. Módulos e imports

```ori
import ori.fs (read_text, write_text)   -- seletivo
import ori.string = str                 -- alias
import ori.math                         -- só ori.math.…
```

- `import ori.io` **não** cria o nome local `io`.
- Prefira pais canônicos `ori.fs`, `ori.string` (não ensinar `.utils` como API nova).
- Aliases de domínio: `import ori.fs (TextResult)`.

---

## 3. Tipos do dia a dia

| Tipo | Significado |
|------|-------------|
| `int`, `float`, `bool`, `string`, `bytes` | primitivos / texto / binário |
| `list[T]`, `map[K, V]`, `set[T]` | coleções |
| `optional[T]` | valor ou `none` (sem null) |
| `result[T, E]` | `ok(T)` ou `err(E)` |
| `void` | sem valor útil |

Tipos compostos só com **`[]`**.

### Inferência local (opção B)

Em `const`/`var` **locais**, pode omitir o tipo se o lado direito for campo,
índice, chamada com retorno conhecido ou pipe `|>`.

---

## 4. Fluxo de controle

- `if` / **`elif`** / `else` (não `else if`)
- `while`, `for … in`
- `match` com `case ok(x):` / `case err(m):` (sem `.` em variantes de enum)
- `case padrão if condição:` — guard: se falso, cai para o próximo case;
  `case else:` é o fallback explícito

```ori
match score
    case n if n >= 90:
        io.println("A")
    case n if n >= 80:
        io.println("B")
    case else:
        io.println("C")
end

-- `match` também funciona como expressão: cada braço é um único valor
const nota: string = match score
    case n if n >= 90: "A"
    case else: "C"
end
```

---

## 5. Result e optional

```ori
load(path: string) -> result[string, string]
    return ori.fs.read_text(path)
end

main() -> result[void, string]
    const text: string = try load("notes.txt")
    io.println(text)
    return ok()
end
```

| Forma | Papel |
|-------|--------|
| `ok` / `err` | construir `result` |
| `some` / `none` | construir `optional` |
| `try expr` | propagar (única forma; sem `?`) |
| `if some(x) = expr` | ramificar na presença, ligando o valor |
| `if ok(v) = expr` / `if err(e) = expr` | ramificar num `result`, ligando qualquer um dos lados |

```ori
if some(user) = find_user(id)
    greet(user)
else
    io.println("não encontrado")
end

if ok(valor) = divide(10, 2)
    io.println(string(valor))
end

if err(motivo) = divide(1, 0)
    io.println(motivo)   -- entra quando o result NÃO deu ok
end
```

---

## 6. Structs, enums, traits

```ori
struct Point
    x: int
    y: int
end

const p: Point = Point { x: 1, y: 2 }

-- derivação: valor novo a partir de `p`; `p` fica intacto
const movido: Point = p with { x: 10 } end
```

Traits: **`apply Type`** + **`use Trait`** (não `implement … for`).
Importe o módulo do trait (`import ori.core = core`) e use
`use core.Displayable`. Conversão: `string(value)`, não método solto
`value.display()` fora do trait.

```ori
import ori.core = core
import ori.io = io

struct Point
    x: int
    y: int
end

apply Point
    use core.Displayable
        display(self) -> string
            return f"({self.x}, {self.y})"
        end
    end
end

main()
    const p: Point = Point { x: 1, y: 2 }
    io.println(string(p))
end
```

---

## 7. Funções

```ori
add(a: int, b: int) -> int
    return a + b
end

double(n: int) -> int => n * 2
```

Pipe `|>` permanece e é tipado como `f(value)`.

---

## 8. Projetos

```text
my_app/
  ori.proj     -- obrigatório
  main.orl     -- entrada recomendada
  docs/
```

```bash
ori new my_app
ori run main.orl
```

Guia: [Primeiro projeto](../guides/first-project.pt-BR.md).

---

## 9. O que não escrever (pré-S3)

| Evite | Use |
|-------|-----|
| `namespace` | `module` |
| `func name()` | `name()` |
| `import x as y` | `import path = y` |
| `success` / `error` | `ok` / `err` |
| `else if` | `elif` |
| `expr?` | `try expr` |

```bash
ori migrate-syntax caminho/
```

---

## 10. Async (nativo)

```ori
module app.main

import ori.io = io
import ori.task = task

async main()
    await task.sleep(10)
    io.println("pronto")
end
```

- `async main()` + `await` só dentro de funções `async`.
- Helpers: `fs.read_text_async`, `net.connect_async`, …
- Exemplo: [`examples/async_demo`](../../examples/async_demo/).
- Backend C/debug **rejeita** async (referência = nativo).

---

## 11. Próximos passos

| Objetivo | Doc |
|----------|------|
| Instalar package (Linux principal) | [../install.pt-BR.md](../install.pt-BR.md) |
| Receitas | [../guides/cookbook.pt-BR.md](../guides/cookbook.pt-BR.md) |
| Erros | [../guides/errors-null-void.pt-BR.md](../guides/errors-null-void.pt-BR.md) |
| Exemplos | [../../examples/](../../examples/) |
| Spec completa | [../spec/](../spec/README.md) (EN) |
