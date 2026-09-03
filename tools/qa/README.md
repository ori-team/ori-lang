# Ori language QA stages

Scripts for daily / weekly language quality. The current workspace baseline is
`0.3.8-dev`; ABI-1 remains in force after the 2026-07-19 freeze closure.

| Script | Role |
|--------|------|
| [`daily_fast.sh`](daily_fast.sh) | Required docs + scoped rustfmt + workspace check + strict full-workspace Clippy + S1–S4 + residual gate |
| [`daily_full.sh`](daily_full.sh) | All fast gates + multifile/async/workspace/examples; external web-package and perf stages are explicitly observational |
| [`rustfmt_scoped.py`](rustfmt_scoped.py) | Enforces the ratcheted Rust formatting baseline in [`rustfmt_scope.txt`](rustfmt_scope.txt) |
| [`catalog_lint.sh`](catalog_lint.sh) | Spec 13 ↔ emitted diagnostic codes |
| [`abi_exports.sh`](abi_exports.sh) | Runtime static/shared exports ↔ stdlib manifest and native declarations |
| [`validate_runtime_link.py`](validate_runtime_link.py) | Runtime-link schema, target/profile, artifact names, and optional SHA-256 validation |
| [`runtime-link.schema.json`](runtime-link.schema.json) | Declarative shape contract consumed by the runtime-link validator |
| [`residual_audit.sh`](residual_audit.sh) | Product surface + intentional residual negatives |
| [`examples_smoke.sh`](examples_smoke.sh) | `ori check` over `examples/*`; `ORI_EXAMPLES_COMPILE=1` adds an isolated native build tier and `ORI_EXAMPLES_RUN=name,...` runs a curated subset |
| [`docs_coverage.sh`](docs_coverage.sh) | Atlas paths, canonical documents, and release-blocking P0 closure |
| [`validate_atlas.py`](validate_atlas.py) | Dependency-free Atlas schema, status, and path validation |
| [`fuzz_smoke.py`](fuzz_smoke.py) | Deterministic malformed-byte, truncation, and nesting front-end smoke |
| [`archive_repro_smoke.sh`](archive_repro_smoke.sh) | Byte-identical same-epoch archives across roots; `.ori` caches excluded |
| [`docs_examples.sh`](docs_examples.sh) | Atlas + canonical example smoke + `ori doc check` |
| [`web_sec8.sh`](web_sec8.sh) | `ori-web` SEC8 golden suite (CSRF, jail, sessions, middleware, upload) |
| [`web_auth_smoke.sh`](web_auth_smoke.sh) | `ori-web-auth` TOTP + recovery codes smoke |
| [`web_session_sqlite_smoke.sh`](web_session_sqlite_smoke.sh) | SQLite session adapter (needs `ori-sqlite` build; AOT) |
| [`perf_daily.sh`](perf_daily.sh) | `performance_guard` + optional microbench |
| [`perf_polyglot_smoke.sh`](perf_polyglot_smoke.sh) | Compile+run fib + list polyglot kernels |

## Usage

From repo root (with Rust toolchain for compiler work):

```bash
./tools/qa/catalog_lint.sh
./tools/qa/daily_fast.sh
# optional weekly:
./tools/qa/daily_full.sh
# front-end hostile-input smoke (requires a built `ori`, or set ORI_BIN):
./tools/qa/fuzz_smoke.py
```

`daily_fast.sh` has no suppressed test failures: every command is required and
non-zero aborts the gate. `daily_full.sh` also fails on all correctness stages.
Its environment-dependent web-package and performance probes are observational;
if either fails, the final line says `INCOMPLETE` and never reports an
unqualified `OK`. Promote an observational probe to required only after its
external dependencies and stable budget exist on every supported runner.

The repository still has historical formatting drift outside the scoped list.
The list is a monotonic ratchet: format a file, add it, and do not remove it to
hide a regression. The release/native workflow runs the same scoped gate and a
strict `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

For polyglot smoke, stage a current `ori` binary on `PATH` or set `ORI_BIN`:

```bash
export PATH="$PWD/compiler/target/release:$PATH"
./tools/qa/perf_polyglot_smoke.sh
```

## Related

| Doc / skill | Role |
|-------------|------|
| [`docs/planning/qa/test-matrix-ori.md`](../../docs/planning/qa/test-matrix-ori.md) | Product-mapped test matrix |
| [`docs/planning/qa/`](../../docs/planning/qa/) | Residual and diagnostics policy |
| [`docs/planning/BACKLOG.md`](../../docs/planning/BACKLOG.md) | Open work (language-first queue) |
