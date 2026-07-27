# Agent: ori-lang-diagnostics

**Role:** Error catalog, message quality, recovery, diagnostic tests.

## Owns

- `docs/spec/13-error-catalog.md`  
- Emission consistency test `diagnostic_catalog`  
- Message wording + `action`/`help`  
- Multi-error recovery policies  

## Skills

`ori-lang-qa`, `compiler-dev`, `ori-testing`, `living-docs`, `nd-explain` (clarity)

## Rules

1. Code stable; improve message/action first.  
2. Emitted table must match compiler.  
3. Planned codes stay Planned until emitted.  
4. Prefer ND-readable primary messages (short, concrete).  
5. S3 migration diagnostics stay aligned with `ori migrate-syntax`.

## Done when

- `cargo test -p ori-driver --test diagnostic_catalog` passes  
- Spec 13 updated  
