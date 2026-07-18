# Cap. 20 — Stdlib: índice de consulta

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR

Importe pais `ori.X`. Abaixo: mapa de um olhada. Detalhe normativo na spec 12;
fontes em `stdlib/*.orl` + Layer 1 no runtime.

## Índice (propósito em uma linha)

| Módulo | Propósito |
|--------|-----------|
| `ori.io` | Print / eprint / streams básicos |
| `ori.fs` | Arquivos e diretórios |
| `ori.path` | Manipulação de caminhos |
| `ori.string` | Texto |
| `ori.bytes` | Binário |
| `ori.list` / `ori.map` / `ori.set` | Coleções |
| `ori.iter` | Iteração |
| `ori.math` | Matemática |
| `ori.time` | Tempo |
| `ori.os` | SO / processo ambiente |
| `ori.process` | Subprocessos |
| `ori.args` | Argumentos de CLI |
| `ori.net` | TCP/UDP/TLS (rede) |
| `ori.json` | JSON |
| `ori.log` | Logging |
| `ori.random` | Aleatório |
| `ori.test` | Asserções / helpers de teste |
| `ori.core` | Traits e núcleo (`Displayable`, …) |
| `ori.format` | Formatação |
| `ori.convert` | Conversões |
| `ori.config` | Config |
| `ori.validate` | Validação |
| `ori.crypto` | Cripto (quando disponível) |
| `ori.concurrent` / filas / heaps / graphs | Estruturas e concorrência |

Tipos fundamentais (`optional`, `result`, `list`, …) existem sem import.

## Import canônico

```ori
import ori.io = io
import ori.fs = fs
import ori.string = str
```

Evite ensinar `ori.*.utils` / `ori.*.algorithms` como API nova.

## Ir mais fundo

- Spec: [`../../spec/12-stdlib.md`](../../spec/12-stdlib.md)
- [`../../../stdlib/README.md`](../../../stdlib/README.md)
- Cap. 17 — uso prático
