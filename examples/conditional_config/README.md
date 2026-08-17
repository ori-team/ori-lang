# Conditional configuration

This example declares a `telemetry` feature and enables it by default. It also
selects a small platform-specific declaration through target facts.

```bash
ori run examples/conditional_config
ori run examples/conditional_config --no-default-features
ori check examples/conditional_config --execution-profile embedded
```

Both branches are parsed, but only the active declaration enters name
resolution, type checking, documentation, and code generation.

Portuguese: [README.pt-BR.md](README.pt-BR.md).
