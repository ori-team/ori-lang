# Linux log report

This medium-sized Linux example reads a line-oriented service log, counts
`INFO`, `WARN`, and `ERROR` records, and prints a report. It demonstrates a
multi-module project, filesystem results, command-line arguments, and a small
regression test.

From the repository root:

```bash
cd compiler
cargo run -p ori-driver -- check ../examples/linux_log_report
cargo run -p ori-driver -- run ../examples/linux_log_report
cargo run -p ori-driver -- test ../examples/linux_log_report/tests.orl
```

Pass a log file to a compiled copy as argument 1 and an optional report path as
argument 2. The checked-in `sample.log` keeps the example deterministic and
does not require access to `/var/log`.
