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
acesso direto a ponteiros nem libera memória manualmente, não pode atravessar
tasks e só é emprestado pelo tempo garantido pela API hospedeira. Agregados
exportados que contenham handles emprestados são rejeitados. `==` e `!=`
comparam apenas a identidade do ponteiro e nunca acessam o objeto apontado;
igualdade semântica, lifetime e afinidade de thread ainda não formam um
contrato completo (`LANG-HANDLE-1`). Para criar explicitamente o sentinela
nulo, atribua `handles.null()` a um valor `handle[T]`. Para testar apenas o
sentinela, importe `ori.handle` e use `handles.is_null(value)`; esse helper
nunca acessa o ponteiro.
Wrappers `@c_export` validam handles gerenciados opacos no registro do runtime
antes de entrar no código Ori. Para payloads concretos e não genéricos, o
compilador grava uma tag e o wrapper confere tamanho e tipo-fonte; ponteiros
nulos, estrangeiros, com tamanho incorreto ou de outro tipo seguem a falha
determinística de limites. Não há fallback somente por procedência: se o layout
concreto ou a tag de tipo não puderem ser gerados, a exportação falha em tempo de
compilação. O host deve usar o handle devolvido pelo export correspondente.

Nós genéricos de `graph.Graph[T]` usam o mesmo contrato `Equatable.equals` de
mapas e conjuntos. Para uma struct do usuário, a primeira operação concreta no
grafo instala esse callback; valores equivalentes passam a compartilhar um nó,
e consultas de arestas não direcionadas comparam por valor. `graph.clone` e
`graph.transitive_closure` preservam o callback. Enums suportam `==`/`!=`
estrutural diretamente e podem ser nós de grafo quando têm `Hashable`; um
`Equatable` explícito continua sobrescrevendo a igualdade estrutural. Chaves
estruturais não recursivas usam callbacks nativos de hash gerados pelo
compilador. Um método `hash(self) -> int` em `Hashable` sobrescreve o callback
gerado. Quando `Equatable` explícito não é estrutural e não há método `hash`, o
runtime usa hash constante para preservar a correção; admissão recursiva e
ajuste de performance continuam abertos.

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

Ori reconhece um conjunto fechado de attributes em declarações de nível
superior: `@test`, `@deprecated("mensagem")`, `@inline`, `@no_inline`, `@noalloc`,
`@align(N)`, `@cfg`, `@repr("C")` e `@c_export`. Um attribute desconhecido é erro.

- `@noalloc` verifica estaticamente que a função não realiza alocações na heap (proíbe literais `list`/`map`/`set`, interpolação de strings, closures, `await`, `using` e chamadas a funções que alocam).
- `@align(N)` força alinhamento explícito da struct para potências de dois (1, 2, 4, 8, 16, 32, 64), essencial para GPU uniform buffers e tipos FFI/GDExtension.

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

## Vetores SIMD Portáteis (`simd[T, N]`)

Ori oferece tipos nativos de vetores SIMD com largura fixa (`simd[float32, 4]`, `simd[int32, lanes: 4]`)
baixados diretamente para instruções vetoriais da CPU (x86_64 SSE/AVX e ARM NEON). Suportam operações
aritméticas paralelas diretas (`+`, `-`, `*`, `/`) e indexação de lanes:

```ori
const a: simd[float32, 4] = [1.0f32, 2.0f32, 3.0f32, 4.0f32]
const b: simd[float32, 4] = [10.0f32, 20.0f32, 30.0f32, 40.0f32]
const c: simd[float32, 4] = a + b    -- instrução vetorial única
const x: float32 = c[0]             -- extração de lane
```

Combinações suportadas: `float32`/`int32` x 2, 4, 8, 16; `float64`/`int64` x 2, 4; `int16`/`u16` x 4, 8, 16; `int8`/`u8` x 8, 16.

## Arenas de Memória e Escopo (`mem.region`)

`mem.region()` cria uma arena do tipo bump allocator para alocações temporárias de frame (loops de jogos a 60/120 FPS,
listas de visibilidade, filas de comandos de renderização), descartando custos de contagem de referência:

```ori
import ori.mem = mem

main()
    using r: mem.Region = mem.region()
    -- alocações da região no frame...
    mem.reset(r)           -- reset instantâneo em O(1)
end                        -- liberação determinística via core.Disposable ao sair do bloco
```

Garantias de análise de escape:
- Uma `Region` não pode escapar do bloco que a declarou via `return` (`using.escape`).
- Uma `Region` não pode cruzar threads ou ser enviada para tarefas assíncronas (não é `Transferable`).

## Buffers contíguos (`buffer[T]`)

`buffer[T]` representa memória contígua e plana na heap para arrays numéricos, buffers de pixels e amostras de áudio.

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
