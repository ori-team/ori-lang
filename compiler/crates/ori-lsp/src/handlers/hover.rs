use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::stdlib_catalog::stdlib_catalog;

/// Hover text for built-in Ori types.
pub fn builtin_type_hover(symbol: &str) -> Option<String> {
    let text = match symbol {
        "int" => "`int`\n\nSigned integer value used for whole numbers.",
        "float" => "`float`\n\nFloating point value used for decimal numbers.",
        "int8" | "int16" | "int32" | "int64" =>
            &format!("`{symbol}`\n\nSigned integer with explicit bit width."),
        "u8" | "u16" | "u32" | "u64" =>
            &format!("`{symbol}`\n\nUnsigned integer with explicit bit width."),
        "float32" | "float64" =>
            &format!("`{symbol}`\n\nFloating point with explicit bit width."),
        "bool" => "`bool`\n\nBoolean value. It is either `true` or `false`.",
        "string" => "`string`\n\nUTF-8 text value managed by the Ori runtime.",
        "bytes" => "`bytes`\n\nByte buffer used for binary data.",
        "void" => "`void`\n\nFunction return type for functions that do not return a value.",
        "list" => "`list[T]`\n\nOrdered runtime collection of values with the same element type.",
        "map" => "`map[K, V]`\n\nHash map. Keys must be `int`, `string`, or implement `Hashable` and `Equatable`.",
        "set" => "`set[T]`\n\nHash set. Elements must be `int`, `string`, or implement `Hashable` and `Equatable`.",
        "optional" => "`optional[T]`\n\nRepresents either a value of type `T` or `none`.",
        "result" => "`result[T, E]`\n\nRepresents either success `ok(T)` or failure `err(E)`.",
        "future" => "`future[T]`\n\nAsynchronous result that will produce a value of type `T`.",
        _ => return None,
    };
    Some(text.to_string())
}

/// Create a Hover response from markdown content.
pub fn markdown_hover(content: String) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: None,
    }
}

/// Rich hover for stdlib symbols (Layer 1 runtime + Layer 2 `.orl`).
pub fn stdlib_hover(path: &str) -> Option<String> {
    stdlib_catalog().hover_markdown(path)
}
