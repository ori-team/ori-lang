# Plano de implementação — texto Unicode

> **Status:** paridade escalar concluída; biblioteca aprovada por fases.  
> **Baseline:** `string` contém UTF-8 válido; native e C/debug indexam por
> valores escalares. Graphemes, normalização e regex não fazem parte da stdlib.

## 1. Contrato de texto

Três unidades não podem ser chamadas todas de “caractere”:

| Unidade | Uso |
|---|---|
| byte UTF-8 | rede, arquivos, FFI e armazenamento |
| valor escalar Unicode | índice simples e iteração base da linguagem |
| grapheme cluster | cursor, seleção, truncamento e texto visível |

`len(string)`, `string.len`, `string.slice`, `string.index_of`, indexação,
`string.chars` e iteração direta compartilham a unidade escalar. APIs de bytes
continuam em `bytes`.

## 2. Estado real

- native e C/debug contam, fatiam, indexam, buscam e iteram por scalar value;
- boundaries de entrada usados por `io.read_line` rejeitam UTF-8 inválido;
- `to_upper`, `to_lower` e helpers têm paridade limitada no C/debug;
- não há grapheme segmentation, normalization ou case folding canônico;
- helpers como `is_digits` e `swap_case` são explicitamente ASCII;
- não há regex na stdlib.

## 3. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **TEXT-UNICODE-1.0** | fechar BUG-UTF8-LEN | **done 2026-08-09:** global/method len, slice, index, get, chars, direct for e input validation concordam entre native e C/debug |
| **TEXT-UNICODE-1.1** | nomenclatura e APIs escalares | docs não usam “char” de forma ambígua |
| **TEXT-UNICODE-1.2** | grapheme views | cursor e truncamento nunca dividem cluster |
| **TEXT-UNICODE-1.3** | normalização | NFC/NFD/NFKC/NFKD com vetores oficiais |
| **TEXT-UNICODE-1.4** | case folding e categorias | busca caseless e classificação Unicode explícitas |
| **TEXT-UNICODE-1.5** | regex como package/stdlib avaliada | limites, Unicode e proteção contra abuso documentados |

## 4. Dependências e segurança

Uma tabela Unicode adiciona tamanho e ciclo de atualização. Antes de nova
dependência, registrar versão Unicode, licença, impacto binário e política de
upgrade. Regex exige limites de tamanho/profundidade e escolha que não permita
backtracking catastrófico por padrão.

Views de grapheme não devem importar lifetimes complexos para a superfície.
Podem ser iteradores explícitos ou offsets validados que mantêm a string dona
viva.

## 5. Validação

- ASCII, acentos, emoji, combining marks e sequências ZWJ;
- entradas vazias e limites no início/fim;
- propriedades oficiais Unicode para normalização e segmentação;
- round-trip `string ↔ bytes` somente com UTF-8 válido;
- paridade AOT/JIT; C/debug apenas nas operações que promete;
- benchmarks de texto curto, logs, JSON e editor.

## 6. Fora de escopo

- locale implícito global;
- medir largura visual de fonte sem contexto de renderização;
- tratar grapheme como unidade de armazenamento do tipo `string`.
