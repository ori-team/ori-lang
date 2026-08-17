# Plano de implementação — compiler service e JIT modular

> **Status:** implementação incremental; a fachada Rust já possui check/diagnostics, compilação JIT persistente escalar e handles geracionais.  
> **Baseline verificada:** JIT Cranelift de programa completo e incremental AOT
> por arquivo em `0.3.8-dev`.  
> **Escopo:** API longa duração para check, metadata, compilação e substituição
> segura de módulos. Não é uma nova linguagem nem uma engine.

## 1. Resultado desejado

Ferramentas e hosts interativos não devem reiniciar todo o compilador para cada
edição nem guardar ponteiros de código sem saber quando ficaram inválidos.

Fluxo alvo:

```text
CompilerSession
  ├─ adiciona/atualiza fontes
  ├─ resolve grafo e publica diagnostics
  ├─ compila módulos afetados
  ├─ produz ModuleHandle com geração
  ├─ resolve FunctionHandle
  └─ substitui módulo e invalida handles antigos
```

## 2. Estado real antes deste plano

### Já existe

- pipeline modular de lexer → parser → resolver → checker → HIR;
- `run_check_source` para fonte em memória;
- índice semântico de projeto usado pelo LSP;
- `.ori/incremental.json` e objetos AOT por arquivo;
- fingerprint de interface pública;
- JIT Cranelift in-process usado por `ori run`;
- `ori repl`, que recompõe uma fonte temporária e executa um JIT completo.

### Ainda não existe

- sessão pública de compilação com check/diagnostics, gerações de módulo e
  execução escalar (a fundação já existe em `ori-embed`);
- atualização incremental de HIR/codegen por módulo no JIT;
- reload concorrente e migração de estado; unload explícito de módulo já existe
  na sessão Rust experimental;
- registry de símbolos `extern host` com callbacks inteiros, `user_data`,
  unregister protegido e reentrada síncrona limitada; o contrato C/thread
  permanece ausente;
- chamada genérica validada por assinatura para agregados/managed;
- source mapping por endereço JIT;
- política para tarefas, closures e handles vivos durante reload;
- budget de execução para loops síncronos;
- API C estável do compiler service.

O JIT atual compila o HIR inteiro, busca `main`, executa e destrói o
`JITModule`. Ele não deve ser descrito como hot reload incremental.

### Fatia implementada (2026-08-13)

`ori-embed::OriEngine` recebe atualizações de fonte, devolve diagnostics
estruturados, compila um `JITModule` persistente e publica handles opacos
amarrados a `ModuleId + ModuleGeneration`. Funções públicas sem `main` podem
ser chamadas com `bool`, `int` ou `float` homogêneos, no máximo quatro
argumentos, e retorno escalar ou `void`. Uma substituição válida retém o
módulo anterior e torna seus handles obsoletos; uma atualização inválida deixa
a geração atual chamável.

O `OriHostRegistry` também valida e injeta endereços nativos para declarações
`extern host` escalares. O endereço é resolvido na finalização do JIT e fica
em cache no módulo. A fatia adicional de callbacks inteiros injeta um ID
oculto, retém `user_data` no registry, rejeita unregister durante uma chamada
ativa e permite reentrada síncrona na mesma sessão com limite de profundidade.
Ela continua restrita ao Rust experimental e não coordena thread affinity,
tasks ou migração de callbacks durante reload.

O baseline `BRASA-ORI-CALL-001` está disponível em
`tools/bench/hosted_scalar_calls.sh`. Ele mede um milhão de chamadas através
do mesmo handle e imprime tempo total, ns/call e checksum; o resultado é
comparativo por máquina e não é um limite de CI.

Esta não é ainda uma API genérica de execução nem hot reload completo. Não há
reuso incremental de HIR, reload concorrente, source mapping, budget, callbacks
de C/managed, tarefas,
closures, aggregates/managed values ou ABI C para o compiler service. Traps
escalares controlados já retornam por um slot de erro estruturado; aborts
arbitrários de helpers, async traps e uma política de budget ainda não fazem
parte da superfície. A API escalar é experimental e existe para validar
lifetimes e a política de gerações antes de ampliar o contrato.

## 3. Princípios

1. **Uma sessão, uma fonte de verdade.** LSP, REPL, host e testes reutilizam o
   mesmo modelo de projeto em vez de criarem índices paralelos.
2. **Handles opacos.** Nenhuma API pública expõe `DefId`, `FuncId` Cranelift ou
   ponteiro para structs Rust internas.
3. **Gerações explícitas.** Um reload invalida handles antigos de forma
   detectável.
4. **Diagnósticos antes de execução.** Módulo com erro não substitui a geração
   válida atual.
5. **Atomicidade observável.** O host vê a geração antiga ou a nova; nunca uma
   mistura parcialmente ligada.
6. **Sem promessa de unload imediato.** Código ainda referenciado por frame,
   closure ou task precisa permanecer vivo ou bloquear a substituição.

## 4. Handles e identidade

Modelo conceitual:

```text
SessionId
ModuleId
ModuleGeneration
FunctionId
FunctionHandle = SessionId + ModuleId + Generation + FunctionId
```

Regras:

- `ModuleId` identifica o módulo lógico enquanto a sessão existir;
- `Generation` aumenta somente após substituição bem-sucedida;
- `FunctionId` não é um endereço nativo;
- `call(handle)` valida sessão, geração e assinatura;
- handle antigo retorna `stale_handle`, não executa memória descarregada;
- endereço nativo bruto, se um dia exposto para hosts confiáveis, tem lifetime
  explícito e perde as garantias de geração.

## 5. Unidade de invalidação

O serviço deve reutilizar a lógica já existente de fingerprints:

| Alteração | Invalidação mínima pretendida |
|---|---|
| Corpo privado | módulo alterado; consumidores mantidos quando interface não muda |
| Constante pública usada em CT-0 | módulo e dependentes semânticos |
| Assinatura pública | consumidores diretos e especializações relacionadas |
| Layout público | consumidores, FFI/header e debug metadata |
| Manifest/lockfile | grafo de dependências afetado |
| Opções/target/runtime ABI | artefatos incompatíveis da sessão |

O primeiro release pode ser mais conservador. Ele deve registrar por que um
módulo foi invalidado para que otimizações futuras sejam mensuráveis.

## 6. Reload e estado vivo

O ponto mais perigoso não é recompilar; é substituir código ainda em uso.

Precisam de política explícita:

- frame síncrono dentro do módulo antigo;
- future suspensa;
- closure com função ou ambiente do módulo antigo;
- callback registrado no host;
- global com layout alterado;
- handle gerenciado retornado ao host;
- custom destructor definido na geração antiga.

Primeira política recomendada:

1. substituição só entra quando não há frame ativo do módulo;
2. tasks/futures antigas bloqueiam unload ou são canceladas cooperativamente;
3. globals não são migrados automaticamente;
4. mudança de layout reinicializa estado somente após confirmação do host;
5. migração automática de objetos fica fora da primeira versão.

## 7. Fases de implementação

| ID | Entrega | Dependências | Critério observável |
|---|---|---|---|
| **COMP-SVC-1.0** | Fachada de sessão para check + diagnostics | metadata/diagnostics estruturados | LSP e teste em memória usam sessão sem regressão |
| **COMP-SVC-1.1** | Grafo persistente e invalidação explicada | 1.0, incremental atual | body edit não reprocessa consumidores sem mudança de interface |
| **COMP-SVC-1.2** | JITModule persistente e lookup tipado | Host ABI básico | **parcial:** módulo sem `main` é compilado e função escalar pública é chamada por handle |
| **COMP-SVC-1.3** | Gerações e substituição atômica | 1.2 | **parcial:** erro mantém geração antiga; sucesso invalida handle antigo; unload explícito existe, mas concorrência e frames ativos ainda não são tratados |
| **COMP-SVC-1.4** | Frames/tasks/closures e unload seguro | 1.3 | reload concorrente nunca chama código descarregado |
| **COMP-SVC-1.5** | Source mapping, budget e cancelamento | 1.4 | loop instrumentado pode ser interrompido e stack aponta para Ori |
| **COMP-SVC-1.6** | API pública C do serviço | 1.5 | host fora do workspace usa somente header/version contract |

O compiler service deve começar como API interna Rust. Congelar uma API C antes
de provar os lifetimes tornaria cada correção futura uma quebra de ABI.

## 8. Execution budget

Budget é cooperativo. Safe points podem ser inseridos em:

- backedges de loops;
- calls selecionadas;
- await/suspend;
- alocações ou lotes de alocação;
- boundaries de host call.

O custo precisa ser configurável. Standalone não deve pagar instrumentação de
embedding quando o perfil não a solicita.

Um budget não interrompe com segurança uma função C bloqueante. O host precisa
de timeout próprio ou isolamento para essa classe.

## 9. Source mapping e stack traces

A sessão deve manter a relação:

```text
endereço JIT
  → geração do módulo
  → função Ori
  → arquivo
  → linha/coluna
```

Quando a geração é descarregada, seu mapa pode permanecer como registro
diagnóstico, mas nunca como endereço chamável.

## 10. Áreas afetadas

| Área | Caminhos principais |
|---|---|
| Frontend/driver | `compiler/crates/ori-driver/src/pipeline/` |
| Resolver/HIR | `compiler/crates/ori-hir/src/`, `compiler/crates/ori-types/src/` |
| Incremental | `compiler/crates/ori-driver/src/incremental.rs` |
| JIT | `compiler/crates/ori-codegen/src/native_backend/jit.rs` |
| Native backend | `compiler/crates/ori-codegen/src/native_backend.rs` |
| LSP | `compiler/crates/ori-lsp/src/` |
| Debugger | `compiler/crates/ori-driver/src/debugger.rs` |
| Runtime | `compiler/crates/ori-runtime/src/lib.rs` |

## 11. Métricas mínimas

- cold session start;
- warm no-op check;
- edição de corpo privado;
- edição de assinatura pública;
- edição em módulo folha e módulo central;
- quantidade de módulos relidos, rechecados e recompilados;
- tempo para substituir geração;
- memória mantida por gerações antigas;
- custo de `call(handle)`;
- overhead de safe point com budget desligado e ligado.

## 12. Testes obrigatórios

- atualização de fonte válida/inválida;
- cycles e erros de import na sessão;
- handle de sessão errada, função errada e geração antiga;
- reload durante call ativa;
- closure/task/future mantendo geração viva;
- mudança de layout de global;
- source map após múltiplos reloads;
- budget, cancellation e deadline;
- stress com centenas de edições;
- leak check de código, metadata e handles antigos;
- compatibilidade AOT/JIT para o mesmo programa.

## 13. Fora de escopo inicial

- migração automática de heap entre layouts;
- serialização universal de globals;
- patch de instruções enquanto a função executa;
- debugger específico de uma engine;
- sandbox de código nativo não confiável;
- tornar todo projeto Ori dinamicamente recarregável por padrão.
