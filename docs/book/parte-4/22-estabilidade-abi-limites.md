# Cap. 22 — Estabilidade, ABI e limites

> **Versão âncora:** Ori 0.3.x (S3) · workspace `0.3.8-dev`
> **Parte:** IV

## TL;DR
Este capítulo explica como a linguagem Ori evolui. Discute o que significa estarmos em uma versão "Pré-1.0", o que é ABI (como o código interage por debaixo dos panos), e o modelo de congelamento de funcionalidades.

---

Quando escolhemos uma linguagem de programação para construir sistemas reais, queremos saber: "Se eu escrever meu código hoje, ele ainda vai compilar daqui a dois anos?". A resposta curta para o Ori é: nós levamos estabilidade muito a sério, mas há regras que você precisa entender sobre o nosso momento atual.

## 1. O que significa "Pré-1.0"?

Atualmente, o Ori está nas versões `0.x.x` (como 0.3.x). No mundo do software livre (via versionamento semântico), uma versão zero significa que **a linguagem ainda está encontrando sua forma ideal**. 

Na prática, isso significa que:
- Sintaxes podem mudar de uma versão menor para outra (como ocorreu da fase S2 para a fase S3).
- Tipos da biblioteca padrão podem ser renomeados.
- **Mas com cuidado:** Não quebramos seu código por diversão. Quando uma mudança que quebra compatibilidade acontece (Breaking Change), ela é amplamente documentada, e ferramentas de migração são fornecidas sempre que possível.

Quando atingirmos a versão `1.0`, o compromisso será: nenhum código quebrado. Um código feito na `1.0` funcionará na `1.99`.

## 2. O Conceito de FREEZE-1

Você pode ouvir desenvolvedores de Ori falarem sobre o "FREEZE-1" (Congelamento 1). 
Isso não significa jargão corporativo complicado. É apenas um acordo da comunidade: **"A partir deste ponto, paramos de inventar coisas novas e passamos a arrumar os bugs do que já existe para lançar a versão 1.0."**

Durante a fase de Freeze, a linguagem congela sua "superfície" (as palavras-chave, as regras). O foco vira totalmente estabilizar, deixar rápido e criar bons erros de compilação.

**Exemplo real:** a versão `0.3.7` só trouxe uma correção de bug (um guard
de `match` que era silenciosamente ignorado em tempo de execução) e um
comando novo de ferramenta (`ori update`) — nenhuma mudança na sintaxe da
linguagem. Isso é o FREEZE-1 funcionando: bugs se corrigem a qualquer
momento dentro da série `0.3.x`; sintaxe nova espera a próxima versão
menor.

## 3. ABI: Como os programas conversam

ABI significa *Application Binary Interface* (Interface Binária de Aplicação). 

**Explicando de forma simples:** Quando você cria um executável no Ori, ele se transforma em linguagem de máquina (zeros e uns). Se você quiser que o seu programa em Ori converse com um programa feito em C, ou uma biblioteca feita em Rust, eles precisam concordar em como os dados são empacotados na memória. O "acordo" de como esses dados são organizados na memória é a ABI.

A Ori tem um contrato de ABI nomeado (**`ori-native-abi-1`**) que define como
o binário gerado conversa com C: layout de tipos primitivos, do cabeçalho de
memória do ARC e as pontes públicas geradas pelo compilador. Os layouts
internos de coleções (`OriList`, `OriMap`, `OriSet`...) continuam privados.

**Onde isso vira prática, hoje:** o comando `ori compile --lib` gera uma
biblioteca compartilhada (`.so`/`.dll`/`.dylib`) com funções marcadas
`@c_export` visíveis para C, Python ou qualquer host que saiba carregar uma
lib nativa — é assim que a Ori pode virar um plugin dentro de outro
programa (por exemplo, uma engine de jogos).

**O que atravessa a fronteira hoje:** escalares (`int`, `float`, `bool`, …),
`string`, structs não genéricas com campos escalares, handles opacos para
estruturas gerenciadas e pontes tag + payload para `optional`/`result`. O
comando `ori compile --lib` gera um cabeçalho C com essas assinaturas e os
helpers de retain/release. Uma string Ori já é, na memória, um `const char *`
terminado em zero — então ela passa direto, sem conversão.

```ori
@c_export
public shout(name: string) -> string
    return "hello, " + name
end
```

```c
const char *s = shout("Ada");   /* "hello, Ada" */
ori_arc_release((void *) s);    /* o host libera */
```

**A regra de posse não é simétrica, e vale decorar:**

| Direção | Quem é dono |
|---------|-------------|
| String que **entra** (parâmetro) | O host. Ori nunca libera. |
| String que **sai** (retorno) | O host. Libere com `ori_arc_release`. |

Se você esquecer o `ori_arc_release`, o programa **vaza** — não quebra. O
ponteiro continua válido para sempre. É um bug lento, não um crash: 200 mil
chamadas sem liberar custam uns 18 MB; com liberação, o uso fica estável.

**O que ainda não atravessa diretamente:** `list`, `map`, `set`, arrays
dinâmicos, structs genéricas ou vazias, callbacks e layouts de coleções. Esses
tipos continuam atrás de handles ou são rejeitados com
`attr.c_export_bad_type`. "Integração com C" na Ori de hoje é real e funcional,
mas a assinatura precisa respeitar a lista fechada da [especificação ABI](../../spec/19-abi.md).

## 4. Limites e Armadilhas (Pitfalls)

Até que a versão 1.0 seja atingida, aqui estão alguns limites práticos da linguagem que usuários avançados devem ter em mente:

- **Macros:** Atualmente, Ori não suporta metaprogramação complexa ou macros (como Rust tem). Você deve gerar código usando scripts separados, se necessário.
- **Herança:** Ori não tem herança tradicional (orientação a objetos). Não perca tempo tentando simular classes. Use `structs` para dados e `traits` para comportamentos. É uma decisão de design que não vai mudar.
- **Recursão Infinita:** O compilador **não detecta** recursão sem fim em tempo
  de compilação — isso é indecidível no caso geral. O que mudou é o que acontece
  ao rodar: em vez de morrer em silêncio com um sinal do sistema, o programa
  agora avisa.

  ```text
  ori: stack overflow -- a function recursed until the stack ran out.
  ori: check for recursion without a base case, or move large local data to the heap.
  ```

  Ou seja: você continua responsável por escrever a condição de parada, mas
  quando errar vai saber exatamente o que houve. Recursão profunda e **limitada**
  (dezenas de milhares de níveis) roda normalmente.

## O que memorizar
- Estamos na fase 0.x (Pré-1.0). Atualizações podem exigir que você adapte partes do seu código.
- Após o FREEZE-1, e na versão 1.0, o código será retrocompatível para sempre.
- Ori sabe conversar nativamente com programas feitos em C por causa do suporte à C ABI.
- Não tente programar com Herança e Classes no Ori; abrace as structs e traits.
