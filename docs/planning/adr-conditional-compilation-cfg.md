# ADR — Compilação condicional estruturada antes da resolução

> **Status:** aceito e implementado em 2026-08-10.  
> **Decisão de produto:** Ori permanece reading-first; `@cfg` seleciona código,
> mas não cria macros nem uma segunda linguagem de programação.

## Contexto

Ori aceitava `@cfg("texto")` e `@cfg(chave: valor)`, mas não alterava o
programa. Essa superfície inerte parecia oferecer portabilidade sem realmente
protegê-la. Embedding, automação, web, embarcados e bibliotecas nativas precisam
de pequenas variações de target sem duplicar módulos inteiros.

## Decisão

1. Aceitar somente predicados estruturados e fechados.
2. Fornecer cinco fatos: sistema, arquitetura, família, perfil de execução e
   feature declarada.
3. Compor condições somente com `all`, `any` e `not`.
4. Parsear o arquivo completo e filtrar declarações antes da resolução.
5. Manter predicados inválidos ativos enquanto o erro é reportado.
6. Usar uma única seleção para checker, docs, HIR, AOT, JIT, C e LSP.
7. Incluir toda seleção observável no cache incremental.
8. Limitar cfg v1 a declarações top-level.

## Por que antes da resolução

Uma declaração inativa não deve criar conflitos, satisfazer imports nem vazar
para a ABI. Filtrar no parser esconderia erros de sintaxe e removeria informação
necessária ao formatter. Filtrar apenas no backend deixaria resolver, checker e
LSP discordarem. O corte após parse e antes do resolver preserva os dois lados.

## Consequências

- ramos mutuamente exclusivos podem declarar o mesmo nome;
- sintaxe inválida continua falhando mesmo num ramo desligado;
- nomes e tipos inválidos dentro de um ramo desligado não são analisados;
- uma API pública inativa inexiste para consumidores e para `compile --lib`;
- alterar target, perfil ou features invalida saídas incrementais;
- `@cfg` não concede nem restringe capacidades de runtime.

## Alternativas rejeitadas

- **String livre:** fácil de parsear, impossível de validar e padronizar.
- **Expressão booleana geral:** amplia parser, evaluator e tooling sem ganho
  proporcional.
- **Filtragem no backend:** produz divergência entre diagnósticos e execução.
- **Módulo implícito por target:** esconde descoberta e multiplica arquivos.
