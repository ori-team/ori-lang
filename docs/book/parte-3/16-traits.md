# Cap. 16 — Traits e comportamento explícito

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Comportamento compartilhado vem de traits. Sintaxe: `apply Type` com seções
`use Trait`. Importe o módulo do trait (`ori.core`). Sem `implement … for`.

## Exemplo

```ori
module app.points

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

## Como funciona

- **Pele S3, motor Ori:** traits de verdade (bounds, monomorph, defaults por corpo).
- `apply Type` agrupa as implementações daquele tipo.
- `use Trait` declara qual contrato está sendo preenchido.
- Conversão comum: `string(value)` via `Displayable`, não método solto mágico fora do trait.
- Default de método: corpo na definição do trait (sem keyword `default` da Auk9).

## O que memorizar

- `apply` + `use`, não `implement`.
- Importe `ori.core` (ou o módulo dono do trait).
- Origem do comportamento fica visível na fonte.

## Ir mais fundo

- Spec: [`../../spec/08-traits.md`](../../spec/08-traits.md)
- Tour §6: [`../../language/tour.pt-BR.md`](../../language/tour.pt-BR.md)
