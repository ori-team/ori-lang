# Como reportar bugs

> Status: política prática Ori **S3 / 0.3.2**  
> **English:** [report-bugs.md](report-bugs.md)

Um bom report permite reproduzir o problema com poucos comandos.

## Linguagem / type checker

Inclua: `ori --version`, SO, arquivo `.orl` mínimo, comando
(`ori check main.orl`), saída completa do diagnóstico.

## Stdlib / runtime

Inclua módulo (`ori.fs`, …), se falha em `ori run` e/ou `ori compile`, e
`ORI_TEST_LEAK_CHECK=1` quando for memória.

## Tooling

`ori fmt`, `ori doc`, LSP, VS Code / Zed, package de release. Comando exato e
projeto mínimo.
## Formato sugerido

```text
Título: descrição curta

Ambiente:
- Ori:
- OS:
- Comando:

Reprodução:
1. ...

Esperado:

Obtido:

Arquivo mínimo:
module app.main

main()
end
```

Não envie projetos grandes no primeiro relato.
