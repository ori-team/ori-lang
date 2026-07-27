# Diagnostics quality (Ori)

## Contract

Every user-facing diagnostic:

| Field | Rule |
|-------|------|
| **Code** | `category.snake_name` present in Emitted table of `docs/spec/13-error-catalog.md` |
| **Severity** | error / warning / note as catalog |
| **Span** | points at the offending token/node, not EOF unless true |
| **Primary message** | what failed (English in compiler strings today) |
| **Action** | how to fix when mechanical (S3 removals always have action) |
| **No stale syntax** | never suggest pre-S3 (`func` decl, `namespace`, `else if`, `?` propagate) |

## Categories (prefixes)

| Prefix | Domain |
|--------|--------|
| `lex.*` | Lexer |
| `parse.*` | Parser / removed surface |
| `name.*` | Name resolution |
| `bind.*` | Binding / import / fields / params |
| `type.*` | Type check / inference |
| `generic.*` | Generics |
| `backend.*` | Codegen residual |
| `doc.*` | Oridoc |
| `package.*` | Packaging |

`bind.undefined` is reserved; emit **`name.undefined`**.

## When changing messages

1. Prefer clearer **action** over rewording only.  
2. Keep code stable (tools/tests depend on it).  
3. `cargo test -p ori-driver --test diagnostic_catalog`.  
4. Add/adjust `check_fails` if behavior changes.  
5. Spec 13 Emitted row if new code.

## Multi-error recovery

- Prefer continuing after parse error when safe (more diagnostics).  
- Do not cascade 50 type errors from one missing `end` without a clear primary parse error.

## S3 removal diagnostics

All `parse.*_removed` / migrate helpers must stay accurate vs `ori migrate-syntax` and CHANGELOG `[0.3.0]`.
