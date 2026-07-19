use ori_diagnostics::Span;
use smol_str::SmolStr;
use std::collections::HashMap;

/// A unique identifier for a top-level definition within a compilation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

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
    /// Simple (unqualified) name: `"User"`, `"connect"`.
    pub name: SmolStr,
    /// Fully-qualified path: `"app.user.User"`, `"ori.io.print"`.
    pub path: SmolStr,
    pub is_public: bool,
    pub span: Span,
}

/// Maps fully-qualified names to their definitions.
///
/// Populated during name resolution; queried by the type checker.
#[derive(Debug, Default)]
pub struct DefMap {
    defs: Vec<Def>,
    by_path: HashMap<SmolStr, DefId>,
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
        span: Span,
    ) -> DefId {
        if let Some(&existing) = self.by_path.get(&path) {
            return existing;
        }
        let id = DefId(self.defs.len() as u32);
        self.defs.push(Def {
            id,
            kind,
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
        if id.0 >= self.defs.len() as u32 {
            static DUMMY_DEF: std::sync::OnceLock<Def> = std::sync::OnceLock::new();
            return DUMMY_DEF.get_or_init(|| Def {
                id: DefId(0x7FFF_FFFF),
                kind: DefKind::Struct,
                name: SmolStr::new(""),
                path: SmolStr::new(""),
                is_public: false,
                span: Span { start: 0, end: 0 },
            });
        }
        &self.defs[id.0 as usize]
    }

    pub fn try_get(&self, id: DefId) -> Option<&Def> {
        if id.0 >= self.defs.len() as u32 {
            static DUMMY_DEF: std::sync::OnceLock<Def> = std::sync::OnceLock::new();
            return Some(DUMMY_DEF.get_or_init(|| Def {
                id: DefId(0x7FFF_FFFF),
                kind: DefKind::Struct,
                name: SmolStr::new(""),
                path: SmolStr::new(""),
                is_public: false,
                span: Span { start: 0, end: 0 },
            }));
        }
        self.defs.get(id.0 as usize)
    }

    pub fn all_defs(&self) -> &[Def] {
        &self.defs
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}
