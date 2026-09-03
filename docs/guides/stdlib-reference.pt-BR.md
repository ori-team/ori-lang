# Mapa de referência da biblioteca padrão

> **English:** [stdlib-reference.md](stdlib-reference.md)
> **Contratos normativos:** [spec/12-stdlib.md](../spec/12-stdlib.md)

A stdlib possui três camadas:

1. primitivas Layer 1 no runtime Rust;
2. wrappers seguros em `.orl`;
3. algoritmos escritos em Ori.

Prefira os módulos pais `ori.string`, `ori.list`, `ori.map` e equivalentes.
Paths antigos como `.utils` e `.algorithms` existem apenas por compatibilidade.

| Domínio | Módulos principais |
|---|---|
| I/O | `ori.io`, `ori.fs`, `ori.path` |
| Texto e bytes | `ori.string`, `ori.string_view`, `ori.bytes`, `ori.convert` |
| Collections e memória | `ori.list`, `ori.map`, `ori.set`, `ori.queue`, `ori.stack`, `ori.deque`, `ori.heap`, `ori.buffer`, `ori.slotmap`, `ori.span` |
| Gráficos e imagens | `ori.image` implementa geração de imagens BMP/PPM |
| Erros e diagnósticos | `ori.err_trace` (traces de erro e formatação) |
| Dados | `ori.json`, `ori.validate` |
| Tempo e aleatoriedade | `ori.time`, `ori.random`, `ori.format` |
| Processos | `ori.args`, `ori.config`, `ori.os`, `ori.process` |
| Rede | `ori.net` |
| Concorrência e async | `ori.task`, `ori.channel`, `ori.atomic`, `ori.concurrent`, `ori.cancel` |
| Segurança | `ori.crypto` |
| Testes | `ori.test` |
| Estruturas | `ori.graph`, `ori.tree` |

```ori
import ori.io = io
import ori.fs (read_text_or)

main()
    const text: string = read_text_or("notes.txt", "")
    io.println(text)
end
```

As assinaturas completas estão em [12-stdlib.md](../spec/12-stdlib.md). O site
usa os dados gerados por `ori doc export`; `ori doc check` valida docs inline e
sidecars `.oridoc`.

Os helpers assíncronos de filesystem, conexão e TLS usam um pool nativo
compartilhado e limitado (até quatro workers e 256 jobs na fila). Assim, uma
operação bloqueante não cria uma thread por requisição.

## Posições de texto e bytes

`string` armazena UTF-8 válido. `len(texto)`, `texto.len()`, slices, indexação,
`index_of`, `chars()` e iteração direta com `for` usam posições de valores
escalares Unicode. Essas APIs não expõem offsets de bytes UTF-8. Um grapheme
visível ainda pode conter vários escalares; use `bytes` e `string.to_bytes`
quando um protocolo exigir os bytes codificados. Segmentação de graphemes e
normalização ainda não fazem parte da stdlib.

Funções nativas são a referência semântica. `ori.io.read_line` retorna
`optional[string]`: `none` indica EOF ou entrada que não é UTF-8 válido.
