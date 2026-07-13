# Cookbook para projetos pequenos e medios

Status: guia pratico para Ori `0.2.x`.

Este cookbook junta tarefas comuns que um usuario deve conseguir fazer sem ler
o codigo do compilador.

## CLI que le argumentos

```lua
namespace app.main

import ori.args as args
import ori.io as io

func main()
    const name: string = args.get_or(1, "Ori")
    io.println("ola, " + name)
end
```

Comando:

```bash
ori run src/main.orl
```

## Ler configuracao local

```lua
namespace app.main

import ori.config as config
import ori.io as io

func main()
    const text: string = config.read_text_or("app.conf", "modo=dev")
    io.println(text)
end
```

Use `config.read_json(path)` quando o arquivo precisar ser JSON estruturado.

## Trabalhar com arquivos

```lua
namespace app.main

import ori.fs as fs
import ori.io as io

func main()
    const written = fs.write_text("out.txt", "feito")
    const text = fs.read_text("out.txt")
    match text
        case success(value):
            io.println(value)
        case error(message):
            io.eprintln(message)
    end
end
```

## Medir tempo

```lua
namespace app.main

import ori.io as io
import ori.time as time

func main()
    const start: time.Instant = time.instant_now()
    time.sleep_duration(time.duration_millis(10))
    const elapsed: time.Duration = time.elapsed_since(start)
    io.println(string(time.duration_to_millis(elapsed)))
end
```

## Usar pacote local

No `ori.proj` do app:

```ini
[dependencies]
demo.math = { path = "../math", version = "0.1.0" }
```

No codigo:

```lua
import demo.math only (double)
```

Depois rode:

```bash
ori check ori.proj
ori test src/main.orl
```

## Gerar documentacao HTML

```bash
ori doc file ori.proj --format html --out docs/api/index.html
ori doc check ori.proj
```

Use `.oridoc` para descricoes longas e mantenha comentarios inline apenas para
contratos curtos.
