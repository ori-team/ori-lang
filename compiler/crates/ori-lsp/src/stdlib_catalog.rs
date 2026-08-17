//! Standard library catalog for LSP completion, hover, and go-to-definition.
//!
//! Merges Layer 1 (runtime manifest in `ori-types`) with Layer 2 (`.orl` sources
//! under `stdlib/`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ori_ast::item::{FuncDecl, Item};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Range};

use crate::utils::position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibLayer {
    Runtime,
    Orl,
}

#[derive(Debug, Clone)]
pub struct StdlibEntry {
    pub qualified: String,
    pub module: String,
    pub name: String,
    pub signature: String,
    pub layer: StdlibLayer,
    pub source_path: Option<PathBuf>,
    pub name_range: Option<Range>,
    pub documentation: Option<String>,
}

#[derive(Debug, Default)]
pub struct StdlibCatalog {
    by_qualified: HashMap<String, StdlibEntry>,
    by_module: BTreeMap<String, Vec<String>>,
    modules: BTreeSet<String>,
}

static CATALOG: OnceLock<StdlibCatalog> = OnceLock::new();

pub fn stdlib_catalog() -> &'static StdlibCatalog {
    CATALOG.get_or_init(build_catalog)
}

fn build_catalog() -> StdlibCatalog {
    let mut catalog = StdlibCatalog::default();

    for entry in ori_types::stdlib::stdlib_runtime_functions() {
        let canonical = entry.canonical_path.to_string();
        let (module, name) = canonical
            .rsplit_once('.')
            .map(|(m, n)| (m.to_string(), n.to_string()))
            .unwrap_or_else(|| (canonical.clone(), canonical.clone()));

        let signature = ori_driver::pipeline::stdlib_doc_signature(&canonical)
            .map(str::to_string)
            .or_else(|| format_ty_sig(&canonical))
            .unwrap_or_else(|| format!("{name}(...)"));

        let doc = stdlib_documentation(&canonical);

        catalog.insert(StdlibEntry {
            qualified: canonical.clone(),
            module: module.clone(),
            name,
            signature,
            layer: StdlibLayer::Runtime,
            source_path: None,
            name_range: None,
            documentation: doc,
        });

        for alias in entry.aliases {
            catalog.insert_alias(alias, &canonical);
        }
    }

    if let Some(root) = ori_driver::pipeline::find_stdlib_root() {
        scan_stdlib_dir(&root, &root, &mut catalog);
    }

    catalog
}

impl StdlibCatalog {
    fn insert(&mut self, entry: StdlibEntry) {
        let key = entry.qualified.clone();
        self.modules.insert(entry.module.clone());
        self.by_module
            .entry(entry.module.clone())
            .or_default()
            .push(key.clone());
        self.by_qualified.insert(key, entry);
    }

    fn insert_alias(&mut self, alias: &str, canonical: &str) {
        if self.by_qualified.contains_key(alias) {
            return;
        }
        if let Some(base) = self.by_qualified.get(canonical).cloned() {
            self.insert(StdlibEntry {
                qualified: alias.to_string(),
                module: base.module,
                name: alias.rsplit('.').next().unwrap_or(alias).to_string(),
                signature: base.signature,
                layer: base.layer,
                source_path: base.source_path,
                name_range: base.name_range,
                documentation: base.documentation,
            });
        }
    }

    pub fn lookup(&self, path: &str) -> Option<&StdlibEntry> {
        self.by_qualified.get(path)
    }

    pub fn modules(&self) -> impl Iterator<Item = &String> {
        self.modules.iter()
    }

    pub fn entries_for_module(&self, module: &str) -> Vec<&StdlibEntry> {
        self.by_module
            .get(module)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.by_qualified.get(k))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn members_for_receiver(
        &self,
        receiver: &str,
        import_map: &HashMap<String, String>,
    ) -> Vec<&StdlibEntry> {
        if let Some(module) = import_map.get(receiver) {
            return self.entries_for_module(module);
        }
        if receiver.starts_with("ori.") {
            return self.entries_for_module(receiver);
        }
        self.entries_for_module(&format!("ori.{receiver}"))
    }

    pub fn completion_items(&self) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = BTreeSet::new();

        for module in &self.modules {
            if seen.insert(module.clone()) {
                items.push(CompletionItem {
                    label: module.clone(),
                    kind: Some(CompletionItemKind::MODULE),
                    detail: Some("Ori stdlib module".into()),
                    ..CompletionItem::default()
                });
            }
        }

        for entry in self.by_qualified.values() {
            if !seen.insert(entry.qualified.clone()) {
                continue;
            }
            let layer = match entry.layer {
                StdlibLayer::Runtime => "Layer 1 runtime",
                StdlibLayer::Orl => "Layer 2 .orl",
            };
            items.push(CompletionItem {
                label: entry.qualified.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("{layer}: {}", entry.signature)),
                documentation: entry.documentation.as_ref().map(|d| {
                    tower_lsp::lsp_types::Documentation::MarkupContent(
                        tower_lsp::lsp_types::MarkupContent {
                            kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                            value: d.clone(),
                        },
                    )
                }),
                ..CompletionItem::default()
            });
        }

        items
    }

    pub fn dot_completion_items(
        &self,
        receiver: &str,
        import_map: &HashMap<String, String>,
    ) -> Vec<CompletionItem> {
        self.members_for_receiver(receiver, import_map)
            .into_iter()
            .map(|entry| CompletionItem {
                label: entry.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(entry.signature.clone()),
                documentation: entry.documentation.as_ref().map(|d| {
                    tower_lsp::lsp_types::Documentation::MarkupContent(
                        tower_lsp::lsp_types::MarkupContent {
                            kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                            value: d.clone(),
                        },
                    )
                }),
                ..CompletionItem::default()
            })
            .collect()
    }

    pub fn hover_markdown(&self, path: &str) -> Option<String> {
        let entry = self.lookup(path)?;
        let layer = match entry.layer {
            StdlibLayer::Runtime => "Layer 1 — native runtime",
            StdlibLayer::Orl => "Layer 2 — `.orl` stdlib",
        };
        let mut md = format!(
            "```ori\n{}\n```\n\n**{}** · `{}`",
            entry.signature, layer, entry.qualified
        );
        if let Some(src) = &entry.source_path {
            md.push_str(&format!("\n\nSource: `{}`", src.display()));
        }
        Some(md)
    }

    /// Resolve a function reference (`print`, `io.print`, `ori.io.print`) to its signature.
    pub fn signature_for_call(
        &self,
        func_ref: &str,
        import_map: &HashMap<String, String>,
    ) -> Option<String> {
        if let Some(entry) = self.lookup(func_ref) {
            return Some(entry.signature.clone());
        }
        if let Some((receiver, method)) = func_ref.rsplit_once('.') {
            let module = import_map
                .get(receiver)
                .cloned()
                .unwrap_or_else(|| format!("ori.{receiver}"));
            let qualified = format!("{module}.{method}");
            if let Some(entry) = self.lookup(&qualified) {
                return Some(entry.signature.clone());
            }
        }
        None
    }
}

fn format_ty_sig(path: &str) -> Option<String> {
    let (params, ret) = ori_types::stdlib::stdlib_func_sig(path)?;
    let params_str = if params.is_empty() {
        String::new()
    } else {
        params
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                let name = match path {
                    "ori.io.print" | "ori.io.println" | "ori.io.eprint" | "ori.io.eprintln" => "s",
                    "ori.process.exit" => "code",
                    _ => "arg",
                };
                if name == "arg" {
                    format!("arg{i}: {}", ty.display())
                } else {
                    format!("{name}: {}", ty.display())
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let name = path.rsplit('.').next().unwrap_or(path);
    Some(format!("{name}({params_str}) -> {}", ret.display()))
}

fn stdlib_documentation(path: &str) -> Option<String> {
    let desc = match path {
        // io
        "ori.io.print" => "Escreve o valor especificado na saída padrão (stdout).",
        "ori.io.println" => "Escreve o valor especificado na saída padrão (stdout), seguido por uma quebra de linha.",
        "ori.io.eprint" => "Escreve o valor especificado na saída de erro padrão (stderr).",
        "ori.io.eprintln" => "Escreve o valor especificado na saída de erro padrão (stderr), seguido por uma quebra de linha.",
        "ori.io.read_line" => "Lê uma linha de texto a partir da entrada padrão (stdin).\n\nRetorna `some(string)` se for bem-sucedido ou `none` se atingir o fim do arquivo (EOF).",

        // fs
        "ori.fs.exists" => "Verifica se o caminho especificado existe no sistema de arquivos.\n\nRetorna `result[bool, string]`.",
        "ori.fs.is_file" => "Verifica se o caminho especificado aponta para um arquivo.\n\nRetorna `result[bool, string]`.",
        "ori.fs.is_dir" => "Verifica se o caminho especificado aponta para um diretório.\n\nRetorna `result[bool, string]`.",
        "ori.fs.read_text" => "Lê todo o conteúdo de um arquivo de texto e o retorna como string.\n\nRetorna `result[string, string]`.",
        "ori.fs.write_text" => "Escreve o texto fornecido em um arquivo no caminho especificado.\n\nRetorna `result[void, string]`.",
        "ori.fs.append_text" => "Adiciona o texto fornecido ao final do arquivo especificado.\n\nRetorna `result[void, string]`.",
        "ori.fs.delete" => "Remove um arquivo do sistema de arquivos.\n\nRetorna `result[void, string]`.",
        "ori.fs.create_dir" => "Cria um novo diretório no caminho especificado.\n\nRetorna `result[void, string]`.",
        "ori.fs.create_dir_all" => "Cria um diretório e todos os diretórios pais necessários no caminho especificado.\n\nRetorna `result[void, string]`.",

        // process
        "ori.process.exit" => "Encerra a execução do programa atual imediatamente com o código de saída especificado.",
        "ori.process.args" => "Retorna a lista de argumentos de linha de comando passados para o programa.",

        // time
        "ori.time.sleep" => "Bloqueia a execução do fluxo de controle atual pelo número especificado de milissegundos.",

        // random
        "ori.random.seed" => "Inicializa o gerador de números pseudo-aleatórios com uma semente numérica.",
        "ori.random.next_int" => "Gera um número inteiro pseudo-aleatório no intervalo especificado.",
        "ori.random.next_float" => "Gera um número de ponto flutuante pseudo-aleatório entre `0.0` e `1.0`.",

        // string utils (Layer 2)
        "ori.string.is_empty" => "Verifica se a string fornecida está vazia (comprimento zero).",
        "ori.string.blank" => "Verifica se a string fornecida está vazia ou contém apenas caracteres de espaço em branco.",
        "ori.string.replicate" => "Cria uma nova string repetindo a string original `n` vezes.",
        "ori.string.default" => "Retorna a string original se não estiver vazia, ou a string de fallback caso esteja.",
        "ori.string.equals_ignore_case" => "Verifica se duas strings são iguais, ignorando a diferença entre maiúsculas e minúsculas.",
        "ori.string.center" => "Centraliza a string dentro de um espaço de largura especificada, preenchendo as laterais com espaços.",
        "ori.string.count" => "Conta o número de ocorrências não sobrepostas de uma substring dentro da string.",
        "ori.string.reverse" => "Retorna uma nova string com a ordem dos caracteres invertida.",
        "ori.string.capitalize" => "Retorna uma cópia da string com a primeira letra maiúscula e as restantes minúsculas.",
        "ori.string.title" => "Retorna a string com a primeira letra de cada palavra em maiúscula.",
        "ori.string.trim_all" => "Remove todos os espaços em branco extras do início, do fim e entre as palavras da string.",
        "ori.string.left" => "Retorna os primeiros `n` caracteres da string.",
        "ori.string.right" => "Retorna os últimos `n` caracteres da string.",
        "ori.string.limit" => "Limita o tamanho da string, cortando-a se exceder `max_len`.",
        "ori.string.lines" => "Divide a string em uma lista de linhas.",
        "ori.string.words" => "Divide a string em uma lista de palavras (separadas por espaços).",
        "ori.string.replace_all" => "Substitui todas as ocorrências de uma substring por outra string de substituição.",

        // list utils
        "ori.list.first_or" => "Retorna o primeiro elemento da lista, ou um valor padrão caso a lista esteja vazia.",
        "ori.list.last_or" => "Retorna o último elemento da lista, ou um valor padrão caso a lista esteja vazia.",
        "ori.list.get_or" => "Retorna o elemento no índice especificado, ou um valor padrão caso o índice esteja fora dos limites.",
        "ori.list.singleton" => "Cria uma nova lista contendo apenas um único elemento.",
        "ori.list.with_capacity" => "Cria uma lista vazia com capacidade mínima pré-alocada (evita realocações no push).",
        "ori.list.capacity" => "Retorna a capacidade atual (slots alocados) da lista.",
        "ori.list.reserve" => "Garante capacidade mínima sem alterar o comprimento da lista.",

        // math utils
        "ori.math.sign" => "Retorna o sinal do número: `1` para positivo, `-1` para negativo e `0` para zero.",
        "ori.math.clamp_int" => "Limita o valor inteiro dentro de um intervalo mínimo e máximo especificado.",
        "ori.math.lerp" => "Realiza a interpolação linear entre dois valores com base em um fator `t`.",
        "ori.math.approx_eq" => "Verifica se dois valores de ponto flutuante são aproximadamente iguais dentro de uma tolerância.",

        // map utils
        "ori.map.get_or" => "Retorna o valor associado à chave no mapa, ou um valor padrão caso a chave não exista.",
        "ori.map.contains_key" => "Verifica se o mapa contém a chave especificada.",

        // validate
        "ori.validate.even" => "Verifica se um número é par.",
        "ori.validate.odd" => "Verifica se um número é ímpar.",
        "ori.validate.in_range" => "Verifica se um valor está dentro do intervalo especificado.",

        _ => return None,
    };
    Some(desc.to_string())
}

fn extract_doc_comments(source: &str, start_offset: usize) -> Option<String> {
    let mut lines = Vec::new();
    let before = &source[..start_offset];
    let mut lines_rev = before.lines().rev();

    // Ignora a linha atual em que a declaração se inicia
    let _ = lines_rev.next();

    let mut in_block = false;
    for line in lines_rev {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if in_block {
                lines.push("".to_string());
                continue;
            }
            break;
        }

        if trimmed.starts_with("--|") || trimmed.ends_with("|--") {
            let clean = trimmed
                .trim_start_matches("--|")
                .trim_end_matches("|--")
                .trim()
                .to_string();
            lines.push(clean);
            in_block = true;
        } else if trimmed.starts_with("--") {
            let clean = trimmed.trim_start_matches("--").trim().to_string();
            lines.push(clean);
        } else {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        lines.reverse();
        Some(lines.join("\n"))
    }
}

fn scan_stdlib_dir(root: &Path, dir: &Path, catalog: &mut StdlibCatalog) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_stdlib_dir(root, &path, catalog);
        } else if path.extension().is_some_and(|e| e == "orl") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                scan_stdlib_orl(&path, &content, catalog);
            }
        }
    }
}

fn scan_stdlib_orl(path: &Path, content: &str, catalog: &mut StdlibCatalog) {
    let file_id = ori_diagnostics::FileId(0);
    let mut sink = ori_diagnostics::DiagnosticSink::default();
    let tokens = ori_lexer::lex(content, file_id, &mut sink);
    let mut source_file = ori_parser::parse(&tokens, content, file_id, &mut sink);
    if let Err(error) = ori_driver::pipeline::filter_intrinsic_source_for_current_configuration(
        &mut source_file,
        file_id,
        &mut sink,
    ) {
        eprintln!(
            "ori-lsp: cannot index stdlib module `{}`: {error}",
            path.display()
        );
        return;
    }
    let namespace = source_file.namespace.name.to_string();

    for item in &source_file.items {
        if let Item::Func(func) = &item.item {
            if !func.visibility.is_public() {
                continue;
            }
            let qualified = format!("{}.{}", namespace, func.name.text);
            let signature = func_signature(func);
            let name_range = name_span_to_range(content, func);
            let doc = stdlib_documentation(&qualified)
                .or_else(|| extract_doc_comments(content, func.span.start as usize));
            catalog.insert(StdlibEntry {
                qualified: qualified.clone(),
                module: namespace.clone(),
                name: func.name.text.to_string(),
                signature,
                layer: StdlibLayer::Orl,
                source_path: Some(path.to_path_buf()),
                name_range: Some(name_range),
                documentation: doc,
            });
        }
    }
}

fn func_signature(func: &FuncDecl) -> String {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.text, type_to_string(&p.ty)))
        .collect();
    let ret = func
        .return_ty
        .as_ref()
        .map(|t| format!(" -> {}", type_to_string(t)))
        .unwrap_or_default();
    let mut prefix = String::new();
    if func.is_async {
        prefix.push_str("async ");
    }
    if func.is_mut {
        prefix.push_str("mut ");
    }
    format!("{}{}({}){}", prefix, func.name.text, params.join(", "), ret)
}

fn type_to_string(ty: &ori_ast::ty::Type) -> String {
    match ty {
        ori_ast::ty::Type::Named(q) => q.to_string(),
        ori_ast::ty::Type::Optional(t, _) => format!("optional[{}]", type_to_string(t)),
        ori_ast::ty::Type::Result(ok, err, _) => {
            format!("result[{}, {}]", type_to_string(ok), type_to_string(err))
        }
        ori_ast::ty::Type::List(t, _) => format!("list[{}]", type_to_string(t)),
        ori_ast::ty::Type::Map(k, v, _) => {
            format!("map[{}, {}]", type_to_string(k), type_to_string(v))
        }
        ori_ast::ty::Type::Set(t, _) => format!("set[{}]", type_to_string(t)),
        ori_ast::ty::Type::Bool(_) => "bool".into(),
        ori_ast::ty::Type::Int(_) => "int".into(),
        ori_ast::ty::Type::Float(_) => "float".into(),
        ori_ast::ty::Type::String(_) => "string".into(),
        ori_ast::ty::Type::Bytes(_) => "bytes".into(),
        ori_ast::ty::Type::Void(_) => "void".into(),
        other => format!("{other:?}"),
    }
}

fn name_span_to_range(source: &str, func: &FuncDecl) -> Range {
    let start = position::position_for_byte_offset(source, func.name.span.start as usize);
    let end = position::position_for_byte_offset(source, func.name.span.end as usize);
    Range::new(start, end)
}

/// Build a map of import alias → stdlib module path from source text.
///
/// Mirrors S3 `import_aliases` / `direct_import_alias`:
/// - `import path = alias` → short key
/// - `import path (items)` → item names/aliases
/// - bare `import path` → full path only (no last-segment short alias)
pub fn import_alias_map(source: &str) -> HashMap<String, String> {
    let file_id = ori_diagnostics::FileId(0);
    let mut sink = ori_diagnostics::DiagnosticSink::default();
    let tokens = ori_lexer::lex(source, file_id, &mut sink);
    let source_file = ori_parser::parse(&tokens, source, file_id, &mut sink);
    let mut map = HashMap::new();
    for import in &source_file.imports {
        let module = import.path.to_string();
        if !import.selected.is_empty() {
            for item in &import.selected {
                let alias = item
                    .alias
                    .as_ref()
                    .map(|n| n.text.to_string())
                    .unwrap_or_else(|| item.name.text.to_string());
                map.insert(alias, format!("{}.{}", module, item.name.text));
            }
        } else if let Some(alias) = import.alias.as_ref() {
            map.insert(alias.text.to_string(), module);
        } else {
            // Bare whole-module import: identity full-path key only.
            map.insert(module.clone(), module);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_includes_layer1_and_layer2() {
        let catalog = stdlib_catalog();
        assert!(catalog.lookup("ori.io.print").is_some());
        assert!(catalog.lookup("ori.string.is_empty").is_some());
    }

    #[test]
    fn import_alias_map_resolves_io() {
        let source = r#"
module app.main
import ori.io = io
main() -> void
end
"#;
        let map = import_alias_map(source);
        assert_eq!(map.get("io"), Some(&"ori.io".to_string()));
    }

    #[test]
    fn import_alias_map_bare_import_has_no_last_segment_alias() {
        let source = r#"
module app.main
import ori.io
main() -> void
end
"#;
        let map = import_alias_map(source);
        assert!(
            map.get("io").is_none(),
            "bare import must not invent short alias `io`: {map:?}"
        );
        assert_eq!(map.get("ori.io"), Some(&"ori.io".to_string()));
    }
}
