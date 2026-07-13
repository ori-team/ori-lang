# Projeto e documentação externa

Status: atual.

Este capítulo define dois arquivos de projeto:

- `ori.proj`: manifesto do projeto.
- `ori.pkg.toml`: manifesto de pacote reutilizavel.
- `.oridoc`: documentação externa de símbolos Ori.

A ideia é manter o código legível sem obrigar comentários longos dentro do
arquivo `.orl`.

## `ori.proj`

`ori.proj` fica na raiz do projeto. O formato é simples e explícito:

```ini
manifest = 1
name = "demo"
version = "0.1.0"
kind = "app"
entry = "src/main.orl"

[source]
root = "src"
root_namespace = "app"

[dependencies]
demo.math = { path = "../math", version = "0.1.0" }

[docs]
paths = ["docs/api"]
mode = "sidecar-first"
require_public = "off"
```

Campos atuais:

| Campo | Obrigatorio | Descricao |
|---|---:|---|
| `manifest` | nao | Versao do formato. Hoje aceita `1`. |
| `name` | nao | Nome humano do projeto. |
| `version` | nao | Versao do projeto. |
| `kind` | nao | `app` ou `lib`. Padrao: `app`. |
| `entry` | sim | Arquivo `.orl` de entrada. |
| `source.root` | nao | Pasta principal de codigo. |
| `source.root_namespace` | nao | Namespace esperado para a raiz de codigo. |
| `dependencies.<name>` | nao | Dependencia local por `{ path = "..." }`; versao opcional. |
| `docs.paths` | nao | Arquivos ou pastas com `.oridoc`. |
| `docs.mode` | nao | `sidecar-first` ou `inline-first`. Padrao: `sidecar-first`. |
| `docs.require_public` | nao | `off`, `warn` ou `error`. Padrao: `off`. |

Compatibilidade: manifestos antigos com apenas `entry = "main.orl"` continuam
validos.

Dependencias locais declaradas em `[dependencies]` participam da resolucao de
imports. O compilador procura primeiro os arquivos do projeto atual. Se nao
encontrar o import, procura nas dependencias por `path`.

```lua
import demo.math only (double)
```

Para `demo.math = { path = "../math" }`, o path deve apontar para um projeto com
`ori.proj` ou um pacote com `ori.pkg.toml`. Dependencias apenas por versao ficam
reservadas para registry remoto; elas nao sao resolvidas localmente.

## `ori.pkg.toml`

`ori.pkg.toml` descreve um pacote instalavel no cache local. Ele nao substitui
`ori.proj`: `ori.proj` organiza o projeto em desenvolvimento; `ori.pkg.toml`
define o contrato de distribuicao.

Formato atual:

```toml
[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.2.0"
description = "Demo app"

[dependencies]
demo.math = { path = "../demo-math", version = "0.1.0" }
```

Campos obrigatorios:

| Campo | Descricao |
|---|---|
| `package.name` | Nome pontilhado alinhado ao namespace Ori. |
| `package.version` | Versao `major.minor.patch`. |
| `package.entry` | Arquivo `.orl` de entrada do pacote. |
| `package.ori_version` | Versao minima esperada do compilador Ori. |

Dependencias locais usam `{ path = "../outro-pacote" }`. O manifesto apontado
pelo path deve declarar o mesmo nome usado na chave da dependencia.
Dependencias somente por versao ficam reservadas para registry remoto ou pacote
ja presente no cache.

`ori check`, `ori run`, `ori test` e `ori doc` aceitam `ori.pkg.toml` como
entrada. Quando o pacote declara dependencias locais, o resolvedor usa esses
paths para carregar imports do pacote antes de emitir `bind.import_not_found`.

## `.oridoc`

Um arquivo `.oridoc` documenta simbolos de um namespace. Ele pode ficar ao lado
do `.orl`:

```text
src/math.orl
src/math.oridoc
```

Ou em uma pasta configurada:

```text
docs/api/math.oridoc
```

Exemplo:

```text
oridoc 1

namespace app.math

doc func add
    summary:
        Soma dois numeros.
    param left:
        Primeiro valor.
    param right:
        Segundo valor.
    returns:
        Soma de `left` e `right`.
end
```

Regras:

- `namespace` deve ser o mesmo namespace do codigo documentado.
- `doc func add` documenta `app.math.add`.
- `doc method User.name` documenta `app.math.User.name`.
- `doc module self` documenta o modulo `app.math`.
- Cada bloco termina com `end`.

Secoes reconhecidas:

| Secao | Uso |
|---|---|
| `summary:` | Texto principal. |
| `details:` | Texto adicional. |
| `param nome:` | Parametro documentado. |
| `returns:` | Valor retornado. |

## Comandos

Criar um projeto novo:

```bash
ori new demo
```

O comando cria `ori.proj`, `src/main.orl` e `docs/api/`. Ele falha quando o
diretorio de destino ja existe e nao esta vazio.

Instalar um pacote local no cache:

```bash
ori install demo.app --path .
```

O cache padrao fica em `~/.ori/packages/<name>/<version>/`. Use
`ORI_PACKAGE_CACHE` ou `--cache` para escolher outra pasta. O comando valida o
manifesto, valida dependencias locais por path e copia os arquivos. Ele nao
executa codigo do pacote.

Gerar Markdown:

```bash
ori doc file ori.proj
```

Validar docs sem gerar arquivo:

```bash
ori doc check ori.proj
```

O LSP usa `.oridoc` no hover quando encontra uma entrada para o simbolo local.

## REPL

`ori repl` inicia um loop interativo pequeno apoiado no JIT nativo. O recorte
inicial aceita:

- `import ...`;
- bindings simples com `const` e `var`;
- chamadas como `io.println("ola")`;
- literais e expressoes simples, impressas automaticamente.

Comandos multi-linha e estado mutavel persistente entre comandos ainda nao sao
parte do contrato do REPL.

## Diagnosticos

`ori doc check` valida:

- sintaxe do `.oridoc`;
- simbolo inexistente;
- parametro documentado que nao existe na assinatura;
- retorno ausente em funcao nao-`void`;
- ausencia de doc publica quando `docs.require_public` for `warn` ou `error`.

## Limitacoes atuais

- O hover inicial cobre simbolos locais. Hovers complexos de metodo por tipo do
  receptor podem ser ampliados depois.
- `docs.require_public` e opcional. O padrao e `off` para nao quebrar projetos
  existentes.
