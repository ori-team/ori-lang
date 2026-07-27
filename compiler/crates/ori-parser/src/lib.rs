// ori-parser real parser lib.rs

mod parse_expr;
mod parse_item;
mod parse_pat;
mod parse_stmt;
mod parse_ty;
mod parser;

use ori_ast::item::SourceFile;
use ori_diagnostics::{DiagnosticSink, FileId};
use ori_lexer::Token;

/// Lex the `source` string and parse it into a `SourceFile` AST.
///
/// All diagnostics (parse errors) are collected in `sink`.
/// The returned `SourceFile` may be partial if errors occurred.
pub fn parse(
    tokens: &[Token],
    source: &str,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> SourceFile {
    let mut p = parser::Parser::new(tokens, source, file_id, sink);
    p.parse_source_file()
}
