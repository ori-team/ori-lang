# Cap. 12 — Tipos, ausência e falha

> **Versão âncora:** Ori 0.3.x (S3)
> **Parte:** III

## TL;DR
Não existe `null` no Ori para evitar bugs invisíveis. Valores ausentes usam `optional[T]`. Falhas usam `result[T, E]`. Use `match` para lidar com eles no mesmo local, ou `try` para empurrar o problema para quem chamou a função. 

## Tipos Primitivos Básicos

Antes de falar de falhas, vamos revisar os tijolos básicos da linguagem:

*   **Números:** `int` (inteiros), `float` (decimais com vírgula), `u8` (byte positivo, ótimo para dados brutos).
*   **Texto:** `string` (texto legível formatado), `bytes` (sequência de u8 brutos).
*   **Lógica:** `bool` (`true` ou `false`).
*   **Vazio:** `void` (usado quando não há nenhuma informação a retornar).

Você pode criar **Tuplas** para agrupar poucos valores rapidamente, sem precisar
declarar um `struct`. Os campos não têm nome — você acessa pela posição, com
`.0`, `.1`, etc:
```ori
const location: tuple[int, string] = (10, "Brasil")
const lat: int = location.0
const country: string = location.1
```

> **Quando usar tupla vs struct:** tupla é ótima para um agrupamento bem local
> e óbvio (ex: retornar `(status, mensagem)` de uma função pequena). Para
> qualquer coisa que vai circular pelo código ou crescer, prefira `struct` —
> `location.0` não diz nada para quem lê meses depois, `location.x` diz.

E pode dar **Apelidos (Aliases)** para deixar o código mais intuitivo e claro:
```ori
alias UserId = int
const my_id: UserId = 42
```

## Por que não existe Null?
Em muitas linguagens, qualquer variável pode valer `null` (nada). Isso causa falhas inesperadas de sistema no meio do código ("Null Pointer Exception"), pois você nunca sabe quem pode estar vazio. 
No Ori, o compilador **obriga** você a tratar a possibilidade de ausência de antemão. Se algo pode faltar, você deve declarar isso claramente com o tipo `optional`.

## Lidando com Ausência: `optional`

Um tipo `optional[T]` pode ter um valor presente (`some`) ou nenhum valor (`none`).

```ori
-- Função que pode não encontrar o usuário
find_user(id: UserId) -> optional[string]
    if id == 1
        return some("Alice")
    end
    return none
end
```

### Extraindo valores do Optional
Para retirar o valor de dentro, você usa `match` ou o atalho elegante `if some`:

```ori
-- Usando 'if some' para extrair o valor, se ele existir
if some(name) = find_user(1)
    io.print(f"Encontrado: {name}")
end

-- Ou fornecendo um valor padrão alternativo com .or()
const final_name = find_user(99).or("Anonimo")
```

## Lidando com Falhas: `result`

Quando uma operação mais complexa pode dar erro (como ler um arquivo ou conectar na rede), usamos `result[T, E]`. Ele retorna sucesso (`ok`) ou o motivo do erro (`err`).

```ori
divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("Divisão por zero")
    end
    return ok(a / b)
end
```

### O operador `try`
Se você não quer lidar com o erro naquele momento, use `try` para passá-lo para cima na cadeia de chamadas.

```ori
calculate() -> result[int, string]
    -- Se falhar, a função calculate encerra na hora retornando o erro pra cima.
    -- Se der certo, extrai o valor dentro de ok() e coloca na variável 'val'.
    const val: int = try divide(10, 2)
    return ok(val + 5)
end
```

### Tratando o erro aqui mesmo: `if ok` e `if err`

O `try` empurra o erro para cima. Quando você quer resolver **nesta** função,
use a mesma forma do `if some`, agora para `result`:

```ori
-- 'ok' liga o valor de sucesso
if ok(valor) = divide(10, 2)
    io.println(f"deu certo: {valor}")
else
    io.println("falhou")
end

-- 'err' liga a mensagem de erro (entra quando NÃO deu certo)
if err(motivo) = divide(1, 0)
    io.println(f"falhou porque: {motivo}")
end
```

As três formas são a mesma ideia — desembrulhar condicionalmente e dar um
nome ao que veio de dentro:

| Forma | Entra quando | O que liga |
|-------|--------------|------------|
| `if some(x) = ...` | o `optional` tem valor | o valor |
| `if ok(v) = ...` | o `result` deu certo | o valor de sucesso |
| `if err(e) = ...` | o `result` falhou | o valor do erro |

**Quando usar cada uma:** `try` quando o erro não é problema seu (propaga),
`if ok`/`if err` quando esta função sabe o que fazer com ele, e `match`
quando você precisa tratar sucesso e falha lado a lado no mesmo bloco.

### Adicionando contexto de erro com `.or_wrap()`
Às vezes o erro original é muito genérico (ex: "arquivo não encontrado"). Use `.or_wrap` para explicar o que seu programa estava tentando fazer quando a falha ocorreu:

```ori
-- Se falhar, a mensagem de erro vira: "loading config: arquivo não encontrado"
const config = try load_file("config.txt").or_wrap("loading config")
```

## Quando algo "Nunca deveria falhar"
Se você tem certeza absoluta de que uma condição matemática ou lógica é verdadeira e, se não for, o programa não deveria nem continuar rodando (deve dar crash para proteção), use `check`.

```ori
-- Aborta a execução do aplicativo se a lista estiver vazia
check items.length > 0, "Erro Crítico: A lista de base não pode estar vazia"
```

## Armadilhas comuns
*   **Inverter `optional` e `result`:** Use `optional` para "pesquisei e não achei, algo normal". Use `result` para "tentei fazer uma operação e ela falhou".
*   **Esquecer de retornar result:** Se você usa a palavra `try` dentro da sua função, a sua função **precisa** declarar no retorno que ela devolve um `result` (ou `optional`), porque ela vai ejetar esse erro por ali se falhar.

## O que memorizar
*   Ausência é `optional` (valores `some` / `none`).
*   Falha é `result` (valores `ok` / `err`).
*   O `try` ejeta o problema; o `match` e o `if some` resolvem o problema.
*   `.or()` te dá um valor seguro de backup, `.or_wrap()` te dá contexto claro sobre o erro.
