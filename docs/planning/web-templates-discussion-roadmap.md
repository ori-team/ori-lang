# Roadmap de discussão — ori-templates / ori-web / HTML-first

> **Status:** design fechado (D0–D29); MVPs Library em `packages/`.  
> **Audience:** maintainers.  
> **Packages alvo (externos ao monorepo language core / FREEZE-1):**  
> `ori-templates` → `ori-web` → `ori-web-demo`.  
> **Última atualização:** 2026-07-14 — D33 phase C; **D34 argon2 + phase D + generate-model**.

Este arquivo é a **fonte viva** das decisões e do que ainda falta discutir.
Atualizar status e decisões aqui a cada rodada de diálogo.

---

## 0. Contexto e princípios

### Meta de produto

| Fase | Package | Papel |
|------|---------|--------|
| 1 | **ori-templates** | Engine server-side estilo ERB (escape, layouts, partials) |
| 2 | **ori-web** | Router, static, JSON, cookies/session, integração templates |
| 3 | **ori-web-demo** | Full-stack HTML-first (htmx/forms) — caminho **C** |

### Filosofia Ori (invariantes de desenho)

1. **Reading-first** — templates leem como HTML + marcas óbvias.
2. **Explícito > mágico** — escape default; raw feio e consciente.
3. **Sem eval** — template **não** executa Ori arbitrário (subset fechado).
4. **`result` em erros** — parse/render falham de forma tipada e acionável.
5. **Safe by default** — XSS/CSRF/path jail como checklist + testes, não “port de framework”.
6. **Package externo** — não engordar `ori-lang` sob FREEZE-1.
7. **Criar em Ori** (API/control flow), inspirado em ERB/Mustache/Axum-chi; **não** portar Axum/Actix/libmicrohttpd literalmente. Crypto/TLS no runtime.

### Caminhos A e C

| ID | Nome | Papel |
|----|------|--------|
| **A** | Server templates (ERB-like em espírito) | Núcleo de `ori-templates` |
| **C** | HTML-first + little JS (htmx/forms) | Arquitetura + demo em cima de A + `ori-web` |

C **usa** A; não substitui A.

---

## 1. Decisões fechadas

| ID | Decisão | Data | Notas |
|----|---------|------|--------|
| D0 | Roadmap packages: templates → web → demo HTML-first | 2026-07-14 | Acordo de produto |
| D1 | Greenfield em Ori; não portar framework C/Rust de código | 2026-07-14 | Inspirar specs/conceitos; crypto/TLS no runtime |
| D2 | Explorar C (htmx) na arquitetura; implementar na fase demo | 2026-07-14 | |
| D3 | Família de delimitadores preferida: **`@{ }` (Ori-bracket)** | 2026-07-14 | Preferência do autor; mini-spec S1 ainda em aberto (detalhes) |
| D4 | Comentários de template: **`@{-- ... --}`** | 2026-07-14 | Não emite HTML; não é comentário HTML `<!-- -->` |
| D5 | Diretivas: **Dir-B** (keywords reservadas em `@{ }`) + **fecho nomeado opcional** (`@{ end }` ou `@{ /if }`) | 2026-07-14 | Ver mini-spec §4 |
| D6 | Raw: **`@{ expr \|> raw }`** (Raw-5) com regras SEC | 2026-07-14 | Só **último** estágio do pipe; ver §4 |
| D7 | Missing key/path: **strict** (erro de render) | 2026-07-14 | Sem string vazia silenciosa no default |
| D8 | `for`: **`for x in xs`** e **`for i, x in xs`** (índice 0-based) no v1 | 2026-07-14 | |
| D9 | Layouts: **L1 (`content`) no v1**; **L2 (slots) depois** | 2026-07-14 | |
| D10 | **`assign` no v1** | 2026-07-14 | binding local no template |
| D11 | Extensão (legado no diálogo): `.html.tmpl` | 2026-07-14 | **Superseded by D23** `.orix` |
| D12 | Helpers v1 allowlist — ver §4.8 | 2026-07-14 | `raw` só no fim do pipe |
| D13 | Path jail para templates + static — ver §5.1 | 2026-07-14 | SEC3 |
| D14 | `ori-web` mental model v1 — ver §10 | 2026-07-14 | W1 esboço |
| D15 | Session: **sid opaco + store servidor**; memória no v1; `SessionStore` pluggable | 2026-07-14 | SEC5 **FECHADO** |
| D16 | Cookie session: **HttpOnly**, **Secure** em prod/HTTPS, **SameSite=Lax** (Strict opt-in) | 2026-07-14 | SEC4 **FECHADO** |
| D17 | CSRF: **synchronizer token** + form hidden + `hx-headers`; POST/mutações; GET sem CSRF | 2026-07-14 | SEC6 **FECHADO** |
| D18 | Flash v1: **notice** / **error** (strings); PRG após POST de form | 2026-07-14 | C2 parcial |
| D19 | HTML-first C1: partials + htmx estático + `HX-Request` helper; convenções §11 | 2026-07-14 | C1 **FECHADO** |
| D20 | **Camadas extras de segurança** (defense-in-depth) — fases §5.5 | 2026-07-14 | além do núcleo session/CSRF |
| D21 | Futuro **opinativo Rails-like**: níveis Library → Batteries → App; package `ori-web-app`; convenções §12 | 2026-07-14 | adaptado à filosofia Ori |
| D22 | **APP1** árvore de pastas — ver §12.5 (fechado) | 2026-07-14 | controllers, views/users, partials, main raiz, routes explícitas, **ori.proj obrigatório** |
| D23 | Extensão de templates: **`.orix`** | 2026-07-14 | marca Ori; editores: associar `*.orix` → HTML |
| D24 | **APP2** rotas: **bloco DSL** `routes(app) … end` — ver §12.8 | 2026-07-14 | leitura opinativa; sob o capô RoutesBuilder |
| D25 | **APP3** controllers — T1–T9 como §12.9; **aliases de import e de tipo** (retornos) | 2026-07-14 | §12.9 fechado |
| D26 | **APP4** views — V1-A, V2-A, V3-A′, V4-A, V5-A, V6-B, V7-A, V8-A | 2026-07-14 | §12.10 fechado |
| D27 | **APP5–APP10** fechados por default (autor: decide restante) — §12.11 | 2026-07-14 | |
| D28 | Package **`packages/ori-templates`** MVP implementado | 2026-07-14 | |
| D29 | Package **`packages/ori-web`** MVP implementado | 2026-07-14 | router, session, CSRF, static, `dispatch`/`serve` (`handle` is reserved keyword) |
| D30 | **`packages/ori-web-demo`** HTML-first notes demo | 2026-07-14 | templates+web+htmx CDN; CSRF; PRG; partials |
| D31 | **Phase B** implemented + demos API/auth | 2026-07-14 | rate limit, CSP, file sessions, secret, regenerate; ports 3458/3459 |
| D32 | **`packages/ori-web-app`** + generators + `blog_app` example | 2026-07-14 | APP8 `bin/new`, `generate-controller`; library boot/render |
| D33 | **Phase C helpers** + `generate-scaffold` | 2026-07-14 | lockout, audit, re-auth, CSRF rotate; scaffold notes on blog_app |
| D34 | **C10 argon2id** + phase D docs + `generate-model` | 2026-07-14 | `ori.crypto`; ops checklist; domain stubs |

---

## 2. Decisões em aberto / tópicos

### Sintaxe / templates (S)

| ID | Tópico | Status | Notas |
|----|--------|--------|-------|
| **S1** | Família de delimitadores | **done** | `@{ }` + D4/D5/D6 |
| **S2** | Raw / unsafe print | **done** | D6: `\|> raw` |
| **S3** | Controle de fluxo (if/elif/else/for/end) | **done** | D5 + **D8** `for x in xs` e `for i, x in xs` |
| **S4** | Layouts v1 | **done** | **D9** L1 v1; L2 v2 |
| **S5** | Missing key | **done** | **D7** strict: missing path → erro de render |
| **S6** | Helpers allowlist | **done** | **D12** §4.8 |
| **S7** | Extensão de arquivo | **done** | **D23** `.orix` |
| **S8** | Comentários e whitespace (`-`) | **done** | Comentário D4; trim `@{ - … -}` em tags |
| **S9** | Pipe em print (`\|>`) | **done** | Parte de D6 + helpers |
| **S10** | `match` em template | aberto | Provável v2 |
| **S11** | Tags HTML `<Ori:If>` | **shelved** | Não preferida |

### Segurança (SEC)

| ID | Tópico | Status | Notas |
|----|--------|--------|-------|
| **SEC1** | Escape default + raw gated | **done** | D6 + D12 |
| **SEC2** | Strict missing keys | **done** | = D7 |
| **SEC3** | Path jail (static + include) | **done** | **D13** §5.1 |
| **SEC4** | Cookies HttpOnly / Secure / SameSite | **done** | **D16** |
| **SEC5** | Session sid + store + secret env | **done** | **D15**; secret prod obrigatório |
| **SEC6** | CSRF synchronizer | **done** | **D17** |
| **SEC7** | Body size / timeouts / header validation | **done** | defaults §10.5 + §5.5 fase A |
| **SEC8** | Suite testes (XSS, traversal, CSRF) | **done** | `packages/ori-web/examples/sec8_tests` + `tools/qa/web_sec8.sh` |
| **SEC9** | Crypto: runtime vs package | **done** (plano) | RNG/HMAC no runtime; argon2id na fase C10 (§5.5) |

### Web (W)

| ID | Tópico | Status | Notas |
|----|--------|--------|-------|
| **W1** | Router + Request/Response | **parcial** | **D14** esboço §10 |
| **W2** | Middleware pipeline explícito | **done** | `use_middleware` + catalog `mw_*` |
| **W3** | Static files | **done** | path jail + `static()` |
| **W4** | JSON helpers | **done** | flat + nested builders (`json_object`/`json_array`/`parse_json_nested`) |
| **W5** | Integração templates | **done** | `web_app.render` / `page_data` (Library free of templates cycle) |

### HTML-first C (C)

| ID | Tópico | Status | Notas |
|----|--------|--------|-------|
| **C1** | Partials + htmx / forms | **done** | **D19** |
| **C2** | Flash messages | **done** | **D18** notice/error |
| **C3** | Demo app mínima | **done** (plano) | esboço §11.7; implementar com package |


### App framework opinativo (Rails-like futuro)

| ID | Tópico | Status | Notas |
|----|--------|--------|-------|
| **APP0** | Níveis Library / Batteries / App | **done** | **D21** §12 |
| **APP1** | Árvore de pastas da app | **done** | **D22** §12.5 |
| **APP2** | Rotas: bloco DSL explícito | **done** | **D24** §12.8; convenção mágica nome↔URL = depois |
| **APP3** | Controllers: forma e onde vivem | **done** | **D25** §12.9 |
| **APP4** | Views: espelho de recursos | **done** | **D26** §12.10 |
| **APP5** | Helpers de view / ctx automático | **done** | **D27** §12.11 |
| **APP6** | `standard_app()` / boot | **done** | **D27** §12.11 |
| **APP7** | Environments (dev/prod) | **done** | **D27** §12.11 |
| **APP8** | Generators (`new`, `generate`) | **done** | D32 `bin/new`, `bin/generate-controller` |
| **APP9** | Config (`config/app`) | **done** | **D27** §12.11 |
| **APP10** | Fronteira com ORM/DB | **done** | **D27** fora do web; pasta opcional |

### Packages (P)

| ID | Tópico | Status | Notas |
|----|--------|--------|-------|
| **P1** | Escopo `ori-templates` | **MVP done** | D28 `packages/ori-templates` |
| **P2** | Escopo `ori-web` | **MVP done** | D29 `packages/ori-web` (Library; not App generators) |
| **P3** | Escopo `ori-web-demo` | **MVP done** | D30 `packages/ori-web-demo` (Library stack; not ori-web-app) |

---

## 3. Ordem sugerida de diálogo

```
S1 (delimitadores) → S2/SEC1 (raw) → S3 (fluxo) → S4 (layouts)
  → S5/SEC2 (strict) → S6/S9 (helpers/pipe) → S7/S8
  → W1–W5 → SEC3–SEC7 → C1–C3 → P1–P3 → SEC8–SEC9
```

Mini-spec templates **FECHADO** (D3–D13). ori-web + session/CSRF/htmx **FECHADO** (D14–D19). Hardening multi-fase **FECHADO no plano** (D20 §5.5). Próximo passo de produto: **implementação** (packages externos).

---

## 4. Mini-spec de sintaxe (acordado até D6)

### 4.0 Resumo

| Papel | Forma | Decisão |
|-------|--------|---------|
| Print escapado | `@{ expr }` | D3 |
| Comentário | `@{-- ... --}` | D4 |
| Diretiva | `@{ keyword ... }` (keywords reservadas) | **D5** |
| Fecho | `@{ end }` **ou** fecho nomeado `@{ /if }`, `@{ /for }`, … | **D5** |
| Raw | `@{ expr \|> raw }` (**último** estágio do pipe) | **D6** |

### 4.1 Print

```html
<h1>@{ user.name }</h1>
<p>@{ items |> len }</p>
```

- `expr`: subset (path, index literal/simples, calls allowlist, pipes de helpers).
- **Sempre** HTML-escape no final do print, **exceto** se o último estágio for `raw` (D6).

### 4.2 Comentário (D4)

```html
@{-- não vai para o HTML --}
@{--
  multi-linha
--}
```

### 4.3 Diretivas — Dir-B + fecho nomeado opcional (D5)

**Regra de parse:** se o primeiro token dentro de `@{ }` for uma **keyword reservada**, é diretiva; senão é print/expr.

**Keywords reservadas (v1 candidatas):**

| Keyword | Papel |
|---------|--------|
| `if` / `elif` / `else` | condicional |
| `for` / `in` | iteração (`for x in xs` ou `for i, x in xs` — D8) |
| `include` | partial |
| `layout` | layout wrapper |
| `assign` | binding local no template (opcional v1) |
| `end` | fecha o bloco mais interno |
| `/if` `/elif` `/else` `/for` `/include` `/layout` | fechos nomeados (opcionais) |
| `content` | yield do layout (ver S4) |
| `slot` | slots nomeados (v2 / S4) |

**Não** são keywords de print: nomes de campos iguais a keywords no **início** do delimitador são diretivas. Para imprimir um campo chamado `if`, usar path qualificado (`@{ data.if }`) ou helper — documentar.

#### Exemplo canônico (fecho genérico)

```html
@{-- users/show.html.tmpl --}
@{ layout "layouts/app" }
  <h1>@{ user.name }</h1>

  @{ if user.is_admin }
    <p class="badge">admin</p>
  @{ elif user.is_mod }
    <p class="badge">mod</p>
  @{ else }
    <p class="badge">member</p>
  @{ end }

  <ul>
  @{ for item in items }
    <li>@{ item.title }</li>
  @{ end }
  </ul>

  @{ include "partials/nav" }
@{ end }
```

#### Mesmo exemplo (fechos nomeados opcionais)

```html
@{ layout "layouts/app" }
  <h1>@{ user.name }</h1>

  @{ if user.is_admin }
    <p class="badge">admin</p>
  @{ elif user.is_mod }
    <p class="badge">mod</p>
  @{ else }
    <p class="badge">member</p>
  @{ /if }

  <ul>
  @{ for item in items }
    <li>@{ item.title }</li>
  @{ /for }
  </ul>

  @{ include "partials/nav" }
@{ /layout }
```

**Regras de fecho nomeado:**

1. `@{ end }` fecha o bloco aberto mais interno (qualquer tipo).
2. `@{ /if }` só é válido se o bloco interno aberto for `if` (idem `/for`, `/layout`, …).
3. Mismatch → erro de parse (`TemplateError.unclosed` / `.mismatch_end`).
4. `elif` / `else` não usam `/elif` obrigatório; fechar o `if` com `end` ou `/if`.

### 4.4 Raw — pipe `|> raw` (D6) + SEC1

```html
@{-- HTML confiado (ex.: markdown já sanitizado na app) --}
<div class="bio">@{ user.bio_html |> raw }</div>

@{-- OK: raw só no fim --}
@{ body |> trim |> raw }

@{-- PROIBIDO: raw no meio do pipe --}
@{-- @{ body |> raw |> upper }  --}
```

**Regras normativas (engine deve enforce):**

1. `raw` só é válido como **último** estágio do pipe (ou único estágio após expr).
2. Após `raw`, **não** há HTML-escape.
3. Sem `raw`, o valor final do print **sempre** passa por HTML-escape.
4. Não existe `@{! … }` nem `{{{ … }}}` na sintaxe canônica.
5. Testes golden: `@{ "<script>" }` → entities; `@{ x |> raw }` com x perigoso → byte-a-byte (documentar risco).

**Pipe de helpers “safe”** (S6 — lista a fechar): `len`, `string`, `upper`, `lower`, `trim`, `truncate`, `url_encode`, `json` (encoding para contexto JSON, não raw HTML).

### 4.5 Layouts (S4 — o que é e L1 vs L2)

#### O problema

Toda página tem um “casco” igual (html, head, nav, footer) e um “miolo” diferente.
**Layout** = o casco. **Página** = o miolo. O engine **encaixa** o miolo no casco.

Analogia: moldura de quadro (layout) + pintura (página).

#### L1 — um buraco só: `content` (simples)

O layout tem **um** lugar marcado `@{ content }`.  
Tudo que está **dentro** de `@{ layout "..." } ... @{ end }` na página vira esse miolo.

```html
@{-- layouts/app.html.tmpl  (a moldura) --}
<!DOCTYPE html>
<html>
<head>
  <title>@{ title }</title>
  <link rel="stylesheet" href="/assets/app.css">
</head>
<body>
  <nav>…menu fixo…</nav>

  <main>
    @{ content }
  </main>

  <footer>© Ori</footer>
</body>
</html>
```

```html
@{-- users/show.orix  (a pintura) --}
@{ layout "layouts/app" }
  <h1>Home</h1>
  <p>Olá, @{ user.name }</p>
@{ end }
```

**Resultado mental do render de `pages/home`:**

```html
<!DOCTYPE html>
<html>
<head>… title do ctx …</head>
<body>
  <nav>…</nav>
  <main>
    <h1>Home</h1>
    <p>Olá, Ana</p>
  </main>
  <footer>…</footer>
</body>
</html>
```

- `title` (e o resto do casco) vêm do **contexto** (`ctx`) ou de `@{ assign title = "Home" }` se tivermos assign.
- O **único** encaixe estrutural é `content` = corpo da página.

**Quando basta L1:** quase todos os sites, blogs, admin, demo htmx.

#### L2 — vários buracos: `slot` (mais flexível)

Às vezes o casco tem **vários** encaixes: título no `<title>`, ações no header, miolo, sidebar.

```html
@{-- layouts/app.html.tmpl --}
<!DOCTYPE html>
<html>
<head>
  <title>@{ slot "title" }</title>
</head>
<body>
  <header>
    <h1>@{ slot "title" }</h1>
    <div class="actions">@{ slot "actions" }</div>
  </header>
  <main>@{ slot "body" }</main>
</body>
</html>
```

```html
@{-- users/show.orix --}
@{ layout "layouts/app" }
  @{ slot "title" }Home@{ end }

  @{ slot "actions" }
    <a href="/new">Novo</a>
  @{ end }

  @{ slot "body" }
    <p>Olá, @{ user.name }</p>
  @{ end }
@{ end }
```

**Quando L2 ajuda:** layouts com header/sidebar/ações por página.  
**Custo:** mais sintaxe, mais regras (slot em falta? default?).

#### Decisão de produto (D9)

| Versão | Escolha |
|--------|---------|
| **v1** | **L1** (`content` + dados no `ctx` e/ou `assign`) |
| **v2+** | **L2** slots quando necessário |

### 4.5.2 `assign` (D10)

Binding **local ao render** (não muta o ctx da app de forma global além do scope do template).

```html
@{ layout "layouts/app" }
  @{ assign title = "Home" }
  @{ assign full_name = user.name }
  <h1>@{ full_name }</h1>
@{ end }
```

- `assign` é keyword reservada (D5).
- RHS: mesma `expr` de print (paths, helpers, pipes), **sem** `raw` no assign (assign guarda valor tipado/string no ctx de template; escape só no print).
- Útil com L1 para preencher `@{ title }` no layout.

### 4.5.3 Extensão de arquivo (D11)

- Canônica: **`*.html.tmpl`**
- Exemplos: `layouts/app.html.tmpl`, `users/show.orix`, `partials/nav.html.tmpl`
- Nome lógico: `"pages/home"` → `{root}/users/show.orix` (sem `..`, ver D13).

### 4.5.1 `for` com índice (D8)

```html
@{ for item in items }
  <li>@{ item.title }</li>
@{ end }

@{ for i, item in items }
  <li>@{ i }: @{ item.title }</li>
@{ /for }
```

- `i` é **0-based** (como índices Ori / a maioria das langs de sistemas).
- Forma sem índice continua válida.


### 4.8 Helpers v1 (D12)

Só funções **puras** (sem I/O, sem net). Usáveis em print e em RHS de `assign`, via call ou pipe.

| Helper | Aridade | Papel | Exemplo |
|--------|---------|--------|---------|
| `string` | 1 | coerção para texto de display | `@{ n \|> string }` |
| `len` | 1 | comprimento list/string/map | `@{ items \|> len }` |
| `trim` | 1 | trim whitespace | `@{ s \|> trim }` |
| `upper` | 1 | maiúsculas | `@{ s \|> upper }` |
| `lower` | 1 | minúsculas | `@{ s \|> lower }` |
| `truncate` | 2 | cortar com `…` se passar do max | `@{ s \|> truncate(40) }` |
| `default` | 2 | se missing/**vazio**, usa fallback (**não** contorna strict de path inexistente no v1) | `@{ nick \|> default("anon") }` só se `nick` existe e é vazio; path missing → erro D7 |
| `url_encode` | 1 | encode para query/path segment | `@{ q \|> url_encode }` |
| `json` | 1 | serializa para JSON **texto** (seguro p/ `<script type="application/json">`) | `@{ data \|> json }` |
| `raw` | 1 | **último** estágio: sem HTML-escape (D6) | `@{ html \|> raw }` |

**Fora do v1:** `safe_html` sanitizer completo, markdown, i18n, date format rico (podem entrar depois).

**Strict + default:** no v1, `default` **não** mascara path inexistente (`user.x` missing → erro). Só aplica a valores presentes e “vazios” (string `""`, list vazia — política exata na implementação).

### 4.9 Whitespace trim (S8 — adiado)

Controle estilo Jinja (`-`) **não** é v1. Pode entrar em v1.1 se o HTML gerado ficar ruidoso.

### 4.7 API Ori candidata (`ori-templates`)

```ori
-- carrega raiz de views (path jail = root)
templates.open(root: string) -> result[TemplateEngine, TemplateError]

-- nome lógico: "pages/home" → root/users/show.orix
engine.render(name: string, ctx: map[string, any]) -> result[string, TemplateError]

engine.render_string(source: string, ctx: map[string, any]) -> result[string, TemplateError]
```

Erros (esboço): `parse`, `not_found`, `undefined` (strict missing), `mismatch_end`, `invalid_raw_pipe`, `path_escape`.

### 4.6 Histórico de alternativas (não canônicas)

Ver log de diálogo; Dir-A/C/D/E/F e Raw-1..4,6..8 documentados em commits/sessões anteriores. **Canônico = §4.0–4.4.**

## 5. Segurança — como “do zero” não é “sem higiene”

Criar `ori-web` / `ori-templates` em Ori **não** significa reinventar TLS.

| Camada | Responsabilidade |
|--------|------------------|
| App | Authz, regras de negócio |
| ori-web | CSRF, cookies, limits, headers, static jail |
| ori-templates | Escape default, raw gated, includes jailed |
| Runtime / OS | TLS, RNG, HMAC/hash primitives |

Checklist amarrado a **SEC\*** neste doc; implementação exige **testes golden** (XSS, path traversal, CSRF).

### 5.1 Path jail (D13 / SEC3)

Aplica-se a: `templates.open(root)`, `include`, `layout`, e `web.static` (ori-web).

| Regra | Comportamento |
|-------|----------------|
| Root fixo | Todo path é resolvido sob `root` absoluto (após canonicalize) |
| Nome lógico | `"pages/home"` → `root/users/show.orix` |
| Separador | `/` no nome lógico; rejeitar `\` |
| `..` | **Proibido** em qualquer segmento → `path_escape` |
| Absoluto | Rejeitar `/etc/passwd`, `C:\...` |
| Symlink | Opcional v1: se o OS seguir symlink, o path **final** ainda deve ficar sob root (se não der para garantir, documentar e preferir realpath check) |
| Include/layout | Mesmas regras; include de fora do root = erro |
| Static | `GET /assets/x` mapeia para `static_root/x` com o mesmo jail |

Testes golden obrigatórios: `../`, encoded `..%2F`, double dots, absolute paths.

### 5.2 Cookies e session (SEC4–5) — **FECHADO** (D15–D16, D18)

#### O que é o quê

| Conceito | Papel |
|----------|--------|
| **Cookie** | Par nome=valor que o browser guarda e reenvia |
| **Session** | Dados no **servidor** (ou blob assinado) indexados por um id no cookie |
| **Session cookie** | Cookie que carrega o **id** (ou o blob) da session |

HTML-first (C) quase sempre precisa de session: login, flash, CSRF token.

#### Modelo recomendado v1: **session id opaco + store no servidor**

```
Browser                     ori-web
   |  Cookie: ori_sid=...      |
   |-------------------------->|
   |                     lookup store[sid]
   |                     req.session = dados
   |                     handler
   |                     Set-Cookie se novo/rotated
```

**Por quê não “cookie só com JSON assinado” no v1?**

| Abordagem | Prós | Contras |
|-----------|------|---------|
| **A. Sid + store servidor** (recomendado) | Revogar session; dados grandes; invalidar no logout | Precisa store (memória v1; Redis depois) |
| **B. Cookie assinado (JWT-like / sealed)** | Stateless | Difícil revogar; tamanho limitado; roubo = válido até exp |

**v1:** store **em memória** (dev/single process) + interface `SessionStore` para trocar depois.  
**Produção séria:** store externo (fase seguinte), não inventar cluster no v1.

#### Cookie `ori_sid` — defaults (SEC4)

| Atributo | Valor v1 | Por quê |
|----------|----------|---------|
| Nome | `ori_sid` (configurável) | explícito |
| `HttpOnly` | **true** | JS não lê (mitiga XSS → roubo de session) |
| `Secure` | **true** em prod / se request HTTPS; false só dev HTTP | não vazar em cleartext |
| `SameSite` | **Lax** default | bom equilíbrio forms top-level; Strict opcional |
| `Path` | `/` | |
| `Max-Age` / Expires | session idle + absolute timeout | ex.: idle 1h, absolute 24h (config) |

#### Session payload (mínimo)

```text
session id: random 256-bit (os RNG / runtime)
data: map[string, string] ou map tipado depois
  - user_id?
  - csrf_token?
  - flash?
created_at, last_seen
```

#### Secrets (SEC5 / SEC9)

| Secret | Uso |
|--------|-----|
| `ORI_WEB_SECRET` (env) | Assinar cookie de session **se** usarmos signed envelope; ou assinar flash; **obrigatório em prod** |
| Sid aleatório | Não precisa HMAC se sid é opaco e store é confiável; HMAC entra se cookie carregar dados |

**Boot:** se `env=production` e secret ausente/curto → **falha ao subir** (não default `"secret"`).

#### API mental

```ori
-- middleware
app = web.use(app, web.session(store, options))

-- no handler
web.session_get(req, "user_id") -> optional[string]
web.session_set(req, "user_id", id)
web.session_delete(req, "user_id")
web.session_regenerate(req)   -- após login (fixa session fixation)
web.flash(req, "notice", "Saved.")  -- vive uma request
web.take_flash(req, "notice") -> optional[string]
```

`Request` ganha campo opaco `session` preenchido pelo middleware (não manipular cookie na mão no app).

#### Session fixation

Após login bem-sucedido: **sempre** `session_regenerate` (novo sid, copia dados úteis, invalida sid antigo).

---

### 5.3 CSRF (SEC6) — **FECHADO** (D17)

#### O problema

Site malicioso faz o browser do user **autenticado** enviar `POST` ao teu app (cookie vai junto). Sem CSRF check, a ação executa.

#### Onde CSRF importa

| Método / contexto | CSRF? |
|-------------------|--------|
| `GET` (safe) | Não (GETs não mudam estado — regra de ouro) |
| `POST` / `PUT` / `PATCH` / `DELETE` form HTML | **Sim** |
| `POST` JSON de SPA com header custom | frequentemente SameSite + header; HTML-first foca em forms |
| htmx `hx-post` | **Sim** (é POST com cookies) |

#### Abordagem recomendada v1: **synchronizer token**

1. Session guarda `csrf_token` (random).  
2. Form/partial inclui campo hidden ou header.  
3. Middleware em mutações: token do request **==** token da session.

```html
<form method="post" action="/users">
  <input type="hidden" name="csrf_token" value="@{ csrf_token }">
  ...
</form>
```

htmx:

```html
<body hx-headers='{"X-CSRF-Token": "@{ csrf_token }"}'>
```

ou meta + JS mínimo; preferir **header global no layout** via `hx-headers` no body.

#### Double-submit cookie? 

| | Synchronizer (recomendado) | Double-submit |
|--|----------------------------|---------------|
| Store | token na session | cookie + form iguais |
| Revogação / login | natural com session | ok |
| Complexidade | média | um pouco menor |
| Fit HTML-first + session | **alto** | ok se session fraca |

**v1 = synchronizer** amarrado à session (D-session).

#### API mental

```ori
app = web.use(app, web.csrf())   -- depois de session

-- template ctx sempre pode expor csrf_token se middleware ativo
-- ou helper:
web.csrf_token(req) -> string

-- mutações sem token válido → 403
```

**Isenções:** rotas opt-in `web.csrf_exempt` só para webhooks com outro auth (assinatura HMAC do provedor) — documentar perigo.

#### Checklist testes SEC8 (csrf)

- POST sem token → 403  
- POST token errado → 403  
- POST token ok → 200  
- GET não exige token  

---

### 5.4 Ordem de middleware recomendada

```text
log → limit_body → session → csrf → route handler
```

CSRF **depois** de session (precisa do token).  
Handler vê `req` já com session e csrf validado em POST.


### 5.5 Camadas extras de segurança — **FECHADO no plano** (D20)

O núcleo (escape, jail, session, CSRF, SameSite, body limit) é o **piso**.  
Abaixo: camadas **adicionais** em fases. Não substituem o núcleo; **somam**.

#### Fase A — v1 do `ori-web` (obrigatório no MVP framework)

| Camada | O quê | Notas |
|--------|--------|--------|
| A1 Escape + raw gated | templates | já D6/D12 |
| A2 Path jail | include/layout/static | D13 |
| A3 Session sid + store | cookie HttpOnly/Secure/SameSite | D15–D16 |
| A4 CSRF synchronizer | mutações | D17 |
| A5 Body size limit | default 1 MiB | configurável |
| A6 Header injection guard | rejeitar CR/LF em nomes/valores de header | |
| A7 Secret de produção | `ORI_WEB_SECRET` (ou nome final) — **fail boot** se prod e ausente/curto | |
| A8 Session regenerate | após login | anti-fixation |
| A9 Timeouts de session | **idle** + **absolute** (ex. 1h / 24h, config) | |
| A10 Testes golden | XSS, path, CSRF, cookie flags em fixtures | SEC8 |

#### Fase B — v1.1 / demo “site real” (obrigatório antes de chamar de produção)

| Camada | O quê | Notas |
|--------|--------|--------|
| B1 HTTPS | TLS no edge (reverse proxy) **ou** listener TLS do runtime | HTTP local só dev |
| B2 `Secure` cookie sempre atrás de HTTPS | | |
| B3 Session store pluggable | interface; **memória** dev; **Redis/SQLite/file** doc para prod | multi-process |
| B4 Rate limit básico | login / POST sensíveis (por IP) | DoS leve / brute force |
| B5 Flash + PRG | D18 | |
| B6 Security headers baseline | `X-Content-Type-Options: nosniff`, `Referrer-Policy`, `X-Frame-Options` ou CSP frame-ancestors | |
| B7 Request timeout | se o runtime permitir | |

#### Fase C — hardening (pós-MVP, plano explícito)

| Camada | O quê | Notas |
|--------|--------|--------|
| C1 **CSP** (Content-Security-Policy) | começar restritivo; afrouxar p/ htmx inline se preciso | documentar tradeoff htmx |
| C2 **Re-auth** / senha de novo | ações sensíveis (trocar email, apagar conta, “admin destroy”) | |
| C3 **2FA / TOTP** (opcional package) | **done** | `packages/ori-web-auth` + `ori.crypto.totp_*` |
| C4 Lockout / backoff login | após N falhas | com B4 |
| C5 Audit log | login, logout, mudanças sensíveis | |
| C6 CSRF rotation | novo token após uso opcional | defesa extra |
| C7 Cookie `__Host-` prefix | se path=/ e Secure e sem Domain | quando HTTPS estável |
| C8 Upload seguro | size, MIME allowlist, store fora de webroot | quando houver upload |
| C9 Dependabot / supply chain | no monorepo dos packages | processo |
| C10 Password hashing | **argon2id** (ou bcrypt) via runtime/FFI — **nunca** MD5/SHA solto | SEC9 |

#### Fase D — ops / edge (fora do código Ori, checklist de deploy)

| Camada | O quê |
|--------|--------|
| D1 Reverse proxy (Caddy/nginx) | TLS, HTTP/2, HSTS |
| D2 HSTS | só com HTTPS estável |
| D3 Backups do session store / DB | |
| D4 Least privilege do processo | user não-root |
| D5 Secrets só em env/secret manager | nunca no git |

#### Princípio (D20)

```text
Núcleo (A) = default do framework e impossível de “esquecer” no happy path.
Extras (B–D) = plano de maturidade; demo ensina A+B; produção exige B+D e C conforme risco.
```

App de alto risco (pagamentos, saúde) deve assumir **C2–C5** cedo, não só o piso A.




---


---

## 10. ori-web — mental model v1 (D14)

Inspirado em Axum/chi **conceitualmente**, API Ori explícita com `result`.

### 10.1 Tipos (esboço)

```ori
struct Request
    method: string          -- "GET", "POST", …
    path: string            -- path sem query
    query: map[string, string]
    headers: map[string, string]
    body: bytes             -- ou string; limits em SEC7
    params: map[string, string]  -- preenchido pelo router (:id)
    -- session: preenchida pelo middleware (D15)
end

struct Response
    status: int
    headers: map[string, string]
    body: bytes
end

-- helpers de construção
web.text(status, body: string) -> Response
web.html(status, body: string) -> Response
web.json(status, value) -> Response      -- usa ori.json
web.redirect(status, location: string) -> Response
web.file(path) -> result[Response, WebError]  -- sob static jail
```

Handler:

```ori
type Handler = fn(req: Request) -> result[Response, WebError]
```

### 10.2 Router

```ori
var app: web.App = web.app()

web.get(app, "/", home)
web.get(app, "/users/:id", user_show)
web.post(app, "/users", user_create)
web.static(app, "/assets", "public")   -- jail em public/

web.listen(app, "0.0.0.0:3000") -> result[void, WebError]
```

- Params `:id` → `req.params["id"]`.
- Primeira rota que casa ganha (ordem de registro).
- 404 default se nenhuma casar.

### 10.3 Middleware (W2 — **FECHADO** ordem)

```ori
type Middleware = fn(next: Handler) -> Handler

app = web.use(app, web.log)
app = web.use(app, web.limit_body(1_048_576))
app = web.use(app, web.session(store, session_opts))
app = web.use(app, web.csrf())
-- opcional fase B: web.rate_limit(...), security headers
```

Ordem normativa: **log → limit_body → session → csrf → handler** (§5.4).

### 10.4 Templates + web (W5)

```ori
var views: templates.TemplateEngine = templates.open("views")?

home(req: Request) -> result[Response, WebError]
    var ctx: map[string, any] = { "title": "Home", "user": current_user(req) }
    match templates.render(views, "pages/home", ctx)
    case ok(html):
        return ok(web.html(200, html))
    case err(e):
        return err(web.template_error(e))
    end
end
```

Ou sugar posterior: `web.render(views, "pages/home", ctx)`.

### 10.5 Defaults de segurança no listen (SEC7 parcial)

| Default v1 | Valor |
|------------|--------|
| Max body | 1 MiB (configurável) |
| Header names/values | rejeitar CR/LF |
| Request timeout | configurável; default conservador se o runtime permitir |
| Server header | mínimo ou genérico (não vazar versões) |

### 10.6 Fora do v1 mínimo vs fases B–D

| Fora do núcleo A (pode ser B/C/D) | Notas |
|----------------------------------|--------|
| WebSocket | depois |
| Multipart upload | fase C8 |
| HTTP/2 push | edge |
| TLS na app | B1 ou proxy D1 |
| 2FA, re-auth, CSP rígida | fase C |
| Redis session | B3 |


---

## 11. HTML-first + htmx (C1) — **FECHADO** (D19)

### 11.1 Ideia

O servidor Ori devolve **HTML**. O browser pede **páginas inteiras** ou **fragments** (partials).  
**htmx** (script estático em `/assets`) adiciona AJAX declarativo sem SPA.

```
GET  /users          → HTML full (layout + lista)
GET  /users/rows     → só <tbody>… (partial)
POST /users          → redirect ou partial + flash
```

### 11.2 Convenções de rotas (proposta)

| Padrão | Uso |
|--------|-----|
| `GET /recurso` | página full |
| `GET /recurso/partials/…` ou `GET /recurso/rows` | fragmento htmx |
| `POST /recurso` | mutação; CSRF |
| Preferir **redirect 303** após POST (PRG) para full page | evita re-POST |
| htmx mutação pode devolver **partial 200** para swap | sem full reload |

Detectar request htmx (opcional helper):

```text
Header HX-Request: true
```

```ori
web.is_htmx(req) -> bool
```

### 11.3 Partials = mesmos templates

```html
@{-- pages/users/index.html.tmpl --}
@{ layout "layouts/app" }
  <h1>Users</h1>
  <table>
    <tbody id="rows"
           hx-get="/users/rows"
           hx-trigger="every 30s"
           hx-swap="innerHTML">
      @{ include "pages/users/rows" }
    </tbody>
  </table>
  <button hx-get="/users/rows" hx-target="#rows" hx-swap="innerHTML">
    Refresh
  </button>
@{ end }
```

```html
@{-- pages/users/rows.html.tmpl  (sem layout) --}
@{ for i, u in users }
  <tr>
    <td>@{ i }</td>
    <td>@{ u.name }</td>
  </tr>
@{ /for }
```

```ori
users_rows(req: Request) -> result[Response, WebError]
    -- sem layout: partial puro
    html = templates.render(views, "pages/users/rows", ctx)?
    return ok(web.html(200, html))
end
```

### 11.4 Layout + htmx + CSRF

```html
@{-- layouts/app.html.tmpl --}
<!DOCTYPE html>
<html>
<head>
  <title>@{ title }</title>
  <script src="/assets/htmx.min.js" defer></script>
</head>
<body hx-headers='{"X-CSRF-Token": "@{ csrf_token }"}'>
  <nav>…</nav>
  @{-- flash --}
  @{ if flash_notice }
    <div class="flash">@{ flash_notice }</div>
  @{ end }
  <main>@{ content }</main>
</body>
</html>
```

- `csrf_token` e `flash_notice` injetados no **ctx** pelo middleware (ou locals do render).  
- Um `hx-headers` no `body` cobre `hx-post` / `hx-delete` sem repetir token.

### 11.5 Forms clássicos (sem htmx)

```html
<form method="post" action="/users">
  <input type="hidden" name="csrf_token" value="@{ csrf_token }">
  <input name="name" value="@{ form.name }">
  <button type="submit">Save</button>
</form>
```

Após POST ok: `web.redirect(303, "/users")` + `web.flash(req, "notice", "Created")`.

### 11.6 O que **não** fazer no C1

- Não reinventar virtual DOM em Ori  
- Não exigir build step de JS (htmx = arquivo estático)  
- Não usar GET para mutações  
- Não devolver JSON para telas HTML-first no demo (JSON fica para API opcional)

### 11.7 Demo mínima (C3 esboço)

1. Lista de itens em memória (ou sqlite depois)  
2. Full page GET `/`  
3. Partial GET `/items/rows`  
4. POST cria item + CSRF + flash + redirect  
5. Static `/assets/htmx.min.js`  


---

## 12. Futuro opinativo Rails-like (D21) — **no plano**

### 12.1 Por quê

O stack **D0–D20** é a base **modular e segura** (biblioteca + HTML-first).  
Rails prova que produtividade explode com **convenção + defaults + generators**.

Não vamos clonar Active Record / magic metaprogramming. Vamos:

- copiar o **espírito** (omakase, pastas, um caminho feliz);
- adaptar à **filosofia Ori**: reading-first, explícito, `result`, pouca magia opaca;
- manter a base **composable** para quem quiser só API JSON.

### 12.2 Três níveis (arquitetura de produto)

```text
Nível 1 — Library     ori-templates + ori-web (peças soltas)
Nível 2 — Batteries   web.standard_stack() / defaults session+CSRF+log+limits
Nível 3 — App         ori-web-app: pastas, boot, generators, convenções
```

| Nível | Package | Usuário típico |
|-------|---------|----------------|
| 1 | `ori-templates`, `ori-web` | controlo total, microserviços, API |
| 2 | helpers em `ori-web` | app pequena sem generators |
| 3 | **`ori-web-app`** (futuro) | “quero um site/blog/admin ao estilo Rails” |

**Implementação:** níveis 1–2 primeiro (já especificados).  
**Nível 3:** convenções definidas **agora** (este §12); código **depois** da base estável.

### 12.3 Princípios das convenções (anti-magia)

1. **Convenção gera código legível** — generators escrevem handlers Ori normais, não metaclasses.  
2. **Defaults seguros ligados** — session/CSRF/escape no caminho feliz do App.  
3. **Override sempre possível** — rota manual, partial fora da pasta, API sem views.  
4. **Um significado por pasta** — reading-first no repositório.  
5. **Nomes estáveis** — snake_case, paths com `/`, templates `.orix`.  
6. **Sem eval / sem DSLs opacas** no template (já D3–D12).

### 12.4 Mapa Rails → Ori (inspiração, não clone)

| Rails | Ori App (futuro) | Notas |
|-------|------------------|--------|
| `rails new` | `ori-web-app new myapp` (nome a fechar) | scaffold de pastas |
| `app/controllers` | `app/handlers` | menos jargão MVC forçado; ou `app/controllers` se preferirmos familiaridade — **APP3** |
| `app/views` | `views/` | templates |
| `app/models` | **não no web core** | DB package depois; pasta opcional `app/models` ou `app/domain` |
| `config/routes.rb` | `config/routes.orl` **explícito no v1 do App** | convenção nome↔rota = v2 opcional |
| `ApplicationController` | `app/application.orl` helpers partilhados | funções, não herança mágica |
| `before_action` | middleware + hooks explícitos | |
| ERB | `@{ }` templates | já fechado |
| `form_with` + CSRF | helpers que injetam token | APP5 |
| Active Record | **fora** | `ori-sqlite` / futuro ORM |
| Asset pipeline | `public/` static | htmx sem bundler no v1 |
| Action Cable | fora | |

### 12.5 Árvore da app — **FECHADO** (D22, D23)

#### `ori.proj` — **sim, obrigatório**

Um site/app web **continua a ser um projeto Ori**. O framework não substitui o manifesto da linguagem.

| Ficheiro | Papel |
|----------|--------|
| **`ori.proj`** | **Obrigatório** para app: `kind`, `entry` (`main.orl`), `root_namespace`, etc. (ver `repo-and-project-layout.md`) |
| **`ori.pkg.toml`** | Para **bibliotecas publicáveis** (`ori-web`, `ori-templates` como packages), não para o dia a dia do site |
| Framework | Só define pastas **além** do que o `ori` já espera (`entry`, sources) |

Sem `ori.proj`, `ori run` / `ori check` / layout de projeto da linguagem não têm contrato estável.  
O “Rails-like” **assenta em** `ori.proj` + `main.orl`, não os remove.

#### Extensão de templates: **`.orix`** (D23)

| | `.html.tmpl` (legado D11) | **`.orix` (D23)** |
|--|---------------------------|-------------------|
| Marca Ori | fraca | **forte** |
| “É HTML?” no Explorer | óbvio | associação no editor |
| Highlight | HTML out of the box | `files.associations` → HTML (+ grammar `@{ }` depois) |

**Decisão:** templates usam **`.orix`**.  
Documentação do App/editores: associar `*.orix` a HTML (e, no futuro, grammar com scopes `@{ }`).

Nome lógico no `render` / `include` / `layout`: `"users/show"` → `views/users/show.orix`.

#### Árvore canónica (D22)

```text
myapp/
  ori.proj                 -- OBRIGATÓRIO (entry = main.orl, kind = app)
  main.orl                 -- boot: web_app.run() / standard stack
  config/
    app.orl                -- port, env flags (secrets só via env)
    routes.orl             -- rotas EXPLÍCITAS
  app/
    application.orl        -- helpers partilhados (funções)
    controllers/           -- HTTP handlers (nome familiar Rails)
      home.orl
      users.orl
  views/
    layouts/
      app.orix
    users/
      show.orix
      rows.orix            -- partial do recurso (ao lado do recurso)
    partials/
      nav.orix             -- partials partilhados entre recursos
  public/
    assets/
      htmx.min.js
      app.css
```

#### Decisões A–E (fechadas)

| # | Tema | Escolha |
|---|------|---------|
| A | Pasta HTTP | **`app/controllers`** |
| B | Views por recurso | **`views/users/`** (não `views/pages/users/`) |
| C | Partials | **`views/partials/`** partilhados; partial **local ao recurso** permitido em `views/users/*.orix` |
| D | Entry | **`main.orl` na raiz** |
| E | Rotas App v1 | **`config/routes.orl` explícito** |

#### `ori.proj` exemplo mental (app web)

```toml
manifest = 1
name = "myapp"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[source]
root_namespace = "app"
```

Dependências de package (`ori-web`, `ori-templates`) conforme ecosystem (path/git/registry) — ver guidelines; não eliminam o `ori.proj`.


### 12.8 Rotas — **FECHADO** (D24) bloco DSL

#### Decisão

App v1 usa **rotas explícitas** num ficheiro `config/routes.orl`, com **sintaxe de bloco DSL** (inspirada em `routes.rb`), não convenção mágica controller#action → URL (isso fica para v2 opcional).

#### Forma canónica (leitura do App)

```ori
module app.routes

import ori.web_app = web_app
import app.controllers.home = home
import app.controllers.users = users

-- Desenho: um bloco que declara o mapa HTTP da app.
public draw(app: web_app.App) -> void
    routes(app)
        get "/", home.index
        get "/users", users.index
        get "/users/:id", users.show
        post "/users", users.create
        get "/users/rows", users.rows
        static "/assets", "public"
    end
end
```

#### Semântica

| Linha | Significado |
|-------|-------------|
| `routes(app)` … `end` | Abre o DSL ligado a esta `App` |
| `get path, action` | `GET path` → função `action` |
| `post` / `put` / `patch` / `delete` | idem |
| `static url_prefix, dir` | ficheiros estáticos (path jail) |
| `action` | referência a função `fn(Request) -> result[Response, WebError]` (ex. `users.show`) |
| `:id` | param → `req.params["id"]` |
| Ordem | **primeira rota que casa ganha** |

#### Alinhamento com a linguagem Ori (importante)

A superfície acima é o **contrato de leitura do App layer**.  
A implementação **não** exige magia no compiler Ori:

**Forma expandida equivalente (Library, 100% Ori atual):**

```ori
public draw(app: web.App) -> void
    web.routes(app, fn(r: web.RouteDrawer)
        r.get("/", home.index)
        r.get("/users/:id", users.show)
        r.post("/users", users.create)
        r.static("/assets", "public")
    end)
end
```

| Camada | Sintaxe |
|--------|---------|
| **Library `ori-web`** | só `web.routes(app, fn(r) … end)` + `r.get(...)` |
| **App `ori-web-app`** | sugar `routes(app) … get … end` que **desuga** para o builder (macro leve do package, preprocessor, ou funções que o `draw` interpreta) |

Filosofia: o sugar é **no package App**, não na linguagem core (FREEZE).  
Quem usa só Library nunca vê o DSL.

#### O que o DSL **não** faz no v1

- Nested resources automáticos (`resources :users`) — v2 opcional  
- `root to:` sem path — usar `get "/", …`  
- Constraints regex complexas — depois  
- Site/engine mounts — depois  

#### Erros

- action não é função com a assinatura esperada → erro ao registar ou no boot  
- path duplicado + mesmo method → erro de boot (ou warning configurável)  
- static fora do jail → `path_escape`  

#### Exemplo mental `config/routes.orl` completo

```ori
module app.routes

import ori.web_app = web_app
import app.controllers.home = home
import app.controllers.users = users

public draw(app: web_app.App) -> void
    routes(app)
        get "/", home.index
        get "/users", users.index
        get "/users/:id", users.show
        post "/users", users.create
        get "/users/rows", users.rows
        static "/assets", "public"
    end
end
```

`main.orl` / boot chama `app.routes.draw(app)` após criar a App e o stack de middleware.


### 12.9 APP3 — Controllers — **FECHADO** (D25)

Controllers vivem em `app/controllers/*.orl` (D22).

#### Decisões T1–T9

| Tópico | Escolha | Significado |
|--------|---------|-------------|
| T1 Assinatura | **A** | `action(req) -> result[Response, WebError]` |
| T2 Nomes | **A** | REST Rails como convenção + nomes livres (`rows`, …) |
| T3 Ficheiros | **A** | Um módulo por recurso (`users.orl`) |
| T4 application | **A** | Funções partilhadas explícitas (sem herança) |
| T5 Views | **D** | `application.render` ligado ao engine no boot |
| T6 Render | **A** | `return application.render(...)` / `render_partial` |
| T7 Filtros | **A** | Wrappers na rota: `authenticate(users.show)` |
| T8 Erros | **A** | `web.not_found()`, redirect, `result` |
| T9 Visibilidade | **A** | Actions `public`; helpers internos não exportados |

#### Aliases (norma de estilo App) — imports **e** tipos/retornos

Duas camadas (ambas desejadas):

**1) Alias de módulo (import)**

```ori
import ori.web = web
import app.application = application
import app.controllers.users = users
```

**2) Alias de tipo** (incl. **tipos de retorno** longos) — spec Ori: preferir `alias` para retornos repetidos

Definir uma vez (ex. `app/application.orl` ou `app/web_types.orl` reexportado):

```ori
module app.application

import ori.web = web

-- Tipos curtos de domínio HTTP (leitura nos controllers)
public alias Request = web.Request
public alias Response = web.Response
public alias WebError = web.WebError

-- Retorno canónico de toda action (T1-A)
public alias ActionResult = result[Response, WebError]
```

Uso no controller:

```ori
module app.controllers.users

import ori.web = web
import app.application = application

public show(req: application.Request) -> application.ActionResult
    match find_user(req.params["id"])
    case none:
        return ok(web.not_found())
    case some(user):
        return application.render("users/show", {
            "title": user.name,
            "user": user,
            "csrf_token": web.csrf_token(req),
        })
    end
end
```

| Preferir | Evitar |
|----------|--------|
| `-> application.ActionResult` | `-> result[web.Response, web.WebError]` em cada action |
| `req: application.Request` (ou `web.Request` se import `web`) | path longo repetido |
| alias legível (`ActionResult`, `Request`) | alias de uma letra (`R`, `E`) |

- Rotas: `users.show`.
- `public alias` de domínio alinha à política M2 do monorepo (tipos de package web).
- Library `ori-web` pode exportar os mesmos aliases; App reexporta ou usa `web.ActionResult` se o package já expuser.

#### Histórico de opções (referência)

Subtópicos originais com alternativas T1-B… etc. mantidos abaixo para arquivo.


---

#### T1 — Assinatura da action

**T1-A — Request → result[Response] (recomendada)**

```ori
public show(req: web.Request) -> result[web.Response, web.WebError]
```

- Explícito, testável, alinhado a `result` Ori.  
- Session/CSRF já no `req` via middleware.

**T1-B — Request + Response builder**

```ori
public show(req: web.Request, res: web.ResponseBuilder) -> result[void, web.WebError]
```

- Parece Java/Express; mais estados mutáveis.

**T1-C — Só Response, Request implícito (context thread-local)**

```ori
public show() -> result[web.Response, web.WebError]
```

- Magia (Rails-ish); **pior fit Ori**.

**T1-D — Context struct da app**

```ori
public show(ctx: app.Ctx) -> result[web.Response, web.WebError]
-- ctx.req, ctx.views, ctx.session helpers
```

- Bom para injetar `TemplateEngine` sem global; um pouco mais de boilerplate no tipo `Ctx`.

| | Clareza | Testes | Magia | Fit Ori |
|--|---------|--------|-------|---------|
| A | alta | fáceis | baixa | **alta** |
| B | média | ok | baixa | média |
| C | baixa | difíceis | alta | baixa |
| D | alta | fáceis | baixa | alta |

**Recomendação:** **T1-A** no Library; App pode oferecer **T1-D** como sugar (`web_app.Ctx` com `req` + `views` + helpers).

---

#### T2 — Nomes das actions

**T2-A — REST Rails clássico (recomendado como *convenção*, não obrigação)**

| Action | Uso típico |
|--------|------------|
| `index` | listar |
| `show` | um registo |
| `new` | form vazio (GET) |
| `create` | POST criar |
| `edit` | form edição (GET) |
| `update` | POST/PATCH |
| `destroy` | DELETE/POST delete |
| nomes livres | `rows`, `dashboard`, … sempre permitidos |

**T2-B — Só nomes livres** — zero tabela REST.

**T2-C — REST obrigatório** — generator só gera os 7; proibir outros (rígido demais).

**Recomendação:** **T2-A** — documentar o set Rails como *default mental* + generators; **sempre** permitir actions extra (`rows` para htmx).

---

#### T3 — Organização em ficheiros

**T3-A — Um módulo por recurso (recomendado)**

```text
app/controllers/users.orl   -- index, show, create, rows
app/controllers/home.orl    -- index
```

**T3-B — Um ficheiro por action**

```text
app/controllers/users/show.orl
```

- Muitos ficheiros; pior para recursos pequenos.

**T3-C — Um único `controllers.orl`**

- Não escala.

**Recomendação:** **T3-A**.

---

#### T4 — Papel de `app/application.orl`

**T4-A — Só funções partilhadas (recomendado)**

```ori
module app.application

public current_user_id(req: web.Request) -> optional[string]
public render(views, name, ctx) -> result[web.Response, web.WebError]
public redirect_to(path: string) -> web.Response
```

Actions **chamam** explicitamente: `application.render(...)`.

**T4-B — “Base controller” com herança**

- Ori não tem herança de classes estilo Rails; forçar seria anti-S3.

**T4-C — Macros / before_action escondido**

```ori
before_action require_login
```

- Produtivo; magia. Se existir, deve ser **lista explícita** na rota ou wrapper:

```ori
web.get(app, "/me", application.require_login(users.me))
```

**Recomendação:** **T4-A** + wrappers explícitos para auth (T4-C light), **sem** herança (T4-B).

---

#### T5 — Como obter o `TemplateEngine` / views

**T5-A — Global/app state no boot**

```ori
-- main guarda views em App
users.show usa web_app.views(app) 
```

**T5-B — Passar `views` em cada action (explícito demais)**

```ori
public show(req, views: templates.Engine) -> ...
```

- Router complica (partial application).

**T5-C — `Ctx` com views (casa com T1-D)**

```ori
public show(ctx: web_app.Ctx) -> result[Response, WebError]
    web_app.render(ctx, "users/show", page_ctx)
end
```

**T5-D — Função em application que fecha sobre views no boot**

```ori
-- application.bind(views)
public render(name, ctx) -> result[Response, WebError]
```

| | Fit Ori | Ergonomia | Testes |
|--|---------|-----------|--------|
| A | média | boa | mock app |
| B | alta | má | fáceis |
| C | alta | boa | fáceis |
| D | boa | boa | ok |

**Recomendação:** **T5-C** (`Ctx`) ou **T5-D** se quisermos manter **T1-A** com `req` only + `application.render` ligado no boot.  
Híbrido forte: **T1-A + T5-D** (assinatura simples; render na application).

---

#### T6 — Render e partial na action

**T6-A — Helpers application (recomendado)**

```ori
public show(req: web.Request) -> result[web.Response, web.WebError]
    const id: string = req.params["id"]
    match load_user(id)
    case none:
        return ok(web.not_found())
    case some(user):
        var ctx: map[string, any] = {
            "title": user.name,
            "user": user,
            "csrf_token": web.csrf_token(req),
        }
        return application.render("users/show", ctx)
    end
end
```

**T6-B — Montar Response à mão**

```ori
html = templates.render(engine, "users/show", ctx)?
return ok(web.html(200, html))
```

- Mais verboso; ok na Library.

**T6-C — Render implícito por nome de action (Rails implicit render)**

- `show` → automaticamente `users/show` — **mágico**; adiar ou nunca.

**Recomendação:** **T6-A** no App; **T6-B** disponível na Library; **não** T6-C no v1.

---

#### T7 — Filtros / “before_action”

**T7-A — Wrapper de função (recomendado)**

```ori
web.get(app, "/users/:id", application.authenticate(users.show))
```

**T7-B — Lista na rota**

```ori
get "/users/:id", users.show, filters: [authenticate]
```

**T7-C — Declaração no controller estilo Rails**

```ori
before_action authenticate, only: [show, edit]
```

- Precisa de infra de metadados; mais magia.

**T7-D — Só middleware global**

- Grosseiro (tudo ou nada).

**Recomendação:** **T7-A** (e opcionalmente T7-B se o DSL de rotas crescer); evitar T7-C no v1.

---

#### T8 — Erros e status

**T8-A — Response explícita (recomendado)**

```ori
return ok(web.not_found())
return ok(web.redirect(303, "/login"))
return err(web.bad_request("missing name"))
```

**T8-B — throw / panic**

- Evitar no happy path.

**T8-C — Exceções de domínio mapeadas por middleware**

- Depois, se doer.

**Recomendação:** **T8-A**; `result` + helpers `web.not_found`, `web.forbidden`, `web.redirect`.

---

#### T9 — Módulo e visibilidade

**T9-A — `public` nas actions usadas nas rotas; helpers `private`/sem public no mesmo ficheiro (recomendado)**

**T9-B — Tudo public**

**Recomendação:** **T9-A**.

---

#### Exemplo canónico (D25) — type aliases nos retornos

```ori
module app.controllers.users

import ori.web = web
import app.application = application

public index(req: application.Request) -> application.ActionResult
    const users: list[User] = list_users()
    return application.render("users/index", {
        "title": "Users",
        "users": users,
        "csrf_token": web.csrf_token(req),
    })
end

public show(req: application.Request) -> application.ActionResult
    const id: string = req.params["id"]
    match find_user(id)
    case none:
        return ok(web.not_found())
    case some(user):
        return application.render("users/show", {
            "title": user.name,
            "user": user,
            "csrf_token": web.csrf_token(req),
        })
    end
end

public rows(req: application.Request) -> application.ActionResult
    return application.render_partial("users/rows", {
        "users": list_users(),
    })
end

public create(req: application.Request) -> application.ActionResult
    match parse_user_form(req)
    case err(e):
        return ok(web.redirect(303, "/users"))
    case ok(form):
        save_user(form)
        web.flash(req, "notice", "Created.")
        return ok(web.redirect(303, "/users"))
    end
end
```

---

#### Tabela de decisão — **preenchida (D25)**

| Tópico | Escolha |
|--------|---------|
| T1 | **A** |
| T2 | **A** |
| T3 | **A** |
| T4 | **A** |
| T5 | **D** |
| T6 | **A** |
| T7 | **A** |
| T8 | **A** |
| T9 | **A** |
| Estilo imports | **aliases** de módulo (`web`, `application`, `users`) |
| Estilo tipos/retornos | **`alias ActionResult`**, `Request`, `Response`, `WebError` (em application ou ori-web) |


### 12.10 APP4 — Views — **FECHADO** (D26)

Espelho entre **controllers** e **templates** (`views/**/*.orix`).

#### Decisões V1–V8

| Tópico | Escolha | Significado |
|--------|---------|-------------|
| V1 nome lógico | **A** | `"users/show"` → `views/users/show.orix` |
| V2 espelho action | **A** | convenção de estilo; render **sempre explícito** |
| V3 full/partial | **A′** | `render` (full + default layout) / `render_partial` (fragmento) |
| V4 layout default | **A** | default `layouts/app` no App; override no `.orix` |
| V5 partials | **A** | `partials/nav`, `users/rows` (sem `_` obrigatório) |
| V6 ctx auto | **B** | merge `csrf_token`, `flash_notice`, `flash_error` |
| V7 assign | **A** | domínio na action; assign no template para title/local |
| V8 root | **A** | `views/` na raiz do projeto |

#### Histórico de opções

Alternativas V1-B… abaixo para arquivo.


---

#### V1 — Nome lógico do template

Como `application.render("…")` mapeia para ficheiro sob `views/`:

| ID | Forma do nome | Ficheiro |
|----|---------------|----------|
| **V1-A** | `"users/show"` | `views/users/show.orix` |
| **V1-B** | `"users.show"` (ponto) | `views/users/show.orix` |
| **V1-C** | path com extensão `"users/show.orix"` | idem |

**Recomendação:** **V1-A** (barra, sem extensão — estilo Rails `users/show`).

---

#### V2 — Espelho action → template

| ID | Regra |
|----|--------|
| **V2-A** | **Convenção documentada, não mágica:** action `users.show` *deve* usar render `"users/show"` por estilo; engine **não** infere sozinha |
| **V2-B** | Implicit render: se a action não chamar render, usa `controller/action` automaticamente | magia Rails |
| **V2-C** | Sem convenção de nome; cada render com string livre |

**Recomendação:** **V2-A** (reading-first + explícito; generators escrevem o par certo).

---

#### V3 — Full page vs partial

| ID | API | Layout |
|----|-----|--------|
| **V3-A** | `render(name, ctx)` aplica layout default se o template **não** disser o contrário; `render_partial(name, ctx)` **nunca** layout | duas funções claras |
| **V3-B** | só `render`; flag `layout: false` no ctx/opts | uma função |
| **V3-C** | layout só se o `.orix` tiver `@{ layout ... }` | partial = ficheiro sem diretiva layout |

**Recomendação:** **V3-A + V3-C juntos:**  
- partial htmx: template **sem** `@{ layout }`, chamado com `render_partial` (ou `render` que não força layout);  
- full page: template **com** `@{ layout "layouts/app" }` **ou** layout default na API full.

Híbrido prático **V3-A′**:  
- `render` = full; se o template não declarar layout, usa `layouts/app` (default App).  
- `render_partial` = raw fragment, ignora layout default e proíbe/ignora `@{ layout }` no ficheiro.

---

#### V4 — Layout default

| ID | Ideia |
|----|--------|
| **V4-A** | Default `layouts/app` só no App; Library sem default |
| **V4-B** | Sempre obrigatório `@{ layout }` no template full |
| **V4-C** | Config `config/app.orl`: `layout = "layouts/app"` |

**Recomendação:** **V4-A** + override por template `@{ layout "…" }` + **V4-C** opcional depois.

---

#### V5 — Onde ficam partials

(Já D22: `views/partials/` + ao lado do recurso.)

| ID | `include` / render_partial |
|----|----------------------------|
| **V5-A** | `"partials/nav"` → `views/partials/nav.orix`; `"users/rows"` → `views/users/rows.orix` |
| **V5-B** | Prefixo `_`: `users/_rows.orix` estilo Rails |

**Recomendação:** **V5-A** (sem underscore obrigatório; opcional `_` permitido se quisermos Rails-feel depois).

---

#### V6 — Ctx injetado automaticamente no render

| ID | O que `application.render` mete no ctx **além** do map da action |
|----|-------------------------------------------------------------------|
| **V6-A** | Nada automático; action passa tudo (incl. `csrf_token`) | máximo explícito |
| **V6-B** | Merge automático: `csrf_token`, `flash_notice`, `flash_error` (e depois `current_user` se houver) | menos boilerplate |
| **V6-C** | Só flash automático; CSRF manual |

**Recomendação:** **V6-B** no App (seguro: valores só do middleware/session); action ainda pode sobrescrever chaves.

*(Liga a APP5; pode fechar já com APP4.)*

---

#### V7 — `assign` no template vs ctx na action

| ID | Papel |
|----|--------|
| **V7-A** | Dados de domínio na **action**; `assign` no template só para title/layout local (D10) |
| **V7-B** | Quase tudo no template com assign | lógica na view — evitar |

**Recomendação:** **V7-A**.

---

#### V8 — Extensão e roots

| ID | Ideia |
|----|--------|
| **V8-A** | Root templates = `views/`; extensão `.orix` (D23); layouts em `views/layouts/` |
| **V8-B** | Root = `app/views/` (Rails `app/views`) |

**Recomendação:** **V8-A** (já D22: `views/` na raiz do projeto, não dentro de `app/`).

---

#### Exemplo de espelho (se V1-A, V2-A, V3-A′, V5-A, V6-B, V8-A)

```text
app/controllers/users.orl
  public show  →  application.render("users/show", { "user": user })
  public rows  →  application.render_partial("users/rows", { "users": users })

views/users/show.orix      -- @{ layout "layouts/app" } ...
views/users/rows.orix      -- sem layout (fragment htmx)
views/layouts/app.orix
views/partials/nav.orix
```

```ori
-- show.orix
@{ layout "layouts/app" }
  @{ assign title = user.name }
  <h1>@{ user.name }</h1>
  @{ include "partials/nav" }
@{ end }
```

---

#### Tabela de decisão — **preenchida (D26)**

| Tópico | Escolha |
|--------|---------|
| V1 | **A** |
| V2 | **A** |
| V3 | **A′** |
| V4 | **A** |
| V5 | **A** |
| V6 | **B** |
| V7 | **A** |
| V8 | **A** |


### 12.11 APP5–APP10 — **FECHADO** por default (D27)

Autor pediu decisão do restante. Defaults opinativos (Ori + Rails-like):

| ID | Decisão |
|----|---------|
| **APP5** | `application.render` faz merge V6-B: `csrf_token`, `flash_notice`, `flash_error`; `current_user` / `current_user_id` quando session tiver; helper `csrf_field` → string HTML do hidden input (App layer, não templates core) |
| **APP6** | Boot: `main.orl` carrega `ori.proj` env → `web_app.run()`: open views, session store, middleware stack §5.4, `routes.draw(app)`, listen. Ordem fixa documentada. |
| **APP7** | `ORI_ENV=development\|production` (default development). Prod: Secure cookies, secret obrigatório, fail boot se secret fraco. Dev: Secure opcional. |
| **APP8** | Generators **v1.1** (não bloqueiam Library): `ori-web-app new`, `generate controller users`. Output = código legível D25. |
| **APP9** | `config/app.orl` exporta defaults: `port`, `views_root = "views"`, `public_root = "public"`, `default_layout = "layouts/app"`. Secrets **só** env. |
| **APP10** | Sem ORM no web. App pode ter `app/domain/` ou `db/` opcional; integração futura com `ori-sqlite` etc. |

### 12.6 Ordem de discussão das convenções

```text
APP1 pastas → APP2 rotas → APP3 handlers → APP4 views
  → APP5 helpers/ctx → APP6 boot → APP7 env → APP8 generators → APP9 config → APP10 DB
```

Cada tópico: proposta → acordo → linha em **Decisões fechadas** (D22+).

### 12.7 O que não entra no opinativo cedo

- ORM completo estilo AR  
- Jobs/Action Cable  
- Asset compile pipeline  
- Engines multi-app  
- Metaprogramação de rotas escondida  

---
## 6. Packages (esboço de fronteiras)

```
ori-lang          → linguagem + stdlib (sem framework web no core)
ori-templates     → Library: HTML engine
ori-web           → Library/Batteries: HTTP + session + CSRF + static
ori-web-app       → App opinativo Rails-like (D21/D32) — library + bin generators
ori-web-demo      → exemplo HTML-first (Library stack)
blog_app          → exemplo gerado por ori-web-app
```

### Escopo v1 `ori-templates` (P1)

- Mini-spec §4 (D3–D12) + path jail D13  
- `open` / `render` / `render_string`  
- Testes: escape, raw pipe, for index, layout content, path escape, strict missing  

### Escopo v1 `ori-web` (P2)

- §10: router, static, text/html/json/redirect, body limit, log middleware  
- Integração opcional com `ori-templates`  
- Session/CSRF: v1.1 (SEC4–6)  

---

## 7. Log de diálogo (append-only resumido)

| Data | Resumo |
|------|--------|
| 2026-07-14 | Acordo A (ERB spirit) + C (HTML-first); greenfield Ori; não port framework |
| 2026-07-14 | Exemplos de sintaxes A–E (+ brainstorm F–M); SEC no roadmap |
| 2026-07-14 | Preferência delimitadores **C `@{ }`**; pergunta sobre `!`; este arquivo criado |
| 2026-07-14 | **D4** comentários `@{-- ... --}`; diálogo Dir-A…G e Raw-1…8 |
| 2026-07-14 | **D5** Dir-B + fecho nomeado opcional; **D6** `\|> raw` (último estágio) |
| 2026-07-14 | **D7** missing strict; **D8** for com índice; explicação S4 L1/L2 |
| 2026-07-14 | **D9** L1 v1 / L2 depois; **D10** assign v1; **D11** `.orix` |
| 2026-07-14 | **D12** helpers v1; **D13** path jail; **D14** ori-web §10 |
| 2026-07-14 | Propostas **SEC4–6** (session/cookie/CSRF) e **C1** (htmx) §§5.2–5.3, §11 |
| 2026-07-14 | **FECHADO** D15–D20: session, cookie, CSRF, flash, htmx C1, camadas extras §5.5 |
| 2026-07-14 | **D21** futuro opinativo Rails-like §12; início diálogo **APP1** pastas |
| 2026-07-14 | **D22** APP1 fechado (controllers, views/users, partials, ori.proj, routes explícitas); **D23** `.orix` |
| 2026-07-14 | **D24** APP2 routes DSL `routes(app)…end` / builder por baixo |
| 2026-07-14 | APP3 diálogo detalhado T1–T9 em §12.9 |
| 2026-07-14 | **D25** APP3 fechado (T1–T9 + aliases de import **e** tipo/retorno) |
| 2026-07-14 | APP4 diálogo V1–V8 em §12.10 |
| 2026-07-14 | **D26** APP4 fechado (V1-A…V8-A, V3-A′, V6-B) |

---

## 8. Próxima rodada sugerida

1. Design App **fechado** (D21–D27).  
2. **Implementar** `packages/ori-templates` (D28 MVP).  
3. Depois: `ori-web` Library.  
4. Depois: `ori-web-app` opinativo.

---

## 9. Referências internas

| Doc | Papel |
|-----|--------|
| [BACKLOG.md](BACKLOG.md) | Open work linguagem (web packages **não** são language-core) |
| [package-ecosystem-guidelines.md](package-ecosystem-guidelines.md) | Convenções de packages |
| Spec / FREEZE | `docs/spec/`; não quebrar superfície S3 no core por causa de templates |
| [web-framework-learning-course.md](web-framework-learning-course.md) | Curso didático (conceitos + mapa para decisões) |
