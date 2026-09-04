# Plano de Evolução e Refatoração Sintática (Ori v0.4+)

> **Status:** Proposta Aceita / Planejamento Arquitetural  
> **Data:** 2026-09-04  
> **Objetivo:** Reduzir verbosidade, simplificar vocabulário, eliminar redundâncias e alinhar a ergonomia com a filosofia *Reading-First*.

---

## 1. Mapeamento das Mudanças Sintáticas

| # | Construção Atual | Nova Proposta | Benefício | Impacto |
|---|---|---|---|:---:|
| **1** | `apply Type use Trait ... end` | `apply Type: TraitA, TraitB ... end` | Elimina keyword `use`, remove 1 nível de indentação, permite agrupamento de traits | **Médio** |
| **2** | `import path = alias` | `import path as alias` | Sintaxe natural e consagrada; restaura o papel de `as` sem colisão no parser | **Baixo** |
| **3** | `array[T, size: N]` e `simd[T, lanes: N]` | `array[T, N]` e `simd[T, N]` | Elimina verbosidade de labels desnecessários para tipos primitivos | **Baixo** |
| **4** | `any[Trait]` em assinaturas | `param: Trait` | Polimorfismo limpo e idiomático; elimina ruído sintático | **Médio** |
| **5** | Métodos inerentes em `apply Type` | Métodos inerentes dentro de `struct` | Centraliza a definição de dados e métodos próprios; `apply` fica exclusivo para traits | **Baixo** |

---

## 2. Análise de Conflito da Keyword `as`

* **Lexer:** `as` já é um token reservado (`TokenKind::As`) em `compiler/crates/ori-lexer/src/token.rs`.
* **Gramática:** No `import`, a posição após o caminho do módulo só aceitava `=` ou `(`; portanto, aceitar `as` não introduz lookahead extra nem ambiguidade de LL(1).
* **Renomeação de membros:** `import ori.fs (read_text as read)` também funciona perfeitamente sem conflitos.

---

## 3. Exemplo Comparativo Completo

### Sintaxe Antiga (S3)
```ori
import ori.io = io
import ori.mem = mem

struct Transform
    position: simd[float32, lanes: 4]
    matrix: array[float, size: 16]
end

trait Renderable
    render(self) -> void
end

apply Transform
    magnitude(self) -> float
        return 0.0
    end
end

apply Transform use Renderable
    render(self) -> void
        io.print("Render")
    end
end

draw(item: any[Renderable])
    item.render()
end
```

### Nova Sintaxe Proposta
```ori
import ori.io as io
import ori.mem as mem

struct Transform
    position: simd[float32, 4]
    matrix: array[float, 16]

    magnitude(self) -> float
        return 0.0
    end
end

trait Renderable
    render(self) -> void
end

apply Transform: Renderable
    render(self) -> void
        io.print("Render")
    end
end

draw(item: Renderable)
    item.render()
end
```

---

## 4. Plano de Execução (Fases)

1. **Fase 1 — Parser & AST:**
   - Habilitar `TokenKind::As` como alias canônico no parser de imports.
   - Suportar `:` opcional com lista de traits separadas por vírgula em `apply`.
   - Permitir números literais posicionais em `array[T, N]` e `simd[T, N]`.
   - Permitir nomes de traits diretamente em anotações de tipo de parâmetros.
2. **Fase 2 — Resolver & Type Checker:**
   - Mapear múltiplos contratos no bloco `apply Type: T1, T2`.
   - Mapear nomes de trait para `any[Trait]` implícito no checker quando usado em parâmetro.
3. **Fase 3 — Migração Automatizada (`ori migrate-syntax`):**
   - Atualizar testes e biblioteca padrão automaticamente com o script de reescrita.
4. **Fase 4 — Documentação & Site:**
   - Atualizar as especificações normativas (`docs/spec/`), guias e site oficial.
