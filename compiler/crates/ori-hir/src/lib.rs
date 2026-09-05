//! High-level intermediate representation and lowering passes.

pub mod hir;
pub mod lower;
pub mod monomorph;
pub mod optimize;
pub mod verify;

pub use hir::*;
pub use lower::{insert_default_arguments, lower};
pub use monomorph::monomorphize_generics;
pub use optimize::{optimize_module, OptLevel};
pub use verify::verify_module;
