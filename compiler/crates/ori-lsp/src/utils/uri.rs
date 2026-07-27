use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

/// Convert an LSP URI to a local file path.
pub fn document_path_from_uri(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

/// Canonicalize a path, falling back to the original on error.
pub fn canonical_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Extract the word (identifier) at a given position in source text.
/// Returns None if no identifier is found at the position.
pub fn word_at_position(source: &str, position: tower_lsp::lsp_types::Position) -> Option<String> {
    let offset = super::position::byte_offset_for_position(source, position);
    if offset >= source.len() {
        return None;
    }

    let bytes = source.as_bytes();
    if !is_ident_byte(bytes[offset]) {
        return None;
    }

    let mut start = offset;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = offset + 1;
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }

    Some(source[start..end].to_string())
}

/// Extract a qualified identifier (`io.print`, `ori.string.utils.is_empty`) at `position`.
pub fn qualified_ident_at_position(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<String> {
    let offset = super::position::byte_offset_for_position(source, position);
    if offset > source.len() {
        return None;
    }
    let bytes = source.as_bytes();

    let mut end = offset;
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    if end == offset && end > 0 && bytes[end - 1] == b'.' {
        end -= 1;
    }
    if end == 0 {
        return None;
    }

    let mut start = end;
    loop {
        while start > 0 && is_ident_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start > 0 && bytes[start - 1] == b'.' {
            start -= 1;
            continue;
        }
        break;
    }

    if start >= end {
        return None;
    }
    let ident = &source[start..end];
    if ident.contains('.') || word_at_position(source, position).is_some() {
        Some(ident.to_string())
    } else {
        None
    }
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Find the matching closing paren for an open paren at `open` position.
#[allow(dead_code)]
pub fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0u32;
    for (i, &byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a string by `sep` without splitting inside nested parens/brackets.
#[allow(dead_code)]
pub fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut depth_paren = 0u32;
    let mut depth_angle = 0u32;
    let mut start = 0usize;
    let mut result = Vec::new();

    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'(' | b'[' | b'<' => depth_paren += 1,
            b')' | b']' | b'>' => {
                if byte == b'>' {
                    depth_angle = depth_angle.saturating_sub(1);
                } else {
                    depth_paren = depth_paren.saturating_sub(1);
                }
            }
            b if b == sep as u8 && depth_paren == 0 && depth_angle == 0 => {
                result.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&s[start..]);
    result
}

/// Take an identifier from the start of `s`. Returns (ident, rest).
#[allow(dead_code)]
pub fn take_identifier(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    let end = s
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    if end == 0 {
        return None;
    }
    Some((&s[..end], &s[end..]))
}
