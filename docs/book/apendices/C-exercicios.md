# Apêndice C — Exercícios

> **Versão âncora:** Ori 0.3.x (S3)

## TL;DR
Uma lista de exercícios práticos cobrindo todas as funcionalidades da linguagem, divididos por nível de dificuldade, para você fixar o que aprendeu.

---

### Nível ★ (Fácil - Fundamentos)

**1. Olá, visitante**
Crie uma função com um argumento padrão `name: string = "Visitante"`. Imprima uma saudação interpolada. Chame a função passando um nome, e depois chame sem passar nada.

**2. Repetição simples**
Use o laço `repeat N times` para imprimir a palavra "Ori" 10 vezes.

**3. Tratamento básico com Match**
Crie uma constante `score` com um valor inteiro. Escreva um bloco `match` que imprime "Perfeito" se for 100, "Bom" para 50, e "Desconhecido" para o resto.

**4. Alias de Tipo**
Crie um type alias chamado `IdUsuario` para o tipo `int`. Crie uma variável desse tipo e a imprima.

**5. Coleções básicas**
Crie uma lista de números inteiros de 1 a 5. Use um `for` para percorrer a lista e imprimir cada número dobrado.

---

### Nível ★★ (Médio - Estruturas e Comportamento)

**6. Structs e Update**
Crie um struct `Product` com `name: string` e `price: float`. Crie um produto. Depois, use a sintaxe de *struct update* (`with`) para criar uma cópia com um preço novo.

**7. Enums com dados**
Crie um enum `Event` com três variações: `Click`, `KeyPress(key: string)` e `Quit`. Instancie o enum como `KeyPress` e use um `match` para extrair e imprimir a tecla pressionada.

**8. Closures e Pipes**
Crie uma lista de strings `["a", "b", "c"]`. Crie uma closure (função anônima) que recebe uma string e retorna ela maiúscula. Use o operador pipe (`|>`) para processar uma variável passando pela closure.

**9. Traits (Interfaces)**
Crie uma trait `Describable` com um método `describe(self) -> string`. Aplique essa trait (usando `apply` e `use`) no struct `Product` do exercício 6.

**10. Argumentos nomeados**
Crie uma função `connect(host: string, port: int)`. Chame essa função usando a sintaxe de argumentos nomeados (ex: `connect(host: "localhost", port: 8080)`).

**11. Opcionais e Bindings**
Crie uma função que procura um número em uma lista e retorna `optional[int]`. Chame a função e use `if some(valor) = resultado` para extrair o valor de forma segura.

---

### Nível ★★★ (Avançado - Fluxos Complexos)

**12. Genéricos e Constraints**
Escreva uma função genérica `first[T](items: list[T]) -> optional[T]` que retorna o primeiro item de qualquer tipo de lista. Teste passando uma lista de inteiros e uma de strings.

**13. Tratamento de Erros com Try**
Simule uma função `read_config() -> result[string, string]` que retorna um erro se algo der errado. Crie outra função `app_start()` que chama a leitura de config usando `try`. Lide com o resultado.

**14. Recursos e Cleanup**
Escreva uma função que usa a sintaxe `using` para criar uma variável "falsa" de conexão. Faça ela imprimir algo, e prove que o recurso seria finalizado no momento do `end` do bloco.

**15. Funções Assíncronas**
Escreva uma função `async fetch_data() -> string` que dorme (usando `ori.task`) por 1 segundo e retorna um dado. Chame-a em outra função usando `await`.

**16. Re-exports e Módulos**
Crie um arquivo de módulo falso. Nele, importe uma funcionalidade de `ori.string` usando `public import` (re-export). Explique em um comentário por que isso é útil para quem consumir o seu módulo.

**17. Guards no `match`**
Escreva uma função `classify(n: int) -> string` que usa `match` com **guards**
(`case n if condição:`) para devolver `"negativo"`, `"zero"` ou `"positivo"`.
Lembre-se: o compilador ainda exige um `case else:` mesmo com guards, porque
ele não consegue provar sozinho que as condições cobrem todos os casos.
