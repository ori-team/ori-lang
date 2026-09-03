# Roadmap de Execução Detalhado para a v1.0 (Ori Language)

> **⚠️ Superseded for open work.**  
> Active backlog with priorities, difficulty, and dependencies:  
> **[`BACKLOG.md`](BACKLOG.md)**.  
> This file remains as an **aspirational 1.0 sketch**; checkboxes below may be
> stale. Map old “Fase N” ideas to BACKLOG IDs (e.g. HTTP → `STDLIB-2`, git deps
> → `PKG-1`/`PKG-2`, C-async → `LANG-3` wontfix-for-v1).

Este documento continha o escopo técnico faseado para 1.0. **Não** use estas
listas como prioridade tática.

## Fase 1: Portabilidade da Stdlib (`.orl`) e Achatamento de Namespaces
A meta é reescrever **100% da biblioteca padrão** (exceto as primitivas intocáveis de *Layer 1* como ARC e Executor Async) em arquivos `.orl`.
Além disso, para melhorar a ergonomia, **eliminaremos sub-módulos verborrágicos como `.utils` e `.algorithms`**. Funções como `sort`, `reverse`, `trim`, ou `split` serão realocadas para dentro da biblioteca pai (ex: `ori.string`, `ori.list`), permitindo imports diretos e uso natural.

### Tarefas de Execução:
- [x] **1.1. Refatoração de Namespaces (Fim dos sub-módulos vazados)** — **STDLIB-1 done**
  (canonical `ori.X`; nested utils/algorithms silent compat; see BACKLOG + merge policy)
  - [x] Helpers em pais `ori.list` / `ori.string` / `ori.map` (e demais domínios)
  - [x] Paths nested ainda compilam; não são API nova
- [ ] **1.2. Lowering para Código Ori (Migração de C/Rust para `.orl`)** — maps to **STDLIB-5**
  - [ ] Escrever o módulo `ori.string` completo em Ori (chamando primitivas C no hot-path).
  - [ ] Escrever o módulo `ori.list` completo em Ori.
  - [ ] Escrever o módulo `ori.map` completo em Ori.
  - [ ] Escrever o módulo `ori.math` completo em Ori.
- [ ] **1.3. Migração da Camada de I/O e Rede**
  - [ ] Portar a infraestrutura de `ori.net` (TCP/UDP, Connection, Listener) para abstrações `.orl`.
  - [ ] Portar arquivos e streams básicos de `ori.io` (Input/Output).
- [ ] **1.4. Bateria de Testes (Paridade de Implementação)**
  - [ ] Compilar a nova *stdlib* e rodar toda a suíte de testes `cargo test -p ori-driver --test e2e`.
  - [ ] Garantir zero regressão de performance e zero vazamento (Leak Check do ARC) nos novos módulos `.orl`.

---

## Fase 2: O Gerenciador de Pacotes e `.oriproj`
A infraestrutura para consumir código de terceiros pelo GitHub de forma descentralizada.

### Tarefas de Execução:
- [x] **Git deps (PKG-1/2)** — `{ git = ... }` + `ori get` + resolve no check/build (2026-07-13)
- [ ] **2.1. Definição do Esquema `.oriproj`**
  - [ ] Implementar o parser de arquivos TOML (se não existir, criar um rudimentar ou usar *binding*) no compilador para ler `.oriproj`.
  - [ ] Definir a estrutura: `[project]` (nome, versão) e `[dependencies]` (alias, url do git, tag/branch).
- [ ] **2.2. Resolução de Downloads (CLI `ori`)**
  - [ ] Criar a rotina no compilador ou no CLI (ex: comando `ori get` ou build implícito) que clona repositórios Git temporariamente.
  - [ ] Armazenar o cache dos repositórios no diretório do usuário (ex: `~/.ori/pkg/github.com/...`).
- [ ] **2.3. Resolução no Type Checker / Parser**
  - [ ] Modificar o sistema de importação (`import "nome_da_lib"`) para buscar primeiro na *stdlib*, e depois buscar o *alias* no arquivo `.oriproj` da raiz do usuário.
  - [ ] Testar importação cruzada (um pacote puxando outro).

---

## Fase 3: Infraestrutura Web Básica
Desacoplando a robustez da rede e abrindo caminho para a comunidade.

### Tarefas de Execução:
- [ ] **3.1. Parser HTTP de Baixo Nível (Stdlib)**
  - [ ] Implementar `ori.net.http` nativo (apenas parser de headers, verbos, URI e body via raw TCP).
  - [ ] Criar testes unitários para injeção de requests HTTP malformadas e validar tolerância.
- [ ] **3.2. Framework Web Enxuto (Projeto 01 Externo)**
  - [ ] Criar o repositório externo do mini-framework web (ex: `orion-web`).
  - [ ] Implementar Roteador básico estático e dinâmico (`/api/users/:id`).
  - [ ] Testar instalação do framework em um projeto novo de usuário utilizando o `.oriproj`.

---

## Fase 4: O Ecossistema C-ABI (Projetos do Mundo Real)
Utilizando o novo `.oriproj` e o Ori JIT/AOT para envelopar bibliotecas em C pesadas e validar o uso da linguagem com gráficos e bancos de dados.

### Tarefas de Execução:
- [ ] **4.1. `ori-raylib`**
  - [ ] Criar repositório `ori-raylib` contendo wrappers baseados em Structs.
  - [ ] Provar o uso: Escrever um jogo da cobrinha (Snake) em Ori puro renderizado pela Raylib.
- [ ] **4.2. `ori-sqlite`**
  - [ ] Criar repositório `ori-sqlite` gerenciando a DB do SQLite em memória e arquivo.
  - [ ] Validar Iteradores e geradores `.orl` com a API FFI do SQLite.
- [x] **4.3. `ori-imgui` (cimgui)** — **cancelado / fora do produto** (2026-07-13)
  - [ ] Construir o *wrapper* baseado em lambdas/closures para esconder chamadas perigosas (`EndWindow`).

---

## Fase 5: C-Transpiler Parity e CI/Bootstrapping Final
Últimos pregos arquiteturais antes de estampar "v1.0" na flag do compilador.

### Tarefas de Execução:
- [ ] **5.1. Async/Await em C**
  - [ ] Estender a emissão de código (`--c`) para transformar `async func` em máquinas de estado em C puro (State Machines).
  - [ ] Passar nos 35+ testes de `concurrency_async.rs` usando o *backend C* ao invés do Cranelift.
- [ ] **5.2. Pipeline de Compilação Cruzada**
  - [ ] Atualizar as Actions do GitHub para empacotar o release para `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu` e `aarch64-apple-darwin` junto com o `libori_runtime` e os scripts do `.oriproj`.
- [ ] **5.3. API Freeze (Fase de Batalha)**
  - [ ] Executar o *Code Freeze* das assinaturas dos métodos da Stdlib. Nenhuma remoção de função ou quebra de tipo de dado por pelo menos 3 a 6 meses.
