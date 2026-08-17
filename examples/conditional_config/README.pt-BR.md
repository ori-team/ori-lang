# Configuração condicional

Este exemplo declara a feature `telemetry` e a ativa por padrão. Ele também
seleciona uma pequena declaração específica de plataforma pelos fatos de
target.

```bash
ori run examples/conditional_config
ori run examples/conditional_config --no-default-features
ori check examples/conditional_config --execution-profile embedded
```

Os dois ramos são parseados, mas somente a declaração ativa entra na resolução
de nomes, checagem de tipos, documentação e geração de código.

English: [README.md](README.md).
