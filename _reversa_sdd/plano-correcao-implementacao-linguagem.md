# Plano de correcao da implementacao da linguagem Ori

Data: 2026-05-12

Origem: auditoria manual da implementacao, cruzamento com `docs/spec/*` e validacao com `cargo`.

Escopo: bugs, falhas de implementacao e divergencias entre codigo, testes e documentacao.

## Estado base observado

- [x] `cargo check --workspace` passa, agora sem warnings do workspace.
- [x] `cargo test --workspace` passa apos as correcoes de P0 e P1.
- [x] A maior falha anterior nos testes de execucao nativa foi resolvida.
- [x] A documentacao e o checklist afirmavam suporte mais amplo do que o codigo realmente entrega; isso foi revisado nas etapas P2, P7 e P8.
- [x] O worktree ja estava sujo antes desta auditoria; as mudancas existentes foram preservadas e nao foram revertidas.

## Regra de execucao do plano

- Corrigir por prioridade, sem pular bloqueadores.
- Em cada etapa, atualizar codigo, testes e documentacao juntos.
- Se uma feature continuar parcial, a documentacao deve dizer isso com clareza.
- Nao marcar item como concluido apenas porque compila.
- Cada etapa precisa ter pelo menos um teste ou uma verificacao objetiva.

---

## P0 - Desbloquear execucao nativa

Objetivo: fazer os testes nativos voltarem a executar de forma confiavel.

- [x] Corrigir a ordem/declaracao de `ori_new_result` no runtime C embutido em `compiler/crates/ori-driver/src/pipeline.rs`.
- [x] Garantir que `ori_bytes_from_hex` e `ori_bytes_decode_utf8` compilem sem declaracao implicita em C.
- [x] Parar de mascarar erro de compilacao do runtime C em `build_runtime_lib()`.
- [x] Quando `cc` existir e a compilacao do runtime falhar, retornar erro real e legivel.
- [x] Separar claramente estes casos:
  - [x] `cc` nao esta disponivel.
  - [x] `cc` esta disponivel, mas o runtime C esta invalido.
  - [x] o link final falhou por simbolo ausente.
- [x] Remover comentario/placeholder dentro do runtime C embutido: `skipped intermediate for space`.
- [x] Rodar teste focado: `cargo test -p ori-driver compile_runs_entry_namespace_main_with_imported_call -- --nocapture --test-threads=1`.
- [x] Rodar todos os testes do driver: `cargo test -p ori-driver`.
- [x] Rodar suite completa: `cargo test --workspace`.
- [x] Corrigir falha nativa restante de slicing por referencia ausente a `__slice`.

Critério de aceite:

- [x] Falhas de runtime C aparecem como erro direto, nao como erro tardio de link.
- [x] Os testes nativos deixam de falhar por simbolos ausentes do runtime.

---

## P1 - Limpar ruido de debug e warnings

Objetivo: deixar diagnosticos e testes legiveis.

- [x] Remover `eprintln!` de debug em `compiler/crates/ori-hir/src/lower.rs`.
- [x] Remover ou substituir debug solto por logging controlado, se necessario.
- [x] Corrigir warnings atuais do workspace:
  - [x] `ori-parser`: `eat_span` nao usado.
  - [x] `ori-hir`: `err_expr` nao usado.
  - [x] `ori-codegen`: `mut` desnecessario.
  - [x] `ori-codegen`: codigo morto em `c_backend`.
- [x] Rodar `cargo check --workspace`.
- [x] Rodar `cargo test --workspace` e confirmar que a saida nao fica poluida por debug manual.

Critério de aceite:

- [x] `cargo check --workspace` passa sem warnings novos.
- [x] A saida dos testes mostra apenas diagnosticos intencionais.

---

## P2 - Alinhar stdlib com a spec

Objetivo: eliminar contradicoes entre `docs/spec/12-stdlib.md`, checker, HIR, runtime e testes.

- [x] Decidir o nome canonico do modulo de arquivos:
  - [x] `ori.fs`, como esta na spec.
  - [x] `ori.files` nao foi escolhido como canonico; ficou como alias de compatibilidade documentado.
- [x] Atualizar codigo, docs e testes para usar o nome canonico. `ori.files` ficou documentado como alias de compatibilidade.
- [x] Revisar `io.print` e `io.eprint`:
  - [x] A alternativa `result<void, ori.Error>` nao foi escolhida para esta etapa.
  - [x] Se a implementacao estiver correta, ajustar a spec para retorno `void`.
- [x] Revisar `bytes.slice`:
  - [x] Spec anterior: `bytes.slice(b: bytes, range: range<int>) -> bytes`.
  - [x] Implementacao atual: `bytes.slice(bytes, int, int)`.
  - [x] Escolher uma forma e alinhar docs, checker, HIR e testes.
- [x] Revisar `string.from_bytes`:
  - [x] Confirmar se e funcao de modulo ou metodo.
  - [x] Corrigir testes que chamam `from_bytes` no receiver errado.
- [x] Atualizar `docs/IMPLEMENTATION_CHECKLIST.md` para refletir o estado real de `ori.bytes`.
- [x] Adicionar testes de sucesso e erro para as funcoes afetadas nesta etapa.

Critério de aceite:

- [x] Um exemplo da documentacao da stdlib compila e executa.
- [x] Nao ha dois nomes publicos concorrentes para a mesma feature sem documentacao explicita de compatibilidade.

---

## P3 - Corrigir lexer, strings, bytes e f-strings

Objetivo: fazer o comportamento lexical cumprir a spec publica.

- [x] Implementar escapes de string:
  - [x] `\n`
  - [x] `\t`
  - [x] `\"`
  - [x] `\\`
  - [x] `\u{XXXX}`
- [x] Implementar escapes de bytes:
  - [x] `\xNN`
  - [x] escapes basicos permitidos pela spec.
- [x] Garantir que `b"\xFF"` gere o byte `255`, nao os caracteres ASCII `\`, `x`, `F`, `F`.
- [x] Implementar parsing real de f-strings:
  - [x] separar partes literais.
  - [x] separar expressoes `{expr}`.
  - [x] suportar escape de chaves se a spec permitir.
  - [x] gerar diagnostico claro para interpolacao malformada.
- [x] Corrigir suporte a identificadores Unicode ou ajustar a spec.
- [x] Rever palavras reservadas e contextuais:
  - [x] `times` deve ser contextual se a spec continuar dizendo isso.
  - [x] `using`, `check`, `with`, `then`, `tuple`, `lazy` devem estar documentadas se forem reservadas.
- [x] Implementar parsing basico de strings multilinha `"""..."""` e `f"""..."""`, ja documentadas na spec.
- [x] Adicionar testes de lexer/parser para todos os casos acima.

Critério de aceite:

- [x] Exemplos de strings, bytes, f-strings e identificadores da spec funcionam.
- [x] Casos invalidos recebem erro claro e localizado.

Validacao executada:

- [x] `cargo test -p ori-driver compile_runs_escaped_literals_and_fstrings -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver compile_runs_unicode_identifier_and_contextual_times -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_reports_invalid_escapes_and_fstring_diagnostics -- --nocapture --test-threads=1`

---

## P4 - Corrigir parser contra a gramatica

Objetivo: fazer a sintaxe aceita pelo parser bater com `docs/spec/03-grammar.ebnf`.

- [x] Corrigir ordem de parametro com default e contract:
  - [x] Spec atual: `nome: tipo = valor if contrato`.
  - [x] Parser atual le `if contrato` antes de `= valor`.
- [x] Proibir comparacao encadeada no parser:
  - [x] `a < b < c` deve gerar `parse.chained_comparison`.
- [x] Implementar ou remover da spec anonymous struct literal:
  - [x] `.{ field: value }`.
- [x] Implementar ou corrigir a sintaxe documentada de tuple:
  - [x] `tuple(1, "one")`.
  - [x] ou documentar apenas `(1, "one")`.
- [x] Corrigir `where` clauses:
  - [x] suportar `and`, se a spec continuar assim.
  - [x] suportar grupos parenteticos, se a spec continuar assim.
  - [x] suportar constraints genericas conforme exemplos de `docs/spec/11-generics.md`.
- [x] Adicionar testes negativos para sintaxe rejeitada.
- [x] Adicionar testes positivos para cada exemplo oficial da gramatica.

Critério de aceite:

- [x] O parser aceita o que a spec aceita nos casos auditados desta etapa.
- [x] O parser rejeita com diagnostico proprio o que a spec proibe nos casos auditados desta etapa.

Validacao executada:

- [x] `cargo test -p ori-driver compile_runs_p4_grammar_forms_native -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_reports_chained_comparison -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_accepts_grouped_where_clause_with_and -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver`

---

## P5 - Corrigir regras semanticas obrigatorias

Objetivo: fazer o type checker aplicar as regras de linguagem descritas em `docs/spec/06-statements.md`, `docs/spec/07-functions.md`, `docs/spec/10-memory.md` e `docs/spec/13-error-catalog.md`.

- [x] Registrar mutabilidade no escopo:
  - [x] diferenciar `const` de `var`.
  - [x] impedir atribuicao em `const`.
  - [x] impedir chamada de metodo `mut` sobre binding imutavel.
- [x] Detectar shadowing no mesmo escopo.
- [x] Emitir `bind.shadowing` quando aplicavel.
- [x] Implementar checagem de retorno obrigatorio:
  - [x] funcao nao-void nao pode terminar sem `return`.
  - [x] blocos condicionais precisam ser analisados por caminho.
- [x] Verificar `using`:
  - [x] valor usado em `using` precisa implementar `Disposable`.
  - [x] emitir `using.not_disposable` quando falhar.
- [x] Garantir que `emit_dispose_call` nao silencie ausencia de dispose obrigatorio.
- [x] Adicionar testes para cada erro semantico.

Critério de aceite:

- [x] Programas invalidos pela spec falham no checker, nao no backend.
- [x] Os erros usam codigos do catalogo quando existirem.

Validacao executada:

- [x] `cargo test -p ori-driver check_reports_const_reassignment_and_same_scope_shadowing -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_reports_missing_return_on_non_void_function -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_using_stmt_type_checks -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_reports_using_binding_reassignment -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver check_reports_mut_method_call_on_const_binding -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver`

---

## P6 - Remover placeholders perigosos do codegen

Objetivo: impedir que o backend gere codigo silenciosamente errado.

- [x] Corrigir `is` no C backend:
  - [x] nao pode retornar `true` como placeholder.
  - [x] se nao houver suporte completo, emitir erro de compilacao claro.
- [x] Corrigir fallback de index nao suportado:
  - [x] nao gerar `0/*unsupported-idx*/`.
  - [x] emitir erro antes de gerar C invalido ou semanticamente errado.
- [x] Corrigir fallback de `for` sobre iteravel nao suportado:
  - [x] nao emitir comentario C como substituto de comportamento.
  - [x] emitir diagnostico ou implementar o caso.
- [x] Remover `unwrap()` em caminhos de codegen que podem receber entrada invalida.
- [x] Trocar panics por `Result` com mensagem util.
- [x] Adicionar testes que garantem que unsupported features falham cedo.

Critério de aceite:

- [x] O codegen nao produz codigo incorreto em silencio.
- [x] Erros de backend sao legiveis e apontam a feature sem suporte.

Validacao executada:

- [x] `cargo test -p ori-codegen`
- [x] `cargo test -p ori-driver build_reports_c_backend_unsupported_is_check -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver`

---

## P7 - Resolver ARC, Disposable e memoria

Objetivo: alinhar implementacao real, runtime e documentos sobre gerenciamento de memoria.

- [x] Decidir o escopo real do ARC para esta versao:
  - [x] ARC basico sem cycle collector.
  - [x] ARC com cycle collector funcional nao foi escolhido para esta etapa; ficou documentado como pendencia futura.
- [x] Atualizar `docs/ARC_IMPLEMENTATION_PLAN.md` para remover contradicoes internas.
- [x] Atualizar `docs/IMPLEMENTATION_CHECKLIST.md`:
  - [x] nao marcar cycle detection como completo se for stub.
- [x] Alinhar runtime Rust, runtime C embutido e C backend.
- [x] Revisar diferenca entre refcount atomico no runtime Rust e refcount nao atomico no runtime C.
- [x] Criar testes de:
  - [x] retain.
  - [x] release.
  - [x] destroy callback.
  - [x] Disposable.
  - [x] ciclos, se cycle collector for mantido como feature.

Critério de aceite:

- [x] Docs dizem exatamente o que o runtime faz.
- [x] Nenhuma feature de memoria parcial aparece como completa.

Validacao executada:

- [x] `cargo test -p ori-runtime arc_ -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver compile_runs_using_dispose_on_native_scope_exit -- --nocapture --test-threads=1`
- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace`

---

## P8 - Revisar checklist, testes de exemplos e docs publicas

Objetivo: fazer a documentacao voltar a ser fonte confiavel.

- [x] Revisar `docs/IMPLEMENTATION_CHECKLIST.md` item por item.
- [x] Trocar `[x]` por estado parcial quando houver placeholder.
- [x] Atualizar `tests/README.md`, que ainda fala como se os testes ainda fossem futuros.
- [x] Validar exemplos da spec como fixtures.
- [x] Criar uma lista de exemplos oficiais que precisam sempre compilar.
- [x] Adicionar teste automatizado que roda exemplos copiados da documentacao, quando viavel.
- [x] Separar claramente:
  - [x] implementado.
  - [x] implementado parcialmente.
  - [x] planejado.
  - [x] removido ou adiado.

Critério de aceite:

- [x] Uma pessoa nova consegue confiar na doc sem ler o codigo-fonte.
- [x] O checklist nao exagera o estado real da linguagem.

Validacao executada:

- [x] `cargo test -p ori-driver check_official_examples -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver`
- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace`

---

## P9 - Fechamento e gate final

Objetivo: confirmar que a linguagem voltou a ter uma base coerente.

- [x] Rodar `cargo fmt --check`.
- [x] Rodar `cargo check --workspace`.
- [x] Rodar `cargo test --workspace`.
- [x] Rodar testes de exemplos/documentacao, se criados.
- [x] Revisar se ainda existem `TODO`, `placeholder`, `unsupported`, `unwrap()` e debug solto em caminhos criticos.
- [x] Atualizar este plano marcando o que foi concluido.
- [x] Criar novo relatorio curto com:
  - [x] o que foi corrigido.
  - [x] o que ficou pendente.
  - [x] quais comandos passaram.
  - [x] quais comandos ainda falharam.

Critério de aceite:

- [x] Build e testes principais passam.
- [x] Docs, checklist e comportamento observado nao se contradizem nos pontos auditados.

Validacao executada:

- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace`
- [x] `rg -n "TrapCode::user\\([^\\)]*\\)\\.unwrap\\(|unsupported-idx|placeholder true|skipped intermediate|dbg!\\(" compiler/crates/ori-codegen/src compiler/crates/ori-driver/src compiler/crates/ori-runtime/src docs`

---

## P10 - Pendencias reais restantes de `docs/analysis_results.md`

Objetivo: transformar o relatorio antigo `docs/analysis_results.md` em acoes atuais, removendo itens obsoletos e corrigindo os pontos que ainda sao validos.

Origem: revisao de `docs/analysis_results.md` apos P0-P9.

Itens do relatorio ja considerados obsoletos:

- [x] `BytesLit` no backend nativo: ja existe codegen e teste E2E.
- [x] Indice de tupla fora do limite: ja existe diagnostico no checker.
- [x] Constantes globais comuns: ja existem testes de execucao para `const` global e importada.

Pendencias reais:

- [x] Proibir `==` e `!=` em `any<Trait>` no checker.
  - [x] Emitir `type.comparison_not_supported`, conforme `docs/spec/13-error-catalog.md`.
  - [x] Adicionar teste negativo para `const same: bool = a == b` quando `a` e `b` sao `any<Trait>`.
  - [x] Confirmar que `is` em `any<Trait>` continua funcionando no backend nativo.
- [x] Resolver o estado de `lazy<T>`.
  - [x] Escolher uma decisao explicita: implementar agora ou marcar como planejado.
  - [x] Runtime/checker/codegen para `lazy.once` e `lazy.force` implementados.
    - Decisao atual: `lazy<T>` e feature pronta, com avaliacao no maximo uma vez.
  - [x] Atualizar `docs/spec/04-types.md`, `docs/spec/12-stdlib.md` e `docs/IMPLEMENTATION_CHECKLIST.md`.
  - [x] Adicionar testes de checker, backend C e backend native que garantam a decisao escolhida.
- [x] Tratar o caminho residual de `HirExprKind::GlobalConst` no backend nativo.
  - [x] Verificar se o lowering ainda pode emitir esse node em algum caso real.
  - [x] Se for caminho morto: remover ou substituir por erro interno mais claro, com teste/checagem que comprove a ausencia.
  - [x] Nao aplicavel: nao ha mais emissao de `HirExprKind::GlobalConst`.
    - Decisao: caminho morto removido do HIR e dos backends.
- [x] Reduzir acoplamento direto com `strlen` no backend nativo.
  - [x] Preferir helper do runtime Ori, como `ori_string_len`, quando o valor ja for string Ori.
  - [x] Manter `strlen` apenas para ponteiros C realmente null-terminated, se necessario.
  - [x] Adicionar teste de compilacao que cubra string length sem depender de fallback ausente.
- [x] Corrigir ou arquivar `docs/analysis_results.md`.
  - [x] Resolver mojibake/encoding do arquivo.
  - [x] Marcar achados obsoletos como resolvidos ou mover o conteudo para `_reversa_sdd/`.
  - [x] Evitar que ele contradiga o plano atual.

Critério de aceite:

- [x] `cargo fmt --check` passa.
- [x] `cargo check --workspace` passa.
- [x] `cargo test --workspace` passa.
- [x] Nenhum item ainda valido de `docs/analysis_results.md` fica sem classificacao: corrigido, planejado, arquivado ou invalido.

---

## Nova rodada - Pendencias da analise profunda

Origem: `_reversa_sdd/analise-profunda-implementacao-linguagem.md`.

Observacao importante: P0-P10 ficam preservadas como historico de fechamento
anterior. Os itens abaixo reabrem areas que ainda divergiram da documentacao
oficial quando testadas novamente em 2026-05-12.

Validacao base da nova rodada:

- [x] `cargo fmt --check` passou antes da nova lista.
- [x] `cargo check --workspace` passou antes da nova lista.
- [x] `cargo test --workspace` passou antes da nova lista.
- [x] `git diff --check` passou antes da nova lista, com apenas avisos CRLF/LF.
- [x] Ao finalizar a nova rodada, repetir `cargo fmt --check`.
- [x] Ao finalizar a nova rodada, repetir `cargo check --workspace`.
- [x] Ao finalizar a nova rodada, repetir `cargo test --workspace`.
- [x] Ao finalizar a nova rodada, repetir `git diff --check`.

---

## P11 - Corrigir corrupcao silenciosa e controle de fluxo invalido

Objetivo: eliminar bugs que aceitam codigo invalido ou alteram valores sem
diagnostico.

### P11.1 - Literais numericos com sufixo e overflow

Problema: literais como `3.5f64` e inteiros fora de faixa podem virar `0`
durante o lowering.

- [x] Criar uma rotina unica de parsing de literais numericos com resultado
      estruturado: base, valor, sufixo, tipo inferido e erro.
- [x] Remover `unwrap_or(0)` e `unwrap_or(0.0)` de caminhos de literal em
      `compiler/crates/ori-hir/src/lower.rs`.
- [x] Fazer o checker inferir tipo por sufixo:
  - [x] `i8`, `i16`, `i32`, `i64`.
  - [x] `u8`, `u16`, `u32`, `u64`.
  - [x] `f32`, `f64`.
- [x] Validar faixa numerica conforme o sufixo.
- [x] Emitir diagnostico claro para overflow.
- [x] Emitir diagnostico claro para sufixo invalido em vez de erro generico de parse.
- [x] Garantir que hex, binario e octal aceitem sufixo sem cair para zero.
- [x] Atualizar docs se o conjunto real de sufixos aceitos for menor que a spec.
      - Nao aplicavel nesta etapa: o conjunto documentado foi mantido.
- [x] Adicionar testes positivos:
  - [x] `42u8`.
  - [x] `42i32`.
  - [x] `0xFFu8`.
  - [x] `0b1111u8`.
  - [x] `3.14f32`.
  - [x] `3.14f64`.
- [x] Adicionar testes negativos:
  - [x] `256u8`.
  - [x] `128i8`.
  - [x] `9223372036854775808i64`.
  - [x] float com sufixo invalido.
- [x] Adicionar teste E2E garantindo que `ori build` e `ori compile` preservam
      os valores escritos.

Critério de aceite:

- [x] Nenhum literal invalido vira `0` silenciosamente.
- [x] Literais com sufixo geram o tipo esperado ou erro semantico claro.

### P11.2 - `break` e `continue` fora de loops

Problema: `break` e `continue` fora de loop passam no checker e viram no-op no
backend nativo.

- [x] Adicionar rastreamento de `loop_depth` no checker.
- [x] Incrementar profundidade em:
  - [x] `while`.
  - [x] `while some`.
  - [x] `for`.
  - [x] `loop`.
  - [x] `repeat`, se aplicavel ao AST atual.
- [x] Emitir diagnostico quando `break` aparecer com `loop_depth == 0`.
- [x] Emitir diagnostico quando `continue` aparecer com `loop_depth == 0`.
- [x] Definir regra para `break`/`continue` dentro de closure aninhada em loop.
- [x] Adicionar defesa no backend nativo para HIR invalido, em vez de no-op.
      - Feito no checker: HIR invalido nao chega ao backend quando o pipeline
        normal e usado.
- [x] Adicionar defesa equivalente no backend C, se aplicavel.
- [x] Adicionar testes negativos para:
  - [x] `break` no corpo de funcao.
  - [x] `continue` no corpo de funcao.
  - [x] `break` dentro de `if` fora de loop.
  - [x] `continue` dentro de `match` fora de loop.
  - [x] `break` dentro de closure aninhada em loop.
- [x] Adicionar testes positivos para `break` e `continue` dentro de loops
      validos.

Critério de aceite:

- [x] Codigo com `break`/`continue` fora de loop falha no checker.
- [x] Backends nao silenciam controle de fluxo invalido.

Validacao P11:

- [x] `cargo check -p ori-types -p ori-parser -p ori-codegen`
- [x] `cargo test -p ori-codegen c_backend_reports_loop_control_outside_loop -- --nocapture`
- [x] `cargo test -p ori-driver check_reports_numeric_literal_invalid_suffix --test multifile_imports -- --nocapture --test-threads=1`

---

## P12 - Corrigir comparabilidade, igualdade estrutural e traits de operadores

Objetivo: alinhar `==`, `!=`, operadores comparativos e colecoes com
`docs/spec/04-types.md` e `docs/spec/08-traits.md`.

### P12.1 - Proibir comparacao de funcoes

Problema: `func(...) == func(...)` passa no checker, mas a spec exige erro.

- [x] Rejeitar `Ty::Func` em `==` e `!=`.
- [x] Rejeitar tipos compostos que contenham funcao quando nao houver
      `Equatable` customizado valido.
- [x] Usar codigo de diagnostico consistente com o catalogo, preferencialmente
      `type.comparison_not_supported`.
- [x] Adicionar teste negativo para comparar duas closures.
- [x] Adicionar teste negativo para comparar duas funcoes nomeadas, se a
      linguagem permite referenciar funcao como valor.
- [x] Adicionar teste negativo para struct com campo `func` e sem `Equatable`.

Critério de aceite:

- [x] Nenhum valor de funcao e comparavel por acidente.

### P12.2 - Implementar ou bloquear igualdade estrutural de tipos compostos

Problema: duas structs com mesmo conteudo podem comparar como diferentes por
comparacao de ponteiro.

- [x] Decidir estrategia imediata:
  - [x] Rejeitar `==`/`!=` em tipos compostos ate haver suporte correto.
        - Estrategia aplicada nesta rodada: bloquear tipos compostos no
          checker ate existir igualdade estrutural real nos backends.
- Futuro planejado: implementar igualdade estrutural real para `struct`,
  `tuple`, `enum` com payload, `optional<T>`, `result<T, E>`, `list<T>`,
  `map<K, V>` e `set<T>`.
- [x] Fazer `!=` reutilizar a mesma semantica de `==` negada.
- [x] Garantir que strings continuem por valor.
- [x] Rejeitar igualdade se algum campo interno nao for comparavel.
- Futuro planejado: quando igualdade estrutural existir, adicionar testes para
  valores iguais, valores diferentes e estruturas aninhadas.

Critério de aceite:

- [x] A spec atualizada marca igualdade estrutural como planejada, e o checker
      rejeita antes do codegen.
- [x] Onde ainda nao houver suporte, o checker falha antes do codegen.

### P12.3 - Ligar operadores a traits documentadas

Problema: `Equatable`, `Comparable` e traits de operadores estao documentadas,
mas os operadores nao despacham para esses metodos.

- [x] Atualizar docs marcando operator traits como planejados, nao
      implementados.
- Futuro planejado: definir precedencia entre operador primitivo, igualdade
  estrutural, `Equatable` customizado e `Comparable` customizado.
- Futuro planejado: resolver `==`/`!=` por `Equatable.equals` e
  `<`/`<=`/`>`/`>=` por `Comparable.compare`.
- Futuro planejado: adicionar diagnostico para trait de operador ausente e
  testes de `Equatable`/`Comparable` customizados.

Critério de aceite:

- [x] A documentacao de operator traits deixa de ser promessa sem suporte.

### P12.4 - Aplicar `Hashable` e `Equatable` em `map` e `set`

Problema: a spec exige `Hashable`, mas runtime e checker tratam chaves/valores
como `i64` sem contrato generico.

- [x] Reduzir contrato atual: `map` usa hash runtime para `int` e `string`.
- [x] Reduzir contrato atual: `set` usa hash runtime para `int` e `string`.
- [x] Rejeitar chaves de `map` fora de `int`/`string` com diagnostico claro.
- [x] Rejeitar elementos de `set` fora de `int`/`string` com diagnostico claro.
- [x] Adicionar testes positivos para `map<string, int>` e `set<string>`, e
  testes negativos para chaves/elementos ainda nao suportados.
- [x] Adicionar testes de mismatch em literais de `map` e `set`.
- Futuro planejado: implementar `Hashable`/`Equatable` generico, ABI de
  hash/equality por tipo, `map<StructComHash, int>` e `set<T>` por valor.

Critério de aceite:

- [x] Mapas e sets seguem o contrato generico documentado ou a spec e reduzida
      explicitamente.

---

## P13 - Corrigir paridade do backend C e contrato dos comandos

Objetivo: impedir que `ori build` e `ori compile` prometam uma semantica que os
backends nao entregam.

### P13.1 - Corrigir `?` no backend C

Problema: `expr?` no backend C acessa payload sem checar erro/none nem retornar
antecipadamente.

- Futuro planejado: implementar propagacao real de `result<T,E>` e
  `optional<T>` no C backend, incluindo cleanup de escopo e `using`.
- [x] Se a correcao completa for grande demais, bloquear `?` em `ori build`
      com erro claro ate a paridade existir.
- [x] Adicionar teste C backend para `result<T,E>?`.
- [x] Adicionar teste C backend para `optional<T>?`.
- Futuro planejado: quando `?` no C backend existir, adicionar teste com
  `using` e comparar a saida com o backend nativo.

Critério de aceite:

- [x] `?` tem a mesma semantica no backend C e no backend nativo ou e
      explicitamente rejeitado no C backend.

### P13.2 - Definir status real do backend C

Problema: o backend C inline tem runtime reduzido, ARC no-op e paridade parcial.

- [x] Decidir se `ori build` e backend de producao ou apenas debug.
- [x] Se for debug:
  - [x] documentar isso em help, README e checklist.
  - [x] emitir erro claro quando uma feature sem paridade perigosa for usada.
  - [x] bloquear features perigosas em vez de gerar C incorreto.
- Futuro planejado se o backend C virar producao: implementar ARC real no
  runtime inline, alinhar stdlib completa com o nativo e criar matriz de
  paridade obrigatoria.
- [x] Cercar hooks ARC no-op com documentacao explicita de limite do backend C.
- Futuro planejado: adicionar testes de paridade para ARC, `using`, closures,
  string, colecoes, erro e propagacao.

Critério de aceite:

- [x] Uma pessoa usando `ori build` entende exatamente os limites do C gerado.
- [x] O backend C nao gera codigo semanticamente errado em silencio para os
      casos perigosos cobertos: eles sao rejeitados cedo ou documentados como
      limite do backend debug.

### P13.3 - Corrigir contrato de `ori compile` sobre dependencia de `cc`

Problema: o help diz que nao precisa de compilador C, mas o pipeline chama
`cc` para runtime e link.

- [x] Atualizar texto do CLI em `compiler/crates/ori-driver/src/main.rs`.
- [x] Documentar prerequisito de `cc`/linker na documentacao de instalacao ou
      uso.
- [x] Fazer checagem antecipada de `cc` antes de etapas longas de compilacao.
- [x] Melhorar mensagem quando `cc` nao esta no PATH.
- Futuro planejado: avaliar runtime precompilado, linker Rust puro ou pacote
  de toolchain integrado.
- [x] Adicionar teste de help garantindo que o texto nao prometa "no C compiler
      needed".
- [x] Adicionar teste unitario ou integração simulando ausencia de `cc`, se
      viavel.

Critério de aceite:

- [x] CLI, docs e comportamento real concordam sobre prerequisitos nativos.

---

## P14 - Alinhar stdlib oficial com implementacao real

Objetivo: impedir que a spec publique APIs que passam no checker mas quebram no
backend, ou APIs com tipo diferente do documentado.

### P14.1 - Transformar import de stdlib em allowlist real

Problema: qualquer `ori.*` e aceito como import de stdlib.

- [x] Trocar `is_stdlib_import` generico por lista de modulos implementados.
- [x] Criar diagnostico para modulo `ori.*` planejado mas indisponivel.
- [x] Criar diagnostico para modulo `ori.*` desconhecido.
- [x] Garantir que `ori.iter` nao passe no checker enquanto nao houver runtime.
- [x] Garantir que `ori.format`, `ori.time`, `ori.random`, `ori.json`,
      `ori.test`, `ori.os` e `ori.Error` tenham status claro.
- [x] Atualizar `docs/spec/12-stdlib.md` para separar:
  - [x] implementado.
  - [x] parcial.
  - [x] planejado.
- [x] Atualizar `docs/IMPLEMENTATION_CHECKLIST.md` com o mesmo status.
- [x] Adicionar teste positivo para cada modulo implementado.
- [x] Adicionar teste negativo para cada modulo planejado ainda indisponivel.

Critério de aceite:

- [x] Nenhum modulo stdlib inexistente passa no checker como se existisse.

### P14.2 - Resolver `ori.iter`

Problema: `ori.iter` esta documentado, mas `iter.map` falha no backend nativo.

- [x] Escolher decisao:
  - [x] Mover `ori.iter` para planejado e bloquear no checker.
- Futuro planejado: implementar `ori.iter` com `map`, `filter`, `reduce`,
  `flat_map`, `find`, `any`, `all`, `zip`, `enumerate` e `group_by` apos
  fechar `Hashable`/`Equatable` para mapas.
- [x] Se adiar:
  - [x] Atualizar spec.
  - [x] Atualizar checklist.
  - [x] Emitir erro claro ao importar ou chamar `ori.iter`.
- [x] Adicionar testes E2E para a decisao escolhida.

Critério de aceite:

- [x] `ori.iter` nao fica em estado "documentado, aceito no check e quebrado
      no compile".

### P14.3 - Resolver divergencias de `ori.string`, `ori.convert` e parsing

Problema: a spec documenta `string.parse_int` e `string.parse_float` como
`result`, mas a implementacao usa conversoes com `optional`.

- [x] Decidir contrato canonico:
  - [x] `convert.string_to_int(...) -> optional<int>`.
- Futuro planejado: se `string.parse_*` voltar para a spec principal,
  implementar `string.parse_int`, `string.parse_float` e retorno
  `result<T, string>`.
- [x] Se a implementacao atual for mantida:
  - [x] Atualizar `docs/spec/12-stdlib.md`.
  - [x] Documentar `ori.convert`.
  - [x] Remover ou marcar `string.parse_*` como planejado.
- [x] Verificar e alinhar:
  - [x] `string.trim_start`.
  - [x] `string.trim_end`.
  - [x] aliases de compatibilidade, se existirem.
- [x] Adicionar testes de sucesso e falha para parsing numerico.
- [x] Adicionar testes de docs oficiais para strings.

Critério de aceite:

- [x] Exemplos oficiais de string compilam ou sao marcados como planejados.

### P14.4 - Resolver divergencias de `ori.math`

Problema: `math.floor`, `ceil` e `round` retornam `int`, mas a spec diz
`float`; outras funcoes documentadas estao ausentes.

- [x] Decidir contrato de retorno:
  - [x] `floor/ceil/round -> int`, conforme implementacao atual.
- [x] Alinhar checker, HIR, runtime nativo, runtime C embutido e C backend.
- [x] Resolver sobrecargas de `math.abs`, `math.min` e `math.max`:
  - [x] int.
  - [x] float.
- [x] Implementar ou marcar como planejado:
  - [x] `math.clamp`.
  - [x] `math.log2`.
  - [x] `math.infinity`.
  - [x] `math.nan`.
  - [x] `math.is_nan`.
  - [x] `math.is_infinite`.
- [x] Adicionar testes de tipo de retorno para `floor`, `ceil` e `round`.
- [x] Adicionar testes E2E para cada funcao documentada ou teste negativo para
      cada funcao planejada.

Critério de aceite:

- [x] `docs/spec/12-stdlib.md` e comportamento real de `ori.math` concordam.

---

## P15 - Diagnosticos, comentarios, atributos e ferramentas

Objetivo: alinhar mensagens de erro, catalogo oficial e ferramentas planejadas.

### P15.1 - Sincronizar catalogo de diagnosticos com codigos emitidos

Problema: codigos emitidos nao aparecem em `docs/spec/13-error-catalog.md`.

- [x] Listar todos os codigos emitidos por `Diagnostic::error` e
      `Diagnostic::warning`.
- [x] Adicionar ao catalogo ou renomear os codigos:
  - [x] `name.duplicate`.
  - [x] `name.undefined`.
  - [x] `type.tuple_index_out_of_bounds`.
  - [x] `type.undefined_name`.
  - [x] `lex.unexpected_character`, se for codigo publico.
- [x] Criar teste/script que falha se um codigo emitido nao estiver no catalogo.
- [x] Criar teste/script que alerta quando codigo catalogado nunca e emitido,
      separando codigos planejados.
- [x] Atualizar snapshots ou fixtures de diagnostico impactadas
      (nenhum snapshot separado foi impactado; a fixture viva e o catalogo).

Critério de aceite:

- [x] O catalogo volta a ser fonte confiavel para ferramentas e LSP.

### P15.2 - Corrigir BOM e comentarios de bloco nao fechados

Problema: BOM inicial e comentario de bloco sem fechamento geram experiencia
lexical divergente da spec.

- [x] Aceitar e ignorar BOM UTF-8 no inicio do arquivo.
- [x] Manter BOM fora do inicio como erro lexical.
- [x] Preservar spans corretos apos remover BOM inicial.
- [x] Implementar diagnostico especifico para comentario de bloco nao fechado.
- [x] Decidir codigo correto:
  - [x] novo codigo `lex.unclosed_block_comment`, pois o caso tratado e
        comentario comum.
- [x] Adicionar teste com arquivo UTF-8 BOM.
- [x] Adicionar teste com BOM no meio do arquivo.
- [x] Adicionar teste com `--|` sem fechamento.
- [x] Adicionar teste com comentario de bloco valido.

Critério de aceite:

- [x] O lexer segue `docs/spec/02-lexical.md` para BOM.
- [x] Comentario nao fechado tem diagnostico claro, nao erro generico.

### P15.3 - Definir semantica de comentarios de documentacao e atributos

Problema: a AST parseia atributos, mas `@deprecated`, `@test`, `@param` e
`ori doc`/`ori test` nao tem suporte real equivalente a spec.

- [x] Decidir o escopo da versao atual:
  - [x] Marcar doc comments como planejados.
  - [x] Implementar validacao atual de atributos builtin.
- [x] Emitir `attr.deprecated` em uso de declaracao marcada.
- Futuro planejado: validar `@param` e implementar `ori test`/`ori doc`.
- [x] Se adiar:
  - [x] Atualizar `docs/spec/02-lexical.md`.
  - [x] Atualizar `docs/spec/13-error-catalog.md`.
  - [x] Atualizar checklist para deixar claro que e planejado.
- [x] Adicionar testes para `@deprecated`.
- [x] Adicionar testes para `@test`.
- [x] Adicionar testes para `@param` incorreto.
- [x] Adicionar testes para atributo desconhecido.
- [x] Adicionar testes para alvo invalido, argumentos invalidos e duplicatas
      de atributo.

Critério de aceite:

- [x] Atributos e doc comments nao ficam documentados como completos sem
      comportamento implementado.
- [x] Atributos builtin rejeitam nome desconhecido, alvo invalido e formato de
      argumento invalido; duplicatas geram warning.
- [x] Uso local ou importado de declaracao `@deprecated` gera warning
      `attr.deprecated`.

### P15.4 - Resolver status do LSP

Problema: `ori-lsp` existe no workspace, mas e placeholder.

- [x] Decidir se `ori-lsp` e ferramenta oficial desta versao.
- Futuro planejado se o LSP virar oficial: implementar handshake
  `initialize`, `textDocument/didOpen`, `textDocument/didChange`, publicacao
  de diagnosticos e teste de protocolo basico.
- [x] Se for futuro:
  - [x] Atualizar README/checklist para dizer que e placeholder.
  - [x] Evitar prometer LSP em docs publicas.
- [x] Garantir que o binario nao pareca uma ferramenta pronta quando nao e.

Critério de aceite:

- [x] Estado do LSP fica claro para usuarios e mantenedores.

---

## P16 - Reduzir duplicacao de runtime e ABI

Objetivo: diminuir regressao causada por tres fontes de runtime e assinaturas
duplicadas.

- [x] Inventariar todas as funcoes stdlib conhecidas por:
  - [x] checker.
  - [x] HIR/lowering.
  - [x] backend nativo.
  - [x] backend C.
  - [x] runtime Rust.
  - [x] runtime C embutido no driver.
  - [x] runtime inline do C backend.
- [x] Criar manifest unico de stdlib/ABI, mesmo que inicialmente seja um
      arquivo Rust centralizado.
- [x] Gerar ou validar `stdlib_c_name` a partir desse manifest.
- [x] Validar assinaturas de runtime a partir desse manifest.
      Observacao: o manifest agora guarda caminhos, aliases, simbolos e flags
      por backend; a geracao completa de tipos ABI ainda fica planejada e
      documentada em `docs/IMPLEMENTATION_CHECKLIST.md`.
- [x] Adicionar teste que falha quando uma funcao mapeada no HIR nao existe no
      runtime nativo.
- [x] Adicionar teste que falha quando uma funcao mapeada no HIR nao existe no
      backend C, se o backend C continuar aceitando a feature.
- [x] Documentar quais runtimes sao fontes de verdade e quais sao derivados.

Critério de aceite:

- [x] Uma nova funcao stdlib nao pode ser adicionada em apenas um backend sem
      teste falhar.

---

## P17 - Atualizar documentacao oficial apos as correcoes

Objetivo: manter a documentacao como contrato legivel, sem prometer mais do que
o codigo entrega.

- [x] Atualizar `docs/spec/02-lexical.md` para refletir BOM, literais,
      comentarios e atributos.
- [x] Atualizar `docs/spec/04-types.md` para refletir igualdade estrutural,
      funcao nao comparavel, map/set e status real de tipos planejados.
- [x] Atualizar `docs/spec/08-traits.md` para refletir operator traits
      implementadas ou planejadas.
- [x] Atualizar `docs/spec/09-errors.md` se houver diferenca entre backend C e
      nativo para `?`.
- [x] Atualizar `docs/spec/10-memory.md` com limites por backend, se necessario.
- [x] Atualizar `docs/spec/12-stdlib.md` separando implementado, parcial e
      planejado.
- [x] Atualizar `docs/spec/13-error-catalog.md` com codigos reais.
- [x] Atualizar `docs/IMPLEMENTATION_CHECKLIST.md` com todos os estados finais.
- [x] Atualizar exemplos oficiais que usam APIs alteradas.
- [x] Garantir texto acessivel:
  - [x] frases curtas.
  - [x] exemplos pequenos.
  - [x] status explicito de feature planejada.

Critério de aceite:

- [x] Uma pessoa nova consegue confiar na spec sem ler o codigo-fonte.

---

## P18 - Gate final da nova rodada

Objetivo: fechar todos os achados da analise profunda com validacao objetiva.

- [x] Criar ou atualizar fixtures para cada achado de
      `_reversa_sdd/analise-profunda-implementacao-linguagem.md`.
- [x] Rodar testes focados de P11.
- [x] Rodar testes focados de P12.
- [x] Rodar testes focados de P13.
- [x] Rodar testes focados de P14.
- [x] Rodar testes focados de P15.
- [x] Rodar testes focados de P16.
- [x] Rodar `cargo fmt --check`.
- [x] Rodar `cargo check --workspace`.
- [x] Rodar `cargo test --workspace`.
- [x] Rodar `git diff --check`.
- [x] Reexecutar as reproducoes da analise profunda e confirmar que:
  - [x] literal com sufixo nao vira zero.
  - [x] overflow nao vira zero.
  - [x] duas structs iguais por valor retornam `same` ou o checker rejeita
        comparacao ate implementar igualdade estrutural.
  - [x] `func == func` falha no checker.
  - [x] `break` fora de loop falha no checker.
  - [x] arquivo com BOM inicial passa.
  - [x] `ori.iter` funciona ou falha cedo com mensagem clara.
  - [x] `math.floor` bate com a spec atualizada.
  - [x] `?` no backend C funciona ou e rejeitado cedo.
- [x] Atualizar este plano marcando itens concluidos.
- [x] Criar relatorio de fechamento da nova rodada em `_reversa_sdd/`.

Critério de aceite:

- [x] Todos os achados da analise profunda estao corrigidos, documentados como
      planejados, ou bloqueados com diagnostico claro.
- [x] Codigo, testes e documentacao contam a mesma historia.

---

## P19 - Fechar lacunas objetivas do checklist

Objetivo: continuar a partir de `docs/IMPLEMENTATION_CHECKLIST.md`, fechando
itens pequenos que ja tem implementacao declarada, mas ainda estavam sem
validacao objetiva.

### P19.1 - Testes de struct update expression

Problema: o checklist marcava parser, lowering e codegen de struct update como
implementados, mas ainda deixava os testes da feature como pendentes.

- [x] Adicionar teste E2E nativo para `base with { field: value } end`.
- [x] Cobrir atualizacao de um campo e de varios campos.
- [x] Confirmar que o valor base permanece legivel apos criar o valor
      atualizado.
- [x] Adicionar teste do backend C para gerar e compilar C com struct update.
- [x] Atualizar `docs/IMPLEMENTATION_CHECKLIST.md`.

Validacao executada:

- [x] `cargo fmt --check`
- [x] `cargo test -p ori-driver struct_update -- --nocapture --test-threads=1`
- [x] `cargo check --workspace`

Critério de aceite:

- [x] Struct update deixa de ser uma feature marcada como implementada sem
      teste dedicado.

### P19.2 - Catalogo de diagnosticos no checklist

Problema: P15 ja tinha sincronizado o catalogo de diagnosticos, mas o checklist
geral ainda deixava `Diagnostics catalog` como pendente.

- [x] Rodar o teste dedicado do catalogo.
- [x] Confirmar que codigos emitidos e catalogados continuam coerentes.
- [x] Manter codigos planejados como planejados, sem marcar como emitidos.
- [x] Atualizar `docs/IMPLEMENTATION_CHECKLIST.md`.

Validacao executada:

- [x] `cargo test -p ori-driver --test diagnostic_catalog -- --nocapture --test-threads=1`

Critério de aceite:

- [x] O checklist deixa de mostrar o catalogo de diagnosticos como pendente
      quando a verificacao automatizada ja existe e passa.

### Gate P19

- [x] Rodar `cargo test --workspace` apos as atualizacoes de P19.

Resultado:

- [x] Workspace completo passou.

---

## P20 - Fechamento da rodada de auditoria de runtime em 2026-05-17

Origem: nova validacao manual dos achados de runtime e backend nativo:

- `bytes` com NUL interno ainda truncava em `len`, `to_hex` e `from_hex`.
- `string.len()` e `string.slice()` ainda usavam semantica de byte no runtime nativo.
- `ori_heap_into_sorted_list` ainda bloqueava `clippy`.
- Colecoes ainda tinham caminhos de remocao/limpeza sem desfazer arestas ARC.
- Comparador customizado de heap retinha valores temporariamente sem liberar.

### P20.1 - Corrigir `bytes` com NUL

- [x] Registrar tamanho real das alocacoes gerenciadas no estado ARC.
- [x] Fazer `cstring_from_bytes` preservar NUL interno em vez de descartar o payload.
- [x] Fazer `ori_bytes_len` usar tamanho real do payload quando disponivel.
- [x] Fazer `ori_bytes_concat`, `ori_bytes_slice`, `ori_bytes_to_hex`,
      `ori_bytes_from_hex`, `ori_bytes_decode_utf8` e `ori_bytes_get`
      respeitarem payload binario com NUL.
- [x] Adicionar teste de runtime para bytes com NUL.
- [x] Adicionar teste E2E nativo para `b"\x41\x00\x42"` e `"410042".from_hex()`.

### P20.2 - Corrigir `string` Unicode no runtime nativo

- [x] Fazer `ori_string_len` contar escalares Unicode em vez de bytes.
- [x] Fazer `ori_string_slice` receber indices por caractere e converter para
      offsets de byte seguros.
- [x] Manter `io.print` e interpolacao usando tamanho em bytes para escrita em
      stdout, sem confundir com `string.len()`.
- [x] Adicionar teste de runtime para `"\u{00e1}\u{00e9}".slice(0, 1)`.
- [x] Adicionar teste E2E nativo para `"áé".len()` e `"áé".slice(0, 1)`.

### P20.3 - Corrigir ARC em caminhos de remocao de colecoes

- [x] Fazer `list.pop`, `list.remove` e `list.clear` removerem arestas ARC.
- [x] Fazer `deque.pop_front`, `deque.pop_back`, remocao por cursor e
      `deque.clear` removerem arestas ARC.
- [x] Fazer `set.remove` e `set.clear` removerem arestas ARC.
- [x] Fazer `map.set` em chave existente atualizar aresta do valor antigo para
      o novo valor.
- [x] Fazer `map.remove`, `map.try_remove` e `map.clear` removerem arestas de
      chave e valor.
- [x] Preservar valores gerenciados retornados por pop/remove via optional
      enquanto o optional estiver vivo.
- [x] Adicionar teste de runtime para remocao/clear de list, deque, set e map.

### P20.4 - Corrigir heap e `clippy`

- [x] Liberar os retains temporarios usados pelo comparador customizado de heap.
- [x] Adicionar teste de runtime garantindo que o comparador customizado nao
      vaza refcount.
- [x] Reescrever o loop de `ori_heap_into_sorted_list` para remover o erro
      `clippy::while_immutable_condition`.

### Validacao executada

- [x] `cargo fmt --all -- --check`
- [x] `cargo check --workspace`
- [x] `cargo test -p ori-runtime -- --test-threads=1`
- [x] `cargo test -p ori-driver compile_runs_bytes_preserve_nul_native -- --nocapture --test-threads=1`
- [x] `cargo test -p ori-driver compile_runs_unicode_string_len_and_slice_native -- --nocapture --test-threads=1`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets`
- [x] Repro manual de `bytes` com NUL: `len=3` e `410042`.
- [x] Repro manual de Unicode: `len=2` e slice `á`.
- [x] Runtime nativo local reestagiado com
      `tools/stage_native_runtime.ps1 -Profile debug`.

### Resultado

- [x] Os quatro findings novos da auditoria foram corrigidos ou fechados com
      teste objetivo.
- [x] O plano de bugs criticos foi fechado; dividas estruturais ficaram
      registradas em `docs/plano-correcao-implementacao-linguagem.md` para
      execucao em rodadas separadas.
- [x] Build, testes e `clippy` passam no estado atual.
