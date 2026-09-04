# Ori — extensão para VS Code / Cursor

A extensão é versionada separadamente do workspace do compilador (`0.3.8-dev`,
última release `v0.3.8`). A superfície atual da linguagem é S3/0.4.

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

Para manter diagnósticos, completion e execução na mesma configuração de
`@cfg`, use `ori.cfg.target`, `ori.cfg.executionProfile`, `ori.cfg.features` e
`ori.cfg.noDefaultFeatures`. A extensão repassa esses valores ao compilador e
ao LSP pelos mesmos contratos `ORI_*` usados pela CLI.
Alterar qualquer configuração `ori.cfg.*` reinicia automaticamente o servidor
de linguagem, mantendo diagnósticos, completion, terminais e debug no mesmo
programa ativo.
