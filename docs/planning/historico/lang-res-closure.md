# LANG-RES closure (native residuals)

> Status: **closed** 2026-07-13  
> Normative inventory: [`docs/spec/14-backend-support.md`](../../spec/14-backend-support.md)

## Definition

**LANG-RES** = fix or document **native backend residuals** only when they
**block real programs** on the product surface. Do **not** invent features
to chase theoretical shapes.

Product surface (for this gate):

- Official `examples/*` mini-projects
- Promised native async subset (LANG-1)
- Stdlib APIs used by those programs

## Audit (2026-07-13)

| Check | Result |
|-------|--------|
| All 21 examples `ori compile` (AOT) | pass |
| All examples `ori run` / `ori test` | pass (prior wave) |
| Spec 14 residual inventory | up to date |
| Negative residual: `for` without iterator ABI | `compile_rejects_for_iterable_without_native_abi` |
| Positive residual risk surface | `compile_runs_lang_res_product_surface_native` |

No open **product-blocking** `backend.native_unsupported` on the surface above.

## Documented residuals (not open work)

These remain **intentional** or **rare** (Spec 14):

| Kind | Policy |
|------|--------|
| Async body layout / planner reject | Rare; actionable `backend.native_unsupported` with function name |
| `for` over types without iterator ABI | Permanent until that type gains next-ABI |
| Indexed lvalue on unsupported bases | Permanent until store path exists |
| Internal defense map/set/graph call names | Must not surface from valid stdlib use |
| C/debug gaps (async, net, …) | Out of LANG-RES; LANG-3 / matrix |

## Reopen criteria

Reopen **LANG-RES** only with **all** of:

1. A minimal `.orl` program that is valid language + types.
2. AOT native path emits `backend.native_unsupported` (or wrong codegen).
3. The program is a realistic product use (stdlib + control flow), not a
   synthetic edge invented only to expand the backend.

Then: fix the residual, add a positive test, update Spec 14 in the same change.

## Living maintenance

After close, Spec 14 inventory + the two regression tests above are the
living contract. No separate “partial LANG-RES” backlog item.
