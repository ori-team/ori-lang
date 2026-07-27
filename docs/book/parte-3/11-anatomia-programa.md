# Cap. 11 — Anatomia de um programa

> **Versão âncora:** Ori 0.3.x (S3)
> **Parte:** III

## TL;DR
Todo arquivo começa com `module`. A entrada principal do programa é a função `main()`. Não usamos a palavra `func` para criar funções. Blocos de código terminam sempre com a palavra `end`. Comentários começam com `--`.

## Exemplo: O esqueleto de um programa

```ori
module app.hello

-- Importa a biblioteca de entrada e saída
import ori.io = io

-- Função principal do programa
main()
    -- Usa o alias 'io' criado no import
    io.print("Iniciando sistema...")
    
    -- Declaramos variáveis com 'var' (podem mudar) e 'const' (fixas)
    var user_count: int = 0
    const app_name: string = "Meu App"
    
    -- Texto formatado (interpolação) usando f"..."
    io.print(f"Bem-vindo ao {app_name}! Usuários: {user_count}")
end
```

## Como funciona o cabeçalho

### Por que `module` é obrigatório?
A primeira linha de qualquer arquivo Ori **deve** ser `module nome.do.modulo`. Isso define a identidade do arquivo. Sem isso, o compilador não sabe como conectar esse arquivo com os outros. Pense nisso como o endereço daquele código dentro do seu projeto.

### O sistema de Imports
Para usar funções de outros arquivos, você precisa importá-las. Existem algumas formas de fazer isso:

*   **Import com apelido (alias):** `import ori.io = io`. O caminho fica à esquerda e o apelido à direita.
*   **Import seletivo:** `import ori.fs (read_text)`. Traz apenas funções específicas daquele caminho.
*   **Import direto:** `import ori.math`. Sem apelido, você precisará digitar o caminho completo ao usar as funções (ex: `ori.math.add()`).
*   **Bloco de imports:** Quando precisar trazer muitas coisas, agrupe para ficar mais limpo:
    ```ori
    imports
        ori.io = io
        ori.fs = fs
        ori.net (connect)
    end
    ```

## Constantes, Variáveis e Inferência

### `const` vs `var`
Use `const` quando o valor nunca vai mudar, e `var` quando precisar atualizar o valor depois.

```ori
-- const não pode ser modificado
const max_retries: int = 3

-- var pode ser alterado
var count: int = 0
count = count + 1
```

### Inferência de tipos local
Você não precisa escrever o tipo o tempo todo se for óbvio para o compilador. Essa é a inferência local:

```ori
-- Explícito
const a: int = 10

-- Implícito (o Ori sabe que 10 é int)
const b = 10

-- Implícito com retorno de função (se get_id retorna string)
const user_id = get_id() 
```

Mas atenção: na assinatura de funções, os tipos são sempre obrigatórios.

## Funções e o ponto de entrada `main`

Não usamos palavras como `func` ou `fn` no Ori S3. Basta colocar o nome e os parâmetros:

```ori
soma(a: int, b: int) -> int
    return a + b
end
```

### A função void (sem retorno)
Se a função só faz uma ação e não devolve nada útil, o tipo é `void` (vazio). Como `void` é o padrão, você pode simplesmente omitir a seta `->` na declaração:

```ori
log_erro(msg: string)
    io.print(f"ERRO: {msg}")
end
```

### O ponto de entrada `main()`
Todo programa que pode ser executado precisa de uma função principal chamada `main`. Ela pode não ter retorno (void), ou pode retornar um erro para o sistema caso algo falhe de forma grave (por exemplo, `result[void, string]`).

## O que memorizar
*   Sempre comece o arquivo com `module`.
*   Funções não usam a palavra `func`.
*   Blocos de código terminam com `end`.
*   Comentários começam com `--`.
*   Use `f"texto {variavel}"` para formatar strings facilmente.
