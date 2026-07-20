# Ori Language Specification — Chapter 17: Project Layout and External Docs

> Status: normative (layout **M2.layout**)
> Audience: tooling implementers, contributors

---

This chapter defines:

- `ori.proj` — the **project** manifest (required at the root);
- `ori.pkg.toml` — the manifest of a reusable / cached **package** (optional);
- `.oridoc` — external documentation for Ori symbols.

Product layout: `docs/planning/repo-and-project-layout.md`.

The goal is to keep code readable without forcing long comments inside the
`.orl` file, and **without** imposing a magic folder (`src/`, `app/`) on the
project.

---

## Canonical project layout

**Required:** `ori.proj` at the root.

**Recommended:** `main.orl` at the root (`entry = "main.orl"`). `entry` may
point somewhere else.

**Optional:** domain folders with more `.orl` files, and a mirrored docs tree.

```text
my-project/
  ori.proj
  main.orl
  kanban-app/                 -- optional domain (name is yours to pick)
    board.orl
    cards.orl
  notes-app/
    stickys.orl
  docs/                       -- optional sidecars
    kanban-app/
      board.oridoc
      cards.oridoc
```

`ori new <path>` creates:

```text
<path>/
  ori.proj
  main.orl          -- app (with --lib: lib.orl)
  docs/             -- empty folder for sidecars
```

It does **not** create `src/`, `lib/`, or `bin/` by default.

---

## `ori.proj`

`ori.proj` lives at the project **root**. The format is simple and explicit:

```ini
manifest = 1
name = "demo"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[source]
root_namespace = "app"
-- source.root is optional; omitted = project root (every subfolder is domain)

[dependencies]
demo.math = { path = "../math", version = "0.1.0" }

[docs]
paths = ["docs"]
mode = "sidecar-first"
require_public = "off"
```

Current fields:

| Field | Required | Description |
|---|:---:|---|
| `manifest` | no | Format version. Currently accepts `1`. |
| `name` | no | Human-readable project name. |
| `version` | no | Project version. |
| `kind` | no | `app` or `lib`. Default: `app`. |
| `entry` | **yes** | Entry `.orl` file (recommended: `main.orl` at the root). |
| `source.root` | no | Code root folder; **omitted = the `ori.proj` directory**. |
| `source.root_namespace` | no | Expected module prefix (e.g. `app`). |
| `dependencies.<name>` | no | Local dependency `{ path = "..." }`; version optional. |
| `docs.paths` | no | Folders/files holding `.oridoc`. |
| `docs.mode` | no | `sidecar-first` or `inline-first`. Default: `sidecar-first`. |
| `docs.require_public` | no | `off`, `warn`, or `error`. Default: `off`. |

Compatibility: `entry = "src/main.orl"` with `source.root = "src"` remains
valid for anyone who prefers that layout.

Local dependencies under `[dependencies]` take part in import resolution:

```ori
import demo.math (double)
```

For `demo.math = { path = "../math" }`, the path must point at a project with
an `ori.proj` or a package with an `ori.pkg.toml`.

---

## `ori.pkg.toml`

`ori.pkg.toml` describes a package installable into the local cache. It does
**not** replace `ori.proj` in day-to-day app work: `ori.proj` organizes the
project, `ori.pkg.toml` defines the distribution contract.

```toml
[package]
name = "demo.app"
version = "0.1.0"
entry = "main.orl"
ori_version = "0.3.1"
description = "Demo app"

[dependencies]
demo.math = { path = "../demo-math", version = "0.1.0" }
```

| Field | Description |
|---|---|
| `package.name` | Dotted name aligned with the Ori module. |
| `package.version` | `major.minor.patch` version. |
| `package.entry` | Entry `.orl` file of the package. |
| `package.ori_version` | Minimum expected Ori compiler version. |

`ori check`, `ori run`, `ori test`, and `ori doc` accept `ori.pkg.toml` as
input when the directory is used as a package.

---

## `.oridoc`

An `.oridoc` file documents the symbols of a module. Preferred layout:

```text
kanban-app/board.orl
docs/kanban-app/board.oridoc
```

Side by side is also valid:

```text
board.orl
board.oridoc
```

As is any folder listed in `[docs].paths`.

### Format (summary)

```text
oridoc 1

module app.kanban.board

doc load_board
  summary:
    Loads the board.
  returns:
    `result[Board, string]`
end

doc module self
  summary:
    Board domain of the kanban app.
end
```

Inline vs sidecar priority is set by `[docs].mode` (`sidecar-first` by
default).

---

## Commands

```bash
ori new my-project
ori new my-lib --lib        # creates lib.orl instead of main.orl
ori check .                 # walks up until it finds ori.proj
ori check ori.proj
ori run .
ori doc file main.orl       # extract docs from one file
ori doc check               # validate inline docs and .oridoc sidecars
ori doc export              # stdlib + error catalog JSON for the website
```

---

## The language monorepo

The `ori-lang` repository is not a user app. The Cargo workspace lives in
`compiler/`. See `docs/planning/repo-and-project-layout.md`.
