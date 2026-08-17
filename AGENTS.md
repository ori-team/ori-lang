# Ori Language — Project Context

> Ori is a reading-first, explicitly typed programming language (**surface S3 / 0.3.0**). Compiler written in Rust.

## Skills (Grok / agentes)

Precedência: **este `AGENTS.md` > skills globais > defaults**.

### Obrigatórias em toda tarefa de código

| Skill | Quando / o que exige |
|-------|----------------------|
| **`clean-code`** | Modularização, nomes (verbo+domínio), DRY/KISS (Rule of Three), anti-primitivos, funções stepdown, sem `utils` lixeira. Refs: `naming`, `modularization`, `types-and-primitives`, `adherence-checklist`. |
| **`rust`** | Newtypes/IDs, `Result` tipado, `pub` mínimo, API Guidelines, fmt/clippy/test. Script: `rust_quality.sh`. |
| **`living-docs`** | Docs com o código; spec normativa vs planning; CHANGELOG; sem duplicar escopos. |
| **`compiler-dev`** | Front-end e processo: fase correta, diagnostics no catálogo, stdlib sync. |
| **`lang-compiled`** | AOT: IR/SSA, multi-backend (native+C), ABI/link/runtime, opts, JIT-as-run, parity. |
| **`ori-testing`** | Feature/fix: L1 `check` → L2 `compile` → L3 run → regressão em `ori-driver` + `diagnostic_catalog`. |
| **`ori-lang-qa`** | QA diário/matriz de testes, residuals Spec 14, qualidade de diagnostics, FREEZE-1. Stages: `tools/qa/daily_fast.sh`. |

### Sob demanda

| Skill | Quando |
|-------|--------|
| **`lang-interpreted`** | Só se houver experimentação de VM/bytecode (produto Ori é AOT-first) |
| **`code-review`** | Review estrutural ambicioso / code judo |
| **`check-work`** | Verificar trabalho antes de fechar slice grande |
| **`semantic-web`** | UI/docs web pontuais |
| **`astro-docs-site`** | Site em `ori-website` (repo irmão) |

### Convenção local que sobrescreve clean-code

- **Identificadores e comentários no código: inglês** (esta matriz do projeto).
- **Documentação de usuário (install, guides, language tour, root README):** inglês canônico + irmão `*.pt-BR.md` (política em `docs/README.md`).
- **Spec normativa (`docs/spec/`):** inglês apenas.
- **Planning:** manter o idioma do arquivo existente (PT ou EN).
- Comentários `// SAFETY:` em `unsafe`: inglês, invariantes explícitas.

## Architecture

```
ori-lang/
├── compiler/                  # Rust compiler Cargo workspace
│   └── crates/                #   ori-* crates
│   ├── ori-lexer/             #   Lexer + tokenizer (logos-based)
│   ├── ori-ast/               #   AST definitions (all nodes)
│   ├── ori-parser/            #   Parser (recursive descent)
│   ├── ori-types/             #   Type system (Ty, TyKind, TypeChecker trait)
│   ├── ori-hir/               #   High-level IR (lowered from AST)
│   ├── ori-codegen/           #   Code generation (Cranelift native + C debug)
│   ├── ori-runtime/           #   Native runtime library (Rust staticlib)
│   ├── ori-diagnostics/       #   Diagnostic codes + rendering
│   ├── ori-lsp/               #   LSP server (tower-lsp)
│   └── ori-driver/            #   CLI driver + integration tests
├── runtime/                   # Pre-built runtime static libs per target triple
│   ├── x86_64-unknown-linux-gnu/
│   └── x86_64-pc-windows-msvc/
├── stdlib/                    # Ori standard library (.orl source)
├── docs/
│   ├── spec/                  #   Language specification (normative)
│   └── planning/              #   Implementation plans
├── tests/                     # End-to-end Ori test programs (.orl)
├── docs/archive/              # Historical audit documents (reverse engineering)
├── branding/                  # Logo and brand assets
├── examples/                  # Example Ori programs
├── extensions/vscode-orl/       # VS Code extension (LanguageClient → ori-lsp)
├── tools/                     # Auxiliary tools
└── (vendor/ — reserved for future vendored deps; not created yet)
```

## Convention Matrix

| Aspect | Convention |
|--------|------------|
| **User-facing docs (GitHub)** | **English primary** + **Portuguese parallel** (`*.pt-BR.md`); index `docs/README.md` |
| **Normative spec** (`docs/spec/`) | **English only** |
| **Planning / backlog** | PT or EN (existing file language); not end-user tutorials |
| **Code + comments** | English |
| **Compiler design** | Follow best practices |
| **Testing** | Always use `ori-testing` skill for new features |
| **Changelog** | Always update `CHANGELOG.md` with changes |
| **Bug fixes** | Always add regression test in `compiler/crates/ori-driver/tests/` |
| **Pre-implementation** | Check [`docs/ATLAS.md`](docs/ATLAS.md), [`docs/planning/BACKLOG.md`](docs/planning/BACKLOG.md), and [`docs/planning/PENDENTES.md`](docs/planning/PENDENTES.md); the maturity plan is historical in `docs/planning/historico/` |
| **Stdlib changes** | Update `stdlib.rs`, `lower.rs` (stdlib_c_name + stdlib_c_func_ty), and changelog |
| **Documentation** | Keep `spec/` (normative), `planning/` (plans), `archive/` (historical) |
| **Dedup** | Consolidate documents of same scope, avoid duplication |

## Key Files

| File | Role |
|------|------|
| `compiler/crates/ori-runtime/src/lib.rs` | Canonical native runtime (Rust) |
| `compiler/crates/ori-driver/src/main.rs` | CLI entry point |
| `CHANGELOG.md` | All notable changes (Keep a Changelog format) |
| `Cargo.toml` | Workspace root (10 crates) |
| `docs/spec/13-error-catalog.md` | Diagnostic code registry |
| `.cargo/config.toml` | relocation-model=pic for PIE-compatible runtime |

## Build & Test Commands

```bash
# Compiler workspace lives under compiler/ (M2.layout)
cd compiler

# Check entire workspace
cargo check --workspace

# Run all tests
cargo test --workspace

# Run specific test suite
cargo test -p ori-driver --test ori_spec

# Build runtime (for native backend staging)
cargo build -p ori-runtime --lib
cp target/debug/libori_runtime.a ../runtime/x86_64-unknown-linux-gnu/

# Run diagnostic catalog consistency test
cargo test -p ori-driver --test diagnostic_catalog

# Ori CLI
cargo run -p ori-driver -- check ../examples/hello
cargo run -p ori-driver -- compile ../examples/hello
cargo run -p ori-driver -- run ../examples/hello
```

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `ORI_RUNTIME_LIB` | Override path to runtime static library |
| `ORI_NATIVE_LINKER` | Diagnose raw native linker route |
| `ORI_USE_RUST_LLD=1` | Use rust-lld instead of system linker (still via `rustc` driver) |
| `ORI_USE_BUNDLED_RUST_LLD=1` | Force bundled `rust-lld` (hard fail if discovery fails). By default, bundled lld is tried automatically before `rustc`. |
| `ORI_USE_RUSTC_DRIVER=1` | Legacy `rustc` link driver. **Not for packaging:** double-links libstd against `ori-runtime` staticlib (`rust_eh_personality`). Prefer SystemLinker / BundledRustLld for AOT. |
| `ORI_RUST_LLD` | Explicit path to `rust-lld[.exe]` for the bundled strategy (else discovered from `<ori.exe dir>` or `rustc` sysroot) |
| `ORI_USE_SYSTEM_LINKER=1` | Bypass `rustc` and `rust-lld` — invoke the platform system linker directly (`link.exe`/`ld`/`mold`) with compiler-side CRT discovery (Rust removal Phase 2: Windows MSVC via `vswhere.exe` + `link.exe` discovery, Linux GNU via PATH `mold`→`ld.lld`→`ld` then `cc -print-prog-name=ld`, macOS via `xcrun --find ld`) |
| `ORI_SYSTEM_LINKER` | Explicit path to the system linker (`link.exe`, `ld`, `mold`, etc.) for the `SystemLinker` strategy |
| `ORI_STAGE_PROFILE` | Default profile for `tools/stage_native_runtime.*` when `--profile`/`-Profile` is omitted (`debug` or `release`; scripts default to **release**) |
| `ORI_USE_JIT=1` | Force JIT for `ori run` — execute Cranelift code in-process via `JITModule` with runtime symbols resolved from the staged cdylib through `libloading` (Rust removal Phase 3: no `.o` file, no linker, no subprocess). When unset, JIT is the default whenever a runtime cdylib is available. `ori compile` and `ori test` remain AOT. |
| `ORI_USE_AOT=1` | Force AOT compile+link for `ori run` even when a runtime cdylib is available (opt-out of JIT default). |
| `ORI_RUNTIME_CDYLIB` | Explicit path to the runtime cdylib (`ori_runtime.dll`/`libori_runtime.so`/`libori_runtime.dylib`) for the JIT path. When unset, resolves via packaged runtime → cargo fallback (same search order as `ORI_RUNTIME_LIB` but for the cdylib artifact). |
| `ORI_DISABLE_INCREMENTAL=1` | Disable reuse of matching native outputs from `.ori/incremental.json`; dependency-bearing projects without `ori.lock` already rebuild conservatively. |
| `ORI_OBJCOPY` | Override the `objcopy`/`llvm-objcopy` executable used to add Linux DWARF sections after linking. |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | Validate release package uses only packaged runtime |
| `UPDATE_EXPECT=1` | Update expected diagnostic outputs in tests |
| `ORI_TEST_LEAK_CHECK=1` | When set, `ori.test.assert_no_leaks(label)` aborts with a stderr diagnostic if live ARC allocations remain after running the cycle collector. Use in E2E tests to fail fast on memory leaks. |
| `ORI_COOPERATIVE_COLLECT_THRESHOLD=N` | Number of managed allocations between cooperative cycle collections in the async executor (default 256). Set to a small value in tests to force frequent collection. |
| `ORI_STDLIB_ROOT` | Override path to the `stdlib/` directory containing `.orl` source modules (Stdlib Phase 0). When unset, resolves to `CARGO_MANIFEST_DIR/../../../stdlib` (dev mode) or `<ori.exe dir>/stdlib` (release package). |
| `ORI_TARGET_TRIPLE` | Select target facts for structured `@cfg` and native runtime artifacts (`ori --target`). Does not by itself enable full native cross-compilation. |
| `ORI_EXECUTION_PROFILE` | Select `standalone` or `embedded` for `@cfg(execution_profile: ...)` (`ori --execution-profile`). |
| `ORI_FEATURES` | Comma-separated manifest-declared features enabled for `@cfg(feature: ...)` (`ori --features`). |
| `ORI_NO_DEFAULT_FEATURES=1` | Disable the manifest's `[features].default` set (`ori --no-default-features`). |
| `ori.lsp.path` / `ori.compiler.path` / `ori.stdlib.root` / `ori.cfg.*` | VS Code extension settings (`extensions/vscode-orl/`) — cfg target/profile/features are forwarded to the compiler, LSP, debugger, and extension terminals. |

## Compiler Pipeline

```
Source (.orl)
  → Lexer (ori-lexer): tokens
  → Parser (ori-parser): AST
  → Resolver (ori-hir): name resolution, binding
  → Type Checker (ori-types): type inference + diagnostics
  → Codegen (ori-codegen):
      ├── Native: Cranelift → object file → link with ori-runtime
      └── C debug: transpile to C (partial parity)
  → Binary
```

## Current Status (2026-08-08)

- **Rust:** 1.95.0 (via `rust-toolchain.toml`)
- **Language surface:** **`0.3.0` S3** + **`0.3.1`** Nim-local inference + **option B** (field/index/call/pipe). Manifesto: `docs/spec/00-manifesto.md`. Decisões: `docs/planning/ori-surface-s3-auk9.md`. Spec: `04-types`, `05-expressions` (pipe), `06-statements`.
- **Cargo workspace package version:** **`0.3.8`** (living development baseline).
  Released tags through **`v0.3.7`**; the current language branch contains the
  0.3.7 fixes as equivalent cherry-picks plus the post-release language work.
- **Etapas 0–9** do plano histórico `docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md` (ciclo 0.2) concluídas; S3 PRs 1–10 + PR11 + 11b (B) = superfície atual.
- **Pipe `|>`:** **mantido** e tipado no checker como `f(value)`; entra na inferência local B.
- **Auk9:** produto **arquivado** (README no repo auk9-lang). Living surface is Ori S3.
- **Product focus:** language completeness, docs/examples, performance, local DX
  (VS Code + Zed under `extensions/`). **No** Marketplace/store publish and **no**
  multi-OS distribution push until that focus is done. Game/imgui packages do
  not exist and must not reappear in docs or plans.
- **`tools/qa/daily_fast.sh`:** PASSES (2026-08-08: workspace check, strict
  Clippy, frontend units, 238 `ori_spec`, diagnostic catalog, ARC/memory,
  robustness, residual product surface).
- **Release smoke:** `tools/smoke_native_release.ps1 -SkipBuild` passes — `ori compile` + `ori test` validados em package isolado com runtime empacotada (Windows MSVC).
- **Rust removal Phase 1 — Windows MSVC (unreleased):** `ORI_USE_BUNDLED_RUST_LLD=1` engaja estratégia `BundledRustLld` que invoca `rust-lld` diretamente (sem `rustc` driver). CRT discovery via `vswhere.exe` + Windows SDK layout. Validado end-to-end com `examples/hello_world.orl` em Windows MSVC. `tools/stage_native_runtime.ps1` agora copia `rust-lld.exe` para `runtime/bin/`.
- **Rust removal Phase 1 — Linux GNU (unreleased):** Estratégia `BundledRustLld` estendida para `x86_64-unknown-linux-gnu`. CRT discovery via `cc -print-file-name` (crt1.o/crti.o/crtn.o) + `cc -print-search-dirs` (lib dirs) + fallback de paths comuns para dynamic linker. `tools/stage_native_runtime.sh` agora copia `rust-lld` para `runtime/bin/`.
- **Rust removal Phase 1 — macOS (unreleased):** Estratégia `BundledRustLld` estendida para `x86_64-apple-darwin` e `aarch64-apple-darwin`. CRT/SDK discovery via `xcrun --show-sdk-path` + `xcrun --show-sdk-version` (requer Xcode Command Line Tools). Link line `rust-lld -flavor darwin` com `-arch`, `-platform_version macos <min> <sdk>`, `-syslibroot`. Deployment target default `10.12` (x86_64) / `11.0` (arm64), override via `MACOSX_DEPLOYMENT_TARGET`. **Phase 1 completa para todos os 3 desktop OSes** (Windows MSVC, Linux GNU, macOS).
- **Rust removal Phase 2 — SystemLinker (unreleased):** Estratégia `SystemLinker` (`link.exe`/`ld`) sem `rustc`. Opt-in forçado via `ORI_USE_SYSTEM_LINKER=1`. Discovery: Windows MSVC / Linux GNU / macOS. **Prioridade default (LANG-PERF 2026-07-13):** `ORI_NATIVE_LINKER` → force `ORI_USE_BUNDLED_RUST_LLD` → force `ORI_USE_SYSTEM_LINKER` → **auto BundledRustLld if found** → **auto SystemLinker** → `RustcDriver`. 4 testes de regressão em `native_backend/tests.rs`.
- **Rust removal Phase 3 — JIT Cranelift (unreleased):** `ori run` usa JIT por default quando cdylib disponível; `ORI_USE_JIT=1` força JIT; `ORI_USE_AOT=1` força AOT. Código Cranelift executado in-process via `JITModule` com símbolos `ori_*` resolvidos on-demand da cdylib do runtime através de `libloading`. Sem `.o` temporário, sem linker, sem subprocesso. `ori-runtime` builda 3 artefatos (`staticlib` + `rlib` + `cdylib`); stage scripts copiam cdylib para `runtime/<triple>/`; smoke release valida cdylib staged + `ori run` JIT no package isolado. `ori compile` e `ori test` permanecem AOT. **Híbrido A→B→D completo** para `ori run`.
- **Stdlib Phase 0 + Gap parity (unreleased):** Prelude loading + **Layer 2/3 `.orl` fechados** para paridade `std.*` v1 (`docs/planning/historico/stdlib-gap-parity.md`): 28 utils + 8 algorithms + `validate`/`path`; Layer 1 hot path Rust (FS metadados, `os.current_dir`, `process.*`, `net.*`, `lazy.is_consumed`, …). Lowering `ori.net.Connection`/`Listener`/`UdpSocket` e `ori.io.Input`/`Output` para módulos `.orl`. ~36 testes stdlib E2E em `multifile_imports.rs` (incl. rede v2).
- **Stdlib/Rede v2 (unreleased):** `connect_tls`, servidor TCP (`listen`/`accept`), UDP síncrono, `task.run_blocking`; design histórico em `docs/planning/historico/net-v2-design.md`; exemplo `examples/http_get.orl`.
- **FFI aggregates ABI v1 scope (unreleased):** `@c_export` accepts non-empty,
  non-generic scalar-field structs through portable pointer/out wrappers and
  managed structs through typed opaque ARC handles, plus direct
  `optional`/`result` through tag + payload/out bridges. Handle parameters are
  borrowed through a wrapper retain; active managed returns transfer one owned
  reference to the host. `ori compile --lib` generates the sibling C header
  from HIR, including scalar typedefs, incomplete handle declarations,
  `OriResultTag`, export signatures, runtime retain/release lifecycle, and C++
  guards. Direct collection layouts remain private behind opaque handles.
- **Custom destructors (unreleased):** `core.Destructor` exposes
  `mut destroy(self) -> void` on structs/enums. Native AOT/JIT call it once
  before payload/field cleanup; cycle collection finalizes all members before
  freeing any payload. C/debug rejects the feature explicitly. Deterministic
  resources still use `using` + `core.Disposable`.
- **Medium language backlog closed (2026-07-23):** explicit `move` shelved by
  language decision; the size-class free-list experiment was ~2% slower than
  glibc tcache and reverted, with
  `tools/bench/managed_temporary_churn.orl` retained as evidence.
- **LSP/VS Code (unreleased):** Catálogo stdlib Layer 1+2, hover/goto stdlib, sync incremental, dot-complete via aliases, `ori doctor`, extensão `extensions/vscode-orl/`.
- **Docs website (unreleased):** Site Starlight em [github.com/raillen/ori-website](https://github.com/raillen/ori-website) — i18n en/pt/es/ja, Pagefind + busca ⌘K, referência gerada via `ori doc export`. Deploy Vercel-ready (`vercel.json`).
- **Master plan histórico:** `docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md` — Etapas 0–9 concluídas; backlog vigente em `docs/planning/BACKLOG.md`. **M2 ✅** (stdlib + `public alias` de domínio); **M3 ✅** (`19-abi.md`); **M1 ✅** (`docs/install.md`, `tools/smoke_no_rust.sh`, CI smoke-no-rust). Próximo opcional: publicar package; **M4** self-host por último.

## Versioning policy (2026-07-13)

**Histórico:** S3 = **`0.3.0`**. Inferência + opção B = **`0.3.1`**. Package + M1/M3 = **`0.3.2`**. Language-first + freeze + deb = **`0.3.3`**. Living link/smoke fix = **`0.3.4`**.

| Linguagem | Tempo em 0.x | Versão atual | Status |
|-----------|-------------|--------------|--------|
| Zig | ~10 anos | 0.14 | Consolidada, ainda sem 1.0 |
| Rust | ~6 anos (pre-1.0) | 1.0 em 2015 | Estável após 0.12 |
| Ori | dias | **0.3.8-dev** (`v0.3.7` released) | Pre-1.0, FREEZE-1 em 0.3.x |

**Regras até 1.0:**
- Superfície S3 = CHANGELOG **`[0.3.0]`**; inference = **`[0.3.1]`**; package/M1 = **`[0.3.2]`**; deb/freeze = **`[0.3.3]`**; link/smoke maintenance = **`[0.3.4]`**.
- Cargo/`runtime-link.json` = versão atual do workspace (**0.3.8**).
- Patch versions (`0.3.5`, …) para correções e small additive features.
- `0.4+` só com breaking real ou marco grande acordado.
- `1.0` é critério de maturidade (anos, não dias), na **ordem tática**:
  1. **Stdlib** consolidada (Layer 2+3; pais `ori.X` + `public alias` de domínio) — **M2 ✅**
  2. **ABI estável documentada** — **M3 ✅** (`docs/spec/19-abi.md`, `ori-native-abi-1`)
  3. **Independência do Rust** para quem instala Ori sem toolchain Rust — **M1 ✅**
  4. Self-hosting = **última** discussão de linguagem (**M4**)
  5. Estabilidade de contrato (ex.: sem breaking prolongado) quando se aproximar de 1.0

**Prioridade tática (2026-07-13):** **M2 ✅ → M3 ✅ → M1 ✅ → M4 (última)**. Ver `docs/planning/PENDENTES.md`.

## Rust Independence Strategy (2026-07-02)

> Definição de "independência do Rust": **um usuário final pode instalar Ori, compilar, rodar e testar programas Ori sem ter a toolchain Rust instalada.**  
> Isso não significa que o *compilador em si* deixa de ser escrito em Rust — isso só ocorre com self-hosting (futuro longo prazo).

### O que já foi feito (Rust removal híbrido A→B→D)

| Phase | O que faz | Status | Gap residual |
|-------|-----------|--------|--------------|
| **Phase 1 — BundledRustLld** | Invoca `rust-lld` direto, sem `rustc` driver | ✅ Completo (3 OSes) | Ainda precisa do binário `rust-lld` (vem do Rust toolchain) |
| **Phase 2 — SystemLinker** | Invoca linker nativo do sistema (`link.exe`/`ld`/`ld64`) direto | ✅ Completo (3 OSes) | Requer toolchain do OS (VS Build Tools, build-essential, Xcode CLT) |
| **Phase 3 — JIT Cranelift** | `ori run` sem `.o`, sem linker, sem subprocesso | ✅ Completo | `ori compile`/`ori test` ainda usam AOT (precisam de linker) |
| **Phase 4 — Self-hosting** | Compilador escrito em Ori | ❌ Não iniciado | Anos de trabalho; **última** discussão de linguagem (M4) |

### Pré-requisitos do sistema por OS (para AOT)

O `ori compile` e `ori test` precisam de um linker. Com package completo
(`runtime/bin/rust-lld`), o default é **BundledRustLld**; senão **SystemLinker**.
Para SystemLinker o usuário final precisa ter instalado:

| OS | Pré-requisito | Como instalar | Ori precisa de Rust? |
|----|---------------|---------------|----------------------|
| **Windows** | Visual Studio Build Tools (ou VS Community) | `winget install Microsoft.VisualStudio.2022.BuildTools` ou via installer com workload "Desktop development with C++" | **Não** |
| **Linux** | `build-essential` (`gcc` + `ld`) | `sudo apt install build-essential` (Debian/Ubuntu) ou equivalente | **Não** |
| **macOS** | Xcode Command Line Tools | `xcode-select --install` | **Não** |

Para `ori run` (JIT): **nenhum linker é necessário** — apenas o cdylib do runtime (`ori_runtime.dll` / `.so` / `.dylib`) empacotado no release.

### Decisões arquiteturais fechadas

1. **Self-hosting adiado** até o restante da linguagem estar funcional (M4 — última discussão). Não é pré-requisito para utilidade. Python, Ruby, Lua nunca foram self-hosted; Zig está em 0.14 após ~10 anos.
2. **Runtime Layer 1 permanece Rust.** ARC, async executor, FFI, I/O e rede são hot paths. A ABI C é o contrato público.
3. **AOT linker default (LANG-PERF):** prefer **BundledRustLld** when
   `runtime/bin/rust-lld` (or discovery) is available — faster link, still no
   `rustc`. Else **SystemLinker** (OS `ld`/`link.exe`). Force with
   `ORI_USE_BUNDLED_RUST_LLD=1` / `ORI_USE_SYSTEM_LINKER=1`.
4. **Rust continua necessário apenas para *desenvolver* o compilador.** Quem instala via release package não precisa de `cargo`/`rustc`.

### Critérios técnicos para 1.0 (ordem: M2 → M3 → M1 → M4)

1. Stdlib consolidada (Layer 2+3; Layer 1 Rust por design) — **M2 ✅**
2. ABI estável documentada (`docs/spec/19-abi.md`, `ori-native-abi-1`) — **M3 ✅**
3. Independência do Rust no caminho do instalador final — **M1 ✅**
4. Self-hosting **ou** bootstrapping documentado — **M4** (última)
5. Estabilidade de contrato (ex. janela sem breaking) ao aproximar 1.0

### Próximos passos táticos

**Ordem:** M2 ✅ → M3 ✅ → M1 ✅ → **M4** self-host (última discussão).

- [x] *(M1)* Smoke package (`tools/smoke_native_release.*` S3 + `compiler/target`)
- [x] *(M1)* CI `smoke-no-rust` (linux/windows/macos) sem Rust no PATH
- [x] *(M1)* `docs/install.md` + `tools/smoke_no_rust.sh`
- [x] *(opcional)* Publicar package Win/Linux em release GitHub (ver `docs/install.md`)
- [ ] *(M4)* Self-hosting — só quando o resto estiver estável  
  **Open work (single list):** `docs/planning/BACKLOG.md`

## Known Pitfalls

1. **S3 surface (`0.3.0`):** pre-S3 forms (`namespace`, declaration `func`, `import as`/`only`, `<>`, `else if`, `?`, `do`, `implement`/`apply Trait to`, struct call literals, …) are **hard errors**. Use `ori migrate-syntax` for mechanical rewrites. Spec: `docs/spec/01-overview.md`. Catalog: `docs/spec/13-error-catalog.md`.

2. **Native runtime staging:** `compile_runs` tests fail with `native.link_failed` → runtime needs re-staging. Fix: `cargo build -p ori-runtime --lib && cp target/debug/libori_runtime.a runtime/x86_64-unknown-linux-gnu/`

3. **OnceLock cache in tests:** `find_native_runtime_link()` caches the FIRST result across all tests. If first test finds broken runtime, all subsequent tests fail. Run single test first to verify fix.

4. **Runtime config:** `.cargo/config.toml` requires `relocation-model=pic`. `runtime-link.json` requires `-lpthread -ldl -lm -no-pie`.

5. **Diagnostic code prefixes:** MUST match catalog in `docs/spec/13-error-catalog.md` (enforced by `diagnostic_catalog_matches_emitted_codes`). Convention: `name.*` for name resolution (`name.undefined`, `name.private`, `name.duplicate` for top-level duplicates); `bind.*` for binding/import/field/param errors (`bind.duplicate_field`, `bind.duplicate_param`, `bind.import_not_found`, `bind.stdlib_module_unknown`, `bind.stdlib_module_unavailable`). `bind.undefined` is a reserved alias only — the emitted code is `name.undefined`.

6. **Ori syntax (S3):** `module` header; no declaration `func`; `end`-delimited blocks; types use `[]`; struct literals `Type { f: v }`; traits via `apply`/`use`. Enum variants with named fields use commas inside parens.

7. **Lock file:** Regenerate with Rust 1.95 if build fails: `cargo update` or delete `Cargo.lock` and rebuild.

8. **Windows LSP process lock:** `ori-lsp.exe` may remain locked in memory on Windows, preventing `cargo test --workspace` from rebuilding the `ori-lsp` crate. Workaround: `taskkill /F /IM ori-lsp.exe` before running tests, or use `cargo test -p <crate>` crate-by-crate.

9. **CDYLIB desynchronization after runtime changes:** When new FFI functions are added to `ori-runtime`, the `cdylib` (`.dll`/`.so`/`.dylib`) must be re-staged alongside the static library. An outdated `cdylib` causes `ori run` (JIT default) to produce corrupted results or panic with undefined behavior (`ptr::copy_nonoverlapping` violation). Fix: `cargo build -p ori-runtime --lib` and copy both the staticlib and the cdylib into `runtime/<triple>/`.
