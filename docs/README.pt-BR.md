# Documentação da Ori

> Versão atual do projeto: **0.3.8**  
> Superfície da linguagem: **S3**  
> ABI nativa: **`ori-native-abi-1`**

O mapa canônico da documentação é o [`ATLAS.md`](ATLAS.md). Ele conecta documentos, código, testes, decisões e procedimentos operacionais.

Índice principal em inglês: [README.md](README.md).

## Por público

| Público | Comece aqui |
|---|---|
| Novo usuário | [Instalação](install.pt-BR.md) → [Tour](language/tour.pt-BR.md) → [Primeiro projeto](guides/first-project.pt-BR.md) |
| Uso cotidiano | [Guias](guides/README.md) · [Referência da CLI](guides/cli-reference.pt-BR.md) · [Exemplos](../examples/) |
| Implementação da linguagem | [Especificação](spec/README.md) · [Pipeline](architecture/compiler-pipeline.md) |
| Contribuição | [Início do projeto](../PROJECT_START.md) · [Regras operacionais](../AGENTS.md) |
| Runtime | [Runtime e memória](architecture/runtime-and-memory.md) · [Política de código unsafe](security/unsafe-code-policy.md) |
| Manutenção | [Estado atual](product/status.md) · [Versionamento](product/versioning.md) · [Operações](operations/README.md) |
| Agentes de IA | [ATLAS](ATLAS.md) · [Catálogo](catalog.yaml) · [`.ai/`](../.ai/README.md) |

## Domínios

- `product/` — identidade, estado atual, versionamento e acessibilidade;
- `architecture/` — sistema atual e invariantes;
- `spec/` — contratos normativos da linguagem, runtime, projetos e ABI;
- `implementation/` — padrões e caminhos seguros de extensão;
- `quality/` — testes, conformidade, diagnósticos e performance;
- `security/` — threat model, FFI e `unsafe`;
- `governance/` — evolução da linguagem e processo de RFC;
- `decisions/` — ADRs;
- `rfcs/` — propostas públicas relevantes;
- `plans/` — planos complexos ativos;
- `operations/` — desenvolvimento e releases;
- `language/` e `guides/` — documentação de uso;
- `book/` — livro em português;
- `archive/` — evidência histórica categorizada, não instrução atual.

A migração histórica foi concluída:

- `docs/planning/BACKLOG.md` é a lista canônica de trabalho aberto;
- decisões aceitas estão em `docs/decisions/adr/`;
- planos concluídos, auditorias, investigações e sessões estão categorizados em `docs/archive/`;
- o antigo diretório `docs/planning/historico/` foi removido;
- o mapa completo dos movimentos está em [`archive/MIGRATION_REPORT.md`](archive/MIGRATION_REPORT.md).

## Regras canônicas

1. Cada assunto possui uma única fonte atual.
2. Outros arquivos devem apontar para ela, sem repetir a explicação completa.
3. Arquitetura descreve o sistema implementado hoje.
4. Especificação descreve o comportamento aceito hoje.
5. ADR registra decisões duráveis.
6. RFC descreve propostas em avaliação.
7. ExecPlan descreve uma implementação complexa já aceita.
8. Arquivos históricos podem conter versões, comandos e sintaxe obsoletos.
9. Documentos ativos usam a versão atual `0.3.8`.
10. Novos documentos canônicos atualizam o ATLAS e `catalog.yaml`.

## Política de idiomas

- Inglês é a fonte canônica para a superfície principal do GitHub.
- Documentação de usuário possui versão portuguesa paralela quando já existe um arquivo irmão `*.pt-BR.md`.
- A especificação normativa permanece apenas em inglês.
- O livro permanece em português.
- Código e comentários de código usam inglês.

Uma mudança visível ao usuário deve atualizar as versões EN/PT mantidas em paralelo.

## Sintaxe atual

Exemplos ativos usam a sintaxe S3 atual:

- `module app.nome`;
- funções sem a palavra de declaração `func`;
- visibilidade `public`;
- tipos com `[]`;
- `import caminho = alias`;
- `ok`, `err` e `try`;
- `apply Type` e `use Trait`;
- blocos encerrados por `end`.

Documentos que preservam sintaxe removida devem estar claramente arquivados.

## Qualidade documental

Mudanças na documentação devem validar:

- links;
- fonte canônica;
- versão ativa;
- exemplos executáveis;
- paridade EN/PT onde mantida;
- catálogo e metadados;
- classificação de arquivos históricos;
- ausência de identidade antiga nos documentos ativos.
