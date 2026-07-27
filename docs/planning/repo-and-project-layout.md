# Layout do monorepo e de projetos Ori (M2.layout)

> **Status:** decisão aceita (2026-07-13)  
> **Item de plano:** **M2.layout** (dentro de M2, com stdlib merge)  
> **Inspiração:** organização Auk9 (clareza de papéis na raiz + examples como projetos)

---

## 1. Monorepo da linguagem (`ori-lang`)

### Decisão

| ID | Tema | Decisão |
|----|------|---------|
| **R1** | Workspace Cargo | Mora em **`compiler/`** (não na raiz do repo) |
| **R2** | Raiz do repo | Papéis da **linguagem**: `stdlib/`, `runtime/`, `docs/`, `examples/`, `tools/`, `extensions/`, … |
| **R3** | Desenvolvimento | `cd compiler && cargo test --workspace` (ou `cargo --manifest-path compiler/Cargo.toml …` da raiz) |
| **R4** | Examples | **Projetos** com `ori.proj` (+ fontes), não scrapbook de `.orl` soltos |
| **R5** | Higiene | WIP/patches/scratch fora da raiz (ou `_archive/`) |

### Árvore alvo

```text
ori-lang/
  AGENTS.md README.md CHANGELOG.md LICENSE*
  compiler/                 # workspace Rust
    Cargo.toml
    Cargo.lock
    crates/
      ori-driver/ ori-lexer/ …
    .cargo/config.toml      # se necessário
  runtime/                  # artefatos staged por triple
  stdlib/                   # ori.X (política M2 merge)
  docs/
    spec/ planning/ guides/
  examples/
    hello/                  # mini-projeto
      ori.proj
      main.orl
      README.md
  tools/
  extensions/
  tests/                    # fixtures .orl se ainda existirem
```

`rust-toolchain.toml` pode permanecer na **raiz** (rustup sobe diretórios).

---

## 2. Projeto do usuário (app / lib)

### Decisão

| ID | Tema | Decisão |
|----|------|---------|
| **P1** | Obrigatório | **`ori.proj` na raiz** do projeto |
| **P2** | Entry default | **`main.orl` na raiz** (recomendado; alterável em `ori.proj`) |
| **P3** | Pastas de domínio | **Opcionais** — `kanban-app/`, `notes-app/`, … ao gosto do autor |
| **P4** | `src/` / `app/` | **Não obrigatório** — critério do usuário; não imposto por `ori new` |
| **P5** | Docs sidecar | Preferência: `docs/<domínio>/arquivo.oridoc` espelhando o layout das fontes |
| **P6** | `ori new` | Gera `ori.proj` + `main.orl` (app) ou `lib.orl` (lib) + `docs/` vazia opcional |

### Exemplo canônico (documentação / `ori new` app)

```text
meu-projeto/
  ori.proj                 # obrigatório
  main.orl                 # recomendado (entry default)
  kanban-app/              # domínio opcional
    board.orl
    cards.orl
  notes-app/
    stickys.orl
  docs/                    # sidecars opcionais
    kanban-app/
      board.oridoc
      cards.oridoc
```

### `ori.proj` default (app)

```ini
manifest = 1
name = "meu-projeto"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[source]
root_namespace = "app"
-- source.root omitido = raiz do projeto (qualquer subpasta é válida de domínio)

[docs]
paths = ["docs"]
mode = "sidecar-first"
require_public = "off"
```

### Regras

1. O compilador resolve imports relativos ao projeto / `source.root` se definido; **sem** forçar `src/`.
2. `entry` aponta para o arquivo de entrada (default `main.orl`).
3. Domínios = pastas com `.orl`; módulos Ori (`module app.kanban.board`) alinhados ao gosto do autor — não amarrar path físico a `src/`.
4. `ori.pkg.toml` continua para **pacotes publicáveis** / cache; o dia a dia do app é **`ori.proj`**.

---

## 3. Plano de execução (M2.layout)

| Fase | Trabalho | Status |
|------|----------|--------|
| **L.docs** | Este arquivo + spec 17 + PENDENTES + READMEs | ✅ |
| **L.scaffold** | `ori new` / `ori init` → layout acima | ✅ |
| **L.cargo** | Mover workspace para `compiler/` + CI/tools | ✅ |
| **L.examples** | Migrar `examples/*` para pastas-projeto | ✅ |
| **L.hygiene** | Remover/arquivar patches e scratch da raiz | ✅ `_archive/` |

---

## 4. Relação com Auk9

| Auk9 | Ori (esta decisão) |
|------|---------------------|
| `compiler/` com Cargo | **Igual** |
| `examples/hello/{proj, main}` | **Igual em espírito**; entry default `main.orl` |
| stdlib flat | Alinhado a M2 merge `ori.X` |
| Forçar `src/` | **Não** — mais flexível que o scaffold Ori antigo |

---

## Histórico

| Data | Evento |
|------|--------|
| 2026-07-13 | Análise Auk9 vs Ori; decisão: mover monorepo + projeto raiz-first com domínios opcionais |
