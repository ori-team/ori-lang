# Ori Examples

Each example is a **mini-project**:

```text
example-name/
  ori.proj      # required
  main.orl      # default entry (tests_demo: @test only, no main)
  *.orl         # optional sibling modules (see multi_module)
```

**Surface:** S3 + inference B · **`ok`/`err`** · `match` expressions · struct destructuring · imports `ori.X` (not `ori.X.utils`).

## Run

```bash
cd compiler
cargo run -p ori-driver -- check ../examples/hello
cargo run -p ori-driver -- run ../examples/hello
```

With a staged/packaged `ori`:

```bash
ori check examples/hello
ori run examples/hello
ori test examples/tests_demo
```

All examples under `examples/*/main.orl` pass `ori check`.  
`http_get` needs network and TLS (runtime uses rustls + ring).  
`using_fs` / `async_io` write a scratch file in the current working directory.

**Performance:** honest polyglot numbers and mid-end notes live in
[`docs/guides/performance.md`](../docs/guides/performance.md)
([PT](../docs/guides/performance.pt-BR.md)). Prefer `lists.with_capacity(n)`
when the final list length is known.

## Learning trail

Suggested order for newcomers:

| Step | Example | What you learn |
|------|---------|----------------|
| 1 | [`hello/`](hello/) | Minimal app, `ori run` |
| 2 | [`language_features/`](language_features/) | Structs, enums, traits, generics, newtypes, destructuring, match-expressions, pipe, `check` |
| 3 | [`error_handling/`](error_handling/) | `result`, `try`, `ok`/`err`, and match |
| 4 | [`tests_demo/`](tests_demo/) | `@test` + `ori.test` (`ori test`) |
| 5 | [`cli_args/`](cli_args/) | `ori.args` |
| 6 | [`string_toolkit/`](string_toolkit/) · [`bytes_usage/`](bytes_usage/) | Text and bytes |
| 7 | [`collections_demo/`](collections_demo/) | List (`with_capacity` / `reserve`), map, queue, stack, deque, linked lists, tree, hash table |
| 8 | [`using_fs/`](using_fs/) | Streams `open_input` / `open_output` + `using` dispose |
| 9 | [`path_time_io/`](path_time_io/) · [`file_organizer/`](file_organizer/) · [`log_analyzer/`](log_analyzer/) | Path, time, file workflows |
| 10 | [`json_validator/`](json_validator/) | JSON |
| 11 | [`process_runner/`](process_runner/) | Process |
| 12 | [`async_demo/`](async_demo/) | `async` / `await` + `task.sleep` |
| 13 | [`async_io/`](async_io/) | `fs.read_text_async` / `write_text_async` |
| 14 | [`concurrency/`](concurrency/) | `task.spawn`/`join`, channel, atomic |
| 15 | [`random_format_iter/`](random_format_iter/) | `ori.random`, `ori.format`, `ori.iter` |
| 16 | [`http_get/`](http_get/) | `ori.net.http` + TLS |
| 17 | [`multi_module/`](multi_module/) | Local modules (`import app.greeter`) |
| 18 | [`native_showcase/`](native_showcase/) | Traits, `any[T]`, generics, `using`, `Displayable` |
| 19 | [`linux_log_report/`](linux_log_report/) | Medium Linux log report with modules, filesystem I/O, and tests |

## Catalog (all 23 mini-projects)

| Example | Topic |
|---------|--------|
| [`hello/`](hello/) | Minimal app (canonical start) |
| [`language_features/`](language_features/) | Broad language surface tour |
| [`error_handling/`](error_handling/) | `result` / match |
| [`tests_demo/`](tests_demo/) | `@test` harness (`ori test`) |
| [`cli_args/`](cli_args/) | CLI args + small task list |
| [`string_toolkit/`](string_toolkit/) | `ori.string` |
| [`bytes_usage/`](bytes_usage/) | `ori.bytes` |
| [`collections_demo/`](collections_demo/) | Collections overview + list capacity API |
| [`using_fs/`](using_fs/) | File streams + `using` |
| [`path_time_io/`](path_time_io/) | Path / time / IO |
| [`file_organizer/`](file_organizer/) | FS organization sketch |
| [`log_analyzer/`](log_analyzer/) | File + string processing |
| [`json_validator/`](json_validator/) | JSON |
| [`process_runner/`](process_runner/) | Process |
| [`async_demo/`](async_demo/) | Minimal async/await |
| [`async_io/`](async_io/) | Async FS |
| [`concurrency/`](concurrency/) | Spawn/join, channel, atomic |
| [`random_format_iter/`](random_format_iter/) | Random / format / iter |
| [`http_get/`](http_get/) | HTTP GET + TLS |
| [`multi_module/`](multi_module/) | Multi-file project (`greeter.orl`) |
| [`conditional_config/`](conditional_config/) | Structured `@cfg`, target facts, and manifest features |
| [`native_showcase/`](native_showcase/) | Traits, generics, nominal IDs, enums, dispose |
| [`linux_log_report/`](linux_log_report/) | Medium Linux log report (multi-module + filesystem I/O) |

### Standalone probe

[`test_generics.orl`](test_generics.orl) is a focused compiler probe for const-generic
type declarations. It intentionally does not construct a `Matrix[N]` value until
that constructor path is supported by the backend.

## Policy

- Prefer **`import ori.X = alias`** and selective imports on the parent.
- **Two or more imports** → use the S3 **`imports … end`** block (not a stack of bare
  `import` lines). Single import may stay as `import ori.io = io`.
- Keep examples **runnable** and S3-clean; pre-S3 forms are hard errors (`ori migrate-syntax`).
- One concern per example when possible; deep collection APIs live in `collections_demo`, not separate demos.
- `tests_demo` has **no `main`** — required so the test harness can inject the entry point.
