# Cookbook — small and medium projects

> Status: practical recipes for Ori **S3 / 0.3.2**  
> **Portuguese:** [cookbook.pt-BR.md](cookbook.pt-BR.md)

## CLI that reads arguments

```ori
module app.main

import ori.args = args
import ori.io = io

main()
    const name = args.get_or(1, "Ori")
    io.println("hello, " + name)
end
```

```bash
ori run main.orl
```

## Local config text

```ori
module app.main

import ori.config = config
import ori.io = io

main()
    const text: string = config.read_text_or("app.conf", "mode=dev")
    io.println(text)
end
```

Use `config.read_json(path)` for structured JSON (`result` / domain aliases).

## Files

```ori
module app.main

import ori.fs = fs
import ori.io = io

main()
    match fs.write_text("out.txt", "done")
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

Helpers: `fs.read_text_or`, `fs.write_text_result`, domain aliases
`TextResult` / `IoResult` via `import ori.fs (TextResult, …)`.

## Measure time

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

## Local package

In the app `ori.proj`:

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

## Documentation export

```bash
ori doc file main.orl --format html --out docs/api/index.html
ori doc check .
```

Use `.oridoc` sidecars for long descriptions; keep inline comments short.

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

TLS: `http.get_tls("example.com", "/", 5000)` (needs network; runtime rustls + ring).  
Full sample: [`examples/http_get`](../../examples/http_get/).  
Also: `build_request`, `parse_response` for manual control.

## File streams (STDLIB-3)

```ori
import ori.io = io

main()
    match io.open_output("out.txt")
        case ok(out):
            match io.write_text(out, "hi")
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
    -- or: using out: ori.io.Output = out_handle  (dispose closes the stream)
end
```

## Pipe and local inference

```ori
module app.main

import ori.string = str
import ori.io = io

main()
    const cleaned = "  hi  " |> str.trim
    io.println(cleaned)
end
```

Pipe `|>` is typed as a normal call. Local `const cleaned = …` may omit the
type when the checker knows the result (option B).

---

See also: [Language tour](../language/tour.md) · [First project](first-project.md) ·
[Errors](errors-null-void.md) · [Examples](../../examples/)
