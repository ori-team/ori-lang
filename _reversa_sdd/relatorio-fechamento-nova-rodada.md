# Relatorio de fechamento da nova rodada

Este relatorio fecha a rodada iniciada em
`_reversa_sdd/analise-profunda-implementacao-linguagem.md` e executada em
`_reversa_sdd/plano-correcao-implementacao-linguagem.md`.

## Estado final

- P11: literais numericos agora preservam valor e sufixo, com diagnostico para
  sufixo invalido e overflow.
- P12: igualdade e colecoes hash foram alinhadas entre checker, backend e
  documentacao. Igualdade estrutural planejada e rejeitada cedo quando ainda
  nao ha suporte.
- P13: controle de fluxo de loop agora rejeita `break` e `continue` fora de
  loop e dentro de closure que atravessa escopo.
- P14: stdlib e backend C ficaram mais explicitos. APIs planejadas falham cedo
  com diagnostico claro.
- P15: lexer, comentarios de documentacao, atributos, LSP e docs oficiais
  foram alinhados com o estado real da implementacao.
- P16: paths, aliases e simbolos de runtime da stdlib passaram a ter manifesto
  central e testes de consistencia entre checker, HIR, codegen e runtime.
- P17: especificacao e checklist foram revisados para separar implementado,
  parcial e planejado.
- P18: reproducoes e gates finais passaram.

## Reproducoes confirmadas

| Caso | Resultado |
| --- | --- |
| Literal com sufixo nao vira zero | Coberto por `check_accepts_numeric_literal_suffixes` e `build_preserves_numeric_literal_suffix_values`. |
| Overflow nao vira zero | Coberto por `check_reports_numeric_literal_overflow`. |
| Igualdade estrutural de struct | Rejeitada cedo ate suporte estrutural existir. Coberto por `check_blocks_structural_equality_until_supported`. |
| `func == func` | Rejeitado no checker. Coberto por `check_reports_function_value_equality`. |
| Igualdade com `any<Trait>` | Rejeitada no checker. Coberto por `check_reports_any_trait_equality`. |
| `map` com chave fora de `int`/`string` | Rejeitado cedo com `type.collection_hash_unsupported`; `map<string, int>` agora usa igualdade textual. |
| `set` com elemento fora de `int`/`string` | Rejeitado cedo com `type.collection_hash_unsupported`; `set<string>` agora usa igualdade textual. |
| Literais de `map`/`set` heterogeneos | Rejeitados com `type.map_value_mismatch` e `type.set_element_mismatch`. |
| `break` fora de loop | Rejeitado no checker. Coberto por `check_reports_loop_control_outside_loop`. |
| Controle de loop dentro de closure | Rejeitado no checker. Coberto por `check_reports_loop_control_inside_closure`. |
| Arquivo com BOM inicial | Aceito pelo lexer mantendo spans corretos. Coberto por teste de lexer. |
| BOM fora do inicio | Rejeitado com diagnostico claro. Coberto por teste de lexer. |
| `ori.iter` planejado | Falha cedo com `bind.stdlib_module_unavailable`. |
| `math.floor` | Alinhado com a spec atualizada e validado por testes de stdlib matematica. |
| `?` no backend C | Rejeitado cedo com erro de backend claro. |

## Comandos executados

- `cargo test -p ori-driver numeric_literal --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver equality --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver check_rejects_non_int_map_and_set_hash_inputs --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver check_reports_map_set_literal_element_mismatches --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver loop_control --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver planned_stdlib_import --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver more_math_stdlib --test multifile_imports -- --nocapture`
- `cargo test -p ori-driver c_backend_unsupported_propagation --test multifile_imports -- --nocapture`
- `cargo test -p ori-lexer utf8_bom --lib -- --nocapture`
- `cargo test -p ori-driver diagnostic_catalog_matches_emitted_codes --test diagnostic_catalog -- --nocapture`
- `cargo test -p ori-driver inert --test multifile_imports -- --nocapture`
- `cargo test -p ori-types manifest -- --nocapture`
- `cargo test -p ori-hir stdlib_manifest_paths_lower_to_declared_runtime_symbols -- --nocapture`
- `cargo test -p ori-codegen manifest -- --nocapture`
- `cargo test -p ori-runtime rust_runtime_exports_manifest_native_symbols -- --nocapture`
- `cargo test -p ori-driver embedded_c_runtime_exports_manifest_native_symbols --lib -- --nocapture`
- `cargo fmt --check`
- `git diff --check`
- `cargo check --workspace`
- `cargo test --workspace`

Todos passaram. `git diff --check` emitiu apenas avisos de normalizacao LF/CRLF
do Git, sem erro de whitespace.

Nota de fechamento: `map<string, int>` e `set<string>` foram promovidos de casos
problemáticos para casos cobertos. O checker, a spec, o runtime nativo e o
runtime C embedado agora estao alinhados para aceitar `int` e `string` em
`map`/`set`; a reexecucao final de `cargo test --workspace` passou.

## Pendencias futuras explicitas

Estas pendencias nao bloqueiam a rodada porque agora estao documentadas como
planejadas, parciais ou indisponiveis com diagnostico claro.

Atualizacao de continuidade: os overloads `float` de `math.abs`, `math.min` e
`math.max` foram implementados depois deste fechamento inicial. Eles agora
passam no checker, no backend nativo e no backend C.

Atualizacao de continuidade: a validacao de atributos tambem foi implementada.
`@test`, `@inline`, `@no_inline`, `@deprecated` e `@cfg` agora validam nome,
alvo, duplicidade e formato de argumentos. Uso de declaracao `@deprecated`
agora emite `attr.deprecated`. Acoes futuras continuam restritas a `ori test`
e `ori doc`.

- Implementar igualdade estrutural para colecoes, tuplas, structs e tipos
  relacionados quando a semantica for fechada.
- Implementar comentarios de documentacao.
- Implementar `ori test` e modulo `ori.test`.
- Implementar `ori doc` e diagnosticos `doc.*`.
- Substituir o placeholder de `ori-lsp` por servidor LSP real.
- Evoluir o manifesto de stdlib para tambem gerar tipos de ABI, nao apenas
  validar paths, aliases, simbolos e disponibilidade por backend.
- Completar ownership do backend C standalone; hoje ele ainda usa hooks ARC
  inline de placeholder.

## Conclusao

Todos os achados da analise profunda agora estao em uma das tres situacoes:

- corrigido com teste;
- documentado como planejado ou parcial;
- bloqueado cedo com diagnostico claro.

O codigo, os testes e a documentacao contam a mesma historia nesta rodada.
