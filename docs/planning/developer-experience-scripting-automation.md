# Plano de implementação — DX, scripts e automação

> **Status:** aprovado para implementação incremental.  
> **Baseline:** `ori run`, `ori fmt`, `ori test`, `ori repl` e
> `ori.process.run/run_capture` existem em `0.3.8-dev`.  
> **Objetivo:** tornar o caminho de script pequeno tão completo quanto o
> caminho de projeto, sem criar uma segunda linguagem.

## 1. Estado real

- `ori run arquivo.orl` não aceita argumentos após `--` para o programa;
- `ori fmt` formata um arquivo e imprime em stdout, sem `--write` ou `--check`;
- não existe `ori lint`;
- `process.run` e `run_capture` são bloqueantes;
- o resultado capturado é `map[string, string]`;
- não há `cwd`, ambiente, stdin, timeout, streaming, kill ou signal;
- não há contrato de shebang; módulos continuam obrigatórios.

## 2. Direção

O primeiro ganho deve vir de CLI e APIs explícitas, não de remover o cabeçalho
`module`. Um arquivo continua sendo Ori normal; o driver apenas melhora como
argumentos, formatação e processos são controlados.

Modelo alvo para processos:

```text
ProcessOptions
  program, arguments, cwd, environment
  stdin, capture_stdout, capture_stderr
  timeout

ProcessResult
  exit_code, stdout: bytes, stderr: bytes
```

Texto é decodificado explicitamente; captura nativa não deve converter bytes
inválidos com perda silenciosa.

## 3. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **DX-SCRIPT-1.0** | `ori run file -- args...` | JIT e AOT observam os mesmos `ori.args` |
| **DX-SCRIPT-1.1** | `fmt --write/--check` + diretórios | CI detecta drift sem reimplementar formatter |
| **DX-SCRIPT-1.2** | `ori lint` MVP | regras têm IDs, severidade, spans e supressão explícita |
| **DX-SCRIPT-1.3** | `ProcessResult` e `ProcessOptions` | cwd/env/stdin/bytes funcionam sem map mágico |
| **DX-SCRIPT-1.4** | processo longo | spawn, wait, timeout, kill e streaming têm lifecycle definido |
| **DX-SCRIPT-1.5** | modo shebang, se aprovado | execução direta preserva módulo e diagnostics previsíveis |

## 4. Regras de lint iniciais

- binding não usado;
- resultado descartado quando a API exige tratamento;
- condição `@cfg` redundante ou impossível, quando o lint puder prová-la sem
  depender de ambiente arbitrário;
- `@inline`/`@no_inline` sem efeito enquanto o optimizer não os consumir;
- comparação ou conversão redundante comprovável sem inferência global.

Lint não duplica erros do checker e não muda significado do programa.

## 5. Áreas afetadas

| Área | Caminhos |
|---|---|
| CLI | `compiler/crates/ori-driver/src/main.rs` |
| Formatter | `compiler/crates/ori-driver/src/pipeline/fmt.rs` |
| Args/JIT | `compiler/crates/ori-driver/src/pipeline/native.rs`, runtime |
| Processos | `ori-types/src/stdlib.rs`, `ori-runtime/src/lib.rs`, `stdlib/process.orl` |
| LSP | code actions e diagnostics de lint |

## 6. Validação

- argumentos vazios, Unicode e iniciados por hífen;
- paridade `run` JIT/AOT e binário compilado;
- formatter idempotente, check exit code e árvore multifile;
- processos com bytes não UTF-8, timeout, cwd, env e cleanup;
- Windows/Linux/macOS para quoting e termination;
- documentação EN/PT e exemplos canônicos executáveis.

## 7. Fora de escopo

- linguagem de shell embutida;
- interpolação que execute comandos implicitamente;
- esconder quoting do SO dentro de uma string única;
- tornar tipos opcionais em scripts.
