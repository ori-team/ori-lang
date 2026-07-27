# Cap. 7 — Decisões de design

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** II

## TL;DR

Cada escolha no design da Ori foi pensada para tornar a leitura do código mais fácil do que a escrita. Preferimos ser explícitos e claros em vez de mágicos e confusos. Se existe uma forma certa de fazer algo, a Ori oferecerá apenas essa forma.

## Exemplo

Veja como as decisões de design se refletem na prática. Tudo começa com a clareza de onde as coisas vêm e para onde vão.

```ori
-- Todo arquivo declara explicitamente de onde ele é
module app.billing

-- Tratamento de erro é visível, não há 'null' escondido
process_payment(amount: int) -> result[int, string]
    -- 'try' deixa óbvio que essa operação pode falhar
    const transaction = try api.charge(amount)
    return ok(transaction)
end
```

## Por que a Ori é assim?

Linguagens de programação tomam decisões de design que moldam como você pensa. A Ori foca em leitores, especialmente aqueles com TDAH ou dislexia, reduzindo o esforço mental necessário para entender um arquivo isolado.

### O `module` no topo

Em muitas linguagens, se você abre um arquivo solto, você não sabe a qual projeto ou pasta ele pertence. Na Ori, o primeiro comando de todo arquivo é `module nome.do.modulo`. Isso significa que o arquivo carrega sua própria "identidade". Você nunca se perde.

### Tipos com Colchetes `[]`

A maioria das linguagens modernas usa símbolos de "menor e maior" (`<>`) para tipos complexos, como `List<String>`. O problema é que esses símbolos são idênticos aos operadores de comparação matemática. O compilador (e os humanos) se confundem. Na Ori, usamos colchetes `list[string]`, separando completamente a gramática de tipos da matemática.

### Ausência e Falhas: `optional` e `result`

Em sistemas antigos, quando algo dava errado ou não existia, as linguagens retornavam `null` (nulo). Isso causava quebras repentinas no programa. 
Na Ori, `null` não existe. 
* Se um valor pode não estar lá, usamos `optional[T]`. 
* Se uma operação pode falhar com um erro, usamos `result[T, E]`. 
Você é forçado a lidar com essas possibilidades logo de cara.

### Apenas `try` para erros

Anteriormente, usava-se um símbolo de interrogação (`?`) no final das palavras para lidar com erros. Mas a interrogação ficava escondida no fim de linhas longas. A Ori adotou a palavra `try` antes da expressão. Isso alerta os seus olhos imediatamente de que a linha pode falhar, antes mesmo de você ler o resto dela.

### Limpeza com `using`

Se você abre um arquivo ou uma conexão de rede, você precisa lembrar de fechar. Se esquecer, o computador pode ficar sem memória (um "resource leak"). O bloco `using` resolve isso amarrando a vida do recurso ao bloco. Quando o bloco `end` chega, a Ori fecha tudo para você, de forma determinística e segura.

```ori
module app.files

read_config() -> string
    -- 'using' garante que o arquivo será fechado no 'end'
    using file = open("config.txt")
    return file.read_all()
end
```

### O Poder do Pipe `|>`

Sem o pipe, para aplicar várias transformações em um dado, você precisaria criar várias variáveis ou alinhar funções de dentro para fora, lendo de trás para frente. O operador `|>` permite que o dado flua naturalmente.

```ori
module app.transform

-- Lendo de dentro para fora (confuso)
const a = print(uppercase(trim(text)))

-- Lendo com pipe (claro e linear)
const b = text |> trim() |> uppercase() |> print()
```

### Mutação Explícita: `mut`

Outra regra de leitura: por padrão, nenhum método pode alterar a struct que
recebeu. Se um método precisa mudar algo, ele **declara isso na assinatura**
com `mut`. Isso significa que, olhando só a primeira linha de um método,
você já sabe se chamá-lo é seguro ou se ele vai mexer nos seus dados.

```ori
apply Counter
    -- Sem 'mut': só lê, nunca muda o Counter
    value(self) -> int
        return self.count
    end

    -- Com 'mut': o nome já avisa que isso altera o estado
    mut increment(self)
        self.count = self.count + 1
    end
end
```

O compilador reforça essa promessa dos dois lados: chamar um método `mut`
numa variável `const` é erro, e um método sem `mut` que tenta alterar `self`
também é erro. Isso é o mesmo espírito do `try`: em vez de confiar que o
programador vai lembrar de documentar o efeito colateral, a linguagem
obriga a promessa a aparecer no texto.

### Mensagens de Erro Úteis

Se você errar, o compilador não solta um jargão incompreensível. A Ori usa diagnósticos com códigos específicos e mensagens que ensinam como consertar. Um erro deixa de ser um "xingamento" da máquina e vira um mapa de como resolver o problema.

## O que memorizar

* Só existe uma maneira correta e recomendada de fazer as coisas.
* Tudo deve ser explícito; o `try` não esconde falhas e não existe `null`.
* A leitura linear (esquerda para a direita) é prioridade, facilitada pelo pipe `|>`.
