---
id: selfhost-exec-plan
title: Self-host gradual do compilador Ori
status: in_progress
rfc: docs/rfcs/0001-selfhost-gradual.md
target_version: undecided
started: 2026-09-06
---

# Plano de execução: self-host gradual

**Objetivo:** referência Rust reproduzível, lockada e auditável, depois migração gradual verificável em Ori. Implementação não concluída. Marco A parcial e bloqueado. Sem iniciar lexer.

**Fonte de verdade:** `docs/rfcs/0001-selfhost-gradual.md`. Esta tabela completa é mantida aqui; outros resumos não substituem estes estados.

**Reversão e fallback:** stage0 permanece referência. Cada fase preserva fallback e reversão explícitos. Não promover implementação Ori como padrão sem rollback testado.

**Advertência bootstrap:** stage2/3 iguais não provam correctness; divergência útil é sinal de inspeção, não falha automática. Compilador e testes compartilham origem; regressões independentes e revisão humana continuam obrigatórias.

## Marco A — referência Rust reproduzível

**Status:** `partial`, bloqueado por divergência binária, teste incompleto e CI. **DoD não satisfeito.** A causa da divergência e de E0460 ainda exige diagnóstico; não se atribui tudo a bloqueio externo.

**Evidência real, sem concluir Marco A:**

- Commit: `b894ca1` (`feat/retire-c-backend-optimize`).
- Toolchain: `1.95.0` (`rust-toolchain.toml`).
- Lockfile base A: `b10f6dde2650e23654cc0fb4327b41dcdc2e5efaf5c37cad1e5de20bfffff475`.
- Comando worktree A concluído com `Finished release ... in 35m 40s`.
- Comando worktree B concluído com `Finished release ... in 33m 44s`.
- Hashes base B:
  - `ori`: `441c48d9c795d6bc44ade0f03464f1141537e46203da9fe06c52a098c65ef0c2`
  - `ori-lsp`: `7d728013df83965214e3de758b45e5fefa8f56e32813b7012972a2a6c17f804f`
  - `libori_runtime.a`: `551ed843ee7965f6e451228fa22963a0587abc941558aa0d6f8a4593a2c00159`
  - `libori_runtime.so`: `be04cd6220d5fc6c8f0926307412875d49c9a9ee31c04fb6627ed55ee43c2328`
- Comparação: `cmp` falhou entre worktrees no byte 41 de `ori`; base A relatou outro hash para `ori`/`ori-lsp`; estáticas/dinâmicas coincidiram entre builds.
- `cargo check --workspace --locked` passou; log `baa6ff1d07d8388ec998ff4a0eac7b944cbdd24a2211108b6e76085536c3fedd`.
- `cargo clippy ... -- -D warnings` passou; log `fbe3422dd0f4dfa8d55f7413703eb36678d44cac865c5116e38b72fb6a82b885`.
- `cargo check --manifest-path compiler/Cargo.toml --workspace --locked` passou; log `baa6ff1d07d8388ec998ff4a0eac7b944cbdd24a2211108b6e76085536c3fedd`.
- `cargo clippy --manifest-path compiler/Cargo.toml --workspace --all-targets --all-features --locked -- -D warnings` passou; log `fbe3422dd0f4dfa8d55f7413703eb36678d44cac865c5116e38b72fb6a82b885`.
- `cargo test --manifest-path compiler/Cargo.toml --workspace --locked` falhou somente no doctest `ori-driver` com `E0460` por metadata mista de `ori-runtime`; log `b67503f52689f1420beac2bc424f5d2f55ca1b57efdef8772460b65ae82645f8`.
- Correção válida no caminho canônico: usar `cargo check/test/run --manifest-path compiler/Cargo.toml`, não `cargo --manifest-path ... check`.
- PR de integração proposta: <https://github.com/ori-team/ori-lang/pull/12>.
- CI da PR ainda não conclusivo: jobs Linux/macOS falharam, Windows pendentes, docs passou; sem logs de falha recuperáveis porque a execução ainda estava em andamento.
- Primeiro commit local do plano: `ae8721d`; confirmação de publicação deve usar o estado remoto, não inferir sucesso de saída truncada.

### Receita e ambiente registrados

Duas worktrees detached limpas do commit `b894ca142f11e2e5c8cb69abeb5a3768ed35456a`: `/home/raillen/Documentos/Projetos/ori-selfhost-base-a` e `ori-selfhost-base-b`. `git status --short` vazio após a execução. Targets separados, sem outputs compartilhados. Cache de downloads Cargo compartilhado; não são duas máquinas independentes nem ambiente hermético. `/tmp` era tmpfs com 2,5 GiB livres; worktrees no disco com 120 GiB livres. Builds sequenciais, um job.

Rust 1.95.0 `59807616e1fa2540724bfbac14d7976d7e4a3860`, LLVM 22.1.2; Cargo 1.95.0 `f2d3ce0bd`; GCC 16.2.1 20260810; GNU ld 2.47; target `x86_64-unknown-linux-gnu`. O ambiente original tinha `RUSTUP_TOOLCHAIN=stable` (Rust 1.98.1); não foi usado nas duas builds de referência.

Em cada worktree, executar (substituir a raiz A pela B na segunda):

```sh
env -i HOME="$HOME" PATH="$HOME/.cargo/bin:/usr/bin:/bin" RUSTUP_TOOLCHAIN=1.95.0 LANG=C LC_ALL=C TZ=UTC CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo fetch --manifest-path compiler/Cargo.toml --locked --target x86_64-unknown-linux-gnu
env -i HOME="$HOME" PATH="$HOME/.cargo/bin:/usr/bin:/bin" RUSTUP_TOOLCHAIN=1.95.0 LANG=C LC_ALL=C TZ=UTC CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true RUSTFLAGS="--remap-path-prefix=/home/raillen/Documentos/Projetos/ori-selfhost-base-a=/ori-source" cargo build --manifest-path compiler/Cargo.toml --workspace --release --locked --offline --target x86_64-unknown-linux-gnu
```

Fetch executado em A; B reutilizou apenas os downloads. Remap não foi suficiente para igualdade de ori/ori-lsp. SHA256 A: ori `f062abf5bddb73a6499e4bb9185ba974d8f294f47192712c8461d9701fa51df8`; ori-lsp `abc7f5003cde662965dd8f489741968cf165f67f45861a3385db2da8cb8be885`. Runtime A tem os mesmos hashes B acima. Toolchain file SHA256 `12dc5d64dbd96c1b07be2943241d52d9f6127a79ba47a820455ba7b873fc896a`.

Logs build A/B SHA256: `bc5ab2434f3f36979566426badca806d6141957d1592aae7c1456ec4fced0ba4` / `75964570a407455fd8d649b05de280d009ecac2b26a38626ed45abf3c2dff053`. Logs temporários não são armazenamento durável; os resultados e erros relevantes estão transcritos aqui. Gates locais usaram `RUSTUP_TOOLCHAIN=1.95.0 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`; diferem do ambiente sanitizado das builds. E0460 impediu terminar doctests subsequentes; não há aprovação da suíte completa.

### Integração segura

`main`/`origin/main`: `a920bc0`. Branch de trabalho: `feat/selfhost-marco-a`, baseada em b894ca1 sem merge remoto. Nenhuma branch ou stash apagada. `origin/master` e `origin/legacy-master-backup` excluídas como histórico. Branches de rename/QA/fix têm patches equivalentes já incorporados. `feat/consolidate-all-open-prs` tem b35b2d7/940935b superados pelo inlining conservador e remoção C de b894ca1; não reintroduzir. `fix/ci-linux-link-and-c-any-adapter` contém patch do backend C removido e hashes antigos de runtime; excluído. PR #12 aguarda gates; não houve bypass.
- Logs brutos: `/tmp/opencode/selfhost-build-a.log`, `/tmp/opencode/selfhost-build-b.log`, `/tmp/opencode/selfhost-check.log`, `/tmp/opencode/selfhost-clippy.log`, `/tmp/opencode/selfhost-tests.log`.

**Bloqueadores para concluir A:**

- Reproduzir segunda build byte-idêntica antes de declarar referência.
- Reexecutar workspace limpo após `E0460` e obter resultado verde rastreável.
- Obter CI verde na integração antes de merge.
- Persistir runtime staged com hashes, provenance e smoke isolado.
- Documentar variâncias remanescentes segundo operações de builds reproduzíveis.

**Sem claim de self-host pronto:** fingerprint de fonte ou build ainda não está completo/verde.

## Marcos restantes

Os artefatos de Marco A vivem nesta seção; não repetir binários grandes no repositório. Registrar apenas comandos, hashes, logs e decisões.

| ID | Marco/tarefa | Entradas e dependências | Effort | Status |
|---|---|---|---:|---|
| SH-BASE01 | Reprodutibilidade de referência: duas builds limpas `1.95.0`, lockfile, hashes, provenance | stage0 fixado; toolchain/lock/target | M | `blocked` |
| SH-BASE02 | Runtime AOT/JIT staged a partir das entradas verificadas | SH-BASE01 | S | `todo` |
| SH-BASE03 | Corpus e baseline de testes/goldens da referência | SH-BASE01/02 | M | `todo` |
| CONTRACT01 | Contrato versionado do protocolo host/processo e fronteira IR | SH-BASE01 | L | `todo` |
| PROBE01 | Probe Ori executada pela referência: validar operações necessárias | SH-BASE01/03 | M | `todo` |
| BRIDGE01 | Processo Rust receptor: parse/validação, erros versionados, geração via Cranelift existente | CONTRACT01 | L | `todo` |
| BRIDGE02 | Endurecimento: limites, framing, timeout/cancelamento, isolamento, negação versionada | BRIDGE01 | M | `todo` |
| DATA01 | Serialização determinística e validação de dados/definições | CONTRACT01, BRIDGE01 | M | `todo` |
| SOURCE01 | Carregamento e grafo de fontes/manifestos em Ori | SH-BASE03, PROBE01 | M | `todo` |
| LEX01 | Lexer Ori equivalente, com spans e recuperação diagnosticada | SOURCE01 | L | `todo` |
| AST01 | AST/parser Ori com goldens e erros recuperáveis | LEX01 | L | `todo` |
| RESOLVE01 | Resolução/import/visibilidade com mesmos diagnósticos | AST01 | L | `todo` |
| TYPE01 | Tipos/signaturas/inferência e negativas | RESOLVE01 | XL | `todo` |
| HIR01 | Lowering tipado/HIR e verificadores | TYPE01 | L | `todo` |
| NATIVE01 | Mid-end: passes, diferenciais opt on/off; sem otimizações extras | HIR01 | M | `todo` |
| OPT01 | Otimizações somente com diferencial semântico + benchmark | NATIVE01 | M | `todo` |
| NATIVE02 | Lowering para IR de backend estável através da bridge | CONTRACT01, HIR01, BRIDGE01/02, DATA01 | XL | `todo` |
| CLI01 | CLI/bootstrap stage0→stage1 com seleção e fallback | Estágios anteriores aplicáveis | L | `todo` |
| BOOT01 | stage1/2/3 mesma fonte; comparação e classificação de diferenças | CLI01, NATIVE02 | XL | `todo` |
| BOOT02 | Promoção/rollback, stage0 preservado, distribuição identificada | BOOT01 | M | `todo` |
| QUALITY01 | Corpus externo, fuzzing minimizado, regressões independentes | BOOT01 | L | `todo` |
| TOOLS01 | Formatter/LSP/docs/debugger sobre contratos compartilhados | Fases donas | L | `todo` |
| ROLLOUT01 | Pacote/experimento controlado com rollback | BOOT02, QUALITY01 | M | `todo` |
| ROLLOUT02 | Estabilização e critérios de padrão | ROLLOUT01 | M | `todo` |
| RUNTIME01 | Manutenção de compatibilidade do runtime Rust/ABI; sem runtime Ori | Contínuo | M | `todo` |

Critério de conclusão por item: comportamento entregue, testes/gates pertinentes, contratos/docs atualizados, compatibilidade/segurança/performance avaliadas e risco residual registrado. Status usa apenas `todo`, `in_progress`, `blocked`, `done`.
