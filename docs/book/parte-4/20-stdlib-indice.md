# Cap. 20 — Stdlib: índice de consulta

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR
A biblioteca padrão (stdlib) do Ori tem cerca de 34 módulos, todos sob o prefixo `ori.*`. Este capítulo é o índice completo — o que cada um faz e quando importar. Para assinaturas exatas de função, use `ori doc` no seu terminal ou leia o arquivo `.orl` do módulo direto na instalação.

---

A Stdlib (Biblioteca Padrão) é o conjunto de módulos que já vêm embutidos na linguagem Ori. Você não precisa baixar nada extra para usá-los.

## Entendendo as Camadas (Layers)

- **Layer 1 (núcleo):** operações essenciais — ARC, coleções, I/O básico. Implementadas em Rust dentro do próprio compilador/runtime, mas acessadas normalmente por `import ori.X`, como qualquer outro módulo.
- **Layer 2 (ergonomia):** funções `.orl` que embrulham a Layer 1 em atalhos mais confortáveis (ex: `read_text_or`, que já devolve um valor padrão em vez de te obrigar a tratar erro toda vez).
- **Layer 3 (algoritmos):** ferramentas de mais alto nível sobre as camadas de baixo.

Na prática, você não precisa se preocupar em qual camada uma função mora —
todas aparecem juntas ao importar `ori.X`.

## Como importar

Sempre no formato **caminho = alias**:

```ori
import ori.io = io
import ori.fs = fs
import ori.string = str
```

## Módulos de uso diário

### `ori.io` — Entrada e Saída
```ori
import ori.io = io

main()
    io.print("Digite seu nome: ")
    const nome: string = io.read_line()
    io.println(f"Bem-vindo, {nome}!")
end
```
`io.print` (sem quebrar linha), `io.println` (quebra linha), `io.read_line` (lê uma linha do terminal).

### `ori.fs` — Sistema de Arquivos
```ori
import ori.io = io
import ori.fs = fs

run() -> result[void, string]
    const config: string = try fs.read_text("config.txt")
    io.println(config)
    return ok()
end
```
`fs.read_text`/`fs.write_text` para o caso comum (ambos retornam `result[.., string]`); `fs.open_read`/`fs.open_write` + `using` para controle fino sobre o arquivo aberto (Cap. 17).

### `ori.string` — Texto
Funções de manipulação de `string` (busca, transformação, particionamento). Importe como `str` por convenção:
```ori
import ori.string = str
```

### `ori.list` — Listas
```ori
import ori.list = lists   -- 'list' já é o nome do tipo; use outro alias

main()
    var nums: list[int] = [1, 2, 3]
    lists.push(nums, 4)
    io.print(f"{lists.len(nums)}")
end
```

### `ori.map` / `ori.set` — Mapas e Conjuntos
Operações sobre `map[K, V]` e `set[T]` — checagem de chave, união, interseção, diferença, conversão de/para lista.

### `ori.test` — Testes
```ori
import ori.test = test

@test
test_math()
    test.assert_eq(2 + 2, 4)
    test.assert(2 + 2 > 0, "deveria ser positivo")
end
```
`test.assert_eq(atual, esperado)` falha o teste se forem diferentes. `test.assert(condição, mensagem)` falha se a condição for falsa.

### `ori.math` — Matemática
Funções além dos operadores (`+`, `-`, `*`, `/`): `sign`, `clamp_int`/`clamp_float`, `lerp`, conversão grau/radiano, `hypot`, comparação aproximada de float (`approx_eq`).

### `ori.time` — Tempo
Tipos `Instant` (um ponto no tempo) e `Duration` (uma quantidade de tempo), com construtores como `duration_seconds`, `duration_millis`.

### `ori.os` — Sistema Operacional
Variáveis de ambiente (`env_or`, `has_env`) e detecção de plataforma (`is_windows`, `is_linux`, `is_macos`).

### `ori.args` — Argumentos de Linha de Comando
```ori
import ori.args = args

main()
    const primeiro: string = args.get_or(1, "padrao")
end
```

### `ori.json` — JSON
```ori
import ori.json = json

run() -> result[void, string]
    const value: json.Value = try json.read("dados.json")
    try json.write_pretty("saida.json", value)
    return ok()
end
```

### `ori.net` — Rede
Clientes e servidores TCP/UDP, com suporte a TLS. Operações de conexão têm variantes `_async` para uso com `await` (Cap. 17).

### `ori.process` — Subprocessos
Lança e controla outros programas a partir do seu.

## Coleções além de list/map/set

A stdlib traz estruturas de dados prontas para os casos em que uma lista
simples não é a ferramenta certa:

| Módulo | Para que serve |
|--------|-----------------|
| `ori.stack` | Pilha (LIFO — último a entrar, primeiro a sair). |
| `ori.queue` | Fila (FIFO — primeiro a entrar, primeiro a sair). |
| `ori.deque` | Fila de duas pontas — insere/remove rápido dos dois lados. |
| `ori.linked_list` / `ori.doubly_linked_list` | Listas ligadas, simples e duplamente encadeada. |
| `ori.hash_table` | Tabela hash de baixo nível (base do `map[K, V]`). |
| `ori.tree` | Árvore genérica. |
| `ori.graph` | Grafo (nós + arestas). |
| `ori.heap` | Fila de prioridade (heap binário). |

## Módulos de suporte

| Módulo | Para que serve |
|--------|-----------------|
| `ori.path` | Junta e resolve caminhos de arquivo sem depender do separador (`/` vs `\`) do sistema operacional. |
| `ori.bytes` / `ori.buffer` | Manipulação de dados binários brutos. |
| `ori.convert` | Conversões entre tipos primitivos. |
| `ori.validate` | Checagens de validação reutilizáveis. |
| `ori.random` | Números e escolhas aleatórias. |
| `ori.format` | Formatação de valores como texto além da interpolação `f"..."`. |
| `ori.log` | Log estruturado. |
| `ori.config` | Leitura de arquivos de configuração. |
| `ori.crypto` | Primitivas de criptografia. |
| `ori.iter` | `map`, `filter`, `reduce` e afins sobre listas. **Atenção:** é *ansioso*, não preguiçoso — cada chamada devolve uma `list` nova, já calculada. Para calcular sob demanda, veja `core.Iterable` no Cap. 14. |
| `ori.concurrent` | Cópia profunda (*deep copy*) segura de dados entre tarefas assíncronas. **Não** oferece threads do sistema operacional nem memória compartilhada — veja o Cap. 17. |

## O que memorizar
- Todo módulo real da stdlib começa com `ori.` (nunca `std.`).
- Para o dia a dia: `ori.io`, `ori.fs`, `ori.string`, `ori.list`/`ori.map`/`ori.set`, `ori.test`.
- Para estruturas de dados especializadas, existe um módulo próprio antes de você precisar implementar na mão (`ori.stack`, `ori.queue`, `ori.tree`, `ori.graph`, …).
- Assinatura exata de uma função? Rode `ori doc` no projeto ou abra o arquivo `.orl` do módulo na sua instalação.
