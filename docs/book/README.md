# Livro Ori — laboratório de linguagem

> **Versão do livro:** `0.3.x-book.2`  
> **Âncora da linguagem:** Ori **0.3.x** (superfície S3 + inferência opção B)  
> **Idioma:** português na prosa · código em sintaxe S3 (inglês de API)  
> **Status:** rascunho vivo — explica e aponta; a verdade normativa fica em [`../spec/`](../spec/)  
> **Revisão:** snippets principais validados com `ori check` (hello, result, optional/`try`,
> fluxo/pipe, struct, traits, FS, `@test`, roteiro 30 min; multiarquivo exige `ori.proj`)

## PDF

- **Arquivo:** [`dist/ori-livro.pdf`](dist/ori-livro.pdf) (A4, syntax highlight Ori/bash/text)
- **Pré-visualização HTML:** [`dist/ori-livro.html`](dist/ori-livro.html)

Regenerar:

```bash
npm --prefix tools/book install
npx --prefix tools/book puppeteer browsers install chrome   # uma vez
npm --prefix tools/book run pdf
```

Detalhes: [`tools/book/README.md`](../../tools/book/README.md).

## Promessa

Entender **por que** a Ori existe, **como** foi construída e **como** escrever
programas pequenos e corretos nela.

## Anti-promessa

Não compete com o mercado de linguagens. Não ensina self-hosting como atalho.
Não revive sintaxe pré-S3.

## Como ler

| Se você quer… | Comece em |
|---------------|-----------|
| Ideia e propósito | [Parte I](parte-1/) |
| Como o compilador e o time trabalham | [Parte II](parte-2/) |
| Aprender a programar | [Parte III](parte-3/) |
| Consulta rápida | [Parte IV](parte-4/) · [Cheat sheet](apendices/A-cheat-sheet.md) |

Cada capítulo segue o [template](TEMPLATE.md): TL;DR → exemplo → explicação →
o que memorizar → links.

## Índice

### Parte I — Identidade e finalidade

1. [O problema da leitura de código](parte-1/01-problema-leitura.md)
2. [O que é Ori](parte-1/02-o-que-e-ori.md)
3. [Para que existe (e para que não)](parte-1/03-para-que-existe.md)
4. [Mapa mental do ecossistema](parte-1/04-mapa-ecossistema.md)

### Parte II — Processo de desenvolvimento

5. [Da ideia à superfície S3](parte-2/05-da-ideia-ao-s3.md)
6. [Pipeline do compilador](parte-2/06-pipeline-compilador.md)
7. [Decisões de design](parte-2/07-decisoes-de-design.md)
8. [Ferramentas do dia a dia](parte-2/08-ferramentas.md)
9. [Desenvolver com assistência de IA](parte-2/09-ia-no-desenvolvimento.md)

### Parte III — Aprender Ori na prática

10. [Instalar e verificar o ambiente](parte-3/10-instalar.md)
11. [Anatomia de um programa](parte-3/11-anatomia-programa.md)
12. [Tipos, ausência e falha](parte-3/12-tipos-ausencia-falha.md)
13. [Expressões, fluxo e pipe](parte-3/13-expressoes-fluxo-pipe.md)
14. [Structs, enums e funções](parte-3/14-structs-enums-funcoes.md)
15. [Módulos, imports e projetos](parte-3/15-modulos-projetos.md)
16. [Traits e comportamento explícito](parte-3/16-traits.md)
17. [Stdlib, I/O, async e testes](parte-3/17-stdlib-io-async-testes.md)

### Parte IV — Manual de consulta

18. [Cheat sheet de sintaxe S3](parte-4/18-cheat-sheet-sintaxe.md) · espelho: [Apêndice A](apendices/A-cheat-sheet.md)
19. [CLI e variáveis de ambiente](parte-4/19-cli-e-env.md)
20. [Stdlib: índice de consulta](parte-4/20-stdlib-indice.md)
21. [Diagnostics mais comuns](parte-4/21-diagnostics.md)
22. [Estabilidade, ABI e limites](parte-4/22-estabilidade-abi-limites.md)

### Apêndices

| | |
|--|--|
| [A — Cheat sheet](apendices/A-cheat-sheet.md) | [B — Mapa do repositório](apendices/B-mapa-repositorio.md) |
| [C — Exercícios](apendices/C-exercicios.md) | [D — Programa em 30 minutos](apendices/D-programa-30-minutos.md) |
| [E — Bibliografia](apendices/E-bibliografia.md) | [Glossário](apendices/glossario.md) |

Exemplos canônicos do repo: [exemplos-canonicos.md](exemplos-canonicos.md).

## Relação com o resto da documentação

| Documento | Papel |
|-----------|--------|
| Este livro | Narrativa + ensino + consulta em PT |
| [`../spec/`](../spec/) | Contrato normativo (EN) |
| [`../language/tour.pt-BR.md`](../language/tour.pt-BR.md) | Tour curto |
| [`../guides/`](../guides/) | How-tos pontuais |
| [`../../CHANGELOG.md`](../../CHANGELOG.md) | O que mudou entre versões |

Em conflito de sintaxe, prevalecem a spec e o compilador — não este rascunho.

## Manutenção

- Mudança user-visible na linguagem → atualizar o capítulo afetado + Cap. 18.
- Snippets devem permanecer S3 válidos (`ori check`).
- Versão do livro: `0.3.x-book.N` neste README (independente do semver do Cargo).
