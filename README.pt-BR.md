# Ori

Ori é uma linguagem de programação compilada para código nativo, com tipagem
explícita e foco em leitura. O compilador é escrito em Rust e foi criado com um
objetivo direto: tornar programas mais fáceis de ler, inspecionar, diagnosticar
e manter.

Ori ainda é pre-1.0. O projeto já é útil para trabalho em compilador, design de
linguagem, ferramentas e runtime, mas a linguagem ainda pode mudar antes de um
contrato estável 1.0.

**Idiomas:** [English](README.md) | Português | [日本語](README.ja.md)

**Menu do projeto:** [Especificação](docs/spec/README.md) | [Planejamento](docs/planning/README.md) | [Biblioteca padrão](stdlib/README.md) | [Runtime](runtime/README.md) | [Exemplos](examples/) | [Changelog](CHANGELOG.md) | [Contribuição](CONTRIBUTING.md)

## Conteúdo

- [O que é Ori](#o-que-é-ori)
- [Por que Ori existe](#por-que-ori-existe)
- [Status atual](#status-atual)
- [Primeiros passos](#primeiros-passos)
- [Primeiro programa](#primeiro-programa)
- [Visão geral da CLI](#visão-geral-da-cli)
- [Visão geral da linguagem](#visão-geral-da-linguagem)
- [Arquitetura do compilador](#arquitetura-do-compilador)
- [Biblioteca padrão](#biblioteca-padrão)
- [Ferramentas de editor](#ferramentas-de-editor)
- [Layout do repositório](#layout-do-repositório)
- [Fluxo de desenvolvimento](#fluxo-de-desenvolvimento)
- [Layout de release](#layout-de-release)
- [Limitações conhecidas](#limitações-conhecidas)
- [Roadmap](#roadmap)
- [Licença](#licença)

## O que é Ori

Ori é uma linguagem estaticamente tipada com namespaces explícitos, tipos
explícitos, erros estruturados, limpeza determinística e geração de código
nativo.

Pipeline atual do compilador:

```text
fonte .orl
  -> lexer
  -> parser
  -> resolvedor de nomes
  -> type checker
  -> HIR
  -> backend nativo Cranelift
  -> binário com runtime ou execução JIT
```

O repositório contém compilador, runtime, fontes da biblioteca padrão,
especificação da linguagem, extensão VS Code, exemplos e ferramentas de release.

## Por que Ori existe

Ori otimiza leitura antes de escrita.

O código deve deixar informações importantes visíveis no ponto em que o leitor
precisa delas:

| Pergunta | Ori deixa visível por meio de |
|---|---|
| Onde este arquivo pertence? | `namespace` no topo de cada arquivo |
| Qual é o tipo deste valor? | anotações de tipo explícitas |
| Este valor pode estar ausente? | `optional<T>` |
| Esta operação pode falhar? | `result<T, E>` |
| Quando um recurso é liberado? | `using` |
| De onde vem este comportamento? | `trait` e `implement` |
| O que deu errado? | códigos de diagnóstico estruturados |

Esse design reduz carga cognitiva: menos regras escondidas, cadeias de
inferência menores e mensagens de erro mais claras.

## Status atual

| Área | Status |
|---|---|
| Versão | `0.2.0`, congelada na linha `0.2.x` até haver breaking change real |
| Estabilidade | pre-1.0; compatibilidade de código-fonte ainda pode mudar |
| Compilador | workspace Rust com lexer, parser, HIR, checker, codegen, diagnósticos, LSP, driver e runtime |
| Backend nativo | código objeto Cranelift mais runtime nativo Ori |
| `ori run` | JIT por padrão quando a cdylib do runtime está disponível; AOT pode ser forçado |
| `ori compile` | geração AOT de binário nativo; rota de link depende da estratégia configurada |
| Backend C | rota de debug/transpile com paridade parcial |
| Biblioteca padrão | primitivas de runtime Layer 1 mais wrappers e algoritmos `.orl` Layer 2/3 |
| Ferramentas | CLI, formatter, catálogo de diagnósticos, export de docs, LSP, extensão VS Code |
| Testes | suíte do workspace e smoke de release nativa fazem parte do gate do projeto |

O versionamento é conservador. `0.3.0` fica reservado para uma quebra real
visível ao usuário, não para cada marco interno.

## Primeiros passos

Pré-requisitos para desenvolvimento do compilador:

- Rust `1.95.0`, definido em `rust-toolchain.toml`
- Um linker da plataforma ou uma das estratégias explícitas de link do Ori
- PowerShell no Windows para scripts de smoke de release
- Toolchain C no Linux/macOS quando usar rotas de descoberta do sistema

Na raiz do repositório:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- check examples/hello_world.orl
cargo run -p ori-driver -- run examples/hello_world.orl
```

No Windows, valide um pacote em formato de release com:

```powershell
.\tools\smoke_native_release.ps1
```

No Linux ou macOS:

```sh
sh tools/smoke_native_release.sh
```

## Primeiro programa

```lua
namespace app.hello

import ori.io as io

func main()
    io.print("Hello, Ori!")

    const answer: int = 21 * 2
    io.print(f"The answer is {answer}")
end
```

Execute pelo repositório:

```bash
cargo run -p ori-driver -- run examples/hello_world.orl
```

Ori usa blocos delimitados por `end`, declarações separadas por linha, imports
explícitos e tipos explícitos em bindings e contratos públicos.

## Visão geral da CLI

A CLI `ori` é implementada em `compiler/crates/ori-driver`.

| Comando | Função |
|---|---|
| `ori check <file.orl>` | faz parse, resolução e checagem de tipos |
| `ori run <file.orl>` | compila e executa por JIT ou AOT, conforme runtime e variáveis de ambiente |
| `ori compile <file.orl>` | emite executável nativo pelo backend Cranelift |
| `ori test <file.orl>` | executa funções marcadas com `@test` |
| `ori fmt <file.orl>` | formata fonte e imprime o resultado |
| `ori doc file <file.orl>` | extrai comentários de documentação como Markdown ou HTML |
| `ori doc export` | exporta símbolos stdlib, diagnósticos e keywords como JSON |
| `ori doctor` | reporta saúde da stdlib, runtime, linker, target e JIT |
| `ori explain <code>` | explica um código de diagnóstico |
| `ori summary [path]` | imprime entry file, namespaces, imports e contagem de diagnósticos |
| `ori build <file.orl>` | emite C pelo backend de debug |
| `ori lex <file.orl>` | imprime tokens para debug do compilador |
| `ori parse <file.orl>` | imprime AST para debug do compilador |
| `ori install <name>` | placeholder de registry; ainda indisponível |
| `ori publish <path>` | placeholder de registry; ainda indisponível |

Variáveis úteis:

| Variável | Função |
|---|---|
| `ORI_STDLIB_ROOT` | sobrescreve a raiz `stdlib/` |
| `ORI_RUNTIME_LIB` | sobrescreve a biblioteca estática do runtime nativo |
| `ORI_RUNTIME_CDYLIB` | sobrescreve a cdylib do runtime usada pelo JIT |
| `ORI_USE_JIT=1` | força JIT em `ori run` |
| `ORI_USE_AOT=1` | força AOT em `ori run` |
| `ORI_USE_BUNDLED_RUST_LLD=1` | linka por `rust-lld` empacotado sem o driver `rustc` |
| `ORI_USE_SYSTEM_LINKER=1` | linka diretamente pelo linker da plataforma |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | rejeita fallback para runtime do workspace durante validação de pacote |

A matriz completa de ambiente está em [AGENTS.md](AGENTS.md).

## Visão geral da linguagem

O modelo central de Ori é pequeno:

- todo arquivo começa com `namespace`;
- imports criam aliases locais;
- declarações top-level são privadas, exceto quando marcadas como `pub`;
- `struct` e `enum` definem dados;
- `trait` e `implement` definem comportamento;
- `optional<T>` modela ausência;
- `result<T, E>` modela falha recuperável;
- `?` propaga valores `result` ou `optional`;
- `using` deixa limpeza explícita;
- diagnósticos usam códigos estáveis como `name.undefined` e
  `project.circular_import`.

Exemplo com `result`:

```lua
namespace app.errors

import ori.io as io

func divide(a: int, b: int) -> result<int, string>
    if b == 0
        return error("division by zero")
    end

    return success(a / b)
end

func main() -> result<void, string>
    const value: int = divide(84, 2)?
    io.print(f"value: {value}")
    return success()
end
```

Para o contrato normativo da linguagem, comece por
[docs/spec/01-overview.md](docs/spec/01-overview.md).

## Arquitetura do compilador

O compilador é dividido em crates focadas:

| Crate | Papel |
|---|---|
| `ori-lexer` | tokenização |
| `ori-ast` | definições dos nós da AST |
| `ori-parser` | parser recursive descent |
| `ori-hir` | resolução de nomes e HIR |
| `ori-types` | sistema de tipos, manifesto stdlib e contratos do checker |
| `ori-codegen` | backend nativo Cranelift, JIT e backend C de debug |
| `ori-runtime` | biblioteca nativa de runtime e ABI |
| `ori-diagnostics` | códigos de diagnóstico e apoio de renderização |
| `ori-lsp` | implementação Language Server Protocol |
| `ori-driver` | CLI, orquestração do pipeline e testes de integração |

O runtime nativo é a referência semântica para `ori compile`, `ori run` e
`ori test`. O backend C é uma rota de debug e não deve ser tratado como fonte de
verdade para async, ARC, coleções ou runtime.

## Biblioteca padrão

A stdlib vive no namespace `ori.*`.

Forma atual:

| Camada | Local | Função |
|---|---|---|
| Layer 1 | `compiler/crates/ori-types/src/stdlib.rs` e `compiler/crates/ori-runtime/src/lib.rs` | manifesto, ABI e primitivas de runtime |
| Layer 2 | `stdlib/**/*.orl` | wrappers seguros sobre primitivas de runtime |
| Layer 3 | `stdlib/**/*.orl` | algoritmos puros escritos em Ori |

Áreas disponíveis:

- `ori.io`, `ori.fs`, `ori.path`
- `ori.string`, `ori.bytes`, `ori.convert`
- `ori.list`, `ori.map`, `ori.set`
- `ori.math`, `ori.random`, `ori.time`
- `ori.json`, `ori.net`, `ori.process`
- `ori.task`, `ori.channel`, `ori.concurrent`
- `ori.test` e helpers de teste

Veja [stdlib/README.md](stdlib/README.md) para o inventário atual e
[docs/spec/12-stdlib.md](docs/spec/12-stdlib.md) para os contratos normativos.

## Ferramentas de editor

Ori inclui um servidor LSP e uma extensão VS Code em
[extensions/vscode-orl](extensions/vscode-orl/).

Ferramentas implementadas:

- diagnósticos do parser, resolver e type checker;
- hover, go-to-definition, find references e rename;
- semantic tokens, document symbols, workspace symbols e inlay hints;
- dot completion baseada em tipo;
- hover/completion/goto cientes da stdlib Layer 1 e Layer 2;
- formatting, code actions, code lens e signature help;
- sync incremental de documento;
- comandos VS Code para check, run, test, format, doctor e summary.

Build local da extensão:

```bash
cd extensions/vscode-orl
npm install
npm run compile
```

Antes, gere o language server:

```bash
cargo build -p ori-lsp -p ori-driver
```

## Layout do repositório

```text
ori-lang/
  compiler/crates/        workspace Rust do compilador, LSP, runtime e driver
  docs/spec/              contratos normativos da linguagem e implementação
  docs/planning/          roadmap, backlog e planos de implementação
  stdlib/                 módulos fonte da biblioteca padrão
  runtime/                artefatos de runtime por target triple
  examples/               programas Ori de exemplo
  tests/                  fixtures E2E e documentação de testes
  extensions/vscode-orl/  extensão VS Code
  tools/                  scripts de staging, smoke, export e validação
  branding/               assets de marca
  _reversa_sdd/           auditorias históricas de engenharia reversa
```

## Fluxo de desenvolvimento

Gates comuns:

```bash
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-lsp
```

Para mudanças na stdlib:

```bash
cargo test -p ori-types --lib stdlib
cargo test -p ori-driver --test multifile_imports
```

Para mudanças no runtime ou backend nativo, re-stage o runtime antes dos testes
de compile/run:

```powershell
.\tools\stage_native_runtime.ps1
```

Unix:

```sh
./tools/stage_native_runtime.sh
```

Regras do projeto:

- bugfix precisa de teste de regressão em `compiler/crates/ori-driver/tests/`;
- comportamento novo deve atualizar docs e `CHANGELOG.md`;
- códigos de diagnóstico novos devem entrar em
  [docs/spec/13-error-catalog.md](docs/spec/13-error-catalog.md);
- mudanças no runtime da stdlib devem manter manifesto, lowering, ABI, testes e
  docs em sincronia.

## Layout de release

Um pacote em formato de release deve seguir esta forma:

```text
ori.exe                         # ou `ori` no Unix
runtime/
  bin/
    rust-lld[.exe]              # linker empacotado opcional
  {target-triple}/
    ori_runtime.lib             # runtime estático Windows MSVC
    libori_runtime.a            # runtime estático Unix
    ori_runtime.dll             # cdylib Windows para JIT
    libori_runtime.so           # cdylib Linux para JIT
    libori_runtime.dylib        # cdylib macOS para JIT
    runtime-link.json
examples/
README.md
```

O workflow `native-route` cobre Windows MSVC, Windows GNU, Linux GNU,
macOS x86_64 e macOS aarch64. Detalhes de staging ficam em
[runtime/README.md](runtime/README.md).

## Limitações conhecidas

Limitações atuais de pre-1.0:

- Ori ainda não é self-hosting.
- `ori compile` é uma rota AOT e ainda depende de uma estratégia de linker
  funcional.
- O backend C é parcial e existe para debug.
- `ori install` e `ori publish` são stubs de registry.
- `ori repl` ainda está no backlog.
- Algumas formas avançadas de async ainda estão documentadas como known issues
  no plano de maturidade.
- Contratos públicos ainda podem mudar antes de 1.0.

Veja [docs/planning/PENDENTES.md](docs/planning/PENDENTES.md) e
[docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md](docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md)
para o backlog ativo.

## Roadmap

Os critérios de longo prazo para 1.0 são intencionalmente rígidos:

1. remover a dependência prática de Rust dos caminhos de compilação para usuário final;
2. manter camadas substantivas da stdlib em `.orl` quando fizer sentido;
3. provar self-hosting ou um caminho de bootstrap crível;
4. documentar ABI estável;
5. ter usuários reais além dos testes do repositório;
6. ficar pelo menos seis meses sem breaking changes.

Até lá, o projeto permanece honesto sobre seu status pre-1.0.

## Licença

Ori é licenciado sob uma das duas opções:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

à sua escolha.
