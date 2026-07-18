# Apêndice B — Mapa do repositório

> Uma página. Detalhe narrativo: [Cap. 4](../parte-1/04-mapa-ecossistema.md).

```text
ori-lang/
├── compiler/crates/     ori-lexer, ori-parser, ori-ast, ori-hir, ori-types,
│                        ori-codegen, ori-runtime, ori-diagnostics,
│                        ori-driver, ori-lsp
├── stdlib/              módulos ori.* (.orl)
├── runtime/<triple>/    staticlib + cdylib empacotados
├── docs/
│   ├── spec/            contrato normativo (EN)
│   ├── language/        tour
│   ├── guides/          how-tos
│   ├── planning/        mantenedores
│   └── book/            este livro (PT)
├── examples/            programas .orl
├── tests/               E2E Ori
├── extensions/          vscode-orl, zed-ori
├── tools/               stage, smoke, QA, migrate
├── branding/            logo
└── CHANGELOG.md
```
