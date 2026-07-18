# Cap. 8 — Ferramentas do dia a dia

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

Desenvolver a Ori: Rust/Cargo, Cranelift, stage do runtime, testes do
`ori-driver`, catálogo de diagnostics, LSP e scripts de smoke/package.
Usar a Ori: CLI `ori` + editor com extensão local.

## Exemplo

Ciclo mínimo no compilador:

```bash
cd compiler
cargo check --workspace
cargo test -p ori-driver --test ori_spec
cargo build -p ori-runtime --lib
# re-estagiar staticlib + cdylib em runtime/<triple>/ se a FFI mudou
```

## Como funciona

### Stack de implementação

| Ferramenta | Uso |
|------------|-----|
| Rust + Cargo | Compilador e `ori-runtime` |
| Cranelift | Codegen nativo / JIT |
| logos | Lexer |
| tower-lsp | `ori-lsp` |
| Scripts `tools/stage_native_runtime.*` | Copiar runtime para `runtime/` |
| `tools/smoke_*` | Validar package isolado |

### CLI do usuário

`ori check` · `ori compile` · `ori run` · `ori test` · `ori migrate-syntax` ·
(e utilitários documentados no README / install).

### Variáveis úteis (resumo)

| Variável | Papel |
|----------|--------|
| `ORI_USE_JIT=1` / `ORI_USE_AOT=1` | Forçar caminho de `ori run` |
| `ORI_RUNTIME_LIB` / `ORI_RUNTIME_CDYLIB` | Override de artefatos |
| `ORI_USE_SYSTEM_LINKER=1` | Linker do SO |
| `ORI_STDLIB_ROOT` | Onde achar `stdlib/` |
| `UPDATE_EXPECT=1` | Atualizar snapshots de diagnostic em testes |

Lista completa: [`../../../AGENTS.md`](../../../AGENTS.md).

### Editor

Extensões locais em `extensions/vscode-orl/` e `extensions/zed-ori/` —
LanguageClient → `ori-lsp`. Sem prioridade de publish em marketplace no foco atual.

## O que memorizar

- Mudou runtime FFI → re-estagiar **staticlib e cdylib**.
- Testes de linguagem: `ori-driver` + catálogo.
- Usuário final não precisa de Cargo.

## Ir mais fundo

- Bootstrapping: [`../../guides/bootstrapping.md`](../../guides/bootstrapping.md)
- Cap. 19 — CLI e env (consulta)
