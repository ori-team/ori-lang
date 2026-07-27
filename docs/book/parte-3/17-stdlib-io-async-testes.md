# Cap. 17 — Stdlib, I/O, async e testes
> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** III

## TL;DR
A biblioteca padrão do Ori (stdlib) fornece ferramentas limpas e seguras para I/O, concorrência (async/await), operações do sistema e testes integrados, sem necessidade de pacotes externos na maioria das tarefas diárias.

## Gerenciamento de Recursos (O Bloco 'using')

Para evitar vazamentos de memória ou arquivos abertos, o Ori utiliza a palavra-chave `using`. Qualquer objeto que implemente a trait `Disposable` pode ser gerenciado por ela. Quando o **bloco em que `using` foi declarado** termina — seja por chegar ao `end`, por um `return`, por `try` propagando erro, por `break`/`continue`, ou até por um pânico — o Ori chama o método de limpeza automaticamente.

`using` é uma declaração comum, **não** abre um bloco próprio com seu
próprio `end`. As linhas seguintes ficam na mesma indentação:

```ori
imports
    ori.fs = fs
end

process_file() -> result[string, string]
    -- O arquivo é aberto; abrir pode falhar, por isso o 'try'
    using file: fs.File = try fs.open_read("dados.txt")
    const text: string = try fs.read_all(file)
    -- 'file' é fechado automaticamente aqui, no 'end' da FUNÇÃO,
    -- não num 'end' próprio do using
    return ok(text)
end
```

Se houver mais de um `using` na mesma função, a limpeza acontece na ordem
**inversa** da declaração (LIFO — o último aberto é o primeiro fechado).

## I/O, Arquivos e Caminhos

O Ori divide I/O e sistema de arquivos em módulos distintos:

- **`ori.io`**: Fornece `io.print`/`io.println` (escrita) e `io.read_line` (leitura do terminal).
- **`ori.fs`**: Manipulação de arquivos no disco. Atalhos rápidos como `fs.read_text(path)` e `fs.write_text(path, dados)` (ambos retornam `result[string, string]` / `result[void, string]`) para o caso comum, e `fs.open_read`/`fs.open_write` + `using` para controle fino.
- **`ori.path`**: Facilita juntar barras (`/` ou `\`) e resolver nomes de arquivos sem depender do sistema operacional.

```ori
imports
    ori.io = io
    ori.fs = fs
end

run() -> result[void, string]
    const content: string = try fs.read_text("config.txt")
    io.println(content)
    return ok()
end
```

## Operações Assíncronas (Async/Await)

Se o código precisar esperar por dados (como na rede), use `async`. Uma função marcada com `async` permite usar `await` dentro dela para pausar a execução sem travar o restante do programa (cooperatividade).

```ori
imports
    ori.task = task
end

-- Uma função que retorna no futuro
async fetch_data() -> string
    -- pausa por 1 segundo sem bloquear as threads do S.O.
    await task.sleep(1000)
    return "dados baixados"
end

-- Ponto de entrada assíncrono
async main()
    -- aguarda o término
    const result: string = await fetch_data()
end
```

Se precisar executar código assíncrono a partir de código comum (síncrono), pode usar `task.run_blocking(minha_funcao_async())`.

> **Um detalhe honesto sobre concorrência:** a Ori **não** expõe threads do
> sistema operacional para o seu código — não existe `spawn` criando uma
> thread nova nem memória compartilhada mutável entre tarefas. O módulo
> `ori.concurrent` cobre apenas cópia profunda (*deep copy*) segura de
> dados entre tarefas. Isso é uma escolha deliberada: sem threads
> compartilhando estado, a classe inteira de bug de *data race* simplesmente
> não existe em código Ori puro.

## Escrevendo e Rodando Testes

O Ori possui um framework de testes embutido, dispensando ferramentas de terceiros. Basta anotar as funções de teste com `@test`.

**Regras do `@test`:**
1. A função não pode receber parâmetros.
2. A função pode ser síncrona (sem retorno útil) **ou** `async` — os dois formatos são suportados nativamente.

```ori
imports
    ori.test = test
    app.math = math
end

@test
test_addition()
    const result: int = math.add(2, 2)
    -- Verifica se é igual; aborta o teste se não for
    test.assert_eq(result, 4)

    -- Checagem genérica, com mensagem personalizada
    test.assert(result > 0, "O resultado deve ser positivo")
end
```

Para rodar, vá ao terminal, na pasta do projeto e digite:
```bash
ori test main.orl      -- roda os @test de um arquivo específico
ori test ori.proj      -- ou aponte para o manifesto do projeto
```

## Explorando Outros Módulos Essenciais

A stdlib da Ori vai muito além de I/O e testes — o [Cap. 20](../parte-4/20-stdlib-indice.md) tem o índice completo. Destaques que valem conhecer cedo:

- **Integração com C (FFI):** Se você precisar exportar uma função Ori para ser chamada de C, use `@c_export`.
- **Rede (`ori.net`):** Suporta criação de servidores e clientes TCP/UDP, com integração pronta para TLS.
- **Processos e Ambiente:**
  - `ori.process`: Lança e controla subprocessos (abrir outros programas).
  - `ori.os`: Acesso a variáveis de ambiente (`os.getenv()`).
  - `ori.args`: Captura o que foi digitado no terminal pelo usuário.
- **Dados (`ori.json`):** Leitura e escrita nativa do formato JSON.
- **Coleções extras (`ori.stack`, `ori.queue`, `ori.deque`, `ori.tree`, `ori.graph`, `ori.heap`, `ori.linked_list`, `ori.hash_table`):** estruturas de dados prontas além de `list`/`map`/`set`.
- **Iteração preguiçosa (`ori.iter`):** filtros e mapeamentos que só calculam quando você pede o próximo valor.

## O que memorizar
- `using recurso = ...` fecha arquivos e conexões sozinho; **não** tem `end` próprio — a limpeza acontece no `end` do bloco onde ele foi declarado (LIFO se houver vários).
- `async` e `await` evitam travamentos em operações demoradas (I/O, rede); a Ori não expõe threads do SO para o seu código.
- Funções de teste começam com `@test`, podem ser síncronas ou `async`, e usam o módulo `ori.test` para asserções.
- `ori test arquivo.orl` ou `ori test ori.proj` descobrem e executam seus testes.
