# Cap. 9 — Desenvolver com assistência de IA

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

O projeto da linguagem Ori foi desenvolvido com forte parceria entre humanos e inteligência artificial (IA). Agentes de IA funcionam como colegas de trabalho incansáveis, mas é o humano quem sempre dá a palavra final nas decisões de design.

## Exemplo

O trabalho em equipe (humano + agente) na Ori funciona de forma estruturada:

1. **O humano** lê os princípios da linguagem e decide que o sistema precisa de uma nova função matemática.
2. **O agente de IA** escreve o código dessa função.
3. **O agente de IA** escreve um teste automatizado garantindo que a função não falha.
4. **O agente de IA** atualiza a documentação.
5. **O humano** revisa os arquivos gerados e aprova as mudanças finais.

## Uma Nova Forma de Construir Software

Construir um compilador e uma linguagem do zero é um esforço monumental. O projeto Ori assumiu, desde o primeiro dia, que agentes de IA estariam lendo e escrevendo partes do código. Isso significa que o repositório é um "laboratório explícito" onde humanos e máquinas coabitam.

Mas os agentes de IA podem se confundir. Para que eles sejam eficientes e não destruam o projeto, criamos o conceito de "Skills" (habilidades).

### O que são Skills?

Em vez de pedir para a IA "fazer de tudo", nós instruímos os agentes a agirem de acordo com papéis (skills) bem definidos. É como dar a um robô um crachá específico antes dele começar o turno:

* **Skill de Qualidade (clean-code):** O agente garante que as variáveis tenham nomes claros e que o código não passe de um certo tamanho. Ele simplifica regras complexas.
* **Skill de Documentação (living-docs):** Se a IA altera o comportamento de uma função, essa skill a obriga a ir no arquivo de documentação e atualizá-lo instantaneamente, para que código e manuais nunca fiquem dessincronizados.
* **Skill de Testes (ori-testing):** A IA nunca entrega um código solto. Ela sempre produz pequenos testes que provam que a alteração faz exatamente o que o humano pediu.

### A Divisão de Papéis (Quem manda em quem?)

Para que esse sistema não fuja do controle, a divisão de responsabilidades é rígida e sagrada.

| Papel do Humano (Você) | Papel do Agente de IA |
|------------------------|-----------------------|
| Decide as prioridades e diz "o que" será feito. | Executa as tarefas com precisão técnica. |
| Corta escopo (ex: "não precisamos dessa feature hoje"). | Propõe soluções e remendos rápidos. |
| Rejeita ideias que firam as regras de legibilidade. | Escreve blocos chatos de testes e registros. |

### Os Limites da IA (O que NÃO pedir)

A IA é uma aceleradora, não uma inventora de regras. Na Ori, humanos são proibidos de pedir certas coisas para a IA:

* **Sintaxe velha:** Nunca pedimos para a IA misturar a sintaxe antiga (pré-S3) com a nova, isso confunde tanto a máquina quanto o leitor.
* **Recursos fantasmas:** Não pedimos para a IA adivinhar regras que ainda não aprovamos. Toda especificação tem que virar código compilável de verdade; se for só "teoria" gerada pela IA, nós rejeitamos.
* **Mudança de curso:** O humano não terceiriza a filosofia da linguagem para o modelo de linguagem natural. O humano defende os princípios; a IA digita a lógica.

## O que memorizar

* Humanos focam no propósito e aprovação final; a IA foca na execução braçal e rápida.
* Código, testes e documentação andam sempre juntos quando construídos por agentes.
* As "skills" impedem a IA de alucinar, restringindo seu escopo de trabalho.
