# Bootstrapping do Ori

> **Público:** contribuidores que compilam o projeto a partir do código-fonte
> **English:** [bootstrapping.md](bootstrapping.md)

Usuários finais devem preferir o [pacote de instalação](../install.pt-BR.md).
Como o compilador ainda é escrito em Rust, o bootstrap de desenvolvimento usa
Rust 1.95, um compilador C/CRT do sistema e os scripts de staging do runtime.

```bash
cd compiler
cargo build -p ori-runtime --lib --release
cargo build -p ori-driver --release
```

Depois, use `tools/stage_native_runtime.sh` ou o equivalente PowerShell para
copiar staticlib e cdylib para `runtime/<triple>/`. O pacote final contém o
executável, `stdlib/`, runtime e, quando necessário, `rust-lld` empacotado. O
usuário final não precisa de `cargo` nem `rustc`.

O self-hosting é uma etapa futura (M4), não um requisito para instalar ou usar
Ori. A definição de ABI está em [19-abi.md](../spec/19-abi.md).
