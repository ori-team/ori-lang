# Relatorio de fechamento - correcao da implementacao da linguagem

Data: 2026-05-12

## Corrigido

- Execucao nativa voltou a falhar cedo quando o runtime C embutido nao compila.
- Warnings e ruido de debug removidos dos caminhos auditados.
- Stdlib, parser, lexer, strings, bytes, f-strings e exemplos foram alinhados com a spec atual.
- Regras semanticas obrigatorias foram reforcadas:
  - mutabilidade de `const` e `var`;
  - shadowing no mesmo escopo;
  - retorno obrigatorio;
  - `using` exige `Disposable`;
  - metodo `mut` nao pode ser chamado por binding imutavel.
- Backend C deixou de gerar codigo silenciosamente errado para:
  - `is`;
  - indice de lvalue nao suportado;
  - `for` sobre iteravel sem suporte.
- Documentacao de memoria agora diz o estado real:
  - ARC basico existe;
  - cycle collector ainda nao existe;
  - C backend de debug nao e ARC-completo.
- Exemplos oficiais em `examples/*.orl` agora passam no type-check e sao cobertos por teste automatizado.

## Backlog futuro fora deste plano

- Cycle collector real para ARC.
- ARC completo no backend C de debug.
- Destrutores completos para todas as formas de alocacao gerenciada no backend nativo.
- Revisao futura de placeholders fora do escopo critico ja corrigido, principalmente em docs historicas e invariantes internas do parser.

Esses itens nao ficaram como checkbox aberto no plano atual. Eles foram
classificados como trabalho futuro porque exigem novas decisoes de escopo e
implementacao propria.

## Comandos que passaram

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo test -p ori-driver`
- `cargo test -p ori-codegen`
- `cargo test -p ori-runtime arc_ -- --nocapture --test-threads=1`
- `cargo test -p ori-driver check_official_examples -- --nocapture --test-threads=1`
- `cargo test -p ori-driver compile_runs_using_dispose_on_native_scope_exit -- --nocapture --test-threads=1`

## Comandos que falharam

Nenhum comando final permanece falhando.
