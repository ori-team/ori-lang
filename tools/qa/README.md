# Ori language QA stages

Scripts for daily / weekly language quality (FREEZE-1 / 0.3.x).

| Script | Role |
|--------|------|
| [`daily_fast.sh`](daily_fast.sh) | S0 compile + strict Clippy for runtime/codegen/driver, S1–S4, and residual surface gate (S8 subset) |
| [`daily_full.sh`](daily_full.sh) | Fast + multifile + full async + workspace + examples + perf |
| [`catalog_lint.sh`](catalog_lint.sh) | Spec 13 ↔ emitted diagnostic codes |
| [`residual_audit.sh`](residual_audit.sh) | Product surface + intentional residual negatives |
| [`examples_smoke.sh`](examples_smoke.sh) | `ori check` over `examples/*` |
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
```

For polyglot smoke, stage a current `ori` binary on `PATH` or set `ORI_BIN`:

```bash
export PATH="$PWD/compiler/target/release:$PATH"
./tools/qa/perf_polyglot_smoke.sh
```

## Related

| Doc / skill | Role |
|-------------|------|
| [`docs/planning/qa/test-matrix-ori.md`](../../docs/planning/qa/test-matrix-ori.md) | Product-mapped test matrix |
| [`.grok/skills/ori-lang-qa/`](../../.grok/skills/ori-lang-qa/) | Agent skill + residual/diagnostics policy |
| [`docs/planning/BACKLOG.md`](../../docs/planning/BACKLOG.md) | Open work (language-first queue) |
