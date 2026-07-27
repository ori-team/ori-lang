# Cap. 15 — Módulos, imports e projetos
> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR
Módulos organizam seu código em pacotes lógicos, ajudando a evitar colisões de nomes. Imports limpos e o manifesto do projeto (`ori.proj`) são a base para construir aplicações no Ori.

## O Manifesto do Projeto (ori.proj)

Todo projeto em Ori precisa de um arquivo `ori.proj` na pasta principal. Ele funciona como o RG do seu projeto, dizendo ao compilador como construir o código.

Diferente dos arquivos `.orl`, o `ori.proj` **não** é escrito na sintaxe da
Ori — ele é um arquivo **TOML** (o mesmo formato do `Cargo.toml` do Rust ou
do `pyproject.toml` do Python: `chave = valor`, com seções entre colchetes).

```toml
# ori.proj
manifest = 1
name = "meu_app"
version = "0.1.0"
kind = "app"        -- "app" (executável) ou "lib" (biblioteca)
entry = "main.orl"  -- o arquivo que contém o main()

[source]
root_namespace = "app"  -- prefixo de todos os seus módulos
```

Para começar do zero, basta usar a ferramenta de linha de comando:
```bash
ori new meu_projeto
```
Isso criará a pasta, o `ori.proj` e o módulo inicial.

## Módulos e Nomes

Todo arquivo de código Ori termina em **`.orl`** e começa com a declaração do módulo:
```ori
module app.api
```
A convenção é usar o `root_namespace` do seu `ori.proj` (como `app`) e o caminho dentro do projeto. Os módulos da biblioteca padrão (stdlib) do Ori começam com `ori` (ex: `ori.io`, `ori.fs`) — nunca importe um módulo próprio usando esse prefixo.

## Imports (Importações)

Para usar código de outro módulo, usamos `import`. O caminho do módulo fica do lado esquerdo do `=` e o alias (apelido que você vai usar) fica no lado direito.

Isso é fundamental porque obriga você a dar um nome explícito, deixando o código limpo e organizado (sem nomes jogados ao acaso no escopo global).

```ori
-- Importando um único módulo
import ori.fs = fs
import app.utils = utils
```

Se você precisar importar várias coisas, use o bloco `imports`:
```ori
imports
    ori.io = io
    ori.fs = fs
    app.models = models
end
```

### Exemplo de Multi-módulos

**Arquivo 1: src/models.orl**
```ori
module app.models

public struct User
    name: string
    age: int
end
```

**Arquivo 2: src/utils.orl**
```ori
module app.utils

imports
    app.models = models
end

-- A função acessa a struct 'User' através do alias 'models'
public format_user(u: models.User) -> string
    return f"Nome: {u.name}"
end
```

**Arquivo 3: src/main.orl**
```ori
module app.main

imports
    app.models = models
    app.utils = utils
end

main()
    const u: models.User = models.User { name: "Alice", age: 30 }
    const text: string = utils.format_user(u)
end
```

## Visibilidade (Public)

Por padrão, tudo no Ori é **privado** (private). Isso significa que structs, funções e enums só podem ser usados dentro do mesmo módulo.

Para liberar o acesso para outros arquivos, adicione a palavra-chave `public`:
```ori
public get_data() -> string
    return "dado"
end
```

### Re-exportação Pública (Public Import)

Às vezes, seu projeto tem vários arquivos internos, mas você quer que quem for usar o seu pacote importe de um lugar só. O `public import` (re-exportação) resolve isso. Ele importa algo e já deixa público para o próximo nível.

```ori
module app.api

-- Quem importar 'app.api' também terá acesso ao que tem em 'app.models' através do alias 'models'
public import app.models = models
```

Isso cria fronteiras (boundaries) limpas. Em vez de o usuário importar `app.internal.db.models`, ele importa apenas `app.api` e usa `api.models`.

## Por que Aliases são Importantes?

O uso obrigatório do sinal de igual (`=`) em imports força o isolamento.

**Sem aliases (como em outras linguagens, que fica bagunçado):**
```text
-- O que é 'User'? De onde veio 'save'?
const u = User { ... }
save(u)
```

**Com aliases do Ori (muito mais limpo e legível):**
```ori
import app.models = models
import app.database = db

const u: models.User = models.User { ... }
db.save(u)
```

## O que memorizar
- Cada arquivo `.orl` começa com `module namespace.nome`.
- Todos os imports exigem um alias (ex: `import ori.fs = fs`), sempre na ordem **caminho = alias**.
- Elementos são privados por padrão. Use `public` para expô-los.
- Use blocos `imports ... end` para organizar múltiplas dependências.
- O `ori.proj` é **TOML**, não sintaxe Ori — ele define nome, tipo de projeto (`kind`), ponto de entrada (`entry`) e o namespace raiz.
