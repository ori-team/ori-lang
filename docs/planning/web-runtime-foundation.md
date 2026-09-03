# Plano de implementação — fundação de runtime para web

> **Status:** aprovado como fundação geral; frameworks permanecem externos.  
> **Baseline:** TCP/TLS client, TCP server, UDP, async I/O e helpers HTTP/1.1
> básicos existem em `0.3.8-dev`.

## 1. Limite arquitetural

Ori core deve fornecer I/O, protocolos básicos e controle de recursos. Router,
ORM, templates, autenticação de aplicação e convenções MVC pertencem a packages
externos. O compilador não ganha sintaxe de rota, request ou controller.

## 2. Estado real

`ori.net.http` atual:

- monta request HTTP/1.1 em `string`;
- oferece GET/POST bloqueantes e conexão fechada por resposta;
- guarda headers como uma string única;
- lê a resposta inteira até um limite fixo;
- converte body para UTF-8, portanto não serve conteúdo binário geral;
- não implementa servidor HTTP, streaming, chunked bodies, keep-alive,
  backpressure, TLS server ou cancellation por request.

Também não há driver de banco oficial no core; bindings e pools devem nascer
como packages sobre FFI/Host ABI estáveis.

## 3. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **WEB-FOUND-1.0** | tipos HTTP orientados a bytes | headers tipados e body `bytes` sem perda UTF-8 |
| **WEB-FOUND-1.1** | parser/encoder HTTP/1.1 incremental | limites, chunked e mensagens parciais testados/fuzzados |
| **WEB-FOUND-1.2** | client streaming/keep-alive | upload/download não carregam corpo inteiro em memória |
| **WEB-FOUND-1.3** | server HTTP básico | accept, request, response e shutdown gracioso |
| **WEB-FOUND-1.4** | TLS server | certificados/config explícitos e testes locais herméticos |
| **WEB-FOUND-1.5** | cancellation/backpressure | cliente lento não bloqueia ou cresce memória sem limite |
| **WEB-FOUND-1.6** | tracing/context | request/task/host spans compartilham IDs estruturados |
| **WEB-FOUND-1.7** | contratos de packages de dados | driver/pool externo usa process/FFI sem API mágica no compilador |

## 4. Segurança

- limites de linha, headers, body e tempo são obrigatórios;
- parser aceita bytes não confiáveis e exige fuzzing;
- request smuggling e ambiguidades de `Content-Length`/`Transfer-Encoding`
  precisam de casos negativos;
- TLS não oferece opção “ignore certificate” como default;
- secrets não entram em diagnostics/tracing;
- capability de rede pode ser negada no perfil embedded.

## 5. Validação

- parser incremental fragmentado em todo byte possível;
- payload binário com NUL;
- keep-alive, chunked, timeout, cancelamento e disconnect parcial;
- milhares de conexões com orçamento de memória;
- TLS local com certificado de teste;
- benchmarks de latência e throughput sem promessas prematuras;
- exemplos canônicos client e server compilados em CI.

## 6. Fora de escopo do core

- framework web oficial monolítico;
- ORM embutido na linguagem;
- decorators mágicos de rota;
- HTML/template syntax no compilador;
- HTTP/2 ou HTTP/3 antes do HTTP/1.1 incremental estar sólido.
