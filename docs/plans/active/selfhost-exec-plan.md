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

**Status:** `partial`, bloqueado externo. **DoD não satisfeito.**

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
- `cargo test --workspace --locked` falhou somente no doctest `ori-driver` com `E0460` por metadata mista de `ori-runtime`; log `b67503f52689f1420beac2bc424f5d2f55ca1b57efdef8772460b65ae82645f8`.
- PR de integração proposta: <https://github.com/ori-team/ori-lang/pull/12>.
- CI da PR ainda não conclusivo: jobs Linux/macOS falharam, Windows pendentes, docs passou.
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
