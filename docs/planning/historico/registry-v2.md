# Ori Package Registry (v2 backlog)

> Status: **superseded for implementation** by [`../registry-v1.md`](../registry-v1.md)
> (PKG-3, 2026-07-13: file/HTTP registry + real `ori publish`). This file remains as
> historical design notes.
> Audience: tooling implementers, release engineers.

## Goal

Distribute Ori libraries and applications without requiring consumers to clone
the full `ori-lang` repository. The registry is separate from compiler runtime
staging (`runtime/<triple>/`).

## Package Manifest (`ori.pkg.toml`)

```toml
[package]
name = "example.demo"
version = "0.1.0"
authors = ["Team <team@example.com>"]
license = "MIT"
description = "Short summary"
entry = "src/main.orl"
ori_version = "0.2.0"

[dependencies]
other.lib = "1.0.0"
local.lib = { path = "../local-lib", version = "0.1.0" }
```

Rules:

- `name` uses dotted namespaces, aligned with Ori `namespace`.
- `version` uses `major.minor.patch`.
- `entry` is the program or library root `.orl` file.
- `ori_version` is the minimum compiler semver the package requires.
- Local dependencies use `{ path = "../other" }`; the dependency manifest name
  must match the dependency key.
- Version-only dependencies require a package already present in the local cache
  until hosted registry fetch is implemented.

## Registry Index Entry

```json
{
  "name": "example.demo",
  "version": "0.1.0",
  "sha256": "<tarball digest>",
  "url": "https://registry.example/ori/example.demo-0.1.0.tar.gz",
  "ori_version": "0.2.0",
  "published_at": "2026-06-29T00:00:00Z"
}
```

## CLI Surface

| Command | Status | Purpose |
|---------|--------|---------|
| `ori install <name> --path <dir>` | implemented | Validate local manifest, validate path deps, copy to local cache |
| `ori install <name>` | future | Resolve version, fetch tarball, extract to local cache |
| `ori publish <path>` | partial | Validate manifest; upload to registry is not implemented |
| `ori add <name>` | future | Add dependency to `ori.pkg.toml` |

Local cache layout:

```text
~/.ori/packages/<name>/<version>/
  ori.pkg.toml
  src/...
```

`ORI_PACKAGE_CACHE` overrides the default cache root.

## Relationship To Stdlib Layer 2

The in-repo `stdlib/` tree remains the prelude shipped with the compiler.
Third-party packages use the same `.orl` module layout but live outside the
compiler install directory. `ORI_STDLIB_ROOT` overrides only the prelude, not
the package cache.

## Security

- Local install never executes package code. It only reads manifests and copies
  files into the cache.
- Symlinks are rejected during local package copy.
- Tarballs, `sha256`, HTTPS, and optional signatures apply to the future hosted
  registry path.

## Implementation Order

1. [x] Manifest parser + `ori.pkg.toml` validation in `ori-driver`.
2. [x] Local path dependencies (`path = "../other"`) for monorepos, no network.
3. [x] Local cache on disk for path-installed packages.
4. [ ] Registry index fetch + tarball cache on disk.
5. [ ] `ori publish` upload API, requiring hosted registry service.
