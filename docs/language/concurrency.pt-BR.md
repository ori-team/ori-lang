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
suspensão ficam retidos no frame. A emissão nativa verifica limites/layout dos
slots e zera bindings gerenciados antes de agendar; a prova completa de
ownership no HIR ainda está pendente. `using` continua funcionando e libera o
recurso em retorno, erro, cancelamento, `try` e saída de loop.

O backend C/debug rejeita async. Shapes async fora do subconjunto nativo são
rejeitados antes do codegen.

## Tasks, channels e atomics

`task.spawn` executa uma closure sem argumentos; `task.join` retorna `result`.
Valores que atravessam tasks ou channels precisam satisfazer `Transferable`, e
uma closure de `task.spawn` não pode capturar uma `var` mutável nem ler ou
escrever diretamente uma `var` mutável de módulo. O checker emite
`concurrency.global_mutable_capture`. O checker também segue chamadas para
helpers do mesmo módulo e helpers nomeados importados (inclusive funções
associadas) por uma análise conservadora de ponto fixo. Chamadas por receptor
e despacho de traits dinâmicas (`any[Trait]` ou `Trait`) usam um resumo conservador por nome de método; assim,
um método que possa tocar uma `var` global mutável é rejeitado na fronteira.
Uma função nomeada passada diretamente é aceita quando seu resumo prova que
ela não toca uma `var` global mutável. Uma closure guardada em binding local
também é aceita quando o checker registra apenas capturas transferíveis;
ambientes de função desconhecidos e capturas de funções aninhadas continuam
conservadores. Channels são tipados e
`atomic.AtomicInt` oferece load/store/add atômicos, não um mutex.

Handles de recurso (`fs.File`, `io.Input`, `io.Output`, `net.Connection`,
`net.Listener` e `net.UdpSocket`) nunca são transferíveis: eles emprestam estado
do processo/SO. `task.CancelToken` é a exceção explícita, pois contém apenas uma
flag atômica de cancelamento feita para coordenação entre tasks.

`channel.create` cria uma fila FIFO sem limite. Para limitar memória, use
`channel.create_bounded(capacity)`, que retorna
`optional[channel.Channel[T]]`: capacidades positivas produzem um channel
limitado; zero ou negativas produzem `none`. Quando a fila limitada está cheia,
`send` espera até um receive liberar espaço. `close` acorda senders bloqueados,
que recebem `err(...)`. Payloads gerenciados em channels são suportados quando o tipo satisfaz
`Transferable`; o runtime retém os valores enfileirados e os libera no receive,
no close ou na destruição. Operações de readiness de rede retêm seus handles e
sincronizam o close: fechar um handle pendente produz o resultado documentado,
sem acessar memória já liberada.

Veja os exemplos executáveis em [`examples/async_demo`](../../examples/async_demo)
e [`examples/concurrency`](../../examples/concurrency).

## Tokens de cancelamento e auxiliares de transferência

`ori.cancel` atualmente encapsula tokens de cancelamento. Ele ainda não possui
nem aguarda uma árvore de tarefas filhas, portanto não é um escopo completo de
concorrência estruturada:

```ori
import ori.cancel as cancel

const scope: cancel.CancelScope = cancel.create_scope()
if cancel.is_cancelled(scope)
    return
end
cancel.cancel(scope)
```

`cancel.defer_cancel` é assíncrono: use `await` ao empregá-lo como timeout. Ele
espera o atraso solicitado antes de cancelar o escopo. A árvore de tarefas
filhas e o cancelamento automático ao sair do escopo continuam pendentes em
`ASYNC-STRUCT-1`.

Os auxiliares de `ori.concurrent` (`transfer_int`, `transfer_string`,
`transfer_list_string`) copiam valores selecionados. Eles ainda não definem um
modelo completo de transferência e ownership para valores gerenciados arbitrários.

`task.block_on` é uma ponte síncrona explícita: aguarda a resolução do future
drenando o executor. `ori_reactor_poll` atualmente espera na fila do executor;
no Unix, prontidão de rede usa também um worker separado com `poll`.

Operações bloqueantes de filesystem, conexão e TLS não criam uma thread por
requisição. Elas compartilham um pool nativo limitado a quatro workers e uma
fila FIFO de 256 jobs. Quando a fila está cheia, o envio espera espaço. Se o
runtime estiver encerrando ou não conseguir criar um worker, o future termina
com falha, sem deixar um job sem dono.

## Iteradores

`iter` + `suspend` é outro mecanismo: gera valores inline e só pode ser
consumido diretamente por `for`, com limitações documentadas. Para um iterador
armazenável, use um estado explícito que implemente o contrato da stdlib.
