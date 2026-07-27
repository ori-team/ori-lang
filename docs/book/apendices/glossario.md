# Glossário

> **Versão âncora:** Ori 0.3.x (S3)

## TL;DR
Um dicionário rápido e direto de todos os termos técnicos usados neste livro e na linguagem Ori, sem jargões desnecessários.

---

### A
- **ABI (Application Binary Interface):** O acordo técnico de como os dados de um programa são organizados na memória, permitindo que ele converse com programas feitos em outras linguagens.
- **Alias:** Um apelido que você dá para facilitar o uso. Exemplo: um apelido mais curto para o caminho de um módulo importado.
- **AST (Abstract Syntax Tree):** A representação em formato de "árvore" do seu código, gerada pelo compilador para entender a estrutura gramatical que você digitou.
- **Async:** Um pedaço de código que pode ser pausado enquanto espera algo (como a internet) sem travar o resto do programa.
- **Await:** O comando usado para esperar um código `Async` terminar o seu trabalho.

### B
- **Binding:** O ato de atrelar um valor a um nome. No Ori, variáveis criadas com `const` ou `var` são chamadas de bindings.
- **Block:** Um agrupamento de linhas de código, geralmente começando após uma declaração e terminando sempre com a palavra `end`.
- **Breaking Change:** Uma atualização na linguagem que faz com que códigos antigos parem de funcionar e precisem ser reescritos.

### C
- **Callback:** Uma função que você passa como argumento para outra função, para ser executada depois que algo acontecer.
- **C ABI:** O formato padrão universal (da linguagem C) de organizar memória. Ori suporta isso, permitindo integração fácil em sistemas antigos.
- **Cdylib:** Uma biblioteca dinâmica compilada no formato C.
- **Closure / Lambda:** Uma função anônima, sem nome, geralmente curta, que pode ser salva em variáveis ou passada como argumento.
- **Codegen (Geração de Código):** A parte do compilador que transforma a sua intenção de código (AST/IR) em instruções binárias para o processador.
- **Concurrent (Concorrência):** A capacidade do programa de lidar com várias tarefas "ao mesmo tempo" intercalando elas rapidamente.
- **Constant (Constante):** Uma variável criada com `const` cujo valor não pode ser alterado depois de inicializado.
- **Constraint / Trait Bound:** Uma regra que diz "esta função genérica só aceita tipos que tenham esta característica (trait) específica".
- **Cranelift:** O gerador de código interno que o Ori usa para compilar programas de forma extremamente rápida.
- **Cycle Collector:** Uma engrenagem de gerenciamento de memória que limpa variáveis que ficam referenciando uma à outra em um círculo infinito.

### D
- **Default Argument (Argumento Padrão):** O valor que uma função assume para um argumento caso você esqueça ou escolha não passar nada.
- **Deque:** Uma lista especial onde você pode adicionar e remover itens tanto do começo quanto do final rapidamente.
- **Deterministic Cleanup:** A garantia de que um recurso (como um arquivo aberto) será fechado imediatamente na linha exata em que o bloco `using` termina.
- **Dispose:** A ação manual ou automática de liberar recursos de memória ou sistema.

### E
- **Entry Point:** A função principal de onde o seu programa começa a rodar. No Ori, é a função `main()`.
- **Enum Variant:** Uma das opções possíveis dentro de um tipo de enumeração (enum).
- **Exhaustive (Exaustivo):** O compilador checa se você lidou com *todas* as possibilidades (ex: num bloco de `match`).
- **Expression (Expressão):** Qualquer pedaço de código que produz e devolve um valor (ex: `2 + 2`).

### F
- **FFI (Foreign Function Interface):** O sistema que permite que um código Ori chame funções escritas em outras linguagens, como C ou C++.
- **FREEZE:** O momento de desenvolvimento em que não se aceita mais invenções sintáticas; apenas correção de bugs visando estabilidade.
- **Future:** Uma "promessa" de um valor que só estará disponível no futuro (relacionado ao Async).

### G
- **Generic (Genérico):** Um molde de código que funciona para diferentes tipos. Exemplo: uma lista que pode ser de `int` ou de `string`.
- **Guard:** Uma condição extra num `case` de `match` (`case n if n > 0:`). Se o padrão casar mas o guard for falso, o `match` tenta o próximo `case` — o corpo daquele braço não roda.

### H
- **Heap:** A parte "lenta" da memória do computador onde colocamos dados que mudam de tamanho ou duram bastante tempo.

### I
- **Immutable (Imutável):** Algo que não pode ser modificado depois de criado. Protege o código de erros inesperados.
- **Import:** Trazer funcionalidades e código de outro arquivo para dentro do seu.
- **IR (Intermediate Representation):** A linguagem secreta e interna que o compilador gera para analisar seu código antes de criar o executável final.
- **Iterator:** Uma ferramenta de coleções que permite buscar o "próximo" item um por vez.

### L
- **Layer 1/2/3:** As camadas da Biblioteca Padrão do Ori, dividindo o que é crucial do que é de alto nível.
- **Lexer:** A parte do compilador que lê as palavras e pontuações do seu arquivo e separa em pequenos pedaços (tokens).
- **Linker:** O programa que "cola" todas as partes traduzidas do seu código com as bibliotecas do sistema para formar o executável final `.exe` ou arquivo rodável.

### M
- **Manifesto (arquivo):** O `ori.proj` — arquivo **TOML** (não sintaxe Ori) na raiz de um projeto, que declara nome, tipo (`app`/`lib`), ponto de entrada e namespace. Não confundir com o **Manifesto da linguagem** (`docs/spec/00-manifesto.md`), o documento que descreve o propósito e a filosofia da Ori — são dois arquivos diferentes com o mesmo nome comum.
- **Module:** O agrupamento de arquivos de código. No Ori, todo arquivo declara a que módulo pertence.
- **Mutable (Mutável):** Uma variável criada com `var` que pode ter seu conteúdo substituído por novos valores.

### N
- **Named Argument (Argumento Nomeado):** A possibilidade de chamar uma função dizendo explicitamente o nome de cada parâmetro que você está passando (ex: `connect(port: 80)`).

### P
- **Parser:** O sistema do compilador que lê os tokens produzidos pelo Lexer e garante que a gramática faz sentido.
- **Pattern Matching:** O poder do bloco `match` de olhar o formato ou tipo de um dado estruturado e extrair valores de dentro dele.
- **Poetic Call (Chamada Poética):** Chamar uma função de um argumento sem parênteses (`io.print "oi"` em vez de `io.print("oi")`). Só vale para uma chamada por linha — encadear duas seguidas é erro.
- **Public / Visibility:** A palavra `public` torna uma função ou tipo visível para ser usada por outros arquivos ou módulos. Sem ela, a visibilidade é privada.

### R
- **Range (Intervalo):** Um objeto que representa números de um ponto a outro. Ex: `1..5`.
- **Reference Counting:** Uma técnica de gerenciamento de memória que apenas "conta" quantas partes do código estão usando um valor e limpa quando a contagem chega a zero.
- **Reexport (Public Import):** A ação de importar algo em um arquivo e imediatamente deixá-lo público para que quem importe o seu arquivo também o tenha.
- **Resolver:** A etapa do compilador que descobre para qual arquivo cada `import` está apontando.
- **Runtime:** O "motor" por baixo dos panos que cuida da memória, testes e concorrência enquanto o seu programa final está executando.

### S
- **Scope (Escopo):** O tempo de vida de uma variável. Ela só é válida dentro do bloco (entre declaração e `end`) onde foi criada.
- **Semantic Versioning:** O padrão (Maior.Menor.Correção, ex: 1.2.4) de numerar as atualizações dizendo o quão drástica é a mudança.
- **Set (Conjunto):** Uma coleção de dados desordenada onde nunca existem itens repetidos.
- **Stack (Pilha):** A parte super rápida da memória usada para dados simples que nascem e morrem junto com uma função.
- **Statement (Declaração):** Uma linha de código que executa uma ação mas não produz um valor de volta (ex: `io.println("oi")`).
- **Staticlib:** Uma biblioteca empacotada diretamente no seu executável, sem precisar de arquivos avulsos.
- **Struct Update:** A sintaxe de pegar um struct antigo e copiar para um novo, trocando apenas o valor de alguns campos usando `with`.
- **Surface (Superfície):** As regras, palavras-chave e sintaxe que você, programador, digita (em contraste com o que acontece internamente).

### T
- **Token:** As menores unidades que o compilador entende, como um nome solto, um número ou um símbolo de `+`.
- **Tuple (Tupla):** Um pequeno conjunto fixo de valores diferentes agrupados, usado para retornar vários valores de uma função.
- **Type Alias:** O ato de criar um nome alternativo e mais claro para um tipo já existente.
- **Type Checker:** A polícia do compilador que assegura que você está passando os dados corretos (ex: barramos passar texto onde se pede número).

### V
- **Value Contract (Contrato de Valor):** Uma regra escrita diretamente na assinatura da função que recusa valores que não cumpram um requisito (ex: `if it >= 0`).
- **Value Semantics:** O comportamento onde as coisas são copiadas, e não ligadas, passando os dados de forma que alterar um lugar não estrague misteriosamente outro.
- **Variable (Variável):** Um espaço na memória atrelado a um nome para guardar um valor mutável.
- **Variadic:** Uma função que tem a capacidade de receber quantos argumentos extras você quiser fornecer de uma só vez.
