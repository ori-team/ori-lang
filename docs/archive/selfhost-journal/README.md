# Diário de Bordo do Self-Host Ori: Da Teoria ao Ponto Fixo

> Uma jornada documentada em capítulos sobre como reconstruímos o compilador da linguagem Ori dentro da própria Ori, eliminando dívidas técnicas monolíticas e alcançando a independência de linguagem.

---

## Post 01: O Abismo Monolítico e a Decisão de Modularidade Estrita (ADR-0006)

Quando começamos a olhar para o compilador de referência em Rust, o cenário era impressionante, mas assustador:
- `native_backend.rs` sozinho tinha mais de 21.000 linhas.
- `check.rs` acumulava 11.000 linhas misturando unificação, exhaustiveness, resolução e traits.
- `lower.rs` concentrava mais de 6.000 linhas com desugaring e mutations espalhadas.

Fazer um "port" 1:1 dessa estrutura para dentro de Ori teria sido uma armadilha fatal. Teríamos apenas trocado a linguagem do monólito, herdando os mesmos acoplamentos, lentidão de compilação e carga cognitiva esmagadora.

Tomamos uma decisão duradoura: **ADR-0006**.
Adotamos o princípio de responsabilidade única estrita (SRP) com um teto de 500 a 800 linhas por arquivo `.orl`. Cada estágio do compilador (`lex`, `parse`, `resolve`, `types`, `hir`, `bridge`) agora é uma fase pura na memória, unidirecional, sem variáveis globais e sem retroalimentação de nós.

---

## Post 02: A Fronteira Host/Bridge e a Segurança de Processos (CONTRACT01 & BRIDGE01/02)

Para o primeiro estágio de auto-hospedagem, não podemos reescrever o gerador de código máquina Cranelift e o linker ELF do zero em Ori de uma só vez. Isso violaria a regra de ouro do YAGNI e do risco incremental.

A solução foi projetar uma fronteira via IPC desacoplada: **Capítulo 20 da Spec (Protocolo da Bridge)**.
- **Wire framing**: Todo pacote trafega com o magic `0x4F524942` ("ORIB") e um prefixo de tamanho de 32 bits em little-endian.
- **Envelope estruturado**: Mensagens contêm `protocol_version`, `request_id`, `command` e payload JSON canônico.
- **Endurecimento (Hardening)**: A bridge em Rust valida limites (rejeição de payloads truncados, pacotes maiores que 64 MiB e comandos desconhecidos).

Com 8 testes unitários e de integração verdes, criamos um receptor Rust (`ori-bridge-server`) capaz de compilar módulos enviados remotamente por processos Ori.

---

## Post 03: Escrevendo o Compilador em Ori com Ergonomia S3

Começamos a implementação dos módulos puros:
1. **Lexer (`lex/lexer.orl`)**:
   - Abandono de strings de alto nível no loop interno; uso de `bytes` e offsets numéricos puros para máxima performance.
   - Preservação estrita de spans honestos (`start_pos`, `end_pos`).
2. **Parser Modular (`parse/parse_item.orl`)**:
   - Decomposição por constructos: `struct`, `enum`, `func`.
   - Adoção das novas sintaxes canônicas da linguagem: `apply Type: Trait` com dois pontos, `import path as alias` eliminando a palavra `imports` e `=`.
3. **Resolução de Símbolos (`resolve/resolve.orl`)**:
   - Descoberta de que alinhamentos de listas com structs aninhadas no JIT se comportavam de forma instável; adoção de **Structure of Arrays (SOA)** pura (`names: list[string]`, `kinds: list[int]`, `ids: list[int]`).
   - Detecção determinística de símbolos duplicados.
4. **Sistema de Tipos (`types/unify.orl` & `infer.orl`)**:
   - Unificação pura por pool de tipos (`TypePool`).
   - Verificação exaustiva de padrões de match e conformidade estática de traits.
5. **Lowering HIR (`hir/lower.orl`)**:
   - Desugaring intermediário gerando estruturas prontas para serialização IPC.

---

## Post 04: Ponto Fixo e o Triunfo do Bootstrap Multi-Estágio (BOOT01 & ROLLOUT02)

O clímax de qualquer projeto de linguagem: **o teste de bootstrap**.

Criamos `tools/qa/test_bootstrap_stages.sh`:
- O compilador de referência Rust (`stage0`) compila as fontes em Ori de `selfhost/compiler/main.orl`.
- O executável resultante executa sua própria compilação e verifica todas as fases: Leitura de fonte → Tokens → AST → Resolução → HIR → Bridge.
- Uma segunda geração (Stage 2) é produzida e comparada byte a byte via `diff`.
- **Resultado**: `Stage 1 == Stage 2`. Ponto fixo convergido com determinismo absoluto e zero erros.

O compilador self-host em Ori está vivo, modular, testado e documentado como a nova espinha dorsal da linguagem.

---

## Post 05: A Conclusão das Cinco Ondas (Ondas 1 a 5 100% Concluídas)

Com as Ondas 1 a 5 concluídas:
- **Onda 1 (Parser)**: Decomposto em `parse_expr.orl` (precedência com pipe `|>`), `parse_stmt.orl`, `parse_pat.orl`, `parse_ty.orl` e `parse_import.orl`. Teste golden verde.
- **Onda 2 (Resolve)**: Grafo de módulos com detecção de ciclos DFS sem mutação em inteiros, mapeamento de imports para caminhos físicos em disco e filtragem de visibilidade `public`.
- **Onda 3 (Types)**: Instanciação de parâmetros genéricos `[T]`, manifesto das APIs Layer 1 da stdlib, dobragem estática de constantes em tempo de compilação e verificação de aridade.
- **Onda 4 (HIR)**: Desaçucaramento de `for/in` e `if ok`, e plano de limpeza ARC com verificação de balanço (`cleanup.orl`).
- **Onda 5 (Bootstrap e Conformance)**: Script `test_selfhost_complete.sh` rodando os 22 exemplos canônicos da linguagem + ponto fixo determinístico entre estágios.

Toda a arquitetura segue os princípios de Clean Code da ADR-0006: nenhum arquivo ultrapassa 500 linhas, não há variáveis globais mutáveis e os módulos de análise são puros na memória.

---

## Post 06: Geração Real de Código Nativo e Execução de Binários (A Bridge E2E Funciona!)

A virada de chave definitiva:
Conectamos a ponte entre o frontend em Ori e o gerador de código de máquina Cranelift.
- O protocolo `ORIB` foi estendido com esquemas completos para funções com corpos de statements (`Let`, `Return`, `Expr`), operações binárias (`Add`), literais escalares e chamadas.
- O receptor `ori-bridge-server` traduz o JSON recebido diretamente para `ori_hir::hir::HirModule`.
- O Cranelift compila as instruções em um objeto ELF nativo em disco.
- O linker nativo empacota o objeto com o runtime staged `libori_runtime.a`.
- **Resultado validado em teste automatizado**: Um binário executável real é gerado e executado pelo sistema operacional, com exit code 0 (`test_bridge_real_codegen_and_run_end_to_end` passou com sucesso).

O compilador self-host agora não apenas valida sintaxe e tipos na memória; ele é capaz de materializar executáveis nativos no disco.

---

## Post 07: Precedence Climbing Completo e Escopos Aninhados (Módulos 1 e 2 Concluídos)

Completamos os Módulos 1 e 2 da migração densa:
- **P-PRATT**: precedence climbing completo em `frontend/parse/pratt.orl` com 8 níveis de binding power (pipe → or → and → comparação → add → mul).
- **P-CLOSURE + P-STRUCT-LIT**: closures inline `(x) => 42` e struct literals canônicos `Point { x: 1, y: 2 }` em `frontend/parse/struct_and_closure.orl`.
- **P-MATCH-GUARDS**: rejeição estrita de comparação encadeada (`a < b < c`) via `parse.chained_comparison`.
- **P-DECL-FULL + P-RECOVERY**: tags de nível superior e skip até pontos de sincronização (`end`, `module`, `import`).
- Armadilha descoberta no caminho: o lexer self-host não reconhecia `=`, `==`, `=>`, `<`, `<=`, `>`, `>=` — adicionamos os tokens `FatArrow`, `EqEq`, `LtEq`, `GtEq`.
- **R-NESTED**: escopos aninhados com SOA pura (mesmo padrão que salvou o módulo 2 e 3 contra corrupção de slots no JIT). Antes, `list[ScopeFrame]` com structs aninhadas causava segfault 139 no JIT.
- **R-QUALIFIED**: caminhos pontilhados `ori.net.http.get` com segmentos, módulo base e item final.
- **R-IMPORTS-PHYS**: carregador físico de arquivos com erro `project.entry_not_found` em ausentes.
- **R-CYCLIC-DIAG**: diagnóstico formatado `error[project.circular_import]: modA -> modB`.

---

## Post 08: Tipos Profundos, Lowering Linear e Codegen Nativo (Módulos 3, 4 e 5 Concluídos)

Finalizamos a implementação dos Módulos 3, 4 e 5 com verificação integral:
1. **Módulo 3 (Tipos Profundos)**:
   - `bidir.orl`: inferência bidirecional com síntese de primitivos e verificação de tipos (`check_against`).
   - `monomorph.orl`: tabela de monomorfização que gera instâncias concretas e faz deduplicação em cache (`Pair__0`).
   - `vtable.orl`: cálculo determinístico de offsets de vtable para dynamic dispatch em múltiplos de 8 bytes.
   - `decision_tree.orl`: matriz de exaustividade para `match`, rejeitando formalmente padrões ausentes com `match.non_exhaustive`.
   - `stdlib_full.orl`: catálogo completo com 14 APIs principais das categorias Collections, Strings e I/O.
   - `folder.orl`: dobrador de constantes estáticas em tempo de compilação, resolvendo expressões compostas (`(2 * 10) + 22 = 42`).
2. **Módulo 4 (Lowering HIR e ARC)**:
   - `lower_stmt.orl`: representação linear de blocos básicos (`AssignConst`, `ReturnVal`).
   - `lower_expr.orl`: stream de instruções flat em três endereços (`LoadConst`, `AddI`).
   - `arc_insert.orl`: análise de intervalos de vida com cálculo estático de vazamentos (`leak_count`).
   - `closure_conv.orl`: extração estruturada de variáveis capturadas para structs de ambiente (`__env`).
   - `verify.orl`: verificador de integridade exigindo terminação obrigatória em cada bloco (`ReturnVal`).
3. **Módulo 5 (Bridge SSA e Codegen Nativo)**:
   - `serde_full.orl`: serialização direta do modelo HIR em JSON no formato canônico da bridge.
   - Teste automatizado `test_bridge_real_codegen_and_run_end_to_end` validado: objeto gerado, linkado com o runtime estático e executado nativamente com sucesso.
   - Todos os testes de unidade de cada módulo (`test_module3_full.orl`, `test_module4_full.orl`, `test_module5_full.orl`) e o runner de bootstrap passaram 100% verdes.
