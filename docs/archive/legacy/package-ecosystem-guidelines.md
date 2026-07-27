# Ori Package Ecosystem Guidelines

> Status: **archived**  
> Original path: `docs/planning/package-ecosystem-guidelines.md`  
> Original date: before 2026-07-27  
> Archived on: 2026-07-27  
> Category: `legacy`  
> Current replacement: `docs/security/supply-chain.md and docs/spec/17-project-and-docs.md`  
> Warning: versions, syntax, commands, paths, and priorities below may be obsolete.


Este documento define as convenções oficiais para o ecossistema de pacotes da linguagem Ori, especialmente para pacotes que provêm *bindings* C via FFI (`extern c`).

## 1. Nomenclatura

### 1.1 Reposit�rio no GitHub
Todos os pacotes criados para a linguagem Ori devem usar o prefixo `ori-` em seus reposit�rios no GitHub para facilitar o _discoverability_ e indicar claramente sua finalidade.
- **Formato:** `ori-<nome-da-lib>`
- **Exemplo (futuro, comunidade):** `ori-raylib`, `ori-sqlite` — **não** são prioridade do core enquanto a linguagem não estiver sólida.

### 1.2 Nome do Pacote (`ori.pkg.toml`)
Internamente, o manifesto do pacote (`ori.pkg.toml`) n�o deve conter o prefixo `ori-`. O nome deve ser limpo, refletindo o namespace pelo qual a biblioteca ser� importada no c�digo fonte.
- **Formato:** `name = "<nome-da-lib>"`
- **Exemplo:** `name = "imgui"`
- **Uso no c�digo:** `import imgui.ui`

## 2. Estrutura de Diret�rios Recomendada

Pacotes que lidam com bibliotecas nativas devem adotar a seguinte estrutura can�nica:

```
ori-<pacote>/
+-- ori.pkg.toml           # Manifesto do pacote
+-- README.md              # Instru��es claras de instala��o e uso
+-- src/                   # C�digo fonte Ori (.orl)
�   +-- ffi.orl            # Declara��es puras de `extern c` e structs opacas
�   +-- ui.orl             # Wrappers idiom�ticos e abstractions do Ori
+-- lib/                   # Artefatos Nativos Pr�-compilados (C/C++)
�   +-- win-x64/           # .dll e .lib para Windows
�   +-- linux-x64/         # .so e .a para Linux
�   +-- macos-arm64/       # .dylib e .a para macOS (Apple Silicon)
+-- tools/                 # Scripts auxiliares (ex: build do C/C++ do zero)
�   +-- build_native.ps1
+-- examples/              # C�digos demonstrando como usar a biblioteca
    +-- demo.orl
```

## 3. Diretrizes de Bibliotecas Nativas (FFI)

Como o Ori v0.2.1 ainda n�o possui um sistema automatizado de *build scripts* nativos (como o `build.rs` do Rust), a responsabilidade de prover os artefatos nativos recai sobre o autor do pacote.

### Regras para o diret�rio `lib/`:
1. **Artefatos Prontos:** O reposit�rio deve conter as bibliotecas `.dll`, `.so` e `.dylib` pr�-compiladas na pasta `lib/<target>/`. Isso garante que quando um usu�rio baixar o pacote e der `ori run`, a execu��o JIT encontre a biblioteca compartilhada imediatamente.
2. **Bibliotecas Est�ticas:** Recomenda-se tamb�m fornecer as bibliotecas est�ticas (`.lib` ou `.a`) nas mesmas pastas. Isso permitir� que comandos futuros como `ori compile` liguem (link) a depend�ncia diretamente no execut�vel final (AOT).
3. **Scripts de Build (Opcional, mas Recomendado):** Para transpar�ncia e atualiza��es futuras, crie um script na pasta `tools/` que baixe o c�digo fonte C/C++ original via `git clone`, rode o `cmake` ou similar, e mova os `.dll`/`.so`/`.dylib` resultantes para a pasta `lib/` apropriada.
