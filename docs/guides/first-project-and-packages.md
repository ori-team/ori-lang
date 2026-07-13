# Primeiro projeto e pacotes locais

Status: guia pratico para Ori `0.2.x`.

Este guia cobre o caminho curto:

1. criar um projeto;
2. rodar comandos basicos;
3. criar uma dependencia local;
4. instalar o pacote no cache local.

## Criar um projeto

```bash
ori new demo
cd demo
ori check ori.proj
ori run src/main.orl
```

`ori new` cria:

```text
demo/
  ori.proj
  src/
    main.orl
  docs/
    api/
```

## Rodar os comandos principais

```bash
ori check ori.proj
ori run src/main.orl
ori fmt src/main.orl
ori doc check ori.proj
ori summary .
```

Para testes, crie uma funcao com `@test`:

```lua
namespace demo.main

import ori.test as test

@test
func math_is_stable()
    test.assert(1 + 1 == 2, "math should work")
end
```

Depois rode:

```bash
ori test src/main.orl
```

## Criar uma biblioteca local

Estrutura:

```text
workspace/
  app/
    ori.proj
    ori.pkg.toml
    src/main.orl
  math/
    ori.pkg.toml
    src/lib.orl
```

`math/ori.pkg.toml`:

```toml
[package]
name = "demo.math"
version = "0.1.0"
entry = "src/lib.orl"
ori_version = "0.2.0"
```

`math/src/lib.orl`:

```lua
namespace demo.math

pub func double(value: int) -> int
    return value * 2
end
```

`app/ori.proj`:

```ini
manifest = 1
name = "demo.app"
version = "0.1.0"
kind = "app"
entry = "src/main.orl"

[source]
root = "src"
root_namespace = "demo.app"

[dependencies]
demo.math = { path = "../math", version = "0.1.0" }
```

`app/ori.pkg.toml`:

```toml
[package]
name = "demo.app"
version = "0.1.0"
entry = "src/main.orl"
ori_version = "0.2.0"

[dependencies]
demo.math = { path = "../math", version = "0.1.0" }
```

`app/src/main.orl`:

```lua
namespace demo.app

import demo.math only (double)
import ori.io as io

func main()
    io.println(string(double(21)))
end
```

Durante o desenvolvimento, rode:

```bash
cd workspace/app
ori check ori.proj
ori run src/main.orl
```

O resolvedor procura `demo.math` no path declarado no `ori.proj`. O
`ori.pkg.toml` usa o mesmo contrato para instalar o pacote no cache local.

## Instalar no cache local

Dentro de `workspace/app`:

```bash
ori install demo.app --path .
```

O cache padrao e:

```text
~/.ori/packages/<name>/<version>/
```

Para usar outra pasta:

```bash
ori install demo.app --path . --cache ./cache
```

Ou:

```bash
ORI_PACKAGE_CACHE=./cache ori install demo.app --path .
```

O instalador local:

- valida `ori.pkg.toml`;
- valida dependencias por `path`;
- copia arquivos para o cache;
- nao executa codigo do pacote;
- rejeita symlinks durante a copia.

## Upgrade em `0.2.x`

Enquanto Ori estiver em `0.2.x`:

- leia `CHANGELOG.md` antes de atualizar;
- rode `ori check ori.proj`;
- rode `ori test`;
- regenere docs com `ori doc check ori.proj`;
- reinstale pacotes locais se o `ori.pkg.toml` mudou.

`0.3.0` fica reservado para quebra real de compatibilidade. Mudancas aditivas e
correcoes pequenas devem continuar em `0.2.x`.
