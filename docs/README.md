# Ori documentation

> **Surface:** S3 (`0.3.0`) · inference option B (`0.3.1`) · package/M1 (`0.3.2`)  
> **Languages:** English is primary on GitHub · Portuguese is maintained in parallel  
> **Status:** living docs — must match the compiler, not aspirational designs

This tree is the product documentation for Ori. Use it by audience:

| Audience | Start here |
|----------|------------|
| **New user** | [Install](install.md) → [Language tour](language/tour.md) → [First project](guides/first-project.md) |
| **Book (draft, PT)** | **[Ori book](book/README.md)** · [PDF](book/dist/ori-livro.pdf) (`0.3.x-book.2`) |
| **Everyday coding** | [Cookbook](guides/cookbook.md) · [Errors / optional / result](guides/errors-null-void.md) · [Examples](../examples/) |
| **Performance** | [Performance microbench](guides/performance.md) (Ori vs Python vs Rust) · [PT](guides/performance.pt-BR.md) |
| **Language contract** | [Specification](spec/README.md) (normative, English) |
| **Maintainers / planning** | **[BACKLOG](planning/BACKLOG.md)** (only open-work list) · [Planning](planning/README.md) · [AGENTS.md](../AGENTS.md) |

Portuguese index: [README.pt-BR.md](README.pt-BR.md).

---

## Language policy (standard)

| Document class | Language | Notes |
|----------------|----------|--------|
| **GitHub primary surface** (root `README.md`, `docs/**` user guides, install) | **English** | Canonical for links, CI badges, releases |
| **Portuguese parallel** | **`*.pt-BR.md` sibling** or `README.pt-BR.md` | Same structure and version as EN; no orphan PT-only user guides |
| **Normative spec** (`docs/spec/`) | **English** | Single source of truth for implementers |
| **Planning / backlog** (`docs/planning/`) | Portuguese or English (file’s existing language) | Not user-facing product docs |
| **Book** (`docs/book/`) | Portuguese prose; S3 code samples | Draft narrative/reference; does **not** replace `spec/` |
| **Historical** (`docs/planning/historico/`, `_reversa_sdd/`) | as written | Do not teach as current surface |
| **Code + code comments** | English | Project matrix in `AGENTS.md` |

**Rules:**

1. User-facing examples must be **valid S3** (`module`, no declaration `func`, `import path = alias`, types with `[]`, `ok`/`err`, `try`, `end` blocks).
2. Prefer `import ori.X = short` and canonical stdlib parents `ori.X` (not `.utils` in new docs).
3. Project layout is **root-first**: `ori.proj` + `main.orl` (no forced `src/`).
4. When changing syntax, runtime, or CLI, update **EN + PT** user docs in the same change as the code when the change is user-visible.
5. Spec stays English-only; do not fork the normative chapters into PT.

---

## Directory map

```text
docs/
├── README.md              # this file (EN index)
├── README.pt-BR.md        # PT index
├── install.md             # end-user install (EN)
├── install.pt-BR.md       # end-user install (PT)
├── language/              # learn the language (user-facing)
│   ├── tour.md
│   └── tour.pt-BR.md
├── guides/                # how-to guides (EN + .pt-BR)
│   ├── first-project.md
│   ├── cookbook.md
│   ├── errors-null-void.md
│   ├── testing.md
│   ├── report-bugs.md
│   ├── bootstrapping.md
│   ├── performance.md     # Ori / Python / Rust microbench (EN + .pt-BR)
│   └── language-comparison.md  # older multi-lang suite (historical notes)
├── spec/                  # normative language + ABI (EN)
├── book/                  # Ori book draft (PT narrative + reference)
└── planning/              # maintainers only (not a tutorial)
    └── historico/         # completed / archived plans
```

Polyglot harness (sources + runner): [`tools/bench/polyglot/`](../tools/bench/polyglot/).

---

## Current language surface (quick)

| Topic | Canonical form |
|-------|----------------|
| File header | `module app.main` |
| Function | `name(params) -> T` / `main()` — **no** `func` keyword |
| Types | `list[T]`, `map[K, V]`, `optional[T]`, `result[T, E]` |
| Result | `ok(v)` / `err(e)` · match `case ok(x):` / `case err(m):` |
| Propagate | `try expr` only |
| Imports | `import ori.io = io` · `import ori.fs (read_text)` · `import ori.io` |
| Traits | `import ori.core = core` · `apply Type` + `use core.Displayable` |
| Pipe | `x \|> f` (kept; typed as `f(x)`) |
| Local inference | option B: field / index / call / pipe when type is known |
| Async | `async main()` + `await` (native); C/debug rejects async |
| Project | `ori.proj` + recommended `main.orl` at project root |

Full contract: [spec/01-overview.md](spec/01-overview.md). Migration from pre-S3: `ori migrate-syntax`.

---

## Version pins

| Artifact | Version |
|----------|---------|
| Language surface (S3) | `0.3.0` |
| Local inference + option B | `0.3.1` |
| Package / M1 / M3 docs / stdlib residual | `0.3.2` |
| Cargo workspace | matches package (`0.3.2`) |

Changelog: [CHANGELOG.md](../CHANGELOG.md).
