# Apêndice D — Programa em 30 minutos

Roteiro único: dobrar um número com pipe e tratar divisão com `result`.

## Minutos 0–5 — ambiente

```bash
ori --version
ori doctor
ori new double_lab
cd double_lab
```

## Minutos 5–15 — código

Substitua o corpo de `main.orl` por:

```ori
module app.main

import ori.io = io

double(n: int) -> int => n * 2

safe_div(a: int, b: int) -> result[int, string]
    if b == 0
        return err("zero")
    end
    return ok(a / b)
end

main()
    const raw: int = 10
    const n: int = raw |> double
    match safe_div(n, 2)
        case ok(v):
            io.println(f"ok {v}")
        case err(m):
            io.println(f"err {m}")
    end
end
```

## Minutos 15–25 — rodar

```bash
ori check main.orl
ori run main.orl
```

Saída esperada: algo como `ok 10` (20 / 2).

## Minutos 25–30 — estender

1. Troque o denominador por `0` e veja o `err`.  
2. Extraia `safe_div` para outro arquivo e importe (Cap. 15 — lembre do `ori.proj`).  
3. Opcional: leia um inteiro via [`examples/cli_args`](../../../examples/cli_args/).

## Ir mais fundo

- Caps 12, 13, 15  
- [`../../guides/first-project.pt-BR.md`](../../guides/first-project.pt-BR.md)
