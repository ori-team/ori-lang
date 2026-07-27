use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::lsp_types::{Range, Url};

use super::project_semantic::ProjectSemanticIndex;
use super::semantic::SemanticIndex;
use crate::utils::{position, uri};

/// Manages the workspace: open documents, parse cache, and project root.
pub struct ProjectManager {
    /// Currently open documents (buffer in memory).
    open_documents: HashMap<Url, DocumentState>,
    /// Per-document project-wide semantic index, produced by `run_check`.
    ///
    /// This is the Etapa 6.1 cross-file index: it captures the driver's
    /// `ResolvedModule` + `SourceCache` so that hover / go-to-definition /
    /// completion / find-references can resolve symbols across imports.
    semantic_indices: HashMap<Url, Arc<ProjectSemanticIndex>>,
    /// Discovered workspace root.
    workspace_root: Option<PathBuf>,
}

struct DocumentState {
    uri: Url,
    content: String,
    version: i32,
    /// Semantic index built from the parsed AST.
    index: Option<SemanticIndex>,
}

impl ProjectManager {
    pub fn new() -> Self {
        Self {
            open_documents: HashMap::new(),
            semantic_indices: HashMap::new(),
            workspace_root: None,
        }
    }

    /// Set the workspace root (discovered during initialization).
    pub fn set_workspace_root(&mut self, root: Option<PathBuf>) {
        self.workspace_root = root;
    }

    /// Get the workspace root, if known.
    #[allow(dead_code)]
    pub fn workspace_root(&self) -> Option<&PathBuf> {
        self.workspace_root.as_ref()
    }

    /// Register or update a document in memory.
    pub fn upsert_document(&mut self, uri: Url, content: String, version: i32) {
        let index = Some(SemanticIndex::build(&content));
        self.open_documents.insert(
            uri.clone(),
            DocumentState {
                uri,
                content,
                version,
                index,
            },
        );
    }

    /// Apply an incremental LSP text edit to an open document.
    pub fn apply_change(&mut self, uri: &Url, range: Range, text: &str, version: i32) {
        let Some(state) = self.open_documents.get_mut(uri) else {
            return;
        };
        let start = position::byte_offset_for_position(&state.content, range.start);
        let end = position::byte_offset_for_position(&state.content, range.end);
        if start <= end && end <= state.content.len() {
            state.content.replace_range(start..end, text);
            state.version = version;
            state.index = Some(SemanticIndex::build(&state.content));
        }
    }

    /// Store the project-wide semantic index produced for `uri` by
    /// `run_check_source`. Replaces any previous snapshot.
    pub fn upsert_semantic_index(&mut self, uri: Url, index: ProjectSemanticIndex) {
        self.semantic_indices.insert(uri, Arc::new(index));
    }

    /// Get the project-wide semantic index for `uri`, if one has been
    /// produced since the last edit.
    pub fn semantic_index(&self, uri: &Url) -> Option<Arc<ProjectSemanticIndex>> {
        self.semantic_indices.get(uri).cloned()
    }

    /// Get the content of a document (from buffer or disk).
    pub fn document_content(&self, uri: &Url) -> Option<String> {
        if let Some(state) = self.open_documents.get(uri) {
            return Some(state.content.clone());
        }
        let path = uri::document_path_from_uri(uri)?;
        std::fs::read_to_string(path).ok()
    }

    /// Get the semantic index for a document, building it if needed.
    pub fn document_index(&self, uri: &Url) -> Option<&SemanticIndex> {
        self.open_documents.get(uri).and_then(|s| s.index.as_ref())
    }

    /// Remove a document (when closed).
    pub fn remove_document(&mut self, uri: &Url) {
        self.open_documents.remove(uri);
        self.semantic_indices.remove(uri);
    }

    /// Check if a document is open.
    #[allow(dead_code)]
    pub fn is_open(&self, uri: &Url) -> bool {
        self.open_documents.contains_key(uri)
    }

    /// Return all currently open documents with (uri, content) pairs.
    pub fn all_open_documents(&self) -> Vec<(Url, String)> {
        self.open_documents
            .values()
            .map(|s| (s.uri.clone(), s.content.clone()))
            .collect()
    }
}
