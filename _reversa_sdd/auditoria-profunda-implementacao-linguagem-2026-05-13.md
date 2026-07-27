# Auditoria profunda da implementacao da linguagem Ori

Data: 2026-05-13

Escopo: arquitetura, lexer, parser, AST, semantica, HIR, backend nativo,
backend C, runtime, biblioteca padrao, diagnosticos e ferramentas.

Fontes comparadas:

- `docs/spec/*.md`
- `docs/IMPLEMENTATION_CHECKLIST.md`
- `_reversa_sdd/plano-correcao-implementacao-linguagem.md`
- `_reversa_sdd/analise-profunda-implementacao-linguagem.md`
- Codigo atual em `compiler/crates/*`

Regra desta auditoria: nenhum arquivo legado foi alterado. Este relatorio novo
foi escrito somente em `_reversa_sdd/`.

## Resumo executivo

A base atual passa na suite existente:

```text
cargo test --workspace -> passou
```

Mesmo assim, a suite ainda nao cobre varias divergencias importantes. Foram
reproduzidos programas pequenos que passam no `check`, mas:

- falham so no backend;
- geram binario com comportamento errado;
- ignoram parte do codigo-fonte;
- ou contradizem a spec oficial.

Os problemas mais graves estao em tres areas:

1. Parser e checker aceitam ou descartam codigo invalido sem diagnostico.
2. A semantica documentada de metodos, booleanos, bounds e helpers de erro nao
   bate com o comportamento real.
3. A stdlib e os tipos centrais ainda tem contratos duplicados ou incompletos.

## Mapa rapido da arquitetura

Fluxo principal:

```text
.orl
  -> ori-lexer
  -> ori-parser / ori-ast
  -> ori-types: resolve + check
  -> ori-hir: lowering + monomorph
  -> ori-codegen: native ou C
  -> ori-driver: CLI, imports, manifests, runtime C embutido e link
```

Superficies que precisam ficar sincronizadas:

- `compiler/crates/ori-types/src/check.rs`: regras de tipo e parte da stdlib.
- `compiler/crates/ori-types/src/stdlib.rs`: manifesto de paths/simbolos.
- `compiler/crates/ori-hir/src/lower.rs`: mapeamento para chamadas HIR.
- `compiler/crates/ori-codegen/src/native_backend.rs`: Cranelift.
- `compiler/crates/ori-codegen/src/c_backend.rs`: backend C de debug.
- `compiler/crates/ori-runtime/src/lib.rs`: runtime Rust exportado.
- `compiler/crates/ori-driver/src/pipeline.rs`: runtime C embutido para link.

## Validacoes executadas

Validacao geral:

```text
cargo test --workspace
```

Resultado: passou.

Reproducoes focadas foram criadas em diretorio temporario fora do projeto. Os
casos abaixo estao resumidos nos achados.

## Achados criticos

### 1. Atribuicao em campo e descartada pelo parser, e `mut func` nao muta

- Descricao: `b.value = 2` nao vira um statement valido. O parser trata
  `b.value` como `QualifiedIdent` multi-segmento, `expr_to_lvalue` retorna
  `None`, a recuperacao sincroniza ate `end` e o resto do bloco e descartado.
- Local provavel:
  - `compiler/crates/ori-parser/src/parse_stmt.rs:66`
  - `compiler/crates/ori-parser/src/parse_stmt.rs:78`
  - `compiler/crates/ori-parser/src/parse_stmt.rs:382`
  - `compiler/crates/ori-parser/src/parse_stmt.rs:390`
- Esperado pela documentacao:
  - `docs/spec/07-functions.md` documenta `mut func` alterando `self.field`.
  - `docs/spec/10-memory.md` diz que mutar um `var` deve alterar esse valor.
- Atual observado:
  - `b.value = 2` some do AST sem erro util.
  - `mut func inc(self) { self.value = self.value + 1 }` compila, mas `b.inc()`
    deixa `b.value` em `1`.
- Impacto: mutabilidade de struct fica quebrada. O usuario ve codigo aceito,
  mas a mutacao nao acontece.
- Severidade: Critica.
- Sugestao:
  - No parser, representar `b.value` como `Expr::Field` quando o primeiro
    segmento e valor local.
  - Quando `expr_to_lvalue` falhar, emitir diagnostico de lvalue invalido em
    vez de retornar `None` silenciosamente.
  - Adicionar assert no checker/HIR para nao permitir statement descartado.
- Testes recomendados:
  - `var b: Counter = Counter(value: 1); b.value = 2; print(b.value)` deve
    imprimir `2`.
  - `mut func inc(self)` deve alterar o receiver.
  - Apos uma atribuicao invalida, statements seguintes nao podem sumir.

### 2. Sintaxe de metodo diverge da spec e pode quebrar no backend

- Descricao: a spec diz que `self` e implicito e nao deve aparecer na lista de
  parametros. A implementacao e os testes atuais usam `mut func inc(self)`.
- Local provavel:
  - `docs/spec/07-functions.md:141`
  - `compiler/crates/ori-types/src/check.rs:2495`
  - `compiler/crates/ori-hir/src/lower.rs:2156`
  - `compiler/crates/ori-driver/tests/multifile_imports.rs:3679`
- Esperado pela documentacao:
  - `mut func increment()` deve ter `self` implicito.
- Atual observado:
  - `mut func inc()` passa no `check`, mas o backend nativo falha com erro de
    verificacao Cranelift.
  - `mut func inc(self)` e o formato que os testes internos usam.
- Impacto: exemplos oficiais de metodos podem passar no `check` e falhar so no
  `compile`.
- Severidade: Critica.
- Sugestao:
  - Escolher uma regra unica.
  - Se `self` for implicito, inserir `self` na assinatura de metodo no resolve
    e no HIR.
  - Se `self` for explicito por enquanto, atualizar a spec e rejeitar metodo
    sem `self` no checker.
- Testes recomendados:
  - Metodo sem `self` com acesso a `self.value`.
  - Metodo com `self` explicito se ainda for aceito.
  - Chamada de metodo em `const` e `var`.

### 3. Nomes desconhecidos passam no `check` como `_#0`

- Descricao: identificadores qualificados que nao resolvem retornam `Ty::Infer(0)`
  em vez de gerar `name.undefined`.
- Local provavel:
  - `compiler/crates/ori-types/src/check.rs:970`
  - `compiler/crates/ori-types/src/check.rs:1015`
  - `compiler/crates/ori-types/src/check.rs:1018`
- Esperado pela documentacao:
  - Nome nao declarado deve falhar no checker.
- Atual observado:
  - `const x: int = unknown_name` retorna `no errors`.
  - `missing_call(1)` retorna `no errors`.
  - `compile` falha depois com `undefined variable` ou `missing function
    reference`.
- Impacto: o checker deixa de ser uma barreira confiavel. Erros aparecem tarde
  e com mensagem de backend.
- Severidade: Critica.
- Sugestao:
  - Em `Expr::QualifiedIdent`, se `q.is_single()` e nao ha local, global,
    stdlib, enum ou alias, emitir `name.undefined`.
  - Para caminhos multi-segmento, emitir diagnostico de path/campo inexistente
    quando o primeiro segmento tambem nao e import/local conhecido.
- Testes recomendados:
  - Valor desconhecido em `const`.
  - Funcao desconhecida chamada como statement.
  - Caminho `foo.bar` sem import `foo`.

### 4. `panic`, `todo` e `unreachable` estao documentados, mas nao implementados

- Descricao: a spec define essas formas como especiais. Hoje elas sao chamadas
  comuns para nomes que nao existem, e o bug de nomes desconhecidos mascara o
  erro no `check`.
- Local provavel:
  - `docs/spec/07-functions.md:330`
  - `docs/spec/09-errors.md:249`
  - `compiler/crates/ori-parser/src/parse_expr.rs`
  - `compiler/crates/ori-types/src/check.rs:970`
- Esperado:
  - `panic("x")`, `todo()` e `unreachable()` devem ser formas especiais, com
    tipo `never` e runtime panic.
- Atual observado:
  - `ori check` retorna `no errors`.
  - `ori compile` falha com `missing function reference panic`.
- Impacto: exemplos e padroes de controle de fluxo documentados nao funcionam.
- Severidade: Alta.
- Sugestao:
  - Criar AST/HIR dedicados ou builtins tipados com retorno `Ty::Never`.
  - Gerar trap/panic com mensagem no backend nativo e C.
- Testes recomendados:
  - `panic("fatal")` termina com panic.
  - `todo()` e `unreachable()` terminam com panic.
  - `return` apos `panic()` nao deve ser exigido em funcao nao-void.

### 5. `and`, `or` e `not` nao validam operandos booleanos

- Descricao: `infer_binary` retorna `bool` para `and/or` sem checar operandos.
  `not` tambem retorna `bool` sem validar o operando.
- Local provavel:
  - `compiler/crates/ori-types/src/check.rs:1192`
  - `compiler/crates/ori-types/src/check.rs:1606`
- Esperado:
  - Operadores logicos devem exigir `bool`.
- Atual observado:
  - `const x: bool = 1 and 2` passa no `check`.
  - `const x: bool = not 1` passa no `check`.
  - `compile` pode falhar em Cranelift por tipo de valor incompatvel.
- Impacto: programas invalidos chegam ao backend e podem causar erro interno.
- Severidade: Critica.
- Sugestao:
  - Reusar `expect_bool` em `UnaryOp::Not`, `BinaryOp::And` e `BinaryOp::Or`.
  - Retornar `Ty::Error` se algum lado nao for bool.
- Testes recomendados:
  - `true and false` passa.
  - `1 and 2`, `true or 1`, `not 1` falham no checker.

### 6. `--|` dentro de string e tratado como comentario de bloco

- Descricao: antes da tokenizacao normal, o lexer faz pre-scan bruto por
  comentario de bloco nao fechado. Esse scan nao respeita strings.
- Local provavel:
  - `compiler/crates/ori-lexer/src/lexer.rs:36`
  - `compiler/crates/ori-lexer/src/lexer.rs:99`
- Esperado:
  - Marcadores de comentario dentro de string sao texto comum.
- Atual observado:
  - `"literal --| not a comment"` gera `lex.unclosed_block_comment`.
- Impacto: codigo valido falha dependendo do conteudo textual da string.
- Severidade: Alta.
- Sugestao:
  - Remover o pre-scan bruto.
  - Fazer o diagnostico de comentario nao fechado dentro do lexer/tokenizador,
    respeitando estados de string, byte string e f-string.
- Testes recomendados:
  - `--|` em string normal.
  - `--|` em byte string.
  - `--|` em f-string literal.
  - Comentario real nao fechado continua gerando erro dedicado.

### 7. Bounds de runtime nao seguem a spec

- Descricao: a spec exige panic para out-of-bounds e repeat negativo. O runtime
  retorna valores silenciosos ou nao executa o loop.
- Local provavel:
  - `docs/spec/05-expressions.md:119`
  - `docs/spec/06-statements.md:211`
  - `compiler/crates/ori-runtime/src/lib.rs:392`
  - `compiler/crates/ori-runtime/src/lib.rs:400`
  - `compiler/crates/ori-runtime/src/lib.rs:494`
  - `compiler/crates/ori-runtime/src/lib.rs:1367`
- Esperado:
  - Index fora do limite deve ser runtime panic.
  - Slice invalido deve ser validado.
  - `repeat -1 times` deve ser runtime panic.
- Atual observado:
  - `xs[2]` em lista com um item imprime `0` e sai com codigo `0`.
  - `ori_string_slice` e `ori_list_slice` clampam os limites.
  - `repeat -1 times` apenas pula o corpo e imprime `done`.
- Impacto: erros de programa viram dados validos falsos.
- Severidade: Alta.
- Sugestao:
  - Adicionar traps/panics no runtime Rust e no runtime C embutido.
  - Decidir se slice deve panicar ou clamp; documentar uma unica regra.
  - Checar repeat negativo antes do loop.
- Testes recomendados:
  - Lista, bytes e string index fora de faixa.
  - Slice com start/end negativos e acima de len.
  - `repeat -1 times`.

## Achados altos

### 8. Helpers de `optional` e `result` documentados nao existem

- Descricao: `.or`, `.or_return` e `.or_wrap` estao na spec, mas nao existem
  no checker/lowering/runtime.
- Local provavel:
  - `docs/spec/09-errors.md:62`
  - `docs/spec/09-errors.md:68`
  - `docs/spec/09-errors.md:125`
  - `compiler/crates/ori-types/src/check.rs:2477`
- Esperado:
  - `find_name().or("fallback")`, `.or_return(...)` e `.or_wrap(...)` devem
    funcionar ou estar marcados como planejados.
- Atual observado:
  - `.or(...)` nem parseia bem porque `or` e token de operador.
  - `.or_return(...)` falha como `field_on_non_struct` em `optional<string>`.
- Impacto: uma parte central da ergonomia de erros documentada nao existe.
- Severidade: Alta.
- Sugestao:
  - Implementar esses helpers como formas especiais no parser/checker ou como
    metodos builtin de `optional`/`result`.
  - Se forem futuros, mover para "planned" na spec.
- Testes recomendados:
  - `optional.or(fallback)`.
  - `optional.or_return(value)`.
  - `result.or_wrap(context)?`.

### 9. `ori.core` nao fornece as traits documentadas

- Descricao: a spec lista traits built-in em `ori.core`, mas nomes como
  `Displayable` nao estao definidos automaticamente.
- Local provavel:
  - `docs/spec/12-stdlib.md:99`
  - `compiler/crates/ori-types/src/check.rs:3320`
- Esperado:
  - `Displayable`, `Equatable`, `Comparable`, `Hashable`, `Disposable`,
    `Default`, `Error` etc. devem existir como tipos/traits centrais.
- Atual observado:
  - `const x: any<Displayable> = 1` gera `type.undefined_name`.
  - `using` procura `resolve_def_id("Disposable")`, ou seja, uma trait local
    chamada `Disposable`, nao a trait built-in de `ori.core`.
- Impacto: examples com `any<Displayable>` e o contrato de `using` dependem de
  definicoes locais nao documentadas.
- Severidade: Alta.
- Sugestao:
  - Criar definicoes reais de core traits no ambiente built-in.
  - Ou documentar que elas ainda sao planejadas e que o usuario precisa
    declarar trait local por enquanto.
- Testes recomendados:
  - `any<Displayable>` deve resolver.
  - `using` deve aceitar tipo que implementa `ori.core.Disposable`.
  - `import ori.core as core; any<core.Displayable>` deve funcionar.

### 10. `ori.list.contains` aceita valor de tipo errado

- Descricao: assinaturas genericas da stdlib usam `Ty::Infer(0)`. Como id `0`
  e tratado como inferencia solta, o segundo argumento nao e unificado com o
  tipo de elemento da lista.
- Local provavel:
  - `compiler/crates/ori-types/src/check.rs:2061`
  - `compiler/crates/ori-types/src/check.rs:3596`
  - `compiler/crates/ori-types/src/check.rs:2892`
- Esperado:
  - `ori.list.contains(list<int>, string)` deve falhar.
- Atual observado:
  - `const ok: bool = ori.list.contains(xs, "1")` passa no `check`.
- Impacto: a stdlib aceita chamadas que o runtime `i64` nao consegue
  representar corretamente para tipos gerenciados ou errados.
- Severidade: Alta.
- Sugestao:
  - Adicionar casos especiais para `ori.list.contains`, `index_of`, `set`,
    `insert`, `push` e funcoes equivalentes.
  - Ou trocar `Ty::Infer(0)` de assinatura por parametros genericos reais com
    substituicao por chamada.
- Testes recomendados:
  - `list<int>` com valor `string` deve falhar.
  - `list<string>` com valor `int` deve falhar.
  - `list<T>` generico deve preservar `T` entre argumentos e retorno.

### 11. Variadic `...` nao bate com o parser

- Descricao: a spec usa `Type...`, mas o parser so consome `DotDot` (`..`),
  deixando um `.` solto.
- Local provavel:
  - `docs/spec/03-grammar.ebnf:100`
  - `docs/spec/07-functions.md:71`
  - `compiler/crates/ori-parser/src/parse_item.rs:258`
- Esperado:
  - `func f(xs: int...)` deve parsear.
  - Variadic so pode ser o ultimo parametro.
- Atual observado:
  - `func f(xs: int..., y: int)` falha como `expected ')' found '.'`.
  - O erro nao e `parse.variadic_not_last`.
- Impacto: sintaxe documentada nao funciona com diagnostico proprio.
- Severidade: Alta.
- Sugestao:
  - Criar token `Ellipsis` para `...` ou consumir `DotDot` + `Dot`.
  - Validar que variadic e ultimo parametro.
  - Emitir `parse.variadic_not_last`.
- Testes recomendados:
  - Variadic valido como ultimo parametro.
  - Variadic no meio da lista.
  - Spread `..expr` em chamada deve continuar separado de `...`.

### 12. Default antes de parametro obrigatorio nao e rejeitado na declaracao

- Descricao: a spec exige que parametros com default venham depois dos
  obrigatorios. A declaracao invalida e aceita, e o erro aparece so em chamadas.
- Local provavel:
  - `docs/spec/07-functions.md:44`
  - `compiler/crates/ori-parser/src/parse_item.rs:268`
  - `compiler/crates/ori-types/src/check.rs:4006`
- Esperado:
  - `func f(a: int = 1, b: int)` deve gerar erro na declaracao.
- Atual observado:
  - A declaracao passa.
  - `f(2)` falha como aridade incorreta, sem explicar o problema real.
- Impacto: APIs invalidas entram no programa e produzem diagnosticos tardios.
- Severidade: Alta.
- Sugestao:
  - Validar ordem de parametros no parser ou no resolve.
  - Emitir diagnostico dedicado.
- Testes recomendados:
  - Default seguido de required.
  - Default seguido de variadic.
  - Parametro com contract sem default continua valido.

### 13. Closure captura `var`, embora a spec proiba

- Descricao: a spec diz que closures capturam `const` por copia e nao podem
  capturar `var`. O checker aceita captura de variavel mutavel.
- Local provavel:
  - `docs/spec/07-functions.md:255`
  - `compiler/crates/ori-types/src/check.rs:1531`
  - `compiler/crates/ori-hir/src/lower.rs:2694`
- Esperado:
  - `var n: int = 1; const f = do() => n` deve gerar erro.
- Atual observado:
  - `ori check` retorna `no errors`.
- Impacto: modelo de captura por valor fica ambiguuo. Pode capturar estado
  mutavel de forma nao documentada.
- Severidade: Alta.
- Sugestao:
  - Ao coletar free names da closure, consultar flags de binding no escopo.
  - Emitir `mut.closure_captures_var`.
- Testes recomendados:
  - Captura de `const` passa.
  - Captura de `var` falha.
  - Snapshot `const current = n; do() => current` passa.

### 14. Result descartado nao gera warning

- Descricao: a spec documenta warning quando `result` e descartado sem `?`.
- Local provavel:
  - `docs/spec/06-statements.md:366`
  - `docs/spec/13-error-catalog.md:242`
  - `compiler/crates/ori-types/src/check.rs:811`
- Esperado:
  - Chamada `may_fail()` como statement deve emitir `type.unused_result`.
- Atual observado:
  - `ori check` retorna `no errors`.
- Impacto: erros esperados podem ser ignorados sem sinal.
- Severidade: Alta.
- Sugestao:
  - Em `Stmt::Expr`, se o tipo inferido for `Ty::Result`, emitir warning.
  - Decidir se `optional` descartado tambem merece warning.
- Testes recomendados:
  - `result` descartado gera warning.
  - `result?` nao gera warning.
  - `const _ = may_fail()` segue a regra escolhida.

## Achados medios

### 15. Struct fields e enum variants duplicados nao sao diagnosticados

- Descricao: campos de struct e variantes de enum com nomes repetidos passam
  sem erro.
- Local provavel:
  - `compiler/crates/ori-parser/src/parse_item.rs:297`
  - `compiler/crates/ori-parser/src/parse_item.rs:346`
  - `compiler/crates/ori-types/src/resolve.rs`
- Esperado:
  - Nomes repetidos no mesmo tipo devem falhar.
- Atual observado:
  - `struct User { id: int; id: int }` retorna `no errors`.
  - `enum Status { Ready; Ready }` retorna `no errors`.
- Impacto: construtores, pattern matching e field lookup ficam ambiguos.
- Severidade: Media.
- Sugestao:
  - Validar unicidade em resolve/check.
  - Emitir codigo como `name.duplicate` ou codigo mais especifico.
- Testes recomendados:
  - Campo duplicado mesmo tipo.
  - Campo duplicado tipos diferentes.
  - Variante duplicada unit e com payload.

### 16. Diagnosticos de f-string podem apontar para span errado

- Descricao: expressoes interpoladas sao lexadas em substring com o mesmo
  `file_id`, mas spans relativos ao inicio da substring.
- Local provavel:
  - `compiler/crates/ori-parser/src/parse_expr.rs:865`
  - `compiler/crates/ori-parser/src/parse_expr.rs:931`
- Esperado:
  - Erro dentro de `{expr}` deve apontar para o local real dentro da f-string.
- Atual observado:
  - `f"prefix {#} suffix"` reporta `#` em `1:1` quando o arquivo tem BOM.
- Impacto: diagnostico confunde o usuario e ferramentas futuras.
- Severidade: Media.
- Sugestao:
  - Offsetar spans da lexagem/parsing interno pelo `base + expr_start`.
  - Ou passar uma source virtual com mapping para a source original.
- Testes recomendados:
  - Token invalido dentro de f-string.
  - Expressao incompleta dentro de f-string.
  - Mesmo caso com arquivo com BOM inicial.

### 17. README tem exemplo invalido e caracteres de controle

- Descricao: o README principal mostra exemplo com `ori.Error` e `io.write`,
  mas `ori.Error` ainda e planejado e `io.write` nao e API atual. O texto
  tambem tem caracteres quebrados em `namespace` e `result`.
- Local provavel:
  - `README.md:33`
  - `README.md:46`
  - `README.md:47`
  - `docs/IMPLEMENTATION_CHECKLIST.md:308`
- Esperado:
  - README deve ter exemplo minimo que compila hoje.
- Atual observado:
  - O exemplo falha em `type.undefined_name` para `ori.Error`.
  - `io.write` nao existe como contrato atual; a API real e `io.print`.
- Impacto: primeira experiencia do usuario quebra.
- Severidade: Media.
- Sugestao:
  - Trocar por exemplo com `func main()` e `io.print("hello from Ori")`.
  - Remover caracteres de controle.
- Testes recomendados:
  - Extrair bloco `ori` do README e rodar `ori check`.

### 18. `ori.mem` e documentado, mas nao existe no status de stdlib

- Descricao: `docs/spec/10-memory.md` fala em `std.mem`/`ori.mem`, mas o
  modulo nao aparece em implementados nem planejados.
- Local provavel:
  - `docs/spec/10-memory.md:186`
  - `docs/spec/12-stdlib.md:30`
  - `compiler/crates/ori-types/src/stdlib.rs`
- Esperado:
  - `ori.mem.size_of<T>()` e `align_of<T>()` existem ou sao marcados como
    planejados.
- Atual observado:
  - Nao ha runtime/checker para `ori.mem`.
- Impacto: documentacao de memoria promete API que nao existe.
- Severidade: Media.
- Sugestao:
  - Adicionar `ori.mem` como planejado no status da stdlib.
  - Ou implementar builtins compile-time.
- Testes recomendados:
  - Import de `ori.mem` deve passar ou falhar com `module_unavailable`.
  - `size_of<int>()` se implementado deve ser constante.

### 19. Checklist marca areas como completas, mas casos centrais ainda falham

- Descricao: `docs/IMPLEMENTATION_CHECKLIST.md` marca itens amplos como
  function call validation, closure capture, method resolution, native backend
  e runtime completeness. Os casos reproduzidos mostram lacunas ainda abertas.
- Local provavel:
  - `docs/IMPLEMENTATION_CHECKLIST.md`
- Esperado:
  - Checklist deve separar "caminho feliz implementado" de "contrato completo".
- Atual observado:
  - Suite passa, mas booleanos invalidos, nomes desconhecidos, mut methods,
    bounds e helper methods ainda divergem.
- Impacto: risco de fechamento falso de milestones.
- Severidade: Media.
- Sugestao:
  - Criar uma secao P20 no plano com estes achados.
  - Marcar checklist como parcial onde o contrato ainda nao esta coberto.
- Testes recomendados:
  - Converter cada achado deste relatorio em teste negativo ou E2E.

### 20. Ferramentas ainda sao placeholder

- Descricao: `ori-lsp` esta no workspace, mas ainda e placeholder. Formatter
  tambem segue pendente no checklist.
- Local provavel:
  - `compiler/crates/ori-lsp/src/main.rs:2`
  - `README.md:24`
  - `docs/IMPLEMENTATION_CHECKLIST.md:362`
- Esperado:
  - Se anunciado como ferramenta, deve implementar LSP minimo.
- Atual observado:
  - O proprio README diz que o LSP nao implementa o protocolo ainda.
- Impacto: baixo para compilador, medio para integracao com editor.
- Severidade: Baixa/Media.
- Sugestao:
  - Manter como planejado de forma visivel.
  - Quando iniciar, primeiro entregar `initialize`, `didOpen`, `didChange` e
    diagnostics via `ori check`.
- Testes recomendados:
  - Handshake LSP minimo.
  - Publicacao de diagnosticos para erro lexical e type error.

## Lacunas de testes automatizados

Adicionar regressao para:

- field assignment e mut method com efeito real.
- metodo sem `self` se a spec mantiver self implicito.
- nomes desconhecidos em valor, chamada e path qualificado.
- `panic`, `todo`, `unreachable`.
- `and`, `or`, `not` com operandos nao booleanos.
- `--|` dentro de string/byte/f-string.
- index out-of-bounds, slice invalido e repeat negativo.
- `.or`, `.or_return`, `.or_wrap`.
- traits de `ori.core`.
- `ori.list.contains` com tipo errado.
- variadic `...` e `parse.variadic_not_last`.
- default antes de required.
- closure capturando `var`.
- result descartado.
- campos/variantes duplicados.
- spans de erro dentro de f-string.
- blocos `ori` do README.
- import ou chamada de `ori.mem`.

## Recomendacao de prioridade

1. Corrigir parser de lvalue/field assignment. E o achado mais destrutivo.
2. Corrigir nomes desconhecidos para falhar no checker.
3. Validar operadores logicos antes do backend.
4. Resolver contrato de `self` em metodos.
5. Corrigir bounds/runtime panics ou atualizar a spec.
6. Implementar ou marcar como planejados os helpers `.or*`.
7. Criar core traits reais ou reduzir a documentacao.
8. Converter todos os achados em testes de regressao.

## Conclusao

O projeto melhorou muito desde a analise anterior, e a suite atual passando e
um bom sinal. Mas o estado ainda nao e "linguagem completa": ha bugs que fazem
codigo sumir do AST, erros de tipo chegarem ao backend e APIs documentadas
falharem em exemplos simples.

O proximo passo pragmatico e abrir uma nova fase no plano para estes achados,
com correcoes pequenas e testes focados antes de alterar mais a superficie da
linguagem.
