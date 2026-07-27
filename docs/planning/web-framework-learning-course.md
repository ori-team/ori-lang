# Curso: construir um framework web e um site (HTML-first) com Ori

> **Público:** você (autor) e qualquer um que vá implementar ou revisar  
> `ori-templates` / `ori-web` / demo.  
> **Tom:** ensinar o *porquê*, não só o *quê*.  
> **Decisões de produto:**  
> [`web-templates-discussion-roadmap.md`](web-templates-discussion-roadmap.md).  
> **Status:** vivo — D15–D20 **fechados** (2026-07-14); atualizar na implementação.  
> **Idioma:** pt-BR (material de estudo).

---

## Como usar este curso

1. Leia os **módulos em ordem** na primeira passagem.  
2. Cada módulo tem: *problema real* → *conceito* → *o que a indústria faz* → *o que decidimos na Ori*.  
3. O roadmap de decisões é o “contrato”; este arquivo é o “livro didático”.  
4. Quando não souber um termo: volte ao glossário (§0).

---

## 0. Glossário mínimo

| Termo | Em uma frase |
|-------|----------------|
| **HTTP** | Pedido (request) e resposta (response) entre browser e servidor |
| **GET** | “Me mostra isso” — não deve *mudar* dados no servidor |
| **POST** | “Faz esta ação / envia estes dados” — pode criar/alterar |
| **Cookie** | Pequeno dado que o browser **guarda** e **reenvia** sozinho em pedidos futuros |
| **Session** | “Estado do usuário logado” no servidor (ou equivalente), ligado a um cookie |
| **Sid** | Session id — número/segredo aleatório que identifica a session |
| **XSS** | Injetar JavaScript malicioso na página (ex.: via campo sem escape) |
| **CSRF** | Site atacante faz o browser da vítima enviar um pedido *autenticado* ao teu site |
| **HttpOnly** | Cookie que **JavaScript não consegue ler** |
| **Secure** | Cookie só via HTTPS |
| **SameSite** | Restringe envio do cookie em pedidos *cross-site* |
| **PRG** | Post/Redirect/Get — após POST, redireciona para GET (evita reenviar form no F5) |
| **Partial / fragment** | Pedaço de HTML (não a página inteira), útil com htmx |
| **htmx** | Biblioteca JS pequena: atributos HTML disparam pedidos e trocam partes da página |
| **Middleware** | Camada em volta do handler (log, session, CSRF…) em ordem fixa |
| **Path jail** | Arquivos só dentro de uma pasta root; bloqueia `../` |
| **Escape HTML** | Transformar `<` em `&lt;` etc. para não virar código na página |
| **Raw** | Imprimir **sem** escape (perigoso se a origem for o usuário) |

---

## Módulo 1 — O que é um “site” do ponto de vista do servidor

### Problema

O usuário abre um URL. O servidor precisa devolver **bytes** (HTML, CSS, JS, JSON…).

### Conceito

```
Browser  --HTTP request-->  Servidor (ori-web)
Browser  <--HTTP response--  (HTML da ori-templates + static)
```

Não precisas de React para um site útil: **HTML gerado no servidor** já é a web clássica (e de novo moderna com htmx).

### O que decidimos

| Peça | Package |
|------|---------|
| Gerar HTML | `ori-templates` |
| Rotas, static, session… | `ori-web` |
| Exemplo | `ori-web-demo` |

---

## Módulo 2 — Templates (por que não “só concatenar string”)

### Problema

```ori
html = "<h1>" + user.name + "</h1>"
```

Se `user.name` for `<script>…</script>`, tens **XSS**.

### Conceito

**Template engine** = HTML com buracos controlados + **escape por padrão**.

### O que a indústria faz

- ERB, Jinja, Mustache, Laravel Blade: print escapado default; raw explícito.

### O que decidimos (resumo)

| Ideia | Nossa forma |
|-------|-------------|
| Delimitador | `@{ … }` |
| Print seguro | `@{ user.name }` |
| Raw | `@{ html \|> raw }` só no **fim** do pipe |
| Comentário | `@{-- … --}` |
| if/for | `@{ if }`, `@{ for i, x in xs }`, `@{ end }` ou `@{ /if }` |
| Layout | um `@{ content }` (L1) |
| Missing | **strict** (erro, não vazio silencioso) |
| Path | jail sob `views/` |

Detalhe normativo: roadmap §4.

---

## Módulo 3 — Cookies (SEC4)

### Problema

HTTP é “sem memória”: cada request chega sozinho. Como saber “é o mesmo usuário”?

### Conceito

O servidor manda:

```http
Set-Cookie: ori_sid=abc123; HttpOnly; Secure; SameSite=Lax; Path=/
```

O browser, nos próximos pedidos ao **mesmo site**, envia:

```http
Cookie: ori_sid=abc123
```

### Flags importantes

| Flag | Se faltar… |
|------|------------|
| **HttpOnly** | JS de um XSS pode **roubar** o cookie |
| **Secure** | cookie pode ir em HTTP aberto (rede hostil) |
| **SameSite=Lax/Strict** | pedidos cross-site levam cookie com mais facilidade → piora CSRF |

### O que propomos

Cookie de session `ori_sid` com **HttpOnly + Secure (prod) + SameSite=Lax**.

---

## Módulo 4 — Session (SEC5)

### Problema

Não queres colocar `user_id=5` em texto claro no cookie (forjável).

### Duas famílias

**A) Sid opaco + dados no servidor (recomendada para HTML-first)**

```
Cookie: só um id aleatório impossível de adivinhar
Servidor: mapa id → { user_id, csrf_token, flash, … }
```

**B) Cookie “cheio” assinado (stateless)**

```
Cookie: dados + assinatura HMAC
Servidor: verifica assinatura, não guarda estado
```

| | A Sid+store | B Cookie assinado |
|--|-------------|-------------------|
| Logout imediato | sim (apaga store) | difícil |
| Roubo do cookie | session válida até expirar/revogar | idem até exp |
| Escala multi-servidor | store partilhado (Redis…) | mais fácil |
| v1 demo | store **memória** | possível, pior revogação |

### Session fixation

Atacante fixa um sid conhecido, vítima faz login, atacante usa o mesmo sid.  
**Defesa:** após login, **trocar o sid** (`session_regenerate`).

### O que propomos

- v1: **A (sid + store)**  
- secret de ambiente em prod  
- regenerate no login  

---

## Módulo 5 — CSRF (SEC6) — “é a forma mais segura?”

### Resposta honesta

**Não existe “a única forma mais segura do universo”.**  
Existe: **a mais recomendada para o *teu* tipo de app**.

Para **site HTML-first com cookies de session** (nosso caso):

> **Session cookie HttpOnly + token CSRF synchronizer + SameSite + nunca mutar estado em GET**  
> é o **pacote padrão recomendado** (alinhado a OWASP / práticas Rails, Django, Laravel).

### O ataque CSRF (história curta)

1. Estás logado em `banco.com` (browser tem cookie).  
2. Abres `evil.com`.  
3. `evil.com` tem `<form action="https://banco.com/transferir" method="POST">` (ou JS).  
4. O browser **envia o cookie** do banco no POST.  
5. Sem CSRF check, o banco acha que **foste tu**.

### Defesas (camadas — “defense in depth”)

| Camada | O que faz | Suficiente sozinha? |
|--------|-----------|---------------------|
| **SameSite=Lax/Strict** | Muitos POSTs cross-site **não** levam cookie | **Não** (browsers velhos, exceções, OAuth, subdomínios, bugs) |
| **CSRF token (synchronizer)** | Form/header precisa de segredo da session | Quase padrão ouro c/ session |
| **Double-submit cookie** | Token no cookie + no form iguais | Ok; um pouco diferente |
| **Re-auth / senha p/ ações críticas** | Transferência grande pede senha de novo | Extra, não substitui CSRF em geral |
| **Só JSON + header custom (SPA)** | App legítima manda `X-Requested-With`; form cross-site simples não | Outro modelo (SPA), não o nosso demo HTML |

### Synchronizer token (o que propomos)

1. Session tem `csrf_token` aleatório.  
2. HTML inclui esse valor (hidden input ou header htmx).  
3. Atacante em `evil.com` **não lê** esse valor (same-origin policy).  
4. POST sem token igual → **403**.

### O que *não* é “mais seguro” no nosso contexto

| Ideia | Por quê não é upgrade automático |
|-------|-----------------------------------|
| “Só SameSite, sem token” | Frágil como única linha |
| JWT no localStorage | XSS rouba fácil; **pior** que HttpOnly cookie p/ XSS |
| “Segurança por obscuridade” | Não |
| CSRF em GET | Errado: GET não deve mutar; se mutar, o problema é outro |

### Quando outro modelo é “mais recomendado”

| App | Modelo típico |
|-----|----------------|
| SPA + API separada | Access token curto, refresh, CORS apertado, às vezes sem cookie de session clássica |
| Mobile app | Tokens, não forms HTML |
| Microserviço a microserviço | mTLS, HMAC de webhook — não CSRF de browser |

**Conclusão para Ori HTML-first:** a proposta session+CSRF+SameSite+HttpOnly é **a recomendada e madura**, não um atalho amador. “Mais seguro ainda” = somar (2FA, re-auth em ações sensíveis, WAF, etc.), não trocar o núcleo por moda.

---

## Módulo 6 — Middleware (esteira de correio)

### Conceito

Cada request passa por uma fila:

```text
log → limite de body → session → csrf → a tua função da rota
```

Cada etapa pode: enriquecer o request, cortar com 403/413, ou passar adiante.

### Por que session antes de csrf?

CSRF precisa ler/gravar o token **na session**.

---

## Módulo 7 — htmx e partials (C1)

### Problema

Queres UI reativa (atualizar uma tabela) sem escrever uma SPA.

### Conceito

- **Página full:** HTML com layout (nav, footer).  
- **Partial:** só o pedaço (`<tr>…</tr>`).  
- htmx: `hx-get="/users/rows" hx-target="#rows"` busca o partial e encaixa no DOM.

### CSRF com htmx

POST do htmx também leva cookies → **mesmo CSRF**.  
Padrão: `hx-headers` no `<body>` com o token (uma vez no layout).

### PRG

Após POST de form clássico: **303 Redirect** para um GET.  
Evita “F5 reenvia o form”.

---

## Módulo 8 — Path jail e static files

### Problema

`include "../../../../etc/passwd"` ou `GET /assets/../../secret`.

### Conceito

Todo ficheiro resolve **dentro** de um root; `..` e paths absolutos → erro.

### O que decidimos

D13: jail em templates + static.

---

## Módulo 9 — Mapa “ameaça → defesa” (checklist)

| Ameaça | Defesa no nosso desenho |
|-------|-------------------------|
| XSS | Escape default; raw só `\| > raw`; strict missing |
| CSRF | Token synchronizer + SameSite |
| Roubo de session via JS | HttpOnly |
| Session fixation | regenerate no login |
| Path traversal | jail D13 |
| Body enorme (DoS) | limit_body |
| Secret fraco | fail boot em prod sem `ORI_WEB_SECRET` |
| Re-POST acidental | PRG |

---

## Módulo 10 — Como se constrói o framework (visão de curso “engenharia”)

Ordem pedagógica de **implementação** (quando fores codar):

| Aula | Entrega | Aprendes |
|------|---------|----------|
| 1 | Template parse + print escape | XSS, AST |
| 2 | if/for/assign/layout | linguagens de template |
| 3 | path jail + include | filesystem seguro |
| 4 | HTTP listen + router | HTTP server |
| 5 | static files | MIME, jail |
| 6 | session cookie + store | estado, cookies |
| 7 | CSRF middleware | web security clássica |
| 8 | demo htmx | HTML-first full stack |
| 9 | flash + PRG | UX de forms |
| 10 | (depois) store Redis, HTTPS terminações | produção |

---

## Módulo 11 — Decisões FECHADAS (session / CSRF / htmx)

Ver roadmap **D15–D19**. Resumo:

| Tema | Decisão |
|------|---------|
| Session | sid opaco + store servidor; memória v1; store pluggable |
| Cookie | HttpOnly, Secure (prod), SameSite=Lax |
| CSRF | synchronizer + hidden + hx-headers |
| Flash | notice / error + PRG |
| htmx | partials, HX-Request, static lib |

---

## Módulo 12 — Camadas extras (defense-in-depth) — **NO PLANO** (D20)

O núcleo não é o teto. Hardening em fases (detalhe no roadmap §5.5):

| Fase | Exemplos |
|------|----------|
| **A** (v1 framework) | núcleo + timeouts session + regenerate + testes golden + secret prod |
| **B** (antes de “produção”) | HTTPS, store real, rate limit login, security headers, PRG/flash |
| **C** (hardening) | CSP, re-auth ações sensíveis, 2FA opcional, audit log, argon2id, uploads seguros |
| **D** (ops) | reverse proxy, HSTS, backups, non-root, secrets manager |

Apps de alto risco antecipam C cedo. O curso e o demo ensinam **A+B**; o checklist de deploy exige **B+D**.

---

## Referências de estudo (externas, conceitos)

- OWASP: Cross-Site Request Forgery Prevention  
- OWASP: Session Management Cheat Sheet  
- OWASP: XSS Prevention  
- htmx docs (essays “hypermedia”)  
- MDN: HTTP cookies, SameSite  

*(Links oficiais mudam; busca pelo título.)*

---

## Módulo 14 — Futuro opinativo (Rails-like, D21)

O framework tem **três níveis**:

1. **Library** — peças (`ori-templates`, `ori-web`)  
2. **Batteries** — defaults seguros ligados  
3. **App** (`ori-web-app`) — pastas, boot, generators, convenções  

Rails inspira **produtividade e convenção**; Ori exige **código gerado legível** e pouca magia.  
Detalhe e árvore candidata: roadmap **§12**. Convenções fecham-se tópico a tópico (**APP1…**).

---

## Próximo passo neste material

1. Acompanhar diálogo **APP1+** no roadmap §12.  
2. **Módulo 13 — Tour do código** quando existir package.  
3. Exercícios opcionais CSRF em lab.  
4. Manter §5.5 alinhado ao Módulo 12.
