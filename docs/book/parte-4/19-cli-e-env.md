# Cap. 19 — CLI e variáveis de ambiente

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR

Comandos do dia: `check`, `compile`, `run`, `test`. `run` tende a JIT; `compile`
e `test` são AOT. Overrides via `ORI_*` quando precisar diagnosticar link/runtime.

## Comandos

| Comando | Papel |
|---------|--------|
| `ori check <path>` | Parse + tipos (rápido) |
| `ori compile <path>` | AOT → binário |
| `ori run <path>` | Executa (JIT se cdylib disponível) |
| `ori test` | Suite de testes do projeto |
| `ori doctor` | Saúde de stdlib, runtime, linker, target, JIT |
| `ori migrate-syntax <path>` | Reescreve várias formas pré-S3 |
| `ori new <name>` | Esqueleto de projeto |

## Variáveis (consulta prática)

| Variável | Quando usar |
|----------|-------------|
| `ORI_USE_AOT=1` | Forçar AOT em `ori run` |
| `ORI_USE_JIT=1` | Forçar JIT |
| `ORI_RUNTIME_LIB` | Staticlib fora do lugar padrão |
| `ORI_RUNTIME_CDYLIB` | Cdylib para JIT |
| `ORI_STDLIB_ROOT` | Stdlib fora do package |
| `ORI_USE_SYSTEM_LINKER=1` | Linker do SO |
| `ORI_USE_BUNDLED_RUST_LLD=1` | `rust-lld` empacotado/descoberto |

Lista completa para mantenedores: [`../../../AGENTS.md`](../../../AGENTS.md).

## O que memorizar

- Check antes de compile quando só quer tipos.
- JIT ≠ AOT — sintomas de “funciona no run e falha no compile” costumam ser linker/runtime.
- Usuário de package raramente precisa de `ORI_*`.

## Ir mais fundo

- Install: [`../../install.pt-BR.md`](../../install.pt-BR.md)
- Cap. 8 — ferramentas do compilador
