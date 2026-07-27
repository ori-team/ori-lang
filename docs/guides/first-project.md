# First project and local packages

> Status: practical guide for Ori **S3 / 0.3.2**  
> **Portuguese:** [first-project.pt-BR.md](first-project.pt-BR.md)  
> Layout: root-first (`ori.proj` + `main.orl`) — see [spec/17](../spec/17-project-and-docs.md)

## Create a project

```bash
ori new demo
cd demo
ori check main.orl
ori run main.orl
```

`ori new` creates:

```text
demo/
  ori.proj    # entry = "main.orl"
  main.orl
```

There is **no** required `src/` folder. Optional `docs/` for `.oridoc` sidecars
(see `ori.proj` `[docs]` section). Put more modules in domain folders if you
want (`board/`, `api/`, …).

## Main CLI commands

```bash
ori check main.orl
ori run main.orl
ori compile main.orl --out demo
ori fmt main.orl
ori doctor
ori summary .
```

Tests — mark functions with `@test`:

```ori
module demo.main

import ori.test = test

@test
math_is_stable()
    test.assert(1 + 1 == 2, "math should work")
end
```

```bash
ori test main.orl
```

## Local library dependency

```text
workspace/
  app/
    ori.proj
    ori.pkg.toml
    main.orl
  math/
    ori.pkg.toml
    lib.orl
```

`math/ori.pkg.toml`:

```toml
[package]
name = "demo.math"
version = "0.1.0"
entry = "lib.orl"
ori_version = "0.3.2"
```

`math/lib.orl`:

```ori
module demo.math

public double(value: int) -> int
    return value * 2
end
```

`app/ori.proj`:

```ini
manifest = 1
name = "demo.app"
version = "0.1.0"
kind = "app"
entry = "main.orl"

[source]
root_namespace = "demo.app"

[dependencies]
demo.math = { path = "../math", version = "0.1.0" }
```

`app/main.orl`:

```ori
module demo.app

import demo.math (double)
import ori.io = io

main()
    io.println(string(double(21)))
end
```

```bash
cd workspace/app
ori check main.orl
ori run main.orl
```

## Install into the local package cache

```bash
ori install demo.app --path .
# cache: ~/.ori/packages/<name>/<version>/
ORI_PACKAGE_CACHE=./cache ori install demo.app --path .
```

The local installer validates manifests, copies files, does not execute package
code, and rejects symlinks during copy.

Registry (optional, local or HTTP via `ORI_REGISTRY`):

```bash
ori publish . --registry /path/to/registry
ori install other.pkg@0.1.0
```

See [registry-v1.md](../planning/registry-v1.md) for layout (planning; not a
marketplace product push).

## After upgrading Ori

1. Read [CHANGELOG.md](../../CHANGELOG.md).
2. Run `ori check` / `ori test` on your project.
3. If you still have pre-S3 sources: `ori migrate-syntax .`

---

Next: [Cookbook](cookbook.md) · [Language tour](../language/tour.md) ·
[Install](../install.md) · [Examples](../../examples/)
