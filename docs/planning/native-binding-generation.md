# Plano de implementação — geração de bindings nativos

> **Status:** aprovado; implementação começa após metadata e Host ABI básico.  
> **Baseline:** Ori declara `extern c` manualmente e gera headers para
> `@c_export`; não importa headers C nem gera wrappers de entrada.

## 1. Resultado desejado

Reduzir o custo de integrar bibliotecas C grandes sem esconder ownership,
layouts ou falhas atrás de código gerado opaco.

Fluxo alvo:

```text
C headers + target configuration
  → modelo de binding versionado
  → declarations Ori de baixo nível
  → wrappers Ori opcionais e legíveis
  → compile/check do resultado em CI
```

## 2. Escopo inicial

- funções, constantes e typedefs escalares;
- enums C simples;
- structs por layout `@repr("C")`;
- pointers transformados em handles/pointers explícitos;
- callbacks com `user_data`;
- allowlist/rename por arquivo de configuração;
- saída determinística e regenerável.

Unions, bitfields, variadics e macros complexas começam como diagnostics de
“não suportado”, não como aproximações silenciosas.

## 3. Fases

| ID | Entrega | Critério observável |
|---|---|---|
| **FFI-BINDGEN-1.0** | schema intermediário | modelo não depende de AST interna do parser C escolhido |
| **FFI-BINDGEN-1.1** | funções/typedefs/constants | header pequeno gera Ori que passa `ori check` |
| **FFI-BINDGEN-1.2** | structs/enums/layout | `sizeof/alignof/offset` conferem em harness C |
| **FFI-BINDGEN-1.3** | callbacks/opaque handles | lifecycle e thread contract aparecem nos wrappers |
| **FFI-BINDGEN-1.4** | configuração e diffs | regeneração mostra mudanças de ABI revisáveis |
| **FFI-BINDGEN-1.5** | package integration | binding participa de lockfile/cache sem executar código arbitrário |
| **FFI-BINDGEN-1.6** | corpus real | duas bibliotecas gerais validam escala e diagnostics |

## 4. Decisões arquiteturais

- parser C é ferramenta de build, não dependência do runtime Ori;
- arquivo gerado de baixo nível não é editado manualmente;
- wrappers ergonômicos ficam em arquivos Ori normais;
- o gerador nunca infere ownership de um pointer sem configuração/evidência;
- ABI é medida por harness, não apenas por tipos textualmente parecidos;
- main-thread affinity é metadata explícita consumível por wrappers/hosts.

## 5. Validação

- headers adversariais e limites de nesting/tamanho;
- nomes reservados, includes condicionais e targets diferentes;
- golden somente para schema estável, não whitespace irrelevante;
- compile do binding + harness C real;
- ASan/LSan nos exemplos de ownership;
- alteração de header invalida cache e produz diff determinístico;
- nenhuma execução de macro/comando arbitrário sem autorização explícita.

## 6. Fora de escopo

- prometer C++ completo na primeira versão;
- converter automaticamente herança/templates em semântica Ori;
- gerar API de engine dentro do compilador;
- esconder funções inseguras como se fossem seguras.
