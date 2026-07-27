> **Histórico — não usar como backlog ativo.**  
> Para o roadmap atual, veja [`docs/planning/PLANO-MATURIDADE-COMPLETO.md`](planning/PLANO-MATURIDADE-COMPLETO.md).

# Plano de Correção — Bugs Remanescentes da Implementação Ori

Data: 2026-05-17
Fonte: Auditoria profunda de 2026-05-17 (`_reversa_sdd/auditoria-profunda-implementacao-2026-05-17.md`)
Status: Plano de ação

---

## Visão Geral

Dos ~14 bugs críticos/altos identificados nas auditorias de 12-13 de Maio de 2026,
**11 já foram corrigidos**. Restam 3 bugs P0/P1 e 5 dívidas técnicas P2.

Este plano cobre:
- **Fase 1:** Correção dos bugs P0/P1 (3 bugs)
- **Fase 2:** Dívidas técnicas P2 (5 itens)
- **Fase 3:** Consolidação documental e changelog

---

## Fase 1 — Bugs Críticos e Altos (P0/P1)

### Bug 1: Campos e variantes duplicados em struct/enum não diagnosticados

**Severidade:** P0 (bloqueia uso correto)
**Arquivos afetados:** `compiler/crates/ori-types/src/resolve.rs`, `check.rs`
**Descrição:** `struct S { x: int; x: int }` e `enum E { A; A }` passam sem erro
**Impacto:** Construtores, field lookup e pattern matching ficam ambíguos

**Plano de correção:**
1. Adicionar validação no `resolve.rs` durante a construção de `StructSig.fields`
   - Usar `HashSet<SmolStr>` para rastrear nomes já vistos
   - Emitir `name.duplicate_field` com span do campo duplicado
2. Adicionar validação similar para `EnumSig.variants`
   - Emitir `name.duplicate_variant`
3. Adicionar validação no checker para campos de variantes de enum com payload
   - Campos nomeados dentro de uma mesma variante não podem repetir
4. Adicionar testes de regressão em `ori_spec.rs`

**Estimativa:** 2-3 horas (60-80 linhas de código + testes)
**Arquivos a modificar:**
- `compiler/crates/ori-types/src/resolve.rs` (~30 linhas)
- `compiler/crates/ori-types/src/check.rs` (~20 linhas)
- `compiler/crates/ori-driver/tests/ori_spec.rs` (~40 linhas de teste)
- `docs/spec/13-error-catalog.md` (~5 linhas)

---

### Bug 2: `ori.iter` passa no checker mas falha no codegen nativo

**Severidade:** P1 (experiência quebrada)
**Arquivos afetados:** `stdlib.rs`, `lower.rs`, `native_backend.rs`
**Descrição:** `import ori.iter as iter; iter.map(xs, do(x) => x * 2)` passa no
`ori check` mas `ori compile` falha com "missing function reference"
**Impacto:** Usuário confia no checker, escreve código, descobre erro apenas no link

**Plano de correção (3 opções):**

**Opção A (Recomendada) — Bloquear no checker:**
1. Adicionar campo `native_runtime: bool` nas entradas da stdlib
2. No checker, ao resolver `import ori.iter`, verificar `native_runtime`
3. Se `native_runtime == false` e não for C backend, emitir
   `bind.stdlib_module_unavailable`
4. Documentar que `ori.iter` é apenas C backend no momento

**Opção B — Implementar runtime nativo:**
1. Adicionar símbolos `ori_iter_map`, `ori_iter_filter`, etc. no `ori-runtime`
2. Registrar no `STDLIB_RUNTIME_FUNCTIONS` com `native_runtime: true`
3. Implementar no runtime Rust e expor via `extern "C"`

**Opção C — Redirecionar para `ori.list.*`:**
1. No HIR lowering, mapear `ori.iter.map` → `ori.list.map` (já implementado)
2. Garantir que o mapeamento funcione para todas as funções de `ori.iter`

**Recomendação:** Opção A (mais rápida e segura), seguida da Opção C para
funções já implementadas, e Opção B no longo prazo.

**Estimativa:** 2-4 horas
**Arquivos a modificar:**
- `compiler/crates/ori-types/src/stdlib.rs` (atualizar flags)
- `compiler/crates/ori-types/src/check.rs` (validação de import)
- `compiler/crates/ori-driver/tests/ori_spec.rs` (testes)

---

### Bug 3: Divergência `math.floor/ceil/round` spec vs implementação

**Severidade:** P1 (contrato público quebrado)
**Arquivos afetados:** `stdlib.rs`, `lower.rs`, `docs/spec/12-stdlib.md`
**Descrição:** Spec diz `math.floor(x: float) -> float`, implementação retorna `int`
**Impacto:** `const x: float = math.floor(1.5)` gera `type.type_mismatch`

**Plano de correção:**
1. Alinhar spec com implementação (manter `-> int` como comportamento correto)
   - `floor`, `ceil`, `round` semanticamente retornam inteiros
2. Atualizar `docs/spec/12-stdlib.md` para refletir `-> int`
3. Atualizar `docs/spec/05-expressions.md` se houver exemplos divergentes
4. Adicionar nota sobre conversão explícita: `float(math.floor(x))`
5. Se no futuro houver overload, adicionar `math.floor_float`, `math.ceil_float`

**Alternativa:** Mudar a implementação para retornar `float` (mais trabalho,
  runtime precisa ser alterado)

**Estimativa:** 30 minutos (apenas documentação)
**Arquivos a modificar:**
- `docs/spec/12-stdlib.md`

---

## Fase 2 — Dívidas Técnicas (P2)

### DT 1: Três fontes de verdade para runtime

**Descrição:** Runtime Rust (`ori-runtime`), runtime C embutido (`pipeline.rs`),
runtime C inline (`c_backend.rs`) precisam ser mantidos em sincronia.

**Plano:**
1. [x] Já existe manifesto central em `stdlib.rs`
2. [ ] Adicionar teste automatizado que compara:
   - Funções declaradas no manifesto `STDLIB_RUNTIME_FUNCTIONS`
   - Funções exportadas pelo `ori-runtime` (`#[no_mangle] pub extern "C"`)
   - Funções referenciadas no `native_backend.rs`
   - Funções no `stdlib_c_name()` do `lower.rs`
3. [ ] Remover duplicatas: funções que aparecem tanto no `stdlib.rs` quanto
   hardcoded no `lower.rs` devem ser unificadas
4. [ ] Script `tools/check_native_runtime_exports.ps1` (se existir) deve ser
   estendido para Linux/macOS

**Estimativa:** 8-16 horas

---

### DT 2: `Ty::Infer(0)` permite tipos errados na stdlib

**Descrição:** Assinaturas de stdlib usam `Ty::Infer(0)` que não é unificado
corretamente com o tipo da coleção.

**Plano:**
1. Identificar todas as funções que usam `Ty::Infer(0)` na stdlib
2. Para cada uma, adicionar validação explícita no `infer_stdlib_call()`
3. Verificar se o tipo do argumento casa com o tipo do elemento da coleção
4. Alternativa de longo prazo: implementar genéricos reais `Ty::Param` na stdlib

**Funções afetadas:** `list.contains`, `list.index_of`, `list.push`, `list.insert`,
`list.set`, `map.contains`, `map.set`, `map.get`, `set.contains`, `set.add`

**Estimativa:** 4-8 horas

---

### DT 3: Catálogo de diagnósticos incompleto

**Descrição:** Vários códigos de erro emitidos não aparecem em
`docs/spec/13-error-catalog.md`.

**Plano:**
1. Extrair todos os códigos de diagnóstico do código fonte
2. Comparar com o catálogo atual
3. Adicionar entradas faltantes
4. Criar script de CI que falha se um código novo não está no catálogo

**Estimativa:** 2-4 horas

---

### DT 4: `Cargo.lock` quebrado

**Descrição:** O lock file versão 4 requer flag nightly.

**Plano:**
1. Rodar `cargo update` com toolchain estável para regenerar lock file v3
2. Ou: usar `cargo +nightly update -Znext-lockfile-bump`
3. Verificar se o toolchain estável atual suporta lock file v4

**Estimativa:** 15 minutos

---

### DT 5: `check_unclosed_block_comments()` no-op

**Descrição:** Função marcada `#[allow(dead_code)]` que não faz nada.

**Plano:**
1. Remover a função ou implementá-la
2. Se remover, limpar referências
3. Se implementar, mover a lógica de `find_unclosed_block_comment` para dentro
   dela e fazer o lexer chamar ambas

**Estimativa:** 30 minutos

---

## Fase 3 — Consolidação Documental

### Documentos para merge

| Documentos | Escopo | Ação |
|---|---|---|
| `docs/planning/IMPLEMENTATION_CHECKLIST.md` | Checklist principal | Manter como fonte única |
| `docs/planning/IMPLEMENTATION_CHECKLIST_2.md` | Rota nativa 100% | Merge no principal OU manter como backlog separado |
| `docs/planning/native-route.md` | Contrato native route | Já está bem separado, manter |
| `docs/planning/native-abi.md` | ABI document | Já está bem separado, manter |
| `_reversa_sdd/plano-correcao-implementacao-linguagem.md` | Plano antigo | Arquivar — substituído por este plano |
| `docs/plano-correcao-implementacao-linguagem.md` | Cópia do plano | Arquivar — substituído por este plano |
| `_reversa_sdd/analise-profunda-implementacao-linguagem.md` | Auditoria 12 Mai | Arquivar como histórico |
| `_reversa_sdd/auditoria-profunda-implementacao-linguagem-2026-05-13.md` | Auditoria 13 Mai | Arquivar como histórico |
| `_reversa_sdd/relatorio-fechamento-correcao-implementacao-linguagem.md` | Fechamento | Arquivar como histórico |
| `_reversa_sdd/relatorio-fechamento-nova-rodada.md` | Fechamento | Arquivar como histórico |
| `docs/planning/analise-completa-implementacao-linguagem-ori.md` | Análise antiga | Arquivar como histórico |
| `docs/planning/analise-limitacoes-collections-ori.md` | Coleções | Já coberto pelo checklist 2, arquivar |
| `docs/planning/lacunas-reais-plano-correcao.md` | Lacunas antigas | Arquivar — bugs já corrigidos |
| `docs/planning/walkthrough-correcoes.md` | Walkthrough | Atualizar com estado atual ou merge no README |
| `docs/planning/analysis_results.md` | Resultados antigos | Arquivar |
| `_reversa_sdd/auditoria-profunda-implementacao-2026-05-17.md` | Auditoria atual | Manter como referência mais recente |

### Novo mapa documental proposto

```
docs/
  CHANGELOG.md                           # NOVO — histórico de mudanças
  spec/                                  # Especificação normativa (inalterado)
    01-overview.md ... 16-runtime-ffi-safety.md
  planning/
    IMPLEMENTATION_CHECKLIST.md          # Mantido — checklist principal
    IMPLEMENTATION_CHECKLIST_2.md        # Mantido — backlog rota nativa 100%
    native-route.md                      # Mantido — contrato native route
    native-abi.md                        # Mantido — ABI document
    native-hir-coverage.md               # Mantido — cobertura HIR
    native-runtime-route-correction-plan.md  # Mantido — plano de correção
    native-async-state-machine-design.md # Mantido — design async
    async-implementation-plan.md         # Mantido — plano async
    ARC_IMPLEMENTATION_PLAN.md           # Mantido — plano ARC
    README.md                            # Mantido — índice do planning
  plano-correcao-bugs-2026-05-17.md      # NOVO — este plano
  plano-implementacao-lsp-avancado.md    # NOVO — plano do LSP

_reversa_sdd/                            # Arquivo histórico (não mexer)
  auditoria-profunda-implementacao-2026-05-17.md  # Auditoria mais recente
  auditoria-profunda-implementacao-linguagem-2026-05-13.md
  analise-profunda-implementacao-linguagem.md
  plano-correcao-implementacao-linguagem.md
  relatorio-fechamento-*.md
```

---

## Ordem de Execução Recomendada

1. **DT 4:** Corrigir `Cargo.lock` (15 min) — ✅ Parcial: formato downgradado de v4 para v3. Rust 1.75.0 é incompatível com crates que requerem edition2024 (`cranelift-bitset 0.131.1`, `idna_adapter 1.2.2`). Necessário Rust >= 1.78.
2. **Bug 1:** Duplicate fields/variants (2-3h) — P0
3. **Bug 2:** `ori.iter` checker gate (2-4h) — P1
4. **Bug 3:** `math.floor` spec alignment (30 min) — P1
5. **DT 5:** Remover no-op function (30 min)
6. **Fase 3:** Consolidação documental (2h)
7. **DT 3:** Completar catálogo de diagnósticos (2-4h)
8. **DT 1:** Teste de consistência de runtime (8-16h)
9. **DT 2:** Corrigir `Ty::Infer(0)` na stdlib (4-8h)

**Tempo total estimado:** 21-38 horas de trabalho

---

## Critérios de Aceitação

Após cada fase:
- [ ] `cargo check --workspace` passa
- [ ] `cargo test --workspace` passa (incluindo novos testes de regressão)
- [ ] Novos testes cobrem cada bug corrigido
- [ ] Documentação atualizada (spec, catálogo de erros, changelog)
- [ ] Skill `ori-testing` executada para cada correção
