# Plano: `ori compile --lib` — shared library embarcável (cdylib)

> **Criado:** 2026-07-16 · Motivação: hospedar código Ori dentro de hosts
> nativos (Godot via GDExtension, Python, qualquer engine C/C++), no modelo
> godot-rust/gdext: o host carrega um `.so`/`.dll` que registra funções.
> Origem da demanda: pivot do editor Ori Studio para Godot
> (`game-engine-full/ori-game-studio/DEV-HANDOFF.md`).

## 0. Estado atual (atualizado 2026-07-23)

### Implementado (P1 + P2)

- `ori compile --lib -o libfoo.so` emite **shared library** (Linux; SystemLinker
  + link dinâmico a `libori_runtime.so`).
- Anotação `@c_export` / `@c_export("sym")` em funções `public` com números,
  `bool`, `string` e structs não vazias/não genéricas.
- Ponteiro/out portátil para structs escalares, handles ARC opacos para structs
  gerenciadas e header `.h` gerado automaticamente ao lado da biblioteca.
- Runtime: `ori_rt_init` / `ori_rt_shutdown`; lib emite `__ori_module_init`.
- Smoke: `tools/qa/embed_smoke.sh` + `tests/native/embed_smoke.c`
  (`add_scores(2,3)==5`, 1M calls ~28 ns/call no host de dev).
- Exemplo: `examples/embed/` (+ stub Godot em `examples/embed/godot/`).

### Ainda em aberto

- Valores diretos `optional`/`result` e handles diretos de `list`/`map`.
- **P3** callbacks host→Ori no path embed.
- **P4** shim GDExtension completo em CI.
- **P5** Windows/mac.
- **Issue #1** (custo/call em módulos grandes) — monitorar no P4; path scalar
  P1 está bem abaixo do teto 2 µs.

### Histórico (pré-implementação)

- Callbacks C→Ori já existiam no path de executável (raylib). Faltava
  empacotamento cdylib + boot sem `main`.
- FFI Ori→C: `int` = i64 no registrador.

## 1. Objetivo e não-objetivos

**Objetivo:** `ori compile --lib -o libfoo.so pacote/` produz uma shared
library com (a) funções Ori marcadas como exportadas visíveis com ABI C,
(b) init/shutdown explícitos do runtime, (c) PIC correto.

**Não-objetivos (fase posterior):** integração como *linguagem de script de
editor* no Godot (ScriptLanguageExtension, hot-reload, debugger); Windows/mac
(seguem depois do Linux, mesmo padrão dos hosts).

## 2. Superfície de linguagem

Exportação explícita por anotação (espelha o `extern c` de importação):

```orl
@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end
```

- Permitido apenas em funções `public` de módulo com assinatura FFI-safe:
  números, `bool`, `void`, `string` NUL-terminated, structs escalares por
  pointer/out e structs gerenciadas por handles ARC opacos.
- Nome do símbolo = nome da função (sem mangling); colisões = erro de
  compilação. Opcional: `@c_export("nome_custom")`.
- Diagnóstico claro quando a assinatura não é FFI-safe.

## 3. Runtime embarcável

Símbolos exportados pelo runtime (`ori-runtime`):

```c
int  ori_rt_init(void);      // aloca/inicializa GC, TLS, stdlib state
void ori_rt_shutdown(void);  // best-effort; processo host segue vivo
```

- `main()` do usuário é **opcional** quando `--lib`.
- Globals de módulo: `__ori_module_init` (export da lib; host deve chamar
  após `ori_rt_init` se o módulo usa globals).
- Contrato de threads fase 1: **single-thread**.
- Reentrância host→Ori→host (callback dentro de export) — P3.

## 4. Codegen / link

- Cranelift: `is_pic = true` no target AOT (já default).
- Link: `cc -shared -fPIC` + **cdylib** `libori_runtime.so` (evita colisão
  compiler-builtins × libgcc do staticlib com `--whole-archive`).
- Símbolos `@c_export` e `__ori_module_init` com `Linkage::Export`.

## 5. Fases e critérios de aceite

| Fase | Entrega | Aceite | Status |
|------|---------|--------|--------|
| **P1** | `--lib` + `@c_export` int/float/bool + `ori_rt_init/shutdown` | Harness C: dlopen, init, `add_scores(2,3)==5`, 1M calls | **done** |
| **P2** | Strings + structs escalares/gerenciadas + header gerado | Harness C inclui o `.h`, valida ida/volta e ownership ARC | **done** |
| **P3** | Callbacks host→Ori registráveis | Harness registra callback e Ori o invoca | open |
| **P4** | Exemplo Godot GDExtension | Cena 60fps + ≤2 µs/call | stub docs |
| **P5** | Windows/mac | CI matrix | open |

**P4 é o teste de realidade**: além de provar a feature, mede o custo por
chamada no contexto real — amarra com a issue #1 (o aceite inclui
`custo/call ≤ 2µs` com o módulo de exemplo, para impedir regressão).

## 6. Riscos

| Risco | Mitigação |
|-------|-----------|
| Issue #1 torna o bridge inútil em módulos grandes | Fix junto/antes; aceite de perf no P4 |
| Runtime assume processo próprio (signals? TLS? argv?) | `ori_rt_init` não toca argv/signals |
| GC × ponteiros retidos pelo host | Fase 1: host **não retém** managed refs (só escalares) |
| PIC / link staticlib × libgcc | Shared link usa **cdylib** do runtime |

## 7. Ligações

- Issue perf: https://github.com/raillen/ori-lang/issues/1
- Consumidor alvo: plano Godot em `game-engine-full/docs/planning/PLANO-GODOT-STUDIO.md`
- Smoke: `tools/qa/embed_smoke.sh`
- Exemplo: `examples/embed/README.md`
