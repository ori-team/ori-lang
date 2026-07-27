# Cookbook — projetos pequenos e médios

> Status: receitas práticas Ori **S3 / 0.3.2**  
> **English:** [cookbook.md](cookbook.md)

## CLI com argumentos

```ori
module app.main

import ori.args = args
import ori.io = io

main()
    const name = args.get_or(1, "Ori")
    io.println("olá, " + name)
end
```

```bash
ori run main.orl
```

## Configuração local

```ori
module app.main

import ori.config = config
import ori.io = io

main()
    const text: string = config.read_text_or("app.conf", "modo=dev")
    io.println(text)
end
```

Use `config.read_json(path)` para JSON estruturado (`result` / aliases de domínio).

## Arquivos

```ori
module app.main

import ori.fs = fs
import ori.io = io

main()
    match fs.write_text("out.txt", "pronto")
        case ok(_):
            match fs.read_text("out.txt")
                case ok(value):
                    io.println(value)
                case err(message):
                    io.eprintln(message)
            end
        case err(message):
            io.eprintln(message)
    end
end
```

Helpers: `fs.read_text_or`, `fs.write_text_result`; aliases `TextResult` via
`import ori.fs (TextResult, …)`.

## Medir tempo

```ori
module app.main

import ori.io = io
import ori.time = time

main()
    const start: time.Instant = time.instant_now()
    time.sleep_duration(time.duration_millis(10))
    const elapsed: time.Duration = time.elapsed_since(start)
    io.println(string(time.duration_to_millis(elapsed)))
end
```

## Pacote local

No `ori.proj` do app:

```ini
[dependencies]
demo.math = { path = "../math", version = "0.1.0" }
```

```ori
import demo.math (double)
```

```bash
ori check main.orl
ori test main.orl
```

## Export de documentação

```bash
ori doc file main.orl --format html --out docs/api/index.html
ori doc check .
```

## HTTP (STDLIB-2)

```ori
module app.main

import ori.io = io
import ori.net.http = http

main()
    match http.get_plain("127.0.0.1", 8080, "/", 3000)
        case ok(resp):
            io.println(string(resp.status))
            io.println(resp.body)
        case err(msg):
            io.eprintln(msg)
    end
end
```

TLS: `http.get_tls("example.com", "/", 5000)` (precisa de rede).  
Exemplo completo: [`examples/http_get`](../../examples/http_get/).

## Streams de arquivo (STDLIB-3)

```ori
module app.main

import ori.io = io

main()
    match io.open_output("out.txt")
        case ok(out):
            match io.write_text(out, "oi")
                case ok(_):
                    match io.flush(out)
                        case ok(_):
                            io.close_output(out)
                        case err(_):
                    end
                case err(_):
            end
        case err(_):
    end
end
```

## Pipe e inferência local

```ori
module app.main

import ori.io = io
import ori.string = str

main()
    const cleaned = "  oi  " |> str.trim
    io.println(cleaned)
end
```

---

[Tour](../language/tour.pt-BR.md) · [Primeiro projeto](first-project.pt-BR.md) ·
[Erros](errors-null-void.pt-BR.md) · [Exemplos](../../examples/)
