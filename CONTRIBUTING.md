# Contributing to Ori Language

Obrigado por contribuir com o compilador Ori.

Este arquivo cobre:

1. Como enviar contribuição
2. Gates de qualidade (build, testes, catálogo, formatter)
3. Política de runtime empacotado e artefatos pre-built
4. Licenciamento de contribuições
5. Regras para código de terceiros

## 1) Fluxo curto

1. Abra issue (bug/feature) com contexto mínimo.
2. Faça branch com foco único.
3. Adicione testes obrigatórios:
   - bugfix: teste de regressão em `compiler/crates/ori-driver/tests/`;
   - feature: teste positivo e teste negativo.
4. Rode os gates do projeto antes de abrir PR.
5. Atualize docs no mesmo PR quando houver mudança de comportamento
   (spec em `docs/spec/`, planejamento em `docs/planning/`, `CHANGELOG.md`).
6. Abra PR com descrição curta e objetiva.

## 2) Gates de qualidade

Gate mínimo oficial (local), rodar a partir da raiz do workspace:

```bash
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-lsp
```

Para mudanças que afetam a stdlib, rode também os testes de paridade do manifesto:

```bash
cargo test -p ori-types --lib stdlib
```

Para mudanças que afetam o runtime nativo, re-stage o runtime local antes de rodar
testes `compile_runs` (senão o `OnceLock` cache pode reter um runtime quebrado):

```bash
cargo build -p ori-runtime --lib
cp target/debug/libori_runtime.a runtime/x86_64-unknown-linux-gnu/
# Windows MSVC:
.\tools\stage_native_runtime.ps1
```

### Regras contínuas obrigatórias

- Todo bug novo deve incluir teste de regressão.
- Toda feature nova deve incluir teste positivo e negativo.
- Toda mudança de comportamento deve atualizar docs no mesmo PR.
- Regressão crítica de performance bloqueia merge sem override documentado.
- Divergência spec × código deve ser classificada em P0/P1/P2.
- Códigos de diagnóstico novos devem ser registrados em
  `docs/spec/13-error-catalog.md` e o teste `diagnostic_catalog` deve continuar
  passando (consistência bidirecional emitted ↔ catálogo).

### Evidência mínima de fechamento

Todo PR de fechamento deve registrar:

- comando executado + resultado;
- arquivo de teste novo ou alterado;
- commit/PR de fechamento;
- risco residual (se houver).

## 3) Runtime empacotado e artefatos pre-built

O runtime nativo (`libori_runtime.a` / `ori_runtime.lib`) é gerado a partir de
`compiler/crates/ori-runtime` via `tools/stage_native_runtime.{ps1,sh}`.

### Triples versionados vs gerados em CI

| Triple | Versionado em git? | Gerado em CI? |
|---|---|---|
| `x86_64-pc-windows-msvc` | sim | sim |
| `x86_64-unknown-linux-gnu` | sim | sim |
| `x86_64-pc-windows-gnu` | não | sim |
| `x86_64-apple-darwin` | não | sim |
| `aarch64-apple-darwin` | não | sim |

**Política:** apenas os dois triples de desenvolvimento canônicos (Windows MSVC
e Linux GNU) são versionados em git como baseline. Os outros três são staging
apenas em CI e em release packages; **não commit** artefatos pre-built para
esses triples — o `.gitignore` os ignora e eles devem ser regenerados a cada
release via `tools/stage_native_runtime.{ps1,sh}` com o `-Target` apropriado.

### Release package

Um release package deve conter:

```text
ori.exe                         # ou `ori` no Unix
runtime/
  {target-triple}/
    {runtime-artifact}
    runtime-link.json
examples/
README.md
```

Para validar um release package localmente:

```bash
# Windows
.\tools\smoke_native_release.ps1
# Unix
sh tools/smoke_native_release.sh
```

O smoke roda com `ORI_REQUIRE_PACKAGED_RUNTIME=1` para garantir que não haja
fallback silencioso para o runtime do workspace.

## 4) Licença de contribuições

Ao enviar código para este repositório, você concorda que sua contribuição é
licenciada como:

- Apache-2.0 OR MIT

Regra padrão (inspirada no ecossistema Rust):

- A menos que você declare o contrário explicitamente,
- toda contribuição submetida intencionalmente para o projeto,
- entra no projeto sob licença dupla Apache-2.0 OR MIT,
- sem termos adicionais.

## 5) Regras para código de terceiros

Antes de copiar/portar código de outro projeto:

- confirme compatibilidade de licença com Apache-2.0 OR MIT;
- preserve avisos de copyright exigidos;
- cite a origem no PR;
- evite qualquer código sob licença incompatível.

Não enviar:

- código GPL que exija relicenciamento do Ori;
- conteúdo sem autorização clara;
- código com termos adicionais restritivos.

## 6) Sign-off (DCO simples)

Recomendado em commits de contribuição:

```
Signed-off-by: Seu Nome <seu-email>
```

Exemplo de comando:

```bash
git commit -s -m "fix: corrigir parser edge case"
```

## 7) Checklist de PR

- [ ] problema está claro
- [ ] testes adicionados/atualizados
- [ ] documentação atualizada se houve mudança de comportamento
- [ ] sem regressão nos comandos de gate
- [ ] origem/licença de código externo validada
- [ ] se mudou stdlib: manifesto `stdlib.rs` + parity tests verdes
- [ ] se mudou diagnósticos: catálogo `13-error-catalog.md` + `diagnostic_catalog` verde
