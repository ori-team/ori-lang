# Ori Tests

The active regression suite lives in the Rust workspace under
`compiler/crates/ori-driver/tests/`. This `tests/` directory is kept only as
a redirect — no test files should be added here.

## Where the real tests live

| Suite | Path | Coverage |
|---|---|---|
| `ori_spec` | `compiler/crates/ori-driver/tests/ori_spec.rs` | Language spec: lexer, parser, type checker, diagnostics, runtime semantics |
| `multifile_imports` | `compiler/crates/ori-driver/tests/multifile_imports.rs` | Multi-file imports, stdlib execution (native + C backend), bytes/string/collections/JSON/fs |
| `concurrency_async` | `compiler/crates/ori-driver/tests/concurrency_async.rs` | Async/await, task.spawn, channels, cancel tokens, using+async dispose |
| `memory_arc` | `compiler/crates/ori-driver/tests/memory_arc.rs` | ARC retain/release, cycle collector, leak check, struct/enum/tuple destructors |
| `method_resolution` | `compiler/crates/ori-driver/tests/method_resolution.rs` | Trait/impl method resolution, inherent vs trait methods |
| `diagnostic_catalog` | `compiler/crates/ori-driver/tests/diagnostic_catalog.rs` | Diagnostic code catalog consistency (emitted ↔ spec) |
| LSP E2E | `compiler/crates/ori-lsp/tests/e2e.rs` | LSP protocol: hover, definition, completion, rename, formatting, project diagnostics |

## Running the suite

```bash
# Full workspace (from repo root)
cd compiler
cargo test --workspace

# Focused checks
cargo test -p ori-driver
cargo test -p ori-codegen
cargo test -p ori-runtime
cargo test -p ori-lsp
```

Testing guide: [docs/guides/testing.md](../docs/guides/testing.md)
(PT maintainer notes: [testing.pt-BR.md](../docs/guides/testing.pt-BR.md)).

Security and performance metrics:

```bash
cargo run -p ori-driver -- run tools/quality_metrics.orl
```

Language comparison workloads:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 5
```

Comparison guide:

```text
docs/guides/language-comparison.md
```

The driver suite also checks the official examples in `examples/*.orl` so the
public samples do not drift from the implemented language.

## Adding a new test

Add it to the appropriate suite in `compiler/crates/ori-driver/tests/` (or
`compiler/crates/ori-lsp/tests/` for LSP). Bug fixes must include a regression
test; new features must include both a positive and a negative test. See
`CONTRIBUTING.md` for the full PR checklist.
