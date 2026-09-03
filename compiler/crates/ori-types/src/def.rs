use ori_diagnostics::FileId;
use ori_diagnostics::Span;
use smol_str::SmolStr;
use std::collections::HashMap;

/// A unique identifier for a top-level definition within a compilation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

impl DefId {
    /// ID reserved for an unresolved definition while lowering recovers from
    /// a prior diagnostic. It is never allocated by [`DefMap::register`].
    ///
    /// Keeping this value named (instead of repeating `u32::MAX`) makes
    /// recovery paths auditable and gives validation code one source of truth.
    pub const INVALID: Self = Self(u32::MAX);

    /// ID used by the driver for the synthetic `main`/test harness function.
    /// Synthetic definitions live outside the sequential user-definition
    /// arena and must not be looked up in a [`DefMap`].
    pub const SYNTHETIC_MAIN: Self = Self(u32::MAX - 1);

    /// Reserved range for IDs used while lowering a literal in a type-shaped
    /// recovery path. These IDs are never inserted into `DefMap`.
    const SYNTHETIC_LITERAL_BASE: u32 = 0x2000_0000;
    /// ID used while the stdlib is bootstrapped before `ori.json.Value` is
    /// registered in the session definition map. It is never allocated by
    /// [`DefMap::register`].
    pub const SYNTHETIC_JSON_VALUE: Self = Self(0x1000_0000);
    /// Reserved range for IDs used for an applied type parameter before
    /// substitution. These IDs are never inserted into `DefMap`.
    const SYNTHETIC_TYPE_PARAM_BASE: u32 = 0x4000_0000;
    /// Reserved range for compiler-generated closure functions.
    const SYNTHETIC_CLOSURE_BASE: u32 = 0xC000_0000;

    pub fn synthetic_literal(hash: u32) -> Self {
        Self(Self::SYNTHETIC_LITERAL_BASE | (hash & 0x1FFF_FFFF))
    }

    pub fn synthetic_type_param(index: u32) -> Self {
        Self(Self::SYNTHETIC_TYPE_PARAM_BASE | (index & 0x3FFF_FFFF))
    }

    pub fn synthetic_closure(index: u32) -> Self {
        Self(
            Self::SYNTHETIC_CLOSURE_BASE
                .checked_add(index)
                .expect("synthetic closure ID space exhausted"),
        )
    }

    pub const fn is_synthetic_type_param(self) -> bool {
        self.0 & 0xC000_0000 == Self::SYNTHETIC_TYPE_PARAM_BASE
    }

    pub const fn is_invalid(self) -> bool {
        self.0 == u32::MAX
    }
}

impl std::fmt::Display for DefId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "def#{}", self.0)
    }
}

/// What kind of thing a `DefId` refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Struct,
    Enum,
    Trait,
    Func,
    Const,
    Var,
    TypeAlias,
    /// `newtype Name = Repr` — nominal type over an existing representation.
    Newtype,
    Extern,
}

/// A single registered definition.
#[derive(Debug, Clone)]
pub struct Def {
    pub id: DefId,
    pub kind: DefKind,
    /// Source file that owns this definition.
    ///
    /// Compiler-provided definitions use [`BUILTIN_FILE_ID`]. Consumers must
    /// not try to resolve that sentinel through a source cache.
    pub file_id: FileId,
    /// Simple (unqualified) name: `"User"`, `"connect"`.
    pub name: SmolStr,
    /// Fully-qualified path: `"app.user.User"`, `"ori.io.print"`.
    pub path: SmolStr,
    pub is_public: bool,
    pub span: Span,
}

/// Sentinel source identity for compiler-provided definitions.
pub const BUILTIN_FILE_ID: FileId = FileId(u32::MAX);

/// A scalar value proven at compile time by the CT-0 evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileTimeValue {
    Int(i64),
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstEvalFailureKind {
    UnsupportedExpression,
    InvalidLiteral,
    UndefinedName,
    NonConstName,
    PrivateName,
    Cycle,
    Overflow,
    DivisionByZero,
    TypeMismatch,
}

/// Why a module constant could not be used in a compile-time expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEvalFailure {
    pub kind: ConstEvalFailureKind,
    pub file_id: FileId,
    pub span: Span,
    pub detail: String,
}

pub type ConstEvaluation = Result<CompileTimeValue, ConstEvalFailure>;

/// Maps fully-qualified names to their definitions.
///
/// Populated during name resolution; queried by the type checker.
#[derive(Debug, Default)]
pub struct DefMap {
    defs: Vec<Def>,
    by_path: HashMap<SmolStr, DefId>,
    const_evaluations: HashMap<DefId, ConstEvaluation>,
}

impl DefMap {
    /// Register a new definition. Returns its `DefId`.
    ///
    /// If `path` is already registered, returns the existing `DefId` without
    /// inserting a duplicate (the caller should emit a `name.duplicate` error).
    pub fn register(
        &mut self,
        kind: DefKind,
        name: SmolStr,
        path: SmolStr,
        is_public: bool,
        file_id: FileId,
        span: Span,
    ) -> DefId {
        if let Some(&existing) = self.by_path.get(&path) {
            return existing;
        }
        let id =
            DefId(u32::try_from(self.defs.len()).expect("definition arena exhausted u32 ID space"));
        self.defs.push(Def {
            id,
            kind,
            file_id,
            name,
            path: path.clone(),
            is_public,
            span,
        });
        self.by_path.insert(path, id);
        id
    }

    pub fn lookup(&self, path: &str) -> Option<DefId> {
        self.by_path.get(path).copied()
    }

    /// Alias `path` to an existing definition (e.g. free bind `Type.slot` → free function).
    ///
    /// No-op if `path` is already registered. Used so inherent method lookup
    /// (`namespace.Type.slot`) resolves to the bound free function's `DefId`.
    pub fn alias_path(&mut self, path: SmolStr, existing: DefId) {
        if self.by_path.contains_key(&path) {
            return;
        }
        self.by_path.insert(path, existing);
    }

    pub fn get(&self, id: DefId) -> &Def {
        self.try_get(id)
            .unwrap_or_else(|| panic!("invalid definition ID {id}"))
    }

    pub fn try_get(&self, id: DefId) -> Option<&Def> {
        self.defs.get(id.0 as usize)
    }

    pub fn contains(&self, id: DefId) -> bool {
        self.try_get(id).is_some()
    }

    pub fn all_defs(&self) -> &[Def] {
        &self.defs
    }

    pub(crate) fn set_const_evaluations(&mut self, evaluations: HashMap<DefId, ConstEvaluation>) {
        self.const_evaluations = evaluations;
    }

    pub fn const_evaluation(&self, id: DefId) -> Option<&ConstEvaluation> {
        self.const_evaluations.get(&id)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_definition_keeps_its_source_identity() {
        let mut definitions = DefMap::default();
        let file_id = FileId(17);
        let id = definitions.register(
            DefKind::Func,
            SmolStr::new("answer"),
            SmolStr::new("app.main.answer"),
            true,
            file_id,
            Span { start: 10, end: 16 },
        );

        assert_eq!(definitions.get(id).file_id, file_id);
        assert_eq!(definitions.get(id).span, Span { start: 10, end: 16 });
    }

    #[test]
    fn invalid_definition_ids_are_rejected_instead_of_returning_a_dummy() {
        let definitions = DefMap::default();

        assert!(!definitions.contains(DefId::INVALID));
        assert!(definitions.try_get(DefId::INVALID).is_none());
        assert_ne!(DefId::synthetic_closure(0), DefId::INVALID);
        assert_ne!(DefId::synthetic_closure(0), DefId::SYNTHETIC_MAIN);
    }

    #[test]
    fn sequential_ids_remain_valid_past_the_old_placeholder_value() {
        let mut definitions = DefMap::default();
        for index in 0..10_001 {
            let id = definitions.register(
                DefKind::Func,
                SmolStr::new(format!("f{index}")),
                SmolStr::new(format!("m.f{index}")),
                false,
                FileId(1),
                Span { start: 0, end: 0 },
            );
            assert_eq!(id.0 as usize, index);
            assert_eq!(definitions.get(id).name, format!("f{index}"));
        }

        assert!(definitions.contains(DefId(10_000)));
        assert!(definitions.contains(DefId(9_999)));
    }

    #[test]
    fn synthetic_json_id_is_outside_the_definition_arena() {
        assert_eq!(DefId::SYNTHETIC_JSON_VALUE.0, 0x1000_0000);
        assert_ne!(DefId::SYNTHETIC_JSON_VALUE, DefId::INVALID);
        assert_ne!(DefId::SYNTHETIC_JSON_VALUE, DefId::SYNTHETIC_MAIN);
        assert_ne!(DefId::SYNTHETIC_JSON_VALUE, DefId::synthetic_literal(0));
        assert_ne!(DefId::SYNTHETIC_JSON_VALUE, DefId::synthetic_type_param(0));
        assert_ne!(DefId::SYNTHETIC_JSON_VALUE, DefId::synthetic_closure(0));
    }
}
