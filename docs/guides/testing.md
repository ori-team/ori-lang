# Testing Ori (user + contributor)

> **Portuguese (maintainer-oriented manual):** [testing.pt-BR.md](testing.pt-BR.md)  
> **Surface:** S3 / workspace under `compiler/`

## As an Ori user

Mark tests with `@test` and run:

```ori
module app.main

import ori.test = test

@test
adds()
    test.assert(1 + 1 == 2, "add")
end

main()
end
```

```bash
ori test main.orl
ori test main.orl --filter adds
```

Async tests are supported when the function is `async` and uses `await`
(native backend). See `compiler/crates/ori-driver/tests/concurrency_async.rs`
for compiler-side coverage; user projects can use `async @test` functions the
same way.

Optional leak check:

```bash
ORI_TEST_LEAK_CHECK=1 ori test main.orl
```

## As a compiler contributor

From the repository root:

```bash
cd compiler
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test multifile_imports
cargo test -p ori-driver --test diagnostic_catalog
```

Release-style package smoke (needs system linker):

```bash
sh tools/smoke_native_release.sh
sh tools/smoke_no_rust.sh --package-root … --allow-rust-on-path
```

See root [AGENTS.md](../../AGENTS.md) for staging the native runtime and env vars.

Editor DX smokes (local only):

```bash
./tools/smoke_vscode_extension.sh
# Zed: install extensions/zed-ori as a dev extension (see its README)
```
