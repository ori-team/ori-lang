# Manifest schema freeze (PKG-4)

> **Status:** frozen for path / git / version / registry shapes (2026-07-13).  
> Breaking changes require a minor bump and CHANGELOG note.

## `ori.proj` (application / local project)

```toml
manifest = 1
name = "demo.app"           # optional
version = "0.1.0"           # optional; major.minor.patch when present
kind = "app"                # app | lib
entry = "src/main.orl"      # required, must exist

[source]
root = "src"                # optional
root_namespace = "demo.app" # optional

[dependencies]
# Exactly one of: bare version, path table, git table
dep.a = "1.0.0"
dep.b = { path = "../lib", version = "0.1.0" }
dep.c = { git = "https://…", tag = "v1.0.0", version = "1.0.0" }
# git pin: at most one of rev | tag | branch (default branch = main)
# cannot combine git + path

[features]
default = ["tls"]           # optional default set
tls = []                    # cfg v1 declarations use empty arrays
telemetry = []

[docs]
paths = ["docs/api"]
mode = "sidecar-first"      # see project docs mode enum
require_public = "off"
```

### Rules

| Field | Rule |
|-------|------|
| `entry` | Required; `.orl` file under project root |
| dependency name | Dotted segments; each starts with letter or `_` |
| bare version | Semver-like `major.minor.patch` digits only |
| `path` | Relative to project root; target needs `ori.pkg.toml` or `ori.proj` |
| `git` | URL / `github.com/…` / local path; pin optional |
| feature name | ASCII identifier other than reserved `default`; must use `[]` in cfg v1 |
| `features.default` | Every listed name must be declared in the same manifest |

## `ori.pkg.toml` (publishable package)

```toml
[package]
name = "demo.math"          # required; dotted package name
version = "0.1.0"           # required; major.minor.patch
entry = "src/lib.orl"       # required; must exist
ori_version = "0.3.0"       # required; minimum compiler surface
description = "…"           # optional
authors = ["…"]             # optional; ignored by parser
native_libs = ["foo"]       # optional string array

[dependencies]
# same shapes as ori.proj (version | path | git)

[features]
default = []
tls = []
```

### Rules

| Field | Rule |
|-------|------|
| `name` | Matches import package root; validated segments |
| `version` | Semver-like; cache key is `name/version` |
| `entry` | Must be `.orl` and exist on disk |
| path dep name | Must match dependency package's `name` |
| git dep | Fetched into cache; optional `version` checks cloned manifest |
| feature | Same identifier, empty-array, and default-subset rules as `ori.proj` |

## Resolution order (version pin)

1. Local cache (`ORI_PACKAGE_CACHE` / `~/.ori/packages/<name>/<version>`)
2. Registry (`ORI_REGISTRY`) — see `registry-v1.md`
3. Error (`package.cache_miss` / `package.registry_*`)

## `ori.lock` v2

`ori lock` writes an atomic, generated snapshot. Every flat dependency entry
records source kind, exact version, normalized source identity, SHA-256 tree
digest, and exact Git commit when applicable. Once the file exists, `ori get`
and compile-time validation restore from the lock instead of re-resolving a
branch, tag, or mutable registry index. A one-byte cache/source change is an
error; `ori lock --locked --offline` additionally refuses network access.

Path dependencies keep a normalized project-relative identity plus the content
digest. This avoids leaking local absolute paths and keeps the lock portable
when the dependency remains at the same relative location; use registry or Git
sources when the source tree itself must be fetched.

## Edge cases (tested)

- Missing `entry` / missing file → error
- Invalid package name / version → error
- `git` + `path` in same table → error
- Multiple of `rev`/`tag`/`branch` → error
- Path dep name mismatch → error
- Version dep without cache/registry → `package.cache_miss` / registry miss

## Non-goals

- Signed package/publisher identity
- Registry auth schema beyond `ORI_REGISTRY_TOKEN`
- Windows-specific path rules beyond OS path handling
