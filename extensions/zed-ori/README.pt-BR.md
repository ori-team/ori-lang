# Ori — extensão para Zed

Suporte da linguagem **Ori** (`.orl`) no [Zed](https://zed.dev): configuração
da linguagem e LSP por meio do `ori-lsp` disponível no `PATH`.

O artefato atual é o **0.3.5**. Ele ainda não está na loja do Zed; instale o
zip da release como extensão de desenvolvimento ou selecione diretamente
`extensions/zed-ori` com **zed: install dev extension**.

## Pré-requisitos

```bash
cd compiler
cargo build -p ori-lsp -p ori-driver
export PATH="$PWD/target/debug:$PATH"
```

A extensão detecta `stdlib/` no monorepo e define `ORI_STDLIB_ROOT`. Para usar
compilação condicional estruturada, inicie o Zed com o mesmo ambiente da CLI:

```bash
export ORI_TARGET_TRIPLE=x86_64-unknown-linux-gnu
export ORI_EXECUTION_PROFILE=standalone
export ORI_FEATURES=tls,telemetry
# export ORI_NO_DEFAULT_FEATURES=1
zed .
```

A API atual de extensões do Zed não oferece um formulário próprio para essas
opções; o `ori-lsp` herda os valores do processo do editor.

## Limites

- diagnósticos, hover e completion funcionam quando `ori-lsp` está no `PATH`;
- o DAP existe via `ori debug --dap`, mas ainda não possui integração
  automática pela API da extensão;
- cores por tree-sitter e publicação na loja do Zed continuam pendentes.

English: [README.md](README.md).
