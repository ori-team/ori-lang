use crate::def::{DefId, DefKind, DefMap};
use smol_str::SmolStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpaqueTy {
    Deque,
    Queue,
    Stack,
    LinkedList,
    DoublyLinkedList,
    Tree,
    NodeId,
    HashTable,
    Graph,
    Heap,
    File,
    CancelToken,
    Connection,
    Input,
    Output,
    Listener,
    UdpSocket,
}

impl OpaqueTy {
    pub fn display_name(self) -> &'static str {
        match self {
            OpaqueTy::Deque => "deque.Deque",
            OpaqueTy::Queue => "queue.Queue",
            OpaqueTy::Stack => "stack.Stack",
            OpaqueTy::LinkedList => "linked_list.LinkedList",
            OpaqueTy::DoublyLinkedList => "doubly_linked_list.DoublyLinkedList",
            OpaqueTy::Tree => "tree.Tree",
            OpaqueTy::NodeId => "tree.NodeId",
            OpaqueTy::HashTable => "hash_table.HashTable",
            OpaqueTy::Graph => "graph.Graph",
            OpaqueTy::Heap => "heap.Heap",
            OpaqueTy::File => "fs.File",
            OpaqueTy::CancelToken => "task.CancelToken",
            OpaqueTy::Connection => "net.Connection",
            OpaqueTy::Input => "io.Input",
            OpaqueTy::Output => "io.Output",
            OpaqueTy::Listener => "net.Listener",
            OpaqueTy::UdpSocket => "net.UdpSocket",
        }
    }

    pub fn is_list_backed_collection(self) -> bool {
        matches!(
            self,
            OpaqueTy::Deque
                | OpaqueTy::Queue
                | OpaqueTy::Stack
                | OpaqueTy::LinkedList
                | OpaqueTy::DoublyLinkedList
        )
    }
}

/// The canonical type representation used throughout the type checker.
///
/// Unlike `ori_ast::ty::Type` (which mirrors source syntax), `Ty` uses
/// resolved `DefId`s so comparisons are O(1) for named types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    // ── Primitives ────────────────────────────────────────────────────────────
    Bool,
    Int,
    Int8,
    Int16,
    Int32,
    Int64,
    U8,
    U16,
    U32,
    U64,
    Float,
    Float32,
    Float64,
    String,
    Bytes,
    Void,

    /// Type of expressions that never return (break, continue, panic, return).
    Never,

    /// Sentinel — emitted after a type error so checking can continue.
    Error,

    // ── Built-in generic types ────────────────────────────────────────────────
    Optional(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    List(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Set(Box<Ty>),
    Range(Box<Ty>),
    Lazy(Box<Ty>),
    Handle(Box<Ty>),
    Future(Box<Ty>),
    TaskJob(Box<Ty>),
    Channel(Box<Ty>),
    AtomicInt,
    TaskJoinError,
    ChannelSendError,
    ChannelReceiveError,
    Opaque {
        kind: OpaqueTy,
        args: Vec<Ty>,
    },

    /// `any<Trait>` — dynamic dispatch; trait identified by `DefId`.
    Any(DefId),

    /// `tuple<A, B, …>` — always 2 or more elements.
    Tuple(Vec<Ty>),

    /// `func(T, U) -> R`
    Func {
        params: Vec<Ty>,
        ret: Box<Ty>,
    },

    // ── User-defined types ────────────────────────────────────────────────────
    /// A named type (struct or enum) with optional generic arguments.
    Named(DefId, Vec<Ty>),

    // ── Generic type parameters ───────────────────────────────────────────────
    /// A generic type parameter inside a declaration: `T` in `func f<T>`.
    Param {
        index: u32,
        name: SmolStr,
    },

    /// An unsolved inference variable (used during type inference).
    Infer(u32),
}

impl Ty {
    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }
    pub fn is_never(&self) -> bool {
        matches!(self, Ty::Never)
    }
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Ty::Int
                | Ty::Int8
                | Ty::Int16
                | Ty::Int32
                | Ty::Int64
                | Ty::U8
                | Ty::U16
                | Ty::U32
                | Ty::U64
                | Ty::Float
                | Ty::Float32
                | Ty::Float64
        )
    }
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Ty::Int
                | Ty::Int8
                | Ty::Int16
                | Ty::Int32
                | Ty::Int64
                | Ty::U8
                | Ty::U16
                | Ty::U32
                | Ty::U64
        )
    }
    pub fn is_float(&self) -> bool {
        matches!(self, Ty::Float | Ty::Float32 | Ty::Float64)
    }
    pub fn is_node_id(&self) -> bool {
        matches!(
            self,
            Ty::Opaque {
                kind: OpaqueTy::NodeId,
                ..
            }
        )
    }
    pub fn is_runtime_managed(&self) -> bool {
        matches!(
            self,
            Ty::String
                | Ty::Bytes
                | Ty::List(_)
                | Ty::Map(_, _)
                | Ty::Set(_)
                | Ty::Range(_)
                | Ty::Optional(_)
                | Ty::Result(_, _)
                | Ty::Tuple(_)
                | Ty::Named(_, _)
                | Ty::Any(_)
                | Ty::Func { .. }
                | Ty::Lazy(_)
                | Ty::Future(_)
                | Ty::TaskJob(_)
                | Ty::Channel(_)
                | Ty::AtomicInt
                | Ty::TaskJoinError
                | Ty::ChannelSendError
                | Ty::ChannelReceiveError
                | Ty::Opaque {
                    kind: OpaqueTy::Deque
                        | OpaqueTy::Queue
                        | OpaqueTy::Stack
                        | OpaqueTy::LinkedList
                        | OpaqueTy::DoublyLinkedList
                        | OpaqueTy::Tree
                        | OpaqueTy::HashTable
                        | OpaqueTy::Graph
                        | OpaqueTy::Heap
                        | OpaqueTy::File
                        | OpaqueTy::CancelToken
                        | OpaqueTy::Connection
                        | OpaqueTy::Input
                        | OpaqueTy::Output
                        | OpaqueTy::Listener
                        | OpaqueTy::UdpSocket,
                    ..
                }
        )
    }

    /// Returns `true` if this type or any contained type is an inference variable.
    pub fn contains_infer(&self) -> bool {
        match self {
            Ty::Infer(_) => true,
            Ty::Optional(t)
            | Ty::List(t)
            | Ty::Set(t)
            | Ty::Range(t)
            | Ty::Lazy(t)
            | Ty::Handle(t)
            | Ty::Future(t)
            | Ty::TaskJob(t)
            | Ty::Channel(t) => t.contains_infer(),
            Ty::Any(_) => false,
            Ty::Result(a, b) | Ty::Map(a, b) => a.contains_infer() || b.contains_infer(),
            Ty::Opaque { args, .. } => args.iter().any(|arg| arg.contains_infer()),
            Ty::Tuple(ts) => ts.iter().any(|t| t.contains_infer()),
            Ty::Func { params, ret } => {
                params.iter().any(|p| p.contains_infer()) || ret.contains_infer()
            }
            Ty::Named(_, args) => args.iter().any(|a| a.contains_infer()),
            _ => false,
        }
    }

    /// `Never` and unsolved `Infer` (at any depth) are treated as assignable in this
    /// structural check. Ori requires explicit binding annotations; local inference
    /// variables are solved in `TypeChecker::unify`, not here. This helper treats
    /// `Infer(_)` as a wildcard when comparing type shapes.
    pub fn is_assignable_to(&self, other: &Ty) -> bool {
        use Ty::*;
        // Reflexive & error/never rules
        if self == other {
            return true;
        }
        if matches!(
            (self, other),
            (Ty::Int, Ty::Int64)
                | (Ty::Int64, Ty::Int)
                | (Ty::Float, Ty::Float64)
                | (Ty::Float64, Ty::Float)
        ) {
            return true;
        }
        if matches!((self, other), (Ty::Int, ty) | (ty, Ty::Int) if ty.is_node_id()) {
            return true;
        }
        if self.is_error() {
            return true;
        }
        if self.is_never() {
            return true;
        }
        if other.is_error() {
            return true;
        }

        // Wildcards — any Infer matches anything
        if matches!(self, Infer(_)) || matches!(other, Infer(_)) {
            return true;
        }

        match (self, other) {
            (Optional(a), Optional(b)) => a.is_assignable_to(b),
            (Result(a_ok, a_err), Result(b_ok, b_err)) => {
                a_ok.is_assignable_to(b_ok) && a_err.is_assignable_to(b_err)
            }
            (List(a), List(b))
            | (Set(a), Set(b))
            | (Range(a), Range(b))
            | (Lazy(a), Lazy(b))
            | (Future(a), Future(b))
            | (TaskJob(a), TaskJob(b))
            | (Channel(a), Channel(b)) => a.is_assignable_to(b),
            (Map(ka, va), Map(kb, vb)) => ka.is_assignable_to(kb) && va.is_assignable_to(vb),
            (
                Opaque {
                    kind: kind_a,
                    args: args_a,
                },
                Opaque {
                    kind: kind_b,
                    args: args_b,
                },
            ) => {
                kind_a == kind_b
                    && args_a.len() == args_b.len()
                    && args_a
                        .iter()
                        .zip(args_b.iter())
                        .all(|(a, b)| a.is_assignable_to(b))
            }
            (Tuple(as_), Tuple(bs)) => {
                as_.len() == bs.len()
                    && as_
                        .iter()
                        .zip(bs.iter())
                        .all(|(a, b)| a.is_assignable_to(b))
            }
            (
                Func {
                    params: ps_a,
                    ret: ra,
                },
                Func {
                    params: ps_b,
                    ret: rb,
                },
            ) => {
                ps_a.len() == ps_b.len()
                    && ps_a
                        .iter()
                        .zip(ps_b.iter())
                        .all(|(a, b)| a.is_assignable_to(b))
                    && ra.is_assignable_to(rb)
            }
            (Named(id_a, args_a), Named(id_b, args_b)) => {
                id_a == id_b
                    && args_a.len() == args_b.len()
                    && args_a
                        .iter()
                        .zip(args_b.iter())
                        .all(|(a, b)| a.is_assignable_to(b))
            }
            (Any(id_a), Any(id_b)) => id_a == id_b,
            _ => false,
        }
    }

    /// Human-readable display name for diagnostics.
    /// Like [`Ty::display`], but prints declared names for `Ty::Named` instead
    /// of raw ids.
    ///
    /// `display` cannot do this because it has no def map, which is why
    /// diagnostics used to leak `<def DefId(16)>` at the reader. Prefer this
    /// wherever a def map is in reach.
    pub fn display_in(&self, def_map: &DefMap) -> std::string::String {
        match self {
            Ty::Named(id, args) => {
                let name = def_map.get(*id).name.clone();
                let name = if name.is_empty() {
                    return self.display();
                } else {
                    name
                };
                if args.is_empty() {
                    name.to_string()
                } else {
                    let inner = args
                        .iter()
                        .map(|a| a.display_in(def_map))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}[{}]", name, inner)
                }
            }
            Ty::Optional(inner) => format!("optional[{}]", inner.display_in(def_map)),
            Ty::Result(ok, err) => format!(
                "result[{}, {}]",
                ok.display_in(def_map),
                err.display_in(def_map)
            ),
            Ty::List(inner) => format!("list[{}]", inner.display_in(def_map)),
            Ty::Set(inner) => format!("set[{}]", inner.display_in(def_map)),
            Ty::Map(k, v) => format!("map[{}, {}]", k.display_in(def_map), v.display_in(def_map)),
            _ => self.display(),
        }
    }

    pub fn display(&self) -> std::string::String {
        match self {
            Ty::Bool => "bool".into(),
            Ty::Int => "int".into(),
            Ty::Int8 => "int8".into(),
            Ty::Int16 => "int16".into(),
            Ty::Int32 => "int32".into(),
            Ty::Int64 => "int64".into(),
            Ty::U8 => "u8".into(),
            Ty::U16 => "u16".into(),
            Ty::U32 => "u32".into(),
            Ty::U64 => "u64".into(),
            Ty::Float => "float".into(),
            Ty::Float32 => "float32".into(),
            Ty::Float64 => "float64".into(),
            Ty::String => "string".into(),
            Ty::Bytes => "bytes".into(),
            Ty::Void => "void".into(),
            Ty::Never => "never".into(),
            Ty::Error => "<error>".into(),
            Ty::Optional(t) => format!("optional[{}]", t.display()),
            Ty::Result(ok, err) => format!("result[{}, {}]", ok.display(), err.display()),
            Ty::List(t) => format!("list[{}]", t.display()),
            Ty::Map(k, v) => format!("map[{}, {}]", k.display(), v.display()),
            Ty::Set(t) => format!("set[{}]", t.display()),
            Ty::Range(t) => format!("range[{}]", t.display()),
            Ty::Lazy(t) => format!("lazy[{}]", t.display()),
            Ty::Handle(t) => format!("handle[{}]", t.display()),
            Ty::Future(t) => format!("future[{}]", t.display()),
            Ty::TaskJob(t) => format!("task.Job[{}]", t.display()),
            Ty::Channel(t) => format!("channel.Channel[{}]", t.display()),
            Ty::AtomicInt => "atomic.AtomicInt".into(),
            Ty::TaskJoinError => "task.JoinError".into(),
            Ty::ChannelSendError => "channel.SendError".into(),
            Ty::ChannelReceiveError => "channel.ReceiveError".into(),
            Ty::Opaque { kind, args } => {
                if args.is_empty() {
                    kind.display_name().into()
                } else {
                    let args = args
                        .iter()
                        .map(|arg| arg.display())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}[{}]", kind.display_name(), args)
                }
            }
            Ty::Any(d) => format!("any[{:?}]", d),
            Ty::Tuple(ts) => {
                let inner = ts
                    .iter()
                    .map(|t| t.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tuple[{}]", inner)
            }
            Ty::Func { params, ret } => {
                let ps = params
                    .iter()
                    .map(|p| p.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("func({}) -> {}", ps, ret.display())
            }
            Ty::Named(id, args) => {
                // No def map here, so the raw id is the only honest fallback.
                // `display_in` prints the declared name and is what
                // diagnostics should use; see its note.
                if args.is_empty() {
                    format!("<def {:?}>", id)
                } else {
                    let as_ = args
                        .iter()
                        .map(|a| a.display())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("<def {:?}>[{}]", id, as_)
                }
            }
            Ty::Param { name, .. } => name.to_string(),
            Ty::Infer(id) => format!("_#{}", id),
        }
    }

    pub fn list_backed_collection_elem(&self) -> Option<&Ty> {
        match self {
            Ty::Opaque { kind, args } if kind.is_list_backed_collection() => args.first(),
            _ => None,
        }
    }
}

// ── Type alias expansion ───────────────────────────────────────────────────────

/// Substitute `Ty::Param { index, .. }` placeholders with the actual type
/// arguments in `args`.  Used when instantiating a generic type alias.
pub fn substitute_ty_params(ty: &Ty, args: &[Ty]) -> Ty {
    match ty {
        Ty::Param { index, .. } => args
            .get(*index as usize)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Ty::Named(id, inner_args) => {
            let new_args = inner_args
                .iter()
                .map(|a| substitute_ty_params(a, args))
                .collect();
            Ty::Named(*id, new_args)
        }
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_ty_params(inner, args))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(substitute_ty_params(ok, args)),
            Box::new(substitute_ty_params(err, args)),
        ),
        Ty::List(elem) => Ty::List(Box::new(substitute_ty_params(elem, args))),
        Ty::Map(k, v) => Ty::Map(
            Box::new(substitute_ty_params(k, args)),
            Box::new(substitute_ty_params(v, args)),
        ),
        Ty::Set(elem) => Ty::Set(Box::new(substitute_ty_params(elem, args))),
        Ty::Range(elem) => Ty::Range(Box::new(substitute_ty_params(elem, args))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(substitute_ty_params(inner, args))),
        Ty::Handle(inner) => Ty::Handle(Box::new(substitute_ty_params(inner, args))),
        Ty::Future(inner) => Ty::Future(Box::new(substitute_ty_params(inner, args))),
        Ty::TaskJob(inner) => Ty::TaskJob(Box::new(substitute_ty_params(inner, args))),
        Ty::Channel(inner) => Ty::Channel(Box::new(substitute_ty_params(inner, args))),
        Ty::Opaque { kind, args: inner } => Ty::Opaque {
            kind: *kind,
            args: inner
                .iter()
                .map(|arg| substitute_ty_params(arg, args))
                .collect(),
        },
        Ty::Tuple(elems) => Ty::Tuple(
            elems
                .iter()
                .map(|e| substitute_ty_params(e, args))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .iter()
                .map(|p| substitute_ty_params(p, args))
                .collect(),
            ret: Box::new(substitute_ty_params(ret, args)),
        },
        other => other.clone(),
    }
}

/// A minimal view of a type alias signature needed for expansion.
pub struct AliasView<'a> {
    pub def_id: DefId,
    pub ty: &'a Ty,
    pub arity: usize,
}

/// Expand all `Ty::Named(id, args)` where `id` refers to a `TypeAlias` def.
///
/// The expansion is performed recursively until no alias remains (with a
/// depth-limit guard to avoid infinite loops on ill-formed cyclic aliases).
pub fn normalize_ty_aliases<F>(ty: Ty, lookup: &F) -> Ty
where
    F: Fn(DefId) -> Option<(usize, Ty)>,
{
    normalize_ty_aliases_depth(ty, lookup, 0)
}

fn normalize_ty_aliases_depth<F>(ty: Ty, lookup: &F, depth: usize) -> Ty
where
    F: Fn(DefId) -> Option<(usize, Ty)>,
{
    if depth > 32 {
        // Safety valve against cyclic aliases.
        return ty;
    }
    match ty {
        Ty::Named(id, args) => {
            let new_args: Vec<Ty> = args
                .into_iter()
                .map(|a| normalize_ty_aliases_depth(a, lookup, depth))
                .collect();
            if let Some((_arity, alias_ty)) = lookup(id) {
                let expanded = substitute_ty_params(&alias_ty, &new_args);
                normalize_ty_aliases_depth(expanded, lookup, depth + 1)
            } else {
                Ty::Named(id, new_args)
            }
        }
        Ty::Optional(inner) => {
            Ty::Optional(Box::new(normalize_ty_aliases_depth(*inner, lookup, depth)))
        }
        Ty::Result(ok, err) => Ty::Result(
            Box::new(normalize_ty_aliases_depth(*ok, lookup, depth)),
            Box::new(normalize_ty_aliases_depth(*err, lookup, depth)),
        ),
        Ty::List(elem) => Ty::List(Box::new(normalize_ty_aliases_depth(*elem, lookup, depth))),
        Ty::Map(k, v) => Ty::Map(
            Box::new(normalize_ty_aliases_depth(*k, lookup, depth)),
            Box::new(normalize_ty_aliases_depth(*v, lookup, depth)),
        ),
        Ty::Set(elem) => Ty::Set(Box::new(normalize_ty_aliases_depth(*elem, lookup, depth))),
        Ty::Range(elem) => Ty::Range(Box::new(normalize_ty_aliases_depth(*elem, lookup, depth))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(normalize_ty_aliases_depth(*inner, lookup, depth))),
        Ty::Handle(inner) => {
            Ty::Handle(Box::new(normalize_ty_aliases_depth(*inner, lookup, depth)))
        }
        Ty::Future(inner) => {
            Ty::Future(Box::new(normalize_ty_aliases_depth(*inner, lookup, depth)))
        }
        Ty::TaskJob(inner) => {
            Ty::TaskJob(Box::new(normalize_ty_aliases_depth(*inner, lookup, depth)))
        }
        Ty::Channel(inner) => {
            Ty::Channel(Box::new(normalize_ty_aliases_depth(*inner, lookup, depth)))
        }
        Ty::Opaque { kind, args } => Ty::Opaque {
            kind,
            args: args
                .into_iter()
                .map(|arg| normalize_ty_aliases_depth(arg, lookup, depth))
                .collect(),
        },
        Ty::Tuple(elems) => Ty::Tuple(
            elems
                .into_iter()
                .map(|e| normalize_ty_aliases_depth(e, lookup, depth))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .into_iter()
                .map(|p| normalize_ty_aliases_depth(p, lookup, depth))
                .collect(),
            ret: Box::new(normalize_ty_aliases_depth(*ret, lookup, depth)),
        },
        other => other,
    }
}

/// Convenience wrapper: expand aliases given a `DefMap` and a slice of
/// `TypeAliasSig`-like pairs `(def_id, ty)`.
pub fn expand_ty_aliases(
    ty: Ty,
    def_map: &DefMap,
    alias_map: &std::collections::HashMap<DefId, (usize, Ty)>,
) -> Ty {
    normalize_ty_aliases(ty, &|id| {
        if def_map.get(id).kind == DefKind::TypeAlias {
            alias_map.get(&id).cloned()
        } else {
            None
        }
    })
}

/// Replace every `newtype` with its representation, everywhere in `ty`.
///
/// The checker needs newtypes to stay nominal (`UserId` is not `int`, so
/// passing one where the other is expected fails). Codegen needs the opposite:
/// a `newtype` over `int` must *be* an `int` at runtime, not a pointer to a
/// boxed value. Lowering to HIR is the seam between those two views — erase
/// there and nothing downstream (HIR, ARC, layout, both backends) has to know
/// newtypes exist, which is also what makes them cost nothing.
pub fn erase_newtypes(
    ty: Ty,
    def_map: &DefMap,
    newtype_map: &std::collections::HashMap<DefId, Ty>,
) -> Ty {
    normalize_ty_aliases(ty, &|id| {
        if def_map.get(id).kind == DefKind::Newtype {
            // Arity 0: newtypes take no type parameters today.
            newtype_map.get(&id).cloned().map(|repr| (0, repr))
        } else {
            None
        }
    })
}
