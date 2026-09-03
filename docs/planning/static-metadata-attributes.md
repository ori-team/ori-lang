# Plano de implementação — metadata estática e attributes extensíveis

> **Status:** baseline implemented; extensible third-party schemas remain
> planned and are not part of the stable surface.
> **Baseline verificada:** attributes built-in de nível superior, workspace
> `0.3.8-dev`.  
> **Princípio:** metadata descreve declarações; não é um segundo sistema de
> programação nem uma forma implícita de macro.

## 1. Resultado desejado

Permitir que ferramentas e frameworks consultem metadata tipada sobre módulos,
tipos, campos e funções sem reparsear Ori nem depender de structs Rust internas.

Fluxo preferido da primeira versão:

```text
fonte Ori
  → parser / resolver / checker
  → modelo semântico verificado
  → metadata versionada
  → LSP, docs, inspector ou gerador externo
```

Serializers, RPC ou ORMs podem consumir o export e gerar fonte `.orl`. O
compilador normal continua responsável por validar e compilar essa fonte.

## 2. Estado real antes deste plano

Ori já possui `@test`, `@deprecated`, `@inline`, `@no_inline`, `@cfg`, `@repr`
e `@c_export`.

Limitações atuais:

- attributes só existem em `ItemWithAttrs`, antes de declarações top-level;
- módulos, campos, variantes, parâmetros e métodos não têm coleção própria;
- argumentos genéricos aceitam string ou `nome: nome`; `@cfg` possui sua árvore
  estruturada própria;
- attributes desconhecidos, incluindo nomes com namespace sem schema, emitem
  `attr.unknown`;
- `@cfg` filtra declarações antes do resolver; `@inline` e `@no_inline`
  continuam sem efeito no otimizador;
- HIR preserva apenas efeitos específicos como `repr_c` e `c_export_name`;
- não há schema público de `ModuleInfo`, `TypeInfo` ou `FieldInfo`;
- `ori doc export` publica catálogo da linguagem/stdlib, não o modelo tipado de
  qualquer projeto do usuário.

`ATTR-REPR-1` foi fechado em 2026-08-09: somente `@repr("C")` é aceito; outras
formas emitem `attr.invalid_arg`.

`META-ATTR-1` foi fechado em 2026-09-01 pelo contrato fail-closed: a sintaxe
continua reservada para futura metadata versionada, mas nenhum nome
desconhecido é aceito silenciosamente. Os sete attributes built-in possuem
validação própria; schemas de terceiros continuam uma decisão futura e não
podem ser simulados por um bypass baseado apenas no nome.

`LANG-CFG-1` foi fechado em 2026-08-10 com predicados tipados, manifests,
CLI, cache e tooling. Este plano não deve recriar sua semântica; metadata de
terceiros apenas coexistirá com o atributo built-in.

## 3. Decisões de escopo

### 3.1 Metadata estática primeiro

A primeira versão não oferece reflection dinâmica obrigatória no executável.
Ferramentas consultam metadata durante build, LSP ou por comando de exportação.

### 3.2 Attributes built-in continuam reservados

Nomes sem namespace continuam pertencendo à linguagem:

```text
@test
@deprecated
@inline
@no_inline
@cfg
@repr
@c_export
```

Metadata de terceiros deve possuir namespace. Exemplo conceitual, ainda não
normativo:

```ori
@editor.inspect
@editor.range(min: 0.0, max: 100.0)
```

Isso evita colisões e mantém erros de digitação detectáveis.

### 3.3 Attributes não executam código

Argumentos serão valores constantes e limitados. Não haverá na primeira versão:

- execução arbitrária de funções;
- leitura de arquivos ou ambiente;
- alteração direta da AST;
- expansão de tokens;
- plugins nativos dentro do compilador;
- macros procedurais.

## 4. Alvos de metadata

| Alvo | Motivação | Fase inicial |
|---|---|---|
| Módulo | catálogo, capability, docs | sim |
| Struct / enum / alias / newtype | schema, bindings, docs | sim |
| Campo de struct | inspector, serialização, validação | sim |
| Variante e campo de enum | schema e bindings | sim |
| Função | docs, RPC, testes, tooling | sim |
| Parâmetro | CLI, RPC, validação | sim |
| Método / trait member | docs e geração de APIs | sim |
| Binding local / expressão | aumenta muito o AST e o custo cognitivo | fora da v1 |

## 5. Modelo de argumentos

O conjunto inicial deve reutilizar valores constantes que não causam efeitos:

- string;
- boolean;
- inteiros e floats com sufixo preservado;
- nome qualificado;
- listas pequenas desses valores, se um caso real exigir.

Questões que precisam de decisão explícita:

1. permitir expressão CT-0 ou somente literais;
2. ordem e duplicação dos argumentos nomeados;
3. tamanho e profundidade máximos;
4. representação de versões e type references;
5. se atributos repetíveis são definidos pelo schema ou proibidos por padrão.

Valores devem ser serializáveis de forma determinística e nunca depender do
endereço de um `DefId` interno.

## 6. Registro e resolução

Attributes customizados não devem ser aceitos apenas porque possuem um ponto.
A ferramenta precisa declarar ownership e schema.

Alternativas a decidir no primeiro RFC técnico:

| Alternativa | Vantagem | Custo |
|---|---|---|
| Manifest registra schemas | descoberta simples por projeto | amplia `ori.proj` / `ori.pkg.toml` |
| Pacote exporta declarações de metadata | acompanha a dependência | exige nova forma semântica no módulo |
| Arquivo sidecar versionado | não muda sintaxe Ori | mais um artefato para manter |

Até essa decisão, unknown attributes continuam sendo erro. Não introduzir um
modo global “ignore unknown”.

## 7. Schema de exportação

O export precisa ser estável e independente das structs Rust:

```text
schema_version
compiler_version
language_surface
module_id / module_name
source files
types
functions
fields / variants / parameters
attributes
source spans
visibility
canonical type expressions
```

IDs públicos devem ser derivados de nomes qualificados e assinatura, ou ser
opacos dentro do documento. Nunca serializar `DefId(N)` como identidade estável.

Comando alvo, ainda conceitual:

```text
ori metadata export projeto/ --format json --output metadata.json
```

O comando falha quando o projeto não resolve ou não passa pelo checker. Um modo
parcial para IDE pode existir depois, com nós explicitamente marcados como
incompletos.

## 8. Fases de implementação

| ID | Entrega | Critério observável |
|---|---|---|
| **META-ATTR-1.0** | Corrigir validação dos attributes built-in | **done 2026-08-09:** `@repr` aceita somente a forma suportada; catálogo e regressão concordam |
| **META-ATTR-1.1** | AST para novos alvos e valores tipados | parser preserva metadata em módulos, campos, variantes, métodos e parâmetros |
| **META-ATTR-1.2** | Schema e resolução de namespace | typo, colisão, target inválido e argumento inválido produzem diagnostics específicos |
| **META-ATTR-1.3** | Metadata semântica e export JSON | projeto real exporta tipos resolvidos e source spans sem `DefId` ou debug Rust |
| **META-ATTR-1.4** | Integração LSP/docs | hover e docs consultam o mesmo modelo; nenhuma ferramenta reparsa a fonte |
| **META-ATTR-1.5** | Gerador externo de referência | uma ferramenta pequena gera fonte Ori a partir de metadata e CI valida o ciclo |

## 9. Impacto por módulo

| Área | Mudança provável |
|---|---|
| Lexer | nenhuma palavra reservada nova; manter `@` |
| AST | metadata nos alvos e valores tipados |
| Parser | parse por alvo e limites de nesting/tamanho |
| Resolver/checker | schema, namespace, target e type references |
| HIR | modelo de metadata separado dos flags de codegen |
| Driver | comando de export e JSON versionado |
| LSP | leitura do modelo compartilhado |
| Formatter | preservar ordem e formato canônico |
| Docs/Atlas | novo contrato, diagnostics e exemplo canônico |

O codegen não precisa carregar toda metadata para o binário na primeira fase.

## 10. Testes obrigatórios

- lexer/parser para cada alvo;
- limites de nesting, quantidade e tamanho de strings;
- duplicate, unknown, wrong target e wrong argument;
- resolução cross-module e de dependência;
- export determinístico byte a byte;
- source spans em arquivos múltiplos;
- formatter idempotente;
- LSP sem parser paralelo;
- projeto de geração externa que compile a saída;
- fuzzing do parser de attributes e do schema de metadata.

## 11. Compatibilidade

- Attributes built-in mantêm seu significado atual.
- `@cfg` segue o ADR estruturado já implementado. Mudanças futuras nesse
  contrato ou um novo significado para `@inline` são trabalhos separados e
  precisam de testes em todos os backends suportados.
- Aceitar metadata customizada é aditivo na superfície, mas novos diagnostics
  podem tornar um nome antes desconhecido em válido; registrar no CHANGELOG.
- O schema JSON possui sua própria versão, separada de `ori-native-abi-1`.
- Alterar o schema de forma incompatível exige migração ou nova versão.

## 12. Fora de escopo

- derive implícito;
- reflection dinâmica completa;
- macros arbitrárias;
- código de engine dentro do compilador;
- tornar `@component`, `@entity`, `@route` ou equivalentes palavras da Ori;
- aceitar attributes desconhecidos sem declaração.
