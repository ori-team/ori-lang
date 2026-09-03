# Implementação concluída — compilação condicional estruturada `@cfg`

> **Status:** `LANG-CFG-1.0…1.5` concluído em 2026-08-10.  
> **Decisão:** [ADR de `@cfg`](adr-conditional-compilation-cfg.md).  
> **Contrato normativo:** [Spec 02](../spec/02-lexical.md#conditional-compilation),
> [Spec 03](../spec/03-grammar.ebnf) e [Spec 17](../spec/17-project-and-docs.md).

## Resultado entregue

`@cfg` deixou de ser uma anotação inerte. O compilador sempre parseia o arquivo
completo e, em seguida, remove declarações inativas antes da resolução de nomes
e da checagem de tipos. O mesmo AST filtrado alimenta documentação, HIR, AOT,
JIT e backend C/debug.

```ori
@cfg(target_os: linux)
linux_api()
end

@cfg(all(target_family: unix, feature: tls))
secure_api()
end

@cfg(not(execution_profile: embedded))
standalone_api()
end
```

## Contrato fechado

| Aspecto | Decisão implementada |
|---|---|
| Forma | Um predicado estruturado; strings livres são erro |
| Chaves | `target_os`, `target_arch`, `target_family`, `execution_profile`, `feature` |
| Composição | `all`/`any` com um ou mais filhos; `not` com exatamente um |
| Escopo v1 | Declarações top-level; imports, campos e statements ficam fora |
| Momento | Parse completo → validação/avaliação cfg → resolução → checker → HIR |
| Erro em cfg | Emite diagnóstico e mantém o item ativo para não esconder código por engano |
| Features | Declaradas em `[features]`; `default` é uma lista e cada feature usa `[]` |
| Seleção | `--features`, `--no-default-features`, `--execution-profile`, `--target` |
| Cache | Target, perfil, features ordenadas e defaults participam do fingerprint |
| Segurança | Não é sandbox, capability nem fronteira de autorização |

Features são um namespace único do projeto raiz nesta primeira versão. A Ori
não avalia variáveis de ambiente arbitrárias dentro do código e não oferece
predicados de valor livre.

## Interfaces implementadas

- `ori_ast::common::CfgPredicate`: árvore tipada de comparação e composição;
- `ori_types::conditional::CfgContext`: fatos normalizados da compilação;
- `ori_types::conditional::filter_source_file`: filtro puro antes do resolver;
- manifests `ori.proj` e `ori.pkg.toml`: declaração/defaults de features;
- driver: seleção por CLI e ambiente (`ORI_TARGET_TRIPLE`,
  `ORI_EXECUTION_PROFILE`, `ORI_FEATURES`, `ORI_NO_DEFAULT_FEATURES`);
- LSP: índices sintáticos aplicam o mesmo filtro do pipeline semântico;
- formatter: preserva predicados estruturados sem reescrevê-los;
- cache incremental: contexto cfg incluído nos fingerprints de projeto e módulo.

## Limites atuais

- `--target` seleciona fatos cfg e artefatos de runtime. O backend nativo ainda
  é host-first; a flag não promete cross-compilation completa.
- Valores canônicos de target que não correspondem aos fatos do triple avaliam
  como falso. Chaves, valores, perfis, features e triples não representáveis
  pelo conjunto fechado de cfg v1 são erros; um OS desconhecido nunca vira
  `target_os: none`.
- Não há dependências ou implicações entre features em cfg v1; por isso o valor
  de uma feature declarada precisa ser `[]`.
- JIT ainda não possui cache persistente; quando existir, deverá usar o mesmo
  fingerprint cfg.

## Evidência e gates

- regressões no driver cobrem filtro antes do resolver/checker, composição,
  funções, structs, enums, consts, extern, formas inválidas,
  manifesto/defaults, paridade C/docs, AOT e formatter;
- catálogo registra todos os diagnósticos `cfg.*`;
- `docs_coverage.sh`, `docs_examples.sh` e `daily_fast.sh` são os gates de
  fechamento;
- matriz multi-OS valida os fatos de target nos workflows existentes.

## Fora de escopo

- macros, geração de AST ou funções em compile time;
- leitura de ambiente arbitrário por predicados;
- capabilities e isolamento de host;
- sintaxe condicional dentro de expressões;
- cross-compilation nativa completa.
