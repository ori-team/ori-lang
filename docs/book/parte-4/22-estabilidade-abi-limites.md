# Cap. 22 — Estabilidade, ABI e limites

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR

Ori está em **0.3.x** (pré-1.0) com FREEZE-1: mudança ainda possível, com
disciplina. ABI nativa documentada como `ori-native-abi-1`. Self-hosting (M4)
não é o próximo passo tático.

## Estabilidade

| Tema | Estado |
|------|--------|
| Superfície S3 | Canônica desde `0.3.0`; pré-S3 rejeitado |
| Inferência B | Desde `0.3.1` |
| Package / install sem Rust | M1 fechado |
| ABI documentada | M3 — [`../../spec/19-abi.md`](../../spec/19-abi.md) |
| Self-host | M4 — última discussão |
| 1.0 | Critério de maturidade (anos), não de marketing |

Ver: [`../../spec/18-stability-and-compatibility.md`](../../spec/18-stability-and-compatibility.md).

## Limites e pitfalls (mantenedor / usuário avançado)

1. Runtime desatualizado → `native.link_failed` — re-estagiar staticlib **e** cdylib.  
2. Cache `OnceLock` de path de runtime em testes — primeiro resultado “gruda”.  
3. Windows: `ori-lsp.exe` pode travar rebuild — encerrar o processo.  
4. Backend C: paridade parcial; async é caminho nativo.  
5. Rede/TLS: precisa de ambiente; exemplos como `http_get` não são unitários puros.

## O que memorizar

- Livro âncora = 0.3.x; leia o CHANGELOG ao atualizar.  
- ABI ≠ estabilidade de sintaxe 1.0.  
- M4 self-host não bloqueia uso do lab hoje.

## Ir mais fundo

- ABI: [`../../spec/19-abi.md`](../../spec/19-abi.md)
- Backend: [`../../spec/14-backend-support.md`](../../spec/14-backend-support.md)
- Backlog: [`../../planning/BACKLOG.md`](../../planning/BACKLOG.md)
