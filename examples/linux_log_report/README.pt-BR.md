# Relatório de logs Linux

Este exemplo de tamanho médio lê um log de serviço linha a linha, conta os
registros `INFO`, `WARN` e `ERROR` e imprime um relatório. Ele demonstra um
projeto com vários módulos, resultados de operações de arquivo, argumentos de
linha de comando e um teste de regressão pequeno.

A partir da raiz do repositório:

```bash
cd compiler
cargo run -p ori-driver -- check ../examples/linux_log_report
cargo run -p ori-driver -- run ../examples/linux_log_report
cargo run -p ori-driver -- test ../examples/linux_log_report/tests.orl
```

Passe um arquivo de log como primeiro argumento de uma cópia compilada e um
caminho opcional de saída como segundo argumento. O `sample.log` versionado
mantém o exemplo determinístico e não exige acesso a `/var/log`.
