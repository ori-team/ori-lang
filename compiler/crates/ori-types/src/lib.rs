// ori-types

pub mod check;
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
pub use lower::{lower_type, lower_type_with_aliases, lower_type_with_local_aliases};
pub use resolve::{
    resolve, EnumSig, EnumVariantSig, FuncSig, ImplMethodSig, ImplSig, ReExport, ResolvedModule,
    StructSig, TraitMethodSig, TraitSig, TypeAliasSig, ValueSig, WhereConstraintSig,
};
pub use ty::{
    expand_ty_aliases, normalize_ty_aliases, substitute_trait_self, substitute_ty_params, OpaqueTy,
    Ty,
};
