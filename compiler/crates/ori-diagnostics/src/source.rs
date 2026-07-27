use crate::Span;
use std::path::{Path, PathBuf};

/// A unique identifier for a source file within a compilation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// A single source file: its path and raw UTF-8 content.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub content: String,
    /// Byte offsets of the start of each line, for span → line/col conversion.
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(id: FileId, path: impl AsRef<Path>, content: String) -> Self {
        let line_starts = std::iter::once(0u32)
            .chain(
                content
                    .char_indices()
                    .filter(|&(_, c)| c == '\n')
                    .map(|(i, _)| (i + 1) as u32),
            )
            .collect();
        Self {
            id,
            path: path.as_ref().to_owned(),
            content,
            line_starts,
        }
    }

    /// Returns the source text covered by `span`.
    pub fn slice(&self, span: Span) -> &str {
        &self.content[span.as_range()]
    }

    /// Converts a byte offset to (line, character column), both 1-indexed.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.content.len() as u32);
        let line = self
            .line_starts
            .partition_point(|&s| s <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line] as usize;
        let offset = previous_char_boundary(&self.content, offset as usize);
        let col = self.content[line_start..offset].chars().count() as u32;
        (line as u32 + 1, col + 1)
    }

    /// Returns the source text of a 1-indexed line (without trailing newline).
    pub fn line_text(&self, line: u32) -> &str {
        let idx = (line as usize).saturating_sub(1);
        let start = self.line_starts.get(idx).copied().unwrap_or(0) as usize;
        let end = self
            .line_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(self.content.len() as u32) as usize;
        self.content[start..end]
            .trim_end_matches('\n')
            .trim_end_matches('\r')
    }
}

fn previous_char_boundary(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Holds all source files for a compilation session.
#[derive(Debug, Default)]
pub struct SourceCache {
    files: Vec<SourceFile>,
}

impl SourceCache {
    pub fn add(&mut self, path: impl AsRef<Path>, content: String) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(id, path, content));
        id
    }

    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    /// Iterate over all source files registered in this cache.
    ///
    /// Used by the LSP `ProjectSemanticIndex` to scan every loaded file
    /// (entry + transitive imports) for cross-file navigation queries
    /// such as find-references.
    pub fn all_files(&self) -> &[SourceFile] {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_reports_character_columns_for_unicode() {
        let file = SourceFile::new(FileId(0), "main.orl", "\"\u{00e1}\u{00e9}\" value\n".into());
        let offset = file.content.find("value").unwrap() as u32;

        assert_eq!(file.line_col(offset), (1, 6));
    }

    #[test]
    fn line_col_handles_emoji_and_crlf() {
        let file = SourceFile::new(
            FileId(0),
            "main.orl",
            "aa\u{1f642}bb\r\nconst value\n".into(),
        );
        let offset = file.content.find("value").unwrap() as u32;

        assert_eq!(file.line_col("aa\u{1f642}".len() as u32), (1, 4));
        assert_eq!(file.line_col(offset), (2, 7));
    }
}
