# Async e concorrência

> **Público:** programas que esperam, agendam trabalho ou compartilham valores
> **English:** [concurrency.md](concurrency.md)
> **Referência:** [07-functions.md](../spec/07-functions.md), [10-memory.md](../spec/10-memory.md) e [12-stdlib.md](../spec/12-stdlib.md)

O backend nativo oferece um modelo async pequeno e explícito. O usuário não
manipula threads do SO nem ponteiros crus: tasks, channels, futures e atomics
são valores tipados do runtime.

## `async` e `await`

Uma função async retorna `future[T]`; dentro dela, `await` transforma
`future[T]` em `T`:

```ori
async delayed_answer() -> int
    await task.sleep(1)
    return 42
end
```

`await` só aparece em funções `async`. Valores gerenciados vivos durante a
suspensão ficam retidos no frame. `using` continua funcionando e libera o
recurso em retorno, erro, cancelamento, `try` e saída de loop.

O backend C/debug rejeita async. Shapes async fora do subconjunto nativo são
rejeitados antes do codegen.

## Tasks, channels e atomics

`task.spawn` executa uma closure sem argumentos; `task.join` retorna `result`.
Valores que atravessam tasks ou channels precisam satisfazer `Transferable`, e
uma closure de `task.spawn` não pode capturar uma `var` mutável. Channels são
tipados e `atomic.AtomicInt` oferece load/store/add atômicos, não um mutex.

Veja os exemplos executáveis em [`examples/async_demo`](../../examples/async_demo)
e [`examples/concurrency`](../../examples/concurrency).

## Cancelamento estruturado e transferência entre threads

Escopos estruturados de cancelamento no módulo `ori.cancel` permitem árvores determinísticas de cancelamento:

```ori
import ori.cancel = cancel

const scope: cancel.CancelScope = cancel.create_scope()
if cancel.is_cancelled(scope)
    return
end
cancel.defer_cancel(scope, 500) -- cancela após timeout
```

Transferências seguras entre threads em `ori.concurrent` (`transfer_int`, `transfer_string`, `transfer_list_string`) garantem isolamento de dados entre threads sem riscos de corrida de memória.

`task.block_on` é uma ponte síncrona explícita: aguarda a resolução do future drenando o executor e o reator nativo de eventos (`ori_reactor_poll`).

## Iteradores

`iter` + `suspend` é outro mecanismo: gera valores inline e só pode ser
consumido diretamente por `for`, com limitações documentadas. Para um iterador
armazenável, use um estado explícito que implemente o contrato da stdlib.
