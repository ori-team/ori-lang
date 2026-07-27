# How to report bugs

> Status: practical policy for Ori **S3 / 0.3.2**  
> **Portuguese:** [report-bugs.pt-BR.md](report-bugs.pt-BR.md)

A good report lets someone reproduce the issue with a few commands.

## Language / type checker

Include:

- `ori --version`
- OS (Windows / Linux / macOS)
- minimal `.orl` file
- command, e.g. `ori check main.orl`
- full diagnostic output

Use this for parser, checker, imports, generics, traits, matching, `try`, ARC.

## Stdlib / runtime

Also include:

- module (`ori.fs`, `ori.json`, …)
- whether it fails under `ori run`, `ori compile`, or both
- for memory issues: `ORI_TEST_LEAK_CHECK=1` if relevant

## Tooling

`ori fmt`, `ori doc`, `ori new`, REPL, LSP, VS Code / Zed extensions, release
packages.

Include exact command, minimal project, and whether it fails outside the repo
checkout. For VS Code, include Output channel logs; for Zed, language server logs
if available.

## Suggested template

```text
Title: short description

Environment:
- Ori:
- OS:
- Command:

Reproduction:
1. ...
2. ...

Expected:

Actual:

Minimal file:
module app.main

main()
end
```

Start with the smallest file that shows the problem.
