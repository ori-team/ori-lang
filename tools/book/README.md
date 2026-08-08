# Ori book → PDF

Gera o PDF do livro em [`docs/book/`](../../docs/book/) com **syntax highlight**
(highlight.js + definição customizada da linguagem Ori).

## Requisitos

- Node.js 20+
- Chrome gerenciado pelo Puppeteer (`npx puppeteer browsers install chrome`)
  - ou Chrome/Chromium do sistema via `ORI_BOOK_CHROME=/caminho/do/chrome`

## Uso

```bash
# na raiz do repo
npm --prefix tools/book install
npx --prefix tools/book puppeteer browsers install chrome
npm --prefix tools/book run pdf
```

Saída (artefatos locais, não versionados):

- `docs/book/dist/ori-livro.pdf`
- `docs/book/dist/ori-livro.html`

## Highlight

| Fence | Motor |
|-------|--------|
| ` ```ori ` / ` ```orl ` | [`ori-hljs.mjs`](ori-hljs.mjs) — keywords S3, `--` comentários, `f"…"`, `|>` |
| `bash`, `text`, … | highlight.js padrão |

Cores pensadas para impressão (fundo claro, keywords verdes, strings azuis).
