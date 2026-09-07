# Cap. 6 — Pipeline do compilador

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

Quando você roda um código Ori, ele passa por uma linha de montagem chamada "pipeline". O compilador fatia o texto, entende a gramática, verifica os tipos e, por fim, transforma tudo em instruções que o computador entende.

## Exemplo

Veja o caminho que o seu código percorre quando você digita `ori run main.orl` no terminal:

```text
1. O texto do arquivo (.orl)
      ↓
2. Lexer (quebra em palavras)
      ↓
3. Parser (monta frases e árvores — a AST)
      ↓
4. Type Checker (resolve nomes e garante que os tipos combinam)
      ↓
5. Lowering para HIR (uma árvore mais simples, já com os tipos decididos)
      ↓
6. Monomorfização (gera uma cópia concreta de cada função genérica usada)
      ↓
7. Codegen (Cranelift ou C — gera o código final)
```

## A Jornada do Código

Vamos usar uma analogia para entender o que acontece quando você compila um programa. Imagine que você escreveu um livro de receitas e quer que um robô cozinheiro o prepare.

### Passo 1: O Lexer (Separando os Ingredientes)

O seu código é apenas um longo texto. O "Lexer" é a primeira ferramenta que lê esse texto. A função dele é quebrar as frases em pequenos pedaços chamados "tokens". 
Por exemplo, se o texto tem `const x = 5`, o Lexer separa em: a palavra `const`, a letra `x`, o sinal `=` e o número `5`. É como listar os ingredientes da receita.

### Passo 2: O Parser (Montando a Estrutura)

Com os ingredientes listados (os tokens), o "Parser" tenta dar sentido a eles. Ele constrói uma estrutura chamada AST (Árvore Sintática Abstrata). 
O Parser verifica se a gramática está correta. Ele não sabe o que é `x`, mas sabe que a frase "guarde o número 5 na constante x" é uma frase válida.

### Passo 3: O Type Checker (Controle de Qualidade)

O "Type Checker" (Verificador de Tipos) é o fiscal rigoroso. Ele resolve
cada nome (se você usou uma função chamada `add`, ele procura em todo o
projeto onde ela foi definida) e garante que você não está tentando somar
um texto com um número. Se ele encontrar um erro, a compilação para aqui e
mostra uma mensagem amigável para você arrumar.

### Passo 4: Lowering para HIR (Simplificando a Receita)

Com os tipos já decididos, o compilador reescreve a árvore numa forma mais
simples de processar, chamada **HIR** (*High-level Intermediate
Representation* — Representação Intermediária de Alto Nível). É como
reescrever uma receita cheia de gírias culinárias numa lista de passos
diretos, sem perder nenhuma informação.

### Passo 5: Monomorfização (Uma Cópia por Tipo Usado)

Se você escreveu uma função genérica como `identity[T](value: T) -> T`, ela
não existe "de verdade" no código de máquina final — ela é só um molde. A
**monomorfização** gera uma cópia especializada para cada tipo que você
realmente usou (`identity[int]`, `identity[string]`, …). O nome vem daí:
"mono" (uma) + "morph" (forma) — o molde genérico vira várias formas
concretas, uma por combinação de tipo usada no seu programa.

### Passo 6: Codegen — dois backends, o mesmo HIR

Se tudo estiver perfeito, o HIR chega ao "Codegen" (Gerador de Código). A
Ori usa o **Cranelift**, uma biblioteca de geração de código nativo,
por meio de `ori run`, `ori compile` e `ori build`. A emissão de código C
foi removida; FFI C e headers de exportação nativos continuam disponíveis.

## AOT vs JIT: Duas formas de executar

A Ori oferece dois caminhos no final do pipeline, dependendo do que você precisa:

* **AOT (Ahead-of-Time):** Usado quando você roda `ori compile`. O compilador gera um arquivo executável pesado e completo (um `.exe` ou arquivo binário). Você pode enviar esse arquivo para um amigo e ele vai rodar sem precisar instalar a Ori. É como entregar o bolo pronto.
* **JIT (Just-in-Time):** Usado quando você roda `ori run`. O código é compilado e executado imediatamente na memória do seu computador, sem gerar arquivos extras. É perfeito para testar coisas rápido durante o desenvolvimento. É como o cozinheiro preparar e você comer na mesma hora.

## O que memorizar

* O Lexer transforma texto em tokens; o Parser organiza os tokens numa árvore (AST).
* O Type Checker resolve nomes e impede que você misture dados incompatíveis.
* Depois do type check, o código vira HIR e a monomorfização especializa cada genérico usado.
* O Cranelift converte o HIR em código de máquina nas rotas nativas AOT/JIT.
* AOT gera um arquivo final pronto; JIT roda na memória para testes rápidos.
