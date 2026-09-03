// ori-types

pub mod check;
pub mod conditional;
pub mod const_eval;
pub mod def;
pub mod literal;
pub mod lower;
pub mod resolve;
pub mod stdlib;
pub mod ty;

pub use check::Checker;
pub use def::{
    CompileTimeValue, ConstEvalFailure, ConstEvalFailureKind, ConstEvaluation, Def, DefId, DefKind,
    DefMap,
};
pub use lower::{
    is_inline_ty, is_inline_ty_with_structs, is_inline_ty_with_structs_visiting, lower_type,
    lower_type_with_aliases, lower_type_with_local_aliases,
    lower_type_with_local_aliases_and_structs, non_inline_reason_with_structs,
    non_inline_reason_with_structs_visiting,
};
pub use resolve::{
    resolve, EnumSig, EnumVariantSig, FuncSig, ImplMethodSig, ImplSig, ReExport, ResolvedModule,
    StructSig, TraitMethodSig, TraitSig, TypeAliasSig, ValueSig, WhereConstraintSig,
};
pub use ty::{
    expand_ty_aliases, normalize_ty_aliases, replace_json_placeholder, substitute_trait_self,
    substitute_ty_params, OpaqueTy, Ty,
};
