# Plano de Auditoria e Saneamento da Documentação

> **Status:** Ativo e em andamento  
> **Início:** 2026-09-04  
> **Escopo:** Revisão minuciosa, correção de drifts, saneamento e consolidação de todos os diretórios da árvore `docs/`.

---

## Tabela de Acompanhamento por Diretório

| # | Diretório / Módulo | Papel / O que é | Arquivos | Linhas | Status | Detalhes & Evidências de Conclusão |
|:---:|---|---|:---:|:---:|:---:|---|
| **01** | `docs/` (raiz) | Ponto de entrada (`ATLAS.md`, `README.md`, `install.md`, `catalog.yaml`) | 6 | 1.246 | **done** | Atualizado status de SIMD/Arena no ATLAS (de 'future' para 'done'); alinhadas referências e pins de release de 0.3.7 para 0.3.8 em install.md e install.pt-BR.md; adicionado resumo de sistemas e alta performance nos READMEs; registrado plano no catalog.yaml; validado com docs_coverage.sh |
| **02** | `docs/spec/` | **Especificação normativa** em inglês (gramática, tipos, memória, ABI, stdlib) | 21 | 9.665 | **done** | Atualizados capítulos normativos (01-overview, 02-lexical, 03-grammar, 04-types, 08-traits, 13-error-catalog, 18-stability, README): `as` formalizado em imports, `apply Type: TraitA, TraitB` canônico com suporte a traits múltiplas, `array[T, N]` posicional compacto, polimorfismo direto pelo nome da trait (`p: Trait`), métodos inerentes dentro do bloco `struct`. Validado contra catálogo e fast gate. |
| **03** | `docs/language/` | Tutoriais da linguagem (Tour, Advanced, Interop, Concorrência) em EN/PT | 8 | 1.850 | **done** | Atualizados tutoriais EN e PT (`tour.md`, `tour.pt-BR.md`, `advanced.md`, `advanced.pt-BR.md`, `concurrency.md`, `concurrency.pt-BR.md`): `import path as alias`, `apply Type: Trait` canônico, `array[int, 4]` posicional compacto e `p: Trait` em parâmetros. Validado contra daily_fast. |
| **04** | `docs/guides/` | Guias práticos (CLI, debugging, cookbook, performance, stdlib, testing) EN/PT | 23 | 2.862 | **done** | Atualizados guias EN e PT (`cookbook`, `cli-reference`, `testing`, `errors-null-void`, `first-project`, `stdlib-reference`): migrados imports para `as`, atualizadas receitas de migração e alinhamento com daily_fast. |
| **05** | `docs/planning/` | Backlog ativo, planos de trabalho e histórico intermediário | 70 | 17.335 | **done** | Atualizadas referências à versão canônica de release (0.3.7 -> 0.3.8) em BACKLOG.md, PENDENTES.md e README.md; sincronizados status dos 14 itens concluídos no backlog principal; mantida hierarquia e catalogação de planos ativos vs histórico. |
| **06** | `docs/architecture/`| Arquitetura do compilador, pipeline, crates e runtime | 8 | 1.368 | **done** | Auditada arquitetura canônica; documentado subsistema de arenas de memória `OriRegion` (`mem.region`) e garantias de análise estática de escape em `runtime-and-memory.md`; alinhados invariantes de layout, alignment `@align(N)` e versão canônica 0.3.8. |
| **07** | `docs/quality/` | Estratégia de QA, testes diferenciais, fuzzing e conformidade | 10 | 1.685 | **done** | Auditada suíte e estratégia de qualidade; adicionada documentação operacional dos gates automatizados `daily_fast.sh` e `sanitizer_smoke.sh` (ASan/TSan) em `test-strategy.md`; verificada consistência de conformidade e testes diferenciais. |
| **08** | `docs/operations/` | Desenvolvimento, release e builds reproduzíveis | 4 | 618 | **done** | Auditadas rotas de desenvolvimento, release e builds reproduzíveis; adicionada rota de sanitizers (`sanitizer_smoke.sh` ASan/TSan) em `development.md`; verificados scripts citados (`daily_fast`, `stage_native_runtime`, `package_native_release`, `smoke_native_release`) e pin 0.3.8. |
| **09** | `docs/security/` | Modelo de ameaças, supply chain e política de código unsafe | 4 | 733 | **done** | Auditados threat model, unsafe-code-policy e supply chain; confirmado que `OriRegion` segue boundary pattern (thin `unsafe extern C` + safe domain), single-cascade-owner e análise estática de escape (`using.escape`, não-`Transferable`) conforme política. |
| **10** | `docs/product/` | Status de produto, matriz de suporte, versionamento e acessibilidade | 5 | 465 | **done** | Auditados status, matriz de suporte, versionamento e princípios de acessibilidade; alinhada menção de superfície para S3/0.4 ergonomics e mantido pin 0.3.8 com backends nativo/JIT/C-debug. |
| **11** | `docs/implementation/` | Padrões de implementação e guias para adicionar features/APIs | 7 | 1.328 | `pendente` | Guias de engenharia para desenvolvedores do compilador. |
| **12** | `docs/decisions/` | Registro de ADRs (Architecture Decision Records) | 6 | 676 | `pendente` | Avaliar se decisões das novas waves exigem formalização em novos ADRs numerados. |
| **13** | `docs/governance/` | Governança da linguagem e processo de RFCs | 3 | 391 | `pendente` | Regras institucionais estáveis. |
| **14** | `docs/book/` | Rascunho do livro da linguagem Ori (narrativa em Português) | 36 | 3.938 | `pendente` | Material educacional/livro; verificar se não ensina formas sintáticas antigas. |
| **15** | `docs/archive/` | Arquivo histórico particionado (`audits`, `plans`, `investigations`, `legacy`) | 59 | 14.576 | `pendente` | Manter isolado como registro histórico; consolidar duplicatas óbvias. |
| **16** | `docs/atlas/` & `templates/` & `rfcs/` & `plans/` | Metadados e templates estruturais | 6 | 634 | `pendente` | Schemas de catalogação e modelos de documentos. |
