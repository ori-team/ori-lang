# Glossário

> Termos usados no livro. Definição curta; detalhe na spec ou no capítulo citado.

| Termo | Significado |
|-------|-------------|
| **AOT** | Ahead-of-time: compila para binário nativo antes de executar (`ori compile`) |
| **JIT** | Just-in-time: executa código Cranelift in-process (`ori run` quando há cdylib) |
| **S3** | Superfície de sintaxe canônica desde `0.3.0` (estilo poema / Auk9-inspired) |
| **HIR** | High-level IR — representação intermediária após resolução de nomes |
| **ARC** | Automatic Reference Counting — gestão de memória do runtime Ori |
| **Diagnostic** | Mensagem de erro/aviso com código estável (ex. `name.undefined`) |
| **Stdlib Layer 1** | Funções hot-path implementadas no runtime Rust (FFI) |
| **Stdlib Layer 2/3** | Módulos `.orl` na pasta `stdlib/` |
| **ABI** | Application Binary Interface — contrato nativo (`ori-native-abi-1`) |
| **FREEZE-1** | Política de estabilidade em 0.3.x (mudança ainda possível, com disciplina) |
| **Pipe `\|\>`** | Operador que passa o valor à esquerda como primeiro argumento da chamada |
| **Inferência local (opção B)** | Omitir tipo em `const`/`var` local quando o RHS é óbvio |
| **Trait** | Contrato de comportamento; em Ori via `apply Type` + `use Trait` |
| **optional[T]** | Valor presente (`some`) ou ausente (`none`) — sem null |
| **result[T, E]** | Sucesso (`ok`) ou falha (`err`) |
| **using** | Escopo que libera recurso de forma determinística e visível |
| **Auk9** | Lab de sintaxe arquivado; a pele vivente está na Ori S3 |
