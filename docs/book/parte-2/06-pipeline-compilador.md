# Cap. 6 — Pipeline do compilador

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

Fonte `.orl` passa por lexer, parser, resolução de nomes, type checker, HIR e
codegen Cranelift. Depois: link AOT com `ori-runtime`, ou JIT em `ori run`.

## Exemplo

```text
Source (.orl)
  → Lexer          (ori-lexer)
  → Parser         (ori-parser) → AST
  → Resolver       (ori-hir)    → nomes / bindings
  → Type checker   (ori-types)
  → Codegen        (ori-codegen)
       ├─ Native: Cranelift → objeto → link runtime
       └─ C debug: transpile parcial
  → Binary  ou  JIT in-process
```

## Como funciona

| Crates (ideia) | Papel |
|----------------|--------|
| `ori-lexer` | Tokens |
| `ori-ast` / `ori-parser` | Árvore sintática |
| `ori-hir` | Resolução + HIR |
| `ori-types` | Tipos e diagnostics |
| `ori-codegen` | Cranelift / C |
| `ori-runtime` | ARC, I/O, FFI, async executor |
| `ori-diagnostics` | Códigos e render |
| `ori-driver` | CLI + testes de integração |
| `ori-lsp` | Language Server |

**AOT** (`compile` / `test`): precisa de linker (bundled `rust-lld` ou linker do SO).  
**JIT** (`run`): Cranelift in-process; símbolos `ori_*` da cdylib do runtime.

Há também backend C de debug com paridade parcial — útil para inspeção, não
como caminho principal de aprendizado.

## O que memorizar

- Ordem: lex → parse → resolve → types → codegen → link/JIT.
- Runtime é parte do produto (staticlib + cdylib).
- Diagnostics estáveis vivem no catálogo da spec 13.

## Ir mais fundo

- Crates: [`../../../compiler/`](../../../compiler/)
- Spec backend: [`../../spec/14-backend-support.md`](../../spec/14-backend-support.md)
- Cap. 8 — ferramentas e env vars
