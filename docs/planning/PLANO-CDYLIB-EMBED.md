# Plano: `ori compile --lib` — shared library embarcável (cdylib)

> **Criado:** 2026-07-16 · **Atualizado:** 2026-08-09.
> **Motivação:** hospedar código Ori dentro de hosts nativos por uma ABI C
> geral. A demanda nasceu durante um experimento com Godot, mas nenhuma engine
> específica faz parte do produto atual. Este arquivo registra a fundação já
> entregue; o trabalho aberto está no
> [plano de runtime hospedado e Host ABI v1](embedded-runtime-host-abi-v1.md).

## 0. Estado atual (atualizado 2026-08-09)

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

### Residual reclassificado

- Handles diretos de `list`/`map` e outros aggregates de collection continuam
  fora do ABI-1. Bridges diretos de `optional`/`result` já foram entregues;
  ver [19-abi.md](../spec/19-abi.md#83b-c_export--the-host-facing-surface).
- Callbacks, traps recuperáveis fora do caminho escalar, lifecycle por contexto,
  buffers em lote e negociação de versão pertencem agora a **EMBED-HOST-1**;
  o `ori-embed` já possui o primeiro slot de erro para traps escalares.
- JIT persistente escalar, handles geracionais e unload explícito já existem
  na sessão Rust experimental; JIT incremental, reload seguro concorrente,
  callbacks e migração de estado pertencem a **COMP-SVC-1**.
- Windows/macOS continuam sujeitos à política geral de distribuição, não a
  uma integração com engine.
- O path escalar P1 mediu cerca de 28 ns/chamada no host de desenvolvimento;
  workloads maiores devem permanecer em benchmarks antes de qualquer nova
  promessa de desempenho.

### Histórico (pré-implementação)

- Callbacks C→Ori já existiam no path de executável (raylib). Faltava
  empacotamento cdylib + boot sem `main`.
- FFI Ori→C: `int` = i64 no registrador.

## 1. Objetivo e não-objetivos

**Objetivo:** `ori compile --lib -o libfoo.so pacote/` produz uma shared
library com (a) funções Ori marcadas como exportadas visíveis com ABI C,
(b) init/shutdown explícitos do runtime, (c) PIC correto.

**Não-objetivos:** integrações específicas com engines, editores ou frameworks;
uma VM separada; sintaxe de entidades/cenas/componentes; e exposição direta do
layout privado das collections. Adaptadores pertencem a repositórios externos
e devem consumir a mesma Host ABI geral.

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
- Contrato de threads fase 1: o runtime possui ARC atômico e aceita os tipos
  documentados em `Transferable`; callbacks host→Ori continuam fora do escopo.
- Reentrância host→Ori→host (callback dentro de export) — P3.

## 4. Codegen / link

- Cranelift: `is_pic = true` no target AOT (já default).
- Link: a estratégia `NativeLinker` seleciona o linker empacotado ou o linker
  do sistema conforme a política documentada em [`docs/install.md`](../install.md);
  o runtime usa a **cdylib** correspondente para evitar colisão de símbolos do
  staticlib.
- Símbolos `@c_export` e `__ori_module_init` com `Linkage::Export`.

## 5. Fases e critérios de aceite

| Fase | Entrega | Aceite | Status |
|------|---------|--------|--------|
| **P1** | `--lib` + `@c_export` int/float/bool + `ori_rt_init/shutdown` | Harness C: dlopen, init, `add_scores(2,3)==5`, 1M calls | **done** |
| **P2** | Strings + structs escalares/gerenciadas + header gerado | Harness C inclui o `.h`, valida ida/volta e ownership ARC | **done** |
| **P3** | Callbacks host→Ori registráveis | Harness C registra callback e Ori o invoca com erro recuperável | moved to **EMBED-HOST-1** |
| **P4** | Compatibilidade com host real | Harness genérico mede chamadas escalares e em lote sem depender de engine | moved to **EMBED-HOST-1** |
| **P5** | Windows/macOS | CI matrix conforme a política geral de distribuição | deferred |

O teste de realidade passa a ser um harness C/C++ genérico com chamadas
escalares, buffers, callbacks, erro e reload. Uma integração externa deve
conseguir usar o mesmo contrato sem alterar o compilador Ori.

## 6. Riscos

| Risco | Mitigação |
|-------|-----------|
| Custo cresce em módulos grandes | Benchmark genérico por tamanho de módulo; otimizar somente com regressão reproduzível |
| Runtime assume processo próprio (signals? TLS? argv?) | `ori_rt_init` não toca argv/signals |
| ARC × referências retidas pelo host | Usar exclusivamente handles e funções de retain/release documentadas |
| PIC / link staticlib × libgcc | Shared link usa **cdylib** do runtime |

## 7. Ligações

- Runtime hospedado/Host ABI: [`embedded-runtime-host-abi-v1.md`](embedded-runtime-host-abi-v1.md)
- Metadata estática: [`static-metadata-attributes.md`](static-metadata-attributes.md)
- Compiler service/JIT modular: [`interactive-compiler-service.md`](interactive-compiler-service.md)
- Performance de value types: [`value-types-performance.md`](value-types-performance.md)
- Smoke: `tools/qa/embed_smoke.sh`
- Exemplo: `examples/embed/README.md`
