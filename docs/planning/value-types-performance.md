# Plano de implementação — performance de value types

> **Status:** `VALUE-PERF-1` **done** para o escopo aprovado de baseline
> (auditado em 2026-08-25); otimizações de representação ficam shelved como
> candidatas P2+ até existir ganho mensurável e critério de produto.
> **Baseline verificada:** arrays inline, monomorfização, operator traits e
> mid-end básico já implementados em `0.3.8-dev`.  
> **Objetivo:** tornar abstrações pequenas baratas sem criar tipos mágicos para
> vetores, games ou gráficos.

### Estado verificado em 2026-08-25

O item P1 aprovado no backlog foi o baseline canônico de três kernels, não a
execução automática de todas as fases propostas neste documento. Esse escopo
está fechado por `vec3_add_loop`, `mat3_multiply`, `optional_scalar_loop` e
`tools/bench/run_value_perf.sh`. A auditoria também verificou que não existe
evidência suficiente para promover mudanças de representação a P1:

| Proposta original | Estado após o fechamento | Evidência / condição de reabertura |
|---|---|---|
| `1.0` | **done no escopo aprovado** | os três kernels canônicos e o runner são permanentes; ampliar para os nove kernels, AOT/JIT release, contadores, CLIF e tamanho é P2+ e só volta com uma decisão de medição específica |
| `1.1` | **não promovida; candidata P2** | `ori-types::is_inline_ty*` já cobre escalares/arrays/structs, mas HIR e native codegen repetem parte da classificação; centralizar só entra junto de uma otimização medida que consuma o contrato |
| `1.2` | **não promovida; candidata P2** | arrays e structs aninhadas inline têm layout, mas `HirExprKind::StructLit` geral ainda usa `malloc_typed_bytes`; reabrir exige demonstrar allocations e wall time no kernel-alvo, mais ABI/AOT/JIT gates |
| `1.3` | **não promovida; candidata P2** | unit/payload enum inline exige primeiro um gate de allocation e match que demonstre ganho |
| `1.4` | **capacidade conservadora já existente** | leaf inlining monomórfico existe em `ORI_OPT=aggressive`; expansão para attributes/operators precisa de benchmark e orçamento de código antes de virar trabalho |
| `1.5` | **shelved P2+** | escape analysis geral só reabre com workload onde aggregates heap dominem o perfil |
| `1.6` | **fora deste ID** | operators heterogêneos são decisão de linguagem separada, não otimização implícita |
| `1.7` | **shelved P2+** | especialização de `optional`/`result` só reabre após baseline mostrar custo material e preservar ABI-1 |

Assim, não resta P1 aberto em `VALUE-PERF-1`. As limitações acima são
documentadas para impedir claims de zero-allocation, mas não constituem um
programa ativo sem medição, critério e prioridade aprovados.

## 1. Problema

Ori já permite código legível com structs e operator traits. A stdlib possui
`Vec2`, `Vec3` e `Mat3`. Porém, o backend nativo atual aloca construções de
struct e enum por meio do heap ARC.

Isso significa que um workload semelhante a:

```ori
const next: Vec3 = vec3.scale(direction, speed)
```

pode pagar alocação, registro ARC e chamada de função mesmo quando o valor é
pequeno, escalar e não escapa do escopo.

Além disso:

- `@inline` e `@no_inline` ainda não controlam o optimizer;
- leaf inlining é intramódulo e somente em `ORI_OPT=aggressive`;
- operators aceitam atualmente operandos do mesmo tipo;
- `Vec3 * float` usa helper explícito, não o operator trait;
- enums usam tagged union com tag `i32`, mas o valor também é alocado;
- `optional` e `result` têm layout tag + payload e wrappers gerenciados em
  vários caminhos;
- fixed arrays já ficam inline, mas só aceitam elementos escalares.

## 2. Resultado desejado

Uma abstração pequena definida pelo usuário deve produzir código próximo de
campos escalares diretos quando:

- o tipo contém apenas dados inline-safe;
- o valor não escapa;
- identidade de endereço não é observável;
- não há custom destructor ou child ARC que exija heap;
- a transformação preserva contratos, debugging e ABI pública.

Não existe meta de “zero-cost” sem benchmark e inspeção do código gerado.

## 3. Invariantes semânticas

Qualquer otimização deve preservar:

- ordem de avaliação;
- side effects e contracts;
- resultado de pattern matching;
- ownership de campos gerenciados;
- comportamento de mutabilidade;
- stack traces e variáveis do debugger quando debug estiver ativo;
- `ori-native-abi-1` nas fronteiras já publicadas;
- paridade AOT/JIT do backend nativo.

Uma representação interna pode mudar sem alterar a superfície. A representação
externa de `@c_export` continua sendo a definida na Spec 19.

## 4. Suite de medição inicial

Antes de qualquer lowering novo, criar workloads canônicos:

| Kernel | O que mede |
|---|---|
| `vec3_add_loop` | criação + soma de structs escalares |
| `vec3_scale_loop` | struct × scalar via helper atual |
| `vec3_trait_loop` | dispatch estático de operator trait |
| `mat3_multiply` | cópias e temporários maiores |
| `fieldless_enum_state` | tag e match sem payload |
| `payload_enum_state` | tagged union com escalares |
| `optional_scalar_loop` | tag + payload pequeno |
| `result_scalar_loop` | branches e payloads pequenos |
| `array_vec_like` | baseline inline sem heap |

Para cada kernel registrar:

- tempo AOT release e JIT;
- alocações e releases;
- bytes alocados;
- tamanho do binário;
- CLIF gerado;
- comportamento com optimizer default/aggressive;
- comparação com C ou Rust equivalente somente como referência, não ranking.

## 5. Classificação de tipos

O compilador precisa de uma classificação explícita, não de regras espalhadas:

```text
InlineSafe
Managed
OpaqueRuntime
NeedsDestructor
UnknownGeneric
```

Critérios candidatos para `InlineSafe`:

- números e `bool`;
- arrays fixos de elementos inline-safe;
- structs não genéricas ou já monomorfizadas cujos campos são inline-safe;
- enums cujos payloads são inline-safe;
- newtypes sobre representação inline-safe.

Strings, bytes, coleções, `any[Trait]`, closures, futures e opaque runtime
handles permanecem pointer-shaped. Tipos com custom destructor não mudam de
representação sem uma regra de lifetime específica.

## 6. Frentes técnicas

### 6.1 Stack/SSA lowering

Construções pequenas e não escapantes podem viver em stack slot ou valores SSA
em vez de `ori_alloc`.

### 6.2 Escape analysis

O valor escapa quando, por exemplo:

- é retornado sem uma ABI inline definida;
- é armazenado num objeto gerenciado;
- é capturado por closure;
- cruza async/await;
- atravessa FFI como handle;
- seu endereço é observado.

A primeira implementação pode ser conservadora: qualquer dúvida mantém heap.

### 6.3 Scalar replacement

Um `Vec3` local pode ser representado como três escalares independentes quando
isso eliminar stores/loads sem quebrar debug ou contracts.

### 6.4 Inlining

Evoluir em ordem:

1. fazer `@inline`/`@no_inline` chegar ao HIR;
2. cobrir chamadas estáticas de métodos/operators;
3. incluir funções pequenas no optimizer default quando o custo de compilação
   for aceitável;
4. registrar summaries para cross-module inlining;
5. medir crescimento de código e deduplicação.

### 6.5 Operators heterogêneos

Permitir `Vec3 * float` requer uma decisão de linguagem separada. Modelo
conceitual:

```text
Multiplicable[Rhs, Output]
```

Isso afeta parser de traits aplicáveis, checker, HIR, monomorfização, stdlib e
diagnostics. Não deve ser escondido como “otimização”. Até a decisão, helpers
como `vec3.scale(v, s)` permanecem a forma válida.

### 6.6 Enums e wrappers

- enum sem payload pode usar apenas discriminant internamente;
- enum com payload continua tagged union;
- `optional`/`result` podem usar representação interna especializada quando a
  ABI pública não observa o layout;
- niche optimization só entra após prova de benefício e matriz de tipos;
- nenhum usuário depende da representação otimizada.

## 7. Fases de implementação

| ID | Entrega | Critério observável |
|---|---|---|
| **VALUE-PERF-1.0** | Benchmarks e contadores de alocação | baseline reproduzível e versionada para todos os kernels |
| **VALUE-PERF-1.1** | Classificação `InlineSafe` central | checker/HIR/codegen concordam; nenhuma heurística duplicada |
| **VALUE-PERF-1.2** | Structs escalares não escapantes sem heap | `vec3_add_loop` reduz alocações a zero e preserva testes/diagnostics |
| **VALUE-PERF-1.3** | Unit enums e payloads escalares inline | state loop não aloca; match mantém semântica |
| **VALUE-PERF-1.4** | Inlining de métodos/operators | helper pequeno desaparece do hot path sem explosão de código |
| **VALUE-PERF-1.5** | Escape analysis ampliada | retornos/capturas/async usam heap somente quando necessário |
| **VALUE-PERF-1.6** | Operators com RHS/output, se aprovados | `Vec3 * float` funciona por trait genérico e possui regressão completa |
| **VALUE-PERF-1.7** | Optional/result specialization, se medida | ganho material sem alterar ABI-1 |

## 8. Gates de performance

Uma fase só permanece se:

- melhora o kernel-alvo de forma repetível;
- não piora workload real relevante em mais de 5%;
- não aumenta cold compile desproporcionalmente;
- não causa crescimento de código sem orçamento;
- passa leak/ARC e robustness suites;
- produz o mesmo resultado em AOT e JIT.

Experimentos sem ganho são revertidos, como já ocorreu com free list e marcação
de tipos acíclicos.

## 9. Áreas afetadas

| Área | Caminhos principais |
|---|---|
| Tipos/layout | `compiler/crates/ori-types/src/ty.rs` |
| HIR/monomorfização | `compiler/crates/ori-hir/src/` |
| Optimizer | `compiler/crates/ori-hir/src/optimize/` |
| Native codegen | `compiler/crates/ori-codegen/src/native_backend.rs` |
| Runtime ARC | `compiler/crates/ori-runtime/src/lib.rs` |
| Math stdlib | `stdlib/math/` |
| Benchmarks | `tools/bench/`, `tools/microbench_lang_perf.sh` |
| Regressões | `compiler/crates/ori-driver/tests/` |

## 10. Testes obrigatórios

- value copied, mutated and returned;
- nested structs/arrays/enums;
- fields com contracts;
- generics monomorfizados;
- closure capture e async frame;
- custom destructor;
- match e destructuring;
- `@c_export` preservando external layout;
- DAP variables em modo debug;
- leak check e cycle collector;
- CLIF/golden apenas para invariantes importantes, não offsets frágeis;
- benchmark guard com tolerância documentada.

## 11. Fora de escopo até medição

- tipos `Vec` mágicos no compilador;
- vector literals especiais;
- SIMD público;
- arena ou scratch allocator;
- fixed-point primitive;
- remover bounds checks globalmente;
- expor layout interno otimizado ao usuário;
- mudar semântica de coleções para COW.
