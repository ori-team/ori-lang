# Repository and project layout — migrated

> Status: **accepted historical decision; current architecture moved**  
> Original decision date: 2026-07-13

The canonical current layout is now:

- [`../architecture/repository-and-project-layout.md`](../architecture/repository-and-project-layout.md)

The current document owns repository roles, Cargo workspace location, runtime staging, stdlib/examples/docs domains, and root-first user projects.

## Historical decision retained

The accepted design established these durable choices:

- the Cargo workspace lives under `compiler/`;
- the repository root contains language-product domains such as `stdlib/`, `runtime/`, `docs/`, `examples/`, `tools/`, and `extensions/`;
- examples are real projects rather than loose source scraps;
- user projects use `ori.proj` at the root;
- `main.orl` is the recommended application entry but is configurable;
- `src/` is optional rather than mandatory;
- domain directories are optional;
- temporary work and scratch content do not belong at the repository root.

The historical rationale should be migrated into a numbered ADR under `docs/decisions/adr/`. Do not extend this file with new current architecture or implementation plans.