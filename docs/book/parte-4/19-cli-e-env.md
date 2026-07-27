# Cap. 19 — CLI e variáveis de ambiente

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR
Este capítulo é a referência completa do comando `ori`: todos os subcomandos reais, o que cada um faz, e as variáveis de ambiente que existem de verdade no compilador.

---

O compilador e gerenciador de projetos da linguagem Ori estão embutidos em um único executável: `ori`. Você não precisa de outras ferramentas para trabalhar com código Ori.

## 1. Comandos do dia a dia

### `ori run <arquivo.orl>`
**O que faz:** Compila o código rapidamente (JIT, via Cranelift) e executa o programa em seguida, sem gerar arquivo nenhum no disco.
**Quando usar:** Enquanto você está programando e testando — é o comando que você vai chamar centenas de vezes por dia.
```bash
ori run src/main.orl
```

### `ori compile <arquivo.orl>`
**O que faz:** Gera um executável nativo (AOT) a partir de **um único arquivo**.
```bash
ori compile src/main.orl -o meu_programa
```
Com `--lib`, gera uma biblioteca compartilhada (`.so`/`.dll`/`.dylib`) com funções exportadas via `@c_export`, para ser usada dentro de outro programa (C, Python, engines de jogo, etc).

### `ori build <arquivo, ori.proj ou pasta>`
**O que faz:** A mesma compilação nativa do `ori compile`, mas para **um projeto inteiro** (vários arquivos `.orl` conectados por `module`/`import`).
```bash
ori build .                 -- compila o projeto na pasta atual
ori build ori.proj -o app   -- ou aponte direto para o manifesto
```
**Regra prática:** um arquivo solto → `ori compile`. Um projeto com `ori.proj` → `ori build`.

### `ori check <arquivo.orl>`
**O que faz:** Só verifica erros de sintaxe e de tipos, sem compilar nem rodar nada. É o mais rápido dos três.
```bash
ori check src/main.orl
```

### `ori test <arquivo.orl ou ori.proj>`
**O que faz:** Procura funções marcadas com `@test` e executa cada uma, reportando quantas passaram e quantas falharam.
```bash
ori test src/main.orl
ori test --filter soma      -- roda só os testes cujo nome contém "soma"
```

### `ori fmt <arquivo.orl>`
**O que faz:** Formata o código no padrão oficial (espaçamento, indentação) e imprime o resultado.
```bash
ori fmt src/main.orl
```

## 2. Projeto e pacotes

| Comando | O que faz |
|---------|-----------|
| `ori new <pasta>` | Cria um projeto novo do zero (`ori.proj` + módulo inicial). |
| `ori init` | Igual ao `new`, mas dentro de uma pasta que já existe. |
| `ori install` | Instala uma dependência (path local, URL de repositório Git, ou `nome@versão` de um registry) no cache local. |
| `ori get` | Baixa as dependências git/path já declaradas no `ori.proj`/manifesto do pacote. |
| `ori publish` | Publica o pacote atual no registry configurado (`ORI_REGISTRY`). |
| `ori summary` | Mostra um resumo do projeto: ponto de entrada, namespaces, imports. |
| `ori doc` | Extrai comentários `.oridoc` como Markdown ou HTML estático. |

## 3. Ambiente e diagnóstico

| Comando | O que faz |
|---------|-----------|
| `ori doctor` | Faz um raio-X do seu ambiente: runtime, stdlib, linker, target, JIT. Primeira ferramenta a rodar se algo parecer quebrado. |
| `ori explain <código>` | Explica um código de diagnóstico do catálogo de erros com mais detalhe (ex: `ori explain mut.const_mutation`). |
| `ori update [--check]` | Atualiza o próprio `ori` para a versão mais nova publicada (instalações via pacote). `--check` só avisa, sem instalar. |
| `ori migrate-syntax` | Reescreve, na medida do possível, arquivos `.orl` de sintaxe antiga (pré-S3) para a sintaxe atual. |
| `ori repl` | Abre um interpretador interativo (backed pelo JIT nativo) para testar expressões rapidamente. |
| `ori debug <arquivo> --breakpoint <linha>` | Executa um programa instrumentado com debugger cooperativo no terminal (`c` continua, `s` avança, `q` encerra). |
| `ori debug --dap` | Inicia o adaptador mínimo DAP pelo stdin/stdout para integração com uma IDE; lista elementos, campos e prévias limitadas de strings/bytes gerenciados. |

## 4. Ferramentas de depuração do compilador

| Comando | O que faz |
|---------|-----------|
| `ori lex <arquivo.orl>` | Imprime o fluxo bruto de tokens (debug do lexer). |
| `ori parse <arquivo.orl>` | Imprime a AST (debug do parser). |
| `ori emit c <arquivo.orl>` | Emite o código-fonte em C gerado pelo backend secundário (debug). |

Esses três são voltados para quem quer entender ou depurar o próprio
compilador — não fazem parte do fluxo normal de escrever programas em Ori.

## 5. Variáveis de Ambiente

**Na imensa maioria dos casos você não precisa de nenhuma** — a Ori funciona sem configuração. Elas existem para destravar situações específicas.

### Ambiente e caminhos

| Variável | O que faz |
|----------|-----------|
| `ORI_STDLIB_ROOT` | Aponta para a pasta da biblioteca padrão, se você instalou a Ori num lugar não padrão. |
| `ORI_RUNTIME_LIB` / `ORI_RUNTIME_CDYLIB` | Sobrescreve o caminho do runtime nativo (staticlib / dinâmica). |
| `ORI_TARGET_TRIPLE` | Força um alvo de compilação específico (arquitetura + SO), em vez de detectar o seu automaticamente. |

### Compilação nativa (AOT)

| Variável | O que faz |
|----------|-----------|
| `ORI_USE_SYSTEM_LINKER=1` | Usa o linker do sistema operacional em vez do padrão empacotado. |
| `ORI_SYSTEM_LINKER` | Caminho explícito para um linker específico. |
| `ORI_NATIVE_LINKER` | Sobrescreve qual linker nativo o backend chama. |
| `ORI_USE_JIT=1` / `ORI_USE_AOT=1` | Força `ori run` a usar um modo específico. |

### Pacotes e registry

| Variável | O que faz |
|----------|-----------|
| `ORI_REGISTRY` | Aponta para outro servidor de pacotes (usado por `ori install`/`ori publish`). |
| `ORI_REGISTRY_TOKEN` | Credencial de autenticação para publicar num registry privado. |
| `ORI_PACKAGE_CACHE` | Muda onde os pacotes baixados ficam guardados localmente. |
| `ORI_UPDATE_RELEASES_URL` | Aponta `ori update` para outra fonte de releases (usado em testes/CI). |

### Engenharia do compilador (avançado)

Estas existem para quem depura o próprio compilador, não o seu programa —
você não precisa delas para programar em Ori:

| Variável | O que faz |
|----------|-----------|
| `ORI_DUMP_ARC` | Imprime as operações de contagem de referência (ARC) de cada função. |
| `ORI_DUMP_CLIF` | Imprime o código intermediário gerado pelo Cranelift. |
| `ORI_TEST_LEAK_CHECK=1` | Faz testes falharem se sobrar memória alocada e não liberada ao final. |
| `ORI_COOPERATIVE_COLLECT_THRESHOLD` | Ajusta o limite que dispara o coletor de ciclos de memória incremental. |
| `ORI_OPT` | Controla o nível de otimização do codegen. |
| `ORI_DEBUG_INSTRUMENT` / `ORI_DEBUG_SOURCE` | Ligam a instrumentação de arquivo/linha, pilha de chamadas (inclusive async), variáveis escalares, snapshots estruturados de structs/optional/result/enums e coleções, capturas de closures e metadados/elementos indexados de listas usados pelo debugger cooperativo. |
| `ORI_DEBUG_PORT` | Uso interno de `ori debug`/`ori debug --dap`: conecta o programa instrumentado ao agente local; eventos incluem `stackTrace` e `variables`. |

## O que memorizar
- `ori run` → testar rápido. `ori check` → só validar. `ori compile`/`ori build` → gerar o executável final (arquivo único vs projeto). `ori test` → rodar os `@test`.
- `ori doctor` conserta dúvidas de ambiente; `ori explain <código>` conserta dúvidas de erro.
- `ori update` mantém seu próprio `ori` atualizado.
- Você quase nunca vai precisar de uma variável de ambiente — e se precisar, a tabela acima cobre os casos reais.
