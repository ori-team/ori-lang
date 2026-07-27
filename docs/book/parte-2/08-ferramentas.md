# Cap. 8 — Ferramentas do dia a dia

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

O kit de ferramentas da Ori foi desenhado para ser simples e direto. Você interage com a linguagem através de um único comando de terminal (`ori`) e pode ter assistência completa no seu editor de texto favorito usando nossa extensão oficial.

## Exemplo

Uma sessão normal de trabalho de um programador Ori é rápida. Você usa o terminal para testar o código no mesmo segundo:

```bash
# Inicia um projeto e roda o código na memória (JIT)
ori run main.orl

# O programa funcionou? Agora crie a versão final pesada (AOT)
ori compile main.orl

# Verifique se todos os testes do projeto passam
ori test
```

## CLI: O Comando `ori`

A "CLI" (Interface de Linha de Comando) é como você conversa com a Ori. Não importa se você está criando uma calculadora pequena ou um servidor gigante, você vai usar as mesmas ferramentas.

### 1. `ori run` (Testes Rápidos)
Este é o seu melhor amigo. Quando você roda `ori run arquivo.orl`, a Ori compila o código super rápido e o executa na mesma hora usando a técnica JIT (Just-In-Time). Ele não cria arquivos extras no seu disco. Use isso centenas de vezes por dia para testar se sua lógica está funcionando.

### 2. `ori compile` (A Entrega Final)
Quando o seu programa está pronto e você quer distribuí-lo para outras pessoas ou servidores, você usa `ori compile`. Este comando gera um arquivo executável real e otimizado (AOT - Ahead-of-Time). O processo demora alguns segundos a mais, mas o resultado final é extremamente veloz.

### 3. `ori check` (O Revisor Rápido)
Se você apenas quer saber se o seu código tem erros de sintaxe ou de tipos, sem executá-lo, use `ori check`. Ele lê todo o projeto e relata os erros quase instantaneamente. É ótimo para descobrir falhas antes mesmo de tentar rodar.

### 4. `ori doctor` (O Clínico Geral)
Às vezes, as coisas quebram e você não sabe o porquê. Pode ser uma configuração do sistema operacional. O comando `ori doctor` varre o seu computador, verifica se o compilador está acessível, se as bibliotecas padrão estão no lugar certo e relata se falta alguma peça para a Ori funcionar.

## O resto do canivete suíço

Os quatro comandos acima cobrem 90% do seu dia a dia, mas o `ori` traz
várias outras ferramentas embutidas — nenhuma exige instalar nada extra:

| Comando | Para que serve |
|---------|-----------------|
| `ori new <pasta>` | Cria um projeto novo (com `ori.proj` e módulo inicial). |
| `ori init` | Como `ori new`, mas dentro de uma pasta que já existe. |
| `ori fmt arquivo.orl` | Formata o código no padrão oficial (indentação, espaços). |
| `ori repl` | Abre um interpretador interativo — digite expressões e veja o resultado na hora. |
| `ori explain <código>` | Explica um diagnóstico do catálogo de erros (ex: `ori explain mut.const_mutation`) com mais detalhe e exemplos. Veja o [Cap. 21](../parte-4/21-diagnostics.md). |
| `ori update` | Baixa e instala a versão mais nova da própria Ori (self-update). Veja o [Cap. 10](../parte-3/10-instalar.md). |
| `ori install` | Instala um pacote (dependência) no cache local do seu usuário. |
| `ori publish` | Publica um pacote seu no registry configurado. |
| `ori build <pasta ou ori.proj>` | Como `ori compile`, mas para o **projeto inteiro** de uma vez (múltiplos arquivos). |
| `ori doc` | Extrai os comentários de documentação (`.oridoc`) como Markdown ou HTML. |
| `ori summary` | Mostra uma visão geral do projeto: entrada, namespaces, imports. |
| `ori migrate-syntax` | Reescreve, na medida do possível, código de sintaxe antiga (pré-S3) para a atual. |

> **`ori compile` vs `ori build`:** os dois geram um executável nativo pelo
> mesmo backend (Cranelift). A diferença é o escopo: `ori compile` recebe
> **um arquivo**; `ori build` recebe um arquivo, um `ori.proj`, ou a pasta
> raiz do projeto, e resolve todos os módulos envolvidos. Para projetos com
> mais de um arquivo `.orl`, use `ori build`.

## Configurando seu Editor

Escrever código em um bloco de notas simples é frustrante. A Ori possui um LSP (Language Server Protocol), um motor inteligente que roda no fundo e avisa sobre erros em tempo real. Existem extensões oficiais para dois editores:

### VS Code (e compatíveis como Cursor)
A extensão oficial colore o código S3 adequadamente (syntax highlighting) e desenha as famosas "cobrinhas vermelhas" debaixo de erros antes de você ir para o terminal. O LSP também autocompleta nomes de funções e mostra a documentação quando você passa o mouse por cima do código.

### Zed
A Ori também tem uma extensão oficial para o editor Zed, com os mesmos benefícios de LSP (erros em tempo real, autocompletar).

Os dois exigem que o `ori-lsp` (instalado junto com o `ori`) esteja no `PATH` do seu sistema.

## Variáveis de Ambiente Úteis

Variáveis de ambiente são configurações invisíveis do sistema operacional que mudam o comportamento da Ori. **Na imensa maioria dos casos você não precisa de nenhuma** — a Ori funciona sem configuração. Elas existem para destravar situações específicas:

| Variável | Quando usar |
|----------|-------------|
| `ORI_USE_JIT=1` / `ORI_USE_AOT=1` | Força `ori run` a usar um modo específico, se a detecção automática não acertar. |
| `ORI_STDLIB_ROOT` | Use quando você instalou a Ori em uma pasta não padrão e o compilador diz que não encontra a biblioteca básica (stdlib). |
| `ORI_USE_SYSTEM_LINKER=1` | A Ori traz seu próprio empacotador para AOT, mas isso força o uso do linker do seu sistema operacional. |
| `ORI_REGISTRY` | Aponta para outro servidor de pacotes (usado por `ori install`/`ori publish`). |
| `ORI_PACKAGE_CACHE` | Muda onde os pacotes baixados ficam guardados localmente. |

Existe também um punhado de variáveis **de engenharia**, voltadas para quem
está depurando o próprio compilador (não o seu programa): `ORI_DUMP_ARC`
(mostra as operações de contagem de referência de uma função),
`ORI_DUMP_CLIF` (mostra o código intermediário do Cranelift) e
`ORI_TEST_LEAK_CHECK=1` (faz os testes falharem se sobrar memória não
liberada). Você não precisa delas para programar — elas existem para quem
trabalha no compilador em si.

## O que memorizar

* `ori run` para agilidade no dia a dia; `ori compile`/`ori build` para a versão final (arquivo único ou projeto inteiro).
* `ori doctor` é a primeira ferramenta a usar se o seu ambiente parecer quebrado; `ori explain <código>` é a primeira se um erro parecer obscuro.
* Use a extensão do VS Code ou do Zed para ter cores, autocompletar e erros em tempo real (LSP).
* Normalmente você não precisa de nenhuma variável de ambiente — elas são para casos específicos.
