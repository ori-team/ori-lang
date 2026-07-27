# Cap. 16 — Traits e comportamento explícito
> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR
Traits definem contratos de comportamento (o que um tipo *pode* fazer). Para que um struct tenha métodos associados a ele, usamos blocos `apply`, conectando a estrutura de dados à funcionalidade, deixando a intenção clara.

## O que são Traits?

Em Ori, as *Structs* contêm os dados, enquanto as *Traits* (características) e os métodos definem o comportamento.

Você declara uma trait para definir quais métodos um tipo deve implementar:

```ori
module app.graphics

-- Define a trait "Drawable" (algo que pode ser desenhado)
trait Drawable
  draw(self)
end
```

Traits podem, opcionalmente, conter métodos com **corpos padrão** (default method bodies), que serão usados se você não fornecer um método específico.

## O Padrão "Apply + Use"

No Ori, para adicionar métodos a um tipo, usamos a palavra-chave `apply`. Para dizer que estamos cumprindo o contrato de uma trait, colocamos `use Trait` dentro desse bloco.

```ori
struct Circle
  radius: float
end

-- Uma trait só: use o cabeçalho compacto (o 'use' na MESMA linha do 'apply')
apply Circle use Drawable
  -- Aqui implementamos o método exigido
  draw(self)
    -- código para desenhar um círculo
  end
end
```

> **Quando é uma trait só, essa é a única forma aceita.** Escrever o `use`
> numa linha separada, sozinho no bloco, dá erro (`apply.redundant_use_block`)
> — seria a mesma coisa com um nível de indentação a mais. O compilador
> escolhe a forma pelo conteúdo; você nunca decide entre duas.
>
> Já tem código na forma antiga? `ori migrate-syntax` reescreve sozinho.

Quando há **mais de uma trait** — ou métodos próprios junto com a trait — aí
sim a forma aninhada é a exigida, porque o cabeçalho compacto não conseguiria
expressar isso:

```ori
apply Circle
  use Drawable
    draw(self)
      -- ...
    end
  end

  use Comparable
    -- ...
  end
end
```

## Métodos Inerentes (Sem Trait)

Você não precisa de uma trait para tudo. Você pode adicionar métodos próprios e diretos à struct, que são chamados de **métodos inerentes**.

```ori
struct Point
  x: float
  y: float
end

apply Point
  -- Não há "use" aqui, apenas funções normais
  distance(self) -> float
    -- calcula distância
    return 0.0
  end
end
```

### Mutação (A palavra-chave 'mut')

Por padrão, parâmetros são imutáveis. Se um método precisar alterar os dados internos da struct, você **deve** usar a palavra-chave `mut` antes do nome da função e usar `mut self` não é necessário, o compilador entende pela assinatura `mut`.

```ori
apply Point
  -- Este método modifica o estado interno da struct
  mut increment(self)
    self.x = self.x + 1.0
  end
end
```

## Method Bind (Vinculação de Métodos)

Às vezes você já tem uma função pronta fora da struct e só quer "plugar" (vincular) essa função em uma trait. O Ori permite isso facilmente sem precisar reescrever o código.

```ori
-- Função solta (free function)
compare_circles(a: Circle, b: Circle) -> int
  -- logica de comparação
  return 0
end

apply Circle use Comparable
  -- Vincula o método 'compare' exigido pela trait à função solta
  compare = compare_circles
end
```

## Dynamic Dispatch (Polimorfismo em Tempo de Execução)

O Ori suporta chamadas dinâmicas (dynamic dispatch) usando o tipo `any[Trait]`. Isso é útil quando você tem uma lista que mistura diferentes tipos (ex: `Circle` e `Square`), desde que todos implementem a mesma Trait.

```ori
-- Uma lista de coisas genéricas que podem ser desenhadas
const shapes: list[any[Drawable]] = [
  Circle { radius: 1.0 },
  Square { side: 2.0 }
]

for s in shapes
  -- O Ori descobre em tempo de execução qual draw() chamar
  s.draw()
end
```

> **Atenção — `any[Trait]` não "vira" o tipo concreto de volta.** Mesmo com
> `if s is Circle`, dentro do bloco `s` continua sendo `any[Drawable]`: você
> só pode chamar métodos que a trait declara (`draw()`), não campos do
> `Circle` real (`s.radius` dá erro). O `is` aqui serve para decidir *ramos*
> de código, não para "destravar" a struct escondida atrás da trait.

## Restrições de Traits em Genéricos (Trait Constraints)

Se você usar genéricos, pode exigir que o tipo fornecido obedeça a uma trait. Isso se chama restrição.

```ori
-- T pode ser qualquer tipo, DESDE QUE (for) obedeça à trait 'Comparable'
max for T: Comparable (a: T, b: T) -> T
  -- O compilador agora sabe que 'a' e 'b' podem ser comparados
  return a
end
```

## Traits Centrais do Ori (ori.core)

A biblioteca padrão traz traits fundamentais:
- **`Displayable`:** Permite converter o tipo em texto. A função embutida `string(value)` exige que o valor implemente isso.
- **`Equatable`:** Permite verificar se dois itens são iguais (`==`).
- **`Comparable`:** Permite checar maior/menor (`>`, `<`).
- **`Iterable`:** Permite usar a estrutura em um loop `for item in colecao`.
- **`Disposable`:** Permite que o objeto limpe seus recursos automaticamente em um bloco `using`.
- **`Addable`:** Permite somar com `+`.

## O que memorizar
- Use `trait` para definir um contrato de comportamento.
- **Uma trait só:** `apply Tipo use Trait` — cabeçalho compacto, `use` na mesma linha (forma obrigatória).
- **Duas ou mais traits, ou trait + método próprio:** aí a forma aninhada é a exigida.
- Só métodos próprios, sem trait: `apply Tipo` com os métodos direto no bloco.
- Se o método altera a struct, ele deve começar com `mut`.
- Restrinja genéricos com `for T: NomeDaTrait`.
