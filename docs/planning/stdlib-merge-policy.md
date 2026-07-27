# Política de mesclagem da stdlib (M2 + STDLIB-1)

> **Status:** M2 + **STDLIB-1 fechados** (2026-07-13)  
> **Ordem tática:** M2 → M3 ABI → M1 → (stdlib polish) → M4 self-host  
> Superfície pública canônica: **somente `ori.X`**. Nested `utils`/`algorithms` = compat.

---

## Decisão (resumo)

| ID | Tema | Decisão |
|----|------|---------|
| **M2.D** | Modelo de API | **D** — namespace **público canônico** = `ori.X` (um módulo por domínio) |
| **M2.A** | Layout preferido no disco | **A** — preferir `stdlib/X.orl` = `module ori.X` |
| **M2.B** | Split em pastas | **B** só se necessário: multi-artefato temático (`math/vec2`, …), **não** por “utils” genérico |
| **M2.compat** | `ori.X.utils` / `ori.X.algorithms` | **Compat silenciosa** (ainda compila; **não** ensinar) |
| **M2.examples** | Exemplos e docs de uso | **Só estilo canônico** `ori.X` |
| **STDLIB-1** | Deprecar paths públicos nested | **Feito** — todos os símbolos públicos de `utils`/`algorithms` também no pai |

**Rejeitado:** C puro (sempre forçar `utils`/`algorithms` como API pública).  
**Rejeitado:** inferir tipo de layout só “pelo uso” em docs; a regra acima é normativa de produto.

---

## Camadas (inalteradas)

| Camada | Papel | Onde vive |
|--------|--------|-----------|
| **L1** | Hot path, FFI, runtime Rust | Manifesto `stdlib.rs` + `ori-runtime` |
| **L2** | Wrappers / ergonomia `.orl` | Preferência: mesmo `ori.X` / `stdlib/X.orl` |
| **L3** | Algoritmos puros `.orl` | Preferência: mesmo `ori.X` se couber; senão submódulo interno |

Mesclar **não** significa portar L1 para `.orl`.

**STDLIB-5:** mass L1→`.orl` ports are **closed as wontfix**. Layer 1 Rust
remains the permanent hot-path design (ARC, executor, FS/net FFI).

---

## Regras de import (produto)

### Canônico (ensinar e usar em exemplos)

```ori
import ori.io = io
import ori.fs = fs
import ori.string = str
import ori.path = path
import ori.list = lists

-- seletivo no pai (quando o pai `.orl` expõe o símbolo)
import ori.string (is_empty, truncate = cut)
import ori.fs (read_text_or)
```

### Compatibilidade (deprecada como superfície pública; não remover ainda)

```ori
import ori.fs.utils = fu          -- ainda funciona (compat silenciosa)
import ori.string.utils = su      -- ainda funciona
import ori.queue.utils = qu       -- ainda funciona
import ori.bytes.algorithms = ba  -- ainda funciona
```

**STDLIB-1:** helpers que existiam só em `X/utils.orl` ou `X/algorithms.orl`
também estão em `stdlib/X.orl`. Código novo **só** importa `ori.X`. Suítes de
regressão podem ainda importar nested paths para provar compat.

---

## Regras de layout no disco

1. **Preferir** `stdlib/<name>.orl` com `module ori.<name>`.
2. Duplicação pai ↔ `utils`/`algorithms` é **aceita para compat**; não inventar
   API nova só no nested path. **Não** reexportar L1 no pai com o **mesmo nome**
   (`public to_list` → `ori.queue.to_list` causa sombra/recursão genérica).
3. **Manter pasta** `X/` quando:
   - há vários artefatos irmãos (`math/vec2.orl`, `mat3.orl`);
   - o pai único ficaria ilegível (>~400 linhas) **e** a divisão for por tema,
     não por “utils” genérico.
4. **README/spec** listam a API como `ori.X`, não como árvore de pastas.
5. Submódulos legados `ori.X.utils` / `ori.X.algorithms` = **compat**, não marca.
   Remoção física (breaking) só com janela de freeze / major.

---

## Plano de implementação de código

| Fase | Trabalho | Status |
|------|----------|--------|
| **M2.1–2.6** | Pais `ori.X` + utils/algorithms compat + testes | ✅ |
| **path.relative** | multi-call un-ignored | ✅ |
| **layout** | Cargo em `compiler/`; examples mini-projetos; `_archive/` | ✅ |
| **result-ctors** | `ok`/`err` | ✅ |
| **STDLIB-1** | Lift only-in-utils/algorithms → pais; deprecar paths nested | ✅ |

---

## Critério de pronto M2

- [x] Política + README + spec
- [x] Pais `stdlib/X.orl` para domínios com helpers
- [x] Compat `ori.X.utils` / `algorithms`
- [x] Exemplos como projetos
- [x] Suíte stdlib/flatten/official examples

### Residual S3 1.3 / 2.4 — **fechado**

| Item | Status |
|------|--------|
| Pais `ori.X` com helpers | ✅ |
| Compat `utils` / `algorithms` (módulos legados) | ✅ |
| “Alias silencioso” de **módulos** (import `ori.X.utils` ainda compila) | ✅ |
| **`public alias` de tipos** nos pais (e espelho em utils) | ✅ `fs`/`io`/`net`/`json`/`config` |

### STDLIB-1 — **fechado**

| Item | Status |
|------|--------|
| Zero símbolos *only-in-utils* / *only-in-algorithms* | ✅ (audit 2026-07-13) |
| Docs/README/spec ensinam só `ori.X` | ✅ |
| Nested paths ainda compilam (compat tests) | ✅ |
| Gate de regressão parent-only | ✅ `compile_runs_stdlib_parent_canonical_no_utils_import` |

Aliases de domínio (ex.: `ori.fs.TextResult`, `ori.net.ConnectionResult`) —
não renomear primitivos (`string` → `text` continua fora).

---

## Histórico

| Data | Evento |
|------|--------|
| 2026-07-13 | Recomendação D+A/B aceita; alias silencioso; docs primeiro; exemplos no estilo `ori.X` |
| 2026-07-13 | **M2 fechado** — merge pais + layout monorepo + examples + path.relative |
| 2026-07-13 | Auditoria: type/`public alias` de domínio **não** implementados (residual) |
| 2026-07-13 | Residual fechado: `public alias` em `fs`/`io`/`net`/`json`/`config` (+ utils) |
| 2026-07-13 | **STDLIB-1 fechado** — lift + depreciação pública de `.utils`/`.algorithms` |
