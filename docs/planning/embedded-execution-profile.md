# Plano de implementação — perfil embarcado e freestanding

> **Status:** aprovado como expansão de longo prazo.  
> **Prioridade:** posterior à Host ABI básica e value types; `@cfg` já foi
> concluído.  
> **Baseline:** o runtime atual depende de SO, libc, heap, atomics e threads; o
> driver ainda gera código nativo para o host. `--target` já seleciona fatos
> cfg e artefatos, mas não oferece cross-compilation completa.

## 1. Resultado desejado

Permitir que um subconjunto documentado de Ori rode em targets sem filesystem,
rede, processo ou executor completo, preservando tipagem e a mesma sintaxe.

Não será “outra Ori”. Será um perfil com capabilities e runtime menores.

## 2. Pré-requisitos

- `LANG-CFG-1` para seleção previsível por target/profile — **concluído**;
- `VALUE-PERF-1` para aggregates inline e arrays úteis;
- `EMBED-HOST-1` para allocator/providers/capabilities explícitos;
- ABI/target metadata separando pointer width, endianness e atomics.

## 3. Estado real

- allocations usam `malloc/realloc/free` diretamente;
- ARC global usa atomics e registros sincronizados;
- stdlib inclui módulos que assumem processo, filesystem, rede e relógio;
- não há allocator injetável;
- não há acesso público seguro a volatile/MMIO, interrupts ou seções de link;
- fixed arrays aceitam somente elementos inline compatíveis;
- C/debug não é uma rota freestanding de produto.

## 4. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **EMBEDDED-1.0** | tiers e targets de referência | MCU/board e budgets explícitos; sem promessa genérica vazia |
| **EMBEDDED-1.1** | `--target` e target spec | object correto sem consultar propriedades do host |
| **EMBEDDED-1.2** | `core` sem OS | programa escalar/array compila sem runtime de processo |
| **EMBEDDED-1.3** | allocator provider | heap opcional, falha tipada e zero chamadas libc diretas no perfil |
| **EMBEDDED-1.4** | memória gerenciada configurável | single-thread ARC ou ausência de managed types conforme profile |
| **EMBEDDED-1.5** | volatile/MMIO seguro | wrappers limitam largura, alinhamento e mutabilidade |
| **EMBEDDED-1.6** | interrupts/startup/link | ABI, sections e entrypoint documentados por target |
| **EMBEDDED-1.7** | CI/emulador/hardware smoke | build reproduzível e teste em emulador; hardware quando disponível |

## 5. Segurança e previsibilidade

- APIs de MMIO ficam atrás de módulos explicitamente `unsafe`/capability;
- não alocar em interrupt handler sem contrato específico;
- panic policy é configurada e não assume stderr;
- tamanho de stack, heap e binário entra nos gates;
- target não suportado falha cedo com diagnostic estruturado.

## 6. Fora de escopo inicial

- suportar qualquer microcontrolador sem target de referência;
- garbage collector completo em targets sem atomics;
- filesystem/rede/processo falsos;
- abstrações de GPIO, sensor ou RTOS como palavras da linguagem.
