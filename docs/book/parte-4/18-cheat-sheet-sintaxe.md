# Cap. 18 — Cheat sheet de sintaxe S3

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV  
> Tabela compacta também em [Apêndice A](../apendices/A-cheat-sheet.md).

## TL;DR

Uma página para consultar formas canônicas e rejeições pré-S3. Em dúvida,
`ori check` e o catálogo de erros (Cap. 21).

## Forma válida (resumo)

Ver [Apêndice A](../apendices/A-cheat-sheet.md) — mesma tabela.

## Programa mínimo

```ori
module app.main

import ori.io = io

main()
    io.println("ok")
end
```

## Migração

```bash
ori migrate-syntax caminho/
```

## Ir mais fundo

- Overview: [`../../spec/01-overview.md`](../../spec/01-overview.md)
- Catálogo: [`../../spec/13-error-catalog.md`](../../spec/13-error-catalog.md)
