# Como reportar bugs

> Status: política prática para Ori **0.3.8 / S3**  
> **English:** [report-bugs.md](report-bugs.md)  
> Vulnerabilidades de segurança: siga [`../../SECURITY.md`](../../SECURITY.md), não abra uma issue pública

Um bom relato permite reproduzir o problema com poucos comandos.

## Linguagem e type checker

Inclua:

- `ori --version`;
- sistema operacional e arquitetura;
- arquivo `.orl` mínimo;
- comando exato, como `ori check main.orl`;
- saída completa dos diagnósticos;
- se o problema também aparece em `ori run`, `ori compile` ou no editor.

Use esta categoria para lexer, parser, resolução de nomes, type checker, imports, generics, traits, matching, `try`, formatter e diagnósticos da linguagem.

## Stdlib e runtime

Inclua também:

- módulo ou operação (`ori.fs`, `ori.json` e similares);
- se falha no JIT de `ori run`, no AOT de `ori compile` ou nos dois;
- target triple, quando conhecido;
- informações sobre staging de staticlib/cdylib ao desenvolver pelo repositório;
- para memória, dados relevantes como `ORI_TEST_LEAK_CHECK=1`;
- se envolve cleanup, aliasing, concorrência, I/O ou comportamento específico da plataforma.

## Ferramentas, projetos e pacotes

Inclui `ori fmt`, `ori doc`, `ori new`, REPL, LSP, integrações VS Code/Zed, manifests, lockfiles, dependências, instaladores, updater e pacotes de release.

Inclua:

- comando ou ação exata no editor;
- layout mínimo do projeto;
- manifest/lockfile relevante, sem segredos;
- se falha fora do checkout do repositório;
- logs do language server, quando aplicável;
- fonte e revisão/versão resolvida do pacote, quando houver dependências.

Remova tokens, credenciais, caminhos privados e dados pessoais sem relação com o defeito.

## Relatos de performance

Inclua:

- workload e entrada;
- modo debug/release;
- rota AOT/JIT e configurações de otimização;
- target, SO, CPU e memória;
- quantidade de amostras e estatística;
- versão/commit de comparação;
- script de reprodução;
- evidência de que o benchmark ainda executa o trabalho pretendido.

Veja [`../quality/performance-policy.md`](../quality/performance-policy.md).

## Formato sugerido

```text
Título: descrição curta

Ambiente:
- Versão/commit da Ori:
- SO e arquitetura:
- Target triple:
- Rota: check / AOT / JIT / LSP / pacote
- Variáveis de ambiente relevantes:

Reprodução:
1. ...
2. ...

Esperado:

Obtido:

Diagnósticos/saída:

Arquivo ou projeto mínimo:
module app.main

main()
end

Regressão:
- Última versão/commit que funcionava, se conhecido:
```

Comece com o menor arquivo ou projeto que preserve o problema. Vincule evidências maiores apenas quando o caso reduzido não conseguir reproduzi-lo.