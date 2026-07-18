# Cap. 10 — Instalar e verificar o ambiente

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR

Usuário final instala o **package** de release (sem Rust). AOT precisa do linker
do SO; `ori run` (JIT) só precisa do runtime empacotado. Confirme com
`ori doctor` e um hello.

## Exemplo

Após instalar e colocar `ori` no `PATH`:

```bash
ori --version
ori doctor
ori run examples/hello/main.orl
```

Ou, no diretório do exemplo:

```bash
cd examples/hello
ori run main.orl
```

`ori doctor` reporta saúde de stdlib, runtime, linker, target e JIT.

## Como funciona

### Pré-requisitos por SO (AOT)

| SO | Precisa | Não precisa |
|----|---------|-------------|
| Windows | VS Build Tools (C++) | Rust |
| Linux | `build-essential` (gcc + ld) | Rust |
| macOS | Xcode CLT | Rust |

JIT (`ori run`): linker **não** é necessário se o cdylib do runtime estiver no package.

### Caminhos de instalação

- Releases: [GitHub Releases](https://github.com/raillen/ori-lang/releases)
- Guia completo: [`../../install.pt-BR.md`](../../install.pt-BR.md)
- Windows (script): `tools/windows/get.ps1`
- Debian: `.deb` nas releases (`dpkg -i` + `build-essential` para AOT)

### Checklist rápido se algo falhar

1. `ori --version` responde? → `PATH`  
2. `ori doctor` reclama de runtime/stdlib? → package incompleto ou `ORI_STDLIB_ROOT`  
3. `ori run` ok e `ori compile` falha? → linker do SO (AOT)  
4. Só no clone do repo? → você precisa da toolchain Rust (Cap. 8 / bootstrapping)

### Desenvolvedor do compilador

Clone o repo, Rust via `rust-toolchain.toml`, `cd compiler && cargo build -p ori-driver`.  
Detalhe: [`../../guides/bootstrapping.md`](../../guides/bootstrapping.md).

## O que memorizar

- Package ≠ toolchain de desenvolvimento do compilador.
- `ori run` ≠ `ori compile` (JIT vs AOT).
- Primeiros sinais de vida: `ori doctor` + `ori run` no `examples/hello`.

## Ir mais fundo

- Install PT: [`../../install.pt-BR.md`](../../install.pt-BR.md)
- Primeiro projeto: [`../../guides/first-project.pt-BR.md`](../../guides/first-project.pt-BR.md)
- Exemplo: [`../../../examples/hello`](../../../examples/hello)
- Cap. 19 — CLI e env  
