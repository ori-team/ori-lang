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
| `ori test <arquivo>` | Roda as funções marcadas com `@test` |
| `ori explain <código>` | Explica um código de erro, ex.: `ori explain type.type_mismatch` |

```bash
ori new meu_app
cd meu_app
ori check main.orl
ori run main.orl
```

`ori check .` sobe as pastas até achar o `ori.proj`, então funciona de qualquer
subpasta do projeto. `ori check ori.proj` é equivalente.

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

Só funções marcadas com `@c_export` ficam visíveis para C. A superfície cobre
escalares (`int`, `float`, `bool`, …) e **`string`**, que atravessa como
`const char *` terminado em NUL. Agregados — structs, `list`, `map`, `optional`,
`result` — ainda não são exportáveis.

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

## Formatação e migração

| Comando | O que faz |
|---|---|
| `ori fmt <arquivo>` | Formata um arquivo e imprime o resultado |
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

`--no-color` é aceito por todos os comandos e desliga a saída ANSI.
