# Cap. 4 — Mapa mental do ecossistema

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

O repositório é um monólito organizado: compilador Rust, runtime, stdlib `.orl`,
spec, exemplos, extensões de editor e ferramentas de release/QA. Cada pasta tem
um dono claro.

## Exemplo

Árvore mínima para se orientar:

```text
ori-lang/
├── compiler/crates/   # ori-lexer … ori-driver, ori-lsp, ori-runtime
├── stdlib/            # módulos .orl (Layer 2/3)
├── runtime/           # libs empacotadas por triple
├── docs/spec/         # contrato normativo (EN)
├── docs/book/         # este livro (PT)
├── examples/          # programas .orl
├── extensions/        # VS Code + Zed (local)
└── tools/             # stage, smoke, QA, migrate
```

## Como funciona

| Pasta | Papel |
|-------|--------|
| `compiler/` | Workspace Cargo do compilador e do runtime em Rust |
| `stdlib/` | Fonte Ori da biblioteca padrão |
| `runtime/<triple>/` | Artefatos nativos para link/JIT |
| `docs/spec/` | Spec normativa |
| `docs/guides/` + `docs/language/` | Guias e tour (EN + `.pt-BR.md`) |
| `examples/` | Amostras rodáveis |
| `extensions/` | LSP clients locais |
| `tests/` | Programas de teste E2E em Ori |

Fluxo do desenvolvedor da linguagem (resumo):

1. Mudar crate ou `.orl` da stdlib  
2. Testar (`cargo test`, `ori check` / exemplos)  
3. Atualizar spec/CHANGELOG quando for user-visible  
4. Re-estagiar runtime se a ABI/FFI mudou  

## O que memorizar

- Compilador em `compiler/`; stdlib em `stdlib/`; verdade normativa em `docs/spec/`.
- Este livro em `docs/book/` **não** substitui a spec.
- Exemplos vivos em `examples/` — use-os nos capítulos.

## Ir mais fundo

- [`../../../AGENTS.md`](../../../AGENTS.md) — contexto para agentes e mantenedores
- Apêndice B — mapa do repositório
- Cap. 8 — ferramentas do dia a dia
