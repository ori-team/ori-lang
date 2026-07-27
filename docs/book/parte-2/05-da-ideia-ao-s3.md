# Cap. 5 — Da ideia à superfície S3

> **Versão âncora:** Ori 0.3.x (S3)
> **Parte:** II

## TL;DR

A Ori passou por uma grande mudança visual. Nós mantivemos as engrenagens internas (o motor) mas trocamos a forma como o código é escrito (a superfície). Esta nova sintaxe, chamada S3, tornou o código muito mais limpo e legível.

## Exemplo

Veja como um simples código evoluiu da sintaxe antiga (pré-S3) para a nova (S3):

```ori
-- Sintaxe antiga (pré-S3): pesada e cheia de ruídos
func process_user(id: int) -> optional[User] {
    let user_opt = db.get_user(id);
    if user_opt.is_some() {
        return user_opt;
    } else {
        return none;
    }
}

-- Sintaxe nova (S3): limpa, direta e sem palavras desnecessárias
module app.users

process_user(id: int) -> optional[User] =>
    -- Encadeamos operações diretamente com o pipe (|>)
    db.get_user(id) |> validate_user()
```

## O que é "Superfície S3"?

Quando falamos de linguagem de programação, o "motor" é como ela funciona por baixo dos panos (gerenciamento de memória, execução rápida). A "superfície" é a camada visual, a sintaxe que você digita todos os dias. 

A Ori começou com uma superfície muito tradicional. Ela exigia palavras como `func` para funções, chaves `{}` para blocos, e `let` para criar variáveis. Isso funcionava, mas o código ficava poluído. 

Durante o desenvolvimento, criamos um projeto experimental chamado Auk9. O objetivo do Auk9 era testar uma sintaxe muito mais leve. Nós percebemos que a sintaxe do Auk9 era muito superior, então decidimos trazê-la para a Ori. Chamamos essa união de S3 (Superfície 3).

## A Jornada: Por que mudamos?

No início, o código da Ori parecia uma mistura de linguagens antigas. O leitor perdia muito tempo decifrando símbolos em vez de focar no que o código fazia. 

Com a mudança para a sintaxe S3, eliminamos o ruído visual. As funções não precisam mais da palavra `func`. Os blocos de código terminam com um simples `end` em vez de chaves complexas. O resultado é um texto que parece quase poesia, muito mais amigável para pessoas com TDAH (Transtorno de Déficit de Atenção com Hiperatividade) ou dislexia.

### O operador Pipe (`|>`)

O Auk9 original não tinha o operador pipe (`|>`), mas percebemos que ele era essencial na Ori. O pipe pega o resultado de uma operação e o passa para a próxima, como uma linha de montagem. Isso evita que você tenha que ler o código de trás para frente.

## Inferência Local (Opção B)

Um dos grandes debates foi sobre "inferência de tipos". Inferência é quando o compilador adivinha o tipo de uma variável sem que você precise escrevê-lo. 

A Ori adota a "Inferência Local Opção B". Isso significa que você precisa declarar os tipos na entrada e saída das funções, mas dentro da função o compilador faz o trabalho duro. 

```ori
module app.math

-- O tipo de entrada (int) e saída (int) é obrigatório
calculate_total(price: int, tax: int) -> int
    -- O compilador sabe que 'total' é um 'int' automaticamente
    const total = price + tax
    return total
end
```

A "Opção B" permite inferência em casos específicos, como acessar campos ou usar o pipe. Isso nos dá o equilíbrio perfeito: o código fica curto, mas você nunca se perde sem saber qual é o tipo de um dado.

## O que memorizar

* A Ori separou o "motor" (como funciona) da "pele" (como se escreve).
* S3 é a sintaxe atual, mais leve e sem ruído visual.
* O operador pipe (`|>`) organiza a leitura da esquerda para a direita.
* A inferência local exige tipos nas funções, mas adivinha o resto.
