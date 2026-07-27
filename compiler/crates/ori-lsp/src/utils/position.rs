use tower_lsp::lsp_types::{Position, Range};

/// Convert a byte offset (0-based) to an LSP Position (line, character).
pub fn position_for_byte_offset(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, byte) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Position::new(line, col)
}

/// Convert an LSP Position to a byte offset into the source text.
pub fn byte_offset_for_position(source: &str, position: Position) -> usize {
    let mut offset = 0usize;
    let mut current_line = 0u32;

    for (i, byte) in source.bytes().enumerate() {
        if current_line >= position.line {
            break;
        }
        if byte == b'\n' {
            current_line += 1;
        }
        offset = i + 1;
    }

    // Advance within the target line to the character position
    let chars_in_line = source[offset..].bytes().take_while(|&b| b != b'\n').count();
    offset += position.character.min(chars_in_line as u32) as usize;

    offset
}

/// Build a Range from line/col indices (0-based).
#[allow(dead_code)]
pub fn range_for_line_and_columns(
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
) -> Range {
    Range::new(
        Position::new(start_line as u32, start_col as u32),
        Position::new(end_line as u32, end_col as u32),
    )
}

/// Create a Range covering the entire document.
#[allow(dead_code)]
pub fn full_document_range(source: &str) -> Range {
    let lines = source.lines().count();
    let last_len = source.lines().last().map(|l| l.len()).unwrap_or(0);
    Range::new(
        Position::new(0, 0),
        Position::new(lines.saturating_sub(1) as u32, last_len as u32),
    )
}

/// Default range (0:0 to 0:1).
pub fn default_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_roundtrip() {
        let source = "module app.main\nmain()\nend\n";
        let pos = Position::new(1, 5);
        let offset = byte_offset_for_position(source, pos);
        let result = position_for_byte_offset(source, offset);
        assert_eq!(result.line, 1);
        assert_eq!(result.character, 5);
    }
}
