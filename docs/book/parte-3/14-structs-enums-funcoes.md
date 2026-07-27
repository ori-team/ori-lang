# Cap. 14 — Structs, enums e funções
> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR
Structs agrupam dados, enums representam escolhas exclusivas, e funções encapsulam blocos de código reutilizáveis. O Ori facilita a criação de estruturas ricas com foco em clareza, desde funções simples até estruturas de dados compostas.

## Structs (Estruturas de Dados)

Structs agrupam diferentes valores sob um mesmo nome. São como "fichas" onde cada campo tem um tipo definido. Structs no Ori são *value types* (tipos de valor), o que significa que, ao serem passadas para uma função ou atribuídas a uma variável, elas são copiadas (copy semantics), não apenas referenciadas.

### Declaração e Inicialização

```ori
module app.shapes

-- Declaramos a struct com os campos e seus tipos
struct Point
  x: float
  y: float
end

-- Criando uma instância (struct literal completo)
const point_a: Point = Point { x: 1.0, y: 2.0 }

-- Criando uma instância (struct literal anônimo - o tipo é inferido)
const point_b: Point = { x: 0.0, y: 0.0 }
```

Você pode atualizar um struct criando uma cópia com alguns campos modificados usando a expressão `with`:

```ori
const point_c: Point = point_a with { y: 10.0 } end
-- point_c agora é { x: 1.0, y: 10.0 }
```

Acessamos os campos usando a notação de ponto (`.`):
```ori
const x_value = point_c.x
```

### Pegando vários campos de uma vez (destructuring)

Quando você precisa de dois ou três campos, repetir o nome da struct em cada
linha cansa:

```ori
-- repetitivo
const pos: Point = get_pos()
const x = pos.x
const y = pos.y
```

Dá para ligar tudo numa linha só:

```ori
const Point { x, y } = get_pos()
```

Variações:

```ori
const { x, y } = get_pos()          -- tipo inferido (mesma regra do const comum)
const Point { x: px, y: py } = pos  -- renomeando na hora de ligar
var { x, y } = get_pos()            -- bindings mutáveis
```

Depois disso, `x` e `y` são variáveis normais — a forma é só um atalho para
"guarde o valor e leia estes campos".

> **Só funciona com campos de struct**, não com tupla. É proposital: `const
> (a, b) = ...` devolveria ao leitor a pergunta "o que era o campo 2 mesmo?",
> que é exatamente o custo que essa forma existe para remover.

## Enums (Enumerações)

Enums são usados quando um valor pode ser apenas *uma* de várias opções (escolhas exclusivas). No Ori, as variantes de um enum podem armazenar dados.

```ori
module app.shapes

enum Shape
  Circle(radius: float)
  Rectangle(width: float, height: float)
  -- Um enum sem dados (variante simples)
  Point
end

-- Construindo uma variante (forma completa)
const s1 = Shape.Circle(radius: 10.0)

-- Variantes podem ser escritas de forma abreviada quando o tipo é conhecido
const s2: Shape = .Rectangle(width: 5.0, height: 5.0)
const s3: Shape = .Point
```

Para usar um enum, o ideal é o `match`, que verifica todas as opções. Repare
que dentro do `match` as variantes **não** levam o ponto inicial, e cada
`case` termina com `:`:

```ori
match s1
case Circle(radius):
  -- faz algo com radius
case Rectangle(width, height):
  -- faz algo com width e height
case Point:
  -- variante simples
end
```

> **Por que sem ponto aqui, se o literal usa `.Rectangle(...)`?** São dois
> lugares diferentes. No literal (criando um valor), o ponto vem de
> `Shape.Rectangle(...)` — você está dizendo "a variante Rectangle do tipo
> Shape". No `case`, você já está dentro de um `match shape`, o compilador
> já sabe o tipo; escrever `case .Rectangle` de novo seria redundante, e por
> isso o Ori **rejeita** essa forma (erro `parse.case_dot_variant_removed`).

## Funções

No Ori, não usamos a palavra-chave `func`. O nome da função, parâmetros e tipo de retorno definem a função, e o corpo é fechado com `end`. Se a função não retorna nada (void), você não precisa colocar a seta `->`.

```ori
module app.logic

-- Função normal com corpo
process_data(data: string) -> int
  -- faz algum processamento
  return 42
end

-- Função que não retorna nada (void)
log_message(msg: string)
  -- Não precisamos de '->' aqui
end
```

### Funções de Uma Expressão

Para funções simples, você pode usar `=>` (seta gorda) no lugar do corpo. Ela retorna a expressão automaticamente.

```ori
double(n: int) -> int => n * 2
```

### Parâmetros Especiais

Você pode usar argumentos nomeados na chamada para deixar o código mais claro, definir valores padrão (default) para os parâmetros, e até aceitar múltiplos valores (variadic).

```ori
-- Argumento com valor padrão
connect(host: string, port: int = 80)
  -- conecta no host e porta
end

-- Parâmetros variádicos (múltiplos valores de um mesmo tipo)
log(prefix: string, values: int...)
  -- values é tratado como uma lista de inteiros
end

-- Chamando as funções:
-- Argumentos nomeados deixam claro o que cada valor significa
connect(host: "localhost", port: 8080)
connect(host: "localhost") -- Usa port = 80

log(prefix: "Errors", 404, 500, 403)
```

### Contratos de Valor (Value Contracts)

O Ori permite restringir os valores que entram em uma função usando a palavra-chave `if` no parâmetro. O identificador especial `it` refere-se ao valor sendo avaliado.

```ori
-- A função só aceita floats que sejam maiores ou iguais a zero
sqrt(val: float if it >= 0.0) -> float
  -- calcula a raiz quadrada
  return 0.0
end
```

### Chamadas Poéticas (Poetic Calls)

Se a função recebe apenas um argumento, você pode omitir os parênteses:
```ori
io.print "Olá, Mundo!" -- O mesmo que io.print("Olá, Mundo!")
```

> **Cuidado:** não existe um `print` "solto" na linguagem — mesmo na forma
> poética, você chama pelo módulo (`io.print`, `io.println`). A chamada
> poética só tira os parênteses; ela não cria um atalho novo.
>
> Essa forma só vale para **uma** chamada por linha — encadear duas
> chamadas poéticas seguidas (`f g "texto"`) é erro. É uma exceção
> deliberada da Ori: o resto da sintaxe segue a regra de "uma forma
> canônica só", mas a chamada poética existe por causa da leitura em
> estilo prosa que dá nome à "superfície S3".

## Closures (Funções Anônimas)

Closures são funções sem nome, muitas vezes passadas como parâmetros para outras funções (como métodos de coleções).

```ori
-- Closure em uma linha (inline)
const dobro = (x: int) => x * 2

-- Closure em bloco, fechado com 'end'
const calcula = (x: int)
  const base = 10
  return x * base
end
```

Exemplo com coleções:
```ori
-- O 'map' aplica a closure a cada item
const dobrados = list_of_numbers |> map((x: int) => x * 2)
```

## Tipos Genéricos (Generics)

Generics permitem escrever funções e structs que funcionam com qualquer tipo, sem perder a segurança (checagem em tempo de compilação). No Ori, usamos colchetes `[]` em vez de sinais de menor/maior `<>`, para evitar que o código pareça uma operação matemática de comparação.

```ori
-- T é um "placeholder" para qualquer tipo
identity[T](value: T) -> T
  return value
end

struct Pair[A, B]
  first: A
  second: B
end
```

## Coleções Padrão

O Ori possui estruturas de dados embutidas, criadas com genéricos. As mais
usadas no dia a dia têm literal próprio:

- **Listas (`list[T]`):** Coleção ordenada. Literal: `[1, 2, 3]`.
- **Mapas (`map[K, V]`):** Dicionário de chave-valor. Literal: `{"chave": "valor"}`.
- **Conjuntos (`set[T]`):** Coleção de valores únicos. Literal: `set{1, 2, 3}`.
- **Tuplas (`tuple[A, B]`):** Agrupamento pequeno e sem nome (`.0`, `.1`, …) — veja o Cap. 12.
- **Range (Intervalo):** Usado para loops, escrito como `0..9` (vai de 0 a 9).

Igual a `string`, essas coleções **não** têm métodos de ponto embutidos
(nada de `lista.push(x)`). As operações vivem como funções no módulo
correspondente da stdlib — `ori.list`, `ori.map`, `ori.set` — que você
chama pelo alias do import:

```ori
-- 'list' já é o nome do tipo (list[T]); o alias do módulo precisa de outro nome
import ori.list = lists

main()
    var nums: list[int] = [1, 2, 3]
    lists.push(nums, 4)
    io.print(f"tamanho: {lists.len(nums)}")
end
```

Além das quatro acima, a stdlib traz mais estruturas prontas, cada uma no
seu módulo (`ori.stack`, `ori.queue`, `ori.deque`, `ori.tree`, `ori.graph`,
`ori.heap`, `ori.linked_list`, `ori.hash_table`) — o índice completo está
no [Cap. 20](../parte-4/20-stdlib-indice.md).

O Ori também possui a estrutura `lazy[T]`, que adia o cálculo de um valor até que ele seja realmente necessário.

## Apelido de Tipo (`alias`)

Se um tipo for longo, use `alias` para encurtar a leitura:

```ori
-- Um apelido para um tipo comprido
alias UserMap = map[int, string]

lookup(users: UserMap) -> string
  return "Alice"
end
```

O `alias` é **transparente**: `UserMap` e `map[int, string]` são a mesma
coisa, e os valores circulam livremente entre os dois nomes.

## Tipo Próprio (`newtype`)

Às vezes você não quer um apelido — quer um tipo **de verdade**, que o
compilador não confunda com outro. É para isso que serve o `newtype`:

```ori
newtype UserId = int
newtype AccountId = int

transfer(from: AccountId, to: AccountId, by: UserId) -> string
  return "ok"
end
```

Agora essa assinatura é um contrato que o compilador defende. Trocar a ordem
dos argumentos por engano — passar um `UserId` onde se espera `AccountId` —
vira erro de compilação, não um bug silencioso em produção:

```text
error[type.arg_type_mismatch]: argument 1 expects `AccountId`, found `UserId`
```

Compare com o que aconteceria usando `int` (ou `alias`) para tudo: os três
argumentos seriam intercambiáveis, e trocar dois deles compilaria numa boa.

### Entrando e saindo

Como é um tipo diferente, a conversão é **escrita**, nos dois sentidos:

```ori
const id: UserId = UserId(7)   -- entra: envolve o int
const raw: int = int(id)       -- sai: volta a ser int
```

Nada disso custa nada em tempo de execução — o `newtype` é apagado na
compilação. Um `newtype` sobre `int` **é** um `int` na memória: sem struct
extra, sem alocação. Você paga só na hora de escrever, e ganha na hora de ler.

### `alias` ou `newtype`?

| Situação | Use |
|----------|-----|
| Encurtar um tipo comprido (`map[int, string]`) | `alias` |
| Dar significado a um valor que não pode ser confundido (`UserId`, `Email`, `Celsius`) | `newtype` |

A regra prática: se trocar dois valores por engano seria um **bug**, use
`newtype`. Se são realmente a mesma coisa com nome menor, use `alias`.

## O que memorizar
- Use `Struct { campo: valor }` para criar structs e `with` para atualizá-las.
- Enums garantem escolhas exclusivas e suportam dados em suas variantes.
- Literal de enum usa ponto (`.Rectangle(...)`); dentro do `match`, o `case` **não** usa ponto e sempre termina com `:`.
- Funções não usam `func`. Use `=>` para retornos de uma linha.
- Contratos como `val: float if it >= 0.0` ajudam a proteger sua lógica.
- Generics usam colchetes `[]` (`list[int]`, não `list<int>`).
- Coleções não têm métodos de ponto — as operações vivem em `ori.list`, `ori.map`, `ori.set`, etc.
