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

**Status:** `partial`, bloqueado por divergência binária, teste incompleto e CI. **DoD não satisfeito.** Caminhos absolutos na `.rodata` foram confirmados; E0460 não reapareceu na rota release focada, mas sua causa histórica e a suíte completa continuam abertas. Ver diagnóstico abaixo.

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

### Diagnóstico limitado de 2026-09-06

Inspeção dos executáveis existentes com `sha256sum`, `cmp -l`, `readelf -h -S -n -W`, `readelf -p .strtab`, `strings -t d` e `objdump -s`. Nenhuma nova dupla de builds completas foi executada.

- Os quatro hashes dos executáveis conferem com A/B acima. Byte 41 é o primeiro byte de `e_shoff` no ELF64, não timestamp: em `ori`, a tabela de seções começa em 18391184 (A) / 18391232 (B).
- `.strtab` de `ori` mede `0x24f62a` / `0x24f65a`; de `ori-lsp`, `0x2eff74` / `0x2eff9e`. A comparação dos nomes mostra sufixos locais `.llvm.*` diferentes em símbolos de `ori_driver::pipeline::project` e `runtime`. Isso explica diferenças de tamanho e deslocamentos; não é prova de equivalência semântica.
- Build IDs de `ori`: `8ecba4c184032db5df9fb08f8ad0daea3efd7822` / `438e9669c82ffa760111cf745fc2d82e8c6056fa`; de `ori-lsp`: `c049c882c33d3d3954025ecbb8e4e36b210aeb88` / `6ad855d4867785730ffac7d6bab5467ea9af45b6`. Retirar build-id não resolve a divergência.
- Em `ori`, `cmp` identifica bytes 1241129 e 1296991 na `.rodata` como `a` versus `b`. `strings` e `objdump` confirmam caminhos absolutos `ori-selfhost-base-a/compiler/crates/ori-driver` versus `ori-selfhost-base-b/compiler/crates/ori-driver`. A fonte incorpora `env!("CARGO_MANIFEST_DIR")` em `compiler/crates/ori-driver/src/pipeline/project.rs:1549` e `runtime.rs:593-606`; remap de caminhos de fonte não normalizou esses valores. Não há evidência para atribuir a divergência a timestamps; `.comment` contém versões datadas das ferramentas, não uma data de build comprovada. A causalidade dos sufixos LLVM ainda requer experimento controlado.
- E0460 original ocorreu antes de executar exemplos do doctest: rustdoc encontrou `libori_runtime.rlib`, duas `.rmeta` e `libori_codegen-a607adec5de6ef18.rlib` incompatíveis no target debug A. `ori-runtime` declara `staticlib`, `rlib`, `cdylib` e gera rlib sem sufixo; a sequência exata que deixou metadata incompatível não foi comprovada. Presença de duas `.rmeta` sozinha não prova a causa.
- Rota focada release no target explícito A passou após recompilar dependências necessárias e `ori-driver`: `Doc-tests ori_driver`, **0 testes**, sem E0460. Não equivale a suíte completa nem corrige retroativamente o target debug. As tentativas anteriores não chegaram a resultado de teste; seus logs foram preservados, não sobrescritos.

Comando final executado a partir da worktree A:

```sh
env -i HOME="$HOME" PATH="$HOME/.cargo/bin:/usr/bin:/bin" RUSTUP_TOOLCHAIN=1.95.0 LANG=C LC_ALL=C TZ=UTC CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true RUSTFLAGS="--remap-path-prefix=/home/raillen/Documentos/Projetos/ori-selfhost-base-a=/ori-source" timeout 300 cargo test --manifest-path compiler/Cargo.toml -p ori-driver --doc --release --locked --offline --target x86_64-unknown-linux-gnu
```

Logs: `/tmp/opencode/selfhost-driver-doc-release.log` (primeira tentativa verbose), `selfhost-driver-doc-release-resume.log` (limite 600 s), `selfhost-driver-doc-release-final.log` (concluiu em 2m36s). Reuso release recompilou bibliotecas; os hashes runtime originais acima são evidência histórica, não promessa sobre outputs posteriores. Nenhum staging ou smoke isolado foi realizado nesta rodada. Próximo passo: correção controlada dos caminhos incorporados com regressão e validação focada, antes de novas duas builds limpas byte-idênticas. Não aceitar strip ou normalização pós-hoc como aprovação dos artefatos originais.

PR de documentação confirmada remotamente: <https://github.com/ori-team/ori-lang/pull/13>, branch `feat/selfhost-marco-a`. Na consulta desta rodada, docs passou e native-route permanecia em andamento; PR #12 tinha Linux/macOS falhos e Windows em andamento. Sem merge, exclusão de branches ou bypass.

Validação adicional desta rodada, na worktree A: `env RUSTUP_TOOLCHAIN=1.95.0 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 timeout 120 cargo check --manifest-path compiler/Cargo.toml --workspace --locked` passou (1m04s); mesmo ambiente com `timeout 120 cargo clippy --manifest-path compiler/Cargo.toml --workspace --all-targets --all-features --locked -- -D warnings` passou (1m40s). Logs `/tmp/opencode/selfhost-check-diagnostic.log` e `selfhost-clippy-diagnostic.log`, SHA256 `d7ef31ad12924b2d5eb893c193f6639d6adc0bf76d6dbea524e8982804c5d74e` / `4989c9d94c3e4cb3701c5830748b50a18a0238502ff0f55c0fb786c3172b609c`. Log do doctest final SHA256 `c1a6e3cb8b69719bb352bc02f9f32ea79ee663a6009a0f32f9346e57302cb01e`. `git diff --check` passou. Alteração desta rodada é somente documentação; nenhuma correção de runtime, ABI, semântica ou performance foi entregue.

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

### DoD individual obrigatório

Esta tabela define critérios, não entrega concluída. Nenhum item muda para `done` apenas por existir documentação. Cada linha também exige contratos/docs sincronizados, compatibilidade, segurança, performance e risco residual registrados; gates falhos precisam de resolução, não relaxamento.

| ID | Evidência necessária para concluir |
|---|---|
| SH-BASE01 | Duas builds limpas byte-idênticas de todos os artefatos declarados, commit/toolchain/lock/flags/linker identificados, hashes e logs preservados; eliminar caminhos locais e explicar toda divergência. |
| SH-BASE02 | Staticlib/cdylib das mesmas entradas verificadas, símbolos/ABI/versão e hashes validados, metadata determinística, staging limpo e smoke AOT/JIT isolado sem fallback de workspace. |
| SH-BASE03 | Corpus versionado e inventariado, resultados/goldens rastreáveis, workspace completo incluindo doctests verde, catálogo/LSP e testes nativos pertinentes verdes; registrar exclusões e CI verde antes de integração. |
| CONTRACT01 | ADR aceita da fronteira IR e protocolo versionado com schemas, capacidades, IDs/tipos/spans, limites, erros, compatibilidade e fixtures válidas/inválidas verificáveis. |
| PROBE01 | Programas Ori executados pelo stage0 para cada operação necessária, resultados esperados e limitações registrados; nenhum requisito essencial sem suporte ou rejeição explícita. |
| BRIDGE01 | Receptor Rust valida pedidos e gera via Cranelift existente; testes de round-trip, versão incompatível, referência inválida e execução equivalente à referência, sem saída parcial em erro. |
| BRIDGE02 | Limites/framing/cancelamento/timeout testados, entradas truncadas e excessivas rejeitadas, morte do processo e recuperação verificadas, modelo de isolamento e riscos documentados. |
| DATA01 | Serialização determinística demonstrada em repetições, round-trip preserva dados, IDs/referências e limites validados, entradas malformadas cobertas por negativas. |
| SOURCE01 | Grafo de fontes/manifestos Ori equivalente à referência, imports/ciclos/visibilidade de arquivos e erros de I/O cobertos; caminhos inseguros rejeitados conforme contrato. |
| LEX01 | Tokens e spans equivalentes no corpus, Unicode/bordas/entrada inválida e recuperação testados; divergências classificadas contra a spec. |
| AST01 | AST e parser com goldens estruturais, precedência e recuperação de erros verificadas, positivos/negativos diferenciais e diagnósticos compatíveis. |
| RESOLVE01 | Imports, nomes, escopos e visibilidade equivalentes, ambiguidades/ciclos/não encontrados testados, mesmos códigos e spans contratados. |
| TYPE01 | Tipos, assinaturas, inferência, traits/generics e monomorfização na superfície definida; positivos/negativos diferenciais e rejeição explícita do não suportado. |
| HIR01 | Lowering tipado preserva ordem/cleanup/ARC, verificadores rejeitam HIR inválida, goldens e execução diferencial cobrem invariantes. |
| NATIVE01 | Passes existentes preservados com diferencial opt on/off, verificadores e corpus; nenhuma otimização extra introduzida nesta tarefa. |
| OPT01 | Cada otimização tem regressão semântica independente, diferencial opt on/off e benchmark com baseline/target/perfil/amostras/trade-offs; ganho não presumido. |
| NATIVE02 | Lowering atravessa protocolo aceito com tipos/layouts/chamadas corretos, testes AOT/JIT e ABI na superfície comum, rejeição explícita de shapes não suportados. |
| CLI01 | Stage0 constrói stage1, seleção de implementação/estágio explícita, erros e fallback testados, CLI/docs preservam compatibilidade. |
| BOOT01 | Stage1/2/3 produzidos da mesma fonte/flags/runtime/target, hashes e logs registrados, diferenças stage2/3 classificadas e corpus semântico verde além da comparação binária. |
| BOOT02 | Promoção e rollback executados em teste, stage0 preservado e recuperável, distribuição identifica estágio/revisão/backend e incompatibilidades. |
| QUALITY01 | Corpus externo com origem/licença, fuzzing com orçamento e seeds, falhas minimizadas e regressões independentes; revisão humana e riscos residuais registrados. |
| TOOLS01 | Formatter/LSP/docs/debugger integrados aos contratos das fases, testes de formatação/diagnósticos/editor/debug e documentação sincronizados sem catálogos concorrentes. |
| ROLLOUT01 | Pacote identificável validado isoladamente, experimento com escopo/critério de parada e rollback demonstrado; sem promoção silenciosa a padrão. |
| ROLLOUT02 | Critérios de adoção como padrão aprovados e satisfeitos, matriz de compatibilidade/targets e CI verdes, evidência do experimento e rollback preservada. |
| RUNTIME01 | Para cada entrega: runtime Rust mantido, ABI/símbolos/layouts e single-cascade-owner preservados, parity estática/dinâmica e AOT/JIT validada; obrigação contínua, não encerrada antecipadamente. |

Status usa apenas `todo`, `in_progress`, `blocked`, `done`.
