#!/usr/bin/env node
/**
 * Assemble docs/book Markdown → HTML (highlight.js + Ori) → PDF via Chromium.
 *
 * Usage:
 *   npm --prefix tools/book install
 *   npm --prefix tools/book run pdf
 */
import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { marked } from "marked";
import { markedHighlight } from "marked-highlight";
import hljs from "highlight.js";
import puppeteer from "puppeteer";
import { registerOri } from "./ori-hljs.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../..");
const BOOK_ROOT = join(REPO_ROOT, "docs/book");
const OUT_DIR = join(BOOK_ROOT, "dist");
const OUT_PDF = join(OUT_DIR, "ori-livro.pdf");
const OUT_HTML = join(OUT_DIR, "ori-livro.html");

const CHAPTERS = [
  "parte-1/01-problema-leitura.md",
  "parte-1/02-o-que-e-ori.md",
  "parte-1/03-para-que-existe.md",
  "parte-1/04-mapa-ecossistema.md",
  "parte-2/05-da-ideia-ao-s3.md",
  "parte-2/06-pipeline-compilador.md",
  "parte-2/07-decisoes-de-design.md",
  "parte-2/08-ferramentas.md",
  "parte-2/09-ia-no-desenvolvimento.md",
  "parte-3/10-instalar.md",
  "parte-3/11-anatomia-programa.md",
  "parte-3/12-tipos-ausencia-falha.md",
  "parte-3/13-expressoes-fluxo-pipe.md",
  "parte-3/14-structs-enums-funcoes.md",
  "parte-3/15-modulos-projetos.md",
  "parte-3/16-traits.md",
  "parte-3/17-stdlib-io-async-testes.md",
  "parte-4/18-cheat-sheet-sintaxe.md",
  "parte-4/19-cli-e-env.md",
  "parte-4/20-stdlib-indice.md",
  "parte-4/21-diagnostics.md",
  "parte-4/22-estabilidade-abi-limites.md",
  "apendices/A-cheat-sheet.md",
  "apendices/B-mapa-repositorio.md",
  "apendices/C-exercicios.md",
  "apendices/D-programa-30-minutos.md",
  "apendices/E-bibliografia.md",
  "apendices/glossario.md",
  "exemplos-canonicos.md",
  "exercicios/README.md",
];

const CHROME_CANDIDATES = [
  process.env.ORI_BOOK_CHROME,
  // puppeteer-managed Chrome (after `npx puppeteer browsers install chrome`)
  (() => {
    try {
      return puppeteer.executablePath();
    } catch {
      return null;
    }
  })(),
  "/usr/bin/chromium-browser",
  "/usr/bin/chromium",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/google-chrome",
  "/snap/bin/chromium",
].filter(Boolean);

hljs.registerLanguage("ori", registerOri);
hljs.registerLanguage("orl", registerOri);

marked.use(
  markedHighlight({
    langPrefix: "hljs language-",
    highlight(code, lang) {
      const language = (lang || "").toLowerCase() || "plaintext";
      if (language === "ori" || language === "orl") {
        return hljs.highlight(code, { language: "ori" }).value;
      }
      if (hljs.getLanguage(language)) {
        return hljs.highlight(code, { language }).value;
      }
      return hljs.highlightAuto(code).value;
    },
  }),
);

marked.setOptions({
  gfm: true,
  breaks: false,
});

function rewriteRepoLinks(markdown) {
  return markdown.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (full, label, href) => {
    if (
      href.startsWith("http://") ||
      href.startsWith("https://") ||
      href.startsWith("#")
    ) {
      return full;
    }
    return `**${label}** (\`${href}\`)`;
  });
}

function titlePageHtml(bookVersion) {
  return `
<section class="titlepage chapter">
  <h1>Ori</h1>
  <p class="meta">Livro-laboratório — ideia, processo, prática e consulta</p>
  <p class="meta">Versão do livro: <strong>${bookVersion}</strong></p>
  <p class="meta">Âncora da linguagem: Ori 0.3.x (superfície S3)</p>
  <p class="meta">Rascunho vivo · não substitui a spec normativa</p>
</section>`;
}

function tocHtml() {
  const items = CHAPTERS.map((rel) => {
    const name = rel.replace(/\.md$/, "").split("/").pop();
    return `<li>${name}</li>`;
  }).join("\n");
  return `
<section class="toc chapter">
  <h1>Índice (ordem do PDF)</h1>
  <ul>${items}</ul>
</section>`;
}

async function readBookVersion() {
  const readme = await readFile(join(BOOK_ROOT, "README.md"), "utf8");
  const m = readme.match(/0\.3\.x-book\.\d+/);
  return m ? m[0] : "0.3.x-book";
}

async function findChrome() {
  for (const candidate of CHROME_CANDIDATES) {
    try {
      await access(candidate, constants.X_OK);
      return candidate;
    } catch {
      // try next
    }
  }
  throw new Error(
    "Chromium/Chrome não encontrado. Defina ORI_BOOK_CHROME=/caminho/do/chrome",
  );
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  const bookVersion = await readBookVersion();
  const css = await readFile(join(__dirname, "book.css"), "utf8");

  const chapterSections = [];
  for (const rel of CHAPTERS) {
    const raw = await readFile(join(BOOK_ROOT, rel), "utf8");
    const html = marked.parse(rewriteRepoLinks(raw));
    chapterSections.push(
      `<section class="chapter" data-source="${rel}">\n${html}\n</section>`,
    );
  }

  const documentHtml = `<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="utf-8" />
  <title>Ori — Livro ${bookVersion}</title>
  <style>${css}</style>
</head>
<body>
${titlePageHtml(bookVersion)}
${tocHtml()}
${chapterSections.join("\n")}
</body>
</html>`;

  await writeFile(OUT_HTML, documentHtml, "utf8");

  const executablePath = await findChrome();
  console.log(`Chrome: ${executablePath}`);

  const browser = await puppeteer.launch({
    executablePath,
    headless: true,
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--disable-dev-shm-usage",
      "--font-render-hinting=none",
    ],
  });

  try {
    const page = await browser.newPage();
    await page.goto(`file://${OUT_HTML}`, { waitUntil: "networkidle0" });
    await page.pdf({
      path: OUT_PDF,
      format: "A4",
      printBackground: true,
      displayHeaderFooter: true,
      headerTemplate: "<div></div>",
      footerTemplate: `
        <div style="font-size:9px;width:100%;text-align:center;color:#666;font-family:sans-serif;">
          Ori — Livro ${bookVersion} — <span class="pageNumber"></span> / <span class="totalPages"></span>
        </div>`,
      margin: { top: "16mm", bottom: "18mm", left: "14mm", right: "14mm" },
    });
  } finally {
    await browser.close();
  }

  console.log(`PDF: ${OUT_PDF}`);
  console.log(`HTML: ${OUT_HTML}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
