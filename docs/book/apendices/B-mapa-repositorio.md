# Apêndice B — Mapa do repositório

> **Versão âncora:** Ori 0.3.x (S3)

## TL;DR
Uma visão panorâmica de como as pastas do código-fonte do compilador Ori e da linguagem estão organizadas, para ajudar novos contribuidores. Todo nome abaixo foi conferido direto no repositório.

---

Se você decidiu contribuir para o desenvolvimento da linguagem, abrir o repositório pela primeira vez pode ser assustador. Aqui está o mapa das pastas principais e o que cada uma faz.

```text
ori-lang/
├── compiler/               -- O coração do projeto: o compilador em Rust
│   └── crates/             -- Cada etapa do pipeline é um "crate" (pacote Rust) separado
│       ├── ori-lexer/      -- Texto → tokens
│       ├── ori-parser/     -- Tokens → AST (árvore sintática)
│       ├── ori-ast/        -- As estruturas de dados da AST
│       ├── ori-types/      -- Type checker: resolve nomes, confere tipos
│       ├── ori-hir/        -- AST tipada → HIR + monomorfização + otimizações
│       ├── ori-codegen/    -- HIR → código de máquina (Cranelift) ou C
│       ├── ori-runtime/    -- O runtime nativo: ARC, coletor de ciclos, coleções
│       ├── ori-diagnostics/-- Formatação e catálogo dos erros/avisos
│       ├── ori-driver/     -- Junta tudo: é o binário `ori` (CLI) em si
│       └── ori-lsp/        -- O Language Server (integração com editores)
│
├── stdlib/                 -- A Biblioteca Padrão (código `.orl`, ~34 módulos)
│   ├── io.orl, fs.orl, string.orl, list.orl, map.orl, set.orl, …
│   └── (cada módulo pode ter uma pasta irmã com submódulos/algoritmos extras)
│
├── runtime/                 -- Artefatos do runtime nativo já compilados,
│                                organizados por plataforma (staticlib + cdylib)
│
├── docs/                    -- Toda a documentação
│   ├── book/                -- O livro que você está lendo agora
│   ├── spec/                -- O contrato normativo formal da linguagem
│   ├── language/            -- Tour rápido (uma página)
│   ├── guides/               -- Tutoriais de tarefas específicas
│   └── planning/             -- Roadmap, decisões de design, ADRs
│
└── examples/                -- Programas completos escritos em Ori para demonstração
```

### Onde ficam os testes

Ao contrário de muitos projetos, a Ori **não** tem uma pasta `tests/` única
na raiz. Cada crate do compilador guarda os seus próprios testes de
integração dentro de si mesma, em `compiler/crates/<nome>/tests/` — por
exemplo, `compiler/crates/ori-driver/tests/` reúne testes que compilam e
rodam programas Ori reais de ponta a ponta.

### Dicas para Contribuidores

- Se você quer melhorar as **mensagens de erro (diagnostics)**, procure em `ori-diagnostics`, `ori-types` ou `ori-parser` (cada etapa emite os seus próprios códigos).
- Se você quer criar uma **função nova na biblioteca padrão** para todos usarem, vá em `stdlib/`.
- Se você quer **adicionar uma palavra-chave**, vai mexer em `ori-lexer` e `ori-parser`.
- Se você quer entender como um programa Ori vira binário de verdade, siga o [Cap. 6](../parte-2/06-pipeline-compilador.md) — ele segue exatamente essa ordem de crates.
