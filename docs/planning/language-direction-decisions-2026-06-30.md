# Decisoes de Direcao da Linguagem

> Status: decisoes aceitas para orientar implementacao
> Data: 2026-06-30
> Escopo: design da linguagem Ori. Nem todos os itens estao implementados ainda.

> **Superseded (parcial, 2026-07-12):** a superfície de **propagação de erros** e a
> forma canônica da linguagem passaram pelo corte **S3 / `0.3.0`**. Em particular:
> **só `try expr`** (postfix `expr?` é erro); ver
> [`ori-surface-s3-auk9.md`](ori-surface-s3-auk9.md),
> [`adr-ori-surface-s3-auk9.md`](adr-ori-surface-s3-auk9.md) e
> [`docs/spec/00-manifesto.md`](../spec/00-manifesto.md).
> As demais decisões deste arquivo (ARC, concorrência, monomorfização, …)
> permanecem referência de direção.


## Resumo curto

| Tema | Decisao |
|---|---|
| Erros | **S3:** só `try expr` (`expr?` removido no `0.3.0`) |
| Backend C | redefinir `ori build`; C deve ser debug explicito ou removido em fase posterior |
| Mutabilidade + ARC | valores previsiveis, copy-on-write onde fizer sentido, mutacao explicita para recursos |
| Concorrencia | modelo cooperativo, explicito, sem dados compartilhados magicos |
| FFI | fechar contrato seguro e simples antes de ampliar tipos suportados |
| Pacotes | implementar projeto, pacote e lockfile de forma separada e legivel |
| ARC + ciclos | aproximar mais de Nim ORC do que de Pony ORCA |
| Generics | monomorfizacao por padrao; medir code bloat antes de criar type erasure |

## Erros: `try` (S3)

> **Histórico pré-S3:** este bloco originalmente documentava `try` *e* `expr?`
> como dual. No corte **S3 / `0.3.0`**, **só `try expr`** permanece; `expr?` é
> erro (`parse.question_propagate_removed`). Ver banner no topo e o registro S3.

Decisao (vigente): **`try expr`** é a **única** forma de propagação.

Motivos (ainda válidos):

- le melhor em codigo longo;
- aparece antes da expressao, entao o leitor sabe cedo que ha saida antecipada;
- e mais facil de ensinar para iniciantes;
- fica mais parecido com Zig sem copiar o modelo inteiro de Zig.

Regra de escrita canônica:

```ori
const config: Config = try read_config(path)
```

~~Forma compacta `expr?`~~ — **removida** no `0.3.0`.

## Mutabilidade + ARC

Decisao: manter mutabilidade explicita e ARC como base, com comportamento de
valor para codigo comum.

Direcao recomendada:

- `const` deve ser o caminho normal.
- `var` e `mut func` devem aparecer quando ha mudanca real de estado.
- Colecoes de alto nivel devem tender a copy-on-write quando isso preservar
  leitura simples.
- Recursos externos, como arquivos e conexoes, devem usar handles explicitos e
  `using`.
- Iteradores devem falhar cedo se a colecao for mutada de forma insegura durante
  a iteracao.

Pros:

- leitura previsivel;
- menos `unsafe` mental;
- bom encaixe com ARC e destrutores;
- facil explicar para usuarios novos.

Contras:

- copy-on-write exige cuidado de runtime;
- mutacao in-place pode exigir APIs mais explicitas;
- performance de colecoes precisa de benchmarks, nao de suposicao.

## Concorrencia

Decisao: seguir com concorrencia cooperativa e explicita.

Direcao:

- `async func`, `await`, tasks e channels devem ser os blocos principais;
- compartilhamento mutavel entre tarefas deve ser restrito e visivel;
- cancelamento deve ser parte do contrato, nao excecao escondida;
- o runtime deve manter pontos seguros para ARC e coleta de ciclos.

Isso preserva a ideia central da Ori: o leitor deve ver onde o programa pode
esperar, falhar, cancelar ou compartilhar estado.

## FFI

Decisao: corrigir e fechar o contrato FFI antes de ampliar ambicao.

Direcao:

- FFI bruto deve aceitar tipos simples e ABI documentada.
- Tipos managed nao devem cruzar fronteira FFI sem wrapper claro.
- Ownership deve estar explicito: quem aloca, quem libera, quem empresta.
- Exemplos pequenos e testes de diagnostico sao obrigatorios.

O objetivo nao e aceitar "qualquer C". O objetivo e aceitar uma fronteira segura
e legivel.

## Pacotes

Decisao: implementar pacotes e corrigir o modelo atual.

Direcao sugerida:

- `ori.proj`: define aplicacao/projeto local.
- `ori.pkg.toml`: define pacote reutilizavel.
- `ori.lock`: trava resolucao exata.
- Dependencias locais por path devem vir antes de registry remoto.
- Registry e publicacao devem vir depois do resolvedor local estar estavel.

Separar projeto, pacote e lockfile reduz confusao e evita um arquivo unico cheio
de papeis diferentes.

## ARC + cycle collector

Decisao: aproximar mais de Nim ORC do que de Pony ORCA.

Motivo: Ori nao e uma linguagem actor-first. O runtime atual ja esta mais perto
de ARC com coletor de ciclos cooperativo. Isso conversa diretamente com o
problema que Nim ORC resolve: manter ARC previsivel, mas recolher ciclos que ARC
puro nao consegue.

Pony ORCA deve ser estudado como referencia para concorrencia segura entre
atores, referencia capabilities e ausencia de data races. Mas copiar esse modelo
como base agora deixaria a linguagem mais pesada do que a filosofia atual pede.

### Risco principal

O risco de ficar perto de Nim ORC e subestimar a dificuldade do grafo de edges.
O compilador e o runtime precisam concordar exatamente sobre:

- quais objetos managed existem;
- quais edges fortes entram e saem de cada objeto;
- quando uma edge muda;
- onde a coleta cooperativa pode rodar.

### Norte recomendado

1. Documentar trade-off de ARC + cycle collector como contrato da linguagem.
2. Medir custo de retain/release em exemplos reais.
3. Cobrir ciclos de structs, colecoes, closures e tasks com testes.
4. Estudar referencia capabilities de Pony como opcao futura para APIs
   concorrentes, nao como modelo central imediato.

Referencias de estudo:

- [Nim memory management](https://nim-lang.org/docs/mm.html)
- [Nim ARC/ORC introduction](https://nim-lang.org/blog/2020/10/15/introduction-to-arc-orc-in-nim.html)
- [Pony ORCA paper](https://www.ponylang.io/media/papers/orca_gc_and_type_system_co-design_for_actor_languages.pdf)

## Linguagens de referencia

### Zig

Trazer para Ori:

- `try` como propagacao explicita e facil de localizar;
- erros no tipo, sem excecoes invisiveis;
- tooling simples e mensagens diretas;
- pouca magica em inferencia e runtime.

Evitar copiar sem filtro:

- `comptime` amplo demais agora; Ori ainda precisa consolidar generics, pacotes
  e stdlib antes disso.

Referencia: [Zig language reference](https://ziglang.org/documentation/master/)

### Roc

Trazer para Ori:

- mensagens de erro extremamente didaticas;
- foco em experiencia de iniciante;
- ownership sem anotacao manual como norte de ergonomia;
- cultura de explicar o erro pela intencao do usuario.

Evitar copiar sem filtro:

- trocar o modelo de memoria inteiro agora. Ori ja assumiu ARC + ciclos.

Referencia: [Roc language site](https://www.roc-lang.org/)

### Gleam

Trazer para Ori:

- sintaxe simples para tipos soma e `Result`;
- documentacao que ensina padroes pequenos;
- cultura de producao sem excesso de recursos;
- clareza em APIs concorrentes.

Evitar copiar sem filtro:

- dependencia conceitual da BEAM. Ori tem runtime nativo proprio.

Referencia: [Gleam language tour](https://tour.gleam.run/everything/)

### Austral

Trazer para Ori:

- ideia de linear/resource types para recursos externos;
- disciplina forte para ownership de handles;
- inspiracao futura para FFI e arquivos/conexoes.

Evitar copiar sem filtro:

- exigir linearidade em todo codigo comum. Isso aumenta a carga cognitiva.

Referencia: [Austral specification](https://austral-lang.org/spec/spec.html)

### Hare

Trazer para Ori:

- superficie pequena;
- foco em controle explicito;
- pouca magia;
- documentacao franca sobre o que a linguagem nao quer ser.

Evitar copiar sem filtro:

- abrir mao de generics. Ori ja precisa de generics para stdlib legivel.

Referencia: [Hare FAQ](https://harelang.org/documentation/faq.html)

### Swift

Trazer para Ori:

- copy-on-write em colecoes como experiencia de valor;
- boas convencoes de API para ARC;
- documentacao clara sobre ciclos.

Evitar copiar sem filtro:

- depender demais de `weak`/`unowned` manuais para ciclos. Ori quer resolver mais
  disso no runtime.

Referencia: [Swift ARC](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/automaticreferencecounting/)

### Nim

Trazer para Ori:

- ARC + ORC como referencia pratica;
- foco em nao parar o mundo;
- documentacao de trade-off entre ARC puro e coletor de ciclos.

Evitar copiar sem filtro:

- herdar complexidade historica de varios modos de GC.

### Pony

Trazer para Ori:

- referencia capabilities como estudo para concorrencia;
- atores sem dados compartilhados inseguros;
- design de runtime pensando em concorrencia desde cedo.

Evitar copiar sem filtro:

- transformar Ori em uma linguagem actor-first. Esse nao e o contrato atual.

## Monomorfizacao

Decisao: manter monomorfizacao como padrao atual.

Explicacao curta:

Uma funcao generica e como um molde. Quando o programa usa esse molde com
`int`, o compilador cria uma versao para `int`. Quando usa com `string`, cria
outra versao para `string`.

Exemplo:

```ori
func identity<T>(value: T) -> T
    return value
end

const a: int = identity(1)
const b: string = identity("ori")
```

O compilador pode gerar algo equivalente a:

```text
identity_int
identity_string
```

Pros:

- runtime rapido;
- codigo especializado por tipo;
- mais simples para o backend nativo;
- combina com Rust, C++ templates e o pipeline atual da Ori.

Contras:

- binario pode crescer;
- compilacao pode ficar mais lenta;
- uma funcao generica usada com muitos tipos vira muitas copias.

Norte futuro:

- medir quantas instanciacoes genericas cada build gera;
- mostrar isso em `ori summary`;
- deduplicar instanciacoes identicas quando possivel;
- estudar `any<Trait>` ou type erasure opcional para APIs frias, plugins e
  fronteiras de pacote;
- manter monomorfizacao como default para hot path.
