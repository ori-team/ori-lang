# Cap. 1 — O problema da leitura de código

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

Código é lido muitas vezes mais do que é escrito. Linguagens que escondem
contratos (tipos, falhas, donos de recurso) cobram pedágio cognitivo a cada
leitura — especialmente para quem já opera com memória de trabalho limitada.

## Exemplo

Dois contratos mentais. Só o da direita deixa a falha **visível na assinatura**:

| Opaco para o leitor | Legível na fonte |
|---------------------|------------------|
| `load(path) -> string` — falha? | `load(path: string) -> result[string, string]` |

```ori
module app.demo

import ori.fs = fs

load(path: string) -> result[string, string]
    return fs.read_text(path)
end
```

Sem `result`, o leitor precisa lembrar (ou adivinhar) se a função pode falhar.

## Como funciona

- **Leitura > digitação.** A Ori otimiza o custo de *entender*, não o de digitar menos caracteres.
- **Perguntas do leitor** devem ter resposta na fonte:
  - Onde este arquivo mora? → `module`
  - O valor pode faltar? → `optional[T]`
  - A operação pode falhar? → `result[T, E]`
  - Quando o recurso some? → `using`
- **Neurodivergência** (TDAH, dislexia, autismo, etc.) não é um “extra”: menos
  regras ocultas e cadeias de inferência longas reduz carga cognitiva para todos.
- Design de linguagem é também design de **acessibilidade cognitiva**.

## O que memorizar

- Um programa bom é um programa **fácil de reler**.
- Contrato visível no ponto de uso > magia conveniente.
- A Ori nasceu deste problema — não de uma disputa de market share.

## Ir mais fundo

- Manifesto: [`../../spec/00-manifesto.md`](../../spec/00-manifesto.md)
- Cap. 3 — finalidade e anti-objetivos
