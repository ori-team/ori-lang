# Documentação Ori

> **Superfície:** S3 (`0.3.0`) · inferência opção B (`0.3.1`) · workspace `0.3.8-dev`
> **Idiomas:** **inglês é o primário no GitHub** · **português é mantido em paralelo**  
> **Status:** documentação viva — deve refletir o compilador, não desenhos aspiracionais

Índice em inglês: [README.md](README.md).

| Público | Comece aqui |
|---------|-------------|
| **Usuário novo** | [Instalação](install.pt-BR.md) → [Tour da linguagem](language/tour.pt-BR.md) → [Primeiro projeto](guides/first-project.pt-BR.md) |
| **Livro (rascunho)** | **[Livro Ori](book/README.md)** (PDF/HTML não versionados) |
| **Uso diário** | [Cookbook](guides/cookbook.pt-BR.md) · [Erros / optional / result](guides/errors-null-void.pt-BR.md) · [Exemplos](../examples/) |
| **Performance** | [Microbench Ori / Python / Rust](guides/performance.pt-BR.md) · [EN](guides/performance.md) |
| **Contrato da linguagem** | [Especificação](spec/README.md) (normativa, **em inglês**) |
| **Recursos avançados** | [Tipos](language/advanced.pt-BR.md) · [Async](language/concurrency.pt-BR.md) · [ABI C](language/interop.pt-BR.md) |
| **Mantenedores** | **[BACKLOG](planning/BACKLOG.md)** · [Planejamento](planning/README.md) · [AGENTS.md](../AGENTS.md) |

## Política de idioma

| Classe | Idioma | Notas |
|--------|--------|--------|
| Superfície GitHub (README, guias, install) | **EN canônico** + **irmão `.pt-BR.md`** | Mesma estrutura e versão |
| Spec normativa (`docs/spec/`) | **Inglês** | Não duplicar capítulos em PT |
| Livro (`docs/book/`) | **Português** na prosa; código S3 | Rascunho narrativo/consulta; **não** substitui `spec/` |
| Planejamento | PT ou EN (idioma do arquivo) | Não é tutorial de usuário |
| Histórico | como escrito | Não ensinar como superfície atual |

Exemplos de usuário: **S3 válido**. Layout de projeto: **`ori.proj` + `main.orl` na raiz**. Stdlib canônica: `ori.X` (não ensinar `.utils` como API nova).

## Mapa de diretórios

Ver [README.md](README.md) (mesmo mapa; nomes de arquivo EN).

## Superfície atual (resumo)

| Tema | Forma canônica |
|------|----------------|
| Cabeçalho | `module app.main` |
| Função | `name(params) -> T` — **sem** `func` |
| Tipos | `list[T]`, `result[T, E]`, … com `[]` |
| Result | `ok` / `err` |
| Propagação | só `try expr` |
| Imports | `import path = alias` (path à esquerda) |
| Traits | `import ori.core = core` · `apply` + `use core.Displayable` |
| Pipe | `|>` mantido |
| Inferência local | opção B |
| Async | `async main()` + `await` (nativo); C/debug rejeita async |
| Projeto | raiz-first |

Contrato completo: [spec/01-overview.md](spec/README.md). Migrar pré-S3: `ori migrate-syntax`.
