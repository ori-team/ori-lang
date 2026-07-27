> **Histórico — não usar como backlog ativo.**  
> Para o roadmap atual, veja [`docs/planning/PLANO-MATURIDADE-COMPLETO.md`](planning/PLANO-MATURIDADE-COMPLETO.md) Etapa 6.

# Plano de Implementação Avançada do Ori LSP

Data: 2026-05-17
Status: Plano de implementação
Crate: `compiler/crates/ori-lsp`
Código atual: 1112 linhas em `src/main.rs`

---

## Sumário Executivo

O `ori-lsp` atual é um servidor LSP funcional (não um placeholder!) que já
implementa:

- ✅ `initialize` / `shutdown` / `initialized`
- ✅ `textDocument/didOpen` / `didChange` / `didSave` / `didClose`
- ✅ Publicação de diagnósticos (parser + checker via `ori_driver::pipeline::run_check`)
- ✅ Hover de tipos built-in (`int`, `string`, `bool`, `list`, `map`, `set`, etc.)
- ✅ Hover de símbolos locais (funções, structs, enums, traits, bindings)
- ✅ Go-to-definition local (regex-based)
- ✅ Completions da stdlib (~200 funções)

O LSP atual funciona como um **indexador léxico + checker integration**.
O plano abaixo transforma isso em um servidor **semântico completo** com
indexação cross-file, análise incremental e refactorings.

---

## Arquitetura Alvo

```
┌─────────────────────────────────────────────────┐
│                  Editor (VS Code / Neovim)       │
└──────────────────┬──────────────────────────────┘
                   │ LSP (stdin/stdout)
┌──────────────────▼──────────────────────────────┐
│              ori-lsp (tower-lsp)                 │
│                                                  │
│  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │
│  │ Handlers │  │ Semantic  │  │ Project      │  │
│  │ (LSP     │  │ Index     │  │ Manager      │  │
│  │  methods)│  │ (cross-   │  │ (workspace,  │  │
│  │          │  │  file)    │  │  multi-file) │  │
│  └──────────┘  └───────────┘  └──────────────┘  │
│                                                  │
│  ┌──────────────────────────────────────────┐    │
│  │         Compiler Integration              │    │
│  │  ori_lexer → ori_parser → ori_types      │    │
│  │  (reused from compiler crates)           │    │
│  └──────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

---

## Estado Atual vs Alvo

> Atualizado em 2026-06-28 para refletir as entregas dos Sprints 1–5 (CHANGELOG
> `[Unreleased]`) e da Etapa 6.1–6.6 do `PLANO-MATURIDADE-COMPLETO.md`. Esta
> tabela é um snapshot histórico; o roadmap ativo é o PLANO-MATURIDADE-COMPLETO.

| Funcionalidade | Atual | Alvo |
|---|---|---|
| Diagnostics (check) | ✅ Via `ori check` + LSP `publishDiagnostics` (debounced) | ✅ Incremental por arquivo |
| Diagnostics (lint) | ❌ | ✅ Warnings de estilo |
| Diagnostics (project) | ✅ `project.circular_import`/`namespace_file_mismatch` (driver) + `project.entry_not_found`/`no_proj_file` (LSP mapping); roteamento cross-file (Etapa 6.5) | ✅ |
| Hover (tipos built-in) | ✅ | ✅ |
| Hover (símbolos locais) | ✅ Semântico (AST via `SemanticIndex`) + cross-file via `ProjectSemanticIndex` (Etapa 6.1) | ✅ |
| Hover (cross-file) | ✅ `ProjectSemanticIndex` resolve sigs de imports transitivos (Etapa 6.1) | ✅ Resolução de imports |
| Hover (stdlib) | ✅ Assinaturas + docs | ✅ |
| Go-to-definition (local) | ✅ Semântico | ✅ Semântico cross-file |
| Go-to-definition (import) | ✅ Cross-file via `ProjectSemanticIndex.cross_file_definition` (Etapa 6.1) | ✅ Resolve imports |
| Go-to-definition (stdlib) | ❌ | ✅ Navega para declaração |
| Find references | ✅ Local + cross-file via `ProjectSemanticIndex.find_references_cross_file` (Etapa 6.2) | ✅ Todos os usos cross-file |
| Completions (stdlib) | ✅ Context-aware (Default/AfterDot/InType) | ✅ |
| Completions (local) | ✅ Escopo + imports | ✅ |
| Completions (dot) | ✅ Type-aware: `complete_after_dot` resolve tipo declarado do receptor e lista campos/variantes/métodos (Etapa 6.2) | ✅ Campos/métodos após `.` por tipo real |
| Signature help | ✅ | ✅ |
| Document symbols | ✅ | ✅ |
| Workspace symbols | ✅ | ✅ |
| Code lens | ✅ `@test` runner, references | ✅ |
| Rename | ✅ Local + imports + cross-file via `ProjectSemanticIndex` (Etapa 6.2) | ✅ Cross-file via DefMap |
| Formatting | ✅ `ori fmt` + LSP `textDocument/formatting` (idempotente em async) | ✅ |
| Code actions | ✅ Quick fixes | ✅ |
| Inlay hints | ✅ Tipos inferidos | ✅ |
| Semantic tokens | ✅ Syntax highlighting | ✅ |
| Diagnostics on change | ✅ Incremental, debounced (300ms) | ✅ |
| E2E test harness | ✅ `ori-lsp/tests/e2e.rs` (10+ cenários via subprocess) | ✅ |

**Pendências remanescentes (Etapa 6.1/6.2/6.5 do PLANO-MATURIDADE-COMPLETO):**
`ProjectSemanticIndex` cross-file com reuso do pipeline do driver, completion
type-aware de membros após `.` (baseada no tipo inferido real), find references
cross-file via DefMap, e diagnósticos de nível de projeto (`project.circular_import`,
`project.namespace_file_mismatch`, etc.) no LSP.

---

## Fase 1 — Fundamentos (Semanas 1-2)

### 1.1 Refatorar para arquitetura multi-file

**Estado atual:** Tudo em `main.rs` (1112 linhas, monolítico)
**Alvo:** Separar em módulos

```
src/
  main.rs            # Entry point + server setup
  handlers/
    mod.rs
    diagnostics.rs   # Publish diagnostics
    hover.rs         # Hover provider
    definition.rs    # Goto definition
    completion.rs    # Completion provider
    symbols.rs       # Document/workspace symbols
  index/
    mod.rs
    semantic.rs      # Semantic index (cross-file, AST-based)
    project.rs       # Project/workspace manager
  analysis/
    mod.rs
    checker.rs       # Integration with ori_types
    resolver.rs      # Cross-file name resolution
  utils/
    mod.rs
    position.rs      # Line/col <-> byte offset
    uri.rs           # URI <-> file path
```

### 1.2 Substituir SemanticIndex regex por AST-based

**Estado atual:** `build_semantic_index()` usa regex para parsear structs,
funções, bindings. Frágil com código complexo.

**Alvo:** Reusar o parser real (`ori_parser`) para construir o índice.

```rust
struct SemanticIndex {
    // File-level symbols
    symbols: HashMap<Url, FileSymbols>,
    // Cross-file resolution cache
    imports: HashMap<Url, Vec<ResolvedImport>>,
}

struct FileSymbols {
    functions: Vec<FunctionSymbol>,
    structs: Vec<StructSymbol>,
    enums: Vec<EnumSymbol>,
    traits: Vec<TraitSymbol>,
    bindings: Vec<BindingSymbol>,
}
```

**Benefícios:**
- Precisão 100% (usa o parser real)
- Suporte a código com erros (parser tem recovery)
- Spans exatos para hover/go-to-definition

### 1.3 Project Manager

Gerenciar múltiplos arquivos abertos e dependências:

```rust
struct ProjectManager {
    /// Documentos abertos (buffer em memória)
    open_documents: HashMap<Url, DocumentState>,
    /// Workspace root
    workspace_root: Option<PathBuf>,
    /// Cache de arquivos parseados
    parse_cache: LruCache<PathBuf, ParsedFile>,
}

struct DocumentState {
    uri: Url,
    content: String,
    version: i32,
    /// Índice semântico do arquivo
    index: Option<FileSymbols>,
    /// Diagnósticos da última análise
    diagnostics: Vec<LspDiagnostic>,
}
```

---

## Fase 2 — Completions e Navegação (Semanas 3-4)

### 2.1 Completions context-aware

**Estado atual:** Lista plana de todas as funções da stdlib.

**Alvo:**

```
Contexto                    → Sugestões
─────────────────────────────────────────
após `import ori.`         → módulos: io, string, list, math, ...
após `import ori.io as io` → funções de io: print, eprint, read_line
após `x.` (struct)         → campos da struct
após `x.` (trait object)   → métodos da trait
após `func`                → sem completions
dentro de bloco            → variáveis locais + globais
dentro de `implement T for`→ métodos exigidos pela trait
após `case` em `match`     → variantes do enum
```

**Implementação:**
1. Determinar o contexto sintático (cursor position → AST node)
2. Filtrar símbolos visíveis no escopo
3. Para `.`, resolver o tipo do receiver e listar campos/métodos
4. Gerar `CompletionItem` com:
   - `label`: nome
   - `kind`: Function, Struct, Field, Method, Variable, Module
   - `detail`: assinatura (ex: `func print(value: string) -> void`)
   - `documentation`: doc comment (Markdown)
   - `insert_text`: snippet com placeholders para parâmetros

### 2.2 Snippets inteligentes

```rust
CompletionItem {
    label: "func".to_string(),
    kind: CompletionItemKind::SNIPPET,
    insert_text: Some("func ${1:name}(${2:params}) -> ${3:ret}\n    ${0}\nend".to_string()),
    insert_text_format: Some(InsertTextFormat::SNIPPET),
}
```

Templates:
- `func` → declaração de função
- `struct` → declaração de struct
- `enum` → declaração de enum
- `trait` → declaração de trait
- `implement` → bloco de implementação
- `match` → match expression com cases
- `if` → if/then/else
- `while` → while loop
- `for` → for-in loop
- `using` → using statement

### 2.3 Go-to-definition cross-file

**Estado atual:** Apenas definições no mesmo arquivo (regex).

**Alvo:**

1. **Local:** Usar AST parser para localizar definição exata
2. **Import:** Resolver `import x.y as z` e navegar para o arquivo importado
3. **Stdlib:** Navegar para declarações built-in (virtual file ou documentação)
4. **Método:** Navegar para `func` dentro de `implement Trait for Type`
5. **Campo:** Navegar para declaração do campo na struct

```rust
async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    // 1. Parse the file (with error recovery)
    let parsed = self.project_manager.parse_file(&uri).await?;

    // 2. Find the symbol at cursor position
    let symbol = parsed.symbol_at(pos)?;

    // 3. Resolve the definition
    match symbol.kind {
        SymbolKind::LocalVar => self.find_local_definition(&parsed, &symbol),
        SymbolKind::Function => self.find_function_definition(&symbol),
        SymbolKind::Import => self.resolve_import_target(&symbol),
        SymbolKind::Field => self.find_field_declaration(&symbol),
        SymbolKind::Method => self.find_method_implementation(&symbol),
    }
}
```

### 2.4 Find References

```rust
async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
    let symbol = self.resolve_symbol_at(params.position)?;

    // Buscar em:
    // 1. Arquivo atual (AST walk)
    // 2. Arquivos importados que usam o símbolo
    // 3. Cache de workspace

    let mut locations = Vec::new();
    for uri in self.project_manager.relevant_files(&symbol) {
        let parsed = self.project_manager.parse_file(&uri).await?;
        locations.extend(parsed.find_references(&symbol));
    }
    Some(locations)
}
```

---

## Fase 3 — Análise e Diagnósticos (Semanas 5-6)

### 3.1 Diagnósticos incrementais

**Estado atual:** `ori check` completo a cada alteração de texto.

**Alvo:** Análise incremental — re-check apenas o que mudou.

```rust
async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;

    // 1. Atualizar buffer
    self.project_manager.update_document(&uri, params.content_changes);

    // 2. Debounce (300ms após última alteração)
    self.schedule_debounced_check(&uri, Duration::from_millis(300));
}

async fn run_incremental_check(&self, uri: &Url) {
    // 1. Parse do arquivo alterado
    let parsed = self.project_manager.reparse(uri).await?;

    // 2. Type-check do arquivo alterado + arquivos que o importam
    let affected = self.project_manager.transitive_dependents(uri);
    for file_uri in affected {
        let diagnostics = self.run_check(&file_uri).await?;
        self.client.publish_diagnostics(file_uri, diagnostics, None).await;
    }
}
```

### 3.2 Lint warnings

Diagnósticos adicionais além do type checker:

| Lint | Descrição |
|---|---|
| `unused-import` | Import não utilizado (já existe) |
| `unused-variable` | Variável declarada mas não usada |
| `unused-function` | Função privada não chamada |
| `shadowed-variable` | Variável sombreia outra no escopo externo |
| `redundant-type` | Tipo redundante que pode ser inferido |
| `prefer-const` | `var` que nunca é mutado → sugerir `const` |
| `missing-doc` | Função/struct pública sem doc comment |

### 3.3 Code Actions (Quick Fixes)

```rust
async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
    let diagnostics = params.context.diagnostics;

    let mut actions = Vec::new();

    for diag in &diagnostics {
        match diag.code.as_ref().map(|c| c.as_str()) {
            Some("name.undefined") => {
                // Sugerir: "did you mean X?"
                actions.push(self.suggest_similar_name(&diag));
            }
            Some("type.type_mismatch") => {
                // Sugerir: adicionar conversão explícita
                actions.push(self.suggest_type_conversion(&diag));
            }
            Some("type.unused_result") => {
                // Sugerir: adicionar `?` ou `const _ =`
                actions.push(self.suggest_propagate_or_discard(&diag));
            }
            _ => {}
        }
    }

    Some(actions)
}
```

---

## Fase 4 — Features Avançadas (Semanas 7-8)

### 4.1 Inlay Hints

Mostrar tipos inferidos e nomes de parâmetros:

```ori
const x := 42           // → const x: int = 42
func main()
    calculate(42, true) // → calculate(count: 42, active: true)
```

```rust
async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
    // Para const/var sem tipo explícito, mostrar o tipo inferido
    // Para chamadas de função, mostrar nomes de parâmetros
}
```

### 4.2 Semantic Tokens

Destaque sintático semântico (melhor que regex do editor):

| Token Type | Exemplo |
|---|---|
| `namespace` | `namespace app.main` |
| `type` | `struct`, `enum`, `trait` |
| `function` | `func main()` |
| `parameter` | `(name: string)` |
| `variable` | `const x: int = 1` |
| `property` | `point.x` |
| `keyword` | `if`, `while`, `return` |
| `operator` | `+`, `-`, `==`, `?` |

### 4.3 Document Symbols

Estrutura hierárquica do arquivo:

```
app.main
  ├── struct User { id, name }
  ├── trait Displayable
  ├── implement Displayable for User
  │   └── func display() -> string
  └── func main() -> void
```

### 4.4 Workspace Symbols

Busca cross-file por símbolo (`Ctrl+T` no VS Code):

```rust
async fn symbol(&self, params: WorkspaceSymbolParams) -> Result<Option<Vec<SymbolInformation>>> {
    let query = params.query.to_lowercase();
    let mut results = Vec::new();

    for file in self.project_manager.all_files() {
        let symbols = file.search_symbols(&query);
        results.extend(symbols);
    }

    // Ordenar por relevância (match exato > prefixo > substring)
    results.sort_by(|a, b| relevance(&query, a).cmp(&relevance(&query, b)));
    Some(results.into_iter().take(50).collect())
}
```

### 4.5 Rename

Renomear símbolo e todos os seus usos:

```rust
async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    let symbol = self.resolve_symbol_at(params.position)?;
    let references = self.find_all_references(&symbol).await?;

    let mut changes = HashMap::new();
    for (uri, ranges) in group_by_uri(references) {
        let edits = ranges.into_iter().map(|range| TextEdit {
            range,
            new_text: params.new_name.clone(),
        }).collect();
        changes.insert(uri, edits);
    }

    Some(WorkspaceEdit { changes: Some(changes), ..Default::default() })
}
```

### 4.6 Signature Help

Mostrar assinatura da função durante digitação de argumentos:

```
calculate(█
         ─────────────────────────────────
         func calculate(count: int, active: bool = true) -> result<void, string>
         ─────────────────────────────────
```

### 4.7 Code Lens

- `@test` → "▶ Run Test" / "🔍 Debug Test"
- Referências → "↑ 5 references"

---

## Fase 5 — Integração com Ferramentas (Semanas 9-10)

### 5.1 `ori fmt` via LSP

```rust
async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
    let formatted = ori_driver::pipeline::format_source(&params.text_document.uri)?;
    // Retornar edição que substitui o documento inteiro
    Some(vec![TextEdit {
        range: full_document_range(&formatted),
        new_text: formatted,
    }])
}
```

### 5.2 `ori doc` via LSP

Hover com documentação rica:

```rust
fn format_hover(symbol: &FunctionSymbol) -> String {
    format!(
        "```ori\n{}\n```\n\n---\n{}\n\n**Parameters:**\n{}\n\n**Returns:** {}",
        symbol.signature,
        symbol.doc_comment.unwrap_or_default(),
        format_params(&symbol.params),
        symbol.return_type,
    )
}
```

### 5.3 Test Runner via Code Lens

Executar `ori test` para funções marcadas com `@test`:

```rust
async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>> {
    match params.command.as_str() {
        "ori.runTest" => {
            let test_name = params.arguments[0].as_str().unwrap();
            let result = run_single_test(test_name)?;
            Ok(Some(serde_json::json!({ "passed": result.passed })))
        }
        _ => Ok(None),
    }
}
```

---

## Plano de Implementação por Sprint

### Sprint 1 (Semana 1-2): Refatoração + Indexação AST
- [ ] Separar `main.rs` em módulos
- [ ] Substituir SemanticIndex regex por AST-based
- [ ] Project Manager com cache de parse
- [ ] Manter funcionalidades existentes funcionando

### Sprint 2 (Semana 3-4): Completions + Navegação
- [ ] Completions context-aware (escopo, tipo, dot)
- [ ] Snippets para construções da linguagem
- [ ] Go-to-definition cross-file
- [ ] Find references básico (arquivo atual)

### Sprint 3 (Semana 5-6): Diagnósticos + Code Actions
- [ ] Diagnósticos incrementais com debounce
- [ ] Lint warnings
- [ ] Code Actions (quick fixes)
- [ ] Document Symbols hierárquico

### Sprint 4 (Semana 7-8): Features Avançadas
- [ ] Inlay Hints (tipos inferidos, nomes de parâmetros)
- [ ] Semantic Tokens
- [ ] Workspace Symbols (busca cross-file)
- [ ] Rename cross-file
- [ ] Signature Help
- [ ] Code Lens (test runner, references)

### Sprint 5 (Semana 9-10): Integração
- [ ] Formatação via `ori fmt`
- [ ] Hover com documentação rica
- [ ] Test Runner integrado
- [ ] Testes end-to-end com editor real
- [ ] Documentação do LSP

---

## Dependências Técnicas

### Crates necessários (já no workspace)
- `tower-lsp` 0.20 — ✅ Já usado
- `lsp-types` 0.95 — ✅ Já usado
- `tokio` 1.x — ✅ Já usado
- `ori-parser` — ✅ Já disponível (adicionar ao Cargo.toml do LSP)
- `ori-ast` — ✅ Já disponível
- `ori-types` — ✅ Já usado
- `ori-diagnostics` — ✅ Já usado
- `ori-driver` — ✅ Já usado

### Crates a adicionar
- `dashmap` ou `parking_lot::RwLock` — cache thread-safe

### Compiler changes necessários
- `ori-parser`: Expor API pública para parsing com recovery (`parse_with_recovery`)
- `ori-types`: Expor `resolve_def_id` e lookup de símbolos como API pública
- `ori-driver`: Expor `run_check` com suporte a fonte em memória (já existe `run_check_source`)

---

## Métricas de Sucesso

| Métrica | Estado Atual | Alvo |
|---|---|---|
| Funcionalidades LSP | 6/25 | 25/25 |
| Precisão de hover | Regex (~80%) | AST (100%) |
| Latência de diagnóstico | Full check (1-3s) | Incremental (<200ms) |
| Completions | Stdlib plana | Context-aware + snippets |
| Cobertura de testes LSP | 0 testes | >80% |
| Módulos Rust | 1 arquivo | 15+ arquivos |

---

## Riscos

| Risco | Mitigação |
|---|---|
| Parser lento para arquivos grandes | Cache de parse, análise incremental |
| Cross-file resolution complexo | Reusar `ori-driver` resolve existente |
| Memory usage com muitos arquivos | LRU cache no ProjectManager |
| Quebra de compatibilidade | Manter testes de regressão das funcionalidades existentes |
