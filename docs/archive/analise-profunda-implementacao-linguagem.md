# Analise profunda da implementacao da linguagem Ori

Data: 2026-05-12

Escopo: arquitetura, lexer, parser, semantica, HIR, codegen C, codegen nativo,
runtime, stdlib, diagnosticos e ferramentas.

Fonte normativa usada: `docs/spec/*.md`, com apoio de `docs/README.md`,
`docs/IMPLEMENTATION_CHECKLIST.md` e `docs/ARC_IMPLEMENTATION_PLAN.md`.

## Resumo executivo

A implementacao ja tem uma base ampla: parser, checker, HIR, backend C,
backend nativo, runtime, importacao multifile, traits, generics, closures,
contratos e uma suite automatizada grande.

Mesmo assim, a comparacao contra a documentacao oficial mostra problemas
criticos de corretude:

1. Literais numericos com sufixo ou overflow podem virar `0` silenciosamente.
2. Igualdade estrutural documentada nao esta implementada para structs,
   listas, mapas, sets, opcionais e results.
3. Funcoes sao comparaveis hoje, embora a spec diga que isso deve ser erro.
4. `break` e `continue` fora de loops passam no checker e viram no-op no
   backend nativo.
5. O operador `?` no backend C nao faz retorno antecipado.
6. Varias APIs documentadas da stdlib nao existem ou existem com outro tipo.
7. O comando `compile` promete nao precisar de compilador C, mas chama `cc`.

As validacoes gerais passam, o que indica lacuna de testes para esses pontos:

```text
cargo fmt --check        -> ok
cargo check --workspace  -> ok
cargo test --workspace   -> ok
git diff --check         -> ok, apenas avisos de fim de linha CRLF/LF
```

## Mapa rapido da arquitetura

Fluxo principal observado:

```text
source .orl
  -> ori-lexer
  -> ori-parser / ori-ast
  -> ori-types resolve + check
  -> ori-hir lower + monomorph
  -> ori-codegen C ou native backend
  -> ori-driver CLI, imports, manifests, runtime embedado e linker
```

Superficies de runtime:

- `compiler/crates/ori-runtime/src/lib.rs`: runtime Rust com ARC e funcoes
  exportadas.
- `compiler/crates/ori-driver/src/pipeline.rs`: runtime C embedado usado no
  caminho nativo para gerar `libori_rt.a`.
- `compiler/crates/ori-codegen/src/c_backend.rs`: runtime C inline usado pelo
  backend C de debug.

Essa duplicacao e uma causa recorrente de divergencia.

## Validacoes pontuais reproduzidas

### Literais numericos

Programa:

```ori
namespace app.main

func main()
    const a: float = 3.5f64
    const b: int = 9223372036854775808
end
```

Resultado do `ori build`:

```c
double a = 0;
int64_t b = INT64_C(0);
```

### Igualdade estrutural de struct

Programa:

```ori
namespace app.main
import ori.io as io

struct User
    id: int
end

func main()
    const a: User = User(id: 1)
    const b: User = User(id: 1)
    io.print(if a == b then "same" else "diff")
end
```

Resultado executado:

```text
diff
```

Pela spec, o esperado e `same`.

### Funcao comparavel

Programa com `f == g`, onde ambos sao `func(int) -> int`, passa no `check`
com `no errors`. Pela spec, funcao nao e comparavel.

### `break` fora de loop

Programa:

```ori
namespace app.main

func main()
    break
end
```

Resultado:

```text
ori check   -> no errors
ori compile -> no errors
```

No backend nativo, o `break` fora de loop nao emite salto porque nao existe
contexto de loop.

### BOM UTF-8

Arquivo salvo com BOM UTF-8 em Windows:

```text
error[lex.unexpected_character]: unexpected character `\u{feff}`
```

A spec diz que BOM no inicio do arquivo deve ser aceito e ignorado.

### `ori.iter`

Programa com `import ori.iter as iter` e `iter.map(...)` passa no `check`.
O `build` gera chamada para `iter_map(...)`, mas nao existe runtime para isso.
O `compile` nativo falha com:

```text
ori: missing function reference `iter.map` in native codegen
```

### `math.floor`

Programa:

```ori
namespace app.main
import ori.math as math

func main()
    const x: float = math.floor(1.5)
end
```

Resultado:

```text
error[type.type_mismatch]: type mismatch: expected `float`, found `int`
```

A spec documenta `math.floor(x: float) -> float`.

## Achados por prioridade

## P0 - Critico

### 1. Literais numericos podem ser corrompidos para zero

- Descricao objetiva: literais com sufixo de tipo e literais fora do intervalo
  sao baixados para `0` em vez de manter o valor correto ou emitir erro.
- Local provavel:
  - `compiler/crates/ori-hir/src/lower.rs:1762`
  - `compiler/crates/ori-hir/src/lower.rs:1770`
  - `compiler/crates/ori-hir/src/lower.rs:3079`
  - `compiler/crates/ori-types/src/check.rs:806`
  - `compiler/crates/ori-types/src/check.rs:807`
- Esperado pela documentacao:
  - `docs/spec/02-lexical.md:162` documenta sufixos inteiros.
  - `docs/spec/02-lexical.md:179` documenta sufixos float.
  - Literais invalidos ou fora de faixa devem gerar diagnostico, nao mudar de
    valor.
- Comportamento atual observado:
  - `3.5f64` vira `0.0`.
  - `9223372036854775808` vira `0`.
  - Hex/bin/oct com sufixo entram em `from_str_radix(...).unwrap_or(0)`.
- Impacto pratico: corrupcao silenciosa de dados. O programa compila e executa
  com valores diferentes dos escritos pelo usuario.
- Severidade: Critica.
- Sugestao de correcao:
  - Criar um parser de literal tipado antes do HIR, retornando valor, base,
    sufixo e diagnostico.
  - Remover `unwrap_or(0)` e `unwrap_or(0.0)` de paths de literal.
  - Validar faixa conforme sufixo: `u8`, `i8`, `u16`, `i16`, `u32`, `i32`,
    `u64`, `i64`, `f32`, `f64`.
  - Fazer o checker inferir tipo a partir do sufixo, nao sempre `int` ou
    `float`.
- Testes recomendados:
  - `42u8`, `42i32`, `0xFFu8`, `0b1111u8`, `3.14f32`, `3.14f64`.
  - Overflow por tipo: `256u8`, `128i8`, `9223372036854775808i64`.
  - Literal decimal sem sufixo dentro e fora de `int`.
  - Teste end-to-end `build` e `compile` garantindo que o valor emitido nao
    muda.

### 2. Igualdade estrutural documentada nao e implementada

- Descricao objetiva: `==` e `!=` em tipos compostos usam comparacao de
  ponteiro ou inteiro, nao igualdade estrutural.
- Local provavel:
  - `compiler/crates/ori-types/src/check.rs:1301`
  - `compiler/crates/ori-types/src/check.rs:1357`
  - `compiler/crates/ori-codegen/src/native_backend.rs:4091`
  - `compiler/crates/ori-codegen/src/native_backend.rs:4118`
  - `compiler/crates/ori-codegen/src/c_backend.rs:973`
  - `compiler/crates/ori-codegen/src/c_backend.rs:1874`
- Esperado pela documentacao:
  - `docs/spec/04-types.md:320` define igualdade estrutural.
  - `docs/spec/04-types.md:327` ate `docs/spec/04-types.md:331` definem
    igualdade de lista, mapa, set, optional e result.
  - `docs/spec/04-types.md:337` diz que `Equatable` sobrescreve `==`.
- Comportamento atual observado:
  - Duas structs `User(id: 1)` imprimem `diff`.
  - O checker aceita o operador porque os tipos sao iguais.
  - O backend nativo cai em `icmp` para valores nao string/float.
- Impacto pratico: comparacoes semanticas basicas retornam resultado errado.
  Isso quebra testes, regras de negocio, colecoes e contratos de linguagem.
- Severidade: Critica.
- Sugestao de correcao:
  - Introduzir lowering para funcoes de igualdade por tipo.
  - Gerar comparadores estruturais para structs, tuples, enums, optionals,
    results, listas, mapas e sets.
  - Consultar `Equatable.equals` quando existir implementacao explicita.
  - Rejeitar tipos que contenham campo nao comparavel.
- Testes recomendados:
  - Struct simples igual/diferente.
  - Struct aninhada com lista/map/optional/result.
  - Tuple e enum com payload.
  - Lista mesma ordem e ordem diferente.
  - Map e set independentes de ordem.
  - `Equatable` customizado alterando o criterio de igualdade.

### 3. Funcoes sao comparaveis apesar de proibidas pela spec

- Descricao objetiva: o checker aceita `f == g` para valores de funcao.
- Local provavel:
  - `compiler/crates/ori-types/src/check.rs:1357`
  - `compiler/crates/ori-codegen/src/native_backend.rs:4118`
- Esperado pela documentacao:
  - `docs/spec/04-types.md:335`: `func(...)` deve ser erro de compilacao em
    igualdade.
- Comportamento atual observado: `ori check` retorna `no errors` para
  `const same: bool = f == g`.
- Impacto pratico: a linguagem permite uma comparacao sem contrato semantico.
  O resultado tende a ser comparacao de ponteiro/closure, nao valor.
- Severidade: Critica.
- Sugestao de correcao:
  - Antes da regra `lt == rt`, rejeitar `Ty::Func`.
  - Rejeitar tambem tipos compostos que contenham `func` se nao houver
    `Equatable` customizado valido.
- Testes recomendados:
  - `func == func` deve emitir `type.comparison_not_supported`.
  - Struct com campo `func` e sem `Equatable` deve rejeitar `==`.
  - Struct com campo `func` e `Equatable` customizado deve usar `equals`.

### 4. `break` e `continue` fora de loop sao aceitos e viram no-op

- Descricao objetiva: o type checker nao valida contexto de loop; o backend
  nativo simplesmente nao emite salto quando nao ha loop ativo.
- Local provavel:
  - `compiler/crates/ori-types/src/check.rs:590`
  - `compiler/crates/ori-types/src/check.rs:796`
  - `compiler/crates/ori-hir/src/lower.rs:1558`
  - `compiler/crates/ori-codegen/src/native_backend.rs:2318`
  - `compiler/crates/ori-codegen/src/native_backend.rs:2328`
- Esperado pela documentacao:
  - `docs/spec/06-statements.md` define `break` e `continue` como controle do
    loop mais interno.
- Comportamento atual observado:
  - `break` no corpo de `main` passa no `check`.
  - `compile` nativo gera binario sem erro.
- Impacto pratico: codigo invalido e aceito. O usuario acha que esta saindo
  de algo, mas o backend ignora o comando.
- Severidade: Critica.
- Sugestao de correcao:
  - Adicionar `loop_depth` ao checker.
  - Incrementar em `while`, `while some`, `for`, `loop` e `repeat`.
  - Emitir diagnostico para `break`/`continue` quando `loop_depth == 0`.
  - Adicionar assert/falha no backend se HIR invalido chegar ao codegen.
- Testes recomendados:
  - `break` fora de loop.
  - `continue` fora de loop.
  - Ambos dentro de loop.
  - Ambos dentro de bloco aninhado dentro de loop.
  - Ambos dentro de closure dentro de loop, se a regra da linguagem proibir
    capturar o controle do loop externo.

### 5. Operador `?` no backend C nao faz retorno antecipado

- Descricao objetiva: o backend C transforma `expr?` em acesso ao payload, mas
  nao verifica erro/none e nao retorna do escopo atual.
- Local provavel:
  - `compiler/crates/ori-codegen/src/c_backend.rs:1088`
  - `compiler/crates/ori-codegen/src/native_backend.rs:3573`
- Esperado pela documentacao:
  - `docs/spec/09-errors.md` define que `?` propaga `none` ou `error(e)` com
    retorno antecipado.
  - `docs/spec/06-statements.md` e `docs/spec/10-memory.md` exigem cleanup em
    saidas antecipadas.
- Comportamento atual observado:
  - Backend nativo tem ramo dedicado para propagacao.
  - Backend C apenas emite `{}.value.ok` ou `{}.value`.
- Impacto pratico: C gerado executa caminho errado em erro, pode ler payload
  invalido e tambem pula cleanup esperado.
- Severidade: Critica para o backend C; Alta para a arquitetura geral.
- Sugestao de correcao:
  - Fazer `Propagate` no backend C emitir bloco temporario com teste de tag.
  - Em caso de erro/none, emitir cleanup de escopo e `return`.
  - Se o backend C for apenas debug, bloquear `build` para programas com `?`
    ate ter suporte correto.
- Testes recomendados:
  - `result<T,E>?` em funcao que retorna `result<T,E>`.
  - `optional<T>?` em funcao que retorna `optional<T>`.
  - `?` dentro de `using`, validando chamada de `dispose`.
  - Mesmo programa em backend C e backend nativo, comparando saida.

## P1 - Alta

### 6. BOM UTF-8 documentado como aceito, mas rejeitado pelo lexer

- Descricao objetiva: arquivo com BOM no inicio falha em `lex.unexpected_character`.
- Local provavel:
  - `compiler/crates/ori-lexer/src/lexer.rs:31`
  - `compiler/crates/ori-lexer/src/token.rs:40`
- Esperado pela documentacao:
  - `docs/spec/02-lexical.md:12`: BOM no inicio do arquivo e aceito e
    ignorado.
- Comportamento atual observado:
  - `\u{feff}` na primeira coluna vira erro lexical.
- Impacto pratico: arquivos salvos por editores Windows podem falhar sem o
  usuario entender que o problema e invisivel.
- Severidade: Alta.
- Sugestao de correcao:
  - Remover BOM no driver antes do lexer ou fazer o lexer iniciar em
    `source.strip_prefix('\u{feff}')`.
  - Ajustar spans para que linha/coluna continuem corretas.
- Testes recomendados:
  - Arquivo com BOM e programa minimo.
  - BOM no meio do arquivo deve continuar sendo erro.
  - Teste de snapshot do diagnostico para caractere invisivel.

### 7. Modulos de stdlib documentados passam pelo import, mas nao existem

- Descricao objetiva: `is_stdlib_import` aceita qualquer `ori.*`, mas muitos
  modulos documentados ou planejados nao tem tipos, lowering nem runtime.
- Local provavel:
  - `compiler/crates/ori-driver/src/pipeline.rs:378`
  - `compiler/crates/ori-hir/src/lower.rs:15`
  - `docs/spec/12-stdlib.md:144`
  - `docs/IMPLEMENTATION_CHECKLIST.md:252`
- Esperado pela documentacao:
  - `docs/spec/12-stdlib.md` apresenta `ori.iter`, `ori.format`, `ori.time`,
    `ori.random`, `ori.json`, `ori.test`, `ori.os` e `ori.Error` como APIs.
- Comportamento atual observado:
  - `ori.iter` passa no `check`.
  - `build` gera `iter_map(...)`.
  - `compile` falha com `missing function reference 'iter.map'`.
  - A checklist marca esses modulos como planejados, mas a spec os apresenta
    como contrato atual.
- Impacto pratico: o usuario confia na documentacao, escreve codigo aceito
  pelo checker e so descobre o problema no codegen/link.
- Severidade: Alta.
- Sugestao de correcao:
  - Tornar `is_stdlib_import` uma allowlist real de modulos implementados.
  - Para modulo planejado, emitir diagnostico claro: `stdlib.module_unavailable`.
  - Ou mover APIs planejadas da spec normativa para documento de roadmap.
- Testes recomendados:
  - Import de cada modulo implementado deve passar.
  - Import de modulo planejado deve emitir diagnostico claro.
  - Chamada `iter.map` deve funcionar end-to-end ou falhar no checker, nunca
    apenas no codegen.

### 8. APIs `ori.string` e `ori.math` divergem da spec

- Descricao objetiva: nomes e tipos documentados nao batem com as assinaturas
  implementadas.
- Local provavel:
  - `docs/spec/12-stdlib.md:113`
  - `docs/spec/12-stdlib.md:122`
  - `docs/spec/12-stdlib.md:190`
  - `compiler/crates/ori-hir/src/lower.rs:63`
  - `compiler/crates/ori-runtime/src/lib.rs`
  - `compiler/crates/ori-driver/src/pipeline.rs`
- Esperado pela documentacao:
  - `string.trim_start`, `string.trim_end`, `string.parse_int`,
    `string.parse_float`.
  - `math.floor(x: float) -> float`, `ceil -> float`, `round -> float`.
  - `math.abs/min/max` com suporte int e float.
  - `math.clamp`, `math.log2`, `math.infinity`, `math.nan`, `math.is_nan`,
    `math.is_infinite`.
- Comportamento atual observado/inferido:
  - `math.floor(1.5)` retorna `int`, causando erro ao atribuir para `float`.
  - Conversoes existem como `string_to_int` / `ori.convert.string_to_int`,
    retornando `optional<int>`, nao `result<int,string>`.
  - Parte das funcoes listadas na spec nao aparece em `stdlib_c_name`.
- Impacto pratico: exemplos escritos pela spec nao compilam ou compilam com
  tipos diferentes do contrato publico.
- Severidade: Alta.
- Sugestao de correcao:
  - Escolher contrato unico: implementar spec ou corrigir spec.
  - Se a linguagem quer `floor -> int`, atualizar doc e exemplos.
  - Se a spec esta correta, ajustar checker, HIR, runtime e testes.
- Testes recomendados:
  - Teste oficial para cada funcao em `docs/spec/12-stdlib.md`.
  - Teste de tipo de retorno para `floor`, `ceil`, `round`.
  - Teste de erro/result para `parse_int` e `parse_float`.

### 9. Mapas e sets nao aplicam contrato `Hashable`/`Equatable`

- Descricao objetiva: a spec exige `Hashable` para chaves de mapa e elementos
  de set, mas a implementacao usa armazenamento `i64` e nao valida traits.
- Local provavel:
  - `docs/spec/04-types.md:129`
  - `docs/spec/04-types.md:130`
  - `compiler/crates/ori-codegen/src/native_backend.rs:1028`
  - `compiler/crates/ori-codegen/src/native_backend.rs:1030`
  - `compiler/crates/ori-runtime/src/lib.rs:881`
  - `compiler/crates/ori-runtime/src/lib.rs:914`
- Esperado pela documentacao:
  - Chaves de `map<K,V>` devem implementar `Hashable`.
  - Elementos de `set<T>` devem implementar `Hashable`.
  - Igualdade de map/set deve ser estrutural e independente de ordem.
- Comportamento atual inferido:
  - Runtime nativo cobre chaves `int` e `string` em `map<K,V>`.
  - Runtime nativo cobre elementos `int` e `string` em `set<T>`.
  - `string` usa hash/equality textual no runtime nativo e no runtime C
    embedado para `map` e `set`.
  - Chaves de tipos definidos pelo usuario ainda nao passam por uma ABI generica
    de `Hashable`/`Equatable`.
  - Elementos de `set<T>` definidos pelo usuario ainda nao passam por uma ABI
    generica de `Hashable`/`Equatable`.
- Impacto pratico: mapas e sets funcionam para casos simples, mas a semantica
  generica documentada nao esta garantida.
- Severidade: Alta.
- Sugestao de correcao:
  - Adicionar constraints de `Hashable` no checker para map/set.
  - Definir ABI generica de hash/equality por tipo.
  - Reusar `Equatable` para colisao e `Hashable.hash()` para bucket.
- Testes recomendados:
  - `map<string,int>` com duas strings iguais por valor deve continuar passando.
  - `set<string>` com duas strings iguais por valor deve continuar removendo
    duplicatas.
  - `map<struct,int>` sem `Hashable` deve falhar.
  - `map<struct,int>` com `Hashable` e `Equatable` deve funcionar.
  - `set<T>` deve remover duplicatas por valor.

### 10. `ori compile` promete nao precisar de C, mas chama `cc`

- Descricao objetiva: o help diz que `compile` usa Cranelift sem compilador C,
  mas o pipeline compila runtime C embedado e chama o linker `cc`.
- Local provavel:
  - `compiler/crates/ori-driver/src/main.rs:39`
  - `compiler/crates/ori-driver/src/pipeline.rs:1792`
  - `compiler/crates/ori-driver/src/pipeline.rs:1803`
  - `compiler/crates/ori-codegen/src/native_backend.rs:4175`
- Esperado pela documentacao/CLI:
  - `Compile to a native binary via Cranelift (no C compiler needed).`
- Comportamento atual observado:
  - `build_runtime_lib()` chama `cc -c`.
  - `link()` chama `cc`.
- Impacto pratico: instalacoes sem toolchain C falham, principalmente em
  Windows limpo, contradizendo a promessa da CLI.
- Severidade: Alta.
- Sugestao de correcao:
  - Atualizar help para dizer que `cc`/linker e necessario.
  - Ou trocar runtime C embedado por objeto/runtime precompilado ou linker
    Rust puro.
  - Adicionar checagem antecipada com mensagem curta e acionavel.
- Testes recomendados:
  - Ambiente sem `cc` no PATH deve gerar diagnostico claro.
  - Teste do texto de help.
  - Teste de compile nativo em Windows documentando prerequisitos.

### 11. Backend C e runtime inline estao atrasados em relacao ao backend nativo

- Descricao objetiva: o backend C tem runtime inline reduzido, ARC no-op e
  semantica incompleta para features ja suportadas no nativo.
- Local provavel:
  - `compiler/crates/ori-codegen/src/c_backend.rs:70`
  - `compiler/crates/ori-codegen/src/c_backend.rs:71`
  - `compiler/crates/ori-codegen/src/c_backend.rs:72`
  - `docs/ARC_IMPLEMENTATION_PLAN.md:17`
  - `docs/ARC_IMPLEMENTATION_PLAN.md:32`
- Esperado pela documentacao:
  - `docs/ARC_IMPLEMENTATION_PLAN.md` reconhece que C backend nao e ARC
    completo.
  - `docs/spec/10-memory.md` define ARC e cleanup como parte do modelo da
    linguagem.
- Comportamento atual inferido:
  - Hooks ARC no C backend sao no-op.
  - O backend C tem suporte parcial e emite erros para alguns recursos.
  - Outros recursos podem gerar C aparentemente valido, mas semanticamente
    divergente.
- Impacto pratico: `ori build` pode produzir C que parece ser uma referencia,
  mas nao representa a semantica real da linguagem.
- Severidade: Alta.
- Sugestao de correcao:
  - Documentar `build` como backend de debug parcial, se essa for a intencao.
  - Ou criar uma unica fonte de runtime compartilhada entre C debug e nativo.
  - Bloquear features sem paridade com diagnosticos claros.
- Testes recomendados:
  - Matrix de paridade: mesmo programa em `build` + compilacao C e `compile`.
  - Casos com ARC, `using`, `?`, closures, stdlib e colecoes.

## P2 - Media

### 12. Traits de operadores estao documentados, mas nao alimentam operadores

- Descricao objetiva: `Equatable`, `Comparable`, `Addable` e similares sao
  descritos como fonte de operadores, mas `infer_binary` nao despacha para
  metodos de trait.
- Local provavel:
  - `docs/spec/08-traits.md:156`
  - `docs/spec/08-traits.md:164`
  - `docs/spec/08-traits.md:179`
  - `compiler/crates/ori-types/src/check.rs:1301`
  - `compiler/crates/ori-codegen/src/native_backend.rs:4091`
- Esperado pela documentacao:
  - `<`, `<=`, `>`, `>=` derivam de `Comparable.compare`.
  - `==`, `!=` derivam de `Equatable.equals`.
- Comportamento atual inferido:
  - Operadores aceitam apenas tipos primitivos ou tipos iguais.
  - Implementacoes de trait nao sao usadas para operadores binarios.
- Impacto pratico: o sistema de traits existe, mas nao cumpre uma parte
  importante do contrato ergonomico da linguagem.
- Severidade: Media, com potencial de Alta quando `Equatable` virar API
  publica usada.
- Sugestao de correcao:
  - Resolver operador como chamada de metodo de trait quando aplicavel.
  - Definir precedencia entre operador nativo, igualdade estrutural e trait
    customizada.
- Testes recomendados:
  - `Comparable` customizado para struct.
  - `Equatable` customizado para comparar apenas `id`.
  - Erro claro quando trait exigida nao esta implementada.

### 13. Catalogo de diagnosticos nao cobre todos os codigos emitidos

- Descricao objetiva: alguns codigos emitidos no codigo nao aparecem em
  `docs/spec/13-error-catalog.md`.
- Local provavel:
  - `docs/spec/13-error-catalog.md`
  - `compiler/crates/ori-types/src/check.rs`
  - `compiler/crates/ori-types/src/resolve.rs`
  - `compiler/crates/ori-lexer/src/lexer.rs`
- Esperado pela documentacao:
  - O catalogo deve listar os codigos estaveis emitidos.
- Comportamento atual observado:
  - Codigos emitidos ausentes do catalogo:
    - `name.duplicate`
    - `name.undefined`
    - `type.tuple_index_out_of_bounds`
    - `type.undefined_name`
  - `lex.unexpected_character` tambem e emitido e precisa estar claramente
    catalogado se for codigo publico.
- Impacto pratico: ferramentas, LSP, docs e testes snapshot ficam sem fonte
  unica para diagnosticos.
- Severidade: Media.
- Sugestao de correcao:
  - Adicionar check automatizado que compara codigos emitidos vs catalogo.
  - Decidir se codigos nao catalogados sao oficiais ou devem ser renomeados.
- Testes recomendados:
  - Script em CI para falhar quando um codigo novo nao aparece no catalogo.
  - Snapshot de diagnosticos principais.

### 14. Comentarios de documentacao e atributos sao parseados, mas quase sem semantica

- Descricao objetiva: a spec documenta `ori doc`, validacao de `@param`,
  `@test` e `@deprecated`, mas a implementacao observada so parseia atributos.
- Local provavel:
  - `docs/spec/02-lexical.md:49`
  - `docs/spec/02-lexical.md:78`
  - `docs/spec/02-lexical.md:299`
  - `docs/spec/13-error-catalog.md:219`
  - `compiler/crates/ori-parser/src/parse_item.rs:91`
  - `compiler/crates/ori-ast/src/common.rs:115`
- Esperado pela documentacao:
  - `@param` validado contra parametros reais.
  - `@deprecated` emite warning no uso.
  - `@test` e executado por `ori test`.
  - Custom attributes nao suportados devem ser erro.
- Comportamento atual inferido:
  - A AST guarda atributos.
  - Nao ha comando `ori doc` ou `ori test` no CLI atual.
  - Nao foi encontrada validacao semantica para `@deprecated`, `@test` ou
    custom attr.
- Impacto pratico: documentacao promete ferramentas de DX que nao existem.
- Severidade: Media.
- Sugestao de correcao:
  - Marcar essa secao como planejada ou implementar validacao minima.
  - Criar diagnosticos `attr.unknown`, `attr.invalid_target`,
    `attr.deprecated`.
  - Adicionar comandos CLI apenas quando houver comportamento real.
- Testes recomendados:
  - `@deprecated` em funcao e uso gerando warning.
  - `@test` em funcao valida.
  - `@test` em struct deve falhar.
  - `@param` inexistente deve gerar warning.

### 15. LSP existe como crate, mas e apenas placeholder

- Descricao objetiva: `ori-lsp` faz parte do workspace, mas o binario so diz
  que nao esta implementado.
- Local provavel:
  - `compiler/crates/ori-lsp/src/main.rs:1`
  - `compiler/crates/ori-lsp/src/main.rs:3`
- Esperado pela documentacao/ferramentas:
  - Um crate `ori-lsp` no workspace sugere integracao com editor, ao menos com
    diagnosticos basicos.
- Comportamento atual observado:
  - `ori-lsp` imprime `not yet implemented`.
- Impacto pratico: integracao com ferramentas nao existe ainda.
- Severidade: Media se anunciado como ferramenta; Baixa se tratado como
  placeholder interno.
- Sugestao de correcao:
  - Marcar explicitamente como futuro no README/checklist.
  - Ou implementar LSP minimo: initialize, didOpen, didChange, diagnostics.
- Testes recomendados:
  - Teste de handshake LSP.
  - Teste de diagnostico para arquivo com erro lexical e type error.

### 16. Comentarios de bloco nao fechados viram erro generico

- Descricao objetiva: existe funcao `check_unclosed_block_comments`, mas ela
  nao diagnostica o caso; o Logos retorna erro generico.
- Local provavel:
  - `compiler/crates/ori-lexer/src/lexer.rs:59`
  - `compiler/crates/ori-lexer/src/lexer.rs:65`
  - `docs/spec/13-error-catalog.md:241`
- Esperado pela documentacao:
  - Catalogo define `doc.unclosed_block` para comentario de bloco nao fechado.
- Comportamento atual inferido:
  - Comentario nao fechado tende a virar `lex.unexpected_character`.
- Impacto pratico: mensagem menos clara e catalogo nao usado.
- Severidade: Media-baixa.
- Sugestao de correcao:
  - Implementar diagnostico dedicado para bloco aberto sem fechamento.
  - Diferenciar comentario comum de comentario de documentacao, se necessario.
- Testes recomendados:
  - `--|` sem `|--`.
  - Bloco fechado corretamente.
  - Bloco contendo marcadores parecidos.

### 17. Runtime tem tres fontes de verdade

- Descricao objetiva: ha runtime Rust, runtime C embedado no driver e runtime
  C inline no backend C.
- Local provavel:
  - `compiler/crates/ori-runtime/src/lib.rs`
  - `compiler/crates/ori-driver/src/pipeline.rs:544`
  - `compiler/crates/ori-codegen/src/c_backend.rs`
- Esperado pela arquitetura:
  - Uma linguagem com dois backends precisa de contrato ABI unico e testado.
- Comportamento atual inferido:
  - Funcoes aparecem em uma superficie e nao em outra.
  - A assinatura de stdlib precisa ser sincronizada manualmente em varios
    lugares.
- Impacto pratico: regressao facil quando uma stdlib nova e adicionada em um
  caminho, mas esquecida nos outros.
- Severidade: Media.
- Sugestao de correcao:
  - Criar manifest ABI/stdlib unico gerado para checker, HIR e codegen.
  - Reduzir runtime inline ou gerar a partir de uma fonte compartilhada.
  - Adicionar teste que lista funcoes usadas pelo HIR e confirma declaracao em
    todos os runtimes relevantes.
- Testes recomendados:
  - Consistencia de nomes `stdlib_c_name` vs runtime exportado.
  - Consistencia de assinaturas.
  - Compile/link de fixtures que usam cada funcao documentada.

## P3 - Baixa ou documentacao

### 18. ARC com ciclo e C backend parcial estao documentados, mas precisam de cerca de seguranca

- Descricao objetiva: a documentacao ja reconhece que cycle collection nao
  existe e que o C backend nao e ARC completo, mas o usuario ainda pode tratar
  o C gerado como equivalente.
- Local provavel:
  - `docs/spec/10-memory.md:72`
  - `docs/ARC_IMPLEMENTATION_PLAN.md:5`
  - `docs/ARC_IMPLEMENTATION_PLAN.md:38`
  - `compiler/crates/ori-runtime/src/lib.rs:68`
- Esperado pela documentacao:
  - Ciclos nao sao coletados por enquanto.
  - C backend nao deve ser descrito como ARC-completo.
- Comportamento atual observado:
  - `ori_arc_collect_cycles()` retorna `0`.
  - Teste do runtime confirma adiamento explicito.
- Impacto pratico: nao e bug oculto, mas e risco se exemplos ou docs futuras
  omitirem a limitacao.
- Severidade: Baixa como divergencia; Alta se o C backend for apresentado como
  target de producao.
- Sugestao de correcao:
  - Manter aviso visivel em `ori build`.
  - Adicionar docs de limites de memoria por backend.
  - Criar tracking issue/checklist para paridade ARC.
- Testes recomendados:
  - Fixture que mostra ciclo vazando deve ficar marcada como comportamento
    conhecido.
  - Teste de aviso/documentacao para backend C parcial.

## Lacunas principais de testes automatizados

- Literais numericos com sufixo e overflow.
- BOM UTF-8 no inicio do arquivo.
- `break` e `continue` fora de loop.
- Igualdade estrutural por tipo composto.
- Proibicao de comparar funcoes.
- Operadores derivados de traits.
- Paridade `?` entre backend C e backend nativo.
- Matrix completa da stdlib contra `docs/spec/12-stdlib.md`.
- Checagem automatica de codigos de diagnostico contra catalogo.
- Import de modulos `ori.*` nao implementados.
- `ori compile` em ambiente sem `cc`.
- LSP minimo, se o crate for mantido no workspace como ferramenta oficial.

## Recomendacao de ordem de correcao

1. Corrigir literais numericos. E corrupcao silenciosa.
2. Bloquear `break`/`continue` fora de loop. E erro simples e de alto impacto.
3. Bloquear comparacao de funcao e implementar base de comparabilidade.
4. Implementar igualdade estrutural ou rejeitar tipos compostos ate haver
   suporte correto.
5. Corrigir `?` no backend C ou bloquear `?` no `ori build`.
6. Transformar imports de stdlib em allowlist real.
7. Sincronizar `docs/spec/12-stdlib.md` com o que existe hoje.
8. Corrigir contrato do CLI sobre dependencia de `cc`.
9. Adicionar checks automatizados para catalogo de diagnosticos e ABI de
   stdlib.
10. Decidir se backend C e target real ou apenas ferramenta de debug.

## Nota final

A suite atual passar e um bom sinal de estabilidade nos caminhos cobertos.
Mas varios problemas acima foram reproduzidos em programas pequenos e passam
pela suite. A prioridade agora deve ser transformar esses casos em testes de
regressao antes ou junto da correcao, para evitar que a documentacao e a
implementacao voltem a divergir.
