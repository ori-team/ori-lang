# Cap. 4 — Mapa mental do ecossistema

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

O mundo da Ori é dividido claramente entre o que você usa para criar seus programas e o que faz a linguagem funcionar por trás das cortinas. Conhecer as pastas principais do projeto ajuda você a saber exatamente onde procurar exemplos e ferramentas sem se perder.

## Exemplo: A árvore da linguagem

Esta é a visão de como as coisas estão organizadas. Pense nisso como o mapa prático da sua cidade:

```text
ori-lang/
├── stdlib/            -- As ferramentas prontas para você usar (textos, listas)
├── examples/          -- Programas de exemplo para você testar e aprender
├── docs/book/         -- Este livro que você está lendo agora
├── compiler/          -- (Bastidores) O motor que transforma seu código em máquina
└── runtime/           -- (Bastidores) O pacote que faz seu programa rodar
```

## O que importa para quem programa em Ori

Quando você está apenas desenvolvendo seus projetos no dia a dia, a sua atenção vai ficar em poucos lugares fundamentais:

- **Seus arquivos `.orl`:** É onde você escreve a lógica da sua própria aplicação.
- **A pasta `stdlib/`:** É a "biblioteca padrão" da Ori. Quando você precisar saber como usar uma ferramenta embutida (como imprimir no console ou ler arquivos), os códigos originais aqui servirão como a melhor referência.
- **A pasta `examples/`:** Se você não sabe como começar algo, essa pasta tem programas completos e funcionais. É uma mina de ouro excelente para você copiar, colar e modificar.
- **O Livro (`docs/book/`):** O guia narrativo e tranquilo que você está lendo agora.

## O que importa para quem desenvolve o compilador

Se um dia você quiser mergulhar e modificar a própria linguagem Ori, precisará entender os motores internos que ficam nas outras pastas:

### O Compilador (`compiler/`)
É o cérebro da operação. Ele lê o seu código, analisa os tipos que você definiu, confere se não há erros visíveis e então traduz tudo para um formato que a máquina consiga rodar.

### O Tempo de Execução (`runtime/`)
Também chamado de "runtime", são pedaços de código de base cruciais que gerenciam a memória e a execução do seu programa enquanto ele está aberto.

Dentro dessas áreas técnicas, você pode ver o runtime gerando resultados para um **Triple** específico. Um "triple" (ou "alvo triplo") é apenas um texto padrão que descreve o seu computador detalhadamente (por exemplo: qual é a arquitetura do processador e qual é o sistema operacional). É assim que sabemos se estamos criando um programa final para rodar no Windows, no Linux ou no Mac.

Você também encontrará formatos de pacotes, como **staticlib** (uma biblioteca estática de código que é embutida para sempre dentro do seu programa final, de uma vez por todas) e **cdylib** (uma biblioteca dinâmica, semelhante a uma DLL no Windows, que o sistema só carrega na memória do computador quando o programa realmente precisar dela).

## O seu dia a dia

O fluxo de trabalho de quem usa a linguagem é direto e projetado para causar pouca ansiedade:
1. Você escreve seu código se baseando nos contratos explícitos da linguagem (como vimos nos primeiros capítulos).
2. Você chama as ferramentas do terminal para verificar se há erros.
3. Quando você precisa de inspiração ou simplesmente não lembra como escrever algo, você consulta a pasta de exemplos vivos em vez de precisar buscar em manuais longos e densos.

## O que memorizar

- Seu foco primário será o seu próprio código, a `stdlib/` (biblioteca padrão) e os `examples/`.
- O compilador e o runtime ficam intencionalmente em pastas separadas para simplificar sua visão e não poluir o ecossistema.
- O "triple" é simplesmente a identidade exata do seu computador (sistema operacional e processador).
