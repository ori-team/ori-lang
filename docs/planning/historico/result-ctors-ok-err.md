# M2.result-ctors — `success`/`error` → `ok`/`err`

> **Status:** **entregue** (2026-07-13) — corte seco + `parse.result_ctor_renamed` + migrate  
> **Pai:** M2 (superfície + stdlib + layout)  
> **Breaking:** sim (nomes de construção/padrão de `result`)

---

## Decisão

| Forma | Canônica (alvo) | Legado (hoje) |
|-------|-----------------|---------------|
| Construtor ok | **`ok(value)`** | `success(value)` |
| Construtor err | **`err(value)`** | `error(value)` |
| Pattern ok | **`case ok(x):`** | `case success(x):` |
| Pattern err | **`case err(e):`** | `case error(e):` |

Tipo continua **`result[T, E]`** (não muda).

Alinhamento mental: Rust `Ok`/`Err`, Go-style curto, menos colisão semântica com a palavra “error” de diagnóstico.

**Soft keywords:** `ok`/`err` só são especiais quando seguidos de `(` (construtor/padrão). Bindings como `const err: T = ...` e `case ok(ok):` continuam válidos.

---

## É fácil?

**Sim — mecânico e contido**, não é redesenho de runtime.

| Camada | Trabalho | Dificuldade |
|--------|----------|-------------|
| Lexer | tokens `ok` / `err`; deprecar/remover `success` / `error` (kw de result) | baixa |
| Parser | expr + pattern usam novos tokens | baixa |
| AST | `Pattern::Success/Error` → `Ok/Err` (ou alias interno) | baixa |
| Types / HIR / codegen | renomear match arms; tags de result no runtime **não** dependem do nome de superfície se o lower já usa tag 0/1 | baixa–média |
| Fontes `.orl` | ~**100** usos em ~**18** arquivos stdlib/examples/tests (ordem de grandeza) | mecânico |
| Spec / README / guides | 05, 06, 09, 10, cookbook, READMEs | mecânico |
| `ori migrate-syntax` | reescrever `success(`→`ok(`, `error(`→`err(`, `case success`→`case ok`, etc. | baixa |
| Catálogo | se houver mensagem citando `success`/`error` | baixa |

**Cuidados:**

1. **`error` como keyword** hoje também é o construtor de result — renomear para `err` reduz ambiguidade com “compile error”. Garantir que `error` não fique keyword residual sem uso (ou vire erro `parse.success_error_removed` / `parse.result_ctor_renamed`).
2. **`ok` / `err` como identificadores** de usuário: viram keywords (ou contextuais só em posição de call/pattern). Preferir **keywords** iguais a `some`/`none` para consistência.
3. **Dual transitório (opcional):** aceitar ambos por 1 fatia + warning; corte seco no mesmo estilo S3 se preferir clareza. Recomendação: **corte seco + migrate-syntax**, como o resto da superfície.
4. **Não** confundir com `Diagnostic::error` / `Ty::Error` no compilador Rust — só superfície Ori.

**Estimativa:** 1 fatia focada (lexer→parser→check→migrate→stdlib/examples/tests/docs), validada com `ori_spec` + `multifile_imports` filtrado + `diagnostic_catalog` se novos códigos.

---

## Plano de implementação

| Passo | Entrega |
|-------|---------|
| 1 | Spec 05/06/09 + catálogo (novos códigos de forma removida, se corte seco) |
| 2 | Lexer/parser/AST + checker patterns |
| 3 | HIR/codegen se nomes de variante aparecerem em dump/diagnostics |
| 4 | `ori migrate-syntax` rewrites |
| 5 | Migrar `stdlib/**`, `examples/**`, fixtures de teste |
| 6 | READMEs + guides |
| 7 | Testes de regressão (aceita `ok`/`err`; rejeita `success`/`error` se corte seco) |

Ordem relativa em M2: pode ir **em paralelo** a merge de módulos e layout, ou **logo após** layout scaffold — não depende de ABI/Rust-indep.

---

## Exemplos-alvo

```ori
load(path: string) -> result[string, string]
    match fs.read_text(path)
        case ok(text):
            return ok(text)
        case err(msg):
            return err(msg)
    end
end

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("division by zero")
    end
    return ok(a / b)
end
```

---

## Histórico

| Data | Evento |
|------|--------|
| 2026-07-13 | Pedido de produto; viabilidade confirmada; item **M2.result-ctors** no plano |
| 2026-07-13 | Implementado: tokens `ok`/`err`, patterns, checker/HIR, migrate-syntax, stdlib/examples/tests/docs |
