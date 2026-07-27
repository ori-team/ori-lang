# Cap. 13 — Expressões, fluxo e pipe

> **Versão âncora:** Ori 0.3.x (S3)
> **Parte:** III

## TL;DR
Controle o caminho do seu código com `if/elif/else`, `while`, `for`, `loop` e `match`. Funções menores ("closures") e a capacidade de encadear operações da esquerda para a direita com o pipe `|>` ajudam a manter a leitura visualmente fluida e limpa.

## Expressões Condicionais

### If clássico
Para testar condições lógicas, sempre use `elif` no lugar de `else if`.

```ori
if age < 18
    io.print("Menor de idade")
elif age == 18
    io.print("Exatamente dezoito")
else
    io.print("Maior de idade")
end
```

### If como expressão (inline)
Se você precisa decidir o valor rápido de uma variável em uma linha, pode usar a versão de expressão:

```ori
-- Retorna "on" se active for verdadeiro, senão "off"
const status = if active then "on" else "off"
```

### Checagem de tipos com `is`
Para verificar se um valor é de um tipo específico no meio do fluxo:

```ori
if shape is Circle
    io.print("Definitivamente é um círculo!")
end
```

`is` devolve `true`/`false`. Duas coisas importantes que **não** são óbvias:

- **Não estreita o tipo (sem narrowing).** Se `shape` for `any[Drawable]`
  (uma trait), dentro do `if` ele continua sendo `any[Drawable]` — você não
  ganha acesso aos campos do `Circle` real, só aos métodos que a trait
  declara. Para extrair os dados de dentro, use `match` (próxima seção).
- **Não funciona com variantes de enum.** `cor is Vermelho` não compila
  (`type.undefined_name`) — variantes de enum não são "tipos" para o `is`,
  elas se distinguem com `match`/`case`.

## Laços de Repetição (Loops)

O Ori oferece várias maneiras de repetir ações sem dor de cabeça, dependendo da necessidade:

### For (Iteração)
Percorre itens de uma coleção ou um intervalo numérico exato.

```ori
-- Usando um intervalo de números (ranges)
for i in 0..9
    io.print(f"Contagem rápida: {i}")
end

-- Iterando com acesso ao índice (posição do item na lista)
for item, index in items
    io.print(f"A posição {index} contém o valor {item}")
end
```

### Repeat (Contagem Fixa)
Quando você só quer repetir algo N vezes mas não precisa controlar variáveis matemáticas como `i`.

```ori
repeat 5 times
    io.print("Som de alerta!")
end
```

### Loop infinito e While condicional

```ori
-- Loop infinito puro, exige um 'break' para poder sair
loop
    const event = wait_event()
    if event.is_quit
        break -- Sai do loop imediatamente cortando a execução
    end
end

-- While clássico com continue
while x < 10
    x = x + 1
    continue -- Pula direto para a próxima repetição ignorando o resto
end

-- While opcional: continua rodando enquanto a função devolver um valor válido 'some'
while some(line) = reader.next()
    io.print(line)
end
```

## Pattern Matching (O `match` exaustivo)

O `match` testa o formato ou conteúdo de um valor contra vários padrões. O compilador do Ori exige que ele seja **exaustivo**, ou seja, você precisa cobrir mentalmente e no código todas as possibilidades existentes. Se esquecer de alguma, ele não deixa compilar para te proteger de surpresas.

```ori
match result
    case ok(val):
        io.print(f"Sucesso na operação: {val}")
    case err(msg):
        io.print(f"Falha detectada: {msg}")
    case else:
        -- O 'case else' pega qualquer coisa não listada antes.
        -- Funciona como um coringa obrigatório se você não quiser listar tudo.
        io.print("Caiu num cenário desconhecido.")
end
```

### Guards: uma condição extra dentro do `case`

Às vezes o formato do valor não basta — você também quer testar uma
condição sobre o que foi capturado. É para isso que serve o **guard**:
`case padrão if condição:`.

```ori
match score
    case n if n >= 90:
        io.print("A")
    case n if n >= 80:
        io.print("B")
    case else:
        io.print("C")
end
```

Como funciona: o Ori testa o padrão (`n` captura qualquer `int`) e, se
casar, testa a `condição` **antes** de rodar o corpo. Se a condição for
falsa, ele desiste desse `case` e tenta o próximo — não executa o corpo.
Sem o guard, o primeiro `case n:` capturaria *qualquer* score, e os outros
nunca seriam testados.

> **Guards não contam como cobertura completa.** Um `match` com só `case n
> if n >= 90:` não é exaustivo — o compilador ainda exige um `case else:`,
> porque ele não sabe provar que a condição cobre todos os valores possíveis.

### Vários valores no mesmo braço: `or`

Quando dois casos fazem a mesma coisa, você não precisa repetir o corpo —
separe as alternativas com a palavra **`or`**:

```ori
match direcao
case Norte or Sul:
    io.println("vertical")
case Leste or Oeste:
    io.println("horizontal")
end
```

Funciona com qualquer padrão simples: variantes de enum, números, textos.

```ori
match n
case 1 or 2 or 3:
    io.println("pequeno")
case else:
    io.println("grande")
end
```

Duas regras:

- **Alternativas não podem ligar valores.** `case Circle(r) or Square(r):` é
  erro (`match.or_pattern_binding`). O motivo é de leitura: cada alternativa
  teria que ligar exatamente os mesmos nomes, com os mesmos tipos, e quem lê
  passaria a carregar essa conferência na cabeça. Se você precisa do valor de
  dentro, escreva um `case` por alternativa (ou use um guard).
- **Para exaustividade, `case a or b:` conta como dois casos.** No primeiro
  exemplo, as quatro direções ficam cobertas sem precisar de `case else:`.

> **Por que `or` e não `|`?** Porque a Ori já escreve os operadores lógicos
> com palavras (`and`, `or`, `not`), e a vírgula já tem dono dentro do `case`
> (separa os campos de um payload). `case Rgb(r, g, b) or Named(n):` fica
> legível; com vírgula haveria dois níveis de vírgula na mesma linha.

### `match` como expressão

Nos exemplos acima o `match` é um **comando**: cada braço executa linhas. Mas
o `match` também pode ser uma **expressão**, ou seja, produzir um valor
direto. Nesse formato, cada braço é uma única expressão:

```ori
const nota: string = match score
case n if n >= 90: "A"
case n if n >= 80: "B"
case else: "C"
end
```

Compare com a alternativa sem isso, que exige uma variável mutável só para
guardar o resultado:

```ori
-- mais longo e mais fácil de errar
var nota: string = ""
match score
case n if n >= 90:
    nota = "A"
case else:
    nota = "C"
end
```

A versão com expressão é melhor por dois motivos de leitura:

1. O `const` avisa que aquele valor **nunca muda depois** — o leitor não
   precisa procurar reatribuições mais adiante.
2. Some a classe de bug "esqueci de atribuir em um dos braços": como todo
   braço tem que produzir um valor, o compilador cobra isso de você.

Regras do formato de expressão:

- Todos os braços precisam produzir o **mesmo tipo** (senão dá
  `type.match_arm_mismatch`).
- Vale a mesma exigência de exaustividade — na prática, quase sempre você
  vai precisar de um `case else:`.
- Só um braço executa; os outros nem são avaliados.

> **Não é uma forma "dual".** É o mesmo padrão que o `if` já segue: em
> posição de comando ele é comando (`if ... end`), em posição de valor ele é
> expressão (`if cond then a else b`). A posição decide, você não escolhe.

## Closures (Funções anônimas)
Closures são funções que não têm nome. Elas costumam ser usadas na hora como parâmetros temporários para outras operações.

```ori
-- Closure de uma linha só
const duplicate = (x: int) => x * 2

-- Closure em formato de bloco (várias linhas)
const complex_calc = (x: int)
    const temp = x * 10
    return temp / 2
end
```

Elas brilham muito ao trabalhar com listas e filtros:
```ori
-- Filtra a lista para manter apenas os números pares
const evens = numbers.filter((n: int) => n % 2 == 0)
```

## O Encadeamento com Pipe `|>`

O operador de pipe `|>` pega o resultado do que está à sua esquerda e o injeta como o **primeiro argumento** da próxima função à sua direita. Ele substitui chamadas matemáticas aninhadas que são difíceis de ler.

```ori
-- Em vez de escrever algo invertido de ler como: to_label(add_ten(double(5)))
-- Escrevemos em um fluxo lógico da esquerda pra direita, como se lê:

const label = 5 
    |> double     -- double(5) resulta em 10
    |> add_ten    -- add_ten(10) resulta em 20
    |> to_label   -- to_label(20) resulta em "20"
```

Isso facilita brutalmente a leitura, pois lemos as transformações na exata ordem em que vão acontecer.

> **Pegadinha comum:** quando a função do lado direito **não** recebe
> argumento extra nenhum (só o valor que vem do pipe), escreva o nome dela
> **sem parênteses**: `raw |> str.trim`, não `raw |> str.trim()`. Parênteses
> vazios chamam a função primeiro, com zero argumentos — o que quase sempre
> dá erro de tipo. Parênteses só aparecem quando há argumentos *extras*
> além do valor do pipe: `raw |> str.center(10)` (equivale a `str.center(raw, 10)`).

## O que memorizar
*   Sempre use `elif` e não `else if`.
*   Para repetir ações, prefira as ferramentas específicas como `repeat 5 times` e `for item, index in items`.
*   O `match` precisa cobrir tudo sem exceção (use `case else:` como coringa seguro).
*   O pipe `|>` empurra o valor sempre para o primeiro parâmetro da próxima etapa.
