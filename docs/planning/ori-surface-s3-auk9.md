# Ori Surface S3 — sintaxe Auk9-inspired

> **Status:** decisões de produto **completas** (blocos 0–9)  
> **Produto:** Ori (features e maturidade)  
> **Superfície:** o mais próximo possível da Auk9 (S3)  
> **Implementação:** PRs 1–9 (compiler + migrate + fontes) + PR 10 (docs) — superfície no compiler  
> **Versão alvo:** `0.3.0` superfície+docs · `0.3.1` inferência Nim-local · **opção B** (campo/index/call/pipe) entregue  

> **Última atualização:** 2026-07-12

## Como usar este documento

- Cada **bloco** fecha com decisões numeradas.
- Novos blocos são **acrescentados** aqui na mesma sessão de decisão.
- Spec normativa (`docs/spec/`) e código só mudam **depois** da implementação acordada.
- Referência de superfície Auk9 (lab/read-only): repositório `auk9-lang` (irmão / lab).

## Norte em uma frase

**Motor e features da Ori; pele e ritmo da Auk9; Auk9 lab aposentado como produto — superfície vivente na Ori.**

## O que não muda (fora de escopo de superfície)

- Async / await, channels, cancel tokens  
- Sistema de traits **semântico** da Ori (poder, bounds, monomorph, defaults)  
- Runtime ARC, codegen nativo, JIT, net, stdlib **capacidade**  
- Extensão `.orl`, CLI `ori`, namespace de stdlib `ori.*`  
- Inferência global de bindings (continua rejeitada)

---

## Bloco 0 — Regras do jogo

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **0.1** | Alcance | **S3** — clone gramatical o mais próximo da Auk9 | Objetivo: aposentar Auk9 como linguagem/produto paralelo |
| **0.2** | Soltura | **Corte seco** no `0.3.0` | Sem período dual longo; forma antiga deixa de ser aceita na release de quebra |
| **0.3** | Destino da Auk9 | **Lab** durante a migração | Depois: congelar/arquivar como produto; Ori absorve a superfície |
| **0.4** | Identidade de arquivos/CLI | Manter **`.orl`**, **`ori`**, **`ori.*`** | Só sintaxe muda, não a marca |

### Traits / `apply` (subdecisão do bloco 0, detalhada)

**Status:** ✅ fechado

| ID | Tema | Decisão |
|----|------|---------|
| **T1** | Modelo | **Opção A** — pele Auk9, **motor Ori** (traits de verdade) |
| **T2** | Sintaxe de aplicação | `apply Tipo` + seções `use Trait` |
| **T3** | Bind externo | **Sim** — `slot = nomeDaFuncao` (compile-time; não é assign runtime) |
| **T4** | Forma antiga | `apply Trait to Type` **some** no corte seco |
| **T5** | Method table sem traits (opção C) | **Rejeitada** |

#### Exemplo canônico — inline

```ori
module app.geo

trait Comparable
  compare(a: Self, b: Self) -> int
end

struct Point
  x: int
  y: int
end

apply Point
  use Comparable
    compare(a: Point, b: Point) -> int
      return a.x - b.x
    end
  end
end
```

#### Exemplo canônico — método fora + bind

```ori
comparePoints(a: Point, b: Point) -> int
  return a.x - b.x
end

apply Point
  use Comparable
    compare = comparePoints
  end
end
```

#### Vários traits no mesmo tipo

```ori
apply Point
  use Displayable
    display(self: Point) -> string
      return f"({self.x},{self.y})"
    end
  end

  use Comparable
    compare = comparePoints
  end
end
```

**Pendente fino (não bloqueia bloco 0):** keyword `default` nos métodos default do trait (estilo Auk9) vs só corpo como Ori hoje — fechar na etapa de sintaxe fina de traits/defaults se necessário.

---

## Bloco 1 — Palavras do arquivo

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **1.1** | Cabeçalho de arquivo | Só **`module`** | `namespace` some no corte seco |
| **1.2** | Declaração de função | **Sem** keyword **`func`** em todo lugar | Inclui trait/apply/async: `async fetch(...) -> ...` |
| **1.3** | Anotação de retorno `->` | **Como a Ori hoje** (omissions permitidas onde já forem) | **Cultura:** preferir **`alias`** para tipos de retorno longos/repetidos; **stdlib** deve aplicar isso de propósito |
| **1.4** | Visibilidade | Manter **`pub`** | Sem rename |

### Exemplo — arquivo modelo (bloco 1)

```ori
module app.users

alias UserResult = result[User, string]

pub struct User
  id: int
  name: string
end

pub loadUser(id: int) -> UserResult
  ...
end

main()
  ...
end
```

### Guia de estilo — alias de retorno (1.3)

| Nível | Regra |
|-------|--------|
| Linguagem (checker) | Omissões de `->` / void como **hoje** (1.3 C) |
| Estilo canônico | Nomear `result[...]`, `list[...]`, etc. com `alias` quando longos ou repetidos |
| Stdlib | Preferir aliases públicos de domínio (`IoResult`, `TextResult`, …) — nomes finais na migração da stdlib |

---

## Bloco 2 — Tipos na tela

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **2.1** | Tipos compostos | **Só `[]`** | `list[string]`, `map[K, V]`, `optional[T]`, `result[T, E]`; `<>` **erro** no corte seco |
| **2.2** | `of` / `to` em tipos | **Removidos** | `list of T`, `map of K to V`, `optional of T` **erro** no corte seco |
| **2.3** | Genéricos de usuário | **Forma Auk9** | `Nome[T]`, bounds `for T: Trait` (não `where T is` / `func foo<T>` como canônico) |
| **2.4** | Alias exportável | **`pub alias` permitido** | Stdlib e APIs de domínio devem preferir aliases públicos legíveis (cultura 1.3) |

### Exemplos canônicos

```ori
alias UserResult = result[User, string]
alias UserList = list[User]
alias Scores = map[string, int]

const names: UserList = []
const score: optional[int] = none

max for T: Comparable (a: T, b: T) -> T
  ...
end

struct Pair[A, B]
  left: A
  right: B
end

pub alias IoResult = result[void, string]
```

### Formas que somem no `0.3.0`

```ori
list<string>
list of string
map of string to int
result<User, string>
func max<T>(a: T, b: T) -> T where T is Comparable   -- forma antiga Ori
```

---

## Bloco 3 — Erros e fluxo de controle

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **3.1** | Propagação de erro/ausência | **Só `try expr`** | `expr?` **erro** no corte seco |
| **3.2** | Cadeia condicional | **Só `elif`** | `else if` **erro** no corte seco |
| **3.3** | `if` como expressão | **Manter forma Ori** | `if cond then expr else expr` (não adotar if-expr só-em-`=>` da Auk9 neste ciclo) |
| **3.4** | Patterns de enum no `match` | **Estilo Auk9** | No `case`, variante **sem ponto** (`case Circle(...):` / `case Point:`); literais fora do match continuam com forma de literal (bloco 4) |

### Exemplos canônicos

```ori
const user: User = try loadUser(id)

if n > 0
  ...
elif n < 0
  ...
else
  ...
end

const label: string = if n > 0 then "pos" else "other"

match shape
  case Circle(radius: r):
    ...
  case Point:
    ...
  case else:
    ...
end
```

### Formas que somem no `0.3.0`

```ori
const user: User = loadUser(id)?
else if n < 0
case .Circle(radius: r):
case .Point:
```

---

## Bloco 4 — Literais (struct / map / enum)

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **4.1** | Struct literal | **Só Auk9** | `{ field: v }` (tipo pelo contexto) e `Type { field: v }`; somem: `Type(...)`, `.{…}`, `(…)` guiado |
| **4.2** | Map literal | **`{ "k": v, ... }`** | Chave literal (string/número); `{}` vazio exige tipo no contexto |
| **4.3** | Enum literal (fora do match) | **Auk9** | `Enum.Variant` / `Enum.Variant(campos)`; forma curta `.Variant` / `.Variant(...)` com contexto |
| **4.4** | List literal | **Manter `[…]`** | Inclui `[]` vazio com tipo no contexto |

### Disambiguação struct vs map

| Token antes do `:` | Interpretação |
|--------------------|---------------|
| Identificador (`name:`) | campo de **struct** |
| Literal (`"a":`, `1:`) | chave de **map** |
| `{}` vazio | tipo **obrigatório** no contexto |

### Exemplos canônicos

```ori
const u: User = { name: "Ada", age: 36 }
const u2 = User { name: "Bo", age: 20 }
const ages: map[string, int] = { "Ada": 36, "Bo": 20 }
const xs: list[int] = [1, 2, 3]
const s: Status = Status.Active
const shape: Shape = .Circle(radius: 1.5)
```

### Formas que somem no `0.3.0`

```ori
User(name: "Ada", age: 36)
.{ name: "Ada" }
(name: "Ada", age: 36)
```

---

## Bloco 5 — Imports

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **5.1** | Formas de import | **3 formas estilo Auk9**, **sem** `only` e **sem** `as` | Ordem do **alias adaptada** (ver abaixo) — **não** copiar `import io = ori.io` da Auk9 |
| **5.2** | Import “nu” | **Auk9 puro** | `import ori.io` **não** cria alias `io`; só caminho completo `ori.io.print(...)` |
| **5.3** | Bloco `imports … end` | **Mantido** + **várias entradas por linha** | Separadas por **vírgula** |
| **5.4** | Re-export | **`pub import`** nas formas novas | Feature Ori preservada |

### As três formas (canônicas Ori-S3)

| Intenção | Forma | Efeito |
|----------|--------|--------|
| Seletivo | `import ori.fs (readText, writeText)` | Nomes soltos no escopo |
| Alias de módulo | `import ori.io = io` | Usa `io.print(...)` — **path à esquerda, apelido à direita** |
| Módulo inteiro | `import ori.io` | Só `ori.io.print(...)` |

**Leitura do alias (decisão do mantenedor):**  
“importa **ori.io** e chama de **io**” → `import ori.io = io`  
(Auk9 faz o inverso: `import io = ori.io`. **Ori-S3 não segue a Auk9 neste detalhe.**)

### Mapa de migração a partir da Ori atual

| Ori hoje | Ori-S3 |
|----------|--------|
| `import ori.io as io` | `import ori.io = io` |
| `import ori.fs only (a, b)` | `import ori.fs (a, b)` |
| `import ori.io` (se criava alias implícito) | `import ori.io` = só caminho completo; ou `import ori.io = io` se quiser alias |

### Bloco `imports` — uma entrada por linha

```ori
imports
  app.config (Config)
  ori.fs (readText, writeText)
  ori.io
  app.users = users
end
```

### Bloco `imports` — várias na mesma linha (5.3)

```ori
imports
  ori.fs (readText), ori.io = io, app.users = users
  ori.json (parse, stringify)
end
```

Regras do multi-import por linha:

- Separador: **vírgula**
- Cada pedaço é uma das 3 formas completas
- Vale também fora do bloco? **Não decidido como obrigatório** — recomendação: multi-vírgula **só dentro** de `imports … end` (mais simples de parsear e de ler). Ver 5.3b abaixo se precisar reabrir.

**5.3b (default adotado no registro):** vírgulas multi-import **apenas** no bloco `imports … end`. Linhas soltas `import …` = **uma** forma por statement.

### `pub import`

```ori
pub import app.users = users
pub import app.types (User, Role)
```

### Formas que somem no `0.3.0`

```ori
import ori.io as io
import ori.fs only (read_text)
import io = ori.io          -- ordem Auk9; não é a forma Ori-S3
```

---

## Bloco 6 — Defaults de trait / finos de apply

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **6.1** | Método default no trait | **Só corpo** (estilo Ori) | Sem keyword `default` da Auk9; assinatura sem corpo = obrigatório; com corpo = default |
| **6.2** | Tipo de `self` | **`self` sem tipo ok** quando o contexto é óbvio | Motor Ori; não exigir `self: Point` sempre |
| **6.3** | Ordem em `apply Tipo` | **Ordem fixa Auk9** | 1) métodos/binds soltos 2) seções `use Trait` 3) dentro do `use`: slots obrigatórios, depois overrides de default |
| **6.4** | Apply sem trait | **Permitido** | `apply Point` só com métodos/`slot = fn`; traits continuam via `use` |

### Exemplo — trait com default (6.1 B)

```ori
trait Displayable
  display(self) -> string

  print(self) -> void
    -- corpo presente = método default
    io.print(self.display())
  end
end
```

### Exemplo — apply completo (6.3 + 6.4 + bind)

```ori
apply Point
  -- 1) métodos soltos (opcional)
  debugName = pointDebugName

  -- 2) traits
  use Displayable
    display(self) -> string
      return f"({self.x},{self.y})"
    end
    -- print: herda default do trait se não sobrescrever
  end

  use Comparable
    compare = comparePoints
  end
end
```

**Nota:** T1–T5 (bloco 0) + bloco 6 fecham a superfície de traits/apply para o S3.

---

## Bloco 7 — Extras de ritmo

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **7.1** | Corpo `=>` | **Sim** | Função nomeada e closure de **uma** expressão; multi-statement continua com `end` |
| **7.2** | Chamada poética | **Sim** | Um argumento na mesma linha; **proibido aninhar** poetic em poetic; ver exemplos |
| **7.3** | `end` rotulado | **Sim** | `end` / `end if` / `end match` / … opcional; mismatch → **erro** |
| **7.4** | Sintaxe de closure | **Opção B** — `(params) => expr` | Sem keyword `do` / `fn` / `given`; corpo longo: `(params)` + bloco + `end` |
| **7.5** | Pipe `\|>` | **Manter** (já implementado na Ori) | Correção 2026-07-13: registro anterior “fora do 0.3” foi **equivoco de ata** (sem voto). Auk9 rejeitou pipe; **Ori conserva** `\|\>`. Teste: `compile_runs_pipe_operator_native` |

### 7.1 — Exemplos

```ori
double(x: int) -> int => x * 2

greet(name: string) -> string => f"hi, {name}"
```

### 7.2 — Chamada poética (regras)

| Válido | Inválido |
|--------|----------|
| `print name` | `print greet name` (poetic aninhada) |
| `print(greet(name))` | |
| `print user.name()` | |
| **`print greet("hello")`** | argumento é chamada **com** `()` — não é poetic aninhada |

Regra mental: **no máximo um** “verbo sem parênteses” por expressão.  
O argumento pode ser literal, nome, campo, método, ou **chamada entre parênteses** / com `()`.

### 7.3 — Exemplos

```ori
if ok
  ...
end if

match shape
  case Circle(radius: r):
    ...
  case else:
    ...
end match
```

### 7.4 — Closures (canônico)

```ori
-- curta
users.map((u) => u.name)
users.filter((u: User) => u.active)

-- longa
users.map((u: User)
  const n: string = u.name
  return n.to_upper()
end)

-- preferir função nomeada se o corpo for grande (guia de estilo)
userName(u: User) -> string => u.name
users.map(userName)
```

**Rejeitado no S3:** `do(...)`, `fn(...)`, `given(...)` como forma canônica de closure.

---

## Bloco 8 — Filosofia e docs ND

**Status:** ✅ fechado (decisões de produto/docs; implementação de docs no plano 9+)

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **8.1** | Identidade em uma frase | **Síntese Ori + ritmo Auk9** | AOT, tipada, legível; features maduras; superfície poema S3 |
| **8.2** | Público e propósito | **ND reforçado** + **propósito explícito de estudo** | Ver texto canônico abaixo |
| **8.3** | Checklist de feature | **Adotar** 4 perguntas (visível / tipo / erro ensina / forma mais simples) | Spec overview + planning; detalhe em guia |
| **8.4** | Uma forma canônica | **Norma** + **reforma documental completa** | Adequar docs ao S3; reorganizar; apagar depreciados; merge; refinar o resto |
| **8.5** | Auk9 | **Lab → arquivar** após absorção | README da Auk9 aponta superfície na Ori |
| **8.6** | Onde escrever | **+ manifesto separado** | `docs/spec/00-manifesto.md` (estilo Auk9) **além** de overview/planning |

### 8.2 — Texto de propósito (canônico para o manifesto)

A Ori **não** visa competir com linguagens de mercado como produto industrial.  
Foi criada **para**:

1. **Estudo** de compiladores e design de linguagens  
2. **Explorar limites** da programação assistida por IA (humano + agente no mesmo código)  
3. **Legibilidade** e acessibilidade (em especial neurodivergência: TDAH, dislexia, etc.)

Uso real pequeno/médio e maturidade de features **existem** como laboratório sério — não como promessa de “substituir Rust/Go/…”.

### 8.4 — Reforma documental (escopo acordado)

Quando a superfície S3 estiver definida (e na implementação/`0.3.0`):

- Reescrever/adequar **toda** a documentação ativa ao que este arquivo decide  
- **Reorganizar** docs antigas  
- **Apagar** documentação depreciada  
- **Merge** de docs com escopo duplicado  
- **Refinar** o restante (guias, README, site)

### 8.1 — Frase-guia (rascunho)

> Ori é uma linguagem **compilada AOT**, tipada e legível, com features de linguagem “de verdade”, superfície de leitura no estilo poema (S3), feita para **estudar compiladores**, **testar programação assistida por IA** e **ler código com menos carga cognitiva** — não para disputar o mercado de linguagens.

---

## Bloco 8b — Inferência de tipos local (estilo Nim-local)

**Status:** ✅ fechado (design); implementação em fatia **após** estabilizar superfície S3 (não misturar no big-bang do parser se arriscar o corte seco)

### Decisão

| ID | Tema | Decisão |
|----|------|---------|
| **8b.1** | Modelo | **Inferência local / contextual** — omitir tipo só quando está **óbvio na mesma linha** |
| **8b.2** | Referência de *feeling* | **Nim-local** (não C# `var` como marca; **não** HM global) |
| **8b.3** | Global HM | **Continua proibida** (decisão Ori 2026-07-01) |
| **8b.4** | API pública | **`pub`, params e retornos de API: anotar** (aliases ok; não esconder `result` difícil) |
| **8b.5** | Quando implementar | **`0.3.1` entregue** (2026-07-13) — omissão local no checker + testes + docs |
| **8b.6** | Ampliação pós-0.3.1 | **Opção B aceita** (2026-07-13); **não** C/D/E |
| **8b.7** | Escopo da B | Calls com retorno conhecido + campo/index tipados + **pipe `\|\>`** como call; rejeitar `void` |
| **8b.8** | Status B | **Entregue** (checker via `infer_expr` + tipagem de `Pipe` + testes) |

### Pode omitir (lista — 8b + B)

- Literais com tipo único: `1`, `"x"`, `true`, floats óbvios, etc.
- Struct com tipo no literal: `User { name: "Ada", age: 36 }`
- Listas/maps literais **não vazios** com elementos de tipo único
- **Campo / index** em receptor já tipado: `u.name`, `xs[0]`
- **Chamada** com retorno monomórfico conhecido (user + stdlib)
- **Pipe** `value |> f` (equivalente a `f(value)` no checker)

### Não omitir (regra dura de design)

- `try expr` / genéricos ambíguos / `none` / `[]` `{}` vazios **sem** tipo no contexto
- Assinaturas que são contrato de leitura (`pub`, parâmetros, retornos de API)
- Qualquer caso em que o tipo **não grita** na mesma linha sem LSP

### Exemplo-alvo (pós-S3 + 8b)

```ori
loadUser(id: int) -> UserResult
  const path = "users.json"
  const raw = try fs.readText(path)    -- se a regra de try exigir tipo, anotar
  ...
end

main()
  const n = 1
  const u = User { name: "Ada", age: 36 }
end
```

---

## Bloco 9 — Plano de migração / corte seco `0.3.0`

**Status:** ✅ fechado

| ID | Tema | Decisão | Notas |
|----|------|---------|--------|
| **9.1** | Escopo de release | **`0.3.0` = S3 + reforma docs (B)**; **`0.3.1` = inferência Nim-local (C)** | Não misturar 8b no big-bang do 0.3.0 |
| **9.2** | Fases internas | **Ordem P0…P10 / PR plan** | Ver `pr-plan-ori-surface-s3.md` |
| **9.3** | Migração | **Script** + dual **só em dev** se preciso; produto = corte seco | `ori migrate-syntax` / tools |
| **9.4** | Pacotes game/imgui | **Cancelado (2026-07-13)** — `ori-game` / `ori-imgui` **fora do produto**; removidos do repo | |
| **9.5** | Checklist pronto | **Aceito** | test workspace, catalog, examples, docs, CHANGELOG, forma antiga = erro |
| **9.6** | Próximo passo de processo | **ADR + PR plan para `/execute-plan`** | Arquivos abaixo |

### Artefatos de implementação

| Arquivo | Papel |
|---------|--------|
| [`adr-ori-surface-s3-auk9.md`](adr-ori-surface-s3-auk9.md) | ADR aceito |
| [`pr-plan-ori-surface-s3.md`](historico/pr-plan-ori-surface-s3.md) | **DAG `## PR Plan`** para `/execute-plan` |
| Este arquivo | Registro de decisões de produto |

### Como executar

```text
/execute-plan docs/planning/historico/pr-plan-ori-surface-s3.md
```

Opcional: `--dry-run` primeiro; para só 0.3.0, instruir a parar após PR10 ou
rodar e pular PR11 até a tag 0.3.0.

---

## Mapa de etapas (roteiro)

| Bloco | Nome | Status |
|-------|------|--------|
| 0 | Regras do jogo + traits | ✅ |
| 1 | Palavras do arquivo | ✅ |
| 2 | Tipos na tela | ✅ |
| 3 | Erros e fluxo (`try`, `elif`, …) | ✅ |
| 4 | Literais (struct/map/enum) | ✅ |
| 5 | Imports | ✅ |
| 6 | Defaults de trait / finos de apply | ✅ |
| 7 | Extras de ritmo | ✅ |
| 8 | Filosofia & docs ND | ✅ |
| 8b | Inferência local Nim-local + opção B | ✅ 0.3.1 + B (field/index/call/pipe) |
| 9 | Plano de migração / corte seco | ✅ |

**Diálogo de decisões de superfície: completo.**

---

## Histórico de sessão

| Data | Evento |
|------|--------|
| 2026-07-12 | Criação do arquivo; registro dos blocos 0 e 1; abertura do bloco 2 |
| 2026-07-12 | Bloco 2 fechado (tudo A); abertura do bloco 3 |
| 2026-07-12 | Bloco 3 fechado (3.1 A, 3.2 A, 3.3 B, 3.4 A); abertura do bloco 4 |
| 2026-07-12 | Bloco 4 fechado (tudo A); abertura do bloco 5 |
| 2026-07-12 | Bloco 5 fechado (formas Auk9 + alias `path = curto` + multi-import no bloco); abertura do bloco 6 |
| 2026-07-12 | Bloco 6 fechado (6.1 B, 6.2 A, 6.3 A, 6.4 A); abertura do bloco 7 |
| 2026-07-12 | 7.1 A, 7.2 A (+ `print greet("hello")`), 7.3 A; 7.4 em propostas |
| 2026-07-12 | 7.4 B `(u)=>`; (ata errada: “pipe fora do 0.3”); bloco 7 fechado; abertura do bloco 8 |
| 2026-07-13 | **7.5 corrigido:** pipe `\|\>` **permanece** na Ori; Auk9 arquivada como produto; prioridade curto/médio prazo redefinida |
| 2026-07-13 | Pipe confirmado pelo usuário; inferência ampla = **B** (calls + campo/index); push tags 0.3.x |
| 2026-07-13 | B aceita e entregue: tipagem `Pipe`, testes field/index/call/pipe, reject void |
| 2026-07-12 | Bloco 8 fechado; abertura 8b (inferência local) antes do 9 |
| 2026-07-12 | 8b fechado (Nim-local, API explícita, impl após S3); abertura do bloco 9 |
| 2026-07-12 | Bloco 9 fechado; ADR + PR plan para execute-plan criados |
