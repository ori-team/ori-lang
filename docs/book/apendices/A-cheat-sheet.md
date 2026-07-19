# Apêndice A — Cheat sheet compacta

> **Versão âncora:** Ori 0.3.x (S3)

Esta é uma versão super compacta e imprimível da sintaxe do Ori.

### Estrutura e Módulos
```ori
module app.name
-- SEMPRE caminho = alias, nunca o contrário
import ori.net = net
public import ori.net = net
imports
    ori.net = net
    ori.fs = fs
end
```

### Variáveis
```ori
const imutavel = 10
var mutavel = 20
const tipado: int = 5
```

### Funções
```ori
-- Normal
name(x: int) -> int
    return x * 2
end

-- Expressão única
name(x: int) -> int => x * 2

-- Assinaturas especiais
async fetch_data() -> string
greet(name: string = "Visitante")
log(msgs: string...)
sqrt(v: float if it >= 0.0) -> float
```

### Estruturas de Dados
```ori
-- Struct
struct User
    id: int
end
const u = User { id: 1 }
const u2 = u with { id: 2 } end

-- Enum
enum Option
    None
    Some(val: int)
end
const op: Option = .Some(val: 5)
```

### Traits e Tipos
```ori
trait Show
    show(self)
end

apply User
    use Show
        show(self) => io.println(self.id)
    end
end

alias Ids = map[int, string]   -- apelido (transparente)
newtype Id = int               -- tipo proprio (nominal); Id(7) entra, int(x) sai
```

### Controle de Fluxo
```ori
if a == b
    -- ...
elif a > b
    -- ...
else
    -- ...
end

while true
    break
end

for item in lista
    -- ...
end

for item, idx in lista
    -- ...
end

loop
    -- infinito
end

repeat 5 times
    -- repete
end

-- sem ponto antes da variante, sempre com ':'; guard é opcional
match valor
case 1:
    -- ...
case n if n > 10:
    -- ...
case else:
    -- ...
end
```

### Tratamento de Erros e Opcionais
```ori
const v = some(10)
const v2 = none
const r = ok(5)
const r2 = err("Falhou")

if some(x) = v
    -- desempacotou v
end

while some(line) = reader.next()
    -- lê até dar none
end

-- Desembrulha Result
const file = try fs.read_text("file")

check x > 0, "Erro crítico"
```

### Coleções
```ori
const l: list[int] = [1, 2, 3]
const m: map[string, int] = {"a": 1}
const s = set{1, 2}
const t: tuple[int, int] = (1, 2)   -- campos por posição: t.0, t.1
const r = 1..10
```

### Closures e Pipes
```ori
const f = (x) => x * 2
const y = 5 |> f
```

### Outros Recursos
```ori
-- Limpeza garantida de recursos — 'using' NÃO abre bloco próprio
using file = try fs.open_read("arq.txt")
-- file é fechado no `end` da função (ou return/try/break/continue)

-- Interpolação
const s = f"Olá {nome}"

-- Genéricos
id[T](v: T) -> T => v
```
