# Depuração de programas Ori

> **English:** [debugging.md](debugging.md)

O Ori oferece um debugger nativo cooperativo e um servidor DAP. Ambos usam os
snapshots gerados pelo compilador.

## Terminal

```text
ori debug examples/cli_args/main.orl --breakpoint 41
```

Use `c` para continuar, `s` para avançar uma linha instrumentada e `q` para
encerrar o alvo. O adapter mostra localização, stack e variáveis locais.

## DAP

```text
ori debug --dap
```

O adapter atende inicialização, breakpoints, continue/step, threads, stack,
scopes, variables, evaluate e disconnect. Campos de structs, payloads de
optional/result/enums, collections suportadas, frames async e captures de
closures aparecem no catálogo. `evaluate` não executa código do alvo.

O build nativo também grava `program.debug.json`. A extensão VS Code inicia o
DAP automaticamente; o Zed atual oferece LSP, mas não wiring automático do
debugger. Use `ori doctor` para problemas de runtime ou linker.
