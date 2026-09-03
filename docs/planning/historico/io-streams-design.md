# I/O streams design

> Status: implementado (MVP Layer 1, 2026-07-01)
> Data: 2026-07-01

Ori ja possui I/O basico por funcoes como `io.print`, `io.read_line` e APIs de
arquivo em `ori.fs`. O proximo passo nao deve ser adicionar tipos opacos sem
contrato. Streams precisam nascer com ownership claro.

## Objetivo

Adicionar leitura e escrita incremental sem esconder falhas:

```ori
alias ReadResult = result<bytes, string>
alias WriteResult = result<int, string>

-- nomes finais ainda podem mudar
io.Input
io.Output
```

## Contrato recomendado

- `io.Input` representa uma fonte de bytes.
- `io.Output` representa um destino de bytes.
- Ambos sao recursos managed pelo runtime e devem implementar `Disposable`.
- Toda operacao que pode falhar retorna `result`.
- EOF deve ser `optional<bytes>` ou `result<optional<bytes>, string>`, nunca
  string magica.
- Tipos managed nao devem atravessar FFI sem wrapper explicito.

## API inicial proposta

```ori
io.stdin() -> io.Input
io.stdout() -> io.Output
io.stderr() -> io.Output

io.read(input: io.Input, max_bytes: int) -> result<optional<bytes>, string>
io.read_text(input: io.Input, max_chars: int) -> result<optional<string>, string>
io.write(output: io.Output, data: bytes) -> result<int, string>
io.write_text(output: io.Output, text: string) -> result<int, string>
io.flush(output: io.Output) -> result<void, string>
```

## Fora do recorte inicial

- sockets async (rede e streams);
- backpressure avancado;
- pipelines lazy;
- adapters complexos de arquivo.

TLS/UDP/servidor TCP vivem em `ori.net` (`docs/planning/net-v2-design.md`), não
em `io.Input`/`Output`. Esses itens de streams só devem entrar depois que
`Input`/`Output` tiverem testes de arquivo, stdin/stdout e cleanup com `using`.
