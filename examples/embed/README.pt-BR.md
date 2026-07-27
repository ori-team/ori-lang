# Embarcando Ori como biblioteca compartilhada (`ori compile --lib`)

Hosts como C/C++, Python e engines podem carregar uma biblioteca Ori e chamar
funções marcadas com `@c_export`.

## Linguagem

```orl
module app.embed_add

@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end

@c_export("mul_scores")
public mul(a: int, b: int) -> int
    return a * b
end
```

- Apenas funções livres e `public`.
- São aceitos números, `bool`, `string` e structs não vazias e não genéricas.
- Structs escalares usam parâmetros `const OriType *`; retornos usam um último
  parâmetro `OriType *out`.
- Structs com campos gerenciados usam handles opacos `OriTypeHandle *`.
  Parâmetros são emprestados; retornos pertencem ao host e precisam ser
  liberados com `ori_arc_release`.
- O símbolo pode ser renomeado com `@c_export("symbol_name")`.

## Compilação

```bash
# Requer o runtime com cdylib preparado (libori_runtime.so):
#   sh tools/stage_native_runtime.sh --profile release
export ORI_USE_SYSTEM_LINKER=1
ori compile --lib examples/embed/add_scores.orl -o libadd_scores.so
```

O comando grava `libadd_scores.so` e o header canônico `libadd_scores.h`.
Inclua esse arquivo gerado no host em vez de manter declarações C manualmente.

A biblioteca depende dinamicamente de `libori_runtime.so` para o mesmo target.
Mantenha o diretório do runtime em `LD_LIBRARY_PATH`, no `rpath` ou ao lado do
binário host.

## Contrato do host

```c
#include "libadd_scores.h"

void *h = dlopen("libadd_scores.so", RTLD_NOW);
int  (*runtime_init)(void) = dlsym(h, "ori_rt_init");
void (*runtime_shutdown)(void) = dlsym(h, "ori_rt_shutdown");
int64_t (*add_scores_fn)(int64_t, int64_t) = dlsym(h, "add_scores");

runtime_init();
printf("%lld\n", (long long)add_scores_fn(2, 3));
runtime_shutdown();
```

## Smoke test

```bash
sh tools/qa/embed_smoke.sh
```

## Fases

| Fase | Estado |
|------|--------|
| P1 `--lib` + escalares + `ori_rt_*` | **concluída** |
| P2 strings + structs escalares/gerenciadas + header gerado | **concluída** |
| P3 callbacks host→Ori | planejada |
| P4 exemplo Godot GDExtension | planejada |
| P5 Windows / macOS | planejada |
