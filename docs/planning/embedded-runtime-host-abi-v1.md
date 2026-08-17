# Plano de implementação — runtime hospedado e Host ABI v1

> **Status:** implementação incremental; a primeira fundação Rust está disponível, mas o contrato C final ainda exige decisão por fatia.  
> **Baseline verificada:** workspace `0.3.8-dev`, `ori-native-abi-1`.  
> **Escopo:** embedding nativo geral para hosts C/C++, ferramentas, plugins e
> aplicações interativas. Não pertence a uma engine específica.  
> **Fonte normativa atual:** [`../spec/19-abi.md`](../spec/19-abi.md). Este plano
> não altera a ABI até que uma fase seja implementada, testada e incorporada à
> especificação.

## 1. Resultado desejado

Um host deve conseguir carregar e executar Ori sem correr o risco de uma falha
controlada da linguagem encerrar todo o processo hospedeiro.

O caminho final deve permitir:

```text
host
  → cria contexto Ori
  → carrega/inicializa módulo
  → registra callbacks e APIs permitidas
  → chama exports ou funções resolvidas
  → recebe sucesso, erro, trap ou cancelamento
  → libera handles e encerra o contexto
```

O contrato continua baseado em ABI C. APIs de C++, Rust, Python ou engines são
adaptadores externos sobre essa fronteira.

## 2. Estado real antes deste plano

| Capacidade | Estado atual |
|---|---|
| `ori compile --lib` sem `main` | implementado |
| `@c_export` e header C gerado | implementado para o escopo ABI-1 |
| Números, `bool`, `string` e structs escalares | implementado |
| Structs gerenciadas como handles ARC opacos | implementado |
| `optional` / `result` diretos | implementado dentro das restrições da ABI-1 |
| `ori_rt_init` / `ori_rt_shutdown` | implementado; shutdown é best-effort |
| Consulta de versão do runtime pelo host | implementado (`ori_rt_version` / `ori_rt_abi_version`) |
| Callback host → Ori no caminho `ori-embed` | parcial: inteiro, Rust, com `user_data` |
| Reentrância host → Ori → host documentada | parcial: síncrona e limitada a 64 níveis |
| `bytes` / buffer por ponteiro + comprimento | ausente no `@c_export` direto |
| Diagnósticos estruturados para o host | DTO Rust experimental em `ori-embed`; ABI C versionada ainda ausente |
| Tipo opaco pertencente ao host | não há declaração pública; APIs usam escalares ou wrappers manuais |
| Falhas recuperáveis | primeira fatia Rust para contracts, `check`, divisão inteira e bounds diretos de escalares; aborts arbitrários ainda não |
| Budget para código síncrono | ausente |
| Smoke genérico de host | Linux cobre o caminho atual; matriz completa ausente |

O caminho Rust experimental já possui um `OriHostRegistry` para símbolos
`extern host` escalares usados pelo JIT. A primeira fatia de callback agora
adiciona `user_data` opaco, IDs estáveis, dispatchers tipados por aridade,
unregister que falha enquanto há chamada ativa e reentrada síncrona limitada.
Isso ainda não é o callback Host ABI C: afinidade de thread, cancelamento
cooperativo, callbacks de aggregates/managed e integração com `--lib` continuam
fora do contrato.

### Primeira fatia implementada (2026-08-13)

O crate `compiler/crates/ori-embed` fornece a primeira fronteira Rust para um
host nativo. `OriConfig` seleciona target, execution profile e features
declaradas; `OriEngine` mantém módulos lógicos, devolve diagnostics copiados
para DTOs próprios, compila um JIT persistente escalar e só publica uma nova
`ModuleGeneration` quando a fonte é compilada (`compile_source`). Uma
atualização inválida preserva a última geração válida; uma substituição válida
torna handles antigos obsoletos. `check_source` é puramente não-destrutivo:
valida um candidato sem avançar a geração nem tocar o executável corrente, e
registra apenas o source aceito para inspeção (`module_source`).

Essa fatia ainda é experimental e não é o Host ABI C: as chamadas aceitam
somente funções públicas com argumentos homogêneos `bool`/`int`/`float`/`slice`/`string`/`bytes` (até
quatro) e retorno escalar, `void`, `slice`, `string` ou `bytes`. Desde 2026-08-17, funções públicas também
podem retornar e receber `slice[T]`, `string` e `bytes`: o host recebe
`OriValue::Slice`, `OriValue::String` ou `OriValue::Bytes` com accessors seguros (`as_str()`,
`as_bytes()`); ponteiros de slice/string/bytes nunca são registráveis como funções ou
callbacks do host. Além disso, `extern host` pode usar a
fatia Rust de callbacks inteiros com até quatro parâmetros e `user_data` opaco.
O modo hospedado captura por retorno
explícito contracts, `check`, guards de divisão inteira e bounds diretos de
listas/texto/bytes; ele não intercepta qualquer abort interno de um runtime
helper. A fatia de callbacks não cobre C header, afinidade de thread,
cancelamento cooperativo ou migração durante reload. O
`unload_module` já libera explicitamente as gerações retidas, `unload_all`
libera todas mantendo a sessão utilizável, e `modules`/`module_source`/
`functions` permitem inspecionar o estado da sessão; frames e tasks
ativos ainda não têm coordenação. Esses limites continuam pertencendo às fases
seguintes do plano.

O plano original de shared library permanece em
[`PLANO-CDYLIB-EMBED.md`](PLANO-CDYLIB-EMBED.md). Ele documenta a fundação já
entregue; este arquivo é canônico para a evolução do contrato hospedado.

## 3. Casos de uso mínimos

As primeiras fases devem atender três hosts de referência:

1. **Host C pequeno:** carrega uma biblioteca, chama funções e trata uma trap.
2. **Ferramenta interativa:** recompila código, apresenta diagnósticos e mantém
   o processo principal vivo após erro do módulo.
3. **Aplicação com callback:** registra uma função do host com `user_data`, Ori
   a chama, e ownership/reentrância são testados.

Esses casos cobrem engines, plugins, GUI, servidores extensíveis e automação
sem acoplar o compilador a nenhum deles.

## 4. Modelo de confiança

### Fase inicial: código nativo confiável

O primeiro Host ABI é para código Ori e bibliotecas nativas confiáveis. Ele
deve impedir que erros controlados da Ori derrubem o host, mas não promete
sandbox contra:

- ponteiro inválido recebido por FFI;
- corrupção de memória numa biblioteca C;
- `extern c` malformado;
- syscall ou chamada nativa bloqueante;
- comportamento indefinido do host.

### Código não confiável

Capabilities, budgets e validação de imports são defesa em profundidade. Um
plugin realmente não confiável precisa de isolamento de processo ou fronteira
equivalente. A documentação nunca deve chamar apenas um allowlist de imports de
“sandbox”.

## 5. Contrato alvo

Os nomes abaixo são ilustrativos até a fase correspondente ser aprovada:

```c
typedef struct OriRuntime OriRuntime;
typedef struct OriModule OriModule;

typedef enum OriExecutionStatus {
    ORI_EXECUTION_SUCCESS,
    ORI_EXECUTION_RUNTIME_ERROR,
    ORI_EXECUTION_TRAP,
    ORI_EXECUTION_CANCELLED
} OriExecutionStatus;
```

O desenho final deve decidir se evolui `ori_rt_init` de forma aditiva ou cria
um contexto explícito. Um contexto é preferível para:

- dois hosts Ori independentes no mesmo processo;
- providers de tempo, random e ambiente;
- capabilities por contexto;
- handler de traps e diagnósticos;
- contabilidade de handles e shutdown verificável.

Não se deve expor `DefId`, structs Rust internas, ponteiros mutáveis do HIR ou
layouts privados das coleções.

## 6. Fases de implementação

| ID | Entrega | Dependências | Critério observável |
|---|---|---|---|
| **EMBED-HOST-1.0** | Fechar contrato atual + diagnostics DTO | nenhuma | `@repr` inválido é rejeitado; ABI/runtime podem ser consultadas; host recebe diagnostic estruturado; header e spec concordam |
| **EMBED-HOST-1.1** | Resultado estruturado e boundary de traps | 1.0 | **parcial:** bounds diretos, divisão por zero, contract, `check` e `panic` escalar retornam controle; helpers arbitrários e async ainda não |
| **EMBED-HOST-1.2** | Callbacks com `user_data` | 1.1 | **parcial:** callback Rust inteiro é registrado, chamado por Ori, reentra de forma síncrona e tem unregister protegido; callback C, thread dispatch e cancelamento cooperativo continuam abertos |
| **EMBED-HOST-1.3** | Tipos opacos pertencentes ao host | 1.0 | dois handles nativos de domínios distintos não são intercambiáveis em Ori e nenhum layout host é exposto |
| **EMBED-HOST-1.4** | Views/buffers e FFI em batch | 1.1 | host envia e recebe buffers contíguos sem uma chamada por elemento e sem expor layout de `list` |
| **EMBED-HOST-1.5** | Lifecycle, threads e shutdown verificável | 1.2/1.3/1.4 | handles vivos bloqueiam unload com erro; shutdown detecta leaks; chamadas em thread inválida falham de forma definida |
| **EMBED-HOST-1.6** | Capabilities e providers opcionais | 1.5 | contexto nega imports/capacidades não concedidas; tempo/random podem ser injetados |

Cada linha deve ser uma fatia vertical. Não implementar todas em um único PR.

## 7. Boundary de traps

O boundary deve capturar apenas falhas controladas produzidas pelo compilador
ou runtime Ori:

- `panic` Ori;
- acesso fora dos limites;
- divisão inteira inválida;
- contract violado;
- stack guard quando recuperável com segurança;
- cancelamento cooperativo.

Requisitos:

1. mensagem, código, módulo, função e source span quando disponíveis;
2. cleanup de referências e recursos pertencentes ao frame interrompido;
3. estado do runtime conhecido após a trap;
4. nenhuma dependência em texto de stderr para descobrir o erro;
5. modo standalone preserva sua política atual até decisão explícita.

Um salto não local que ignore destrutores ou ARC não é aceitável. A estratégia
precisa de RFC técnico antes de escolher retorno explícito, handler por thread,
landing pads ou outra representação.

### Diagnósticos estruturados

O compilador já possui diagnostics estruturados internamente. O contrato
hospedado deve publicar um DTO próprio e versionado, sem expor structs Rust:

```text
code, severity, message
file, start_line, start_column, end_line, end_column
labels, notes, suggestions
```

O host pode escolher renderização textual, JSON ou UI própria. Texto de stderr
nunca é protocolo. A primeira versão pode entregar uma lista imutável válida
até a próxima operação no contexto; ownership e encoding precisam constar no
header gerado.

## 8. Ownership e handles

O Host ABI deve manter as regras já usadas pelo `@c_export`:

- parâmetros gerenciados são borrowed durante a chamada;
- retornos gerenciados entregam uma referência owned ao host;
- retain/release são explícitos quando o host cria outro dono;
- handles carregam tipo público, mas não layout privado;
- ponteiro arbitrário nunca é aceito como handle Ori válido.

Handles geracionais pertencem ao host/runtime que controla reload. O compilador
não deve impor `index + generation` a toda FFI. Uma biblioteca `SlotMap[T]` pode
oferecer o mesmo padrão para aplicações comuns.

### Tipos opacos pertencentes ao host

Os handles ARC atuais representam valores que pertencem à Ori e são entregues
ao host. Também falta o caminho inverso: um valor cujo conteúdo e lifecycle
pertencem ao host, mas que Ori enxerga como um tipo nominal seguro.

Enquanto esse recurso não existe, um wrapper `@repr("C")` com inteiro é uma
ponte possível, porém deixa o identificador acessível e não impede toda mistura
acidental. A fase **EMBED-HOST-1.3** deve comparar duas opções antes de congelar
sintaxe:

1. declaração Ori explícita de tipo externo/opaco;
2. tipo nominal gerado pelo binding a partir de metadata/manifest.

Em ambos os casos, construção, validação geracional e destruição pertencem à
API host registrada. A linguagem não deve impor um layout `index + generation`
nem liberar memória que não possui.

## 9. Callbacks

### Fatia Rust entregue

`ori-embed::OriHostRegistry::register_int_callback` é a primeira implementação
vertical. A assinatura Ori continua mostrando apenas seus parâmetros normais;
o backend injeta internamente um ID `i64` antes dos argumentos e resolve esse
ID em um dispatcher fixo. O callback nativo recebe:

```text
unsafe extern "C" fn(
    user_data: *mut u8,
    arg0: i64,
    arg1: i64,
    arg2: i64,
    arg3: i64,
) -> i64
```

Somente os primeiros parâmetros declarados são válidos; os demais slots são
zero. `int` e `void` são suportados nesta fatia, até quatro parâmetros. O host
mantém `user_data` vivo até `remove_callback` retornar sucesso. Uma remoção
durante uma chamada ativa retorna `CallbackActive`; chamadas posteriores
retornam um trap estruturado de callback cancelado. Reentrada síncrona na mesma
sessão é permitida, com limite de 64 frames para impedir recursão ilimitada.

O callback é código nativo confiável: ponteiro inválido, panic através de FFI,
bloqueio e acesso concorrente ao estado do host continuam sendo responsabilidade
do host. A implementação não afirma sandbox, não despacha para a thread
principal e ainda não publica esse contrato no header C gerado.

Todo callback público precisa definir:

- assinatura C exata;
- `user_data` opaco;
- quem mantém o callback vivo;
- thread permitida;
- se exige a thread principal/event loop do host e como o dispatch ocorre;
- se chamadas reentrantes são aceitas;
- como uma trap Ori atravessa ou não o callback;
- como unregister concorre com uma chamada ativa.

Callbacks não devem receber objetos internos mutáveis do compilador ou
runtime. Dados complexos atravessam por views, valores escalares ou handles.

## 10. Buffers e chamadas em lote

O objetivo não é expor `OriList`. O alvo é um contrato estável semelhante a:

```text
read-only view  = pointer + length
mutable output  = pointer + capacity + written length
owned buffer    = opaque handle + data/len queries + release
```

As três formas precisam de regras de lifetime, alinhamento, tamanho máximo e
element type. Views não podem sobreviver à chamada sem uma operação explícita
de cópia ou retenção.

## 11. Compatibilidade

- Adições de símbolos podem permanecer em `ori-native-abi-1` quando antigos
  binários continuam válidos.
- Mudança de layout, assinatura ou ownership exige novo tag ABI.
- O host deve consultar a ABI em runtime antes de carregar o módulo.
- Headers gerados devem carregar a versão do schema e os requisitos mínimos.
- Pelo menos um host de compatibilidade da versão anterior permanece na CI.

## 12. Áreas de implementação afetadas

| Área | Caminhos principais |
|---|---|
| Runtime | `compiler/crates/ori-runtime/src/lib.rs` |
| Checker de exports | `compiler/crates/ori-types/src/check.rs` |
| HIR / wrappers | `compiler/crates/ori-hir/src/lower.rs` |
| Native codegen | `compiler/crates/ori-codegen/src/native_backend.rs` |
| Header C | `compiler/crates/ori-codegen/src/c_header.rs` |
| Driver `--lib` | `compiler/crates/ori-driver/src/pipeline/compile.rs` |
| Smoke | `tools/qa/embed_smoke.sh`, `tests/native/embed_smoke.c` |

## 13. Validação obrigatória

- testes Rust de layout e lifecycle;
- regressões `ori-driver` para cada assinatura aceita/rejeitada;
- harness C real, sem duplicar manualmente o header gerado;
- ASan/LSan ou Valgrind no harness quando disponível;
- teste de chamadas concorrentes e unregister durante uso;
- diagnostics com múltiplos labels/spans consumidos sem parsear stderr;
- dois tipos opacos host-owned incompatíveis no checker e no header;
- trap em cada categoria mantendo o host vivo;
- um milhão de chamadas escalares e chamadas batch com orçamento registrado;
- smoke com runtime empacotado, sem Cargo/Rust no PATH;
- atualização de Spec 13, 16, 19, Atlas e CHANGELOG em cada fase pública.

## 14. Fora de escopo

- API específica de Godot, Unity, Unreal ou outra engine;
- reflection dinâmica geral;
- layout público de coleções internas;
- recuperação garantida após falha nativa arbitrária;
- sandbox de código hostil dentro do mesmo processo;
- hot reload completo — pertence ao plano do compiler service.
