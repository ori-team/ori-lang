# Ideias de Programas Avançados para Validar a Linguagem Ori

Este documento serve como um repositório de ideias e casos de uso "batalhados" no mundo real. O objetivo de implementar essas ideias na versão 1.0 é estressar o compilador, descobrir *bugs* e validar o design da arquitetura, especialmente em relação ao desempenho e à alocação de memória (ARC) do Ori.

## 1. Mini Banco de Dados em Memória (In-Memory KV Store)
**Objetivo:** Testar a robustez das estruturas de dados e ponteiros de memória do Ori.
**Descrição:**
Um armazenamento Chave-Valor (KV) acessível via TCP (estilo Redis simplificado).
- **Funcionalidades:**
  - Armazenamento em um Hash Map interno (tipo `Map<String, Any>`).
  - Rotinas para Expiração de Chave (TTL) usando o `task.spawn` e `task.sleep`.
  - Escutar conexões TCP simultâneas com o módulo `ori.net.Listener`.
  - Serialização e Desserialização via TCP (text-based ou bytes).
- **Áreas estressadas:** Concorrência, Vazamento de memória (Cycles in ARC), Operações assíncronas contínuas, Mutabilidade.

## 2. Web Scraper Concorrente
**Objetivo:** Validar a resiliência de I/O assíncrono de rede.
**Descrição:**
Um programa CLI que recebe uma URL semente, faz requisições HTTP (usando a nova estrutura HTTP/TCP do Ori), e mapeia o DOM (ou extrai *strings* e *links* usando *regex/parsers* básicos) em profundidade controlada (Crawler).
- **Funcionalidades:**
  - Baixar `N` páginas simultaneamente com um limite de *pool* assíncrono.
  - Armazenar nós já visitados em um `Set<String>` (Hash Set).
  - Escrita direta em um arquivo (`ori.io.File`) com os resultados.
- **Áreas estressadas:** Manipulação extensiva de Strings, Network I/O, File I/O.

## 3. Emulador / Intérprete Simples (Ex: CHIP-8 ou Brainfuck)
**Objetivo:** Testar o motor do Cranelift e a otimização matemática/bit-level.
**Descrição:**
Construir um pequeno interpretador ou VM que leia arquivos binários e reproduza estados.
- **Funcionalidades:**
  - Carregamento de *Bytecode* direto de arquivos no disco para *arrays/bytes*.
  - *Loops* pesados lidando com operações de bit (`AND`, `OR`, `XOR`, shifts) que o Ori suportar nativamente.
  - Renderização opcional de saída via CLI (desenho no terminal).
- **Áreas estressadas:** Matemática, Performance *loop-bound*, Casts e primitivas binárias.

## 4. Servidor de Chat Multicast (UDP/TCP)
**Objetivo:** Estressar canais assíncronos e compartilhamento de referências.
**Descrição:**
Um servidor de bate-papo onde clientes podem entrar, definir um *nickname* e transmitir mensagens (broadcast) para outras pessoas na mesma "sala".
- **Funcionalidades:**
  - Coleções de conexões ativas gerenciadas em memória (ex: `List<Connection>`).
  - Leitura infinita bloqueante nas conexões (`await conn.read()`).
  - Remoção elegante (graceful degradation) de nós desconectados.
- **Áreas estressadas:** Gerenciamento de tempo de vida (ciclo ARC, liberação correta ao desconectar).

## 5. Gerador de Site Estático (SSG)
**Objetivo:** Estressar acesso a sistema de arquivos, processamento de texto e recursão.
**Descrição:**
Um programa de CLI (como um micro-Hugo ou Jekyll) que lê um diretório de arquivos Markdown `.md` customizados e gera um site `.html`.
- **Funcionalidades:**
  - Travessia de diretório recursiva buscando por arquivos (usando `ori.os.path`).
  - Função de *Parsing* para separar *Frontmatter* do conteúdo principal.
  - Concatenação intensiva de strings formatando o código HTML final e escrevendo nos artefatos.
- **Áreas estressadas:** *FileSystem API*, alocação de pequenas *Strings* constantes e algoritmos clássicos DFS/BFS.
