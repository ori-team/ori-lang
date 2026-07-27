# LANG-MEM-9 — wrappers result/optional do runtime nativo sob ARC

- Data: 2026-07-18
- Contexto: item aberto na verificação de bugs externos
  ([`bugcheck-native-ori-ide-2026-07-18.md`](bugcheck-native-ori-ide-2026-07-18.md) §5.1)

## 1. TL;DR

Os wrappers `result`/`optional` construídos pelo runtime nativo
(`new_result*`, `new_optional_ptr`) usavam `libc::malloc` cru — invisíveis
ao registro ARC. Todo release emitido pelo codegen era no-op silencioso e
cada chamada de `ori.fs`/`ori.process`/net vazava o payload (12 alocações
a cada 10 `fs.read_text_or`). Agora os wrappers passam por `ori_alloc` e
**possuem o payload via edge** (dono único da cascata). No caminho, o
`try`/`?` foi corrigido: ele **abandonava o wrapper owned no caminho ok** e
retinha sem consumir no caminho err (32 leaks a cada 10 `try`).

## 2. O que mudou

### Runtime (`ori-runtime/src/lib.rs`)

- Helper novo `wrapper_owns_payload(wrapper, payload)`: registra a edge
  (+1) e solta o +1 do temporário — transferência de posse; no-op para
  payload null/não-managed.
- `new_result(is_ok, payload)` → `ori_alloc` + transfer. Auditados os
  ~130 call sites: todos os payloads são frescos (cstrings, streams
  `ori_alloc`+dtor, maps/lists recém-criados) — nenhum borrowed.
- `new_result_raw`/`_i64_ok`/`_f64_ok`/`_bool_ok` → `ori_alloc`, sem edge
  (payload inline; `_raw` carrega i64 opaco de task/channel — transfer às
  cegas arriscaria colisão int-vs-endereço, fica como está).
- `new_optional_ptr` → já era `ori_alloc`; ganhou o transfer (antes o
  payload ficava órfão quando o wrapper era liberado).

### Codegen (`native_backend.rs`, `Propagate`)

- Caminho **ok**: payload agora sai **sempre owned** (retain antes de
  liberar o wrapper); wrapper owned é consumido ali. `Propagate` entrou em
  `expr_produces_owned_ref`.
- Caminho **err** sync: wrapper owned transfere o +1 direto para o retorno
  (sem retain); borrowed ganha retain para o caller.
- Caminho **err** async: retain só quando borrowed; o release existente
  passa a consumir o +1 do temp owned.

### Testes

- `tests.rs` do runtime: helpers da era malloc
  (`release_result_payload_and_free`/`free_result`) faziam release manual
  do payload + `libc::free` do wrapper — agora um único
  `ori_arc_release(wrapper)` (o free cru abortava com
  `free(): invalid pointer` sobre o ponteiro pós-header).
- 2 regressões novas em `memory_arc.rs`: `fs_read_text_or_loop_no_leak`
  (12→0) e `try_unwrap_loop_no_leak` (32→0, ok e err).

## 3. Armadilha de medição documentada

`assert_no_leaks` **dentro** de uma expressão de concat
(`io.println("x=" + string(a) + " leaks=" + string(test.assert_no_leaks(...)))`)
conta o temporário do primeiro `+` (avaliação esquerda→direita) e reporta
1 falso leak. Padrão correto (usado na suíte): bind o assert num `const`
antes do print. Três "leaks" investigados nesta sessão eram esse artefato.

## 4. Validação

Runtime 48/48; memory_arc 29/29 (+1 ignored pré-existente); ori_spec
161/161; performance guard verde; suíte completa idêntica à baseline
(51/1 + 340/8 pré-existentes de closures/C-backend).
