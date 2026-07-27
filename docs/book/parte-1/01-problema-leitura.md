# Cap. 1 — O problema da leitura de código

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I

## TL;DR

Escrevemos código uma vez, mas o lemos dezenas de vezes. Quando uma linguagem esconde informações cruciais (como tipos ou falhas), ela aumenta a nossa exaustão mental ao ler. A Ori foi criada para colocar tudo o que importa visível no próprio texto.

## Exemplo: O visível contra o invisível

Antes da teoria, veja dois jeitos de declarar a mesma função. O primeiro esconde o "contrato" (a promessa explícita de como a função se comporta). O segundo mostra tudo.

| Opaco para o leitor | Legível na fonte |
|---------------------|------------------|
| `load(path) -> string` | `load(path: string) -> result[string, string]` |

```ori
module app.config

-- Importamos o módulo de sistema de arquivos do sistema
import ori.fs = fs

-- O contrato deixa claro: recebe uma string e retorna um "result"
-- Isso avisa que a operação pode dar certo (string) ou falhar (string de erro)
load_config(path: string) -> result[string, string]
    -- Executa a leitura; se falhar, o erro sobe automaticamente para quem chamou
    return fs.read_text(path)
end
```

No modelo opaco, o leitor precisa memorizar se a função pode falhar. Na Ori, a palavra `result` avisa que precisamos lidar com o erro.

## O custo de entender

### O que é "Carga Cognitiva"?
Carga cognitiva é o esforço mental exigido para usar sua memória de curto prazo (memória de trabalho). Quando o código é cheio de magias ocultas, você precisa guardar na cabeça muitas regras não escritas. Isso esgota sua energia e sua concentração rapidamente.

### O que é um "Contrato"?
Em programação, um contrato é o acordo firme entre quem escreve a função e quem a usa. Ele define o que a função precisa receber e o que promete devolver. A Ori força que todo contrato esteja sempre explícito no texto da assinatura da função.

## Design para cérebros reais

A Ori foi desenhada pensando na **neurodivergência**. O termo "neurodivergência" engloba cérebros que funcionam, aprendem e processam informações de forma diferente do padrão (como pessoas com TDAH, dislexia ou autismo nível 1).

- Para o TDAH, perder o contexto no meio de um código longo é muito fácil.
- A dislexia pode dificultar a leitura de símbolos acumulados e abreviações estranhas.
- O autismo costuma se beneficiar de regras consistentes e da ausência de duplos sentidos.

A Ori entende que acessibilidade não é um bônus. É a fundação. Quando você tira o peso de adivinhar o código, a leitura melhora incrivelmente para todo mundo.

### Como a Ori responde às suas dúvidas

Enquanto você lê um arquivo Ori, a própria linguagem responde às suas perguntas:

- **Onde estou?** A palavra `module` no topo diz exatamente a qual pacote o arquivo pertence.
- **O valor pode estar vazio?** A notação `optional[T]` avisa que o dado pode simplesmente não existir.
- **Quando esta conexão fecha?** A palavra `using` diz que, ao final do bloco, aquele recurso (como um arquivo aberto) será limpo automaticamente, sem você precisar chamar uma função de encerramento manualmente.

## O que memorizar

- Um programa excelente é aquele que você consegue reler com facilidade meses depois.
- Contratos explícitos no ponto de uso sempre vencem truques convenientes de digitação.
- Reduzir a carga cognitiva beneficia todos os programadores, neurodivergentes ou não.
