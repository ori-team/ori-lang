# ADR — Copy-on-write para coleções: **deferido** (rejeitado para 0.3.x)

> **Status:** decidido 2026-07-18 — **não adotar** COW na janela FREEZE-1.
> **Contexto:** plano F6 ([`plano-arc-nim-2026-07-16.md`](plano-arc-nim-2026-07-16.md));
> estudo Nim C0 §Eixo 7 (`isUniqueRef` / seq mutate-in-place).

## Problema

O Nim (e o Swift) mutam coleções **in place quando a referência é única**
(`isUniqueRef`: refcount == 1) e copiam apenas quando compartilhadas —
dando semântica de valor observável com custo de cópia só onde há
aliasing real. A pergunta do plano: cabe o mesmo na Ori sem quebrar a
leitura S3?

## Situação atual da Ori (medida)

- As coleções da Ori **já mutam in place** (`lists.push`, `maps.set`,
  `sets.add` alteram o objeto heap compartilhado). Aliasing é observável:
  `const b = a` seguido de `lists.push(a, x)` é visto por `b`. O texto do
  Spec 10 ("mutações produzem valores novos") descreve strings/bytes e as
  operações funcionais, não os mutadores de coleção.
- Custo atual dos caminhos quentes (2026-07-17/18, pós LANG-PERF-2/3 e
  LANG-MEM-*): builder-loop de listas ~320 ns/iter incluindo alloc+free
  (bench S3, 500k iterações); residual vs Rust em list workloads ~1,25×.
  Não há pressão de performance atribuível à ausência de COW.

## O que COW mudaria — e por que isso é o bloqueio

Adotar COW converteria os mutadores para "copia se compartilhado":
`b` **deixaria de ver** o push feito via `a`. Isso é **mudança semântica
observável** — proibida na janela FREEZE-1 sem bump para 0.4+ e nota de
saída de freeze. Não é uma otimização transparente no estado atual da
linguagem; seria uma migração de semântica de referência para semântica
de valor plena.

## Esboço técnico registrado (para quando/SE for retomado)

- Unicidade: `refcount == 1` no header já basta — edges de containers
  também contam no refcount, então "único" = nenhum binding nem container
  extra aponta para o objeto.
- Ponto de gancho: cada mutador de coleção (`ori_list_push`,
  `ori_map_set_*`, …) testaria unicidade antes de mutar; não-único →
  clone raso (elementos ganham +1 via edges do clone) e mutação no clone,
  com o binding do chamador reapontado — o que exige **out-param ou
  retorno do ponteiro** nos mutadores (mudança de ABI dos símbolos
  `ori_*`, hoje `void`).
- Custo por mutação no caso único: um load atômico do refcount (barato;
  o lock global já é pago pelas edges quando o elemento é managed).

## Decisão

**Deferir.** Reavaliar somente se (1) a janela FREEZE-1 fechar rumo a
0.4+/1.0 **e** (2) houver demanda real: relatos de bugs de aliasing em
programas de usuários ou workloads onde cópias defensivas manuais dominem
perfis. Nesse caso, retomar por aqui: decidir primeiro a **semântica**
(valor pleno vs referência documentada) e só então o mecanismo COW.

## Consequências imediatas

- Nenhuma mudança de código.
- Spec 10 permanece descrevendo o comportamento real (mutação in place,
  aliasing observável em coleções) — ajuste de redação incluído no F5.
- LANG-MEM-6 fecha como *decisão registrada*, completando o plano F0–F7
  do estudo Nim.
