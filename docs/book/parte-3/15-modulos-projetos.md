# Cap. 15 — Módulos, imports e projetos

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Cada arquivo declara `module`. Imports: `import path = alias` ou seletivo.
Projeto multiarquivo exige `ori.proj` com `entry` (raiz-first, sem `src/`
obrigatório).

## Exemplo — dois arquivos + manifest

`ori.proj` (mínimo útil; veja o exemplo real do repo):

```text
manifest = 1
name = "greet_lab"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[source]
root_namespace = "app"
```

```ori
-- greeter.orl
module app.greeter

public hello(name: string) -> string
    return f"Hello, {name}!"
end
```

```ori
-- main.orl
module app.main

import app.greeter = greeter
import ori.io = io

main()
    io.println(greeter.hello("Ori"))
end
```

```bash
ori check .
ori run main.orl
```

(Referência completa: [`examples/multi_module`](../../../examples/multi_module/).)

Sem `ori.proj` / sem `entry`, `ori check` no diretório do projeto falha.

## Como funciona

### Imports

| Forma | Efeito |
|-------|--------|
| `import ori.io = io` | Apelido local `io` |
| `import ori.fs (read_text)` | Nomes selecionados |
| `import ori.math` | Só `ori.math.…` (sem apelido curto automático) |

`import ori.io` **sozinho** não cria o nome local `io`.

Há também bloco `imports … end` em alguns exemplos (vários aliases juntos). A forma
de linha `import path = alias` é a canônica do tour e deste livro.

### Visibilidade

`public` em itens que outros módulos do projeto devem importar (como `hello` acima).

### Projeto

```text
my_app/
  ori.proj
  main.orl
```

```bash
ori new my_app
ori run main.orl
```

Layout e contrato: [`../../spec/17-project-and-docs.md`](../../spec/17-project-and-docs.md).

## O que memorizar

- Path do import à esquerda do `=`.
- Prefira pais `ori.X`, não `.utils` em material novo.
- Multiarquivo → `ori.proj` com `entry`.

## Ir mais fundo

- Guia: [`../../guides/first-project.pt-BR.md`](../../guides/first-project.pt-BR.md)
- Exemplo: [`../../../examples/multi_module`](../../../examples/multi_module/)
