mod lexer;
mod token;

pub use lexer::{lex, Token};
pub use ori_diagnostics::Span;
pub use token::TokenKind;
