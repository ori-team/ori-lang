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
| Texto e bytes | `ori.string`, `ori.bytes`, `ori.convert` |
| Collections | `ori.list`, `ori.map`, `ori.set`, `ori.queue`, `ori.stack`, `ori.deque`, `ori.heap` |
| Dados | `ori.json`, `ori.validate` |
| Tempo e aleatoriedade | `ori.time`, `ori.random`, `ori.format` |
| Processos | `ori.args`, `ori.config`, `ori.os`, `ori.process` |
| Rede | `ori.net` |
| Concorrência | `ori.task`, `ori.channel`, `ori.atomic`, `ori.concurrent` |
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
