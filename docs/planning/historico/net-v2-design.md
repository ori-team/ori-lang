# Rede v2 — TLS, servidor TCP, UDP e I/O blocking

> Status: implementado em `[Unreleased]` (2026-07-01)  
> Audiência: implementadores da stdlib, autores de programas Ori

---

## Decisões fechadas

| Tópico | Decisão |
|--------|---------|
| **TLS** | Opção A — `net.connect_tls(host, port, timeout_ms) -> result<Connection, string>` separado de `net.connect`; mesmo tipo opaco `ori.net.Connection` (transporte Plain ou TLS internamente). |
| **UDP** | Entregue na mesma leva que TLS (datagramas síncronos). |
| **Async de rede** | APIs permanecem **blocking síncronas** no runtime Layer 1. Para não bloquear o executor async, usar `task.run_blocking` (alias de `task.spawn` para closures sem captura pesada) ou helpers em `ori.net.utils` (`connect_in_background`, `connect_tls_in_background`). I/O nativo `*_async` fica para backlog longo prazo. |
| **Servidor TCP** | `listen` / `accept` / `close_listener` + `listener_port` para porta efetiva após bind em `:0`. |
| **Exemplo** | `examples/http_get.orl` — GET HTTPS mínimo via `connect_tls`. |

---

## Superfície Layer 1 (manifesto + runtime)

Tipos opacos:

- `ori.net.Connection` — stream TCP ou TLS
- `ori.net.Listener` — `TcpListener` bound
- `ori.net.UdpSocket` — socket UDP bound

Funções runtime (`compiler/crates/ori-runtime/src/lib.rs`):

| Função | Descrição |
|--------|-----------|
| `net.connect` | TCP cliente com timeout |
| `net.connect_tls` | TCP + handshake TLS (rustls + webpki-roots) |
| `net.listen` / `net.accept` / `net.close_listener` | Servidor TCP |
| `net.listener_port` | Porta local após bind (`0` → ephemeral) |
| `net.read_some` / `net.write_all` / `net.close` / `net.is_closed` | I/O em `Connection` |
| `net.udp_bind` / `net.udp_send_to` / `net.udp_recv_from` / `net.udp_close` | UDP |
| `net.udp_local_port` | Porta local do socket UDP |

`task.run_blocking` — alias documentado de `task.spawn` para offload de I/O blocking.

---

## Layer 2 (`.orl`)

- `stdlib/net.orl` — flatten de símbolos Layer 1 para `import ori.net only (...)`
- `stdlib/net/utils.orl` — helpers (`read_text`, `write_text`, `listen_local`, `connect_*_in_background`, …)

---

## Limitações conhecidas

1. **Sem SNI customizado / certificados cliente** — apenas TLS cliente com trust store padrão (webpki-roots).
2. **Sem UDP multicast / IPv6 dedicado** — host string + porta; IPv6 funciona se o SO resolver.
3. **C backend** — rede não portada; símbolos marcados sem `c_backend`.
4. **Testes TLS reais** — E2E usa porta recusada; smoke manual via `examples/http_get.orl` (rede externa).

---

## Testes de regressão

- `compile_runs_net_tcp_listen_accept_loopback`
- `compile_runs_net_udp_loopback`
- `compile_runs_net_connect_tls_reports_error_on_refused_port`
- `check_accepts_net_v2_flatten_selective_imports`

---

## Próximos passos (backlog)

- UDP avançado (multicast, `connect` UDP)
- TLS: pinning, certificados customizados
- `net.*_async` integrado ao executor quando houver epoll/io_uring wrapper estável
