# hello

Minimal Ori project layout (M2.layout):

```text
hello/
  ori.proj
  main.orl
```

```bash
ori check .
ori run .
```

From a compiler build:

```bash
cargo run -p ori-driver -- check examples/hello
cargo run -p ori-driver -- run examples/hello
```
