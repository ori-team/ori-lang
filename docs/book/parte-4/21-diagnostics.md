# Cap. 21 — Diagnostics mais comuns

> **Versão âncora:** Ori 0.3.x (S3)  
> **Parte:** IV

## TL;DR

Todo diagnostic tem um **código** estável. Leia o código + a mensagem + a ação
sugerida. Catálogo completo: spec 13. Aqui: os que mais aparecem no dia a dia.

## Como ler um erro

1. **Código** — família (`name.*`, `bind.*`, `type.*`, …)  
2. **Span** — onde no arquivo  
3. **Mensagem** — o que falhou  
4. **Nota / help** — o que tentar  

## Famílias frequentes

| Prefixo | Significado típico |
|---------|---------------------|
| `name.*` | Nome não resolvido, privado, duplicado no topo |
| `bind.*` | Binding/import/campo/param (ex. import stdlib desconhecido) |
| `type.*` | Incompatibilidade de tipos |
| (códigos de forma pré-S3) | Sintaxe removida — migrar |

Convenção importante: o emitido para “não definido” é `name.undefined`
(`bind.undefined` é alias reservado no catálogo).

## Exemplo real

Código sem import:

```ori
module app.ex
main()
    io.println("boom")
end
```

`ori check` emite algo como:

```text
error[name.undefined]: undefined name `io.println`
   = action: declare or import the name before using it
```

Correção: `import ori.io = io`.

## Hábitos úteis

- `ori check` primeiro — barato.  
- Se veio de código antigo: `ori migrate-syntax`.  
- Em testes do compilador: catálogo deve bater com
  `diagnostic_catalog_matches_emitted_codes`.

## Ir mais fundo

- Catálogo: [`../../spec/13-error-catalog.md`](../../spec/13-error-catalog.md)
- Reportar bugs: [`../../guides/report-bugs.pt-BR.md`](../../guides/report-bugs.pt-BR.md)
