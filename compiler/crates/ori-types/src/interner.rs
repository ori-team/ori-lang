//! Arena-backed Type Interner (`TyInterner`) and compact type handles (`TyId`).
//!
//! Provides canonical deduplication and O(1) Copy type handles to eliminate
//! deep `Ty` cloning overhead in large AST trees and generic type unifications.

use crate::ty::Ty;
use std::collections::HashMap;

/// A lightweight, copyable 32-bit identifier pointing into a `TyInterner` arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TyId(pub u32);

impl TyId {
    pub const BOOL: TyId = TyId(0);
    pub const INT: TyId = TyId(1);
    pub const INT8: TyId = TyId(2);
    pub const INT16: TyId = TyId(3);
    pub const INT32: TyId = TyId(4);
    pub const INT64: TyId = TyId(5);
    pub const U8: TyId = TyId(6);
    pub const U16: TyId = TyId(7);
    pub const U32: TyId = TyId(8);
    pub const U64: TyId = TyId(9);
    pub const FLOAT: TyId = TyId(10);
    pub const FLOAT32: TyId = TyId(11);
    pub const FLOAT64: TyId = TyId(12);
    pub const STRING: TyId = TyId(13);
    pub const BYTES: TyId = TyId(14);
    pub const VOID: TyId = TyId(15);
    pub const NEVER: TyId = TyId(16);
    pub const ERROR: TyId = TyId(17);

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// An arena and interning table that stores canonical `Ty` representations.
#[derive(Debug, Clone)]
pub struct TyInterner {
    arena: Vec<Ty>,
    map: HashMap<Ty, TyId>,
}

impl Default for TyInterner {
    fn default() -> Self {
        let mut interner = Self {
            arena: Vec::with_capacity(128),
            map: HashMap::with_capacity(128),
        };
        interner.seed_primitives();
        interner
    }
}

impl TyInterner {
    /// Creates a new `TyInterner` initialized with canonical primitive types.
    pub fn new() -> Self {
        Self::default()
    }

    fn seed_primitives(&mut self) {
        let primitives = [
            Ty::Bool,
            Ty::Int,
            Ty::Int8,
            Ty::Int16,
            Ty::Int32,
            Ty::Int64,
            Ty::U8,
            Ty::U16,
            Ty::U32,
            Ty::U64,
            Ty::Float,
            Ty::Float32,
            Ty::Float64,
            Ty::String,
            Ty::Bytes,
            Ty::Void,
            Ty::Never,
            Ty::Error,
        ];
        for (i, ty) in primitives.into_iter().enumerate() {
            let id = TyId(i as u32);
            self.arena.push(ty.clone());
            self.map.insert(ty, id);
        }
    }

    /// Interns a `Ty`, returning its unique `TyId`. If the type was already
    /// interned, returns the existing `TyId` without re-allocating.
    pub fn intern(&mut self, ty: Ty) -> TyId {
        if let Some(&id) = self.map.get(&ty) {
            return id;
        }
        let id = TyId(self.arena.len() as u32);
        self.arena.push(ty.clone());
        self.map.insert(ty, id);
        id
    }

    /// Resolves a `TyId` back to its canonical reference `&Ty`.
    #[inline]
    pub fn get(&self, id: TyId) -> &Ty {
        &self.arena[id.0 as usize]
    }

    /// Returns the total number of unique interned types.
    #[inline]
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Returns true if the interner is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interner_primitives() {
        let interner = TyInterner::new();
        assert_eq!(interner.get(TyId::INT), &Ty::Int);
        assert_eq!(interner.get(TyId::STRING), &Ty::String);
        assert_eq!(interner.get(TyId::BOOL), &Ty::Bool);
        assert_eq!(interner.get(TyId::VOID), &Ty::Void);
    }

    #[test]
    fn test_interner_dedup() {
        let mut interner = TyInterner::new();
        let list_int = Ty::List(Box::new(Ty::Int));
        let id1 = interner.intern(list_int.clone());
        let id2 = interner.intern(list_int);
        assert_eq!(id1, id2);
        assert_eq!(interner.get(id1), &Ty::List(Box::new(Ty::Int)));
    }
}
