# Ori — extensão para VS Code / Cursor

> Suporte de linguagem para Ori (`.orl` e `.oridoc`). A versão em inglês é a
> referência completa de instalação e desenvolvimento: [README.md](README.md).

## Debugger

A extensão registra o tipo de debug `ori` e inicia automaticamente
`ori debug --dap`:

1. Abra um arquivo `.orl`.
2. Execute **Ori: Debug Current File** ou abra **Run and Debug**.
3. Coloque breakpoints normalmente na margem do editor.

A Debug View mostra breakpoints, continue/step, pilha síncrona e async,
variáveis escalares, caminhos estruturados de `struct`/`optional`/`result`/enum
e coleções, metadados e elementos indexados de listas, capturas de closures e
prévias limitadas de strings/bytes gerenciados (bytes aparecem em hexadecimal).
Buffers estáticos ou estrangeiros exigem um comprimento exato registrado.
`evaluate` aceita aritmética escalar, comparações, lógica booleana e strings do
último snapshot parado, sem executar código no alvo.

Se o compilador não estiver no `PATH`, configure `ori.compiler.path`. A
extensão procura também `compiler/target/{debug,release}/` no workspace.
