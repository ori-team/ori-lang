# Instalação de Ori

> **Público-alvo:** usuários finais que querem instalar Ori para desenvolver programas, sem clonar o repositório ou ter a toolchain Rust instalada.

## Requisitos do sistema

O Ori Language usa o **linker nativo do sistema operacional** para compilação AOT (`ori compile`, `ori test`). Para execução JIT (`ori run`), nenhum linker é necessário — apenas o runtime empacotado.

### Windows (10/11)

**Pré-requisito:** Visual Studio Build Tools ou Visual Studio Community com a workload **"Desktop development with C++"**.

**Como instalar:**

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

Ou baixe o installer em [visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/) e selecione a workload **"Desktop development with C++"**.

**Por que é necessário:** Ori usa `link.exe` (o linker do MSVC) para compilar binários nativos. O Build Tools é gratuito e não requer IDE completa.

**NÃO é necessário:**
- Rust (`rustc`, `cargo`)
- `rust-lld` (Ori empacota seu próprio runtime e preferencialmente usa o linker do sistema)

---

### Linux (Debian, Ubuntu, Fedora, Arch, etc.)

**Pré-requisito:** `build-essential` (ou equivalente: `gcc`, `ld`, `libc-dev`).

**Debian / Ubuntu:**

```bash
sudo apt update
sudo apt install build-essential
```

**Fedora / RHEL:**

```bash
sudo dnf install gcc gcc-c++ make glibc-devel
```

**Arch:**

```bash
sudo pacman -S base-devel
```

**Por que é necessário:** Ori usa `ld` (linker GNU) para compilar binários nativos.

**NÃO é necessário:**
- Rust (`rustc`, `cargo`)
- `rust-lld` (Ori preferencialmente usa o linker do sistema)

---

### macOS (Intel e Apple Silicon)

**Pré-requisito:** Xcode Command Line Tools.

```bash
xcode-select --install
```

**Por que é necessário:** Ori usa `ld` (linker do Xcode) para compilar binários nativos.

**NÃO é necessário:**
- Rust (`rustc`, `cargo`)
- Xcode completo (apenas Command Line Tools)
- `rust-lld`

---

## Download e instalação

### Ubuntu / Debian — pacote `.deb` (recomendado neste SO)

1. Instale o pré-requisito de AOT (linker do sistema):

```bash
sudo apt update
sudo apt install build-essential
```

2. Instale o `.deb` (arquivo gerado localmente ou baixado):

```bash
sudo apt install ./ori_0.2.0_amd64.deb
# se o apt recusar o caminho local:
sudo dpkg -i ./ori_0.2.0_amd64.deb
sudo apt-get install -f
```

3. Verifique:

```bash
ori --version
ori doctor
```

**Layout instalado:**

| Caminho | Conteúdo |
|---------|----------|
| `/usr/bin/ori` | CLI (symlink) |
| `/usr/bin/ori-lsp` | Language server (symlink) |
| `/usr/lib/ori/` | Binários reais + `stdlib/` + `runtime/` |

**Gerar o `.deb` a partir do repositório (mantenedores):**

```bash
# requer Rust toolchain + build-essential
./tools/package_deb.sh --force
# artefato: target/dist/ori_<versão>_amd64.deb
```

---

### Via release package (`.tar.gz` / `.zip`)

1. Acesse a página de releases do projeto no GitHub.
2. Baixe o arquivo correspondente ao seu sistema operacional:
   - Windows: `ori-x86_64-pc-windows-msvc.zip`
   - Linux: `ori-x86_64-unknown-linux-gnu.tar.gz`
   - macOS Intel: `ori-x86_64-apple-darwin.tar.gz`
   - macOS Apple Silicon: `ori-aarch64-apple-darwin.tar.gz`

3. Extraia o conteúdo em um diretório de sua preferência (ex: `C:\Tools\ori`, `~/ori`, `/usr/local/ori`).

4. Adicione o diretório ao `PATH`:

   **Windows (PowerShell):**
   ```powershell
   [Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\Tools\ori", "User")
   ```

   **Linux / macOS (bash/zsh):**
   ```bash
   echo 'export PATH="$HOME/ori:$PATH"' >> ~/.bashrc
   source ~/.bashrc
   ```

5. Verifique a instalação:
   ```bash
   ori --version
   ori doctor
   ```

---

### Verificação com `ori doctor`

O comando `ori doctor` verifica se o ambiente está saudável:

```bash
ori doctor
```

Saída esperada em uma instalação correta:
- ✅ stdlib root encontrado
- ✅ runtime estática (AOT) encontrada
- ✅ runtime cdylib (JIT) encontrada
- ✅ target triple detectado
- ✅ linker strategy: SystemLinker (ou BundledRustLld fallback)
- ✅ modo `ori run`: JIT disponível

Se `ori doctor` reportar problemas, consulte a seção **Troubleshooting** abaixo.

---

## Primeiro programa

Crie um arquivo `hello.orl`:

```lua
import ori.io as io

func main()
    io.println("Hello, Ori!")
end
```

Execute com JIT (não precisa de linker):

```bash
ori run hello.orl
```

Compile para um binário nativo (requer linker do sistema):

```bash
ori compile hello.orl --out hello
./hello
```

---

## Variáveis de ambiente para override

Em situações especiais, você pode forçar um comportamento específico:

| Variável | Propósito |
|----------|-----------|
| `ORI_USE_SYSTEM_LINKER=1` | Força o uso do linker nativo do sistema (default desde 2026-07-02) |
| `ORI_SYSTEM_LINKER` | Override explícito do caminho do linker (`link.exe`, `ld`, etc.) |
| `ORI_USE_BUNDLED_RUST_LLD=1` | Força o uso do `rust-lld` empacotado (fallback) |
| `ORI_RUST_LLD` | Override explícito do caminho do `rust-lld` |
| `ORI_USE_RUSTC_DRIVER=1` | Volta ao driver `rustc` legacy (não recomendado para usuários finais) |
| `ORI_USE_JIT=1` | Força JIT para `ori run` |
| `ORI_USE_AOT=1` | Força AOT para `ori run` (desabilita JIT) |
| `ORI_RUNTIME_CDYLIB` | Override do caminho da cdylib para JIT |
| `ORI_STDLIB_ROOT` | Override do caminho dos módulos `.orl` da stdlib |

**Normalmente nenhuma variável precisa ser setada.** A instalação default detecta tudo automaticamente.

---

## Troubleshooting

### "linker not found" ou "native.link_failed"

**Causa:** O linker do sistema não foi encontrado.

**Windows:**
- Instale Visual Studio Build Tools com workload "Desktop development with C++"
- Verifique: `where link.exe` deve retornar um caminho válido

**Linux:**
- Instale `build-essential`
- Verifique: `ld --version` deve funcionar

**macOS:**
- Instale Xcode Command Line Tools: `xcode-select --install`
- Verifique: `xcrun --find ld` deve retornar um caminho válido

### "runtime not found" ou `ORI_REQUIRE_PACKAGED_RUNTIME=1` falha

**Causa:** O runtime empacotado não foi encontrado no diretório esperado.

**Solução:**
- Certifique-se de que o diretório `runtime/` está presente ao lado do executável `ori`
- Para uso fora do release package, defina `ORI_RUNTIME_CDYLIB` ou `ORI_RUNTIME_LIB`

### `ori run` funciona, mas `ori compile` falha

**Causa:** JIT funciona sem linker, mas AOT precisa do linker do sistema.

**Solução:** Instale os pré-requisitos do sistema conforme a seção acima para seu OS.

### `ori-lsp` não inicializa no VS Code

**Causa:** O caminho para `ori-lsp` não está configurado ou o binário não foi encontrado.

**Solução:**
- Verifique que `ori-lsp` está no `PATH`
- Na extensão VS Code, configure `ori.lsp.path` se necessário
- Certifique-se de que `ori.compiler.path` aponta para o `ori` correto

---

## Desinstalação

O Ori é um pacote portátil. Para desinstalar, simplesmente remova o diretório de instalação e remova a entrada do `PATH`.

---

## Veja também

- `AGENTS.md` — Seção "Rust Independence Strategy" (estratégia completa de independência do Rust)
- `docs/planning/rust-independence.md` — Documento técnico sobre a estratégia de independência
- `docs/planning/uso-real-pequeno-medio.md` — Seção "Decisões futuras sobre 1.0"
