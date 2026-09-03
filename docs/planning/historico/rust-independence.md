# Estratégia de Independência do Rust — Ori Language

> **Data:** 2026-07-02  
> **Status:** Ativo — Phase 1, 2 e 3 completas; Phase 4 (self-hosting) adiada indefinidamente  
> **Documentos relacionados:** `AGENTS.md` (seção "Rust Independence Strategy"), `docs/planning/uso-real-pequeno-medio.md` (seção "Decisões futuras sobre 1.0"), `CHANGELOG.md` `[Unreleased]`

---

## 1. Definição de "independência do Rust"

> **Um usuário final pode instalar Ori, compilar, rodar e testar programas Ori sem ter a toolchain Rust instalada.**

Isso não significa que o *compilador em si* deixa de ser escrito em Rust. A independência do compilador (self-hosting) é um objetivo de longo prazo, não um pré-requisito para utilidade.

### Dois níveis de independência

| Nível | Definição | Status | Para 1.0? |
|-------|-----------|--------|-----------|
| **Usuário final** | Instala via release package; não precisa de `rustc`/`cargo` | **M1 ✅** (smoke + CI + install) | **Sim** |
| **Self-hosting** | Compilador escrito em Ori | Não iniciado (M4) | **Não** |

---

## 2. O que já foi feito (Rust removal híbrido A→B→D)

### Phase 1 — BundledRustLld (COMPLETA)

- **O que faz:** Invoca `rust-lld` diretamente, sem usar `rustc` como driver de link
- **Status:** Implementada para 3 desktop OSes (Windows MSVC, Linux GNU, macOS)
- **CRT Discovery:** Própria, sem depender de `rustc`
  - Windows: `vswhere.exe` + Windows SDK layout
  - Linux: `cc -print-file-name` + `cc -print-search-dirs`
  - macOS: `xcrun --show-sdk-path` + `xcrun --show-sdk-version`
- **Bundle:** `tools/stage_native_runtime.ps1/.sh` copiam `rust-lld` para `runtime/bin/`
- **Gap:** Ainda precisa do binário `rust-lld` (proveniente da instalação Rust)

### Phase 2 — SystemLinker (COMPLETA)

- **O que faz:** Invoca o linker nativo do sistema diretamente (`link.exe`, `ld`, `ld64`), sem `rust-lld` nem `rustc`
- **Status:** Implementada para os 3 desktop OSes
- **Opt-in:** `ORI_USE_SYSTEM_LINKER=1`
- **Gap:** Requer que o usuário tenha o toolchain nativo do sistema instalado

**Mudança de 2026-07-02:** `NativeLinker::discover()` agora tenta `SystemLinker` **antes** de `BundledRustLld` no caminho default. Isso elimina a dependência de `rust-lld` para usuários finais que já possuem o linker do sistema.

### Phase 3 — JIT Cranelift (COMPLETA)

- **O que faz:** `ori run` executa código Cranelift diretamente em memória, sem escrever `.o`, sem linker, sem subprocesso
- **Como:** `JITModule` + `libloading` sobre cdylib do runtime
- **Default:** Sim, quando cdylib está disponível
- **Gap:** `ori compile` e `ori test` ainda usam AOT (necessitam linker)

### Phase 4 — Self-hosting (FUTURO)

- **O que seria:** Compilador Ori escrito em Ori
- **Status:** Não iniciado, adiado indefinidamente
- **Raciocínio:** Self-hosting é um *sinal* de maturidade, não um *pré-requisito* para utilidade. Python, Ruby, Lua nunca foram self-hosted.

---

## 3. Pré-requisitos do sistema por OS

### Para AOT (`ori compile` e `ori test`)

O default é `SystemLinker`. O usuário final precisa ter instalado:

| OS | Pré-requisito | Como instalar |
|----|---------------|---------------|
| **Windows** | Visual Studio Build Tools (ou VS Community) | `winget install Microsoft.VisualStudio.2022.BuildTools` com workload "Desktop development with C++" |
| **Linux** | `build-essential` (`gcc` + `ld`) | `sudo apt install build-essential` (Debian/Ubuntu) ou equivalente |
| **macOS** | Xcode Command Line Tools | `xcode-select --install` |

### Para JIT (`ori run`)

**Nenhum linker é necessário.** Apenas o cdylib do runtime (`ori_runtime.dll` / `.so` / `.dylib`) empacotado no release.

---

## 4. Decisões arquiteturais fechadas

1. **Self-hosting adiado** até o restante da linguagem estar funcional (última discussão, M4).  
   Não é pré-requisito para utilidade.

2. **Runtime Layer 1 permanece Rust.**  
   ARC, async executor, FFI, I/O e rede são hot paths que beneficiam da safety do Rust. A ABI C é o contrato público; a implementação interna pode mudar no futuro.

3. **SystemLinker é o default para AOT.**  
   A partir de 2026-07-02, `NativeLinker::discover()` prefere o linker do sistema. Isso elimina a dependência de `rust-lld` para usuários finais.

4. **Rust continua necessário apenas para desenvolver o compilador.**  
   Quem clona o repo e trabalha no código do compilador precisa de `cargo` + `rustc`. Quem instala via release package não precisa.

5. **Modelo de 3 camadas da stdlib é permanente.**  
   Layer 1 (Rust runtime, hot path), Layer 2 (safe wrappers `.orl`), Layer 3 (algoritmos puros `.orl`).

6. **Versionamento congelado em `0.2.x`.**  
   `0.3.0` só quando houver breaking change real. `1.0` é critério de maturidade (anos, não dias).

---

## 5. Critérios para 1.0

| # | Critério | Status atual | O que falta |
|---|----------|--------------|-------------|
| 1 | Rust dependency removida para usuários finais | Phase 1, 2, 3 completas; SystemLinker default implementado | Smoke em máquinas sem Rust; CI job sem Rust; `docs/install.md` |
| 2 | Stdlib portada em `.orl` (Layer 2+3) | Layer 2/3 entregues; Layer 1 permanece Rust | Mais módulos Layer 2 cold-path; trait gate para genéricos |
| 3 | Self-hosting ou bootstrapping documentado | Não iniciado | Adiado; bootstrapping documentado é alternativa aceitável |
| 4 | ABI estável documentada | Parcial | Documentar layout, calling convention, name mangling |
| 5 | Usuários reais | Zero | Primeiros projetos externos; feedback |
| 6 | Sem breaking changes por ≥6 meses | Não atingido | Congelar sintaxe por 6 meses após estabilização |

---

## 6. Próximos passos táticos

### Imediato (esta semana)
- [ ] Smoke em máquina Windows sem Rust instalado (apenas VS Build Tools)
- [ ] Smoke em máquina Linux sem Rust instalado (apenas build-essential)
- [ ] Smoke em máquina macOS sem Rust instalado (apenas Xcode CLT)

### Curto prazo (próximo mês)
- [ ] CI job que valida release package em runner sem Rust toolchain
- [ ] Documentar instalação de prereqs do sistema (`docs/install.md`)
- [ ] Reduzir tamanho do release package (rust-lld como fallback opcional)

### Médio prazo (próximos 3-6 meses)
- [ ] Documentar ABI C completa
- [ ] Congelar sintaxe central por 6 meses
- [ ] Bootstrapping documentado (mesmo que parcial)

---

## 7. Referências

- `compiler/crates/ori-codegen/src/native_backend.rs` — `NativeLinker::discover()`, `discover_system_linker()`, `discover_bundled_rust_lld()`
- `compiler/crates/ori-codegen/src/native_backend/jit.rs` — `run_jit()`
- `tools/stage_native_runtime.ps1` / `.sh` — Staging de runtime + cdylib + rust-lld
- `AGENTS.md` — Seção "Rust Independence Strategy"
- `CHANGELOG.md` — Entradas `[Unreleased]` sobre Rust removal
- `docs/planning/uso-real-pequeno-medio.md` — Seção "Decisões futuras sobre 1.0"
