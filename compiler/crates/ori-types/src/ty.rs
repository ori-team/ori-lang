use crate::def::{DefId, DefKind, DefMap};
use smol_str::SmolStr;
use std::collections::HashMap;

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
    /// `buffer[T]` — a contiguous, mutably-indexable, fixed-length heap block.
    Buffer(Box<Ty>),
    /// `slice[T]` — a read-only window over a `list[T]`.
    Slice(Box<Ty>),
    /// `array[T, size: N]` — element type and length.
    ///
    /// The length is carried as a `Ty` so it reuses the const-generic
    /// machinery: it is `ConstInt("size", n)` once known, or a `Param` while
    /// still generic (`array[byte, size: cap]` inside `Buffer[const cap: int]`).
    /// Two arrays of different length are different types.
    Array(Box<Ty>, Box<Ty>),
    /// `simd[T, N]` — portable fixed-width SIMD vector (LANG-SIMD-1).
    Simd(Box<Ty>, u16),
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

    /// A compile-time constant standing in a type argument position:
    /// `Buffer[size: 8]` is `Named(Buffer, [ConstInt("size", 8)])`.
    ///
    /// Two buffers with different sizes are different types. `array[T, size: N]`
    /// consumes the value for its layout; elsewhere it stays a compile-time tag
    /// that never reaches runtime.
    ConstInt(SmolStr, i64),
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
    /// `true` for the unsigned integer types, which need `udiv`/`urem` and
    /// unsigned comparisons instead of their signed counterparts.
    pub fn is_unsigned_integer(&self) -> bool {
        matches!(self, Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64)
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
                | Ty::Buffer(_)
                | Ty::Slice(_)
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
            | Ty::Buffer(t)
            | Ty::Slice(t)
            | Ty::Set(t)
            | Ty::Range(t)
            | Ty::Lazy(t)
            | Ty::Handle(t)
            | Ty::Future(t)
            | Ty::TaskJob(t)
            | Ty::Channel(t) => t.contains_infer(),
            Ty::Any(_) => false,
            Ty::Result(a, b) | Ty::Map(a, b) => a.contains_infer() || b.contains_infer(),
            Ty::Array(elem, size) => elem.contains_infer() || size.contains_infer(),
            Ty::Simd(elem, _) => elem.contains_infer(),
            Ty::Opaque { args, .. } => args.iter().any(|arg| arg.contains_infer()),
            Ty::Tuple(ts) => ts.iter().any(|t| t.contains_infer()),
            Ty::Func { params, ret } => {
                params.iter().any(|p| p.contains_infer()) || ret.contains_infer()
            }
            Ty::Named(_, args) => args.iter().any(|a| a.contains_infer()),
            _ => false,
        }
    }

    /// `true` when `Ty::Error` appears anywhere in the type.
    ///
    /// A type that already carries an error is the *result* of a diagnostic
    /// that was reported earlier; comparing it against anything else can only
    /// produce follow-on noise, so callers use this to stay silent.
    pub fn contains_error(&self) -> bool {
        match self {
            Ty::Error => true,
            Ty::Optional(t)
            | Ty::List(t)
            | Ty::Buffer(t)
            | Ty::Slice(t)
            | Ty::Set(t)
            | Ty::Range(t)
            | Ty::Lazy(t)
            | Ty::Handle(t)
            | Ty::Future(t)
            | Ty::TaskJob(t)
            | Ty::Channel(t) => t.contains_error(),
            Ty::Any(_) => false,
            Ty::Result(a, b) | Ty::Map(a, b) => a.contains_error() || b.contains_error(),
            Ty::Array(elem, size) => elem.contains_error() || size.contains_error(),
            Ty::Simd(elem, _) => elem.contains_error(),
            Ty::Opaque { args, .. } => args.iter().any(|arg| arg.contains_error()),
            Ty::Tuple(ts) => ts.iter().any(|t| t.contains_error()),
            Ty::Func { params, ret } => {
                params.iter().any(|p| p.contains_error()) || ret.contains_error()
            }
            Ty::Named(_, args) => args.iter().any(|a| a.contains_error()),
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
            | (Buffer(a), Buffer(b))
            | (Slice(a), Slice(b))
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
            // An *applied* type parameter (`F[A]`) is encoded as a synthetic id
            // that carries no name, so there is nothing to look up. This shape
            // only arises from higher-kinded syntax, which is out of scope.
            Ty::Named(id, args) if id.is_synthetic_type_param() => {
                let inner = args
                    .iter()
                    .map(|a| a.display_in(def_map))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("<type parameter>[{inner}]")
            }
            Ty::Named(id, args) => {
                let Some(definition) = def_map.try_get(*id) else {
                    let inner = args
                        .iter()
                        .map(|a| a.display_in(def_map))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return if inner.is_empty() {
                        "<unresolved type>".to_string()
                    } else {
                        format!("<unresolved type>[{inner}]")
                    };
                };
                let name = definition.name.clone();
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
            Ty::Buffer(inner) => format!("buffer[{}]", inner.display_in(def_map)),
            Ty::Slice(inner) => format!("slice[{}]", inner.display_in(def_map)),
            Ty::Array(elem, size) => match &**size {
                Ty::ConstInt(_, n) => format!("array[{}, size: {}]", elem.display_in(def_map), n),
                other => format!(
                    "array[{}, size: {}]",
                    elem.display_in(def_map),
                    other.display_in(def_map)
                ),
            },
            Ty::Simd(elem, lanes) => format!("simd[{}, {}]", elem.display_in(def_map), lanes),
            Ty::Set(inner) => format!("set[{}]", inner.display_in(def_map)),
            Ty::Map(k, v) => format!("map[{}, {}]", k.display_in(def_map), v.display_in(def_map)),
            Ty::Range(inner) => format!("range[{}]", inner.display_in(def_map)),
            Ty::Lazy(inner) => format!("lazy[{}]", inner.display_in(def_map)),
            Ty::Handle(inner) => format!("handle[{}]", inner.display_in(def_map)),
            Ty::Future(inner) => format!("future[{}]", inner.display_in(def_map)),
            Ty::TaskJob(inner) => format!("task.Job[{}]", inner.display_in(def_map)),
            Ty::Channel(inner) => format!("channel.Channel[{}]", inner.display_in(def_map)),
            Ty::Tuple(items) => {
                let inner = items
                    .iter()
                    .map(|t| t.display_in(def_map))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tuple[{}]", inner)
            }
            Ty::Func { params, ret } => {
                let ps = params
                    .iter()
                    .map(|p| p.display_in(def_map))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("func({}) -> {}", ps, ret.display_in(def_map))
            }
            Ty::Opaque { kind, args } if !args.is_empty() => {
                let args = args
                    .iter()
                    .map(|arg| arg.display_in(def_map))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}[{}]", kind.display_name(), args)
            }
            // Remaining variants carry no nested type, so `display` is exact.
            _ => self.display(),
        }
    }

    /// Human-readable display name backed by a code-generation name table.
    ///
    /// Backends intentionally do not retain the resolver's full [`DefMap`].
    /// Their compact name table is sufficient to keep internal `DefId` values
    /// out of diagnostics while preserving declared names in nested types.
    pub fn display_with_names(&self, names: &HashMap<DefId, SmolStr>) -> std::string::String {
        match self {
            Ty::Named(id, args) => {
                let Some(name) = names.get(id) else {
                    return self.display();
                };
                if args.is_empty() {
                    name.to_string()
                } else {
                    let inner = args
                        .iter()
                        .map(|arg| arg.display_with_names(names))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{name}[{inner}]")
                }
            }
            Ty::Optional(inner) => format!("optional[{}]", inner.display_with_names(names)),
            Ty::Result(ok, err) => format!(
                "result[{}, {}]",
                ok.display_with_names(names),
                err.display_with_names(names)
            ),
            Ty::List(inner) => format!("list[{}]", inner.display_with_names(names)),
            Ty::Buffer(inner) => format!("buffer[{}]", inner.display_with_names(names)),
            Ty::Slice(inner) => format!("slice[{}]", inner.display_with_names(names)),
            Ty::Array(elem, size) => match &**size {
                Ty::ConstInt(_, value) => {
                    format!("array[{}, size: {value}]", elem.display_with_names(names))
                }
                other => format!(
                    "array[{}, size: {}]",
                    elem.display_with_names(names),
                    other.display_with_names(names)
                ),
            },
            Ty::Simd(elem, lanes) => {
                format!("simd[{}, {lanes}]", elem.display_with_names(names))
            }
            Ty::Set(inner) => format!("set[{}]", inner.display_with_names(names)),
            Ty::Map(key, value) => format!(
                "map[{}, {}]",
                key.display_with_names(names),
                value.display_with_names(names)
            ),
            Ty::Range(inner) => format!("range[{}]", inner.display_with_names(names)),
            Ty::Lazy(inner) => format!("lazy[{}]", inner.display_with_names(names)),
            Ty::Handle(inner) => format!("handle[{}]", inner.display_with_names(names)),
            Ty::Future(inner) => format!("future[{}]", inner.display_with_names(names)),
            Ty::TaskJob(inner) => format!("task.Job[{}]", inner.display_with_names(names)),
            Ty::Channel(inner) => format!("channel.Channel[{}]", inner.display_with_names(names)),
            Ty::Tuple(items) => {
                let inner = items
                    .iter()
                    .map(|item| item.display_with_names(names))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tuple[{inner}]")
            }
            Ty::Func { params, ret } => {
                let params = params
                    .iter()
                    .map(|param| param.display_with_names(names))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("func({params}) -> {}", ret.display_with_names(names))
            }
            Ty::Opaque { kind, args } if !args.is_empty() => {
                let args = args
                    .iter()
                    .map(|arg| arg.display_with_names(names))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}[{args}]", kind.display_name())
            }
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
            Ty::Buffer(t) => format!("buffer[{}]", t.display()),
            Ty::Slice(t) => format!("slice[{}]", t.display()),
            Ty::Array(elem, size) => match &**size {
                Ty::ConstInt(_, n) => format!("array[{}, size: {}]", elem.display(), n),
                other => format!("array[{}, size: {}]", elem.display(), other.display()),
            },
            Ty::Simd(elem, lanes) => format!("simd[{}, {}]", elem.display(), lanes),
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
            // Printed the way it is written: `size: 8`.
            Ty::ConstInt(name, value) => format!("{}: {}", name, value),
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
        Ty::Buffer(elem) => Ty::Buffer(Box::new(substitute_ty_params(elem, args))),
        Ty::Slice(elem) => Ty::Slice(Box::new(substitute_ty_params(elem, args))),
        // Substituting the length is the point: `array[byte, size: cap]` becomes
        // `array[byte, size: 8]` once `cap` is bound.
        Ty::Array(elem, size) => Ty::Array(
            Box::new(substitute_ty_params(elem, args)),
            Box::new(substitute_ty_params(size, args)),
        ),
        Ty::Simd(elem, lanes) => Ty::Simd(Box::new(substitute_ty_params(elem, args)), *lanes),
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

/// Replace the internal JSON `Value` placeholder with the concrete stdlib
/// definition in a resolved module.
///
/// Runtime signatures are declared once in the stdlib manifest, before the
/// source module is inserted into a project's definition map. They therefore
/// use [`DefId::SYNTHETIC_JSON_VALUE`] as a temporary marker. Keeping this
/// normalization here gives the checker and HIR one implementation and makes
/// missing `ori.json.Value` fail closed as `Ty::Error` instead of leaking a
/// synthetic identity into backend code.
pub fn replace_json_placeholder(ty: Ty, def_map: &DefMap) -> Ty {
    let json_value = def_map.lookup("ori.json.Value");

    fn recurse(ty: Ty, json_value: Option<DefId>) -> Ty {
        match ty {
            Ty::Named(id, _) if id == DefId::SYNTHETIC_JSON_VALUE => json_value
                .map(|resolved| Ty::Named(resolved, Vec::new()))
                .unwrap_or(Ty::Error),
            Ty::Named(id, args) => Ty::Named(
                id,
                args.into_iter()
                    .map(|arg| recurse(arg, json_value))
                    .collect(),
            ),
            Ty::Optional(inner) => Ty::Optional(Box::new(recurse(*inner, json_value))),
            Ty::Result(ok, err) => Ty::Result(
                Box::new(recurse(*ok, json_value)),
                Box::new(recurse(*err, json_value)),
            ),
            Ty::List(inner) => Ty::List(Box::new(recurse(*inner, json_value))),
            Ty::Buffer(inner) => Ty::Buffer(Box::new(recurse(*inner, json_value))),
            Ty::Slice(inner) => Ty::Slice(Box::new(recurse(*inner, json_value))),
            Ty::Array(inner, size) => Ty::Array(
                Box::new(recurse(*inner, json_value)),
                Box::new(recurse(*size, json_value)),
            ),
            Ty::Simd(inner, lanes) => Ty::Simd(Box::new(recurse(*inner, json_value)), lanes),
            Ty::Map(key, value) => Ty::Map(
                Box::new(recurse(*key, json_value)),
                Box::new(recurse(*value, json_value)),
            ),
            Ty::Set(inner) => Ty::Set(Box::new(recurse(*inner, json_value))),
            Ty::Range(inner) => Ty::Range(Box::new(recurse(*inner, json_value))),
            Ty::Lazy(inner) => Ty::Lazy(Box::new(recurse(*inner, json_value))),
            Ty::Handle(inner) => Ty::Handle(Box::new(recurse(*inner, json_value))),
            Ty::Future(inner) => Ty::Future(Box::new(recurse(*inner, json_value))),
            Ty::TaskJob(inner) => Ty::TaskJob(Box::new(recurse(*inner, json_value))),
            Ty::Channel(inner) => Ty::Channel(Box::new(recurse(*inner, json_value))),
            Ty::Opaque { kind, args } => Ty::Opaque {
                kind,
                args: args
                    .into_iter()
                    .map(|arg| recurse(arg, json_value))
                    .collect(),
            },
            Ty::Tuple(elements) => Ty::Tuple(
                elements
                    .into_iter()
                    .map(|element| recurse(element, json_value))
                    .collect(),
            ),
            Ty::Func { params, ret } => Ty::Func {
                params: params
                    .into_iter()
                    .map(|param| recurse(param, json_value))
                    .collect(),
                ret: Box::new(recurse(*ret, json_value)),
            },
            other => other,
        }
    }

    recurse(ty, json_value)
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
        Ty::Buffer(elem) => Ty::Buffer(Box::new(normalize_ty_aliases_depth(*elem, lookup, depth))),
        Ty::Array(elem, size) => Ty::Array(
            Box::new(normalize_ty_aliases_depth(*elem, lookup, depth)),
            Box::new(normalize_ty_aliases_depth(*size, lookup, depth)),
        ),
        Ty::Simd(elem, lanes) => Ty::Simd(
            Box::new(normalize_ty_aliases_depth(*elem, lookup, depth)),
            lanes,
        ),
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
        if def_map
            .try_get(id)
            .is_some_and(|definition| definition.kind == DefKind::TypeAlias)
        {
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
        if def_map
            .try_get(id)
            .is_some_and(|definition| definition.kind == DefKind::Newtype)
        {
            // Arity 0: newtypes take no type parameters today.
            newtype_map.get(&id).cloned().map(|repr| (0, repr))
        } else {
            None
        }
    })
}

/// Replace a trait's `Self` stand-in with the implementing type.
///
/// A trait signature carries `Self` as `Named(trait_def_id, [])`. Both the
/// checker (call sites and impl validation) and HIR lowering need to bind it,
/// which is why this lives here rather than inside the checker.
pub fn substitute_trait_self(ty: &Ty, trait_def_id: DefId, self_ty: &Ty) -> Ty {
    match ty {
        Ty::Named(id, args) if *id == trait_def_id && args.is_empty() => self_ty.clone(),
        Ty::Any(id) if *id == trait_def_id => self_ty.clone(),
        Ty::Named(id, args) => Ty::Named(
            *id,
            args.iter()
                .map(|arg| substitute_trait_self(arg, trait_def_id, self_ty))
                .collect(),
        ),
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(substitute_trait_self(ok, trait_def_id, self_ty)),
            Box::new(substitute_trait_self(err, trait_def_id, self_ty)),
        ),
        Ty::List(inner) => Ty::List(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Buffer(inner) => Ty::Buffer(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Map(key, value) => Ty::Map(
            Box::new(substitute_trait_self(key, trait_def_id, self_ty)),
            Box::new(substitute_trait_self(value, trait_def_id, self_ty)),
        ),
        Ty::Set(inner) => Ty::Set(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Range(inner) => Ty::Range(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Future(inner) => Ty::Future(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::TaskJob(inner) => Ty::TaskJob(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Channel(inner) => Ty::Channel(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Opaque { kind, args } => Ty::Opaque {
            kind: *kind,
            args: args
                .iter()
                .map(|arg| substitute_trait_self(arg, trait_def_id, self_ty))
                .collect(),
        },
        Ty::Tuple(items) => Ty::Tuple(
            items
                .iter()
                .map(|item| substitute_trait_self(item, trait_def_id, self_ty))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .iter()
                .map(|param| substitute_trait_self(param, trait_def_id, self_ty))
                .collect(),
            ret: Box::new(substitute_trait_self(ret, trait_def_id, self_ty)),
        },
        _ => ty.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{replace_json_placeholder, DefId, DefKind, DefMap, HashMap, SmolStr, Ty};
    use ori_diagnostics::{FileId, Span};

    #[test]
    fn codegen_type_display_uses_declared_names_recursively() {
        let user_id = DefId(7);
        let names = HashMap::from([(user_id, SmolStr::new("User"))]);
        let ty = Ty::Result(
            Box::new(Ty::List(Box::new(Ty::Named(user_id, Vec::new())))),
            Box::new(Ty::String),
        );

        assert_eq!(ty.display_with_names(&names), "result[list[User], string]");
    }

    #[test]
    fn display_in_recovers_from_synthetic_or_unknown_definition_ids() {
        let definitions = DefMap::default();
        assert_eq!(
            Ty::Named(DefId::synthetic_literal(7), Vec::new()).display_in(&definitions),
            "<unresolved type>"
        );
        assert_eq!(
            Ty::Named(DefId::synthetic_type_param(0), vec![Ty::String]).display_in(&definitions),
            "<type parameter>[string]"
        );
    }

    #[test]
    fn json_placeholder_normalization_is_shared_and_fail_closed() {
        let mut definitions = DefMap::default();
        let json_id = definitions.register(
            DefKind::Enum,
            SmolStr::new("Value"),
            SmolStr::new("ori.json.Value"),
            true,
            FileId(0),
            Span::new(0, 0),
        );
        let placeholder = Ty::Result(
            Box::new(Ty::List(Box::new(Ty::Named(
                DefId::SYNTHETIC_JSON_VALUE,
                Vec::new(),
            )))),
            Box::new(Ty::Optional(Box::new(Ty::Named(
                DefId::SYNTHETIC_JSON_VALUE,
                Vec::new(),
            )))),
        );

        assert_eq!(
            replace_json_placeholder(placeholder.clone(), &definitions),
            Ty::Result(
                Box::new(Ty::List(Box::new(Ty::Named(json_id, Vec::new())))),
                Box::new(Ty::Optional(Box::new(Ty::Named(json_id, Vec::new())))),
            )
        );
        assert_eq!(
            replace_json_placeholder(placeholder, &DefMap::default()),
            Ty::Result(
                Box::new(Ty::List(Box::new(Ty::Error))),
                Box::new(Ty::Optional(Box::new(Ty::Error))),
            )
        );
    }
}
