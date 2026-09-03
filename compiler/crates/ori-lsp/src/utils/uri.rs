use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

pub fn document_path_from_uri(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

pub fn canonical_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Extract a Unicode identifier at an LSP position.
pub fn word_at_position(source: &str, position: tower_lsp::lsp_types::Position) -> Option<String> {
    let offset = super::position::byte_offset_for_position(source, position).ok()?;
    let (start, end) = identifier_range_at_offset(source, offset)?;
    Some(source[start..end].to_string())
}

/// Extract a qualified identifier (`io.print`, `ori.string.utils.is_empty`).
pub fn qualified_ident_at_position(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<String> {
    let offset = super::position::byte_offset_for_position(source, position).ok()?;
    let (mut start, mut end) = identifier_range_at_offset(source, offset)?;

    while let Some(dot) = previous_char(source, start) {
        if dot.1 != '.' {
            break;
        }
        let Some((identifier_start, _)) = identifier_range_ending_at(source, dot.0) else {
            break;
        };
        start = identifier_start;
    }

    while let Some('.') = source[end..].chars().next() {
        let after_dot = end + '.'.len_utf8();
        let Some((_, identifier_end)) = identifier_range_at_offset(source, after_dot) else {
            break;
        };
        end = identifier_end;
    }

    Some(source[start..end].to_string())
}

fn identifier_range_at_offset(source: &str, offset: usize) -> Option<(usize, usize)> {
    if offset >= source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let current = source[offset..].chars().next()?;
    if !is_ident_char(current) {
        return None;
    }

    let mut start = offset;
    while let Some((previous_offset, character)) = previous_char(source, start) {
        if !is_ident_char(character) {
            break;
        }
        start = previous_offset;
    }

    let mut end = offset + current.len_utf8();
    while let Some(character) = source[end..].chars().next() {
        if !is_ident_char(character) {
            break;
        }
        end += character.len_utf8();
    }
    Some((start, end))
}

fn identifier_range_ending_at(source: &str, end: usize) -> Option<(usize, usize)> {
    let (offset, character) = previous_char(source, end)?;
    if !is_ident_char(character) {
        return None;
    }
    identifier_range_at_offset(source, offset)
}

fn previous_char(source: &str, offset: usize) -> Option<(usize, char)> {
    source[..offset].char_indices().next_back()
}

fn is_ident_char(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Position;

    #[test]
    fn finds_unicode_word_after_emoji_with_utf16_position() {
        super::super::position::PositionEncoding::negotiate(None);
        let source = "🙂 café.東京";
        assert_eq!(
            word_at_position(source, Position::new(0, 4)).as_deref(),
            Some("café")
        );
        assert_eq!(
            qualified_ident_at_position(source, Position::new(0, 9)).as_deref(),
            Some("café.東京")
        );
    }
}
