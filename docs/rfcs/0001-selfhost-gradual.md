---
id: RFC-0001
title: Self-host gradual do compilador Ori
status: accepted
authors: [raillen]
created: 2026-09-06
updated: 2026-09-06
target_version: undecided
related_adrs: [ADR-0005]
related_issues: []
---

# RFC-0001: Self-host gradual do compilador Ori

## Resumo e decisão

**Projeto aceito pelo usuário em 2026-09-06; implementação não concluída.** A autorização imediata cobre planejamento e Marco A, sem iniciar lexer. A versão permanece **0.3.8**. [Plano e evidências](../plans/active/selfhost-exec-plan.md).

Ori passará gradualmente a implementar seu compilador: frontend, resolução, tipos, HIR, mid-end e lowering. O runtime continua Rust; Cranelift continua responsável pela geração nativa. A ponte recomendada é um processo Rust com protocolo versionado, ainda não implementado. Aceitar esta direção não aprova um protocolo ainda inexistente nem declara maturidade de produção.

## Glossário

- **Self-host:** compilador cuja implementação principal está na linguagem que compila; não significa ausência de componentes Rust.
- **Bootstrap:** construir um compilador usando outro compilador previamente confiável.
- **Stage0:** compilador Rust de referência, fixado por commit e entradas verificáveis.
- **Stage1:** compilador Ori compilado pelo stage0.
- **Stage2:** mesma fonte Ori compilada pelo stage1.
- **Stage3:** mesma fonte Ori compilada pelo stage2.
- **AST:** árvore que representa a sintaxe do programa.
- **Resolver:** associa nomes às definições corretas.
- **HIR:** representação intermediária tipada, anterior à geração nativa.
- **Mid-end:** verificações e transformações sobre representações intermediárias.
- **Lowering:** tradução entre representações com contratos progressivamente mais próximos da máquina.
- **ABI:** contrato de símbolos, chamadas, layouts e ownership entre código nativo e runtime.
- **DoD:** evidência necessária para considerar uma tarefa concluída.

## Motivação e comportamento atual

O compilador é um workspace Rust em `compiler/`. A linguagem já possui frontend, resolução, tipos, HIR, Cranelift AOT/JIT e runtime Rust. A mudança permite usar Ori em um programa de sistemas real e testar seus contratos continuamente. Não há evidência de que reescrever o compilador melhore automaticamente desempenho ou correção. ADR-0005 retirou emissão C; isso não fornece uma ponte self-host.

## Objetivos

- Migrar por fases comparáveis à referência Rust e à especificação normativa.
- Tornar entradas, artefatos, diagnósticos e bootstrap auditáveis.
- Preservar recuperação por stage0 e reversão por fase.
- Chegar a stage1, stage2 e stage3 usando a mesma fonte Ori, sem confundir convergência binária com correção.

## Não objetivos

Não reescrever runtime em Ori, substituir Cranelift, criar emissor próprio, ressuscitar backend C, adicionar otimizações extras, prometer prazo/versão, remover Rust do bootstrap ou começar lexer no Marco A. Não alterar sintaxe ou ABI para facilitar a tradução sem processo separado.

## Arquitetura proposta e impacto

1. Fixar stage0 Rust com toolchain, lockfile, target, linker, runtime e corpus.
2. Definir contratos e provar que Ori suporta as estruturas de dados e operações necessárias antes da migração.
3. Migrar carregamento de fontes, lexer/AST/parser, resolução e tipos, preservando fronteiras de fase.
4. Migrar HIR, verificadores, transformações e lowering em fatias diferenciais.
5. Encaminhar representação validada a processo Rust que usa as bibliotecas Cranelift existentes.
6. Integrar CLI e bootstrap, depois ferramentas e rollout controlado.

A fronteira exata da IR serializada será decidida em CONTRACT01/BRIDGE01 após probes. Não há C API de Cranelift assumida. O protocolo precisará declarar versão, capacidades, target, tipos, IDs, spans, ownership lógico, limites, framing, erros e resultados. Representações não podem transportar ponteiros Rust, endereços de heap ou layouts internos sem contrato. O processo deve rejeitar versões incompatíveis, referências inválidas e IR malformada antes de codegen. Um ADR específico registrará a fronteira definitiva antes da implementação.

## Exemplos e casos de borda

Exemplo de bootstrap: stage0 compila a fonte S e produz stage1; stage1 compila S e produz stage2; stage2 compila S e produz stage3. S, flags, runtime e target não mudam entre etapas.

Exemplo válido de intercâmbio futuro: pedido com versão negociada, target suportado e IDs que referenciam definições presentes. Exemplo inválido: referência a tipo ausente ou versão desconhecida; rejeitar com erro estruturado sem gerar objeto parcial. Entrada truncada, tamanho excessivo, saída interrompida e processo morto também precisam de testes. Estes exemplos são requisitos, não comandos disponíveis.

## Regras de tipos e semântica

A especificação continua superior às duas implementações. Preservar inferência local permitida, resolução de imports/visibilidade, traits, generics, monomorfização, ordem de avaliação, overflow, controle de fluxo, cleanup e ARC. Quando a implementação Rust divergir da especificação, classificar defeito; não copiar comportamento acidental como contrato. Cada fase requer positivos, negativos e recuperação de erros. IDs e ordem de diagnósticos precisam ser determinísticos onde contratados.

## Runtime e ABI

Manter `ori-native-abi-1`, símbolos, layouts e single-cascade-owner. Runtime Rust estático e dinâmico precisam vir das mesmas entradas verificadas. AOT/JIT devem concordar na superfície compartilhada e rejeitar explicitamente formas não suportadas. RUNTIME01 é manutenção de compatibilidade, não reescrita. A fronteira de processo separa memória dos compiladores; não elimina vulnerabilidades no runtime ou backend.

## Ferramentas e diagnósticos

CLI deve permitir seleção e fallback explícitos durante experimentação; nomes de flags ainda não estão definidos. LSP, formatter, documentação e debugger devem reutilizar contratos, não duplicar regras. Preservar códigos públicos, spans e ações existentes. Novos erros de bridge exigirão catálogo e testes negativos antes de exposição. Esta RFC não introduz códigos fictícios.

## Compatibilidade e migração

Mudança interna experimental, sem promessa de equivalência até os gates. Programas existentes não migram sintaxe. Distribuição deve identificar estágio, revisão e backend. Não substituir o compilador padrão antes de cobertura e rollback verificáveis. Mudanças incompatíveis descobertas exigem revisão de contrato, changelog e migração próprios.

## Acessibilidade

Diagnósticos devem explicar problema e ação próximos do span. Glossário distingue linguagem de implementação, runtime e backend. Expor estágio usado e falha da ponte sem esconder fallback. Documentação PT acessível não substitui a especificação normativa inglesa.

## Segurança e riscos

Riscos: bugs duplicados, divergência semântica, falso consenso entre implementações, comprometimento de stage0, entradas malformadas, explosão de memória/tempo, colisões de IDs, paths absolutos, travessia de diretórios, execução de ferramenta não confiável e resultados parciais. Validar limites antes de alocar, usar argumentos de processo sem interpolação shell, restringir caminhos de saída, registrar hashes sem segredos e escrever resultados atomicamente. Pin e lockfile não são prova de origem confiável; assinatura/attestation e reconstrução independente permanecem necessárias. Processo separado não é sandbox automático.

## Performance

Medir tempo total e por fase, RSS, alocações, tamanho de IR, serialização, inicialização da ponte e tamanho de artefatos. Comparar cold/warm separadamente, com target, perfil, flags e corpus fixos. Não prometer ganho. Bridge persistente somente após evidência de custo e contrato de isolamento. Não otimizar no Marco A.

## Testes e conformidade

- Baseline: duas builds limpas locked, toolchain exata, hashes e logs; runtime staticlib/cdylib e smoke isolado.
- Fases: goldens estruturais, testes positivos/negativos, propriedades e diferenciais contra spec e Rust.
- Bridge: versões incompatíveis, truncamento, limites, IR inválida, cancelamento e falha de processo.
- Backend: execução AOT/JIT, otimização ligada/desligada, ARC, ABI e targets declarados.
- Bootstrap: stage1/2/3 com mesma fonte; comparação stage2/3 e classificação de diferenças.
- Qualidade: corpus externo, fuzzing com minimização, regressões independentes e revisão humana.

**Hashes iguais de stage2/3 não provam correctness:** ambos podem reproduzir o mesmo bug ou comprometimento. Testes também não constituem prova completa.

## Alternativas

Manter tudo Rust evita custo de migração e continua sendo fallback. FFI direta exigiria ABI própria estável e ownership entre linguagens; não é o primeiro passo recomendado. Processo Rust adiciona serialização mas fornece isolamento de memória e uma fronteira inspecionável. Backend próprio ou runtime Ori ampliariam muito o risco e estão excluídos.

## Questões abertas

Formato e fronteira da IR; subconjunto necessário para escrever o compilador; limites de protocolo; transporte e persistência; formato de distribuição stage0; política de atualização da referência; cobertura suficiente para promoção. Resolver por tarefas e decisões explícitas, não pressupor implementação.

## Rollout

Marco A fixa referência. Marcos posteriores definem contratos/probes, bridge/dados, frontend, semântica/HIR, nativo/mid-end, CLI/bootstrap, qualidade/ferramentas e distribuição. A tabela completa e o estado real ficam apenas no plano canônico. M4 está reaberto com planejamento iniciado, não completed. Versão de entrega e prazos permanecem indefinidos.
