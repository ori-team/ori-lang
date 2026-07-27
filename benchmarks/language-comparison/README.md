# Comparacao de linguagens

> **Comparação viva (2026-07-13+):** Ori AOT vs CPython vs Rust release —
> [`tools/bench/polyglot/`](../../tools/bench/polyglot/) ·
> docs [performance.md](../../docs/guides/performance.md) /
> [performance.pt-BR.md](../../docs/guides/performance.pt-BR.md).
>
> Este diretório é a **suite PowerShell alternativa** (também C e Node).
> Números antigos em [docs/guides/language-comparison.md](../../docs/guides/language-comparison.md)
> são históricos.

Este diretorio contem workloads equivalentes em Ori, Rust, C, Python e Node.js.

Cada arquivo implementa as mesmas funcoes:

- `fib`
- `fib_work`
- `sum_squares`
- `list_push_sum`

Entradas fixas:

| Workload | Entrada |
| --- | ---: |
| `fib_work` | `fib(32)` repetido `80000` vezes |
| `sum_squares` | `1..200000` |
| `list_push_sum` | `80000` pushes e soma |

Saida esperada:

```text
fib_acc=174264720000
sum_squares=2666686666700000
list_push_sum=9600440000
score=2666870531860000
```

Rode a comparacao com:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 5
```

Os resultados ficam em:

```text
target/language-comparison/
```

O TXT de resumo inclui versoes das ferramentas, melhor tempo, tempo medio e validade da saida.

## Limites da comparacao

- O benchmark mede processo externo, entao inclui startup do runtime.
- Ori usa o backend nativo atual do projeto.
- Rust e C sao compilados com otimizacao local (`rustc -C opt-level=3`, `gcc -O2`).
- Python e Node rodam nos interpretadores instalados localmente.
- O resultado mede performance destes workloads, nao maturidade geral de cada linguagem.

Relatorio completo:

```text
docs/guides/language-comparison.md
```
