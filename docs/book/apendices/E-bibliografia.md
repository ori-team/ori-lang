# Apêndice E — Bibliografia

> **Versão âncora:** Ori 0.3.x (S3)

## TL;DR
Uma lista recomendada de livros e artigos externos que influenciaram o design da linguagem Ori e que podem te ajudar a entender melhor como linguagens funcionam por debaixo dos panos.

---

A linguagem Ori não foi criada no vácuo. Ela pega emprestado ideias de várias outras linguagens modernas (como Rust, Swift, Zig e Go) e tenta simplificá-las para reduzir a carga cognitiva.

Se você tem interesse em se aprofundar na teoria por trás disso, aqui estão nossas recomendações de leitura:

### Construção de Compiladores (Compiler Construction)

- **"Crafting Interpreters"** por Bob Nystrom.  
  *Por que ler:* É o melhor e mais acessível livro já escrito sobre como criar uma linguagem do zero. Ele foge do jargão acadêmico denso (como a "teoria dos autômatos") e mostra código prático construindo duas linguagens passo-a-passo.

- **"Engineering a Compiler"** por Keith Cooper e Linda Torczon.  
  *Por que ler:* Se você quer ir além dos interpretadores e entender otimizações de nível industrial e geração de código de máquina real (como o Cranelift faz no Ori), este livro traz a fundação de engenharia pesada.

### Design de Linguagem de Programação (PL Design)

- **"Types and Programming Languages" (TAPL)** por Benjamin C. Pierce.  
  *Por que ler:* A "Bíblia" dos sistemas de tipos. O Ori tenta manter a tipagem simples, mas toda a lógica por trás de como garantimos que você não some uma String com um Inteiro de forma matemática estrita está fundamentada nas teorias deste livro.

- **The Rust Programming Language Book (O Livro do Rust)**  
  *Por que ler:* Ori pega forte inspiração nos sistemas de Result, Tratamento de Erros por valor, e Traits do Rust. O livro oficial deles explica brilhantemente por que essas escolhas de design resolvem os maiores problemas do software moderno.

### Carga Cognitiva e Acessibilidade (Cognitive Load & Plain Language)

- **"The Programmer's Brain"** por Felienne Hermans.  
  *Por que ler:* Um livro revolucionário que explica como nossa memória funciona durante a programação. O design sintático do Ori (evitar símbolos complexos, preferir palavras como `end` e `and`) é focado em reduzir a carga na Memória de Curto Prazo, conceitos perfeitamente explicados por Hermans.

- **"Design for Cognitive Bias"** por David Dylan Thomas.  
  *Por que ler:* Explica como a maneira que apresentamos a informação afeta pessoas de formas diferentes. As regras de documentação do projeto Ori (direto ao ponto, TL;DR primeiro) são baseadas nesses princípios de acessibilidade para mentes neurodivergentes.
