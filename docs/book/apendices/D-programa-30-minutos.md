# Apêndice D — Programa em 30 minutos

> **Versão âncora:** Ori 0.3.x (S3)

## TL;DR
Um guia passo-a-passo para construir uma aplicação completa (um gerenciador de tarefas via terminal) em menos de meia hora. Demonstra a união de sintaxe, I/O, erro e estruturação.

---

Neste apêndice, vamos juntar tudo o que você aprendeu para criar um CLI de "To-Do List" (Lista de Tarefas). 

## O Código Completo

Crie um arquivo `src/main.orl` e adicione o seguinte:

```ori
-- Todo arquivo começa declarando seu módulo
module app.todo

-- Importação em bloco para organização (sempre caminho = alias)
-- ('ori.fs' entra só na seção de expansão mais abaixo, para persistência)
imports
    ori.io = io
    ori.string = str
    ori.list = lists
end

-- Nosso tipo de dados principal
struct Task
    id: int
    title: string
    done: bool
end

-- Variável global mutável para armazenar tarefas na memória
-- (bindings de topo de arquivo exigem tipo explícito)
var task_list: list[Task] = []
var next_id: int = 1

-- Adiciona uma nova tarefa
add_task(title: string)
    -- Criação da struct usando literal
    const new_task = Task {
        id: next_id,
        title: title,
        done: false
    }
    
    -- Atualiza variáveis mutáveis
    next_id = next_id + 1
    lists.push(task_list, new_task)
    io.println(f"Tarefa adicionada: {title}")
end

-- Lista as tarefas na tela
list_tasks()
    io.println("--- Suas Tarefas ---")
    
    -- For loop em coleção com verificação de estado
    for task in task_list
        const status = if task.done then " [X] " else " [ ] "
        io.println(f"{task.id}:{status}{task.title}")
    end
end

-- Função principal de execução (Entry Point)
run()
    io.println("Bem-vindo ao Ori Todo CLI!")
    
    -- Loop infinito para manter o programa rodando
    loop
        io.print("> ")
        -- io.read_line() devolve optional[string] (pode faltar, ex: fim do
        -- arquivo) — .or("") dá um valor padrão para desembrulhar com segurança.
        -- Repare: 'str.trim' vai SEM parênteses no pipe (senão o Ori tenta
        -- chamar trim() com zero argumentos antes de receber o valor).
        const input: string = io.read_line().or("") |> str.trim

        -- Controle de fluxo usando match no comando digitado
        match input
        case "add":
            io.print("Título: ")
            const title: string = io.read_line().or("")
            add_task(title)
        case "list":
            list_tasks()
        case "exit":
            io.println("Tchau!")
            break -- Sai do loop infinito
        case else:
            io.println("Comando desconhecido. Use: add, list, exit.")
        end
    end
end

main()
    run()
end
```

## Como rodar

No terminal, execute:
```bash
ori run src/main.orl
```

Você verá o prompt `>`. Digite `add`, forneça um título. Depois digite `list` para ver a lista. Digite `exit` para sair.

## Expandindo o Programa (Exercício)

Para tornar este programa em uma ferramenta de produção, tente estendê-lo usando essas ideias:

1. **Persistência de Arquivo:** Use `fs.write_text` dentro da função `add_task` para salvar a string da tarefa em um arquivo `.txt`. Use `try` para lidar com falhas de disco.
2. **Carregamento inicial:** Crie uma função `load_tasks()` que usa `fs.read_text` para carregar o arquivo na inicialização do programa. Lembre-se de usar `if some(line) = reader.next()` para ler linha por linha.
3. **Completar tarefas:** Adicione um comando `done` que pede o ID. Encontre a tarefa correspondente, use a sintaxe de update de struct (`task with { done: true } end`) e substitua na lista.
