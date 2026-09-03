# Verificação dos 4 bugs reportados pelo native-ori-ide (Ori 0.3.5)

- Data: 2026-07-18
- Fonte do relato: `/home/raillen/Documentos/Projetos/native-ori-ide/`
  (comentários em `fs/files.orl`, `core/checker.orl`, `ui/project_pane.orl`,
  `lsp/protocol.orl` + CHANGELOG.md do projeto)
- Compilador testado: master pós-onda LANG-MEM (F1–F4 + fix de ABI uext)

## 1. TL;DR

Dos 4 bugs reportados contra a 0.3.5 de release: **as duas corrupções de
memória (bugs 1 e 2) não reproduzem mais** — a classe foi eliminada pela
onda ARC desta semana (dono único da cascata + contabilidade de temps).
**O crash do Cranelift (bug 3) reproduzia e foi corrigido nesta sessão.**
O bug 4 (LSP float-vs-int) **persiste por design** do `json.Value` — é
decisão de API, não correção. A verificação ainda rendeu **2 fixes bônus**
(scrutinee de match/if-some vazava; `none` fora da lista de owned) e **2
achados novos** para o backlog.

## 2. Veredito por bug

| # | Bug reportado | Veredito | Ação |
|---|---------------|----------|------|
| 1 | String de `ori.fs` empacotada em struct custom corrompe sob 2+ módulos locais | **Corrupção não reproduz** (20 iterações multi-módulo, conteúdo íntegro). Era a classe dtor×edges/temps corrigida em LANG-MEM-1/2 | Nenhuma — já corrigido. Bônus: leak de scrutinee achado no caminho (§3) |
| 2 | Struct-de-struct-com-lista corrompe ao reatribuir o wrapper por 2+ frames | **Corrupção não reproduz** (60 "frames" com verificação de conteúdo por iteração, 0 corrupções, 0 leaks) | Nenhuma — já corrigido |
| 3 | Panic Cranelift `declared type of variable varN doesn't match type of value vNN` com binding de mesmo nome em match aninhado | **Reproduzido** (payload `float` externo × `string` interno, mesmo nome `value`) e **corrigido** | Fix nos 5 sites que reusavam `Variable` sem checar tipo (§4) |
| 4 | `json.Value.Number` sempre float → `id`/`version` LSP serializados `1.0`, rejeitados pelo tower-lsp | **Persiste por design**: o enum builtin só tem `Number(value: float)` (`ori-types/src/resolve.rs`, `builtin_stdlib_json_value_enum_sig`) | Não alterado — mudar o enum público é decisão de API (registrar como candidato a discussão de stdlib; o workaround do IDE é válido) |

## 3. Fix 1 — scrutinee owned de match/if-some nunca era liberado

Achado ao re-verificar o bug 1: `match mk(i)` (scrutinee = temporário
managed owned, não um `Var`) extraía payloads por load simples (borrow) e
**nunca liberava o wrapper** — 1-2 leaks por match executado; o mesmo em
`if some(x) = mk(i)`.

Fix em `native_backend.rs`, seguindo a regra do ADR de dono único:

- `emit_match`: scrutinee owned → `bind_pattern` com `retain_bindings`
  (cada leaf managed extraído ganha +1 próprio e entra no `managed_stack`
  do arm, com truncamento por arm para não vazar entries entre arms) e
  release do scrutinee logo após o bind.
- `emit_if_some`: mesma regra; o caminho "none" ganha um else-block real
  (mesmo sem `else` do usuário) para liberar o wrapper.
- Bônus: `HirExprKind::None_` faltava em `expr_produces_owned_ref` —
  `return none` deixava o wrapper optional sem dono.

## 4. Fix 2 — reuso de `Variable` do Cranelift sem checar tipo

`bind_pattern`, `HirStmt::Let`, `HirStmt::Using`, `emit_if_some` e
`emit_while_some` reusavam a `Variable` existente quando `lookup_var`
achava o nome em **qualquer** escopo da pilha — inclusive um arm de match
externo. Com tipos nativos diferentes (ex.: `f64` × ponteiro), o
`def_var` estourava o panic interno do Cranelift. Fix: só reusar quando o
tipo bate (`.filter(|(_, t)| t == alvo)`); senão declara `Variable` nova
(shadowing léxico correto).

## 5. Achados novos para o backlog (não corrigidos aqui)

1. **`new_result` do runtime nativo não é ARC-managed**: `ori.fs.read_text`
   e ~134 call sites em `ori-runtime/src/lib.rs` constroem
   `result`/`optional` com `libc::malloc` cru (sem `ori_alloc`/registro).
   Esses wrappers ficam invisíveis ao ARC — o release compensatório do
   codegen vira no-op e o par retain/release fica órfão (20 leaks em 20
   chamadas de `fs.read_text_or` em loop). Precisa de campanha própria
   (migrar `new_result`/`new_optional_*` para `ori_alloc` + edges).
2. **Builtin bare `len` sombreia variável local**: `const len: int = ...`
   falha com `undefined variable ori_len` — a resolução de nomes prefere o
   builtin sem prefixo (`stdlib!("len", ...)`) ao local. Bug de frontend
   (ori-types/ori-hir).

## 6. Regressão

4 testes novos em `memory_arc.rs` (28 no arquivo, 27 rodam + 1 ignored
pré-existente): scrutinee owned de match (loop), payload retornado do
match (interage com return-transfer elision), if-some owned nos dois
caminhos, e o repro do crash de shadowing (float × string). Suíte
completa idêntica à baseline (51/1 + 340/8 pré-existentes de
closures/C-backend).
