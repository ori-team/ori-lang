# Template de capítulo

> Copie este bloco ao criar um capítulo novo. Remova esta linha de instrução.
> **Lembrete:** O livro é um contrato narrativo autossuficiente. Não terceirize a explicação para arquivos da `spec`. Explique tudo de forma didática, com chunking e linguagem simples, dentro do próprio capítulo.

---

# Cap. N — Título do Capítulo

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** I | II | III | IV

## TL;DR

Duas a quatro linhas com o resumo principal da ideia, em linguagem acessível (sem jargões não explicados).

## Um exemplo prático

Antes da teoria, mostre um código ou cenário. 
Use comentários no código para explicar **o porquê** de cada linha importante.

```ori
module app.exemplo

main()
    -- Comente o que esta linha resolve de forma prática
    const valor: int = 42
end
```

## Como funciona (A Explicação)

- Desempacote os conceitos técnicos.
- Se usar uma sigla ou jargão novo, explique o que ela faz em uma frase curta.
- Prefira estruturar em pequenos blocos com títulos ou bullets. Não faça parágrafos enormes.
- Se houver regras de linguagem, explique-as como uma narrativa lógica, mostrando o benefício para o desenvolvedor.

## Decisões (Quando aplicável)

Se houver mais de uma forma de fazer algo (ex: tratar erro com `match` ou propagar com `try`), explique:
1. **Opção A:** O que é e quando usar.
2. **Opção B:** O que é e quando usar.
3. **Recomendação:** Qual é a regra de ouro que o desenvolvedor deve seguir por padrão.

## O que memorizar

- Resumo em bullets do que o leitor precisa levar para o seu dia a dia de código.
- Ponto 2
- Ponto 3
