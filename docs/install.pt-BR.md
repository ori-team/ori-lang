# Instalação de Ori

> **Público-alvo:** usuários finais que querem desenvolver em Ori **sem** clonar
> o repositório e **sem** toolchain Rust.  
> **English:** [install.md](install.md)  
> **Superfície:** S3 + inference B · último release **v0.3.8** · M1 fechada

## Requisitos do sistema

Ori usa o `rust-lld` empacotado quando disponível e, caso contrário, descobre o
linker nativo do SO para AOT (`ori compile`, `ori test`). O pacote não exige
`rustc` nem `cargo`. Para JIT (`ori run`), nenhum linker é necessário — só o
runtime empacotado em `runtime/<triple>/` ao lado do binário `ori`.

### Windows (10/11)

**Pré-requisito para fallback:** Visual Studio Build Tools ou Community com a
workload **"Desktop development with C++"**. Packages de release normalmente
incluem `rust-lld`; instale isso somente se `ori doctor` indicar fallback para
o linker do sistema.

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

**Não é necessário:** Rust (`rustc`, `cargo`). O `rust-lld` empacotado é um
detalhe interno do pacote.

### Linux

**Pré-requisito para fallback:** `build-essential` (ou `gcc` + `ld` + headers
da libc). Packages de release normalmente incluem `rust-lld`; instale isso
somente se `ori doctor` indicar fallback para o linker do sistema.

```bash
# Debian / Ubuntu
sudo apt update && sudo apt install build-essential
```

### macOS

**Pré-requisito para fallback:** Xcode Command Line Tools
(`xcode-select --install`). Packages de release normalmente incluem `rust-lld`;
instale as ferramentas somente se `ori doctor` indicar fallback para o linker
do sistema.

---

## Download e instalação

> **Política de distribuição (2026-07-14):** packages oficiais de **release** para
> **Linux, Windows (MSVC) e macOS** (Apple Silicon + Intel) via GitHub Actions
> (`.github/workflows/release.yml`). Assets no tag `v*` em
> [GitHub Releases](https://github.com/raillen/ori-lang/releases).

1. Baixe em [GitHub Releases](https://github.com/raillen/ori-lang/releases)
   (para a tag **`vX.Y.Z`**):

   | Plataforma | Arquivo |
   |------------|---------|
   | Linux x86_64 | `ori-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
   | Linux deb | `ori_X.Y.Z_amd64.deb` |
   | Windows MSVC x86_64 | `ori-vX.Y.Z-x86_64-pc-windows-msvc.zip` |
   | macOS Apple Silicon | `ori-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
   | macOS Intel | `ori-vX.Y.Z-x86_64-apple-darwin.tar.gz` |

   Releases produzidas pelo workflow atual também trazem `SHA256SUMS`,
   `ori-vX.Y.Z.spdx.json` e
   atestações GitHub de proveniência do build. No diretório dos downloads:

   ```bash
   sha256sum --check SHA256SUMS
   gh attestation verify ori-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz --repo raillen/ori-lang
   ```

**Windows (recomendado — estilo Scoop):**

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser   # opcional, uma vez
irm https://raw.githubusercontent.com/raillen/ori-lang/main/tools/windows/get.ps1 | iex
```

Versão fixa / reinstalar: `$env:ORI_VERSION="0.3.8"; $env:ORI_FORCE="1"; irm …/get.ps1 | iex`.

Instala em `%LOCALAPPDATA%\Programs\Ori` e adiciona ao **PATH do usuário**.  
Sistema: `$env:ORI_SYSTEM="1"` (Administrador).  
Desinstalar: `pwsh -File "$env:LOCALAPPDATA\Programs\Ori\uninstall.ps1"`.  
Zip manual: extraia e rode `install.cmd`.  
Detalhes: [`tools/windows/README.md`](../tools/windows/README.md).

**Tarball / zip (manual):** extraia (ex. `~/ori` ou `C:\ori`), layout
`ori`/`ori.exe` + `ori-lsp` + `stdlib/` + `runtime/<triple>/`, coloque no `PATH`.

**Debian/Ubuntu:**

```bash
sudo dpkg -i ori_0.3.8_amd64.deb
# AOT fallback: sudo apt install build-essential
```

Verifique: `ori --version` e `ori doctor`.

Esperado: stdlib, runtime AOT + JIT, triple, linker (BundledRustLld ou SystemLinker), JIT para `ori run`.

---

## Primeiro programa

```ori
module app.hello

import ori.io = io

main()
    io.println("Hello, Ori!")
end
```

```bash
ori run hello.orl
ori new my_app && cd my_app && ori run main.orl
```

### Extensões de editor (VS Code / Zed)

Na mesma [Release](https://github.com/raillen/ori-lang/releases) da linguagem:

| Editor | Asset | Instalação |
|--------|--------|------------|
| VS Code / Cursor | `ori-vscode-orl-0.3.5.vsix` (release) | `code --install-extension <file>.vsix` |
| Zed | `ori-zed-0.3.5.zip` | extrair → **zed: install dev extension** |

Requer `ori-lsp` no `PATH`. Detalhes: [`extensions/README.md`](../extensions/README.md).

Próximo: [Tour da linguagem](language/tour.pt-BR.md) ·
[Primeiro projeto](guides/first-project.pt-BR.md) ·
[Exemplos](../examples/) · Editores: [VS Code](../extensions/vscode-orl/) ·
[Zed](../extensions/zed-ori/).

---

## Atualizando

Instalações via pacote (tar.gz / zip do Windows) se atualizam sozinhas:

```console
$ ori update --check   # só informa se há versão nova
$ ori update           # baixa, verifica (sha256) e troca no lugar
```

O `ori update` recusa instalações do gerenciador do sistema (use o novo
`.deb`) e builds de desenvolvimento (atualize via `git pull` + `cargo
build`). O checksum vem do manifest do release no GitHub; divergência
aborta antes de tocar em qualquer arquivo.

---

## Variáveis de ambiente (opcional)

Normalmente **nenhuma** é necessária.

| Variável | Propósito |
|----------|-----------|
| `ORI_USE_SYSTEM_LINKER=1` | Forçar linker do SO |
| `ORI_USE_JIT=1` / `ORI_USE_AOT=1` | Forçar modo de `ori run` |
| `ORI_STDLIB_ROOT` | Raiz da stdlib |
| `ORI_RUNTIME_LIB` / `ORI_RUNTIME_CDYLIB` | Runtime nativo |

---

## Troubleshooting

| Sintoma | Ação |
|---------|------|
| `native.link_failed` | Instale o linker do SO |
| Runtime not found | `runtime/` deve ficar ao lado de `ori` |
| Só `ori run` funciona | AOT precisa do linker; JIT não |
| LSP no VS Code / Zed | `ori-lsp` no PATH (ou settings `ori.*.path` no VS Code) |

## Veja também

- [spec/19-abi.md](spec/19-abi.md) — ABI `ori-native-abi-1`
- [AGENTS.md](../AGENTS.md) — independência do Rust (M1)
- [BACKLOG.md](planning/BACKLOG.md) — o que falta implementar
