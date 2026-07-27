# Ori package registry (v1 — PKG-3)

> Status: **implemented** (2026-07-13).  
> Living contract for `ori publish` / version fetch. Historical sketch: `historico/registry-v2.md`.

## Goal

Distribute Ori libraries without git clones or path monorepos. Consumers pin
versions in manifests; producers publish once to a registry root.

## Configuration

| Variable / flag | Meaning |
|-----------------|---------|
| `ORI_REGISTRY` | Registry root: **directory path**, `file:///…`, or `https://…` base URL |
| `--registry` | Override on `ori publish` |
| `ORI_REGISTRY_TOKEN` / `--token` | Bearer token for HTTP PUT publish |
| `ORI_PACKAGE_CACHE` | Local install cache (default `~/.ori/packages`) |

## File registry layout

```text
{ORI_REGISTRY}/
  index.json                          # {"packages":{"demo.math":["0.4.0"]}}
  packages/
    demo.math/
      versions.json                   # {"versions":["0.4.0"]}
      0.4.0/
        ori.pkg.toml
        src/...
      0.4.0.tar.gz                    # same contents (for HTTP mirrors)
```

## HTTP registry layout

```text
{base}/packages/{name}/{version}.tar.gz
{base}/packages/{name}/versions.json   # optional; required for `ori install name` without @version
```

- **Fetch:** `GET` the tarball; extract into the package cache.
- **Publish:** `PUT` the tarball (optional Bearer token). Index updates are owned
  by the server or by using a **file registry** (recommended for self-host).

## CLI

```bash
# Publish (file registry)
export ORI_REGISTRY=/var/ori-registry
ori publish ./my-lib
ori publish ./my-lib --force          # replace same version

# Install from registry into local cache
ori install demo.math@0.4.0
ori install demo.math                 # latest from versions.json

# Consumer project
# ori.pkg.toml:
#   [dependencies]
#   demo.math = "0.4.0"
ori check .                           # fetches on cache miss when ORI_REGISTRY is set
```

## Manifest dependencies (unchanged surface)

```toml
[dependencies]
demo.math = "0.4.0"                              # registry or cache
local.lib = { path = "../local", version = "0.1.0" }
remote.lib = { git = "https://…", tag = "v1.0.0" }
```

Resolution order for a bare version pin:

1. Local package cache (`ORI_PACKAGE_CACHE` / `~/.ori/packages`)
2. Configured registry (`ORI_REGISTRY`)
3. Error (`package.cache_miss` / `package.registry_miss` / `package.registry_unconfigured`)

## Security notes

- File publish only copies trees (symlinks rejected, same as local install).
- HTTP publish does not run package code; it uploads a tarball.
- No signature/verification layer yet — treat registry hosts as trusted (v1).

## Out of scope (later)

- Central public ori-lang.org index hosting
- Signing / TUF
- Yank / dependency lockfile
- `ori add` helper
