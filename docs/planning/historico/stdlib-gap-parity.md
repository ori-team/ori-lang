# Stdlib Gap Parity — Referência `std.*` vs Ori `ori.*`

> Status: **Layer 2/3 stdlib `.orl` fechados para uso prático v1** (`[Unreleased]`, congelado em `0.2.x`)
> Última revisão: 2026-07-01

## Objetivo

Fechar gaps funcionais entre a stdlib de referência (módulos `std.io`, `std.text`,
`std.fs.path`, etc.) e a stdlib Ori, **sem violar a arquitetura de 3 camadas**:

| Camada | Papel | Regra |
|--------|-------|-------|
| **Layer 1** | Manifesto Rust + FFI | Hot path, I/O de SO, rede, metadados FS |
| **Layer 2** | `.orl` composicional | Wrappers, validação, paths, erros uniformes |
| **Layer 3** | `.orl` algoritmos | Lógica pura sobre Layer 1+2 |

## Mapa de equivalência (estado atual)

| Referência | Ori | Status |
|------------|-----|--------|
| `std.io` | `ori.io` + `ori.io.utils` | ✅ Layer 1 + Layer 2 |
| `std.text` | `ori.string` + `ori.string.utils` + `ori.string.algorithms` | ✅ |
| `std.bytes` | `ori.bytes` + `ori.bytes.utils` + `ori.bytes.algorithms` | ✅ |
| `std.json` | `ori.json` + `ori.json.utils` | ✅ |
| `std.format` | `ori.format` + `ori.format.utils` | ✅ |
| `std.fs` | `ori.fs` + `ori.fs.utils` | ✅ Layer 1 `result` + Layer 2 |
| `std.fs.path` | `ori.path` | ✅ (`relative` implementado) |
| `std.os` | `ori.os` + `ori.os.utils` | ✅ |
| `std.os.process` | `ori.process` + `ori.process.utils` | ✅ |
| `std.time` | `ori.time` + `ori.time.utils` | ✅ (ms `int`, sem tipos ricos) |
| `std.list` / `std.map` / `std.set` | `ori.list` / `ori.map` / `ori.set` + utils + algorithms | ✅ Ori **supera** referência |
| `std.collections` | `ori.queue`, `stack`, `deque`, `heap`, `hash_table`, `linked_list`, `doubly_linked_list`, `tree`, `graph` + utils | ✅ |
| `std.math` | `ori.math` + `ori.math.utils` + `ori.math.algorithms` | ✅ |
| `std.random` | `ori.random` + `ori.random.utils` | ✅ |
| `std.validate` | `ori.validate` | ✅ (subconjunto útil; expandível) |
| `std.concurrent` | `ori.concurrent` + `ori.concurrent.utils` + `task`/`channel` | ⚠️ `transfer_*` aliases; contrato `Transferable` real = backlog |
| `std.lazy` | `ori.lazy` + `is_consumed` | ✅ |
| `std.test` | `ori.test` + `ori.test.utils` | ✅ |
| `std.net` | `ori.net` + `ori.net.utils` | ✅ TCP + TLS + UDP (v2 síncrono) |
| `std.iter` | `ori.iter` + `ori.iter.utils` | ✅ |

## Inventário Layer 2 (`.orl`) — completo

| Módulo | Arquivo |
|--------|---------|
| `ori.validate` | `stdlib/validate.orl` |
| `ori.path` | `stdlib/path.orl` |
| `ori.io` | `stdlib/io.orl` |
| `ori.net` | `stdlib/net.orl` |
| `ori.format.utils` | `stdlib/format/utils.orl` |
| `ori.iter.utils` | `stdlib/iter/utils.orl` |
| `ori.net.utils` | `stdlib/net/utils.orl` |
| `ori.os.utils` | `stdlib/os/utils.orl` |
| `ori.random.utils` | `stdlib/random/utils.orl` |
| `ori.string.utils` | `stdlib/string/utils.orl` |
| `ori.list.utils` | `stdlib/list/utils.orl` |
| `ori.convert.utils` | `stdlib/convert/utils.orl` |
| `ori.map.utils` | `stdlib/map/utils.orl` |
| `ori.set.utils` | `stdlib/set/utils.orl` |
| `ori.bytes.utils` | `stdlib/bytes/utils.orl` |
| `ori.math.utils` | `stdlib/math/utils.orl` |
| `ori.json.utils` | `stdlib/json/utils.orl` |
| `ori.io.utils` | `stdlib/io/utils.orl` |
| `ori.fs.utils` | `stdlib/fs/utils.orl` |
| `ori.time.utils` | `stdlib/time/utils.orl` |
| `ori.test.utils` | `stdlib/test/utils.orl` |
| `ori.process.utils` | `stdlib/process/utils.orl` |
| `ori.concurrent.utils` | `stdlib/concurrent/utils.orl` |
| `ori.queue.utils` | `stdlib/queue/utils.orl` |
| `ori.stack.utils` | `stdlib/stack/utils.orl` |
| `ori.deque.utils` | `stdlib/deque/utils.orl` |
| `ori.heap.utils` | `stdlib/heap/utils.orl` |
| `ori.hash_table.utils` | `stdlib/hash_table/utils.orl` |
| `ori.linked_list.utils` | `stdlib/linked_list/utils.orl` |
| `ori.doubly_linked_list.utils` | `stdlib/doubly_linked_list/utils.orl` |

## Inventário Layer 3 (`.orl` algorithms) — completo

| Módulo | Arquivo |
|--------|---------|
| `ori.list.algorithms` | `stdlib/list/algorithms.orl` |
| `ori.tree.algorithms` | `stdlib/tree/algorithms.orl` |
| `ori.graph.algorithms` | `stdlib/graph/algorithms.orl` |
| `ori.map.algorithms` | `stdlib/map/algorithms.orl` |
| `ori.set.algorithms` | `stdlib/set/algorithms.orl` |
| `ori.string.algorithms` | `stdlib/string/algorithms.orl` |
| `ori.bytes.algorithms` | `stdlib/bytes/algorithms.orl` |
| `ori.math.algorithms` | `stdlib/math/algorithms.orl` |

## Layer 1 entregue (ciclo gap parity)

- FS metadados + `create_dir_all`
- `os.current_dir` / `os.change_dir`
- `random.seed`
- `process.run` / `process.run_capture`
- `net.*` TCP/TLS/UDP síncrono + tipos opacos `Connection`/`Listener`/`UdpSocket` (design: `docs/planning/net-v2-design.md`)
- `test.skip` (exit 77)
- `lazy.is_consumed`
- `bytes.from_list` / `to_list`
- `math` estendido (`trunc`, `ln`, `exp`, trig inversa, `log10`, `is_finite`)

## Lacunas remanescentes — o que ainda falta **para uso da linguagem**

### Stdlib (não bloqueia hello-world / CLI / scripts)

| Item | Impacto | Rastreabilidade |
|------|---------|-----------------|
| Uniformizar Layer 1 FS/io (`bool`/`string` → `result`/`optional`) | ✅ Feito | [`PENDENTES.md`](PENDENTES.md) |
| `time.Instant` / `Duration` tipados | ✅ Feito | Backlog v2 |
| `io.Input` / `io.Output` streams | ✅ MVP Layer 1 | `stdlib/io.orl` + runtime FFI |
| Flatten `import ori.X only (...)` | ✅ Feito | agrega `utils` + `algorithms` |
| `net` TLS / UDP / servidor TCP (síncrono) | ✅ Feito | `docs/planning/net-v2-design.md`, `PENDENTES.md` §4 |
| `net` async nativa (`*_async` no executor) | Rede avançada | Backlog longo prazo |
| Genéricos `map`/`set`/`graph` em `.orl` | Chaves user-defined | Trait gate `Hashable`+`Equatable` |
| `grid2d` / `circbuf` / `btree` | Coleções extras | Sem demanda concreta |
| `format.date_pattern` locale | i18n | Backlog v2 |
| Contrato `Transferable` real em `concurrent.utils` | Async seguro | Design futuro |
| `validate.*` completo vs referência | Validação rica | Expandir sob demanda |

### Toolchain / DX (bloqueia adoção pedagógica, não execução)

| Item | Prioridade | Rastreabilidade |
|------|------------|-----------------|
| `ori explain <code>` | Alta | [`PENDENTES.md`](PENDENTES.md) §1 |
| `ori doctor` | Alta | §1 |
| Guia Errors/Null/Void | Alta | §1 |
| `ori repl` | Média | §3 |
| `ori summary` | Média | §3 |
| `if then else` expressão | Média | §3 |
| Registry / installer | Baixa | [`registry-v2.md`](registry-v2.md) |
| Self-hosting / bootstrapping | Longo prazo | Critério 1.0 |

### Infra / distribuição

| Item | Nota |
|------|------|
| `ori compile` ainda usa toolchain Rust (link) | Phase 1–2 mitigam; `ori run` JIT elimina link |
| Stdlib ~70% Layer 1 Rust | Layer 2/3 `.orl` cobre wrappers e algoritmos comuns |
| Paridade C debug async | Nativo only; spec cap. 14 |

## Filosofia Ori preservada

1. **Erros explícitos** — preferir `result<T, string>`; Layer 2 converte legado.
2. **Reading-first** — APIs por intenção (`blank`, `parse_int_or`, `has_path`).
3. **Sem null** — `optional`/`result` only.
4. **Hot path no Rust** — rede, subprocess, metadados FS.
5. **Composição em Ori** — validate, path, wrappers, algorithms.

## Testes

- E2E por módulo novo: `compiler/crates/ori-driver/tests/multifile_imports.rs`
- Rede v2: `compile_runs_net_tcp_listen_accept_loopback`, `compile_runs_net_udp_loopback`, `compile_runs_net_connect_tls_reports_error_on_refused_port`, `check_accepts_net_v2_flatten_selective_imports`
- Smoke: `check_accepts_stdlib_gap_parity_imports`, `compile_runs_stdlib_layer2_remaining_utils`, `compile_runs_stdlib_layer3_algorithms_extensions`

## Mudança Arquitetural: Unificação de Namespaces (Opção C)

Recorte inicial entregue: `ori.string`, `ori.list` e `ori.fs` agora têm
módulos pai `.orl` para import seletivo de helpers/algoritmos diretamente no
namespace principal. Os caminhos antigos (`ori.string.utils`, `ori.string.algorithms`,
`ori.list.utils`, `ori.list.algorithms`, `ori.fs.utils`) seguem compatíveis.
Para detalhes de manutenção, consulte `docs/spec/15-stdlib-maintenance.md`
na seção "Namespace flattening (Opção C)".

## Referências

- `docs/spec/12-stdlib.md` — contratos públicos
- `docs/spec/15-stdlib-maintenance.md` — workflow Layer 2/3
- `stdlib/README.md` — inventário de módulos `.orl`
- [`net-v2-design.md`](net-v2-design.md) — TLS, servidor TCP, UDP, blocking vs async
- [`PENDENTES.md`](PENDENTES.md) — backlog v2 DX + uniformização APIs
