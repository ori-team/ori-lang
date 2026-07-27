# How to report bugs

> Status: practical policy for Ori **0.3.8 / S3**  
> **Portuguese:** [report-bugs.pt-BR.md](report-bugs.pt-BR.md)  
> Security vulnerabilities: follow [`../../SECURITY.md`](../../SECURITY.md), not a public issue

A good report lets someone reproduce the issue with a few commands.

## Language / type checker

Include:

- `ori --version`;
- OS and target architecture;
- minimal `.orl` file;
- exact command, such as `ori check main.orl`;
- complete diagnostic output;
- whether the behavior also appears in `ori run`, `ori compile`, or the editor.

Use this category for lexer, parser, name resolution, type checker, imports, generics, traits, matching, `try`, formatter, and language diagnostics.

## Stdlib / runtime

Also include:

- module or operation (`ori.fs`, `ori.json`, and so on);
- whether it fails under JIT `ori run`, AOT `ori compile`, or both;
- target triple when known;
- runtime/staticlib/cdylib staging information when developing from the repository;
- for memory issues, relevant leak/ARC output such as `ORI_TEST_LEAK_CHECK=1`;
- whether cleanup, aliasing, concurrency, I/O, or platform behavior is involved.

## Tooling, projects, and packages

This includes `ori fmt`, `ori doc`, `ori new`, REPL, LSP, VS Code/Zed integrations, manifests, lockfiles, dependencies, installers, updater, and release packages.

Include:

- exact command or editor action;
- minimal project layout;
- relevant manifest/lockfile without secrets;
- whether it fails outside the repository checkout;
- editor language-server logs when applicable;
- package source and resolved revision/version when dependencies are involved.

Remove registry tokens, credentials, private paths, and unrelated personal data.

## Performance reports

Include:

- workload and input;
- debug/release mode;
- AOT/JIT and optimization settings;
- target, OS, CPU, and memory;
- sample count and statistic;
- baseline version/commit;
- reproduction script;
- evidence that the benchmark still performs the intended work.

See [`../quality/performance-policy.md`](../quality/performance-policy.md).

## Suggested template

```text
Title: short description

Environment:
- Ori version/commit:
- OS and architecture:
- Target triple:
- Route: check / AOT / JIT / LSP / package
- Relevant environment variables:

Reproduction:
1. ...
2. ...

Expected:

Actual:

Diagnostics/output:

Minimal file or project:
module app.main

main()
end

Regression:
- Last known working version/commit, if known:
```

Start with the smallest source or project that preserves the problem. Link larger evidence only when the minimized case cannot reproduce it.