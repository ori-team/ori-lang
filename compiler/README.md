# Ori compiler workspace

Rust workspace for the **Ori** language compiler (AOT + JIT).

## Develop

From this directory:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- check ../examples/hello/main.orl
```

From the repository root:

```bash
cargo --manifest-path compiler/Cargo.toml test --workspace
```

Language assets live **above** this folder: `../stdlib`, `../runtime`, `../docs`, `../examples`.

## Crates

| Crate | Role |
|-------|------|
| `ori-lexer` | Tokens |
| `ori-ast` | AST |
| `ori-parser` | Parse |
| `ori-types` | Resolve + check + stdlib manifest |
| `ori-hir` | HIR lower |
| `ori-codegen` | Cranelift native + C debug |
| `ori-runtime` | Native runtime (ARC, I/O, …) |
| `ori-diagnostics` | Codes + render |
| `ori-lsp` | Language server |
| `ori-driver` | CLI `ori` |

See repo root `AGENTS.md` and `docs/planning/repo-and-project-layout.md`.
