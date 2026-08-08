# Debugging Ori programs

> **Audience:** users diagnosing a native program or wiring an IDE
> **Portuguese:** [debugging.pt-BR.md](debugging.pt-BR.md)

Ori provides a cooperative native debugger and a small Debug Adapter Protocol
(DAP) server. Both consume the same compiler-generated snapshot data.

## Terminal debugger

```text
ori debug examples/cli_args/main.orl --breakpoint 41
```

At a stop:

| Key | Action |
|---|---|
| `c` | continue |
| `s` | step to the next instrumented line |
| `q` | terminate the target |

The terminal adapter shows the source location, stack (including supported
async frames), and visible locals.

## DAP server

Start the adapter over stdio:

```text
ori debug --dap
```

The current adapter implements `initialize`, `launch`, `setBreakpoints`,
`configurationDone`, `continue`, `next`, `threads`, `stackTrace`, `scopes`,
`variables`, `evaluate`, and `disconnect`.

Variables expose qualified struct fields, optional/result payloads, enum
payloads, maps, sets, supported opaque collections, and bounded list children.
Async frames remain visible across `await`; closure captures appear in their
closure frame. Strings and bytes are shown with bounded previews. `evaluate`
only evaluates scalar arithmetic, comparisons, boolean logic, and strings from
the latest stopped snapshot; it never executes target code.

Native builds also emit `program.debug.json`, a portable catalogue of source
lines, parameters, locals, pattern bindings, and closure captures.

## IDE integration

The VS Code extension registers the `ori` debugger type and starts
`ori debug --dap`. Zed currently exposes LSP integration; automatic debugger
wiring is not available through its extension API.

For linker/runtime diagnosis, run `ori doctor` before changing source code.
