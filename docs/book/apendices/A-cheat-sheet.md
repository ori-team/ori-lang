# Apêndice A — Cheat sheet de sintaxe S3

> **Versão âncora:** Ori 0.3.x  
> Espelho expandido: [Cap. 18](../parte-4/18-cheat-sheet-sintaxe.md)

## Forma válida

| Tema | Forma |
|------|--------|
| Módulo | `module app.main` |
| Função | `name(x: int) -> int` · `main()` — **sem** `func` |
| Bloco | corpo + `end` |
| Tipos | `list[T]`, `optional[T]`, `result[T, E]` com `[]` |
| Result | `ok(v)` / `err(e)` · `case ok(x):` / `case err(m):` |
| Optional | `some(v)` / `none` |
| Propagar | `try expr` apenas |
| Import alias | `import ori.io = io` (path à esquerda) |
| Import seletivo | `import ori.fs (read_text)` |
| If | `if` / `elif` / `else` (não `else if`) |
| If-expressão | `if cond then a else b` |
| Struct literal | `Point { x: 1, y: 2 }` |
| Traits | `apply Type` + `use Trait` |
| Pipe | `value \|> f` → `f(value)` |
| Cleanup | `using … end` |
| Comentário | `-- …` |

## Rejeitado (pré-S3 → erro duro)

| Evite | Use |
|-------|-----|
| `func` / `namespace` | `name()` / `module` |
| `else if` | `elif` |
| `?` | `try` |
| `import as` / ordem Auk9 invertida | `import path = alias` |
| `<>` em tipos | `[]` |
| `implement` / `apply Trait to` | `apply Type` + `use Trait` |
| `success` / `error` | `ok` / `err` |
| `do` em closures | `(u) => …` |

Migração: `ori migrate-syntax`.

## CLI mínima

```bash
ori check path.orl
ori compile path.orl
ori run path.orl
ori test
```
