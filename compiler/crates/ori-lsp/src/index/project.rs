use std::collections::HashMap;
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
        }
    }

    /// Register or update a document in memory.
    pub fn upsert_document(&mut self, uri: Url, content: String, version: i32) {
        let path = uri.to_file_path().ok();
        let index = Some(SemanticIndex::build(&content, path.as_deref()));
        // A full-text update invalidates the project-wide resolution snapshot.
        // Keeping it would let navigation answer from the previous source
        // while the debounced validator is still running.
        self.semantic_indices.remove(&uri);
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
    pub fn apply_change(
        &mut self,
        uri: &Url,
        range: Range,
        text: &str,
        version: i32,
    ) -> Result<(), position::PositionError> {
        let Some(state) = self.open_documents.get_mut(uri) else {
            return Ok(());
        };
        let start = position::byte_offset_for_position(&state.content, range.start)?;
        let end = position::byte_offset_for_position(&state.content, range.end)?;
        if start > end {
            return Err(position::PositionError::ReversedRange);
        }
        state.content.replace_range(start..end, text);
        state.version = version;
        let path = state.uri.to_file_path().ok();
        state.index = Some(SemanticIndex::build(&state.content, path.as_deref()));
        // Incremental edits invalidate cross-file resolution immediately;
        // only a validation produced for this exact version may publish a new
        // project index.
        self.semantic_indices.remove(uri);
        Ok(())
    }

    /// Store the project-wide semantic index produced for `uri` by
    /// `run_check_source`. Replaces any previous snapshot.
    pub fn upsert_semantic_index(&mut self, uri: Url, index: ProjectSemanticIndex) {
        self.semantic_indices.insert(uri, Arc::new(index));
    }

    /// Publish a validation result only if the document still has the version
    /// captured before the blocking compiler pass. The version check and
    /// replacement happen under one write lock, closing the race between a
    /// read-only freshness check and a later mutation.
    pub fn upsert_semantic_index_if_current(
        &mut self,
        uri: Url,
        expected_version: Option<i32>,
        index: ProjectSemanticIndex,
    ) -> bool {
        let current_version = self.document_version(&uri);
        if current_version != expected_version {
            return false;
        }
        self.semantic_indices.insert(uri, Arc::new(index));
        true
    }

    /// Get the project-wide semantic index for `uri`, if one has been
    /// produced since the last edit.
    pub fn semantic_index(&self, uri: &Url) -> Option<Arc<ProjectSemanticIndex>> {
        self.semantic_indices.get(uri).cloned()
    }

    /// Get the version of an open document, if it is buffered.
    pub fn document_version(&self, uri: &Url) -> Option<i32> {
        self.open_documents.get(uri).map(|s| s.version)
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

    /// Return all currently open documents with (uri, content) pairs.
    pub fn all_open_documents(&self) -> Vec<(Url, String)> {
        self.open_documents
            .values()
            .map(|s| (s.uri.clone(), s.content.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tower_lsp::lsp_types::Position;

    #[test]
    fn incremental_edit_after_emoji_uses_utf16_columns() {
        position::PositionEncoding::negotiate(None);
        let uri = Url::parse("file:///unicode.orl").expect("valid fixture URI");
        let mut project = ProjectManager::new();
        project.upsert_document(uri.clone(), "🙂value".into(), 1);

        project
            .apply_change(
                &uri,
                Range::new(Position::new(0, 2), Position::new(0, 7)),
                "renamed",
                2,
            )
            .expect("valid UTF-16 edit");

        assert_eq!(project.document_content(&uri).as_deref(), Some("🙂renamed"));
    }

    #[test]
    fn incremental_edit_rejects_middle_of_surrogate_pair() {
        position::PositionEncoding::negotiate(None);
        let uri = Url::parse("file:///unicode-invalid.orl").expect("valid fixture URI");
        let mut project = ProjectManager::new();
        project.upsert_document(uri.clone(), "🙂value".into(), 1);

        let result = project.apply_change(
            &uri,
            Range::new(Position::new(0, 1), Position::new(0, 2)),
            "x",
            2,
        );

        assert_eq!(
            result,
            Err(position::PositionError::CharacterInsideCodePoint)
        );
        assert_eq!(project.document_content(&uri).as_deref(), Some("🙂value"));
    }

    #[test]
    fn incremental_edit_rejects_reversed_range() {
        position::PositionEncoding::negotiate(None);
        let uri = Url::parse("file:///reversed.orl").expect("valid fixture URI");
        let mut project = ProjectManager::new();
        project.upsert_document(uri.clone(), "value".into(), 1);

        let result = project.apply_change(
            &uri,
            Range::new(Position::new(0, 4), Position::new(0, 1)),
            "x",
            2,
        );

        assert_eq!(result, Err(position::PositionError::ReversedRange));
        assert_eq!(project.document_content(&uri).as_deref(), Some("value"));
    }

    #[test]
    fn document_version_tracks_upsert_and_apply() {
        let uri = Url::parse("file:///version.orl").expect("valid fixture URI");
        let mut project = ProjectManager::new();
        assert_eq!(project.document_version(&uri), None);
        project.upsert_document(uri.clone(), "v1".into(), 1);
        assert_eq!(project.document_version(&uri), Some(1));
        project
            .apply_change(
                &uri,
                Range::new(Position::new(0, 0), Position::new(0, 0)),
                "x",
                2,
            )
            .expect("valid edit");
        assert_eq!(project.document_version(&uri), Some(2));
        assert_eq!(project.document_content(&uri).as_deref(), Some("xv1"));
    }

    #[test]
    fn document_updates_invalidate_project_semantic_snapshot() {
        let uri = Url::parse("file:///invalidate.orl").expect("valid fixture URI");
        let mut project = ProjectManager::new();
        let source = "module app.invalidate\n\npublic answer() -> int\n    return 1\nend\n";
        project.upsert_document(uri.clone(), source.into(), 1);
        let path = PathBuf::from("invalidate.orl");
        let output = ori_driver::pipeline::run_check_source(&path, source.into())
            .expect("valid semantic fixture");
        let index = ProjectSemanticIndex::new(output.resolved, output.cache, path);
        assert!(project.upsert_semantic_index_if_current(uri.clone(), Some(1), index));
        assert!(project.semantic_index(&uri).is_some());

        project
            .apply_change(
                &uri,
                Range::new(Position::new(0, 0), Position::new(0, 0)),
                "// edit\n",
                2,
            )
            .expect("valid edit");
        assert!(project.semantic_index(&uri).is_none());
    }

    #[test]
    fn semantic_snapshot_cannot_publish_for_an_old_version() {
        let uri = Url::parse("file:///stale.orl").expect("valid fixture URI");
        let mut project = ProjectManager::new();
        let source = "module app.stale\n\npublic answer() -> int\n    return 1\nend\n";
        project.upsert_document(uri.clone(), source.into(), 2);
        let path = PathBuf::from("stale.orl");
        let output = ori_driver::pipeline::run_check_source(&path, source.into())
            .expect("valid semantic fixture");
        let index = ProjectSemanticIndex::new(output.resolved, output.cache, path);
        assert!(!project.upsert_semantic_index_if_current(uri.clone(), Some(1), index));
        assert!(project.semantic_index(&uri).is_none());
    }

    #[tokio::test]
    async fn generation_safe_stale_validation_is_discarded() {
        // Simulates AUD-LSP-3: slow-first validation must not overwrite fresh state.
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let uri = Url::parse("file:///gen.orl").expect("valid fixture URI");
        let project = Arc::new(RwLock::new(ProjectManager::new()));
        project
            .write()
            .await
            .upsert_document(uri.clone(), "version 1".into(), 1);

        // Snapshot for slow validation (version 1).
        let slow_snapshot_version = {
            let p = project.read().await;
            p.document_version(&uri)
        };
        let slow_content = {
            let p = project.read().await;
            p.document_content(&uri).unwrap()
        };

        // Fast edit advances to version 2 before slow completes.
        project
            .write()
            .await
            .upsert_document(uri.clone(), "version 2".into(), 2);

        // Simulate slow blocking work.
        let blocking = tokio::task::spawn_blocking(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            slow_content
        })
        .await
        .unwrap();

        // Generation check that Backend now performs before commit.
        let still_current = {
            let p = project.read().await;
            p.document_version(&uri) == slow_snapshot_version
        };
        assert!(
            !still_current,
            "stale snapshot version 1 must be discarded after version 2"
        );
        assert_eq!(blocking, "version 1");

        // Fresh snapshot must be current.
        let fresh_version = {
            let p = project.read().await;
            p.document_version(&uri)
        };
        assert_eq!(fresh_version, Some(2));
        assert_eq!(
            project.read().await.document_content(&uri).as_deref(),
            Some("version 2")
        );
    }
}
