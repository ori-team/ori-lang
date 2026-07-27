# Residual cleanup snapshot — 2026-07-13

> **Status:** cleaned for product surface under FREEZE-1.  
> Normative inventory remains [`docs/spec/14-backend-support.md`](../../spec/14-backend-support.md).  
> Policy: [`lang-res-closure.md`](../historico/lang-res-closure.md).

## Definition of “clean”

| Check | Result |
|-------|--------|
| Examples + promised async + product residual gate compile/run native | **required green** |
| Intentional limits documented in Spec 14 | **kept** |
| No open BACKLOG item inventing borrow/VM/GC suites | **confirmed** |
| Negative tests for permanent exclusions | **present** (for-without-iterator ABI) |

## Intentional residuals (not bugs)

| Residual | Policy |
|----------|--------|
| Rare async frame layout / planner reject | Actionable `backend.native_unsupported`; reopen only with repro |
| `for` over type without iterator ABI | Permanent until ABI exists |
| Indexed store on unsupported bases | Permanent until store path |
| C/debug incomplete vs native | Out of LANG-RES; matrix partial/no |

## Actions completed this cleanup

1. Residual policy skill + `tools/qa/residual_audit.sh`.  
2. Daily stages include residual gate (S8).  
3. Spec 14 + lang-res-closure remain source of truth (no silent expansion).  
4. Product surface gate remains regression-protected.

## How to re-audit

```bash
tools/qa/residual_audit.sh
```

## Not cleaned by design (would be feature work / freeze exit)

- Full static infinite-loop analysis  
- Borrow/lifetime system  
- Bytecode VM  
- C async parity  
- Perfect DCE/const-prop suite as product gates  

These map to **N/P** in `docs/planning/qa/test-matrix-ori.md`.
