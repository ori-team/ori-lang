# Plano de implementação — controle e observabilidade de runtime

> **Status:** aprovado para implementação por capacidades independentes.  
> **Baseline:** `result`, futures, cancel token, tasks, ARC diagnostics e DAP já
> existem; os recursos abaixo ainda não formam um contrato de usuário.

## 1. Resultado desejado

Aplicações longas devem explicar erros, compor tarefas, reproduzir simulações e
medir o runtime sem depender de variáveis internas do compilador.

O programa reúne quatro capacidades gerais:

1. contexto de propagação de `result`;
2. structured concurrency;
3. RNG como valor independente;
4. métricas, leak report e tracing opt-in.

## 2. Estado real

- `try` propaga o payload, mas não registra por quais funções ele passou;
- não há `select`, `race`, timeout composto ou escopo de tarefas filhas;
- `random.seed` altera estado global do processo;
- `ORI_DUMP_ARC` e `ORI_TEST_LEAK_CHECK` são mecanismos de desenvolvimento;
- DAP possui frames, mas aplicações normais não recebem spans de tracing;
- não há `SlotMap[T]`/pool geracional canônico para recursos de longa duração.

## 3. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **RUNTIME-CTRL-1.0** | medição e contrato de observabilidade | custo desligado mensurado como zero ou ruído |
| **RUNTIME-CTRL-1.1** | error return trace opt-in | cadeia de `try` mostra arquivo/função/linha sem alterar `E` |
| **RUNTIME-CTRL-1.2** | timeout/race/select | loser é cancelado e cleanup terminal é executado |
| **RUNTIME-CTRL-1.3** | task scopes | escopo não termina com filho perdido; erro/cancelamento propagam |
| **RUNTIME-CTRL-1.4** | `Random` por valor | dois seeds iguais produzem sequências iguais e independentes |
| **RUNTIME-CTRL-1.5** | leak report e contadores | CLI/API mostra allocations, bytes e leaks com schema estável |
| **RUNTIME-CTRL-1.6** | tracing estruturado | spans sync/async correlacionam task, módulo e host callback |
| **RUNTIME-CTRL-1.7** | `SlotMap[T]` genérico | handle antigo é rejeitado após remoção/reuso do slot |

## 4. Decisões

- error traces são metadata paralela e opt-in; não modificam todo
  `result[T,E]` nem introduzem exception;
- structured concurrency deve usar APIs/collections antes de nova sintaxe;
- RNG global pode permanecer como helper, mas bibliotecas reutilizáveis recebem
  `Random` explicitamente;
- métricas não expõem ponteiros nem layout ARC privado;
- tracing aceita um sink do host no perfil embedded.

## 5. Validação

- sucesso sem alocação ou branch adicional quando tracing está desligado;
- `try` profundo com trace e sem trace;
- cancelamento durante await, recurso `using` e custom destructor;
- race determinística e ausência de task órfã;
- RNG isolado entre threads/tasks e vetores de teste versionados;
- stress de SlotMap com wrap/reuse e handles stale;
- DAP, Host ABI e CLI consumindo o mesmo schema de evento.

## 6. Fora de escopo

- exceptions como fluxo normal;
- profiler de CPU completo dentro do runtime;
- garantia determinística para código nativo/FFI arbitrário;
- entity/component storage na linguagem.
