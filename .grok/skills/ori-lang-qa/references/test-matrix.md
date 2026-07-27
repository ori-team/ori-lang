# Test matrix — generic compiler suite → Ori 0.3.x

> **Product:** Ori AOT native (Cranelift) + optional JIT for `ori run`.  
> **Memory:** ARC + cooperative cycle collection (not a classic tracing GC product).  
> **Surface:** S3 · FREEZE-1 on 0.3.x.

Legend: **Y** = first-class / suite exists or required · **P** = partial / subset · **N** = N/A (do not fake) · **F** = future optional.

| # | Teste (tabela usuário) | Ori | Onde / como |
|---|------------------------|-----|-------------|
| 1 | Léxicos (Lexer) | **Y** | `ori-lexer` unit + `ori_spec` / invalid char |
| 2 | Sintáticos (Parser) | **Y** | `ori-parser` + `parse.*` diagnostics |
| 3 | Semânticos | **Y** | resolve + types (`name.*`, `bind.*`) |
| 4 | Type Checker | **Y** | `ori-types` + `type.*` + inference B |
| 5 | Constant Folding | **P** | opts onde existirem; não suíte dedicada completa |
| 6 | Constant Propagation | **P** | idem |
| 7 | Dead Code Detection | **P** | warnings/unused; não DCE full product |
| 8 | Unreachable Code | **P** | checker parcial / futuros warns |
| 9 | Variáveis Não Utilizadas | **Y** | `bind.unused_*` / warns |
| 10 | Variáveis Não Inicializadas | **Y/P** | init rules; expand se gap real |
| 11 | Shadowing | **Y** | bind/name rules + tests |
| 12 | Ownership | **P** | value + ARC semantics (não move checker Rust) |
| 13 | Borrow Checker | **N** | Ori não tem borrow checker |
| 14 | Lifetime | **N** | sem lifetimes explícitas de ref |
| 15 | Null Safety | **Y** | `optional` / no null; `type.*` |
| 16 | Overflow | **P** | runtime/int rules; documentar |
| 17 | Underflow | **P** | idem |
| 18 | Divisão por Zero | **P** | runtime / diagnostic se houver |
| 19 | Índice Fora dos Limites | **Y/P** | list/index runtime + tests |
| 20 | Stack Overflow | **P** | OS/runtime; stress opcional |
| 21 | Heap Overflow | **P** | allocator/OS; ASAN em CI host opcional |
| 22 | Memory Leak | **Y/P** | ARC + `ORI_TEST_LEAK_CHECK` / cycle collect tests |
| 23 | Double Free | **N/P** | ARC evita free manual; cycle edge cases |
| 24 | Use After Free | **N/P** | managed model; fuzz residual |
| 25 | Garbage Collector | **P** | cycle collector (não GC full generational) |
| 26 | Loops Infinitos | **N/P** | static detect limited; timeout em run tests |
| 27 | Recursão Infinita | **P** | stack / timeout |
| 28 | Complexidade Algorítmica | **F** | analysis tool; não gate 0.3 |
| 29 | Loops Custosos | **F** | lint future |
| 30 | Profundidade de Recursão | **P** | stress |
| 31 | Call Graph | **P** | internal/tools; not product suite |
| 32 | CFG | **P** | IR/HIR tests internal |
| 33 | Data Flow | **P** | checker subsets |
| 34 | Escape Analysis | **P** | codegen decisions undocumented suite |
| 35 | Alias Analysis | **P** | limited |
| 36 | SSA Validation | **P** | backend internal if present |
| 37 | IR integrity | **Y/P** | HIR lower tests |
| 38 | Bytecode Validation | **N** | sem bytecode produto |
| 39 | VM | **N** | AOT-first |
| 40 | Compiler Crash | **Y** | security_robustness + fuzz lite |
| 41 | Fuzzing | **P** | expand `tools/qa`; start small |
| 42 | Differential Testing | **P** | native vs C subset; JIT vs AOT |
| 43 | Snapshot | **P** | diagnostics UPDATE_EXPECT |
| 44 | Golden | **Y** | diagnostic / output goldens |
| 45 | Benchmark | **Y** | `tools/microbench_lang_perf.sh` |
| 46 | Stress | **P** | large files / deep nests |
| 47 | Property-Based | **F** | optional proptest |
| 48 | Mutation | **F** | meta-quality |
| 49 | Conformance | **Y** | `ori_spec` + examples + Spec |
| 50 | Diagnóstico | **Y** | `diagnostic_catalog` + message quality |
| 51 | Coverage | **P** | `cargo llvm-cov` optional CI |
| 52 | Regressão | **Y** | whole driver suite |
| 53 | Performance | **Y** | microbench + performance_guard |
| 54 | Concorrência | **Y** | async suite + task tests |
| 55 | Determinismo | **Y** | same input → same diags/binary behavior |
| 56 | Compatibilidade | **Y** | FREEZE-1 / 0.3.x / migrate-syntax |
| 57 | Serialização | **P** | package/manifest; not full AST dump product |
| 58 | Incremental Compilation | **P** | LSP incremental; not full incr AOT product |
| 59 | Linkagem | **Y** | native link strategies + smoke packages |
| 60 | Importação de Módulos | **Y** | multifile_imports |
| 61 | Segurança | **Y** | security_robustness |
| 62 | Recuperação de Erros | **Y/P** | multi-diagnostic where implemented |

## Daily stage map

| Stage | Covers rows (approx) |
|-------|----------------------|
| S0 | Workspace compile + strict Clippy for runtime/codegen/driver |
| S1 | 1–4 |
| S2 | 3–4, 49–50, 52 |
| S3 | 12, 15, 22, 25, 54 |
| S4 | 59–60 |
| S5 | 40, 52 |
| S6 | 49 |
| S7 | 45, 53 |
| S8 | residuals / product surface |

## Policy

- Prefer **Y/P with real tests** over inventing **N** categories.  
- Expanding **N** into fake checkers is out of FREEZE-1 scope.  
- New **Y** suite needs: harness + ≥1 positive + ≥1 negative when diagnostic.
