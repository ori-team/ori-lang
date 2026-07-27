# Cap. 21 — Diagnostics mais comuns

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR
Diagnostics são as mensagens de erro ou aviso que o compilador Ori emite. Cada uma tem um código único no formato `categoria.nome_do_problema` (ex: `mut.const_mutation`). Este capítulo explica como ler essas mensagens, mostra erros reais e comuns, e apresenta o `ori explain`, que expande qualquer código do catálogo.

---

Quando o código tem algum problema, o compilador Ori para e mostra um "Diagnostic" (Diagnóstico). Eles são feitos para serem legíveis, apontando exatamente onde está o problema — nunca um jargão de máquina.

## Entendendo o Formato do Erro

Um erro do compilador Ori sempre tem esta estrutura:

1. **O código:** sempre no formato `categoria.nome_do_problema`, em inglês e minúsculo (ex: `mut.const_mutation`, `parse.expected_expression`). A categoria (antes do ponto) já diz de qual etapa do compilador o erro veio — `parse.*` é o Parser, `type.*` é o Type Checker, `mut.*` é a checagem de mutabilidade, `name.*` é resolução de nomes.
2. **A Mensagem Principal:** o que de fato deu errado, em uma frase.
3. **O Trecho de Código:** a linha exata do seu arquivo, com `^^^^` sublinhando o ponto do problema.
4. **`action:` (quando existe):** uma sugestão direta de como consertar.

## O comando `ori explain`

Se uma mensagem de erro não bastar, peça mais contexto pelo próprio código:

```bash
ori explain mut.const_mutation
```

Isso imprime uma explicação mais longa do erro, com exemplos — direto do
catálogo de diagnósticos do compilador (mais de 190 códigos documentados).
Vale usar sempre que uma mensagem parecer densa demais.

## 6 Erros Comuns e Como Corrigir

Os exemplos abaixo são reais — cada um foi reproduzido rodando o
compilador, não inventado.

### 1. Chaves `{}` em vez de `end`
**Código:** `parse.unexpected_token`
**Por que acontece:** Você está usando sintaxe de outra linguagem, ou de uma versão pré-S3 do Ori. No S3, blocos **não** usam chaves.
**Código Errado:**
```ori
if x > 10 {
    io.println("Maior")
}
```
**A Solução:** Troque as chaves pela palavra-chave `end`.
```ori
if x > 10
    io.println("Maior")
end
```

### 2. Esquecer o `try` em funções que retornam `result`
**Código:** um erro de tipo (`type.*`) do tipo "expected `string`, found `result[string, string]`".
**Por que acontece:** Funções como `fs.read_text()` não devolvem o texto direto — devolvem um `result` que pode ser o texto OU um erro. Você tentou guardar o `result` inteiro numa variável de texto.
**Código Errado:**
```ori
const texto: string = fs.read_text("arquivo.txt")
```
**A Solução:** Use `try` para desembrulhar o sucesso (ou já propagar o erro).
```ori
const texto: string = try fs.read_text("arquivo.txt")
```

### 3. Tentar mudar uma variável `const`
**Código:** `mut.const_mutation`
**Mensagem real:** `` `frozen` is not mutable `` — **action:** `declare it with 'var' if reassignment is intended`
**Por que acontece:** Por padrão, tudo criado com `const` é imutável.
**Código Errado:**
```ori
const count: int = 0
count = count + 1
```
**A Solução:** Troque `const` por `var`.
```ori
var count: int = 0
count = count + 1
```

### 4. Usar um nome que não existe no escopo
**Código:** `name.undefined`
**Mensagem real:** `` undefined name `print` `` — a Ori não tem uma função `print` solta; é sempre `io.print`, pelo módulo importado.
**Código Errado:**
```ori
print "Olá, Mundo!"
```
**A Solução:**
```ori
import ori.io = io
-- ...
io.print "Olá, Mundo!"
```

### 5. Acessar um campo que a coleção não tem
**Código:** `type.field_on_non_struct`
**Mensagem real:** `` cannot access field `length` on `list[int]` `` — listas não têm propriedades de ponto; o tamanho vem de uma função do módulo.
**Código Errado:**
```ori
io.println(f"{items.length}")
```
**A Solução:**
```ori
import ori.list = lists
-- ...
io.println(f"{lists.len(items)}")
```

### 6. Ponto antes da variante dentro de `match`
**Código:** `parse.case_dot_variant_removed`
**Por que acontece:** No literal (`Shape.Circle(...)`) o ponto identifica de qual enum é a variante. Dentro de um `match shape`, o compilador já sabe o tipo — repetir o ponto no `case` foi removido de propósito no S3, para não haver duas formas de escrever a mesma coisa.
**Código Errado:**
```ori
match shape
case .Circle(radius):
    io.println(f"{radius}")
end
```
**A Solução:**
```ori
match shape
case Circle(radius):
    io.println(f"{radius}")
end
```

## Famílias de código úteis de reconhecer

| Prefixo | Vem de… |
|---------|---------|
| `lex.*` | O Lexer — caractere ou token que a Ori nunca reconhece. |
| `parse.*` | O Parser — a gramática da frase não fecha, ou usa uma forma removida (`parse.*_removed`). |
| `type.*` | O Type Checker — tipos incompatíveis, nome de tipo não encontrado, campo inexistente. |
| `name.*` | Resolução de nomes — identificador não está em escopo. |
| `mut.*` | Checagem de mutabilidade — método `mut` chamado num `const`, ou o contrário. |
| `impl.*` | Implementação de trait (`apply`/`use`) que não bate com o que a trait exige. |
| `update.*` | O comando `ori update` (checksum, instalação não suportada, etc). |

## O que memorizar
- Todo diagnóstico tem código `categoria.nome`, sempre em inglês minúsculo — nunca um número solto como `E001`.
- `ori explain <código>` expande qualquer um deles com mais contexto.
- O prefixo antes do ponto já entrega de qual etapa do compilador o erro veio.
- Ori não usa chaves `{}` para blocos — sempre `end`.
