# Recursos avançados da linguagem

> **Público:** quem já terminou o [tour da linguagem](tour.pt-BR.md)
> **Referência normativa:** [04-types.md](../spec/04-types.md), [07-functions.md](../spec/07-functions.md), [08-traits.md](../spec/08-traits.md) e [11-generics.md](../spec/11-generics.md)
> **English:** [advanced.md](advanced.md)

## Arrays e slices

`list[T]` cresce dinamicamente. `array[T, size: N]` tem o tamanho dentro do
tipo e armazena os elementos inline:

```ori
struct Grid
    cells: array[int, size: 4]
    label: string
end

main()
    var values: array[int, size: 3] = [1, 2, 3]
    values[1] = 99
end
```

O tamanho faz parte da identidade do tipo, o literal precisa ter exatamente o
mesmo número de elementos e o backend nativo atual exige elementos escalares.

`slice[T]` é uma janela somente leitura sobre uma lista. `lists.slice` copia;
uma janela não copia e continua observando a lista proprietária.

## Lazy, handles e contratos

`lazy[T]` executa uma função sem argumentos no máximo uma vez:

```ori
const delayed: lazy[int] = lazy.once(() => compute_value())
const value: int = lazy.force(delayed)
```

`handle[T]` é um valor opaco para recursos ou fronteiras FFI. Ele não permite
acesso direto a ponteiros nem libera memória manualmente.

Contratos de parâmetros usam `it`:

```ori
sqrt(value: float if it >= 0.0) -> float
```

Violação gera `contract.param_violation`.

## Variádicos e itens associados

Somente o último parâmetro pode ser variádico; uma lista existente é expandida
com `..`. Traits podem declarar tipos associados usando `alias` dentro do bloco
`use`, e funções sem `self` são funções associadas chamadas pelo nome do tipo.

## Const generics e newtype

```ori
struct Buffer[const size: int]
    used: int
end

const small: Buffer[size: 8] = Buffer { used: 0 }

newtype UserId = int
```

Const generics aceitam apenas expressões de compilação sem efeitos colaterais.
`newtype` não se mistura automaticamente com sua representação; a conversão
precisa ser explícita.

## Attributes de declaração

Ori reconhece hoje um conjunto fechado de attributes em declarações de nível
superior: `@test`, `@deprecated("mensagem")`, `@inline`, `@no_inline`, `@cfg`,
`@repr("C")` e `@c_export`. Um attribute desconhecido é erro.

`@repr` é propositalmente restrito: somente `@repr("C")` exato em uma struct é
aceito. Layout packed e outras strings de representação não existem.

`@cfg` agora seleciona declarações de nível superior antes da resolução de
nomes e da checagem de tipos. Ele usa predicados estruturados, não strings
livres:

```ori
@cfg(all(target_family: unix, feature: tls))
public connect_securely()
end

@cfg(not(execution_profile: embedded))
public spawn_process()
end
```

As chaves são `target_os`, `target_arch`, `target_family`,
`execution_profile` e `feature`, que precisa estar declarada no manifesto.
Os predicados podem ser combinados com `all`, `any` e `not`. A sintaxe do
código inativo continua sendo verificada; nomes e tipos dentro dele, não.
Consulte as [regras normativas](../spec/02-lexical.md#conditional-compilation)
e os [campos de features](../spec/17-project-and-docs.md#oriproj).

## Buffers contíguos e vetorização SIMD

`buffer[T]` representa memória contígua e plana na heap para arrays numéricos, buffers de pixels e amostras de áudio.
O otimizador HIR vetoriza automaticamente e divide em blocos laços numéricos contáveis (`GFX-SIMD-1`) sobre buffers e arrays em instruções SIMD de 128 bits:

```ori
import ori.buffer = buf

var pixels: buffer[int] = buf.alloc[int](1920 * 1080)
buf.set(pixels, 0, 0xFF0000FF)
```

## Destrutores customizados

Structs e enums podem implementar `core.Destructor` com `mut destroy(self) -> void` para liberar recursos externos deterministicamente antes que o payload ARC seja desalocado:

```ori
struct RecursoNativo
    handle: int
end

apply RecursoNativo use core.Destructor
    mut destroy(self) -> void
        -- Fecha descritor do SO ou recurso externo
    end
end
```

HKTs, move explícito, layouts diretos de collections na ABI e paridade completa
do backend C continuam fora do contrato estável.
