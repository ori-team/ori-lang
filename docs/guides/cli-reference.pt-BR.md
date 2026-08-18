# Referência da CLI

> **Público:** quem usa o comando `ori`  
> **English:** [cli-reference.md](cli-reference.md)  
> Gerado a partir de `ori --help` na série `0.3.x`. Rode `ori <comando> --help`
> para a lista completa de flags de qualquer comando.

Veja o que você tem instalado:

```bash
ori --version
```

---

## Os cinco primeiros

| Comando | O que faz |
|---|---|
| `ori new <path>` | Cria um projeto novo (`--lib` para biblioteca) |
| `ori check <arquivo>` | Checa tipos e mostra diagnósticos — não gera binário |
| `ori run <arquivo>` | Compila e roda pelo runtime nativo |
| `ori test <arquivo> [--doc]` | Roda as funções marcadas com `@test` (ou `--doc` para testar blocos markdown) |
| `ori explain <código>` | Explica um código de erro, ex.: `ori explain type.type_mismatch` |

```bash
ori new meu_app
cd meu_app
ori check main.orl
ori run main.orl
```

`ori check .` sobe as pastas até achar o `ori.proj`, então funciona de qualquer
subpasta do projeto. `ori check ori.proj` é equivalente.

Todos os comandos de compilação aceitam a mesma seleção condicional:

```bash
ori check . --features tls,telemetry
ori run . --no-default-features
ori check . --execution-profile embedded
ori check . --target x86_64-unknown-linux-gnu
```

As features precisam estar declaradas em `[features]` no manifesto do projeto
ou package. Essas flags também participam do fingerprint do cache AOT.
`--target` seleciona fatos de cfg e artefatos de runtime; sozinho, ele não
promete cross-compilation nativa completa. Um triple cuja arquitetura ou OS
esteja fora de cfg v1 é rejeitado, em vez de ser tratado como alvo sem OS.
Da mesma forma, `--execution-profile embedded` seleciona ramos de cfg; ele
ainda não transforma o runtime desktop em um runtime freestanding ou isolado.

---

## Construir

| Comando | O que faz |
|---|---|
| `ori compile <arquivo> -o <saída>` | Compila para binário nativo via Cranelift |
| `ori build` | Constrói um arquivo ou projeto pelo backend nativo |
| `ori run <arquivo>` | Compila e roda em um passo |

```bash
ori compile main.orl -o meu_app
./meu_app
```

Para gerar biblioteca compartilhada em vez de executável:

```bash
ori compile main.orl --lib -o libmeu_app.so
```

Só funções marcadas com `@c_export` ficam visíveis para C. A ABI cobre escalares,
`string`, structs escalares não vazias e não genéricas, structs gerenciadas por
handles ARC opacos e bridges diretos de `optional`/`result`. `list`, `map`,
`set`, `tuple`, unions aninhadas, structs genéricas e structs vazias diretas
continuam rejeitadas. O header gerado é a declaração canônica para o host; veja
[ABI-1](../spec/19-abi.md#83b-c_export--the-host-facing-surface).

Uma string **retornada** ao host pertence ao host: libere com
`ori_arc_release`, ou ela vaza. Uma string **passada** continua sendo do host;
Ori nunca a libera. Veja [../spec/19-abi.md](../spec/19-abi.md) §8.3b.

---

## Projeto e pacotes

| Comando | O que faz |
|---|---|
| `ori new <path>` | Cria projeto em uma pasta nova |
| `ori init` | Inicializa projeto em uma pasta existente |
| `ori summary` | Mostra entry, namespaces e imports do projeto |
| `ori install <nome> --path .` | Instala um pacote no cache local |
| `ori get` | Baixa dependências git/path para o cache local |
| `ori lock [path]` | Resolve dependências e grava `ori.lock`; `--locked` apenas valida |
| `ori publish` | Publica um pacote no registry de `ORI_REGISTRY` |
| `ori update` | Atualiza a toolchain para a última release publicada |

Os campos do manifesto estão em
[../spec/17-project-and-docs.md](../spec/17-project-and-docs.md).

---

## Documentação

| Comando | O que faz |
|---|---|
| `ori doc file <arquivo>` | Extrai comentários de doc como Markdown ou HTML |
| `ori doc check` | Valida docs inline e sidecars `.oridoc` |
| `ori doc export` | Exporta JSON da stdlib + catálogo de erros para o site |

---

## Formatação, linting e ferramentas

| Comando | O que faz |
|---|---|
| `ori fmt <path> [-w / --write] [-c / --check]` | Formata um arquivo ou pasta recursivamente (`-w` in-place, `-c` check) |
| `ori lint <path>` | Executa linter semântico de código para variáveis não usadas e redundâncias |
| `ori daemon [--stdio]` | Executa o daemon compilador JSON-RPC 2.0 contínuo via stdio para avaliação/formatação rápida |
| `ori bindgen <header.h> [--module <nome>]` | Gera bindings `extern "c"` e structs `@repr("C")` a partir de cabeçalho C |
| `ori migrate-syntax <path>` | Reescreve sintaxe pré-S3 em arquivos `.orl` |

O `ori migrate-syntax` cuida da parte mecânica do corte S3. Ele reescreve:

| De | Para |
|---|---|
| `namespace` | `module` |
| `import x as y` / `import x only (…)` | `import x = y` / `import x (…)` |
| `implement T for X` | `apply X` + `use T` (só o cabeçalho) |
| `apply Trait to Type` | `apply Type` + `use Trait` |
| bloco aninhado com um só `use` | cabeçalho compacto `apply T use Trait` |
| `type Name = …` | `alias Name = …` |
| `Foo<T>` / `list of T` | `Foo[T]` / `list[T]` |
| `where T is Trait` | `for T: Trait` |
| `success` / `error` | `ok` / `err` |
| `else if` | `elif` |
| `do(x) =>` | `(x) =>` |
| `case .Variant` | `case Variant` |
| `func` de declaração | removido |

Duas coisas que ele **não** termina, e reporta como nota:

- `expr?` posfixo não é migrado automaticamente — reescreva para `try expr` na mão;
- um cabeçalho `implement` reescrito deixa o corpo para você revisar como seção `use`.

Use `--dry-run` para pré-visualizar. Rode `ori check` depois, nos dois casos.

---

## Ambiente e diagnóstico

| Comando | O que faz |
|---|---|
| `ori doctor` | Relata saúde do ambiente, stdlib e runtime nativo |
| `ori explain <código>` | Explica um código do catálogo de erros |

Rode `ori doctor` primeiro sempre que um build falhar por um motivo que não
está no seu código.

---

## Interativo

| Comando | O que faz |
|---|---|
| `ori repl` | REPL interativo pequeno, apoiado no JIT nativo |

O REPL é explicitamente uma superfície **experimental** — seus limites podem
mudar antes da `1.0`
([../spec/18-stability-and-compatibility.md](../spec/18-stability-and-compatibility.md)).

---

## Depuração do compilador

Estes imprimem intermediários do compilador. Servem para trabalhar **no** Ori,
não com ele.

| Comando | O que faz |
|---|---|
| `ori lex <arquivo>` | Imprime o fluxo de tokens cru |
| `ori parse <arquivo>` | Imprime a AST |
| `ori emit c <arquivo>` | Emite código C pelo backend de debug parcial |

O backend C é auxílio de depuração, não referência semântica — a referência é o
backend nativo
([../spec/14-backend-support.md](../spec/14-backend-support.md)).

## Depuração de programas

Use o debugger nativo cooperativo para programas nativos, inclusive funções
async e closures:

```text
ori debug examples/cli_args/main.orl --breakpoint 41
```

Quando parar, `c` continua, `s` avança para a próxima linha instrumentada e
`q` encerra o alvo. O adaptador de terminal mostra local, pilha (inclusive
frames async) e variáveis visíveis. O mesmo catálogo de variáveis é gerado no
Linux, macOS e Windows; os valores ao vivo vêm do snapshot cooperativo do
runtime.

Para uma IDE, inicie o servidor mínimo do Debug Adapter Protocol pelo stdio:

```text
ori debug --dap
```

O adaptador DAP aceita `initialize`, `launch`, `setBreakpoints`,
`configurationDone`, `continue`, `next`, `threads`, `stackTrace`, `scopes`,
`variables`, `evaluate` e `disconnect`. Campos de structs, `optional`,
`result`, payloads de enums, mapas, conjuntos e coleções opacas suportadas
aparecem com nomes qualificados (por exemplo `user.name`); listas expõem
`length`/`capacity` e filhos indexados com limite seguro. Frames async continuam
visíveis através de `await`, e capturas aparecem no frame da closure. Strings e
bytes gerenciados mostram uma prévia limitada (bytes em hexadecimal); buffers
estáticos ou estrangeiros só são lidos depois do registro de um comprimento
exato. `evaluate` limita-se a aritmética escalar, comparações, lógica booleana e
strings do último snapshot parado, sem executar código no alvo. Builds nativos
também escrevem `program.debug.json`, um catálogo portátil de parâmetros,
variáveis locais, bindings de padrões e capturas de closures com suas linhas.

---

## Variáveis de ambiente

| Variável | Efeito |
|---|---|
| `ORI_PACKAGE_CACHE` | Onde os pacotes são instalados (padrão `~/.ori/packages`) |
| `ORI_REGISTRY` | URL do registry usada por `ori publish` / `ori install` |
| `ORI_REGISTRY_TOKEN` | Token de autenticação do registry |
| `ORI_STDLIB_ROOT` | Sobrescreve o local da stdlib |
| `ORI_RUNTIME_LIB` / `ORI_RUNTIME_CDYLIB` | Aponta para um runtime nativo específico |
| `ORI_REQUIRE_PACKAGED_RUNTIME` | Falha em vez de cair no build do runtime via Cargo |
| `ORI_USE_JIT` / `ORI_USE_AOT` | Força a rota de execução do `ori run` |
| `ORI_USE_SYSTEM_LINKER` / `ORI_USE_BUNDLED_RUST_LLD` | Escolhe o linker |
| `ORI_DISABLE_INCREMENTAL` | Desativa a reutilização da saída completa e dos objetos por arquivo em `.ori/incremental.json` / `.ori/modules/` |
| `ORI_OBJCOPY` | Escolhe `objcopy`/`llvm-objcopy` para emitir seções DWARF no Linux |

`--no-color` é aceito por todos os comandos e desliga a saída ANSI.

Quando o projeto contém `ori.lock`, a resolução de dependências é validada
antes da compilação. Imports continuam limitados ao pacote: use o módulo
qualificado da dependência (`demo.math`) em vez de uma busca sem prefixo entre
todos os pacotes.
Rebuilds nativos informam quantos módulos-fonte mudaram. O rebuild mantém em
`.ori/modules/` os objetos de arquivos inalterados e faz o link com os objetos
regenerados; projetos com inicializadores globais dinâmicos, `--lib` ou
instrumentação explícita de debug usam a rota monolítica conservadora. No
Windows, um link nativo bem-sucedido escreve o `.pdb` irmão com caminho
determinístico quando o linker o suporta.
