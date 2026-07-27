> Notas de medição local (PT). Benchmarks em `benchmarks/language-comparison/`.  
> Índice de docs: [../README.md](../README.md) · [../README.pt-BR.md](../README.pt-BR.md).

# Comparacao da Ori com outras linguagens

> **Atualização 2026-07-13:** a comparação **viva** de runtime (Ori AOT vs
> CPython vs Rust release, com timer de alta resolução e ressalvas honestas)
> está em:
>
> - **[performance.md](performance.md)** (EN) · **[performance.pt-BR.md](performance.pt-BR.md)** (PT)
> - Harness: [`tools/bench/polyglot/`](../../tools/bench/polyglot/)
> - Snapshot no README: [../../README.md#performance-snapshot](../../README.md#performance-snapshot)
>
> **Nesta página:** suite mais antiga (Ori / Rust / C / Node / Python via
> PowerShell). Os números abaixo **não** substituem o polyglot atual — em
> especial, medições antigas em que Python aparecia “mais rápido que Ori” foram
> superadas no polyglot 2026-07-13 (Ori ~4–46× à frente do CPython nesses
> kernels). Mantenha este arquivo como **histórico / suite alternativa**.

Este guia compara a Ori com outras linguagens usando funcoes e entradas equivalentes.

Status: **histórico** — medicao local de referencia (suite PowerShell).

## Resumo rapido

Resultado da medicao local com 5 iteracoes (**histórico**, ~2026-06-30).

Execucao de referencia local, gerada pelo runner e nao versionada:

```text
target/language-comparison/language-comparison-20260630-223327.csv
```

Ao rodar o runner novamente, um novo arquivo com outro timestamp sera criado.

| Linguagem | Build | Melhor execucao | Media de execucao | Execucoes validas | Relacao com Ori |
| --- | ---: | ---: | ---: | ---: | ---: |
| C | `315.423 ms` | `12.076 ms` | `14.429 ms` | 5/5 | `0.013x` |
| Rust | `464.569 ms` | `16.927 ms` | `18.951 ms` | 5/5 | `0.019x` |
| Node.js | `0 ms` | `97.397 ms` | `105.388 ms` | 5/5 | `0.109x` |
| Python | `0 ms` | `339.105 ms` | `350.300 ms` | 5/5 | `0.378x` |
| Ori | `1204.405 ms` | `897.555 ms` | `996.356 ms` | 5/5 | `1x` |

Leitura correta:

- Rust e C ficaram muito a frente neste workload numerico simples.
- Node.js ficou mais rapido que Ori neste teste, mesmo incluindo startup do processo.
- Python ficou mais rapido que Ori neste teste local.
- Ori ainda esta em fase pre-1.0; estes numeros mostram onde otimizar o backend e a runtime.

## O que foi comparado

Os arquivos ficam em:

```text
benchmarks/language-comparison/
```

Cada linguagem implementa as mesmas funcoes:

| Funcao | O que mede |
| --- | --- |
| `fib` | Loop numerico pequeno e previsivel. |
| `fib_work` | Repeticao de chamada de funcao. |
| `sum_squares` | Loop numerico longo com acumulador inteiro. |
| `list_push_sum` | Criacao de lista/vetor, insercao e leitura sequencial. |

Entradas fixas:

| Workload | Entrada |
| --- | ---: |
| `fib_work` | `fib(32)` repetido `80000` vezes |
| `sum_squares` | `1..200000` |
| `list_push_sum` | `80000` insercoes e soma |

Todas as linguagens devem imprimir exatamente:

```text
fib_acc=174264720000
sum_squares=2666686666700000
list_push_sum=9600440000
score=2666870531860000
```

Se a saida nao bater, a execucao nao entra na comparacao.

## Como executar

Na raiz do repositorio:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 5
```

Os arquivos gerados ficam em:

```text
target/language-comparison/
```

O runner salva:

- CSV com todas as execucoes;
- TXT com resumo legivel;
- versoes das ferramentas usadas;
- stdout/stderr de cada comando.

## Ambiente usado na medicao

Runtimes encontrados localmente:

| Linguagem | Ferramenta |
| --- | --- |
| Ori | `cargo 1.95.0` via `cargo run -p ori-driver -- compile` |
| Rust | `rustc 1.95.0` |
| C | `gcc 15.2.0` |
| Node.js | `node v25.2.1` |
| Python | `python 3.12.0` |

Go e Zig nao estavam disponiveis neste ambiente durante a medicao.

## Limites da comparacao

Esta comparacao e util, mas nao e um ranking final de linguagens.

Limites principais:

- o tempo de execucao inclui startup do processo;
- Ori usa o backend nativo atual do projeto, ainda pre-1.0;
- Rust usa `rustc -C opt-level=3`;
- C usa `gcc -O2 -std=c11`;
- Python e Node.js rodam nos runtimes instalados localmente;
- Rust e C usam prealocacao no workload de vetor/lista;
- Ori usa a API atual de lista disponivel na linguagem;
- o teste mede somente estes workloads, nao IO, async, LSP, pacotes, diagnosticos ou ergonomia.

## Comparacao de seguranca

Performance e seguranca nao medem a mesma coisa.

Resumo pratico:

| Linguagem | Perfil de seguranca neste contexto |
| --- | --- |
| Ori | Tipagem explicita, diagnosticos estaveis, testes de robustez, ARC e leak-check no runtime. Ainda precisa de mais maturidade e auditoria pre-1.0. |
| Rust | Forte seguranca de memoria por padrao, checagens em tempo de compilacao e escape hatch via `unsafe`. Melhor referencia do grupo para memoria segura. |
| C | Muito rapido, mas sem seguranca de memoria por padrao. Buffer overflow, use-after-free e double-free dependem de disciplina, sanitizers e revisao. |
| Node.js | Memoria gerenciada e bom isolamento do runtime, mas tipagem dinamica em JavaScript puro empurra muitos erros para runtime. |
| Python | Memoria gerenciada e boa seguranca contra erros manuais de ponteiro, mas tipagem dinamica tambem empurra muitos erros para runtime. |

Para a Ori, a suite de seguranca atual cobre:

- entradas malformadas sem panic;
- spans de diagnostico validos;
- escaping de HTML em `ori doc`;
- catalogo de diagnosticos;
- leak-check em cenarios do runtime;
- testes de ARC, async e concorrencia.

## Como interpretar o resultado da Ori

O resultado atual nao significa que a linguagem esta "lenta para sempre".

Ele mostra gargalos provaveis:

- custo do backend nativo atual;
- custo da runtime de listas e strings;
- falta de otimizacoes equivalentes as de Rust/C;
- possivel custo de startup e inicializacao.

Use esta comparacao como linha de base.

Quando uma otimizacao for feita, rode o mesmo comando de novo e compare:

```powershell
.\tools\compare_language_workloads.ps1 -Iterations 10
```

Compare sempre:

- `output_ok=True`;
- melhor execucao;
- media de execucao;
- variacao entre iteracoes;
- mudancas no tempo de build da Ori.
