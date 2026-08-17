# Interoperabilidade e ABI C

> **English:** [interop.md](interop.md)
> **Referência normativa:** [16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md) e [19-abi.md](../spec/19-abi.md)

Há duas direções de FFI. `extern` importa um símbolo nativo para o código Ori;
`@c_export` publica uma função `public` escolhida para que um host C a chame.

## Exportar uma biblioteca

```ori
module app.embed_add

@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end
```

Compile com:

```bash
ori compile --lib examples/embed/add_scores.orl -o libadd_scores.so
```

O compilador grava a biblioteca e um header C irmão. O host deve chamar
`ori_rt_init()` antes de usar a biblioteca e `ori_rt_shutdown()` ao terminar.
O header também declara `ori_rt_version()` e `ori_rt_abi_version()`; ambas
retornam strings NUL-terminadas emprestadas, que o host não deve liberar. Se o
host aceitar várias revisões, valide a string ABI antes de chamar os exports.

## Fundação de sessão hospedada

Hosts Rust podem usar o crate experimental `ori-embed` para uma fronteira de
sessão com análise estruturada e uma superfície deliberadamente pequena de JIT
persistente:

- `OriConfig` seleciona target, execution profile e features declaradas/ativas;
- `OriEngine` verifica fonte em memória e devolve diagnostics estruturados
  próprios;
- módulos aceitos recebem `ModuleId` estável e gerações crescentes;
- uma atualização inválida preserva a última geração aceita;
- uma atualização válida pode compilar funções públicas sem `main`, resolver um
  handle opaco amarrado à geração e chamar assinaturas homogêneas de `bool`,
  `int` ou `float` com no máximo quatro argumentos.
- `OriHostRegistry` pode registrar uma função escalar `extern host` uma vez por
  sessão; o JIT mantém o endereço em cache e não faz lookup a cada chamada.
- `OriHostRegistry::register_int_callback`, `register_float_callback` e
  `register_bool_callback` podem registrar callbacks escalares homogêneos com
  `user_data` opaco; o JIT injeta um ID estável e faz o dispatch sem lookup
  textual por chamada. Cada callback aceita até quatro parâmetros do seu tipo
  escalar e retorna esse mesmo tipo ou `void` nesta fronteira Rust experimental.
- A remoção é segura para o lifecycle: uma chamada nova após
  `remove_callback` retorna um trap de cancelamento estruturado, enquanto a
  remoção durante uma chamada ativa retorna `CallbackActive`. Reentrada na
  mesma `OriEngine` é suportada para chamadas síncronas e a recursão é limitada
  a 64 frames de callback.
- `OriEngine::unload_module` libera as gerações executáveis retidas; os handles
  passam a falhar em vez de chamar código liberado.
- traps escalares hospedados (contracts, `check`, guards de divisão inteira e
  bounds diretos de collections/texto escalares) retornam
  `OriExecutionError` pela API Rust; não fazem unwind pelo host nem encerram o
  processo.

Isso ainda não é uma API geral de execução nem um sistema completo de hot
reload. Aggregates/managed values, execução assíncrona, unload concorrente e
uma ABI C versionada continuam planejados; caminhos de abort do runtime fora
desse conjunto ainda não estão cobertos. O slice de callback é limitado a
hosts Rust confiáveis e assinaturas escalares homogêneas; header C, dispatch
por afinidade de thread, destruição de objetos e migração durante reload
continuam fora.
Veja o [plano do Host ABI](../planning/embedded-runtime-host-abi-v1.md)
e o [plano do compiler service](../planning/interactive-compiler-service.md).

## Tipos aceitos

ABI-1 aceita escalares, `bool`, `void`, `string`, structs escalares não vazias e
não genéricas por wrappers pointer/out, structs gerenciadas por handles ARC
opacos e bridges diretos de `optional`/`result` sobre esses payloads.

`list`, `map`, `set`, `tuple`, unions aninhadas, structs genéricas e structs
vazias diretas são rejeitadas. Uma collection pode ficar dentro de uma struct
gerenciada, pois seu layout permanece privado.

Parâmetros gerenciados são emprestados. Retornos transferem uma referência ao
host, que deve liberá-la com `ori_arc_release`.

## Limites atuais

`@c_export` pertence ao backend nativo; o backend C/debug não é a referência de
ABI. Callbacks host→Ori continuam fora do header C da ABI-1; a implementação
Rust experimental aceita apenas callbacks escalares homogêneos (`int`, `float`
ou `bool`). Layouts diretos de
collections também continuam fora da ABI-1. Os nomes exportados precisam ser
identificadores portáveis de C/C++.

O caminho completo está em [`examples/embed`](../../examples/embed).
