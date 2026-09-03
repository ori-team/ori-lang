# Sessão 2026-07-16 — estudo Nim→Ori (C0) + plano ARC + LANG-PERF-3

> **Propósito deste arquivo:** retomada de contexto após troca de distro.
> Tudo que importa desta sessão está commitado no repo; o que vive fora do
> repo (e se perde na troca) está listado em "O que refazer na máquina nova".

## TL;DR — onde paramos

1. **LANG-PERF-3 corrigido** (registro ARC linear → HashMap; retain/release
   O(1)) — código em `ori-runtime` + guard em `performance_guard.rs`.
2. **Estudo Nim C0 concluído** — nota em
   [`nim-study-2026-07-16-c0.md`](nim-study-2026-07-16-c0.md)
   (glossário 18 termos, destroy path Nim vs Ori, 5 perguntas abertas).
3. **Plano ARC criado** — [`../plano-arc-nim-2026-07-16.md`](../plano-arc-nim-2026-07-16.md),
   fases F0–F7 = itens **LANG-MEM-0…7** no BACKLOG.

**Próximo passo (1 só):** executar **F0 + início de F1** do plano ARC
(corrigir comentário de layout do header + auditoria dtor×edges com testes
S1–S2).

## O que foi feito nesta sessão

| Entrega | Onde |
|---------|------|
| Fix LANG-PERF-3 (ARC registry HashMap + edges indexadas) | `compiler/crates/ori-runtime/src/lib.rs`, `performance_guard.rs`, `native_backend/tests.rs`, `CHANGELOG.md` |
| Issue doc da investigação FFI | `../issue-ffi-dispatch-large-binary-2026-07-16.md` |
| Nota de estudo Nim C0 (Eixos A + esboço 2) | `nim-study-2026-07-16-c0.md` |
| Plano de implementação/correção ARC (F0–F7) | `../plano-arc-nim-2026-07-16.md` |
| Itens LANG-MEM-0…7 no backlog | `../BACKLOG.md` §2 |
| Prompt mestre do estudo preservado no repo | `../prompt-analisar-nim-para-ori.md` |
| Organização dos docs de planning (concluídos → historico) | `../README.md` (índice) |

## Achados-chave do estudo C0 (resumo de 5 linhas)

- Nim: RC inline no header, não-atômico; Ori: lock global + HashMap por op
  — maior gap de perf (elisão é F4, só com baseline).
- Ori tem **duas** cascatas de liberação (dtor de campos + edges);
  sobreposição = double-free/leak em potencial → auditoria é **F1**.
- Collector Nim é incremental (buffer de suspeitos, threshold adaptativo);
  Ori faz full-heap scan → **F3**.
- Comentário do layout do header em `lib.rs` está errado
  (`[u32 rc][u32 type_tag]` ≠ `AtomicI64 + dtor ptr`) → **F0**.
- Spec 10 dizia "sem coleta periódica", mas existe contador cooperativo de
  256 allocs → corrigido nesta sessão.

## O que refazer na máquina nova (troca de distro)

1. Clonar o repo `ori-lang` (este arquivo vem junto).
2. **Baixar o código-fonte do Nim** (não versionado; pré-requisito das
   fases F1+ do plano — instruções completas na seção "Pré-requisito" de
   `../plano-arc-nim-2026-07-16.md`):
   ```bash
   cd <raiz do repo ori-lang>
   git clone --depth 1 --branch devel https://github.com/nim-lang/Nim.git _references/nim-lang
   ```
   Commit estudado na C0: `3bb46d3` (devel de 2026-07-17). O `devel` terá
   avançado; citar sempre `git -C _references/nim-lang rev-parse --short HEAD`
   nas notas novas.
3. Reinstalar o pack de skills do grok-memory
   (`/home/raillen/Documentos/Projetos/grok-memory` → `./scripts/install-claude.sh -y`)
   se esse repo também for migrado.
4. Avisar o Claude: a memória persistente dele (`~/.claude/projects/...`)
   não migra — este arquivo é a fonte de retomada. Pedir:
   *"leia docs/planning/historico/sessao-nim-arc-2026-07-16.md e continue
   com F0+F1 do plano ARC"*.

## Pendências conhecidas (fora deste plano)

- Re-medir o shell ImGui do lab `game-engine-full/ori-studio` (fora do
  repo) com runtime re-stageado (resto do LANG-PERF-3).
- `ori compile` com 10k funções ~4min — candidato a novo LANG-PERF
  (custo quadrático provável no front/mid-end).
- Teste `compile_runs_managed_closure_capture_across_await_native` já
  falhava no master antes desta sessão ("closure capture not available in
  native codegen") — gap pré-existente, não relacionado.
