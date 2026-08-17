# Ori — Evolução da Linguagem para Computação Gráfica From Scratch

> **Status:** Proposta técnica  
> **Projeto:** Ori Language  
> **Repositório:** `raillen/ori-lang`  
> **Base analisada:** Ori `0.3.8`, superfície S3, ABI nativa `ori-native-abi-1`  
> **Branch de referência:** `master`  
> **Commit de referência da análise:** `3c481e6ae28e92093729ccdde5dbffec21cad475`  
> **Re-auditoria (2026-08-16):** revalidada contra o worktree atual
> (`cursor/ori-book-pdf`, 0.3.8-dev com `@cfg`/`conditional` e `ori-embed` ainda
> não commitados). A análise original continua correta; a única divergência
> material é o estado real de `ori.buffer` (ver §6.5).
> **Objetivo:** tornar a Ori uma linguagem especialmente adequada para estudar e implementar computação gráfica desde os fundamentos, preservando a matemática e evitando abstrações que escondam o pipeline gráfico.

---

## 1. Contexto

A Ori já possui uma base suficientemente forte para iniciar estudos e implementações de computação gráfica sem frameworks gráficos.

O estado atual inclui, entre outros recursos:

- compilador escrito em Rust;
- frontend com lexer, parser, resolução de nomes, type checking e diagnósticos;
- HIR tipada;
- otimizações de mid-end;
- backend nativo via Cranelift;
- compilação AOT;
- JIT para `ori run`;
- inteiros de larguras explícitas;
- `float32` e `float64`;
- structs com semântica de valor;
- arrays inline de tamanho fixo;
- listas e coleções gerenciadas;
- ARC;
- FFI C;
- `@c_export`;
- funções matemáticas básicas na stdlib;
- módulos existentes para `Vec2`, `Vec3` e `Mat3`.

Esses recursos tornam a Ori viável para implementar:

- framebuffer em CPU;
- rasterização 2D;
- rasterização de triângulos;
- transformações matriciais;
- pipeline 3D;
- câmera;
- clipping;
- perspective divide;
- z-buffer;
- interpolação;
- iluminação;
- texturas;
- renderer 3D em software.

O objetivo desta proposta não é transformar a Ori em uma engine gráfica ou introduzir uma API de renderização de alto nível.

O objetivo é remover gargalos da linguagem que dificultariam uma implementação gráfica feita pelo próprio programador.

---

# 2. Princípio central

A Ori deve fornecer **bons blocos fundamentais de programação de sistemas e computação numérica**, mas não esconder os conceitos de computação gráfica que se deseja estudar.

Portanto:

## A linguagem deve fornecer

- tipos numéricos adequados;
- estruturas de dados eficientes;
- memória contígua;
- boa otimização de loops;
- operações bitwise;
- acesso previsível à memória;
- FFI;
- ferramentas de benchmark;
- possibilidade futura de SIMD.

## A linguagem não deve fornecer como parte obrigatória do core

- `look_at()`;
- `perspective()`;
- câmera 3D pronta;
- rasterizador de triângulos;
- barycentric coordinates prontas;
- depth buffer pronto;
- materiais;
- shaders de alto nível;
- scene graph;
- ECS;
- renderer;
- engine gráfica.

Essas abstrações podem existir futuramente em bibliotecas separadas, mas não devem ser necessárias para o projeto educacional.

---

# 3. Avaliação atual da Ori para computação gráfica

| Área | Estado atual | Avaliação |
|---|---|---:|
| Tipos inteiros | `int8..int64`, `u8..u64` | Excelente |
| Floating point | `float32`, `float64` | Excelente |
| Structs numéricas | Disponíveis | Excelente |
| Loops | Código nativo | Muito bom |
| Funções matemáticas | `sqrt`, `sin`, `cos`, etc. | Muito bom |
| Arrays inline escalares | Disponíveis | Excelente |
| Arrays inline de structs | Não suportados | Gargalo importante |
| Collections dinâmicas | Disponíveis via runtime | Bom |
| Memória contígua low-level | Limitada | Precisa evoluir |
| Operações bitwise | Deve ganhar superfície explícita/completa | Prioridade |
| Mutable slices | Limitadas | Precisa evoluir |
| Bounds-check elimination | Não é uma garantia forte atual | Precisa evoluir |
| SIMD | Não é foco atual | Futuro |
| FFI C | Disponível e versionada | Muito bom |
| Janela / framebuffer nativo | Não é responsabilidade do core | Pode existir como módulo fino |
| Renderer GPU | Fora do objetivo inicial | Não necessário |

---

# 4. Prioridade P0 — Arrays inline de structs

## 4.1 Problema atual

A Ori já possui:

```ori
array[T, size: N]
```

com características desejáveis:

- tamanho faz parte do tipo;
- armazenamento inline;
- sem heap;
- sem ARC;
- bom potencial de otimização;
- bounds checking;
- possibilidade de índices constantes verificados em compile time.

Entretanto, atualmente os elementos de `array` precisam ser escalares.

Isso impede estruturas naturais como:

```ori
array[Vec3, size: 8]
array[Vertex, size: 1024]
array[Triangle, size: 256]
array[Color, size: 640]
```

Essa restrição se torna especialmente limitante em:

- gráficos;
- física;
- áudio;
- processamento de sinais;
- simulação;
- álgebra linear;
- engines;
- programação científica.

---

## 4.2 Proposta

Generalizar o conceito atual de elemento inline.

Adicionar internamente uma classificação semelhante a:

```text
Inline(T)
```

### Regra inicial sugerida

```text
Inline(bool)      = true
Inline(integer)   = true
Inline(float)     = true

Inline(array[T])  = Inline(T)

Inline(struct S)  =
    true se todos os campos de S forem Inline

Inline(resto)     = false
```

Dessa forma:

```ori
struct Vec3
    x: float32
    y: float32
    z: float32
end
```

seria automaticamente um tipo inline.

E:

```ori
array[Vec3, size: 8]
```

seria válido.

---

## 4.3 Tipos inicialmente proibidos dentro de structs inline

Uma struct deve deixar de ser elegível para armazenamento inline se possuir campos como:

```text
string
bytes
list[T]
map[K, V]
set[T]
slice[T]
future[T]
any[Trait]
handles ARC
closures com environment
tipos runtime-managed
```

Essa restrição mantém o primeiro estágio simples e previsível.

---

## 4.4 Exemplo

```ori
struct Vec3
    x: float32
    y: float32
    z: float32
end

struct Triangle
    a: Vec3
    b: Vec3
    c: Vec3
end

const cube_vertices: array[Vec3, size: 8] = [
    Vec3 { x: -1.0f32, y: -1.0f32, z: -1.0f32 },
    Vec3 { x:  1.0f32, y: -1.0f32, z: -1.0f32 },
    ...
]
```

---

## 4.5 Critérios de aceite

- `array[StructInline, size: N]` compila no backend nativo;
- layout dos elementos é contíguo;
- `size_of` corresponde ao bloco completo;
- atribuição por índice funciona;
- leitura por índice funciona;
- structs inline podem conter outras structs inline;
- structs inline podem conter arrays inline;
- managed types continuam rejeitados;
- diagnósticos deixam claro qual campo tornou o tipo não-inline;
- testes de layout são adicionados;
- testes AOT e JIT são adicionados;
- a ABI interna permanece consistente;
- documentação da Spec 04 e Spec 19 é atualizada.

---

# 5. Prioridade P0 — Benchmark gráfico oficial

A linguagem não deve ser otimizada para gráficos com base em suposições.

O renderer educacional deve virar também um workload real de performance da Ori.

---

## 5.1 Novo conjunto de benchmarks

Criar:

```text
tools/bench/graphics/
```

ou:

```text
benchmarks/graphics/
```

---

## 5.2 Kernels mínimos

### GFX-BENCH-01 — Fill buffer

Preencher:

```text
1920 × 1080 × RGBA
```

com uma cor.

Avalia:

- escrita sequencial;
- bounds checks;
- loop optimization;
- bandwidth.

---

### GFX-BENCH-02 — Gradient

Gerar uma imagem com gradiente:

```text
R = x / width
G = y / height
B = ...
```

Avalia:

- integer → float;
- operações floating point;
- loops aninhados.

---

### GFX-BENCH-03 — Line rasterization

Implementar Bresenham.

Avalia:

- branching;
- integer math;
- random-ish writes.

---

### GFX-BENCH-04 — Triangle rasterization

Rasterização de triângulos via edge functions.

Avalia:

- nested loops;
- multiplicações;
- comparações;
- escrita condicional.

---

### GFX-BENCH-05 — Z-buffer

Rasterização com:

```text
depth[pixel]
```

Avalia:

- dois buffers;
- leitura + comparação + escrita.

---

### GFX-BENCH-06 — Vertex transform

Transformar centenas de milhares de `Vec4` por `Mat4`.

Avalia:

- structs;
- arrays;
- arithmetic throughput;
- inlining;
- futuras oportunidades de SIMD.

---

## 5.3 Comparação

Comparar inicialmente com:

- C;
- Rust;
- Nim;
- Go.

Python/JS podem ser mantidos apenas como referência adicional, mas não devem orientar o target de performance.

---

## 5.4 Métricas

Registrar:

```text
tempo total
pixels/s
triângulos/s
vertices/s
alocações
memória máxima
tempo de compilação
AOT vs JIT
ORI_OPT=none
ORI_OPT=default
ORI_OPT=aggressive
```

---

# 6. Prioridade P1 — Buffer numérico contíguo

## 6.1 Motivação

`list[T]` é uma collection de uso geral.

Um framebuffer não é uma collection genérica de alto nível.

É essencialmente:

```text
N elementos contíguos
+
leitura/escrita indexada
```

Usar a mesma abstração para os dois casos pode introduzir:

- metadata desnecessária;
- ARC;
- versioning;
- guards;
- operações de collection;
- custo semântico que o framebuffer não precisa.

---

## 6.2 Proposta

Estudar um tipo dedicado:

```ori
buffer[T]
```

Inicialmente permitido apenas quando:

```text
Inline(T) == true
```

Exemplos:

```ori
var pixels: buffer[u32] = buffer.new(width * height)
var depth: buffer[float32] = buffer.new(width * height)
var vertices: buffer[Vec3] = buffer.new(vertex_count)
```

---

## 6.3 Semântica proposta

Um `buffer[T]` deve ser:

- mutável;
- contíguo;
- sem crescimento implícito;
- sem hashing;
- sem versionamento de iterator;
- indexável;
- length-aware;
- apropriado para FFI;
- apropriado para upload futuro à GPU;
- otimizado para loops.

API mínima:

```ori
buffer.new[T](len)
buffer.len(buf)
buffer.get(buf, index)
buffer.set(buf, index, value)
buffer.fill(buf, value)
buffer.as_slice(buf)
```

Sintaxe indexada também deve funcionar:

```ori
const value = buf[i]
buf[i] = value
```

---

## 6.4 O que `buffer` não deve ser

Não deve ser:

- uma segunda `list`;
- resizable automaticamente;
- uma abstração gráfica;
- uma texture;
- um vertex buffer da GPU;
- uma API específica de renderer.

É somente memória contígua tipada.

---

## 6.5 Estado real de `ori.buffer` (re-auditoria 2026-08-16)

O módulo `stdlib/buffer.orl` **já existe**, mas como *stub gerenciado*, não
como o buffer contíguo desta proposta:

```ori
module ori.buffer

public struct Buffer[T]
    _ptr: handle[T]
    _len: int
end

public buffer_len[T](buf: Buffer[T]) -> int
    return buf._len
end

public buffer_is_empty[T](buf: Buffer[T]) -> bool
    return buf._len == 0
end
```

Implicações:

- `Buffer[T]` é um wrapper **ARC** (`handle[T]`), com `buffer_len` e
  `buffer_is_empty` como única API — não é memória contígua garantida, não
  indexa, não é FFI-appropriada e não serve de framebuffer.
- O nome `ori.buffer` **já está publicado** e a referência `docs/spec/12-stdlib.md`
  o lista em "Text and bytes". Qualquer implementação de `buffer[T]` contíguo
  deve decidir explicitamente: (a) evoluir `Buffer[T]` para contiguidade real,
  ou (b) introduzir um tipo novo com nome distinto (`framebuffer[T]`,
  `raw_buffer[T]`, `vec[T]`, …) e reservar `Buffer[T]` para o papel gerenciado.
- A evolução do tipo existente é **breaking** para qualquer usuário do stub
  (nenhum uso conhecido além do catálogo); a introdução de tipo novo é
  aditiva e conservadora. A recomendação desta proposta é a opção (b) nesta
  fase, mantendo `Buffer[T]` como coleção gerenciada opaca.


---

# 7. Prioridade P1 — Mutable slices / views

A Ori já possui `slice[T]` como janela read-only.

Isso é uma boa decisão para segurança geral.

Entretanto, workloads numéricos frequentemente precisam dividir um buffer em regiões mutáveis.

---

## 7.1 Proposta

Não alterar silenciosamente `slice[T]`.

Criar um conceito distinto, explícito.

Possíveis nomes:

```text
mut_slice[T]
span[T]
mut_span[T]
view[T]
```

Recomendação inicial:

```text
span[T]
```

com mutabilidade determinada pela origem ou por tipo explícito.

Alternativa mais conservadora:

```text
slice[T]
mut_slice[T]
```

---

## 7.2 Requisitos de segurança

Não permitir duas views mutáveis sobrepostas sem um modelo claro.

Inicialmente pode-se limitar a API a operações como:

```text
split_at_mut
chunks_mut
row_mut
```

que provem ausência de overlap estruturalmente.

---

## 7.3 Aplicação gráfica

Framebuffer:

```text
row = pixels.row_mut(y)
```

Worker threads futuros:

```text
top, bottom = buffer.split_at_mut(mid)
```

Cada thread opera em uma região independente.

---

# 8. Prioridade P1 — Operações bitwise completas

Computação gráfica low-level utiliza frequentemente:

```text
&
|
^
~
<<
>>
```

Casos comuns:

- packing RGBA;
- unpacking de canais;
- masks;
- formatos de pixel;
- fixed-point;
- Morton codes;
- flags;
- bitmaps;
- conversões de formatos.

---

## 8.1 Superfície mínima

Suportar claramente:

```ori
a & b
a | b
a ^ b
~a
a << n
a >> n
```

para todos os tipos inteiros apropriados.

---

## 8.2 Semântica

Definir normativamente:

- signed vs unsigned shift;
- arithmetic vs logical right shift;
- comportamento de shift >= bit width;
- overflow;
- integer promotion;
- literal inference.

---

## 8.3 Exemplo RGBA

```ori
pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32
    return
        (u32(r) << 24) |
        (u32(g) << 16) |
        (u32(b) << 8)  |
        u32(a)
end
```

---

# 9. Prioridade P2 — Bounds Check Elimination

Bounds checking deve continuar sendo o comportamento padrão.

A evolução desejada é o compilador provar quando determinado check é redundante.

---

## 9.1 Exemplo

```ori
var i: int = 0

while i < buffer.len(pixels)
    pixels[i] = 0
    i = i + 1
end
```

O compilador deveria conseguir provar que:

```text
0 <= i < len
```

na operação indexada.

---

## 9.2 Casos iniciais

Implementar BCE progressivamente para:

```text
for i in 0..N
array[i]

while i < len
buffer[i]

loops simples com limite invariável

array fixo com range comprovável
```

---

## 9.3 Regra

A ausência de prova nunca remove segurança.

```text
prova encontrada    -> elimina check
prova inconclusiva  -> mantém check
```

---

## 9.4 Métrica

Os benchmarks gráficos devem medir:

```text
BCE ON
BCE OFF
```

antes de qualquer alteração agressiva.

---

# 10. Prioridade P2 — Melhorias do mid-end para kernels numéricos

O renderer deve ser usado para identificar otimizações reais.

Candidatas:

- loop invariant code motion;
- common subexpression elimination;
- scalar replacement;
- dead store elimination;
- bounds-check elimination;
- inlining orientado por custo;
- strength reduction;
- const propagation;
- alias-aware optimization;
- escape analysis limitada.

Não implementar passes apenas porque existem em LLVM.

Cada otimização deve mostrar ganho em workloads reais.

---

# 11. Prioridade P3 — SIMD e auto-vectorization

SIMD não é requisito para começar o projeto.

Também não deve ser implementado cedo demais.

Primeiro:

1. estabilizar layout inline;
2. estabilizar buffers;
3. ter benchmarks gráficos reais;
4. identificar hot loops;
5. medir código escalar.

Somente então avaliar SIMD.

---

## 11.1 Caminhos possíveis

### Opção A — Autovectorização

O compilador reconhece loops simples.

Mais amigável ao usuário.

### Opção B — Tipos vetoriais explícitos

Exemplo futuro:

```text
simd[f32, 4]
```

Mais complexo.

### Opção C — Intrinsics

Exemplo:

```text
ori.simd.*
```

Menos elegante, mas controlável.

---

## 11.2 Recomendação

Começar por autovectorização de casos triviais somente quando houver benchmark demonstrando necessidade.

Não tornar SIMD parte da superfície estável antes disso.

---

# 12. Janela e apresentação do framebuffer

Isso não deve ser uma feature do core da linguagem.

Deve ser um módulo de plataforma fino.

---

## 12.1 Fase inicial

Não usar janela.

Gerar:

```text
PPM
```

depois eventualmente:

```text
BMP
PNG
```

O renderer continua completamente independente de qualquer API gráfica.

---

## 12.2 Fase realtime

Criar futuramente algo como:

```text
ori.window
```

com responsabilidades mínimas:

```text
open
close
poll_events
width
height
present
```

---

## 12.3 Regra arquitetural

`ori.window.present()` não deve rasterizar nada.

Ele apenas copia/exibe um buffer criado pelo programa.

Camadas:

```text
Aplicação Ori
    ↓
Renderer software Ori
    ↓
buffer[u32]
    ↓
ori.window
    ↓
X11 / Wayland / Win32 / Cocoa
```

Toda a matemática permanece na camada Ori.

---

# 13. Caminho de estudo que servirá como workload da linguagem

O estudo deve evoluir junto com a linguagem.

---

## Etapa 1 — Imagem

Implementar:

- framebuffer;
- RGB;
- PPM;
- `set_pixel`;
- gradientes.

Validar:

- loops;
- buffers;
- integer conversion;
- file output.

---

## Etapa 2 — Rasterização 2D

Implementar:

- DDA;
- Bresenham;
- círculo;
- retângulo;
- preenchimento.

Validar:

- integer arithmetic;
- branching;
- bounds;
- bitwise.

---

## Etapa 3 — Vetores e matrizes

Implementar do zero:

```text
Vec2
Vec3
Vec4
Mat3
Mat4
```

Não usar inicialmente:

```text
ori.math.vec2
ori.math.vec3
ori.math.mat3
```

A stdlib pode ser utilizada depois como referência de comparação.

---

## Etapa 4 — Triângulos

Implementar:

- edge function;
- winding;
- bounding box;
- barycentric coordinates;
- triangle fill.

Validar:

- float throughput;
- nested loops;
- buffer writes.

---

## Etapa 5 — Pipeline 3D

Implementar:

```text
model space
world space
view space
clip space
perspective projection
perspective divide
NDC
viewport
```

---

## Etapa 6 — Visibilidade

Implementar:

- backface culling;
- clipping;
- z-buffer.

---

## Etapa 7 — Interpolação

Implementar:

- vertex colors;
- barycentric interpolation;
- perspective-correct interpolation;
- UV.

---

## Etapa 8 — Iluminação

Implementar:

- normals;
- Lambert;
- diffuse;
- specular;
- Phong;
- Blinn-Phong.

---

## Etapa 9 — Texturas

Implementar:

- texture sampling;
- nearest;
- bilinear;
- wrapping;
- mipmaps.

---

## Etapa 10 — Realtime

Somente neste ponto introduzir:

```text
ori.window
```

---

# 14. Alterações que não devem ser feitas agora

Evitar adicionar prematuramente:

- API de GPU;
- OpenGL wrapper ao core;
- Vulkan wrapper ao core;
- WebGPU ao core;
- ECS;
- scene graph;
- shader language;
- renderer;
- material system;
- texture class gráfica;
- camera class;
- mesh abstraction;
- physics;
- game loop automático.

Esses recursos criariam um desvio de escopo.

---

# 15. Mudanças sugeridas na documentação oficial

Ao implementar esta proposta, atualizar:

```text
docs/spec/04-types.md
docs/spec/05-expressions.md
docs/spec/10-memory.md
docs/spec/12-stdlib.md
docs/spec/14-backend-support.md
docs/spec/19-abi.md
docs/guides/performance.md
docs/guides/performance.pt-BR.md
CHANGELOG.md
```

Se `buffer[T]` se tornar parte da linguagem:

```text
docs/spec/04-types.md
docs/spec/10-memory.md
docs/spec/19-abi.md
```

devem ser consideradas fontes normativas.

> **Re-auditoria (2026-08-16):** `docs/spec/12-stdlib.md` já referencia
> `ori.buffer` no grupo "Text and bytes" (linha ~132); ao evoluir ou
> substituir o módulo, essa referência deve ser atualizada junto. Sem novas
> entradas de docs para esta proposta além das listadas — a análise anterior
> não acrescenta arquivos adicionais.

---

# 16. Testes necessários

## Inline structs

Adicionar testes para:

```text
array[Vec2]
array[Vec3]
array[nested_struct]
array[array]
```

E negativos para:

```text
array[StructWithString]
array[StructWithList]
array[StructWithMap]
```

---

## Layout

Verificar:

```text
sizeof(Vec3)
alignof(Vec3)
sizeof(array[Vec3, N])
offset de cada elemento
```

> **Re-auditoria (2026-08-16):** `ori.mem.size_of` já reporta o bloco completo
> para arrays escalares (`docs/spec/04-types.md`); a validação de layout com
> structs inline dependerá da mesma função estendida para os novos tipos.

---

## Codegen

Cobrir:

```text
AOT
JIT
release/debug
x86_64
ARM64 onde aplicável
```

---

## Buffer

Testar:

```text
allocation
zero length
large length
overflow
index read
index write
fill
slice/view
cleanup
FFI
```

---

## Bitwise

Testar todos:

```text
i8/i16/i32/i64
u8/u16/u32/u64
```

incluindo limites.

---

# 17. Critérios para considerar a Ori "graphics-ready"

A Ori pode ser classificada como pronta para workloads gráficos em CPU quando:

- [x] `array[InlineStruct, N]` estiver disponível; (**2026-08-16** — GFX-INLINE-1)
- [x] houver benchmark oficial de framebuffer; (**2026-08-16** — GFX-BENCH-1)
- [x] houver benchmark oficial de triangle rasterization; (**2026-08-16** — GFX-BENCH-1)
- [ ] workloads não produzirem alocações por pixel;
- [ ] framebuffer contíguo estiver disponível;
- [ ] depth buffer contíguo estiver disponível;
- [x] operações bitwise estiverem completas e documentadas; (**2026-08-16** — GFX-BITWISE-1)
- [ ] o compilador eliminar bounds checks triviais;
- [ ] AOT e JIT produzirem resultados equivalentes;
- [ ] layouts usados em FFI estiverem documentados;
- [ ] regressões de performance forem detectadas em CI.

SIMD não é requisito para atingir esse estágio.

> **Re-auditoria (2026-08-16):** nenhum destes itens está hoje implementado;
> todos permanecem como critérios de aceite. O único artefato que poderia
> parecer parcial (`ori.buffer`) é um stub gerenciado e não satisfaz os itens
> "framebuffer contíguo" nem "depth buffer contíguo" (ver §6.5).

---

# 18. Ordem de implementação recomendada

```text
P0.1  Inline classification (predicado Inline(T) no checker, §4.2)
P0.2  array[InlineStruct, N]
P0.3  layout/codegen/tests
P0.4  graphics benchmark suite

P1.1  bitwise surface
P1.2  buffer[T] (decidir evolução vs. tipo novo — ver §6.5)
P1.3  mutable views/spans

P2.1  bounds-check elimination
P2.2  numeric-loop mid-end improvements

P3.1  profiling de workloads
P3.2  avaliar vectorization
P3.3  SIMD se justificado

ECO.1  PPM/BMP helpers
ECO.2  minimal ori.window
```

> **Correção de auditoria (2026-08-16):** o passo P1.2 deve ser precedido por
> uma decisão de contrato sobre `ori.buffer` existente (evoluir vs. novo nome),
> conforme §6.5. O P0.1 é o verdadeiro bloqueio inicial: hoje `Ty::Named`
> (structs) está incondicionalmente em `is_runtime_managed`
> (`compiler/crates/ori-types/src/ty.rs`), e o checker rejeita arrays de
> structs com `type.array_element_not_inline` em
> `compiler/crates/ori-types/src/lower.rs` — é o único ponto que impede
> `array[Vec3, size: 8]` de compilar.

---

# 19. Decisão arquitetural recomendada

A Ori não precisa se tornar uma linguagem especializada em computação gráfica.

A direção recomendada é:

> **Ori deve se tornar melhor em computação numérica, estruturas inline, memória contígua e loops eficientes.**

Computação gráfica será um dos workloads que se beneficiará disso.

As mesmas melhorias também fortalecem:

- game engines;
- áudio;
- física;
- processamento de imagem;
- computação científica;
- emuladores;
- codecs;
- bancos de dados;
- networking low-level;
- ferramentas de sistemas.

---

# 20. Resultado esperado

Após as mudanças P0 e P1, código como este deve ser natural na Ori:

```ori
struct Vec3
    x: float32
    y: float32
    z: float32
end

struct Color
    r: u8
    g: u8
    b: u8
    a: u8
end

struct Vertex
    position: Vec3
    color: Color
end

main()
    const width: int = 640
    const height: int = 480

    var pixels: buffer[u32] =
        buffer.new[u32](width * height)

    var depth: buffer[float32] =
        buffer.new[float32](width * height)

    var vertices: array[Vertex, size: 3] = [
        ...
    ]

    rasterize(vertices, pixels, depth, width, height)
end
```

Todo o pipeline seguinte pode continuar sendo escrito manualmente pelo usuário:

```text
transformação
câmera
projeção
clipping
rasterização
z-buffer
interpolação
textura
iluminação
```

sem depender de um framework gráfico.

---

# 21. Conclusão

A Ori já possui base suficiente para iniciar um renderer software educacional.

As principais limitações atuais não estão na matemática ou no codegen básico, mas na representação eficiente de dados numéricos compostos.

A mudança mais importante é permitir **structs totalmente inline dentro de arrays inline**.

Logo depois, a linguagem se beneficiaria de um **buffer contíguo especializado**, operações bitwise completas, mutable views controladas e otimizações de bounds checking.

O renderer "from scratch" deve ser usado como benchmark contínuo da linguagem.

Isso evita evolução especulativa e cria um ciclo produtivo:

```text
estudar computação gráfica
        ↓
criar workload real
        ↓
encontrar gargalo da Ori
        ↓
melhorar linguagem/runtime/compiler
        ↓
medir novamente
        ↓
avançar para o próximo estágio gráfico
```

Essa abordagem mantém a Ori simples, generalista e educacional, ao mesmo tempo em que a força a amadurecer em áreas relevantes para programação nativa de alto desempenho.
