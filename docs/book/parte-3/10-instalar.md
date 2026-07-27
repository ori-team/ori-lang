# Cap. 10 — Instalar e verificar o ambiente

> **Versão âncora:** Ori 0.3.x (S3)
> **Parte:** III

## TL;DR
Instale a linguagem baixando o pacote pronto (Release). Se for compilar para um executável final (AOT), você precisa das ferramentas de C++ do seu sistema operacional. Se for rodar direto (JIT), basta o pacote do Ori. O comando `ori doctor` verifica se está tudo certo.

## O Básico: JIT vs AOT
Antes de instalar, é importante entender as duas formas de rodar um programa em Ori:

*   **JIT (Just-In-Time):** Usado durante o desenvolvimento com o comando `ori run`. Ele lê o seu código e roda na mesma hora. É rápido para testar.
*   **AOT (Ahead-Of-Time):** Usado para gerar a versão final do seu aplicativo com `ori compile`. Ele cria um arquivo executável solto (como um `.exe`), mas é mais lento para compilar.

Para o JIT funcionar, você só precisa do pacote básico do Ori. Já o AOT precisa de um "linker".

## O que é um Linker?
O compilador do Ori (AOT) transforma seu código em linguagem de máquina, mas não sabe montar o arquivo executável final sozinho. 
Um **linker** (ou "linkador") é uma ferramenta do sistema operacional que junta todas as peças traduzidas e forma o arquivo final `.exe` ou binário do Linux/Mac. 
Por isso, para usar AOT, você deve instalar os pacotes básicos de compilação (C/C++) do seu sistema.

## Exemplo Prático de Instalação e Teste

Baixe o arquivo da linguagem no repositório oficial, descompacte e adicione a pasta `bin` ao `PATH` do seu sistema.

Depois, abra o terminal e rode:

```bash
-- Verifica a versão instalada
ori --version

-- Testa a saúde do seu ambiente
ori doctor

-- Roda um programa de exemplo que vem junto
ori run examples/hello/main.orl
```

## Entendendo o `ori doctor`
O comando `doctor` faz um raio-X no seu sistema. Eis o que cada verificação significa:

*   **Runtime:** Verifica se o motor principal do Ori foi encontrado. Se falhar, o pacote foi baixado incompleto.
*   **Stdlib:** Confere se a biblioteca padrão (código base do Ori) está no lugar certo. 
*   **Linker:** Procura o linker do seu sistema operacional (como `gcc`, `ld` ou o da Microsoft). Só é necessário para AOT.
*   **Target:** Verifica a arquitetura do seu processador (como `x86_64` ou `arm`).
*   **JIT:** Confirma se a execução em tempo real está pronta.

## Mantendo a Ori atualizada

Se você instalou pelo pacote (tar.gz/zip), não precisa baixar manualmente
toda vez que sair uma versão nova:

```bash
ori update --check   # só avisa se há versão nova
ori update            # baixa, confere o checksum e troca no lugar
```

O `ori update` recusa instalações feitas pelo gerenciador do sistema (`.deb`)
e builds de desenvolvimento — nesses casos, atualize pelo canal de origem.

## Checklist se algo falhar

Se as coisas derem errado, verifique:

1.  **Comando `ori` não encontrado:** A pasta onde você descompactou o Ori não está no `PATH` do seu sistema operacional. Procure tutoriais sobre como "Adicionar pasta ao PATH" no seu sistema.
2.  **`ori doctor` reclama de runtime/stdlib:** O pacote que você baixou pode estar incompleto ou a variável de ambiente `ORI_STDLIB_ROOT` não aponta para a pasta correta.
3.  **`ori run` funciona, mas `ori compile` falha:** Seu sistema não tem um linker instalado. No Linux, instale o pacote `build-essential`. No Mac, o `Xcode CLT`. No Windows, o `VS Build Tools (C++)`.
4.  **Mensagens sobre `cargo` ou Rust:** Isso só acontece se você baixou o código-fonte inteiro para trabalhar no compilador em si (como desenvolvedor da linguagem). Para uso normal, você não precisa de Rust, use o pacote pré-compilado.

## O que memorizar
*   **JIT (`ori run`):** Roda na hora, ótimo para testar, fácil de instalar.
*   **AOT (`ori compile`):** Cria executável final, precisa de linker (ferramentas de C++ do sistema).
*   **Primeiro passo:** Sempre rode `ori doctor` após instalar.
