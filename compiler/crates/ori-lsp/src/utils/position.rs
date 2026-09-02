use std::sync::atomic::{AtomicU8, Ordering};

use tower_lsp::lsp_types::{Position, PositionEncodingKind, Range};

const UTF8: u8 = 0;
const UTF16: u8 = 1;
const UTF32: u8 = 2;

// One LSP process serves one client session. Keeping the negotiated encoding
// here makes every inbound and outbound position use the same codec, including
// diagnostics and indexes built outside `Backend`.
static NEGOTIATED_ENCODING: AtomicU8 = AtomicU8::new(UTF16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionEncoding {
    Utf8,
    Utf16,
    Utf32,
}

impl PositionEncoding {
    pub fn negotiate(client_encodings: Option<&[PositionEncodingKind]>) -> Self {
        let encoding = client_encodings
            .and_then(|encodings| {
                encodings.iter().find_map(|encoding| {
                    if *encoding == PositionEncodingKind::UTF8 {
                        Some(Self::Utf8)
                    } else if *encoding == PositionEncodingKind::UTF16 {
                        Some(Self::Utf16)
                    } else if *encoding == PositionEncodingKind::UTF32 {
                        Some(Self::Utf32)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(Self::Utf16);
        NEGOTIATED_ENCODING.store(encoding.tag(), Ordering::Relaxed);
        encoding
    }

    pub fn as_lsp_kind(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
            Self::Utf32 => PositionEncodingKind::UTF32,
        }
    }

    fn current() -> Self {
        match NEGOTIATED_ENCODING.load(Ordering::Relaxed) {
            UTF8 => Self::Utf8,
            UTF32 => Self::Utf32,
            _ => Self::Utf16,
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Utf8 => UTF8,
            Self::Utf16 => UTF16,
            Self::Utf32 => UTF32,
        }
    }

    fn units(self, text: &str) -> usize {
        match self {
            Self::Utf8 => text.len(),
            Self::Utf16 => text.encode_utf16().count(),
            Self::Utf32 => text.chars().count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionError {
    LineOutOfBounds,
    CharacterOutOfBounds,
    CharacterInsideCodePoint,
    ReversedRange,
}

/// Convert a byte offset to a position using the encoding negotiated at
/// initialization. Invalid compiler offsets are moved to the preceding UTF-8
/// boundary so this function never slices inside a code point.
pub fn position_for_byte_offset(source: &str, offset: usize) -> Position {
    position_for_byte_offset_with_encoding(source, offset, PositionEncoding::current())
}

fn position_for_byte_offset_with_encoding(
    source: &str,
    offset: usize,
    encoding: PositionEncoding,
) -> Position {
    let mut safe_offset = offset.min(source.len());
    while !source.is_char_boundary(safe_offset) {
        safe_offset -= 1;
    }

    let before = &source[..safe_offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let character = encoding.units(&source[line_start..safe_offset]) as u32;
    Position::new(line, character)
}

/// Convert an inbound LSP position to a byte offset. Positions beyond a line,
/// or in the middle of a UTF-8/UTF-16 code point, are rejected instead of
/// being clamped into a potentially invalid `String` boundary.
pub fn byte_offset_for_position(source: &str, position: Position) -> Result<usize, PositionError> {
    byte_offset_for_position_with_encoding(source, position, PositionEncoding::current())
}

fn byte_offset_for_position_with_encoding(
    source: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Result<usize, PositionError> {
    let (line_start, line_end) = line_bounds(source, position.line)?;
    let line = &source[line_start..line_end];
    let requested = position.character as usize;

    if requested == 0 {
        return Ok(line_start);
    }

    let mut units = 0usize;
    for (relative_offset, character) in line.char_indices() {
        if units == requested {
            return Ok(line_start + relative_offset);
        }
        let next_units = units
            + match encoding {
                PositionEncoding::Utf8 => character.len_utf8(),
                PositionEncoding::Utf16 => character.len_utf16(),
                PositionEncoding::Utf32 => 1,
            };
        if requested < next_units {
            return Err(PositionError::CharacterInsideCodePoint);
        }
        units = next_units;
    }

    if units == requested {
        Ok(line_end)
    } else {
        Err(PositionError::CharacterOutOfBounds)
    }
}

fn line_bounds(source: &str, requested_line: u32) -> Result<(usize, usize), PositionError> {
    let mut line = 0u32;
    let mut start = 0usize;
    for (offset, byte) in source.bytes().enumerate() {
        if line == requested_line && byte == b'\n' {
            let end = if offset > start && source.as_bytes()[offset - 1] == b'\r' {
                offset - 1
            } else {
                offset
            };
            return Ok((start, end));
        }
        if byte == b'\n' {
            line += 1;
            start = offset + 1;
        }
    }
    if line == requested_line {
        Ok((start, source.len()))
    } else {
        Err(PositionError::LineOutOfBounds)
    }
}

pub fn default_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_roundtrip_handles_accents_cjk_combining_and_emoji() {
        let source = "aé界e\u{301}🙂 value\n";
        let offset = source.find("value").expect("fixture contains value");
        let position =
            position_for_byte_offset_with_encoding(source, offset, PositionEncoding::Utf16);

        assert_eq!(position, Position::new(0, 8));
        assert_eq!(
            byte_offset_for_position_with_encoding(source, position, PositionEncoding::Utf16),
            Ok(offset)
        );
    }

    #[test]
    fn rejects_position_inside_utf16_surrogate_pair() {
        let source = "🙂x";
        assert_eq!(
            byte_offset_for_position_with_encoding(
                source,
                Position::new(0, 1),
                PositionEncoding::Utf16,
            ),
            Err(PositionError::CharacterInsideCodePoint)
        );
    }

    #[test]
    fn supports_all_negotiable_encodings() {
        let source = "é🙂x";
        let x = source.find('x').expect("fixture contains x");
        for (encoding, column) in [
            (PositionEncoding::Utf8, 6),
            (PositionEncoding::Utf16, 3),
            (PositionEncoding::Utf32, 2),
        ] {
            let position = position_for_byte_offset_with_encoding(source, x, encoding);
            assert_eq!(position.character, column);
            assert_eq!(
                byte_offset_for_position_with_encoding(source, position, encoding),
                Ok(x)
            );
        }
    }

    #[test]
    fn rejects_out_of_bounds_line_and_column() {
        let source = "abc\n";
        assert_eq!(
            byte_offset_for_position_with_encoding(
                source,
                Position::new(2, 0),
                PositionEncoding::Utf16,
            ),
            Err(PositionError::LineOutOfBounds)
        );
        assert_eq!(
            byte_offset_for_position_with_encoding(
                source,
                Position::new(0, 4),
                PositionEncoding::Utf16,
            ),
            Err(PositionError::CharacterOutOfBounds)
        );
    }

    #[test]
    fn excludes_crlf_terminator_from_line_columns() {
        let source = "abc\r\nnext";
        assert_eq!(
            byte_offset_for_position_with_encoding(
                source,
                Position::new(0, 3),
                PositionEncoding::Utf16,
            ),
            Ok(3)
        );
        assert!(byte_offset_for_position_with_encoding(
            source,
            Position::new(0, 4),
            PositionEncoding::Utf16,
        )
        .is_err());
    }
}
