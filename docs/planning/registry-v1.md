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
| `ORI_OFFLINE=1` / `ori lock --offline` | Refuse network access and restore only verified lock/cache entries |
| `ORI_ALLOW_INSECURE_REGISTRY=1` | Explicit local-development opt-in for plain HTTP; HTTPS is otherwise required |

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
      0.4.0.tar.gz.sha256             # archive SHA-256, written before availability
```

## HTTP registry layout

```text
{base}/packages/{name}/{version}.tar.gz
{base}/packages/{name}/{version}.tar.gz.sha256
{base}/packages/{name}/versions.json   # optional; required for `ori install name` without @version
```

- **Fetch:** `GET` the digest then tarball; verify before bounded extraction.
- **Publish:** conditional `PUT` (`If-None-Match: *`) of digest then tarball
  (optional Bearer token). Index updates are owned
  by the server or by using a **file registry** (recommended for self-host).

## CLI

```bash
# Publish (file registry)
export ORI_REGISTRY=/var/ori-registry
ori publish ./my-lib
# --force is retained for CLI compatibility but deliberately fails: bump version

# Install from registry into local cache
ori install demo.math@0.4.0
ori install demo.math                 # latest from versions.json

# Consumer project
# ori.pkg.toml:
#   [dependencies]
#   demo.math = "0.4.0"
ori check .                           # fetches on cache miss when ORI_REGISTRY is set
ori lock --locked --offline .         # exact, digest-verified restore without network
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
- Published versions are immutable. File publication stages privately and
  rolls back partial artifact moves; HTTP clients require the digest/archive
  pair, so concurrent publication is either verified or a safe miss.
- Plain HTTP is refused unless `ORI_ALLOW_INSECURE_REGISTRY=1`; redirects are
  disabled. Bearer tokens stay in the HTTP client, not process arguments.
- Downloads are capped at 64 MiB compressed, 256 MiB expanded, 10,000 entries,
  4,096-byte paths, and depth 64. Traversal, backslashes, links, devices,
  duplicate/case-colliding names, and truncated archives fail before extraction.
- SHA-256 proves content integrity, not publisher identity. Treat registry hosts
  as trusted until signing/transparency is implemented.

## Out of scope (later)

- Central public ori-lang.org index hosting
- Signing / TUF
- Yank / retention policy
- `ori add` helper
