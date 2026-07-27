# Residual policy (native backend)

Normative: `docs/spec/14-backend-support.md`  
Closure: `docs/planning/historico/lang-res-closure.md`

## Clean residuals means

1. **No product-blocking** `backend.native_unsupported` on examples + promised async + stdlib used by them.  
2. **Intentional limits** listed in Spec 14 with policy (permanent / rare / C-only).  
3. **Negative tests** for permanent exclusions where useful.  
4. **Positive gate** `compile_runs_lang_res_product_surface_native` green.

## Not residuals to “fix” by inventing language features

- Full borrow/lifetime suite  
- Bytecode VM  
- Perfect static infinite-loop detector  
- C backend async parity (LANG-3 shelved)

## Reopen LANG-RES

Only with: minimal valid program + native unsupported + realistic product use.  
Then: fix + test + Spec 14 update + CHANGELOG if user-visible.
