# Cap. 18 — Cheat sheet de sintaxe S3

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR
Uma referência completa e rápida de toda a sintaxe da linguagem Ori, no padrão S3. Perfeito para manter aberto ao lado enquanto você programa. Todo trecho aqui foi validado rodando o compilador de verdade.

---

## 1. Módulos e Imports

Todo arquivo precisa de um cabeçalho de módulo na primeira linha. Imports
sempre na ordem **caminho = alias** (nunca o contrário).

```ori
module app.main

-- Importação simples (caminho = alias)
import ori.io = io

-- Importação pública (re-exporta o módulo para quem importar o seu)
public import ori.net = net

-- Múltiplos imports em bloco
imports
    ori.fmt = fmt
    ori.fs = fs
end
```

## 2. Variáveis (Bindings)

Ori usa `const` para imutável e `var` para mutável.

```ori
-- Inferência de tipo
const name = "Alice"
var age = 30

-- Com tipo explícito
const score: int = 100

-- Atualização de variável (só se for `var`)
age = 31
```

## 3. Funções

Não existe a palavra `func`. Funções são declaradas direto pelo nome.

```ori
-- Função normal
add(x: int, y: int) -> int
    return x + y
end

-- Função de expressão única (seta dupla `=>`)
multiply(x: int, y: int) -> int => x * y

-- Função com argumento padrão e nomeado
greet(name: string = "Visitante") -> string
    return f"Olá, {name}!"
end
-- Uso: greet() ou greet(name: "Bob")

-- Função variádica (aceita vários argumentos)
log(prefix: string, values: int...)
    io.println(prefix)
end

-- Contratos de valor (validação na assinatura; 'it' é o valor recebido)
sqrt(val: float if it >= 0.0) -> float
    -- ...
end
```

## 4. Controle de Fluxo

### if / elif / else
```ori
if age < 18
    io.println("Menor de idade")
elif age == 18
    io.println("Exatamente 18")
else
    io.println("Maior de idade")
end

-- if como expressão (precisa de 'then')
const status: string = if age >= 18 then "adulto" else "menor"
```

### Loops
```ori
-- While
while count > 0
    count = count - 1
end

-- For em coleções
for item in items
    io.println(item)
end

-- For com índice
for item, index in items
    io.println(f"{index}: {item}")
end

-- Repeat (repete N vezes)
repeat 5 times
    io.println("Oi!")
end

-- Loop infinito
loop
    -- Use `break` para sair ou `continue` para pular a vez
end
```

### Match — sem ponto antes da variante, sempre com `:`
```ori
-- como comando: cada braço executa linhas
match score
case 100:
    io.println("Perfeito!")
case n if n >= 50:
    -- guard: só entra aqui se n >= 50 (e não bateu no case 100 antes)
    io.println("Bom trabalho.")
case else:
    io.println("Tente de novo.")
end

-- como expressão: cada braço é UMA expressão e produz o valor
const nota: string = match score
case n if n >= 90: "A"
case else: "C"
end

-- alternativas com 'or' (não ligam valores; contam para exaustividade)
match direcao
case Norte or Sul:
    io.println("vertical")
case else:
    io.println("horizontal")
end
```

### Checagem de tipo com `is`
```ori
if shape is Circle
    -- 'is' devolve bool; NÃO estreita o tipo (sem narrowing).
    -- NÃO funciona com variante de enum — só com tipos (struct/any[Trait]).
end
```

## 5. Structs e Enums

### Structs
```ori
struct User
    name: string
    age: int
end

-- Literal de struct
const user = User { name: "Bob", age: 30 }

-- Literal de struct anônima (quando o tipo já é esperado pelo contexto)
const p: Point = { x: 0, y: 0 }

-- Atualização de struct (copia mudando alguns campos; NÃO tem 'end' de bloco extra)
const user2 = user with { age: 31 } end

-- Destructuring: liga vários campos de uma vez (só struct, não tupla)
const User { name, age } = get_user()
const { name: n } = get_user()   -- tipo inferido + renomeando
```

### Enums
```ori
enum Status
    Active
    Inactive
    Banned(reason: string)
end

-- Literal de enum (com ponto inicial) — fora do match
const current: Status = .Active
const penalty = Status.Banned(reason: "Spam")

-- Dentro de match, SEM ponto e com ':'
match current
case Active:
    io.println("ativo")
case Banned(reason):
    io.println(reason)
case else:
    io.println("outro")
end
```

## 6. Traits (Interfaces)

Traits definem comportamentos que tipos podem adotar.

```ori
trait Printable
    print(self)
end

-- Uma trait só: cabeçalho compacto ('use' na MESMA linha) — forma obrigatória
apply User use Printable
    print(self)
        io.println(self.name)
    end
end

-- Duas traits (ou trait + método próprio): aí a forma aninhada é a exigida
apply User
    use Printable
        print(self)
            io.println(self.name)
        end
    end
    use Comparable
        compare(self, other: User) -> int
            return self.age - other.age
        end
    end
end

-- Método que ALTERA a struct precisa de 'mut' na assinatura
apply User
    mut rename(self, new_name: string)
        self.name = new_name
    end
end
```

## 7. Tratamento de Erros

Ori usa `optional` para ausência de valor e `result` para erros esperados.

```ori
-- Criando valores
const v1 = some(10)
const v2 = none
const r1 = ok(20)
const r2 = err("Falhou")

-- Binding condicional: mesma forma para optional e result
if some(val) = v1
    io.println(val)
end

if ok(val) = r1
    io.println(val)       -- liga o valor de sucesso
end

if err(msg) = r2
    io.println(msg)       -- liga o erro (entra quando NÃO deu ok)
end

-- Lidando com Result (operador `try`)
-- `try` desembrulha o `ok` ou retorna o `err` antecipadamente.
const data: string = try fs.read_text("file.txt")

-- Checagens rápidas de erro crítico (aborta o programa se falso)
check age >= 0, "Idade não pode ser negativa"
```

## 8. Coleções

```ori
-- Listas (operações via ori.list, não método de ponto)
const numbers: list[int] = [1, 2, 3]

-- Mapas (Chave-Valor)
const ages: map[string, int] = {"Alice": 30, "Bob": 25}

-- Sets (Conjuntos únicos)
const ids = set{1, 2, 3}

-- Tuplas (campos por posição: .0, .1, ...)
const pair: tuple[int, string] = (1, "um")
const n: int = pair.0

-- Ranges
const interval = 1..10
```

## 9. Closures (Funções anônimas) e Pipes

```ori
-- Closure inline
const double = (x: int) => x * 2

-- Closure em bloco
const add_print = (x: int, y: int)
    const sum = x + y
    io.println(sum)
    return sum
end

-- Pipe operator (`|>`) - Passa o resultado da esquerda como primeiro argumento da direita
const final = 10 |> double |> add_print(5)
```

## 10. Recursos Adicionais

```ori
-- Interpolação de string
const greeting = f"Olá, {name}!"

-- Gerenciamento de recursos: 'using' NÃO tem bloco/end próprio,
-- a limpeza acontece no 'end' da função (ou return/try/break/continue)
using file = try fs.open_read("data.txt")
const conteudo = try fs.read_all(file)

-- Apelido de tipo (transparente: mesma coisa, nome menor)
alias UserMap = map[int, string]

-- Tipo proprio (nominal: NAO se mistura com int nem com outro newtype)
newtype UserId = int
const id: UserId = UserId(7)   -- entra
const raw: int = int(id)       -- sai

-- Tipos Genéricos
identity[T](value: T) -> T => value
```

## O que memorizar
- Não existe `func`. Não existe chaves `{}` para blocos (é `end`).
- Import é sempre **caminho = alias** (`import ori.io = io`), nunca o contrário.
- `match`: sem ponto antes da variante, sempre `case ... :`.
- `using` é uma declaração simples — não abre bloco próprio.
- `is` não estreita tipo e não funciona com enum.
- Use `try` para desembrulhar `result`; `if some(x) = ...` para `optional`.
