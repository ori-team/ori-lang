# Cap. 2 — O que é a linguagem Ori?

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

Ori é uma linguagem de programação criada para ser **fácil de ler**, **segura** e **rápida**. Você escreve o código de forma limpa (como um texto bem organizado) e o computador o transforma em um programa ultrarrápido, sem que você precise gerenciar a memória manualmente.

## Um exemplo para começar

Antes da teoria, veja como é a "cara" de um programa em Ori:

```ori
module app.hello

import ori.io as io

main()
    -- Escrevemos na tela de forma simples
    io.println("Olá, Ori!")
    
    -- Criamos um valor constante (que não muda)
    const resposta: int = 21 * 2
    
    -- Juntamos texto e números facilmente (o "f" indica texto formatado)
    io.println(f"A resposta é {resposta}")
end
```

> **Note os detalhes visuais:** Não usamos chaves `{ }` ou ponto e vírgula `;` no final de cada linha. Usamos a palavra `end` para fechar os blocos. O foco da Ori é remover ruídos visuais para não cansar seus olhos e seu cérebro durante a leitura.

## As 3 grandes características da Ori

Para entender a Ori, você só precisa conhecer três pilares principais. Não se preocupe em decorar siglas, foque no que elas fazem por você:

### 1. Sintaxe "S3" (A Pele da Linguagem)
É a forma como você escreve o código. O "S3" é o nome da nossa versão atual de design de texto.
- **O que significa:** O código parece uma poesia ou um texto estruturado, baseado em palavras claras em vez de símbolos matemáticos densos.
- **O benefício:** Cansa muito menos a vista. Pessoas com TDAH, dislexia ou apenas sob forte carga de trabalho se beneficiam de uma estrutura visual previsível, sem uma "sopa de símbolos".

### 2. AOT e JIT (O Motor de Velocidade)
Essas duas siglas explicam como o seu texto vira um programa que o computador entende:

- **AOT (Ahead-of-Time / Antes do Tempo):** Quando você quer entregar seu programa pronto, a Ori o transforma em um arquivo executável "duro" e ultrarrápido *antes* de alguém usar (comando `ori compile`). É ótimo para velocidade final.
- **JIT (Just-in-Time / Na Hora):** Quando você está apenas programando e testando no seu computador, a Ori não perde tempo criando um arquivo final pesado. Ela lê seu código e o roda *na mesma hora* (comando `ori run`), acelerando seu ciclo de teste.

### 3. ARC (A Limpeza Automática)
- **O problema:** Quando um programa roda, ele precisa guardar informações na memória RAM. Se ele esquecer de limpar o que não usa mais, o computador trava ou fica lento. Em linguagens antigas, o programador tinha que limpar isso manualmente.
- **Como a Ori resolve:** Ela usa **ARC** (Contagem Automática de Referências). O próprio sistema vigia quantas vezes um dado está sendo usado no seu código. Quando esse número chega a zero (ninguém mais precisa do dado), a Ori joga o lixo fora sozinha. Você não precisa se preocupar em "liberar memória".

## O que memorizar

- A extensão dos seus arquivos será sempre **`.orl`** (exemplo: `main.orl`).
- Você foca na lógica e na leitura; a Ori cuida da velocidade e da limpeza de memória por você.
- Quando for testar rápido no dia a dia, use o comando `ori run`. 
- Quando for construir a versão final para entregar, use `ori compile`.
