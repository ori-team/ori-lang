# Plano: `ori compile --lib` — shared library embarcável (cdylib)

> **Criado:** 2026-07-16 · Motivação: hospedar código Ori dentro de hosts
> nativos (Godot via GDExtension, Python, qualquer engine C/C++), no modelo
> godot-rust/gdext: o host carrega um `.so`/`.dll` que registra funções.
> Origem da demanda: pivot do editor Ori Studio para Godot
> (`game-engine-full/ori-game-studio/DEV-HANDOFF.md`).

## 0. Estado atual (verificado 2026-07-16)

- `ori compile` emite **apenas executável com `main`** (Cranelift → objeto →
  system linker `cc`). Não há flag de biblioteca.
- **Callbacks C→Ori já funcionam em runtime**: hosts (raylib `run_window`)
  recebem ponteiros de função Ori e chamam de volta — o mecanismo de
  trampolim existe. O que falta é o *empacotamento* (cdylib) e o *boot*
  (inicializar o runtime sem passar por `main`).
- FFI Ori→C: `int` = i64 no registrador (provado por probe echo);
  hosts do ecossistema já migrados para `int64_t` em params de ponteiro.
- **Dependência crítica**: issue #1 (custo por chamada cresce com o tamanho
  do binário — 0,55µs → ~1,5ms). Um bridge de scripting multiplica
  crossings/frame; **o fix da #1 deve acompanhar ou preceder este plano**,
  senão a feature nasce inutilizável para jogos.

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
  `int`, `float` (f64), `bool`, `void`; strings **na fase 2** via par
  (ptr: int, len: int) + helpers `ori.mem`.
- Nome do símbolo = nome da função (sem mangling); colisões = erro de
  compilação. Opcional: `@c_export("nome_custom")`.
- Diagnóstico claro quando a assinatura não é FFI-safe.

## 3. Runtime embarcável

Novos símbolos exportados pelo runtime (ori-runtime):

```c
int  ori_rt_init(void);      // aloca/inicializa GC, TLS, stdlib state
void ori_rt_shutdown(void);  // best-effort; processo host segue vivo
```

- `main()` do usuário passa a ser **opcional** quando `--lib`.
- Globals de módulo: inicializadores rodam em `ori_rt_init` (ordem =
  ordem de módulo já usada pelo executável).
- Contrato de threads fase 1: **single-thread** — todas as chamadas do host
  na mesma thread (Godot chama scripts na main thread; suficiente).
  Documentar; assert em debug.
- Reentrância host→Ori→host (callback dentro de export) já é o padrão
  raylib — cobrir com teste.

## 4. Codegen / link

- Cranelift: flag `is_pic = true` no target quando `--lib` (relocações PIC);
  hoje o executável pode estar non-PIC — verificar `TargetFrontendConfig`.
- Link: `cc -shared -fPIC obj... -o libX.so` + `native_libs` estáticas do
  pacote (mesma resolução de paths do executável).
- **Não** exportar símbolos internos: version-script/`-fvisibility=hidden`
  com lista dos `@c_export` + `ori_rt_*` (evita colisão com o host).

## 5. Fases e critérios de aceite

| Fase | Entrega | Aceite |
|------|---------|--------|
| **P1** | `--lib` + `@c_export` int/float/bool + `ori_rt_init/shutdown` | Harness C (`tests/native/embed_smoke.c`): dlopen, init, chama `add_scores(2,3)==5`, shutdown, sem leaks/crash em loop de 1M chamadas |
| **P2** | Strings (ptr+len in/out) + listas opacas (handle) | Harness passa string UTF-8 ida/volta |
| **P3** | Callbacks host→Ori registráveis (ponteiro de função C recebido do host) | Harness registra callback e Ori o invoca |
| **P4** | Exemplo `examples/embed/godot/`: shim GDExtension C mínimo registrando 1 classe com métodos Ori | Cena Godot 4.x chama lógica Ori a 60fps (Compatibility, HD 4000) |
| **P5** | Windows/mac (depois dos smokes de host do ecossistema) | CI matrix |

**P4 é o teste de realidade**: além de provar a feature, mede o custo por
chamada no contexto real — amarra com a issue #1 (o aceite inclui
`custo/call ≤ 2µs` com o módulo de exemplo, para impedir regressão).

## 6. Riscos

| Risco | Mitigação |
|-------|-----------|
| Issue #1 torna o bridge inútil em módulos grandes | Fix junto/antes; aceite de perf no P4 |
| Runtime assume processo próprio (signals? TLS? argv?) | Auditar `ori-runtime` init path; `ori_rt_init` não pode tocar argv/signals |
| GC × ponteiros retidos pelo host | Fase 1: host **não retém** managed refs (só escalares); handles opacos com pin na fase 2 |
| PIC quebra algo no Cranelift path atual | P1 começa por um hello-lib sem stdlib pesada |

## 7. Ligações

- Issue perf: https://github.com/raillen/ori-lang/issues/1
- Consumidor alvo: plano Godot em `game-engine-full/docs/planning/PLANO-GODOT-STUDIO.md`
- Referência de shim host: `game-engine-full/ori-imgui/native/ori_imgui_host.cpp`
  (padrão de host C + callbacks) e godot-rust/gdext (modelo cdylib).
