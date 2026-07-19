use crate::hir::*;
use ori_ast::common::{Attr, AttrArg, Visibility};
use ori_ast::expr::{Arg, BinaryOp, ClosureBody, ClosureExpr, Expr, FStrPart, UnaryOp};
use ori_ast::item::{Item, SourceFile};
use ori_ast::stmt::{Block, MatchCase, Stmt};
use ori_diagnostics::{DiagnosticSink, FileId, Span};
use ori_types::literal::{parse_float_literal, parse_int_literal};
use ori_types::{
    expand_ty_aliases, DefId, DefKind, DefMap, EnumSig, FuncSig, ImplSig, OpaqueTy, ReExport,
    StructSig, TraitSig, Ty, TypeAliasSig, ValueSig,
};
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};

/// Maps an Ori stdlib qualified path to the C function name used at link time.
///
/// This is a thin wrapper over `ori_types::stdlib::stdlib_runtime_symbol`,
/// which is the single source of truth for stdlib path -> runtime symbol
/// resolution. The runtime manifest (`STDLIB_RUNTIME_FUNCTIONS`) holds every
/// canonical path and alias; do not add parallel match tables here.
fn stdlib_c_name(ori_path: &str) -> Option<&'static str> {
    ori_types::stdlib::stdlib_runtime_symbol(ori_path)
}

fn string_conversion_c_name(ty: &Ty) -> Option<&'static str> {
    if ty.is_integer() || ty.contains_infer() {
        Some("ori_to_string")
    } else if ty.is_float() {
        Some("ori_float_to_string")
    } else if matches!(ty, Ty::Bool) {
        Some("ori_bool_to_string")
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemIntrinsic {
    SizeOf,
    AlignOf,
}

fn mem_intrinsic(path: &str) -> Option<MemIntrinsic> {
    match path {
        "ori.mem.size_of" => Some(MemIntrinsic::SizeOf),
        "ori.mem.align_of" => Some(MemIntrinsic::AlignOf),
        _ => None,
    }
}

fn ori_mem_size_of_ty(ty: &Ty) -> i64 {
    match ty {
        Ty::Bool | Ty::Int8 | Ty::U8 => 1,
        Ty::Int16 | Ty::U16 => 2,
        Ty::Int32 | Ty::U32 | Ty::Float32 => 4,
        Ty::Int | Ty::Int64 | Ty::U64 | Ty::Float | Ty::Float64 => 8,
        Ty::Void | Ty::Never => 0,
        Ty::Error => 0,
        Ty::Infer(_) | Ty::Param { .. } => 8,
        Ty::String
        | Ty::Bytes
        | Ty::Optional(_)
        | Ty::Result(_, _)
        | Ty::List(_)
        | Ty::Map(_, _)
        | Ty::Set(_)
        | Ty::Range(_)
        | Ty::Lazy(_)
        | Ty::Handle(_)
        | Ty::Future(_)
        | Ty::TaskJob(_)
        | Ty::Channel(_)
        | Ty::AtomicInt
        | Ty::TaskJoinError
        | Ty::ChannelSendError
        | Ty::ChannelReceiveError
        | Ty::Opaque { .. }
        | Ty::Any(_)
        | Ty::Tuple(_)
        | Ty::Func { .. }
        | Ty::Named(_, _) => 8,
    }
}

fn ori_mem_align_of_ty(ty: &Ty) -> i64 {
    match ori_mem_size_of_ty(ty) {
        0 | 1 => 1,
        2 => 2,
        4 => 4,
        _ => 8,
    }
}

fn expr_to_qualified_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(ident) => Some(ident.text.to_string()),
        Expr::QualifiedIdent(q) => Some(q.to_string()),
        Expr::Field { object, field, .. } => {
            let prefix = expr_to_qualified_path(object)?;
            Some(format!("{}.{}", prefix, field.text))
        }
        _ => None,
    }
}

fn replace_json_placeholder_in_ty(ty: Ty, def_map: &DefMap) -> Ty {
    let json_val_def_id = def_map.lookup("ori.json.Value");
    fn recurse(t: Ty, json_val_def_id: Option<DefId>) -> Ty {
        match t {
            Ty::Named(id, _) if id == ori_types::stdlib::JSON_VALUE_PLACEHOLDER => {
                if let Some(concrete_id) = json_val_def_id {
                    Ty::Named(concrete_id, vec![])
                } else {
                    Ty::Named(id, vec![])
                }
            }
            Ty::Named(id, args) => {
                let new_args = args
                    .into_iter()
                    .map(|arg| recurse(arg, json_val_def_id))
                    .collect();
                Ty::Named(id, new_args)
            }
            Ty::Optional(inner) => Ty::Optional(Box::new(recurse(*inner, json_val_def_id))),
            Ty::Result(ok, err) => Ty::Result(
                Box::new(recurse(*ok, json_val_def_id)),
                Box::new(recurse(*err, json_val_def_id)),
            ),
            Ty::List(inner) => Ty::List(Box::new(recurse(*inner, json_val_def_id))),
            Ty::Map(k, v) => Ty::Map(
                Box::new(recurse(*k, json_val_def_id)),
                Box::new(recurse(*v, json_val_def_id)),
            ),
            Ty::Set(inner) => Ty::Set(Box::new(recurse(*inner, json_val_def_id))),
            Ty::Range(inner) => Ty::Range(Box::new(recurse(*inner, json_val_def_id))),
            Ty::Tuple(elems) => Ty::Tuple(
                elems
                    .into_iter()
                    .map(|e| recurse(e, json_val_def_id))
                    .collect(),
            ),
            Ty::Func { params, ret } => Ty::Func {
                params: params
                    .into_iter()
                    .map(|p| recurse(p, json_val_def_id))
                    .collect(),
                ret: Box::new(recurse(*ret, json_val_def_id)),
            },
            _ => t,
        }
    }
    recurse(ty, json_val_def_id)
}

fn stdlib_c_func_ty(c_name: &str, def_map: Option<&DefMap>) -> Ty {
    let raw = stdlib_c_func_ty_raw(c_name);
    if let Some(def_map) = def_map {
        replace_json_placeholder_in_ty(raw, def_map)
    } else {
        raw
    }
}

fn stdlib_c_func_ty_raw(c_name: &str) -> Ty {
    if let Some(entry) = ori_types::stdlib::stdlib_runtime_functions()
        .iter()
        .find(|entry| entry.runtime_symbol == c_name)
    {
        if let Some((params, ret)) = ori_types::stdlib::stdlib_func_sig(entry.canonical_path) {
            return Ty::Func {
                params,
                ret: Box::new(ret),
            };
        }
    }
    let (params, ret) = match c_name {
        "ori_io_print" | "ori_io_eprint" => (vec![Ty::String], Ty::Void),
        "ori_io_read_line" => (vec![], Ty::Optional(Box::new(Ty::String))),
        "ori_string_len" | "ori_len" => (vec![Ty::String], Ty::Int),
        "ori_string_concat" => (vec![Ty::String, Ty::String], Ty::String),
        "ori_string_split" => (vec![Ty::String, Ty::String], Ty::List(Box::new(Ty::String))),
        "ori_string_slice" => (vec![Ty::String, Ty::Int, Ty::Int], Ty::String),
        "ori_string_contains" | "ori_string_starts_with" | "ori_string_ends_with" => {
            (vec![Ty::String, Ty::String], Ty::Bool)
        }
        "ori_string_trim"
        | "ori_string_trim_start"
        | "ori_string_trim_end"
        | "ori_string_to_upper"
        | "ori_string_to_lower" => (vec![Ty::String], Ty::String),
        "ori_string_replace" => (vec![Ty::String, Ty::String, Ty::String], Ty::String),
        "ori_string_chars" => (vec![Ty::String], Ty::List(Box::new(Ty::String))),
        "ori_to_string" => (vec![Ty::Int], Ty::String),
        "ori_to_int" => (vec![Ty::Int], Ty::Int),
        "ori_to_float" => (vec![Ty::Int], Ty::Float),
        "ori_list_new" => (vec![], Ty::List(Box::new(Ty::Infer(0)))),
        "ori_list_with_capacity" => (vec![Ty::Int], Ty::List(Box::new(Ty::Infer(0)))),
        "ori_list_push" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori_list_get" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int],
            Ty::Infer(0),
        ),
        "ori_list_set" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Infer(0)],
            Ty::Void,
        ),
        "ori_list_len" | "ori_list_capacity" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori_list_reserve" => (vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int], Ty::Void),
        "ori_list_free" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori_set_new" => (vec![], Ty::Set(Box::new(Ty::Infer(0)))),
        "ori_set_add" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori_set_contains" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Bool,
        ),
        "ori_set_len" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori_set_free" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori_map_new" => (
            vec![],
            Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
        ),
        "ori_map_set" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
                Ty::Infer(0),
                Ty::Infer(0),
            ],
            Ty::Void,
        ),
        "ori_map_get" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
                Ty::Infer(0),
            ],
            Ty::Infer(0),
        ),
        "ori_map_contains" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
                Ty::Infer(0),
            ],
            Ty::Bool,
        ),
        "ori_map_len" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0)))],
            Ty::Int,
        ),
        "ori_map_free" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0)))],
            Ty::Void,
        ),
        "ori_math_sqrt" => (vec![Ty::Float], Ty::Float),
        "ori_math_abs" => (vec![Ty::Int], Ty::Int),
        "ori_math_abs_float" => (vec![Ty::Float], Ty::Float),
        "ori_math_min" | "ori_math_max" => (vec![Ty::Int, Ty::Int], Ty::Int),
        "ori_math_min_float" | "ori_math_max_float" => (vec![Ty::Float, Ty::Float], Ty::Float),
        "ori_math_clamp" => (vec![Ty::Int, Ty::Int, Ty::Int], Ty::Int),
        "ori_math_pow" => (vec![Ty::Float, Ty::Float], Ty::Float),
        "ori_math_floor" | "ori_math_ceil" | "ori_math_round" => (vec![Ty::Float], Ty::Int),
        "ori_math_log" | "ori_math_log2" | "ori_math_sin" | "ori_math_cos" | "ori_math_tan" => {
            (vec![Ty::Float], Ty::Float)
        }
        "ori_math_is_nan" | "ori_math_is_infinite" => (vec![Ty::Float], Ty::Bool),
        "ori_time_now" => (vec![], Ty::Int),
        "ori_time_sleep" => (vec![Ty::Int], Ty::Void),
        "ori_time_duration_ms" => (vec![Ty::Int, Ty::Int], Ty::Int),
        "ori_format_number" | "ori_format_percent" => (vec![Ty::Float, Ty::Int], Ty::String),
        "ori_format_hex" | "ori_format_binary" => (vec![Ty::Int], Ty::String),
        "ori_format_date" => (vec![Ty::Int, Ty::String], Ty::String),
        "ori_format_datetime" => (vec![Ty::Int, Ty::String, Ty::String], Ty::String),
        "ori_format_bytes_size" => (vec![Ty::Int, Ty::String], Ty::String),
        "ori_files_read_bytes" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori_files_write_bytes" => (
            vec![Ty::String, Ty::Bytes],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori_files_read_all" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori_os_args" => (vec![], Ty::List(Box::new(Ty::String))),
        "ori_os_env" => (vec![Ty::String], Ty::Optional(Box::new(Ty::String))),
        "ori_os_exit" => (vec![Ty::Int], Ty::Void),
        "ori_os_pid" => (vec![], Ty::Int),
        "ori_os_platform" | "ori_os_arch" => (vec![], Ty::String),
        "ori_random_int" => (vec![Ty::Int, Ty::Int], Ty::Int),
        "ori_random_float" => (vec![Ty::Float, Ty::Float], Ty::Float),
        "ori_random_bool" => (vec![], Ty::Bool),
        "ori_test_assert" => (vec![Ty::Bool, Ty::String], Ty::Void),
        "ori_test_fail" => (vec![Ty::String], Ty::Never),
        "ori_panic" => (vec![Ty::String], Ty::Never),
        "ori_list_pop" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Infer(0)),
        "ori_list_remove" => (vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int], Ty::Void),
        "ori_list_insert" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Infer(0)],
            Ty::Void,
        ),
        "ori_list_contains" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Bool,
        ),
        "ori_list_index_of" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Int,
        ),
        "ori_list_sort" | "ori_list_reverse" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori_list_slice" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Int],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori_map_remove" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
                Ty::Infer(0),
            ],
            Ty::Void,
        ),
        "ori_map_keys" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori_map_values" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori_map_entries" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Tuple(vec![Ty::Infer(0), Ty::Infer(0)]))),
        ),
        "ori_set_remove" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori_set_union" | "ori_set_intersection" | "ori_set_difference" => (
            vec![
                Ty::Set(Box::new(Ty::Infer(0))),
                Ty::Set(Box::new(Ty::Infer(0))),
            ],
            Ty::Set(Box::new(Ty::Infer(0))),
        ),
        "ori_list_map" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Infer(1)),
                },
            ],
            Ty::List(Box::new(Ty::Infer(1))),
        ),
        "ori_list_filter" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori_string_index_of" => (vec![Ty::String, Ty::String], Ty::Int),
        "ori_string_join" => (vec![Ty::List(Box::new(Ty::String)), Ty::String], Ty::String),
        "ori_string_repeat" => (vec![Ty::String, Ty::Int], Ty::String),
        "ori_string_pad_left" | "ori_string_pad_right" => {
            (vec![Ty::String, Ty::Int, Ty::String], Ty::String)
        }
        "ori_string_parse_int" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori_string_parse_float" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Float), Box::new(Ty::String)),
        ),
        "ori_string_to_bytes" => (vec![Ty::String], Ty::Bytes),
        "ori_string_from_bytes" => (
            vec![Ty::Bytes],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori_bytes_len" => (vec![Ty::Bytes], Ty::Int),
        "ori_bytes_concat" => (vec![Ty::Bytes, Ty::Bytes], Ty::Bytes),
        "ori_bytes_slice" => (vec![Ty::Bytes, Ty::Int, Ty::Int], Ty::Bytes),
        "ori_bytes_to_hex" => (vec![Ty::Bytes], Ty::String),
        "ori_bytes_from_hex" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori_bytes_decode_utf8" => (
            vec![Ty::Bytes],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori_bytes_get" => (vec![Ty::Bytes, Ty::Int], Ty::U8),
        "ori_float_to_string" => (vec![Ty::Float], Ty::String),
        "ori_bool_to_string" => (vec![Ty::Bool], Ty::String),
        "ori_string_to_int" => (vec![Ty::String], Ty::Optional(Box::new(Ty::Int))),
        "ori_string_to_float" => (vec![Ty::String], Ty::Optional(Box::new(Ty::Float))),
        "ori_files_read_text" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori_files_read_text_async" => (
            vec![Ty::String],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::String),
                Box::new(Ty::String),
            ))),
        ),
        "ori_files_write_text" => (
            vec![Ty::String, Ty::String],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori_files_write_text_async" => (
            vec![Ty::String, Ty::String],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::String),
                Box::new(Ty::String),
            ))),
        ),
        "ori_net_connect_async" | "ori_net_connect_tls_async" => (
            vec![Ty::String, Ty::Int, Ty::Int],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Infer(0)),
                Box::new(Ty::String),
            ))),
        ),
        "ori_net_accept_async" => (
            vec![Ty::Infer(0)],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Infer(0)),
                Box::new(Ty::String),
            ))),
        ),
        "ori_net_read_some_async" => (
            vec![Ty::Infer(0), Ty::Int],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Bytes),
                Box::new(Ty::String),
            ))),
        ),
        "ori_net_write_all_async" => (
            vec![Ty::Infer(0), Ty::Bytes],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Int),
                Box::new(Ty::String),
            ))),
        ),
        "ori_net_udp_recv_from_async" => (
            vec![Ty::Infer(0), Ty::Int],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Bytes),
                Box::new(Ty::String),
            ))),
        ),
        "ori_net_udp_send_to_async" => (
            vec![Ty::Infer(0), Ty::String, Ty::Int, Ty::Bytes],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Int),
                Box::new(Ty::String),
            ))),
        ),
        "ori_files_append_text" => (
            vec![Ty::String, Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori_files_exists" | "ori_files_is_file" | "ori_files_is_dir" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Bool), Box::new(Ty::String)),
        ),
        "ori_files_delete" | "ori_files_create_dir" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori_files_list_dir" => (
            vec![Ty::String],
            Ty::Result(
                Box::new(Ty::List(Box::new(Ty::String))),
                Box::new(Ty::String),
            ),
        ),
        "ori_files_copy" | "ori_files_rename" => (
            vec![Ty::String, Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        _ => return Ty::Infer(0),
    };
    Ty::Func {
        params,
        ret: Box::new(ret),
    }
}

fn specialized_stdlib_call_ret_ty(c_name: &str, args: &[HirArg], fallback: Ty) -> Ty {
    let canonical = ori_types::stdlib::stdlib_runtime_functions()
        .iter()
        .find(|entry| entry.runtime_symbol == c_name)
        .map(|entry| entry.canonical_path)
        .unwrap_or(c_name);
    let list_elem = |arg: &HirArg| match &arg.value.ty {
        Ty::List(elem) => Some(*elem.clone()),
        ty => ty.list_backed_collection_elem().cloned(),
    };
    let closure = |arg: &HirArg| match &arg.value.ty {
        Ty::Func { params, ret } => Some((params.clone(), *ret.clone())),
        _ => None,
    };
    match canonical {
        "ori.list.get" | "ori.list.pop" => args.first().and_then(list_elem).unwrap_or(fallback),
        "ori.list.try_get" | "ori.list.try_pop" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::Optional(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.list.slice" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::List(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.list.clone" | "ori.list.to_list" | "ori.list.from_list" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::List(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.deque.pop_front"
        | "ori.deque.pop_back"
        | "ori.deque.front"
        | "ori.deque.back"
        | "ori.queue.dequeue"
        | "ori.queue.peek"
        | "ori.stack.pop"
        | "ori.stack.peek"
        | "ori.linked_list.pop_front"
        | "ori.linked_list.front"
        | "ori.doubly_linked_list.pop_front"
        | "ori.doubly_linked_list.pop_back"
        | "ori.doubly_linked_list.front"
        | "ori.doubly_linked_list.back" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::Optional(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.linked_list.value_at"
        | "ori.linked_list.remove_at"
        | "ori.doubly_linked_list.value_at"
        | "ori.doubly_linked_list.remove_at" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::Optional(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.linked_list.cursor_front"
        | "ori.linked_list.cursor_back"
        | "ori.linked_list.find"
        | "ori.doubly_linked_list.cursor_front"
        | "ori.doubly_linked_list.cursor_back"
        | "ori.doubly_linked_list.find" => Ty::Optional(Box::new(Ty::Int)),
        "ori.deque.to_list"
        | "ori.queue.to_list"
        | "ori.stack.to_list"
        | "ori.linked_list.to_list"
        | "ori.doubly_linked_list.to_list" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::List(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.deque.clone"
        | "ori.queue.clone"
        | "ori.stack.clone"
        | "ori.linked_list.clone"
        | "ori.doubly_linked_list.clone" => args
            .first()
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.tree.value" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Tree,
                    args,
                } => args.first().cloned(),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.tree.try_value" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Tree,
                    args,
                } => args
                    .first()
                    .cloned()
                    .map(|elem| Ty::Optional(Box::new(elem))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.tree.children"
        | "ori.tree.pre_order"
        | "ori.tree.post_order"
        | "ori.tree.breadth_first" => Ty::List(Box::new(Ty::Opaque {
            kind: OpaqueTy::NodeId,
            args: vec![],
        })),
        "ori.tree.parent" | "ori.tree.find" => Ty::Optional(Box::new(Ty::Opaque {
            kind: OpaqueTy::NodeId,
            args: vec![],
        })),
        "ori.tree.clone" | "ori.tree.clone_subtree" => args
            .first()
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.list.map" => args
            .get(1)
            .and_then(closure)
            .map(|(_, ret)| Ty::List(Box::new(ret)))
            .unwrap_or(fallback),
        "ori.list.filter" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::List(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.map.get" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Map(_, value) => Some(*value.clone()),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.map.try_get" | "ori.map.try_remove" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Map(_, value) => Some(Ty::Optional(value.clone())),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.map.clone" => args
            .first()
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.map.keys" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Map(key, _) => Some(Ty::List(Box::new(*key.clone()))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.map.values" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Map(_, value) => Some(Ty::List(Box::new(*value.clone()))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.map.entries" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Map(key, value) => Some(Ty::List(Box::new(Ty::Tuple(vec![
                    *key.clone(),
                    *value.clone(),
                ])))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.hash_table.get" | "ori.hash_table.remove" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::HashTable,
                    args,
                } if args.len() == 2 => Some(Ty::Optional(Box::new(args[1].clone()))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.hash_table.clone" => args
            .first()
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.hash_table.keys" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::HashTable,
                    args,
                } if args.len() == 2 => Some(Ty::List(Box::new(args[0].clone()))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.hash_table.values" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::HashTable,
                    args,
                } if args.len() == 2 => Some(Ty::List(Box::new(args[1].clone()))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.hash_table.entries" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::HashTable,
                    args,
                } if args.len() == 2 => Some(Ty::List(Box::new(Ty::Tuple(vec![
                    args[0].clone(),
                    args[1].clone(),
                ])))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.graph.neighbors"
        | "ori.graph.nodes"
        | "ori.graph.bfs"
        | "ori.graph.dfs"
        | "ori.graph.topological_sort" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Graph,
                    args,
                } => args.first().cloned().map(|elem| Ty::List(Box::new(elem))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.graph.try_topological_sort"
        | "ori.graph.shortest_path"
        | "ori.graph.shortest_weighted_path" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Graph,
                    args,
                } => args
                    .first()
                    .cloned()
                    .map(|elem| Ty::Optional(Box::new(Ty::List(Box::new(elem))))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.graph.edge_weight" => Ty::Optional(Box::new(Ty::Int)),
        "ori.graph.components" | "ori.graph.strongly_connected_components" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Graph,
                    args,
                } => args
                    .first()
                    .cloned()
                    .map(|elem| Ty::List(Box::new(Ty::List(Box::new(elem))))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.graph.transitive_closure" | "ori.graph.clone" => args
            .first()
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.graph.edges" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Graph,
                    args,
                } => args
                    .first()
                    .cloned()
                    .map(|elem| Ty::List(Box::new(Ty::Tuple(vec![elem.clone(), elem])))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.heap.pop" | "ori.heap.peek" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Heap,
                    args,
                } => args
                    .first()
                    .cloned()
                    .map(|elem| Ty::Optional(Box::new(elem))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.heap.clone" | "ori.heap.merge" => args
            .first()
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.heap.to_list" | "ori.heap.into_sorted_list" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Opaque {
                    kind: OpaqueTy::Heap,
                    args,
                } => args.first().cloned().map(|elem| Ty::List(Box::new(elem))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.heap.from_list" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::List(elem) => Some(Ty::Opaque {
                    kind: OpaqueTy::Heap,
                    args: vec![*elem.clone()],
                }),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.random.choice" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::Optional(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.random.shuffle" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::List(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.task.join" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::TaskJob(inner) => Some(Ty::Result(inner.clone(), Box::new(Ty::TaskJoinError))),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.task.block_on" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Future(inner) => Some(*inner.clone()),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.channel.receive" => args
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Channel(inner) => {
                    Some(Ty::Result(inner.clone(), Box::new(Ty::ChannelReceiveError)))
                }
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.iter.map" => args
            .get(1)
            .and_then(closure)
            .map(|(_, ret)| Ty::List(Box::new(ret)))
            .unwrap_or(fallback),
        "ori.iter.filter" | "ori.iter.take" | "ori.iter.skip" | "ori.iter.reverse"
        | "ori.iter.sort" | "ori.iter.sort_by" | "ori.iter.unique" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::List(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.iter.any" | "ori.iter.all" => Ty::Bool,
        "ori.iter.count_where" => Ty::Int,
        "ori.iter.reduce" => args
            .get(1)
            .map(|arg| arg.value.ty.clone())
            .unwrap_or(fallback),
        "ori.iter.find" => args
            .first()
            .and_then(list_elem)
            .map(|elem| Ty::Optional(Box::new(elem)))
            .unwrap_or(fallback),
        "ori.iter.flat_map" => args
            .get(1)
            .and_then(closure)
            .and_then(|(_, ret)| match ret {
                Ty::List(inner) => Some(Ty::List(inner)),
                _ => None,
            })
            .unwrap_or(fallback),
        "ori.iter.zip" => match (
            args.first().and_then(list_elem),
            args.get(1).and_then(list_elem),
        ) {
            (Some(left), Some(right)) => Ty::List(Box::new(Ty::Tuple(vec![left, right]))),
            _ => fallback,
        },
        "ori.iter.partition" => args
            .first()
            .and_then(list_elem)
            .map(|elem| {
                Ty::Tuple(vec![
                    Ty::List(Box::new(elem.clone())),
                    Ty::List(Box::new(elem)),
                ])
            })
            .unwrap_or(fallback),
        "ori.iter.group_by" => match (
            args.first().and_then(list_elem),
            args.get(1).and_then(closure),
        ) {
            (Some(elem), Some((_, key))) => {
                Ty::Map(Box::new(key), Box::new(Ty::List(Box::new(elem))))
            }
            _ => fallback,
        },
        "ori.iter.flatten" => args
            .first()
            .and_then(list_elem)
            .and_then(|elem| match elem {
                Ty::List(inner) => Some(Ty::List(inner)),
                _ => None,
            })
            .unwrap_or(fallback),
        _ => fallback,
    }
}

// ── Scope stack ───────────────────────────────────────────────────────────────

#[derive(Default)]
struct Scope {
    vars: HashMap<SmolStr, Ty>,
}

struct Lowerer<'a> {
    def_map: &'a DefMap,
    func_sigs: &'a [FuncSig],
    value_sigs: &'a [ValueSig],
    struct_sigs: &'a [StructSig],
    enum_sigs: &'a [EnumSig],
    trait_sigs: &'a [TraitSig],
    impl_sigs: &'a [ImplSig],
    namespace: &'a str,
    file_id: FileId,
    sink: &'a mut DiagnosticSink,
    scopes: Vec<Scope>,
    /// `import ori.io = io` → `io` maps to `ori.io`.
    aliases: HashMap<SmolStr, SmolStr>,
    /// Current function's return type (for `try` propagation lowering).
    ret_ty: Ty,
    /// Inner return type for async functions. `None` means the current function
    /// returns synchronously.
    async_inner_ret_ty: Option<Ty>,
    closure_counter: usize,
    generated_funcs: Vec<HirFunc>,
    /// `DefId` → `(arity, underlying_ty)` for each `type alias` declaration.
    type_alias_map: HashMap<DefId, (usize, Ty)>,
    local_type_aliases: HashMap<SmolStr, ori_ast::ty::Type>,
}

impl<'a> Lowerer<'a> {
    fn new(
        def_map: &'a DefMap,
        func_sigs: &'a [FuncSig],
        value_sigs: &'a [ValueSig],
        struct_sigs: &'a [StructSig],
        enum_sigs: &'a [EnumSig],
        trait_sigs: &'a [TraitSig],
        impl_sigs: &'a [ImplSig],
        type_alias_sigs: &[TypeAliasSig],
        namespace: &'a str,
        file_id: FileId,
        sink: &'a mut DiagnosticSink,
    ) -> Self {
        let type_alias_map: HashMap<DefId, (usize, Ty)> = type_alias_sigs
            .iter()
            .map(|s| (s.def_id, (s.type_params.len(), s.ty.clone())))
            .collect();
        Self {
            def_map,
            func_sigs,
            value_sigs,
            struct_sigs,
            enum_sigs,
            trait_sigs,
            impl_sigs,
            namespace,
            file_id,
            sink,
            scopes: vec![Scope::default()],
            aliases: HashMap::new(),
            ret_ty: Ty::Void,
            async_inner_ret_ty: None,
            closure_counter: 0,
            generated_funcs: Vec::new(),
            type_alias_map,
            local_type_aliases: HashMap::new(),
        }
    }

    /// Resolve `io.print` → `ori.io.print` using the import alias map,
    /// then look up in the stdlib table.
    fn resolve_stdlib(&self, name: &str) -> Option<&'static str> {
        // Direct hit (e.g., builtin `string`, `len`, `int`)
        if let Some(c) = stdlib_c_name(name) {
            return Some(c);
        }
        // Qualified via alias: `io.print` where `io → ori.io`
        let expanded = self.expand_alias(name);
        if expanded != name {
            return stdlib_c_name(&expanded);
        }
        None
    }

    fn resolve_math_overload(&self, name: &str) -> Option<&'static str> {
        let expanded = self.expand_alias(name);
        match expanded.as_str() {
            "ori.math.abs" => Some("ori.math.abs"),
            "ori.math.min" => Some("ori.math.min"),
            "ori.math.max" => Some("ori.math.max"),
            _ => None,
        }
    }

    fn struct_field_ty(&self, def_id: DefId, field: &str) -> Option<Ty> {
        self.struct_sigs
            .iter()
            .find(|sig| sig.def_id == def_id)
            .and_then(|sig| sig.fields.iter().find(|(name, _)| name == field))
            .map(|(_, ty)| ty.clone())
    }

    fn lower_math_overload_call(
        &mut self,
        path: &str,
        args: &[Arg],
        tp: &[SmolStr],
        span: Span,
    ) -> HirExpr {
        let args_h = self.lower_call_args(args, tp);
        let use_float = args_h.iter().any(|arg| arg.value.ty.is_float());
        let symbol = match (path, use_float) {
            ("ori.math.abs", true) => "ori_math_abs_float",
            ("ori.math.min", true) => "ori_math_min_float",
            ("ori.math.max", true) => "ori_math_max_float",
            ("ori.math.abs", false) => "ori_math_abs",
            ("ori.math.min", false) => "ori_math_min",
            ("ori.math.max", false) => "ori_math_max",
            _ => "ori_math_abs",
        };
        let sig_ty = stdlib_c_func_ty(symbol, Some(self.def_map));
        let ret_ty = match &sig_ty {
            Ty::Func { ret, .. } => *ret.clone(),
            _ => Ty::Infer(0),
        };
        HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(HirExpr {
                    kind: HirExprKind::Var(SmolStr::new(symbol)),
                    ty: sig_ty,
                    span,
                }),
                args: args_h,
            },
            ty: ret_ty,
            span,
        }
    }

    fn lower_lazy_once_call(&mut self, args: &[Arg], tp: &[SmolStr], span: Span) -> HirExpr {
        let args_h = self.lower_call_args(args, tp);
        let inner_ty = args_h
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Func { ret, .. } => Some(*ret.clone()),
                _ => None,
            })
            .unwrap_or(Ty::Infer(0));
        let lazy_ty = Ty::Lazy(Box::new(inner_ty.clone()));
        let sig_ty = Ty::Func {
            params: vec![Ty::Func {
                params: vec![],
                ret: Box::new(inner_ty),
            }],
            ret: Box::new(lazy_ty.clone()),
        };
        HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(HirExpr {
                    kind: HirExprKind::Var(SmolStr::new("ori_lazy_once")),
                    ty: sig_ty,
                    span,
                }),
                args: args_h,
            },
            ty: lazy_ty,
            span,
        }
    }

    fn lower_lazy_force_call(&mut self, args: &[Arg], tp: &[SmolStr], span: Span) -> HirExpr {
        let args_h = self.lower_call_args(args, tp);
        let inner_ty = args_h
            .first()
            .and_then(|arg| match &arg.value.ty {
                Ty::Lazy(inner) => Some(*inner.clone()),
                _ => None,
            })
            .unwrap_or(Ty::Infer(0));
        let sig_ty = Ty::Func {
            params: vec![Ty::Lazy(Box::new(inner_ty.clone()))],
            ret: Box::new(inner_ty.clone()),
        };
        HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(HirExpr {
                    kind: HirExprKind::Var(SmolStr::new("ori_lazy_force")),
                    ty: sig_ty,
                    span,
                }),
                args: args_h,
            },
            ty: inner_ty,
            span,
        }
    }

    fn push(&mut self) {
        self.scopes.push(Scope::default());
    }
    fn pop(&mut self) {
        self.scopes.pop();
    }
    fn bind(&mut self, name: SmolStr, ty: Ty) {
        if let Some(s) = self.scopes.last_mut() {
            s.vars.insert(name, ty);
        }
    }
    fn lookup_var(&self, name: &str) -> Option<Ty> {
        for s in self.scopes.iter().rev() {
            if let Some(t) = s.vars.get(name) {
                return Some(t.clone());
            }
        }
        None
    }
    fn lookup(&self, name: &str) -> Ty {
        self.lookup_var(name).unwrap_or(Ty::Error)
    }
    fn next_closure_name(&mut self) -> SmolStr {
        let index = self.closure_counter;
        self.closure_counter += 1;
        SmolStr::new(format!("{}.__closure_{}", self.namespace, index))
    }
    fn expand_alias(&self, name: &str) -> SmolStr {
        let mut prefix_end = name.len();
        loop {
            let prefix = &name[..prefix_end];
            if let Some(full_ns) = self.aliases.get(prefix) {
                let suffix = &name[prefix_end..];
                if suffix.is_empty() {
                    return SmolStr::new(full_ns.to_string());
                }
                return SmolStr::new(format!("{}{}", full_ns, suffix));
            }
            if let Some(dot) = name[..prefix_end].rfind('.') {
                prefix_end = dot;
            } else {
                break;
            }
        }
        SmolStr::new(name)
    }
    fn resolve_def_path(&self, name: &str) -> Option<SmolStr> {
        let expanded = self.expand_alias(name);
        if self.def_map.lookup(&expanded).is_some() {
            return Some(expanded);
        }
        let local = SmolStr::new(format!("{}.{}", self.namespace, expanded));
        if self.def_map.lookup(&local).is_some() {
            return Some(local);
        }
        None
    }
    fn resolve_def_id_with_kind(&self, name: &str, kind: DefKind) -> Option<ori_types::DefId> {
        let path = self.resolve_def_path(name)?;
        let id = self.def_map.lookup(&path)?;
        if self.def_map.get(id).kind == kind {
            Some(id)
        } else {
            None
        }
    }
    fn resolve_enum_variant(
        &self,
        q: &ori_ast::common::QualifiedName,
    ) -> Option<(ori_types::DefId, SmolStr)> {
        let enum_path = qualified_prefix(q)?;
        let id = self.resolve_def_id_with_kind(&enum_path, DefKind::Enum)?;
        Some((id, q.last().text.clone()))
    }
    fn ty_for_def_path(&self, path: &str) -> Ty {
        if let Some(id) = self.def_map.lookup(path) {
            match self.def_map.get(id).kind {
                DefKind::Struct | DefKind::Enum | DefKind::TypeAlias => Ty::Named(id, Vec::new()),
                DefKind::Func | DefKind::Extern => {
                    if let Some(sig) = self.func_sigs.iter().find(|sig| sig.def_id == id) {
                        Ty::Func {
                            params: sig.params.clone(),
                            ret: Box::new(sig.return_ty.clone()),
                        }
                    } else {
                        Ty::Infer(0)
                    }
                }
                DefKind::Const | DefKind::Var => self
                    .value_sigs
                    .iter()
                    .find(|sig| sig.def_id == id)
                    .map(|sig| sig.ty.clone())
                    .unwrap_or(Ty::Infer(0)),
                _ => Ty::Infer(0),
            }
        } else {
            Ty::Error
        }
    }
    fn trait_method_return_ty(&self, trait_def_id: ori_types::DefId, method: &str) -> Option<Ty> {
        self.trait_sigs
            .iter()
            .find(|sig| sig.def_id == trait_def_id)?
            .methods
            .iter()
            .find(|sig| sig.name == method)
            .map(|sig| sig.return_ty.clone())
    }
    fn trait_method_func_for_type(
        &self,
        type_def_id: ori_types::DefId,
        method: &str,
    ) -> Option<(SmolStr, Ty)> {
        let mut matches = Vec::new();
        for impl_sig in self
            .impl_sigs
            .iter()
            .filter(|sig| sig.type_def_id == type_def_id)
        {
            let Some(trait_sig) = self
                .trait_sigs
                .iter()
                .find(|sig| sig.def_id == impl_sig.trait_def_id)
            else {
                continue;
            };
            let Some(method_sig) = trait_sig.methods.iter().find(|sig| sig.name == method) else {
                continue;
            };
            if let Some(impl_method) = impl_sig.methods.iter().find(|sig| sig.name == method) {
                matches.push((
                    self.def_map.get(impl_method.func_def_id).path.clone(),
                    method_sig.return_ty.clone(),
                ));
            } else if method_sig.has_default {
                let trait_path = self.def_map.get(trait_sig.def_id).path.clone();
                matches.push((
                    SmolStr::new(format!("{}.{}", trait_path, method_sig.name)),
                    method_sig.return_ty.clone(),
                ));
            }
        }
        (matches.len() == 1).then(|| matches.remove(0))
    }
    fn trait_method_func_for_trait_and_type(
        &self,
        trait_def_id: ori_types::DefId,
        type_def_id: ori_types::DefId,
        method: &str,
    ) -> Option<(SmolStr, Ty)> {
        let trait_sig = self
            .trait_sigs
            .iter()
            .find(|sig| sig.def_id == trait_def_id)?;
        let method_sig = trait_sig.methods.iter().find(|sig| sig.name == method)?;
        let impl_sig = self
            .impl_sigs
            .iter()
            .find(|sig| sig.type_def_id == type_def_id && sig.trait_def_id == trait_def_id)?;
        if let Some(impl_method) = impl_sig.methods.iter().find(|sig| sig.name == method) {
            return Some((
                self.def_map.get(impl_method.func_def_id).path.clone(),
                method_sig.return_ty.clone(),
            ));
        }
        if method_sig.has_default {
            let trait_path = self.def_map.get(trait_sig.def_id).path.clone();
            return Some((
                SmolStr::new(format!("{}.{}", trait_path, method_sig.name)),
                method_sig.return_ty.clone(),
            ));
        }
        None
    }
    fn displayable_conversion_expr(
        &self,
        value: HirExpr,
        callee_span: Span,
        span: Span,
    ) -> Option<HirExpr> {
        let Ty::Named(type_def_id, _) = &value.ty else {
            return None;
        };
        let displayable_def_id =
            self.resolve_def_id_with_kind("ori.core.Displayable", DefKind::Trait)?;
        let (method_path, return_ty) =
            self.trait_method_func_for_trait_and_type(displayable_def_id, *type_def_id, "display")?;
        let callee = HirExpr {
            kind: HirExprKind::Var(method_path.clone()),
            ty: self.ty_for_def_path(method_path.as_str()),
            span: callee_span,
        };
        Some(HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(callee),
                args: vec![HirArg {
                    label: None,
                    spread: false,
                    value,
                }],
            },
            ty: return_ty,
            span,
        })
    }
    fn iterable_element_ty(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::List(t) | Ty::Set(t) | Ty::Range(t) => *t.clone(),
            Ty::Map(key, _) => *key.clone(),
            Ty::String => Ty::String,
            Ty::Bytes => Ty::U8,
            Ty::Opaque { kind, args } if kind.is_list_backed_collection() => {
                args.first().cloned().unwrap_or(Ty::Infer(0))
            }
            Ty::Opaque {
                kind: OpaqueTy::Heap | OpaqueTy::Graph | OpaqueTy::HashTable,
                args,
            } => args.first().cloned().unwrap_or(Ty::Infer(0)),
            Ty::Named(type_def_id, _) => self
                .impl_sigs
                .iter()
                .filter(|sig| sig.type_def_id == *type_def_id)
                .find(|sig| {
                    self.def_map
                        .get(sig.trait_def_id)
                        .path
                        .ends_with(".Iterable")
                })
                .and_then(|impl_sig| impl_sig.methods.iter().find(|method| method.name == "next"))
                .and_then(|method| {
                    self.func_sigs
                        .iter()
                        .find(|sig| sig.def_id == method.func_def_id)
                })
                .and_then(|sig| match &sig.return_ty {
                    Ty::Optional(inner) => Some(*inner.clone()),
                    _ => None,
                })
                .unwrap_or(Ty::Infer(0)),
            _ => Ty::Infer(0),
        }
    }
    fn operator_trait_method_func_for_type(
        &self,
        type_def_id: ori_types::DefId,
        trait_name: &str,
        method: &str,
    ) -> Option<(SmolStr, Ty)> {
        let trait_def_id = self.def_map.lookup(&format!("ori.core.{trait_name}"))?;
        let impl_sig = self
            .impl_sigs
            .iter()
            .find(|sig| sig.type_def_id == type_def_id && sig.trait_def_id == trait_def_id)?;
        let impl_method = impl_sig.methods.iter().find(|sig| sig.name == method)?;
        let path = self.def_map.get(impl_method.func_def_id).path.clone();
        let return_ty = self
            .func_sigs
            .iter()
            .find(|sig| sig.def_id == impl_method.func_def_id)
            .map(|sig| sig.return_ty.clone())
            .unwrap_or(Ty::Infer(0));
        Some((path, return_ty))
    }
    fn lower_operator_overload(
        &self,
        op: BinaryOp,
        lhs: HirExpr,
        rhs: HirExpr,
        span: Span,
    ) -> Option<HirExpr> {
        if lhs.ty != rhs.ty {
            return None;
        }
        let Ty::Named(type_def_id, _) = &lhs.ty else {
            return None;
        };
        let (trait_name, method_name) = operator_trait_method(op)?;
        let (method_path, return_ty) =
            self.operator_trait_method_func_for_type(*type_def_id, trait_name, method_name)?;
        let call = self.operator_method_call(method_path, return_ty.clone(), lhs, rhs, span);

        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => Some(call),
            BinaryOp::Eq => Some(call),
            BinaryOp::Ne => Some(HirExpr {
                kind: HirExprKind::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(call),
                },
                ty: Ty::Bool,
                span,
            }),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let zero = HirExpr {
                    kind: HirExprKind::IntLit(0),
                    ty: Ty::Int,
                    span,
                };
                Some(HirExpr {
                    kind: HirExprKind::Binary {
                        op,
                        lhs: Box::new(call),
                        rhs: Box::new(zero),
                    },
                    ty: Ty::Bool,
                    span,
                })
            }
            _ => None,
        }
    }

    fn operator_method_call(
        &self,
        method_path: SmolStr,
        return_ty: Ty,
        lhs: HirExpr,
        rhs: HirExpr,
        span: Span,
    ) -> HirExpr {
        let callee_ty = self.ty_for_def_path(method_path.as_str());
        let callee = HirExpr {
            kind: HirExprKind::Var(method_path),
            ty: callee_ty,
            span,
        };
        HirExpr {
            kind: HirExprKind::Call {
                callee: Box::new(callee),
                args: vec![
                    HirArg {
                        label: None,
                        spread: false,
                        value: lhs,
                    },
                    HirArg {
                        label: None,
                        spread: false,
                        value: rhs,
                    },
                ],
            },
            ty: return_ty,
            span,
        }
    }
    fn lower_ast_ty(&mut self, t: &ori_ast::ty::Type, tp: &[SmolStr]) -> Ty {
        let raw = ori_types::lower_type_with_local_aliases(
            t,
            self.namespace,
            tp,
            self.def_map,
            self.file_id,
            self.sink,
            &self.aliases,
            &self.local_type_aliases,
        );
        expand_ty_aliases(raw, self.def_map, &self.type_alias_map)
    }
    fn lower_named_args(
        &mut self,
        args: &[ori_ast::expr::Arg],
        tp: &[SmolStr],
    ) -> Vec<(SmolStr, HirExpr)> {
        args.iter()
            .filter_map(|arg| {
                let label = arg.label.as_ref()?;
                let value = match &arg.value {
                    ori_ast::expr::ArgValue::Expr(e) | ori_ast::expr::ArgValue::Spread(e) => e,
                };
                Some((label.text.clone(), self.lower_expr(value, tp)))
            })
            .collect()
    }
    fn lower_call_arg(&mut self, arg: &Arg, tp: &[SmolStr]) -> HirArg {
        let (spread, value) = match &arg.value {
            ori_ast::expr::ArgValue::Expr(e) => (false, self.lower_expr(e, tp)),
            ori_ast::expr::ArgValue::Spread(e) => (true, self.lower_expr(e, tp)),
        };
        HirArg {
            label: arg.label.as_ref().map(|label| label.text.clone()),
            spread,
            value,
        }
    }
    fn lower_call_args(&mut self, args: &[Arg], tp: &[SmolStr]) -> Vec<HirArg> {
        args.iter()
            .map(|arg| self.lower_call_arg(arg, tp))
            .collect()
    }
    fn lower_local_field_path(
        &mut self,
        q: &ori_ast::common::QualifiedName,
        _tp: &[SmolStr],
    ) -> Option<HirExpr> {
        let first = q.parts.first()?;
        let mut ty = self.lookup_var(&first.text)?;
        let mut expr = HirExpr {
            kind: HirExprKind::Var(first.text.clone()),
            ty: ty.clone(),
            span: first.span,
        };
        for field in q.parts.iter().skip(1) {
            let field_ty = match &ty {
                Ty::Named(def_id, _) => self
                    .struct_field_ty(*def_id, field.as_str())
                    .unwrap_or(Ty::Infer(0)),
                Ty::Tuple(elems) => field
                    .text
                    .parse::<usize>()
                    .ok()
                    .and_then(|idx| elems.get(idx).cloned())
                    .unwrap_or(Ty::Infer(0)),
                _ => Ty::Infer(0),
            };
            expr = HirExpr {
                kind: HirExprKind::Field {
                    object: Box::new(expr),
                    field: field.text.clone(),
                },
                ty: field_ty.clone(),
                span: first.span.cover(field.span),
            };
            ty = field_ty;
        }
        Some(expr)
    }
    fn lower_param(&mut self, p: &ori_ast::item::Param, tp: &[SmolStr]) -> HirParam {
        use ori_ast::item::ParamKind;
        let raw_ty = self.lower_ast_ty(&p.ty, tp);
        let variadic = matches!(p.kind, ParamKind::Variadic);
        let ty = if variadic {
            Ty::List(Box::new(raw_ty.clone()))
        } else {
            raw_ty
        };
        let default = match &p.kind {
            ParamKind::Default(expr) | ParamKind::DefaultAndContract(expr, _) => {
                Some(self.lower_expr(expr, tp))
            }
            ParamKind::Required | ParamKind::Variadic | ParamKind::Contract(_) => None,
        };
        let contract = match &p.kind {
            ParamKind::Contract(expr) | ParamKind::DefaultAndContract(_, expr) => {
                Some(self.lower_param_contract(&p.name.text, &ty, expr, tp))
            }
            ParamKind::Required | ParamKind::Variadic | ParamKind::Default(_) => None,
        };
        HirParam {
            name: p.name.text.clone(),
            ty,
            default,
            contract,
            variadic,
            span: p.span,
        }
    }

    fn lower_param_contract(
        &mut self,
        name: &SmolStr,
        ty: &Ty,
        expr: &Expr,
        tp: &[SmolStr],
    ) -> HirExpr {
        self.push();
        self.bind(SmolStr::new("it"), ty.clone());
        self.bind(name.clone(), ty.clone());
        let contract = self.lower_expr(expr, tp);
        self.pop();
        contract
    }

    fn lower_field_contract(&mut self, ty: &Ty, expr: &Expr, tp: &[SmolStr]) -> HirExpr {
        self.push();
        self.bind(SmolStr::new("it"), ty.clone());
        let contract = self.lower_expr(expr, tp);
        self.pop();
        contract
    }

    fn lower_params(&mut self, params: &[ori_ast::item::Param], tp: &[SmolStr]) -> Vec<HirParam> {
        self.push();
        let lowered: Vec<HirParam> = params
            .iter()
            .map(|p| {
                let param = self.lower_param(p, tp);
                self.bind(param.name.clone(), param.ty.clone());
                param
            })
            .collect();
        self.pop();
        lowered
    }

    fn lower_method_params(
        &mut self,
        params: &[ori_ast::item::Param],
        tp: &[SmolStr],
        self_ty: Ty,
        span: Span,
    ) -> Vec<HirParam> {
        let mut lowered = self.lower_params(params, tp);
        if !has_explicit_self_param(params) {
            lowered.insert(
                0,
                HirParam {
                    name: SmolStr::new("self"),
                    ty: self_ty,
                    default: None,
                    contract: None,
                    variadic: false,
                    span,
                },
            );
        }
        lowered
    }
}

// ── Public entry ──────────────────────────────────────────────────────────────

fn lower_trait_sigs(def_map: &DefMap, trait_sigs: &[TraitSig]) -> Vec<HirTrait> {
    trait_sigs
        .iter()
        .map(|sig| HirTrait {
            def_id: sig.def_id,
            name: def_map.get(sig.def_id).path.clone(),
            methods: sig
                .methods
                .iter()
                .map(|method| HirTraitMethod {
                    name: method.name.clone(),
                    params: method.params.clone(),
                    return_ty: method.return_ty.clone(),
                    default_func_name: method.has_default.then(|| {
                        SmolStr::new(format!("{}.{}", def_map.get(sig.def_id).path, method.name))
                    }),
                })
                .collect(),
        })
        .collect()
}

fn lower_impl_sigs(def_map: &DefMap, impl_sigs: &[ImplSig]) -> Vec<HirTraitImpl> {
    impl_sigs
        .iter()
        .map(|sig| HirTraitImpl {
            trait_def_id: sig.trait_def_id,
            type_def_id: sig.type_def_id,
            methods: sig
                .methods
                .iter()
                .map(|method| HirTraitImplMethod {
                    name: method.name.clone(),
                    func_name: def_map.get(method.func_def_id).path.clone(),
                })
                .collect(),
        })
        .collect()
}

/// Lower an inline method from `apply Type` (free or inside `use Trait`).
fn lower_apply_method(
    l: &mut Lowerer<'_>,
    m: &ori_ast::item::FuncDecl,
    namespace: &str,
    type_name: &SmolStr,
    trait_name: Option<&str>,
    apply_tp: &[SmolStr],
    self_ty: Ty,
    def_map: &DefMap,
    funcs: &mut Vec<HirFunc>,
) {
    let mut all_tp = apply_tp.to_vec();
    all_tp.extend(m.type_params.iter().map(|p| p.name.text.clone()));
    l.aliases.insert(SmolStr::new("Self"), type_name.clone());
    let params = l.lower_method_params(&m.params, &all_tp, self_ty, m.span);
    let body_ret_ty = m
        .return_ty
        .as_ref()
        .map(|t| l.lower_ast_ty(t, &all_tp))
        .unwrap_or(Ty::Void);
    let return_ty = async_return_ty(m.is_async, body_ret_ty.clone());
    l.aliases.remove("Self");
    l.push();
    for p in &params {
        l.bind(p.name.clone(), p.ty.clone());
    }
    l.ret_ty = body_ret_ty.clone();
    l.async_inner_ret_ty = m.is_async.then(|| body_ret_ty.clone());
    let body = l.lower_block(&m.body, &all_tp);
    l.async_inner_ret_ty = None;
    l.pop();
    let path = match trait_name {
        Some(trait_name) => format!("{}.{}.{}.{}", namespace, type_name, trait_name, m.name.text),
        None => format!("{}.{}.{}", namespace, type_name, m.name.text),
    };
    let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
    funcs.push(HirFunc {
        def_id,
        name: SmolStr::new(&path),
        params,
        return_ty,
        body,
        closure_captures: Vec::new(),
        is_public: m.visibility == Visibility::Public,
        is_async: m.is_async,
        is_mut: m.is_mut,
        c_export_name: None,
        span: m.span,
    });
}

fn builtin_stdlib_structs(def_map: &DefMap) -> Vec<HirStruct> {
    let Some(error_def_id) = def_map.lookup("ori.Error") else {
        return Vec::new();
    };

    vec![HirStruct {
        def_id: error_def_id,
        name: SmolStr::new("ori.Error"),
        fields: vec![
            HirField {
                name: SmolStr::new("code"),
                ty: Ty::String,
                contract: None,
                span: Span::DUMMY,
            },
            HirField {
                name: SmolStr::new("message"),
                ty: Ty::String,
                contract: None,
                span: Span::DUMMY,
            },
            // Error chaining: message describing the original cause.
            // Empty string means there is no cause.
            // Future: migrate to `optional[any[Error]]` once the C backend supports
            // recursive struct field types.
            HirField {
                name: SmolStr::new("cause"),
                ty: Ty::String,
                contract: None,
                span: Span::DUMMY,
            },
        ],
        is_public: true,
        repr_c: false,
        span: Span::DUMMY,
    }]
}

pub fn lower(
    file: &SourceFile,
    def_map: &DefMap,
    func_sigs: &[FuncSig],
    value_sigs: &[ValueSig],
    struct_sigs: &[StructSig],
    enum_sigs: &[EnumSig],
    trait_sigs: &[TraitSig],
    impl_sigs: &[ImplSig],
    type_alias_sigs: &[TypeAliasSig],
    reexports: &[ReExport],
    namespace: &str,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> HirModule {
    let mut l = Lowerer::new(
        def_map,
        func_sigs,
        value_sigs,
        struct_sigs,
        enum_sigs,
        trait_sigs,
        impl_sigs,
        type_alias_sigs,
        namespace,
        file_id,
        sink,
    );

    // Build alias map from imports: `import ori.io = io` → `io → ori.io`
    l.aliases = ori_types::resolve::import_aliases(file, reexports);
    let mut structs = builtin_stdlib_structs(def_map);
    let mut enums = Vec::new();
    let traits = lower_trait_sigs(def_map, trait_sigs);
    let trait_impls = lower_impl_sigs(def_map, impl_sigs);
    let mut funcs = Vec::new();
    let mut consts = Vec::new();
    let mut externs = Vec::new();

    for item in &file.items {
        match &item.item {
            Item::Struct(s) => {
                let tp: Vec<SmolStr> = s.type_params.iter().map(|p| p.name.text.clone()).collect();
                let fields = s
                    .fields
                    .iter()
                    .map(|f| {
                        let ty = l.lower_ast_ty(&f.ty, &tp);
                        let contract = f
                            .contract
                            .as_ref()
                            .map(|expr| l.lower_field_contract(&ty, expr, &tp));
                        HirField {
                            name: f.name.text.clone(),
                            ty,
                            contract,
                            span: f.span,
                        }
                    })
                    .collect();
                let path = format!("{}.{}", namespace, s.name.text);
                let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                let repr_c = item.attrs.iter().any(|a| {
                    a.name.text == "repr"
                        && a.args
                            .iter()
                            .any(|arg| matches!(arg, ori_ast::common::AttrArg::String(s, _) if *s == "C"))
                });
                structs.push(HirStruct {
                    def_id,
                    name: SmolStr::new(&path),
                    fields,
                    is_public: s.visibility == Visibility::Public,
                    repr_c,
                    span: s.span,
                });

                for m in &s.methods {
                    let mut all_tp = tp.clone();
                    all_tp.extend(m.type_params.iter().map(|p| p.name.text.clone()));
                    l.aliases.insert(SmolStr::new("Self"), s.name.text.clone());
                    let self_ty = Ty::Named(def_id, Vec::new());
                    let params = l.lower_method_params(&m.params, &all_tp, self_ty, m.span);
                    let body_ret_ty = m
                        .return_ty
                        .as_ref()
                        .map(|t| l.lower_ast_ty(t, &all_tp))
                        .unwrap_or(Ty::Void);
                    let return_ty = async_return_ty(m.is_async, body_ret_ty.clone());
                    l.aliases.remove("Self");
                    l.push();
                    for p in &params {
                        l.bind(p.name.clone(), p.ty.clone());
                    }
                    l.ret_ty = body_ret_ty.clone();
                    l.async_inner_ret_ty = m.is_async.then(|| body_ret_ty.clone());
                    let body = l.lower_block(&m.body, &all_tp);
                    l.async_inner_ret_ty = None;
                    l.pop();
                    let path = format!("{}.{}.{}", namespace, s.name.text, m.name.text);
                    let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                    funcs.push(HirFunc {
                        def_id,
                        name: SmolStr::new(&path),
                        params,
                        return_ty,
                        body,
                        closure_captures: Vec::new(),
                        is_public: m.visibility == Visibility::Public,
                        is_async: m.is_async,
                        is_mut: m.is_mut,
                        c_export_name: None,
                        span: m.span,
                    });
                }
            }
            Item::Enum(e) => {
                let path = format!("{}.{}", namespace, e.name.text);
                let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                let tp: Vec<SmolStr> = e.type_params.iter().map(|p| p.name.text.clone()).collect();
                let variants = e
                    .variants
                    .iter()
                    .map(|v| HirVariant {
                        name: v.name.text.clone(),
                        fields: v
                            .fields
                            .iter()
                            .map(|f| HirField {
                                name: f.name.text.clone(),
                                ty: l.lower_ast_ty(&f.ty, &tp),
                                contract: None,
                                span: f.span,
                            })
                            .collect(),
                        span: v.span,
                    })
                    .collect();
                enums.push(HirEnum {
                    def_id,
                    name: SmolStr::new(&path),
                    variants,
                    is_public: e.visibility == Visibility::Public,
                    span: e.span,
                });
            }
            Item::Apply(apply) => {
                let type_name = apply.for_type.last().text.clone();
                let self_ty = l
                    .resolve_def_path(&apply.for_type.to_string())
                    .and_then(|path| def_map.lookup(&path))
                    .map(|def_id| Ty::Named(def_id, Vec::new()))
                    .unwrap_or(Ty::Infer(0));
                let tp: Vec<SmolStr> = apply
                    .type_params
                    .iter()
                    .map(|p| p.name.text.clone())
                    .collect();

                // Free methods: inherent path `namespace.Type.method`.
                for member in &apply.free_members {
                    if let ori_ast::item::ApplyMember::Method(m) = member {
                        lower_apply_method(
                            &mut l,
                            m,
                            &namespace,
                            &type_name,
                            None,
                            &tp,
                            self_ty.clone(),
                            def_map,
                            &mut funcs,
                        );
                    }
                }

                for use_sec in &apply.uses {
                    l.local_type_aliases = use_sec
                        .associated_types
                        .iter()
                        .map(|(name, ty)| (name.text.clone(), ty.clone()))
                        .collect();
                    for member in &use_sec.members {
                        if let ori_ast::item::ApplyMember::Method(m) = member {
                            lower_apply_method(
                                &mut l,
                                m,
                                &namespace,
                                &type_name,
                                Some(use_sec.trait_name.last().text.as_str()),
                                &tp,
                                self_ty.clone(),
                                def_map,
                                &mut funcs,
                            );
                        }
                        // Binds reuse the free function's HirFunc; no new lowering.
                    }
                    l.local_type_aliases.clear();
                }
            }
            Item::Trait(t) => {
                let trait_path = format!("{}.{}", namespace, t.name.text);
                let trait_self_ty = def_map
                    .lookup(&trait_path)
                    .map(|def_id| Ty::Named(def_id, Vec::new()))
                    .unwrap_or(Ty::Infer(0));
                let mut tp: Vec<SmolStr> =
                    t.type_params.iter().map(|p| p.name.text.clone()).collect();
                for m in &t.members {
                    if let ori_ast::item::TraitMember::Type(name) = m {
                        tp.push(name.text.clone());
                    }
                }
                for m in &t.members {
                    if let ori_ast::item::TraitMember::Default(func) = m {
                        let mut all_tp = tp.clone();
                        all_tp.extend(func.type_params.iter().map(|p| p.name.text.clone()));
                        l.aliases.insert(SmolStr::new("Self"), t.name.text.clone());
                        let params = l.lower_method_params(
                            &func.params,
                            &all_tp,
                            trait_self_ty.clone(),
                            func.span,
                        );
                        let body_ret_ty = func
                            .return_ty
                            .as_ref()
                            .map(|ty| l.lower_ast_ty(ty, &all_tp))
                            .unwrap_or(Ty::Void);
                        let return_ty = async_return_ty(func.is_async, body_ret_ty.clone());
                        l.aliases.remove("Self");
                        l.push();
                        for p in &params {
                            l.bind(p.name.clone(), p.ty.clone());
                        }
                        l.ret_ty = body_ret_ty.clone();
                        l.async_inner_ret_ty = func.is_async.then(|| body_ret_ty.clone());
                        let body = l.lower_block(&func.body, &all_tp);
                        l.async_inner_ret_ty = None;
                        l.pop();
                        let path = format!("{}.{}.{}", namespace, t.name.text, func.name.text);
                        let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                        funcs.push(HirFunc {
                            def_id,
                            name: SmolStr::new(&path),
                            params,
                            return_ty,
                            body,
                            closure_captures: Vec::new(),
                            is_public: func.visibility == Visibility::Public,
                            is_async: func.is_async,
                            is_mut: func.is_mut,
                            c_export_name: None,
                            span: func.span,
                        });
                    }
                }
            }
            Item::Func(f) => {
                let tp: Vec<SmolStr> = f.type_params.iter().map(|p| p.name.text.clone()).collect();
                let params = l.lower_params(&f.params, &tp);
                let body_ret_ty = f
                    .return_ty
                    .as_ref()
                    .map(|t| l.lower_ast_ty(t, &tp))
                    .unwrap_or(Ty::Void);
                let return_ty = async_return_ty(f.is_async, body_ret_ty.clone());
                l.push();
                for p in &params {
                    l.bind(p.name.clone(), p.ty.clone());
                }
                l.ret_ty = body_ret_ty.clone();
                l.async_inner_ret_ty = f.is_async.then(|| body_ret_ty.clone());
                let body = l.lower_block(&f.body, &tp);
                l.async_inner_ret_ty = None;
                l.pop();
                let path = format!("{}.{}", namespace, f.name.text);
                let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                let c_export_name = c_export_name_from_attrs(&item.attrs, f.name.text.as_str());
                funcs.push(HirFunc {
                    def_id,
                    name: SmolStr::new(&path),
                    params,
                    return_ty,
                    body,
                    closure_captures: Vec::new(),
                    is_public: f.visibility == Visibility::Public,
                    is_async: f.is_async,
                    is_mut: f.is_mut,
                    c_export_name,
                    span: f.span,
                });
            }
            Item::Const(c) => {
                let ty = l.lower_ast_ty(&c.ty, &[]);
                let mut value = l.lower_expr(&c.value, &[]);
                apply_expected_expr_ty(&mut value, &ty);
                let path = format!("{}.{}", namespace, c.name.text);
                let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                consts.push(HirConst {
                    def_id,
                    name: SmolStr::new(&path),
                    ty,
                    value,
                    mutable: false,
                    is_public: c.visibility == Visibility::Public,
                    span: c.span,
                });
            }
            Item::Var(v) => {
                let ty = l.lower_ast_ty(&v.ty, &[]);
                let mut value = l.lower_expr(&v.value, &[]);
                apply_expected_expr_ty(&mut value, &ty);
                let path = format!("{}.{}", namespace, v.name.text);
                let def_id = def_map.lookup(&path).unwrap_or(ori_types::DefId(u32::MAX));
                consts.push(HirConst {
                    def_id,
                    name: SmolStr::new(&path),
                    ty,
                    value,
                    mutable: true,
                    is_public: v.visibility == Visibility::Public,
                    span: v.span,
                });
            }
            Item::Extern(ext) => {
                let abi = match ext.abi {
                    ori_ast::item::AbiLabel::C => SmolStr::new("C"),
                    ori_ast::item::AbiLabel::Host => SmolStr::new("host"),
                };
                for member in &ext.members {
                    match member {
                        ori_ast::item::ExternMember::Func {
                            name,
                            params,
                            return_ty,
                            span,
                            ..
                        } => {
                            let hir_params = l.lower_params(params, &[]);
                            let ret = return_ty
                                .as_ref()
                                .map(|t| l.lower_ast_ty(t, &[]))
                                .unwrap_or(Ty::Void);
                            externs.push(HirExtern::Func {
                                path: SmolStr::new(format!("{}.{}", namespace, name.text)),
                                name: name.text.clone(),
                                params: hir_params,
                                return_ty: ret,
                                abi: abi.clone(),
                                span: *span,
                            });
                        }
                        ori_ast::item::ExternMember::Var { name, ty, span, .. } => {
                            externs.push(HirExtern::Var {
                                path: SmolStr::new(format!("{}.{}", namespace, name.text)),
                                name: name.text.clone(),
                                ty: l.lower_ast_ty(ty, &[]),
                                abi: abi.clone(),
                                span: *span,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    funcs.extend(std::mem::take(&mut l.generated_funcs));

    HirModule {
        namespace: SmolStr::new(namespace),
        structs,
        enums,
        traits,
        trait_impls,
        funcs,
        consts,
        externs,
    }
}

// ── Statement lowering ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DefaultParam {
    name: SmolStr,
    ty: Ty,
    default: Option<HirExpr>,
    variadic: bool,
}

pub fn insert_default_arguments(module: &mut HirModule) {
    let defaults: HashMap<SmolStr, Vec<DefaultParam>> = module
        .funcs
        .iter()
        .map(|func| {
            (
                func.name.clone(),
                func.params
                    .iter()
                    .map(|param| DefaultParam {
                        name: param.name.clone(),
                        ty: param.ty.clone(),
                        default: param.default.clone(),
                        variadic: param.variadic,
                    })
                    .collect(),
            )
        })
        .collect();

    if defaults.is_empty() {
        return;
    }

    for func in &mut module.funcs {
        for param in &mut func.params {
            if let Some(default) = &mut param.default {
                insert_default_arguments_expr(default, &defaults);
            }
        }
        insert_default_arguments_block(&mut func.body, &defaults);
    }
    for konst in &mut module.consts {
        insert_default_arguments_expr(&mut konst.value, &defaults);
    }
}

fn insert_default_arguments_block(
    block: &mut HirBlock,
    defaults: &HashMap<SmolStr, Vec<DefaultParam>>,
) {
    for stmt in &mut block.stmts {
        insert_default_arguments_stmt(stmt, defaults);
    }
}

fn insert_default_arguments_stmt(
    stmt: &mut HirStmt,
    defaults: &HashMap<SmolStr, Vec<DefaultParam>>,
) {
    match stmt {
        HirStmt::Let { value, .. } | HirStmt::Using { value, .. } => {
            insert_default_arguments_expr(value, defaults);
        }
        HirStmt::Assign { lvalue, value, .. } => {
            insert_default_arguments_lvalue(lvalue, defaults);
            insert_default_arguments_expr(value, defaults);
        }
        HirStmt::Return(Some(value), _) | HirStmt::Expr(value) => {
            insert_default_arguments_expr(value, defaults);
        }
        HirStmt::Return(None, _) | HirStmt::Break(_) | HirStmt::Continue(_) => {}
        HirStmt::If {
            cond,
            then,
            else_ifs,
            else_,
            ..
        } => {
            insert_default_arguments_expr(cond, defaults);
            insert_default_arguments_block(then, defaults);
            for (cond, block) in else_ifs {
                insert_default_arguments_expr(cond, defaults);
                insert_default_arguments_block(block, defaults);
            }
            if let Some(block) = else_ {
                insert_default_arguments_block(block, defaults);
            }
        }
        HirStmt::While { cond, body, .. } => {
            insert_default_arguments_expr(cond, defaults);
            insert_default_arguments_block(body, defaults);
        }
        HirStmt::Loop { body, .. } => insert_default_arguments_block(body, defaults),
        HirStmt::For { iterable, body, .. } => {
            insert_default_arguments_expr(iterable, defaults);
            insert_default_arguments_block(body, defaults);
        }
        HirStmt::Repeat { count, body, .. } => {
            insert_default_arguments_expr(count, defaults);
            insert_default_arguments_block(body, defaults);
        }
        HirStmt::Match {
            scrutinee, arms, ..
        } => {
            insert_default_arguments_expr(scrutinee, defaults);
            for arm in arms {
                for stmt in &mut arm.body {
                    insert_default_arguments_stmt(stmt, defaults);
                }
            }
        }
        HirStmt::IfSome {
            value, then, else_, ..
        } => {
            insert_default_arguments_expr(value, defaults);
            insert_default_arguments_block(then, defaults);
            if let Some(block) = else_ {
                insert_default_arguments_block(block, defaults);
            }
        }
        HirStmt::WhileSome { value, body, .. } => {
            insert_default_arguments_expr(value, defaults);
            insert_default_arguments_block(body, defaults);
        }
        HirStmt::Check { condition, .. } => {
            insert_default_arguments_expr(condition, defaults);
        }
    }
}

fn insert_default_arguments_lvalue(
    lvalue: &mut HirLValue,
    defaults: &HashMap<SmolStr, Vec<DefaultParam>>,
) {
    match lvalue {
        HirLValue::Var(_) => {}
        HirLValue::Field { base, .. } => insert_default_arguments_lvalue(base, defaults),
        HirLValue::Index { base, index } => {
            insert_default_arguments_lvalue(base, defaults);
            insert_default_arguments_expr(index, defaults);
        }
    }
}

fn normalize_call_arguments(args: &mut Vec<HirArg>, params: &[DefaultParam]) {
    if let Some(variadic_index) = params.iter().position(|param| param.variadic) {
        normalize_variadic_call_arguments(args, params, variadic_index);
        return;
    }

    let original = std::mem::take(args);
    let mut slots: Vec<Option<HirExpr>> = vec![None; params.len()];
    let mut extras = Vec::new();
    let mut next_positional = 0usize;

    for arg in original {
        if let Some(label) = &arg.label {
            if let Some(index) = params.iter().position(|param| &param.name == label) {
                if slots[index].is_none() {
                    slots[index] = Some(arg.value);
                } else {
                    extras.push(arg.value);
                }
            } else {
                extras.push(arg.value);
            }
        } else {
            while next_positional < slots.len() && slots[next_positional].is_some() {
                next_positional += 1;
            }
            if next_positional < slots.len() {
                slots[next_positional] = Some(arg.value);
                next_positional += 1;
            } else {
                extras.push(arg.value);
            }
        }
    }

    let mut stopped_at_required = false;
    for (index, param) in params.iter().enumerate() {
        if let Some(value) = slots[index].take() {
            args.push(HirArg {
                label: None,
                spread: false,
                value,
            });
        } else if let Some(default) = &param.default {
            args.push(HirArg {
                label: None,
                spread: false,
                value: default.clone(),
            });
        } else {
            stopped_at_required = true;
            break;
        }
    }

    if stopped_at_required {
        for value in slots.into_iter().flatten() {
            args.push(HirArg {
                label: None,
                spread: false,
                value,
            });
        }
    }
    args.extend(extras.into_iter().map(|value| HirArg {
        label: None,
        spread: false,
        value,
    }));
}

fn normalize_variadic_call_arguments(
    args: &mut Vec<HirArg>,
    params: &[DefaultParam],
    variadic_index: usize,
) {
    let original = std::mem::take(args);
    let mut slots: Vec<Option<HirArg>> = vec![None; variadic_index];
    let mut varargs = Vec::new();
    let mut next_positional = 0usize;

    for arg in original {
        if let Some(label) = &arg.label {
            if let Some(index) = params.iter().position(|param| &param.name == label) {
                if index < variadic_index {
                    if slots[index].is_none() {
                        slots[index] = Some(arg);
                    } else {
                        varargs.push(HirListElement {
                            spread: arg.spread,
                            value: arg.value,
                        });
                    }
                } else if index == variadic_index {
                    varargs.push(HirListElement {
                        spread: arg.spread,
                        value: arg.value,
                    });
                }
            } else {
                varargs.push(HirListElement {
                    spread: arg.spread,
                    value: arg.value,
                });
            }
        } else {
            while next_positional < slots.len() && slots[next_positional].is_some() {
                next_positional += 1;
            }
            if next_positional < slots.len() {
                slots[next_positional] = Some(arg);
                next_positional += 1;
            } else {
                varargs.push(HirListElement {
                    spread: arg.spread,
                    value: arg.value,
                });
            }
        }
    }

    let mut stopped_at_required = false;
    for (index, param) in params.iter().take(variadic_index).enumerate() {
        if let Some(arg) = slots[index].take() {
            args.push(HirArg {
                label: None,
                spread: false,
                value: arg.value,
            });
        } else if let Some(default) = &param.default {
            args.push(HirArg {
                label: None,
                spread: false,
                value: default.clone(),
            });
        } else {
            stopped_at_required = true;
            break;
        }
    }

    if stopped_at_required {
        for arg in slots.into_iter().flatten() {
            args.push(HirArg {
                label: None,
                spread: false,
                value: arg.value,
            });
        }
        args.extend(varargs.into_iter().map(|element| HirArg {
            label: None,
            spread: element.spread,
            value: element.value,
        }));
        return;
    }

    let list_ty = params
        .get(variadic_index)
        .map(|param| param.ty.clone())
        .unwrap_or_else(|| Ty::List(Box::new(Ty::Infer(0))));
    let elem_ty = match &list_ty {
        Ty::List(elem) => *elem.clone(),
        other => other.clone(),
    };
    let span = varargs
        .iter()
        .map(|element| element.value.span)
        .reduce(|acc, span| acc.cover(span))
        .unwrap_or(Span::DUMMY);
    let has_spread = varargs.iter().any(|element| element.spread);
    let kind = if has_spread {
        HirExprKind::ListSpreadLit {
            elem_ty: elem_ty.clone(),
            elements: varargs,
        }
    } else {
        HirExprKind::ListLit {
            elem_ty: elem_ty.clone(),
            elements: varargs.into_iter().map(|element| element.value).collect(),
        }
    };
    args.push(HirArg {
        label: None,
        spread: false,
        value: HirExpr {
            kind,
            ty: list_ty,
            span,
        },
    });
}

fn insert_default_arguments_expr(
    expr: &mut HirExpr,
    defaults: &HashMap<SmolStr, Vec<DefaultParam>>,
) {
    match &mut expr.kind {
        HirExprKind::Binary { lhs, rhs, .. } => {
            insert_default_arguments_expr(lhs, defaults);
            insert_default_arguments_expr(rhs, defaults);
        }
        HirExprKind::Unary { operand, .. } => insert_default_arguments_expr(operand, defaults),
        HirExprKind::Field { object, .. } | HirExprKind::TupleIndex { object, .. } => {
            insert_default_arguments_expr(object, defaults);
        }
        HirExprKind::Index { object, index } => {
            insert_default_arguments_expr(object, defaults);
            insert_default_arguments_expr(index, defaults);
        }
        HirExprKind::Call { callee, args } => {
            insert_default_arguments_expr(callee, defaults);
            for arg in args.iter_mut() {
                insert_default_arguments_expr(&mut arg.value, defaults);
            }
            if let HirExprKind::Var(name) = &callee.kind {
                if let Some(param_defaults) = defaults.get(name) {
                    normalize_call_arguments(args, param_defaults);
                }
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            insert_default_arguments_expr(receiver, defaults);
            for arg in args {
                insert_default_arguments_expr(arg, defaults);
            }
        }
        HirExprKind::StructLit { fields, .. } | HirExprKind::EnumVariant { fields, .. } => {
            for (_, value) in fields {
                insert_default_arguments_expr(value, defaults);
            }
        }
        HirExprKind::ListLit { elements, .. }
        | HirExprKind::SetLit { elements, .. }
        | HirExprKind::TupleLit(elements) => {
            for elem in elements {
                insert_default_arguments_expr(elem, defaults);
            }
        }
        HirExprKind::ListSpreadLit { elements, .. } => {
            for elem in elements {
                insert_default_arguments_expr(&mut elem.value, defaults);
            }
        }
        HirExprKind::Some_(inner)
        | HirExprKind::Ok_(inner)
        | HirExprKind::Err_(inner)
        | HirExprKind::Propagate(inner)
        | HirExprKind::Await(inner) => insert_default_arguments_expr(inner, defaults),
        HirExprKind::IfExpr { cond, then, else_ } => {
            insert_default_arguments_expr(cond, defaults);
            insert_default_arguments_expr(then, defaults);
            insert_default_arguments_expr(else_, defaults);
        }
        HirExprKind::MatchExpr { scrutinee, arms } => {
            insert_default_arguments_expr(scrutinee, defaults);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    insert_default_arguments_expr(guard, defaults);
                }
                insert_default_arguments_expr(&mut arm.body, defaults);
            }
        }
        HirExprKind::Range { start, end } => {
            insert_default_arguments_expr(start, defaults);
            insert_default_arguments_expr(end, defaults);
        }
        HirExprKind::MapLit { entries, .. } => {
            for (key, value) in entries {
                insert_default_arguments_expr(key, defaults);
                insert_default_arguments_expr(value, defaults);
            }
        }
        HirExprKind::StructUpdate { base, updates, .. } => {
            insert_default_arguments_expr(base, defaults);
            for (_, value) in updates {
                insert_default_arguments_expr(value, defaults);
            }
        }
        HirExprKind::InterpolatedStr(parts) => {
            for part in parts {
                if let HirStrPart::Expr(value) = part {
                    insert_default_arguments_expr(value, defaults);
                }
            }
        }
        HirExprKind::BoolLit(_)
        | HirExprKind::IntLit(_)
        | HirExprKind::FloatLit(_)
        | HirExprKind::StrLit(_)
        | HirExprKind::BytesLit(_)
        | HirExprKind::Unit
        | HirExprKind::Var(_)
        | HirExprKind::None_
        | HirExprKind::Closure { .. } => {}
        HirExprKind::IsCheck { value, .. } => insert_default_arguments_expr(value, defaults),
    }
}

impl<'a> Lowerer<'a> {
    fn lower_block(&mut self, block: &Block, tp: &[SmolStr]) -> HirBlock {
        self.push();
        let stmts = block
            .stmts
            .iter()
            .filter_map(|s| self.lower_stmt(s, tp))
            .collect();
        self.pop();
        HirBlock {
            stmts,
            span: block.span,
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt, tp: &[SmolStr]) -> Option<HirStmt> {
        match stmt {
            Stmt::Const(c) => {
                let mut val = self.lower_expr(&c.value, tp);
                let ty = if let Some(ast_ty) = &c.ty {
                    let ty = self.lower_ast_ty(ast_ty, tp);
                    apply_expected_expr_ty(&mut val, &ty);
                    ty
                } else {
                    // 0.3.1: type omitted; HIR type comes from the lowered value.
                    val.ty.clone()
                };
                self.bind(c.name.text.clone(), ty.clone());
                Some(HirStmt::Let {
                    name: c.name.text.clone(),
                    ty,
                    mutable: false,
                    value: val,
                    span: c.span,
                })
            }
            Stmt::Var(v) => {
                let mut val = self.lower_expr(&v.value, tp);
                let ty = if let Some(ast_ty) = &v.ty {
                    let ty = self.lower_ast_ty(ast_ty, tp);
                    apply_expected_expr_ty(&mut val, &ty);
                    ty
                } else {
                    val.ty.clone()
                };
                self.bind(v.name.text.clone(), ty.clone());
                Some(HirStmt::Let {
                    name: v.name.text.clone(),
                    ty,
                    mutable: true,
                    value: val,
                    span: v.span,
                })
            }
            Stmt::Return(r) => {
                let val = r.value.as_ref().map(|e| {
                    let mut value = self.lower_expr(e, tp);
                    let expected = self.async_inner_ret_ty.as_ref().unwrap_or(&self.ret_ty);
                    apply_expected_expr_ty(&mut value, expected);
                    value
                });
                Some(HirStmt::Return(val, r.span))
            }
            Stmt::Break(sp) => Some(HirStmt::Break(*sp)),
            Stmt::Continue(sp) => Some(HirStmt::Continue(*sp)),
            Stmt::Expr(e) => Some(HirStmt::Expr(self.lower_expr(e, tp))),
            Stmt::If(i) => {
                let cond = self.lower_expr(&i.condition, tp);
                let then = self.lower_block(&i.then_block, tp);
                let else_ifs = i
                    .else_ifs
                    .iter()
                    .map(|(c, b)| (self.lower_expr(c, tp), self.lower_block(b, tp)))
                    .collect();
                let else_ = i.else_block.as_ref().map(|b| self.lower_block(b, tp));
                Some(HirStmt::If {
                    cond,
                    then,
                    else_ifs,
                    else_,
                    span: i.span,
                })
            }
            Stmt::While(w) => {
                let cond = self.lower_expr(&w.condition, tp);
                let body = self.lower_block(&w.body, tp);
                Some(HirStmt::While {
                    cond,
                    body,
                    span: w.span,
                })
            }
            Stmt::For(f) => {
                let iterable = self.lower_expr(&f.iterable, tp);
                let elem_ty = self.iterable_element_ty(&iterable.ty);
                let second_ty = for_second_binding_ty(&iterable.ty);
                self.push();
                self.bind(f.binding.text.clone(), elem_ty.clone());
                if let Some(ref sb) = f.second_binding {
                    self.bind(sb.text.clone(), second_ty);
                }
                let body = self.lower_block(&f.body, tp);
                self.pop();
                Some(HirStmt::For {
                    binding: f.binding.text.clone(),
                    index_binding: f.second_binding.as_ref().map(|n| n.text.clone()),
                    elem_ty,
                    iterable,
                    body,
                    span: f.span,
                })
            }
            Stmt::Loop(l) => {
                let body = self.lower_block(&l.body, tp);
                Some(HirStmt::Loop { body, span: l.span })
            }
            Stmt::Repeat(r) => {
                let count = self.lower_expr(&r.count, tp);
                let body = self.lower_block(&r.body, tp);
                Some(HirStmt::Repeat {
                    count,
                    body,
                    span: r.span,
                })
            }
            Stmt::Match(m) => {
                let scrutinee = self.lower_expr(&m.scrutinee, tp);
                let arms = m
                    .cases
                    .iter()
                    .map(|c| self.lower_match_case(c, tp, &scrutinee.ty))
                    .collect();
                Some(HirStmt::Match {
                    scrutinee,
                    arms,
                    span: m.span,
                })
            }
            Stmt::Assign(a) => {
                let lvalue = lower_lvalue(&a.lvalue, self, tp);
                let value = self.lower_expr(&a.value, tp);
                Some(HirStmt::Assign {
                    lvalue,
                    value,
                    span: a.span,
                })
            }
            Stmt::CompoundAssign(c) => {
                // Lower `x += v` to `x = x + v` for v1
                let lvalue = lower_lvalue(&c.lvalue, self, tp);
                let cur = lvalue_to_expr(&lvalue, c.span);
                let rhs = self.lower_expr(&c.value, tp);
                let op = compound_op_to_binary(c.op);
                let ty = binary_result_ty(op, &cur.ty, &rhs.ty);
                let value = HirExpr {
                    kind: HirExprKind::Binary {
                        op,
                        lhs: Box::new(cur),
                        rhs: Box::new(rhs),
                    },
                    ty,
                    span: c.span,
                };
                Some(HirStmt::Assign {
                    lvalue,
                    value,
                    span: c.span,
                })
            }
            Stmt::IfSome(s) => {
                use ori_ast::stmt::UnwrapKind;
                let value = self.lower_expr(&s.value, tp);
                // `unwrap_ty` peels optional/result to the ok side; `err`
                // binds the other side of the same wrapper.
                let inner_ty = match (s.kind, &value.ty) {
                    (UnwrapKind::Err, Ty::Result(_, err)) => *err.clone(),
                    _ => unwrap_ty(&value.ty),
                };
                self.push();
                self.bind(s.binding.text.clone(), inner_ty.clone());
                let then = self.lower_block(&s.then_block, tp);
                self.pop();
                let else_ = s.else_block.as_ref().map(|b| self.lower_block(b, tp));
                Some(HirStmt::IfSome {
                    kind: s.kind,
                    binding: s.binding.text.clone(),
                    inner_ty,
                    value,
                    then,
                    else_,
                    span: s.span,
                })
            }
            Stmt::WhileSome(s) => {
                let value = self.lower_expr(&s.value, tp);
                let inner_ty = unwrap_ty(&value.ty);
                self.push();
                self.bind(s.binding.text.clone(), inner_ty.clone());
                let body = self.lower_block(&s.body, tp);
                self.pop();
                Some(HirStmt::WhileSome {
                    binding: s.binding.text.clone(),
                    inner_ty,
                    value,
                    body,
                    span: s.span,
                })
            }
            Stmt::Using(u) => {
                let ty = self.lower_ast_ty(&u.ty, tp);
                let val = self.lower_expr(&u.value, tp);
                self.bind(u.name.text.clone(), ty.clone());
                Some(HirStmt::Using {
                    name: u.name.text.clone(),
                    ty,
                    value: val,
                    span: u.span,
                })
            }
            Stmt::Check(c) => {
                let cond = self.lower_expr(&c.condition, tp);
                Some(HirStmt::Check {
                    condition: cond,
                    message: c.message.clone(),
                    span: c.span,
                })
            }
        }
    }

    fn lower_match_case(&mut self, case: &MatchCase, tp: &[SmolStr], scr_ty: &Ty) -> HirArm {
        match case {
            MatchCase::Pattern {
                pattern,
                guard,
                body,
                span,
            } => {
                let resolved_scr_ty = replace_json_placeholder_in_ty(scr_ty.clone(), self.def_map);
                let pat = lower_pattern(pattern, &resolved_scr_ty, self.enum_sigs);
                self.push();
                bind_hir_pattern_scope(self, &pat);
                // The guard is lowered inside the pattern scope: `case n if
                // n >= 90:` reads `n` from the arm's own bindings.
                let guard = guard.as_ref().map(|g| self.lower_expr(g, tp));
                let stmts = body.iter().filter_map(|s| self.lower_stmt(s, tp)).collect();
                self.pop();
                HirArm {
                    pattern: pat,
                    guard,
                    body: stmts,
                    span: *span,
                }
            }
            MatchCase::Else { body, span } => {
                self.push();
                let stmts = body.iter().filter_map(|s| self.lower_stmt(s, tp)).collect();
                self.pop();
                HirArm {
                    pattern: HirPattern::Wildcard,
                    guard: None,
                    body: stmts,
                    span: *span,
                }
            }
        }
    }
}

// ── Expression lowering ───────────────────────────────────────────────────────

impl<'a> Lowerer<'a> {
    pub fn lower_expr(&mut self, expr: &Expr, tp: &[SmolStr]) -> HirExpr {
        let span = expr.span();
        match expr {
            Expr::BoolLit(b, _) => HirExpr {
                kind: HirExprKind::BoolLit(*b),
                ty: Ty::Bool,
                span,
            },
            Expr::IntLit { raw, .. } => {
                let parsed = parse_int_literal(raw).unwrap_or_else(|err| {
                    eprintln!(
                        "ori: ICE: invalid integer literal reached HIR lowering: {}",
                        err.message
                    );
                    ori_types::literal::ParsedIntLiteral {
                        value: 0,
                        ty: Ty::Int,
                    }
                });
                HirExpr {
                    kind: HirExprKind::IntLit(parsed.value),
                    ty: parsed.ty,
                    span,
                }
            }
            Expr::FloatLit { raw, .. } => {
                let parsed = parse_float_lit(raw);
                HirExpr {
                    kind: HirExprKind::FloatLit(parsed.value),
                    ty: parsed.ty,
                    span,
                }
            }
            Expr::StrLit { value, .. } => HirExpr {
                kind: HirExprKind::StrLit(value.clone()),
                ty: Ty::String,
                span,
            },
            Expr::FStrLit { parts, .. } => {
                let hparts = parts
                    .iter()
                    .map(|p| match p {
                        FStrPart::Literal(s) => HirStrPart::Literal(s.clone()),
                        FStrPart::Interpolated(e) => {
                            let value = self.lower_expr(e, tp);
                            let value = self
                                .displayable_conversion_expr(value.clone(), e.span(), e.span())
                                .unwrap_or(value);
                            HirStrPart::Expr(value)
                        }
                    })
                    .collect();
                HirExpr {
                    kind: HirExprKind::InterpolatedStr(hparts),
                    ty: Ty::String,
                    span,
                }
            }
            Expr::BytesLit { bytes, .. } => HirExpr {
                kind: HirExprKind::BytesLit(bytes.clone()),
                ty: Ty::Bytes,
                span,
            },
            Expr::None(_) => HirExpr {
                kind: HirExprKind::None_,
                ty: Ty::Optional(Box::new(Ty::Infer(0))),
                span,
            },
            Expr::SelfExpr(_) => HirExpr {
                kind: HirExprKind::Var(SmolStr::new("self")),
                ty: self.lookup("self"),
                span,
            },
            Expr::Ident(n) => {
                if let Some(ty) = self.lookup_var(&n.text) {
                    HirExpr {
                        kind: HirExprKind::Var(n.text.clone()),
                        ty,
                        span,
                    }
                } else if let Some(path) = self.resolve_def_path(&n.text) {
                    let ty = self.ty_for_def_path(&path);
                    HirExpr {
                        kind: HirExprKind::Var(path),
                        ty,
                        span,
                    }
                } else {
                    HirExpr {
                        kind: HirExprKind::Var(n.text.clone()),
                        ty: Ty::Error,
                        span,
                    }
                }
            }
            Expr::QualifiedIdent(q) => {
                let path = q.to_string();
                // A single-segment "qualified" name that names a local
                // binding is just a variable reference — locals shadow bare
                // stdlib builtins (`len`, `string`, ...). Without this,
                // `const len = ...; return len` lowered to the stdlib
                // runtime symbol (`ori_len`) and native codegen failed with
                // "undefined variable" (LANG-FRONT-1).
                if q.parts.len() == 1 {
                    if let Some(ty) = self.lookup_var(&path) {
                        return HirExpr {
                            kind: HirExprKind::Var(SmolStr::new(&path)),
                            ty,
                            span,
                        };
                    }
                }
                // Check stdlib / alias resolution first
                // Math constants emitted as float literals
                let expanded_path = self.expand_alias(&path);
                if expanded_path.as_str() == "ori.math.pi" {
                    return HirExpr {
                        kind: HirExprKind::FloatLit(std::f64::consts::PI),
                        ty: Ty::Float,
                        span,
                    };
                }
                if expanded_path.as_str() == "ori.math.e" {
                    return HirExpr {
                        kind: HirExprKind::FloatLit(std::f64::consts::E),
                        ty: Ty::Float,
                        span,
                    };
                }
                if expanded_path.as_str() == "ori.math.infinity" {
                    return HirExpr {
                        kind: HirExprKind::FloatLit(f64::INFINITY),
                        ty: Ty::Float,
                        span,
                    };
                }
                if expanded_path.as_str() == "ori.math.nan" {
                    return HirExpr {
                        kind: HirExprKind::FloatLit(f64::NAN),
                        ty: Ty::Float,
                        span,
                    };
                }
                if let Some(c_name) = self.resolve_stdlib(&path) {
                    return HirExpr {
                        kind: HirExprKind::Var(SmolStr::new(c_name)),
                        ty: stdlib_c_func_ty(c_name, Some(self.def_map)),
                        span,
                    };
                }
                if let Some((def_id, variant)) = self.resolve_enum_variant(q) {
                    return HirExpr {
                        kind: HirExprKind::EnumVariant {
                            def_id,
                            variant,
                            fields: Vec::new(),
                        },
                        ty: Ty::Named(def_id, Vec::new()),
                        span,
                    };
                }
                if q.is_single() {
                    let name = q.last().text.clone();
                    if let Some(ty) = self.lookup_var(&name) {
                        HirExpr {
                            kind: HirExprKind::Var(name),
                            ty,
                            span,
                        }
                    } else if let Some(path) = self.resolve_def_path(&name) {
                        let ty = self.ty_for_def_path(&path);
                        HirExpr {
                            kind: HirExprKind::Var(path),
                            ty,
                            span,
                        }
                    } else {
                        HirExpr {
                            kind: HirExprKind::Var(name),
                            ty: Ty::Error,
                            span,
                        }
                    }
                } else {
                    if let Some(local_field) = self.lower_local_field_path(q, tp) {
                        return local_field;
                    }
                    let resolved = self
                        .resolve_def_path(&path)
                        .unwrap_or_else(|| SmolStr::new(&path));
                    let ty = self.ty_for_def_path(&resolved);
                    HirExpr {
                        kind: HirExprKind::Var(resolved),
                        ty,
                        span,
                    }
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let l = self.lower_expr(lhs, tp);
                let r = self.lower_expr(rhs, tp);
                if let Some(overloaded) =
                    self.lower_operator_overload(*op, l.clone(), r.clone(), span)
                {
                    return overloaded;
                }
                let ty = binary_result_ty(*op, &l.ty, &r.ty);
                HirExpr {
                    kind: HirExprKind::Binary {
                        op: *op,
                        lhs: Box::new(l),
                        rhs: Box::new(r),
                    },
                    ty,
                    span,
                }
            }
            Expr::Unary { op, operand, .. } => {
                let e = self.lower_expr(operand, tp);
                let ty = e.ty.clone();
                HirExpr {
                    kind: HirExprKind::Unary {
                        op: *op,
                        operand: Box::new(e),
                    },
                    ty,
                    span,
                }
            }
            Expr::Field { object, field, .. } => {
                if let Some(path) = expr_to_qualified_path(expr) {
                    let first_part = path.split('.').next().unwrap_or(&path);
                    if self.lookup_var(first_part).is_none() {
                        if let Some(c_name) = self.resolve_stdlib(&path) {
                            return HirExpr {
                                kind: HirExprKind::Var(SmolStr::new(c_name)),
                                ty: stdlib_c_func_ty(c_name, Some(self.def_map)),
                                span,
                            };
                        }
                    }
                }
                let obj = self.lower_expr(object, tp);
                let ty = if let Ty::Named(def_id, _) = &obj.ty {
                    self.struct_field_ty(*def_id, field.as_str())
                        .unwrap_or(Ty::Infer(0))
                } else {
                    Ty::Infer(0)
                };
                HirExpr {
                    kind: HirExprKind::Field {
                        object: Box::new(obj),
                        field: field.text.clone(),
                    },
                    ty,
                    span,
                }
            }
            Expr::TupleIndex { object, index, .. } => {
                let obj = self.lower_expr(object, tp);
                let ty = if let Ty::Tuple(elems) = &obj.ty {
                    elems.get(*index as usize).cloned().unwrap_or(Ty::Error)
                } else {
                    Ty::Infer(0)
                };
                HirExpr {
                    kind: HirExprKind::TupleIndex {
                        object: Box::new(obj),
                        index: *index,
                    },
                    ty,
                    span,
                }
            }
            Expr::Index { object, index, .. } => {
                let obj = self.lower_expr(object, tp);
                match index {
                    ori_ast::expr::IndexExpr::Single(index) => {
                        let idx = self.lower_expr(index, tp);
                        let ty = index_result_ty(&obj.ty);
                        HirExpr {
                            kind: HirExprKind::Index {
                                object: Box::new(obj),
                                index: Box::new(idx),
                            },
                            ty,
                            span,
                        }
                    }
                    ori_ast::expr::IndexExpr::Range { start, end } => {
                        // Desugar `obj[a..b]` to a method call on the underlying type.
                        // For strings: ori_string_slice(obj, a, b)
                        // For lists: runtime slice (future)
                        let start_h = start.as_ref().map(|e| self.lower_expr(e, tp));
                        let end_h = end.as_ref().map(|e| self.lower_expr(e, tp));
                        let s = start_h.unwrap_or(HirExpr {
                            kind: HirExprKind::IntLit(0),
                            ty: Ty::Int,
                            span,
                        });
                        // For end, use a large sentinel if not specified
                        let e = end_h.unwrap_or(HirExpr {
                            kind: HirExprKind::IntLit(i64::MAX),
                            ty: Ty::Int,
                            span,
                        });
                        let result_ty = obj.ty.clone();
                        HirExpr {
                            kind: HirExprKind::MethodCall {
                                receiver: Box::new(obj),
                                method: SmolStr::new("__slice"),
                                args: vec![s, e],
                            },
                            ty: result_ty,
                            span,
                        }
                    }
                }
            }
            Expr::Call { callee, args, span } => {
                // Desugar .or() method on optional/result
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    if field.text.as_str() == "or" && args.len() == 1 {
                        let obj = self.lower_expr(object, tp);
                        let fallback = &args[0];
                        let fallback_expr = match &fallback.value {
                            ori_ast::expr::ArgValue::Expr(e)
                            | ori_ast::expr::ArgValue::Spread(e) => e,
                        };
                        let fallback_val = self.lower_expr(fallback_expr, tp);
                        return lower_optional_or_result_or(obj, fallback_val, *span);
                    }
                    if field.text.as_str() == "or_wrap" && args.len() == 1 {
                        let obj = self.lower_expr(object, tp);
                        let context = &args[0];
                        let context_expr = match &context.value {
                            ori_ast::expr::ArgValue::Expr(e)
                            | ori_ast::expr::ArgValue::Spread(e) => e,
                        };
                        let context_val = self.lower_expr(context_expr, tp);
                        return lower_result_or_wrap(obj, context_val, *span);
                    }
                    if field.text.as_str() == "or_return" && args.is_empty() {
                        // Desugar .or_return() to propagation (`try` / compact `?`).
                        let obj = self.lower_expr(object, tp);
                        let inner_ty = match &obj.ty {
                            Ty::Optional(t) => *t.clone(),
                            Ty::Result(ok, _) => *ok.clone(),
                            _ => Ty::Error,
                        };
                        return HirExpr {
                            kind: HirExprKind::Propagate(Box::new(obj)),
                            ty: inner_ty,
                            span: *span,
                        };
                    }
                }

                // Intercept stdlib calls that might be written as field accesses like `json.parse`
                if let Some(path) = expr_to_qualified_path(callee) {
                    let first_part = path.split('.').next().unwrap_or(&path);
                    if self.lookup_var(first_part).is_none() {
                        if let Some(math_path) = self.resolve_math_overload(&path) {
                            return self.lower_math_overload_call(math_path, args, tp, *span);
                        }
                        if path == "string" {
                            let args_h = self.lower_call_args(args, tp);
                            if let Some(first) = args_h.first() {
                                if matches!(first.value.ty, Ty::String) {
                                    let mut value = first.value.clone();
                                    value.span = *span;
                                    return value;
                                }
                                if let Some(c_name) = string_conversion_c_name(&first.value.ty) {
                                    let sig_ty = stdlib_c_func_ty(c_name, Some(self.def_map));
                                    return HirExpr {
                                        kind: HirExprKind::Call {
                                            callee: Box::new(HirExpr {
                                                kind: HirExprKind::Var(SmolStr::new(c_name)),
                                                ty: sig_ty,
                                                span: callee.span(),
                                            }),
                                            args: args_h,
                                        },
                                        ty: Ty::String,
                                        span: *span,
                                    };
                                }
                                if let Some(display_call) = self.displayable_conversion_expr(
                                    first.value.clone(),
                                    callee.span(),
                                    *span,
                                ) {
                                    return display_call;
                                }
                            }
                        }
                        if let Some(c_name) = self.resolve_stdlib(&path) {
                            let sig_ty = stdlib_c_func_ty(c_name, Some(self.def_map));
                            let args_h = self.lower_call_args(args, tp);
                            let fallback_ret_ty = if let Ty::Func { ret, .. } = &sig_ty {
                                *ret.clone()
                            } else {
                                Ty::Infer(0)
                            };
                            let ret_ty =
                                specialized_stdlib_call_ret_ty(c_name, &args_h, fallback_ret_ty);
                            return HirExpr {
                                kind: HirExprKind::Call {
                                    callee: Box::new(HirExpr {
                                        kind: HirExprKind::Var(SmolStr::new(c_name)),
                                        ty: sig_ty,
                                        span: callee.span(),
                                    }),
                                    args: args_h,
                                },
                                ty: ret_ty,
                                span: *span,
                            };
                        }
                    }
                }

                // Intercept builtin wrapper functions before generic lowering
                if let Expr::QualifiedIdent(q) = callee.as_ref() {
                    let name = q.to_string();
                    if let Some(path) = self.resolve_math_overload(&name) {
                        return self.lower_math_overload_call(path, args, tp, *span);
                    }
                    if name == "string" {
                        let args_h = self.lower_call_args(args, tp);
                        if let Some(first) = args_h.first() {
                            if matches!(first.value.ty, Ty::String) {
                                let mut value = first.value.clone();
                                value.span = *span;
                                return value;
                            }
                            if let Some(c_name) = string_conversion_c_name(&first.value.ty) {
                                let sig_ty = stdlib_c_func_ty(c_name, Some(self.def_map));
                                return HirExpr {
                                    kind: HirExprKind::Call {
                                        callee: Box::new(HirExpr {
                                            kind: HirExprKind::Var(SmolStr::new(c_name)),
                                            ty: sig_ty,
                                            span: callee.span(),
                                        }),
                                        args: args_h,
                                    },
                                    ty: Ty::String,
                                    span: *span,
                                };
                            }
                            if let Some(display_call) = self.displayable_conversion_expr(
                                first.value.clone(),
                                callee.span(),
                                *span,
                            ) {
                                return display_call;
                            }
                        }
                    }
                    if name == "panic" {
                        let args_h = self.lower_call_args(args, tp);
                        return HirExpr {
                            kind: HirExprKind::Call {
                                callee: Box::new(HirExpr {
                                    kind: HirExprKind::Var(SmolStr::new("ori_panic")),
                                    ty: stdlib_c_func_ty("ori_panic", Some(self.def_map)),
                                    span: callee.span(),
                                }),
                                args: args_h,
                            },
                            ty: Ty::Never,
                            span: *span,
                        };
                    }
                    let expanded_name = self.expand_alias(&name);
                    if let Some(intrinsic) = mem_intrinsic(&expanded_name) {
                        let args_h = self.lower_call_args(args, tp);
                        let value_ty = args_h
                            .first()
                            .map(|arg| arg.value.ty.clone())
                            .unwrap_or(Ty::Error);
                        let value = match intrinsic {
                            MemIntrinsic::SizeOf => ori_mem_size_of_ty(&value_ty),
                            MemIntrinsic::AlignOf => ori_mem_align_of_ty(&value_ty),
                        };
                        return HirExpr {
                            kind: HirExprKind::IntLit(value),
                            ty: Ty::Int,
                            span: *span,
                        };
                    }
                    let canonical_name = ori_types::stdlib::canonical_stdlib_path(&expanded_name)
                        .unwrap_or(expanded_name.as_str());
                    match canonical_name {
                        "ori.lazy.once" => {
                            return self.lower_lazy_once_call(args, tp, *span);
                        }
                        "ori.lazy.force" => {
                            return self.lower_lazy_force_call(args, tp, *span);
                        }
                        _ => {}
                    }
                    if let Some(trait_path) = qualified_prefix(q) {
                        if let Some(trait_def_id) =
                            self.resolve_def_id_with_kind(&trait_path, DefKind::Trait)
                        {
                            let args_h = self.lower_call_args(args, tp);
                            if let Some(first_arg) = args_h.first() {
                                if let Ty::Named(type_def_id, _) = &first_arg.value.ty {
                                    if let Some((method_path, return_ty)) = self
                                        .trait_method_func_for_trait_and_type(
                                            trait_def_id,
                                            *type_def_id,
                                            q.last().as_str(),
                                        )
                                    {
                                        let callee_h = HirExpr {
                                            kind: HirExprKind::Var(method_path.clone()),
                                            ty: self.ty_for_def_path(method_path.as_str()),
                                            span: callee.span(),
                                        };
                                        return HirExpr {
                                            kind: HirExprKind::Call {
                                                callee: Box::new(callee_h),
                                                args: args_h,
                                            },
                                            ty: return_ty,
                                            span: *span,
                                        };
                                    }
                                }
                            }
                        }
                    }
                    if q.parts.len() == 2 {
                        let receiver_name = &q.parts[0];
                        let method_name = &q.parts[1];
                        if let Some(receiver_ty) = self.lookup_var(&receiver_name.text) {
                            if let Ty::Any(trait_def_id) = &receiver_ty {
                                if let Some(return_ty) =
                                    self.trait_method_return_ty(*trait_def_id, method_name.as_str())
                                {
                                    let receiver = HirExpr {
                                        kind: HirExprKind::Var(receiver_name.text.clone()),
                                        ty: receiver_ty.clone(),
                                        span: receiver_name.span,
                                    };
                                    let args_h = args
                                        .iter()
                                        .map(|arg| match &arg.value {
                                            ori_ast::expr::ArgValue::Expr(e)
                                            | ori_ast::expr::ArgValue::Spread(e) => {
                                                self.lower_expr(e, tp)
                                            }
                                        })
                                        .collect();
                                    return HirExpr {
                                        kind: HirExprKind::MethodCall {
                                            receiver: Box::new(receiver),
                                            method: method_name.text.clone(),
                                            args: args_h,
                                        },
                                        ty: return_ty,
                                        span: *span,
                                    };
                                }
                            }
                            if matches!(receiver_ty, Ty::Param { .. }) {
                                let receiver = HirExpr {
                                    kind: HirExprKind::Var(receiver_name.text.clone()),
                                    ty: receiver_ty.clone(),
                                    span: receiver_name.span,
                                };
                                let args_h = args
                                    .iter()
                                    .map(|arg| match &arg.value {
                                        ori_ast::expr::ArgValue::Expr(e)
                                        | ori_ast::expr::ArgValue::Spread(e) => {
                                            self.lower_expr(e, tp)
                                        }
                                    })
                                    .collect();
                                return HirExpr {
                                    kind: HirExprKind::MethodCall {
                                        receiver: Box::new(receiver),
                                        method: method_name.text.clone(),
                                        args: args_h,
                                    },
                                    ty: Ty::Infer(0),
                                    span: *span,
                                };
                            }
                            if let Ty::Named(def_id, _) = &receiver_ty {
                                let def = self.def_map.get(*def_id);
                                let method_path =
                                    SmolStr::new(format!("{}.{}", def.path, method_name.text));
                                // Free binds alias `Type.slot` → free fn DefId; use the
                                // canonical def path so codegen calls the free function.
                                let resolved = if let Some(m_def_id) =
                                    self.def_map.lookup(&method_path)
                                {
                                    Some((self.def_map.get(m_def_id).path.clone(), Ty::Infer(0)))
                                } else {
                                    self.trait_method_func_for_type(*def_id, method_name.as_str())
                                };
                                if let Some((method_path, return_ty)) = resolved {
                                    let receiver = HirExpr {
                                        kind: HirExprKind::Var(receiver_name.text.clone()),
                                        ty: receiver_ty.clone(),
                                        span: receiver_name.span,
                                    };
                                    let callee_ty = self.ty_for_def_path(method_path.as_str());
                                    let callee_h = HirExpr {
                                        kind: HirExprKind::Var(method_path),
                                        ty: callee_ty,
                                        span: callee.span(),
                                    };
                                    let mut args_h = vec![HirArg {
                                        label: None,
                                        spread: false,
                                        value: receiver,
                                    }];
                                    args_h.extend(self.lower_call_args(args, tp));
                                    return HirExpr {
                                        kind: HirExprKind::Call {
                                            callee: Box::new(callee_h),
                                            args: args_h,
                                        },
                                        ty: return_ty,
                                        span: *span,
                                    };
                                }
                            }
                            // Check for primitive methods on strings/bytes
                            let m_name = method_name.text.as_str();
                            let namespaces = ["ori.string", "ori.bytes"];
                            for ns in namespaces {
                                if let Some(c_name) = stdlib_c_name(&format!("{ns}.{m_name}")) {
                                    let sig_ty = stdlib_c_func_ty(c_name, Some(self.def_map));
                                    if let Ty::Func { params, .. } = &sig_ty {
                                        if let Some(first_param) = params.first() {
                                            if receiver_ty.is_assignable_to(first_param) {
                                                let receiver = HirExpr {
                                                    kind: HirExprKind::Var(
                                                        receiver_name.text.clone(),
                                                    ),
                                                    ty: receiver_ty.clone(),
                                                    span: receiver_name.span,
                                                };
                                                let callee_h = HirExpr {
                                                    kind: HirExprKind::Var(SmolStr::new(c_name)),
                                                    ty: sig_ty.clone(),
                                                    span: callee.span(),
                                                };
                                                let mut args_h = vec![HirArg {
                                                    label: None,
                                                    spread: false,
                                                    value: receiver,
                                                }];
                                                args_h.extend(self.lower_call_args(args, tp));
                                                return HirExpr {
                                                    kind: HirExprKind::Call {
                                                        callee: Box::new(callee_h),
                                                        args: args_h,
                                                    },
                                                    ty: if let Ty::Func { ret, .. } = sig_ty {
                                                        *ret
                                                    } else {
                                                        Ty::Infer(0)
                                                    },
                                                    span: *span,
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    match name.as_str() {
                        "some" | "Some" => {
                            if let Some(a) = args.first() {
                                let e = match &a.value {
                                    ori_ast::expr::ArgValue::Expr(e)
                                    | ori_ast::expr::ArgValue::Spread(e) => e,
                                };
                                let inner = self.lower_expr(e, tp);
                                let ty = Ty::Optional(Box::new(inner.ty.clone()));
                                return HirExpr {
                                    kind: HirExprKind::Some_(Box::new(inner)),
                                    ty,
                                    span: *span,
                                };
                            }
                        }
                        "ok" | "Ok" => {
                            if let Some(a) = args.first() {
                                let e = match &a.value {
                                    ori_ast::expr::ArgValue::Expr(e)
                                    | ori_ast::expr::ArgValue::Spread(e) => e,
                                };
                                let inner = self.lower_expr(e, tp);
                                let ty =
                                    Ty::Result(Box::new(inner.ty.clone()), Box::new(Ty::String));
                                return HirExpr {
                                    kind: HirExprKind::Ok_(Box::new(inner)),
                                    ty,
                                    span: *span,
                                };
                            } else {
                                let inner = HirExpr {
                                    kind: HirExprKind::Unit,
                                    ty: Ty::Void,
                                    span: *span,
                                };
                                let ty = Ty::Result(Box::new(Ty::Void), Box::new(Ty::String));
                                return HirExpr {
                                    kind: HirExprKind::Ok_(Box::new(inner)),
                                    ty,
                                    span: *span,
                                };
                            }
                        }
                        "err" | "Err" => {
                            if let Some(a) = args.first() {
                                let e = match &a.value {
                                    ori_ast::expr::ArgValue::Expr(e)
                                    | ori_ast::expr::ArgValue::Spread(e) => e,
                                };
                                let inner = self.lower_expr(e, tp);
                                let ty = Ty::Result(Box::new(Ty::Void), Box::new(inner.ty.clone()));
                                return HirExpr {
                                    kind: HirExprKind::Err_(Box::new(inner)),
                                    ty,
                                    span: *span,
                                };
                            }
                        }
                        _ => {}
                    }
                    if let Some(def_id) = self.resolve_def_id_with_kind(&name, DefKind::Struct) {
                        let fields = self.lower_named_args(args, tp);
                        return HirExpr {
                            kind: HirExprKind::StructLit { def_id, fields },
                            ty: Ty::Named(def_id, Vec::new()),
                            span: *span,
                        };
                    }
                    if let Some((def_id, variant)) = self.resolve_enum_variant(q) {
                        let fields = self.lower_named_args(args, tp);
                        return HirExpr {
                            kind: HirExprKind::EnumVariant {
                                def_id,
                                variant,
                                fields,
                            },
                            ty: Ty::Named(def_id, Vec::new()),
                            span: *span,
                        };
                    }
                }
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    let obj_h = self.lower_expr(object, tp);
                    let mut resolved_method = None;
                    let mut resolved_return_ty = Ty::Infer(0);
                    if let Ty::Named(def_id, _) = &obj_h.ty {
                        let def = self.def_map.get(*def_id);
                        let m_path = format!("{}.{}", def.path, field.text);
                        if let Some(m_def_id) = self.def_map.lookup(&m_path) {
                            // Free binds: alias path → free fn; use canonical path.
                            resolved_method = Some(self.def_map.get(m_def_id).path.clone());
                        } else if let Some((m_path, return_ty)) =
                            self.trait_method_func_for_type(*def_id, field.as_str())
                        {
                            resolved_method = Some(m_path);
                            resolved_return_ty = return_ty;
                        }
                    }
                    if let Some(m_path) = resolved_method {
                        let callee_ty = self.ty_for_def_path(m_path.as_str());
                        let callee_h = HirExpr {
                            kind: HirExprKind::Var(m_path),
                            ty: callee_ty,
                            span: callee.span(),
                        };
                        let mut args_h = vec![HirArg {
                            label: None,
                            spread: false,
                            value: obj_h,
                        }];
                        args_h.extend(self.lower_call_args(args, tp));
                        return HirExpr {
                            kind: HirExprKind::Call {
                                callee: Box::new(callee_h),
                                args: args_h,
                            },
                            ty: resolved_return_ty,
                            span: *span,
                        };
                    }
                    if let Ty::Any(trait_def_id) = &obj_h.ty {
                        if let Some(return_ty) =
                            self.trait_method_return_ty(*trait_def_id, field.as_str())
                        {
                            let args_h = args
                                .iter()
                                .map(|arg| match &arg.value {
                                    ori_ast::expr::ArgValue::Expr(e)
                                    | ori_ast::expr::ArgValue::Spread(e) => self.lower_expr(e, tp),
                                })
                                .collect();
                            return HirExpr {
                                kind: HirExprKind::MethodCall {
                                    receiver: Box::new(obj_h),
                                    method: field.text.clone(),
                                    args: args_h,
                                },
                                ty: return_ty,
                                span: *span,
                            };
                        }
                    }
                    if matches!(obj_h.ty, Ty::Param { .. }) {
                        let args_h = args
                            .iter()
                            .map(|arg| match &arg.value {
                                ori_ast::expr::ArgValue::Expr(e)
                                | ori_ast::expr::ArgValue::Spread(e) => self.lower_expr(e, tp),
                            })
                            .collect();
                        return HirExpr {
                            kind: HirExprKind::MethodCall {
                                receiver: Box::new(obj_h),
                                method: field.text.clone(),
                                args: args_h,
                            },
                            ty: Ty::Infer(0),
                            span: *span,
                        };
                    }
                    let m_name = field.text.as_str();
                    let namespaces = ["ori.string", "ori.bytes"];
                    for ns in namespaces {
                        if let Some(c_name) = stdlib_c_name(&format!("{ns}.{m_name}")) {
                            let sig_ty = stdlib_c_func_ty(c_name, Some(self.def_map));
                            if let Ty::Func { params, .. } = &sig_ty {
                                if let Some(first_param) = params.first() {
                                    if obj_h.ty.is_assignable_to(first_param) {
                                        let callee_h = HirExpr {
                                            kind: HirExprKind::Var(SmolStr::new(c_name)),
                                            ty: sig_ty.clone(),
                                            span: callee.span(),
                                        };
                                        let mut args_h = vec![HirArg {
                                            label: None,
                                            spread: false,
                                            value: obj_h,
                                        }];
                                        args_h.extend(self.lower_call_args(args, tp));
                                        return HirExpr {
                                            kind: HirExprKind::Call {
                                                callee: Box::new(callee_h),
                                                args: args_h,
                                            },
                                            ty: if let Ty::Func { ret, .. } = sig_ty {
                                                *ret
                                            } else {
                                                Ty::Infer(0)
                                            },
                                            span: *span,
                                        };
                                    }
                                }
                            }
                        }
                    }
                    // If not resolved as method, fall through to normal lowering
                    let callee_h = HirExpr {
                        kind: HirExprKind::Field {
                            object: Box::new(obj_h),
                            field: field.text.clone(),
                        },
                        ty: Ty::Infer(0),
                        span: callee.span(),
                    };
                    let args_h = self.lower_call_args(args, tp);
                    return HirExpr {
                        kind: HirExprKind::Call {
                            callee: Box::new(callee_h),
                            args: args_h,
                        },
                        ty: Ty::Infer(0),
                        span: *span,
                    };
                }

                let callee_h = self.lower_expr(callee, tp);
                let args_h = self.lower_call_args(args, tp);
                let fallback_ret_ty = match &callee_h.ty {
                    Ty::Func { ret, .. } => *ret.clone(),
                    _ => Ty::Infer(0),
                };
                let ret_ty = match &callee_h.kind {
                    HirExprKind::Var(name) => {
                        specialized_stdlib_call_ret_ty(name.as_str(), &args_h, fallback_ret_ty)
                    }
                    _ => fallback_ret_ty,
                };
                HirExpr {
                    kind: HirExprKind::Call {
                        callee: Box::new(callee_h),
                        args: args_h,
                    },
                    ty: ret_ty,
                    span: *span,
                }
            }
            Expr::Try { expr: inner, .. } => {
                let inner_h = self.lower_expr(inner, tp);
                let ty = unwrap_ty(&inner_h.ty);
                HirExpr {
                    kind: HirExprKind::Propagate(Box::new(inner_h)),
                    ty,
                    span,
                }
            }
            Expr::Await { expr: inner, .. } => {
                let inner_h = self.lower_expr(inner, tp);
                let ty = match &inner_h.ty {
                    Ty::Future(inner_ty) => *inner_ty.clone(),
                    _ => Ty::Infer(0),
                };
                HirExpr {
                    kind: HirExprKind::Await(Box::new(inner_h)),
                    ty,
                    span,
                }
            }
            Expr::Range { start, end, .. } => {
                let s = self.lower_expr(start, tp);
                let e = self.lower_expr(end, tp);
                HirExpr {
                    kind: HirExprKind::Range {
                        start: Box::new(s),
                        end: Box::new(e),
                    },
                    ty: Ty::Range(Box::new(Ty::Int)),
                    span,
                }
            }
            Expr::List { elements, .. } => {
                let elems: Vec<HirExpr> = elements.iter().map(|e| self.lower_expr(e, tp)).collect();
                let elem_ty = elems.first().map(|e| e.ty.clone()).unwrap_or(Ty::Infer(0));
                let ty = Ty::List(Box::new(elem_ty.clone()));
                HirExpr {
                    kind: HirExprKind::ListLit {
                        elem_ty,
                        elements: elems,
                    },
                    ty,
                    span,
                }
            }
            Expr::Tuple { elements, .. } => {
                let elems: Vec<HirExpr> = elements.iter().map(|e| self.lower_expr(e, tp)).collect();
                let tys = elems.iter().map(|e| e.ty.clone()).collect();
                HirExpr {
                    kind: HirExprKind::TupleLit(elems),
                    ty: Ty::Tuple(tys),
                    span,
                }
            }
            Expr::IfExpr {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let cond = self.lower_expr(condition, tp);
                let then = self.lower_expr(then_expr, tp);
                let else_ = self.lower_expr(else_expr, tp);
                let ty = if then.ty.is_never() {
                    else_.ty.clone()
                } else if else_.ty.is_never() {
                    then.ty.clone()
                } else {
                    then.ty.clone()
                };
                HirExpr {
                    kind: HirExprKind::IfExpr {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        else_: Box::new(else_),
                    },
                    ty,
                    span,
                }
            }
            Expr::MatchExpr {
                scrutinee, arms, ..
            } => {
                let scr = self.lower_expr(scrutinee, tp);
                let resolved_scr_ty = replace_json_placeholder_in_ty(scr.ty.clone(), self.def_map);
                let mut hir_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    let pat = match &arm.pattern {
                        Some(pattern) => lower_pattern(pattern, &resolved_scr_ty, self.enum_sigs),
                        None => HirPattern::Wildcard,
                    };
                    self.push();
                    bind_hir_pattern_scope(self, &pat);
                    // Guard and body both read the arm's own bindings.
                    let guard = arm.guard.as_ref().map(|g| self.lower_expr(g, tp));
                    let body = self.lower_expr(&arm.body, tp);
                    self.pop();
                    hir_arms.push(HirExprArm {
                        pattern: pat,
                        guard,
                        body,
                        span: arm.span,
                    });
                }
                // A `never` arm (one that returns) does not decide the result
                // type; take the first arm that produces a real value.
                let ty = hir_arms
                    .iter()
                    .map(|arm| &arm.body.ty)
                    .find(|ty| !ty.is_never())
                    .cloned()
                    .unwrap_or(Ty::Never);
                HirExpr {
                    kind: HirExprKind::MatchExpr {
                        scrutinee: Box::new(scr),
                        arms: hir_arms,
                    },
                    ty,
                    span,
                }
            }
            Expr::StructLit {
                ty: type_name,
                fields,
                ..
            } => {
                let def_id = self
                    .resolve_def_id_with_kind(&type_name.to_string(), DefKind::Struct)
                    .unwrap_or(ori_types::DefId(u32::MAX));
                let ty = Ty::Named(def_id, Vec::new());
                let hfields: Vec<(SmolStr, HirExpr)> = fields
                    .iter()
                    .map(|f| (f.name.text.clone(), self.lower_expr(&f.value, tp)))
                    .collect();
                HirExpr {
                    kind: HirExprKind::StructLit {
                        def_id,
                        fields: hfields,
                    },
                    ty,
                    span,
                }
            }
            Expr::AnonStructLit { fields, .. } => {
                let hfields: Vec<(SmolStr, HirExpr)> = fields
                    .iter()
                    .map(|f| (f.name.text.clone(), self.lower_expr(&f.value, tp)))
                    .collect();
                HirExpr {
                    kind: HirExprKind::StructLit {
                        def_id: ori_types::DefId(u32::MAX),
                        fields: hfields,
                    },
                    ty: Ty::Infer(0),
                    span,
                }
            }
            Expr::EnumVariantUnit {
                ty: type_name,
                variant,
                ..
            } => {
                let def_path = type_name
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                let def_id = self
                    .resolve_def_id_with_kind(&def_path, DefKind::Enum)
                    .unwrap_or(ori_types::DefId(u32::MAX));
                HirExpr {
                    kind: HirExprKind::EnumVariant {
                        def_id,
                        variant: variant.text.clone(),
                        fields: Vec::new(),
                    },
                    ty: Ty::Named(def_id, Vec::new()),
                    span,
                }
            }
            Expr::EnumVariantNamed {
                ty: type_name,
                variant,
                fields,
                ..
            } => {
                let def_path = type_name
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                let def_id = self
                    .resolve_def_id_with_kind(&def_path, DefKind::Enum)
                    .unwrap_or(ori_types::DefId(u32::MAX));
                let hfields: Vec<(SmolStr, HirExpr)> = fields
                    .iter()
                    .map(|f| (f.name.text.clone(), self.lower_expr(&f.value, tp)))
                    .collect();
                HirExpr {
                    kind: HirExprKind::EnumVariant {
                        def_id,
                        variant: variant.text.clone(),
                        fields: hfields,
                    },
                    ty: Ty::Named(def_id, Vec::new()),
                    span,
                }
            }
            // ── Pipe: `value |> func` desugars to `func(value)` ─────────────
            Expr::Pipe { value, func, .. } => {
                let val = self.lower_expr(value, tp);
                let callee = self.lower_expr(func, tp);
                let ret_ty = match &callee.ty {
                    Ty::Func { ret, .. } => *ret.clone(),
                    _ => Ty::Infer(0),
                };
                HirExpr {
                    kind: HirExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![HirArg {
                            label: None,
                            spread: false,
                            value: val,
                        }],
                    },
                    ty: ret_ty,
                    span,
                }
            }

            // ── Map literal: `{k: v, ...}` ──────────────────────────────────
            Expr::Map { entries, .. } => {
                let hentries: Vec<(HirExpr, HirExpr)> = entries
                    .iter()
                    .map(|(k, v)| (self.lower_expr(k, tp), self.lower_expr(v, tp)))
                    .collect();
                let key_ty = hentries
                    .first()
                    .map(|(k, _)| k.ty.clone())
                    .unwrap_or(Ty::Infer(0));
                let value_ty = hentries
                    .first()
                    .map(|(_, v)| v.ty.clone())
                    .unwrap_or(Ty::Infer(0));
                let ty = Ty::Map(Box::new(key_ty.clone()), Box::new(value_ty.clone()));
                HirExpr {
                    kind: HirExprKind::MapLit {
                        key_ty,
                        value_ty,
                        entries: hentries,
                    },
                    ty,
                    span,
                }
            }

            // ── Set literal: `#{a, b, ...}` ─────────────────────────────────
            Expr::Set { elements, .. } => {
                let elems: Vec<HirExpr> = elements.iter().map(|e| self.lower_expr(e, tp)).collect();
                let elem_ty = elems.first().map(|e| e.ty.clone()).unwrap_or(Ty::Infer(0));
                let ty = Ty::Set(Box::new(elem_ty.clone()));
                HirExpr {
                    kind: HirExprKind::SetLit {
                        elem_ty,
                        elements: elems,
                    },
                    ty,
                    span,
                }
            }

            // ── Struct update: `base with { field: value } end` ──────────────
            Expr::StructUpdate { base, updates, .. } => {
                let base_h = self.lower_expr(base, tp);
                let def_id = if let Ty::Named(id, _) = &base_h.ty {
                    *id
                } else {
                    ori_types::DefId(u32::MAX)
                };
                let hupdates: Vec<(SmolStr, HirExpr)> = updates
                    .iter()
                    .map(|f| (f.name.text.clone(), self.lower_expr(&f.value, tp)))
                    .collect();
                HirExpr {
                    kind: HirExprKind::StructUpdate {
                        def_id,
                        base: Box::new(base_h.clone()),
                        updates: hupdates,
                    },
                    ty: base_h.ty.clone(),
                    span,
                }
            }

            // ── `expr is TypeName` — runtime type checking ─
            Expr::IsCheck {
                value,
                ty: check_ty_ast,
                ..
            } => {
                let val = self.lower_expr(value, tp);
                // Resolve the check type. Primitive builtin names (int, string,
                // bool, etc.) are not in the DefMap, so we handle them directly
                // before calling lower_ast_ty, which would otherwise emit a
                // spurious "undefined type" diagnostic for primitives.
                let check_ty = lower_is_target_ty(check_ty_ast, self, tp);
                HirExpr {
                    kind: HirExprKind::IsCheck {
                        value: Box::new(val),
                        check_ty,
                    },
                    ty: Ty::Bool,
                    span,
                }
            }

            Expr::Closure(closure) => self.lower_closure_expr(closure, tp, span),
        }
    }

    fn lower_closure_expr(&mut self, closure: &ClosureExpr, tp: &[SmolStr], span: Span) -> HirExpr {
        let func_name = self.next_closure_name();
        let user_params: Vec<HirParam> = closure
            .params
            .iter()
            .map(|param| HirParam {
                name: param.name.text.clone(),
                ty: param
                    .ty
                    .as_ref()
                    .map(|ty| self.lower_ast_ty(ty, tp))
                    .unwrap_or(Ty::Infer(0)),
                default: None,
                contract: None,
                variadic: false,
                span: param.span,
            })
            .collect();
        let param_names: Vec<SmolStr> =
            user_params.iter().map(|param| param.name.clone()).collect();
        let free_names = collect_closure_free_names(closure, &param_names);
        let captures: Vec<HirClosureCapture> = free_names
            .into_iter()
            .filter_map(|name| {
                self.lookup_var(&name)
                    .map(|ty| HirClosureCapture { name, ty })
            })
            .collect();

        let declared_ret = closure
            .return_ty
            .as_ref()
            .map(|ty| self.lower_ast_ty(ty, tp));
        let previous_ret = self.ret_ty.clone();

        self.push();
        for capture in &captures {
            self.bind(capture.name.clone(), capture.ty.clone());
        }
        for param in &user_params {
            self.bind(param.name.clone(), param.ty.clone());
        }
        self.ret_ty = declared_ret.clone().unwrap_or(Ty::Infer(0));

        let (body, return_ty) = match &closure.body {
            ClosureBody::Expr(expr) => {
                let mut value = self.lower_expr(expr, tp);
                if let Some(expected) = &declared_ret {
                    apply_expected_expr_ty(&mut value, expected);
                }
                let return_ty = declared_ret.clone().unwrap_or_else(|| value.ty.clone());
                let body = HirBlock {
                    stmts: vec![HirStmt::Return(Some(value), span)],
                    span,
                };
                (body, return_ty)
            }
            ClosureBody::Block(block) => {
                let return_ty = declared_ret.clone().unwrap_or(Ty::Void);
                self.ret_ty = return_ty.clone();
                (self.lower_block(block, tp), return_ty)
            }
        };
        self.ret_ty = previous_ret;
        self.pop();

        let mut synthetic_params = Vec::with_capacity(user_params.len() + 1);
        synthetic_params.push(HirParam {
            name: SmolStr::new("__env"),
            ty: Ty::Bytes,
            default: None,
            contract: None,
            variadic: false,
            span,
        });
        synthetic_params.extend(user_params.clone());

        let def_seed = u32::MAX.saturating_sub(self.closure_counter as u32);
        self.generated_funcs.push(HirFunc {
            def_id: ori_types::DefId(def_seed),
            name: func_name.clone(),
            params: synthetic_params,
            return_ty: return_ty.clone(),
            body,
            closure_captures: captures.clone(),
            is_public: false,
            is_async: false,
            is_mut: false,
            c_export_name: None,
            span,
        });

        HirExpr {
            kind: HirExprKind::Closure {
                func_name,
                captures,
            },
            ty: Ty::Func {
                params: user_params.into_iter().map(|param| param.ty).collect(),
                ret: Box::new(return_ty),
            },
            span,
        }
    }
}

// ── Pattern lowering ──────────────────────────────────────────────────────────

fn collect_closure_free_names(closure: &ClosureExpr, params: &[SmolStr]) -> Vec<SmolStr> {
    let mut bound: HashSet<SmolStr> = params.iter().cloned().collect();
    let mut names = Vec::new();
    match &closure.body {
        ClosureBody::Expr(expr) => collect_free_names_expr(expr, &mut bound, &mut names),
        ClosureBody::Block(block) => collect_free_names_block(block, &mut bound, &mut names),
    }
    names
}

fn push_free_name(name: &SmolStr, bound: &HashSet<SmolStr>, names: &mut Vec<SmolStr>) {
    if !bound.contains(name) && !names.contains(name) {
        names.push(name.clone());
    }
}

fn collect_free_names_block(block: &Block, bound: &mut HashSet<SmolStr>, names: &mut Vec<SmolStr>) {
    for stmt in &block.stmts {
        collect_free_names_stmt(stmt, bound, names);
    }
}

fn collect_free_names_stmt(stmt: &Stmt, bound: &mut HashSet<SmolStr>, names: &mut Vec<SmolStr>) {
    match stmt {
        Stmt::Const(local) => {
            collect_free_names_expr(&local.value, bound, names);
            bound.insert(local.name.text.clone());
        }
        Stmt::Var(local) => {
            collect_free_names_expr(&local.value, bound, names);
            bound.insert(local.name.text.clone());
        }
        Stmt::Assign(assign) => {
            collect_free_names_lvalue(&assign.lvalue, bound, names);
            collect_free_names_expr(&assign.value, bound, names);
        }
        Stmt::CompoundAssign(assign) => {
            collect_free_names_lvalue(&assign.lvalue, bound, names);
            collect_free_names_expr(&assign.value, bound, names);
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                collect_free_names_expr(value, bound, names);
            }
        }
        Stmt::Expr(expr) => collect_free_names_expr(expr, bound, names),
        Stmt::If(if_stmt) => {
            collect_free_names_expr(&if_stmt.condition, bound, names);
            collect_free_names_nested_block(&if_stmt.then_block, bound, names);
            for (cond, block) in &if_stmt.else_ifs {
                collect_free_names_expr(cond, bound, names);
                collect_free_names_nested_block(block, bound, names);
            }
            if let Some(block) = &if_stmt.else_block {
                collect_free_names_nested_block(block, bound, names);
            }
        }
        Stmt::IfSome(if_some) => {
            collect_free_names_expr(&if_some.value, bound, names);
            let mut nested = bound.clone();
            nested.insert(if_some.binding.text.clone());
            collect_free_names_block(&if_some.then_block, &mut nested, names);
            if let Some(block) = &if_some.else_block {
                collect_free_names_nested_block(block, bound, names);
            }
        }
        Stmt::While(while_stmt) => {
            collect_free_names_expr(&while_stmt.condition, bound, names);
            collect_free_names_nested_block(&while_stmt.body, bound, names);
        }
        Stmt::WhileSome(while_some) => {
            collect_free_names_expr(&while_some.value, bound, names);
            let mut nested = bound.clone();
            nested.insert(while_some.binding.text.clone());
            collect_free_names_block(&while_some.body, &mut nested, names);
        }
        Stmt::For(for_stmt) => {
            collect_free_names_expr(&for_stmt.iterable, bound, names);
            let mut nested = bound.clone();
            nested.insert(for_stmt.binding.text.clone());
            if let Some(second) = &for_stmt.second_binding {
                nested.insert(second.text.clone());
            }
            collect_free_names_block(&for_stmt.body, &mut nested, names);
        }
        Stmt::Repeat(repeat) => {
            collect_free_names_expr(&repeat.count, bound, names);
            collect_free_names_nested_block(&repeat.body, bound, names);
        }
        Stmt::Loop(loop_stmt) => collect_free_names_nested_block(&loop_stmt.body, bound, names),
        Stmt::Match(match_stmt) => {
            collect_free_names_expr(&match_stmt.scrutinee, bound, names);
            for case in &match_stmt.cases {
                let mut nested = bound.clone();
                match case {
                    MatchCase::Pattern {
                        pattern,
                        guard,
                        body,
                        ..
                    } => {
                        bind_pattern_names(pattern, &mut nested);
                        if let Some(guard) = guard {
                            collect_free_names_expr(guard, &mut nested, names);
                        }
                        for stmt in body {
                            collect_free_names_stmt(stmt, &mut nested, names);
                        }
                    }
                    MatchCase::Else { body, .. } => {
                        for stmt in body {
                            collect_free_names_stmt(stmt, &mut nested, names);
                        }
                    }
                }
            }
        }
        Stmt::Using(using) => {
            collect_free_names_expr(&using.value, bound, names);
            bound.insert(using.name.text.clone());
        }
        Stmt::Check(check) => collect_free_names_expr(&check.condition, bound, names),
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_free_names_nested_block(
    block: &Block,
    bound: &HashSet<SmolStr>,
    names: &mut Vec<SmolStr>,
) {
    let mut nested = bound.clone();
    collect_free_names_block(block, &mut nested, names);
}

fn collect_free_names_lvalue(
    lvalue: &ori_ast::stmt::LValue,
    bound: &HashSet<SmolStr>,
    names: &mut Vec<SmolStr>,
) {
    match lvalue {
        ori_ast::stmt::LValue::Ident(name) => push_free_name(&name.text, bound, names),
        ori_ast::stmt::LValue::Field { base, .. } => collect_free_names_lvalue(base, bound, names),
        ori_ast::stmt::LValue::Index { base, index, .. } => {
            collect_free_names_lvalue(base, bound, names);
            let mut nested = bound.clone();
            collect_free_names_expr(index, &mut nested, names);
        }
    }
}

fn collect_free_names_expr(expr: &Expr, bound: &mut HashSet<SmolStr>, names: &mut Vec<SmolStr>) {
    match expr {
        Expr::Ident(name) => push_free_name(&name.text, bound, names),
        Expr::QualifiedIdent(q) if q.is_single() => push_free_name(&q.last().text, bound, names),
        Expr::SelfExpr(_) => push_free_name(&SmolStr::new("self"), bound, names),
        Expr::Range { start, end, .. } => {
            collect_free_names_expr(start, bound, names);
            collect_free_names_expr(end, bound, names);
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } | Expr::Tuple { elements, .. } => {
            for item in elements {
                collect_free_names_expr(item, bound, names);
            }
        }
        Expr::Map { entries, .. } => {
            for (key, value) in entries {
                collect_free_names_expr(key, bound, names);
                collect_free_names_expr(value, bound, names);
            }
        }
        Expr::StructLit { fields, .. } | Expr::AnonStructLit { fields, .. } => {
            for field in fields {
                collect_free_names_expr(&field.value, bound, names);
            }
        }
        Expr::EnumVariantNamed { fields, .. } => {
            for field in fields {
                collect_free_names_expr(&field.value, bound, names);
            }
        }
        Expr::Unary { operand, .. }
        | Expr::Try { expr: operand, .. }
        | Expr::Await { expr: operand, .. } => {
            collect_free_names_expr(operand, bound, names);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_free_names_expr(lhs, bound, names);
            collect_free_names_expr(rhs, bound, names);
        }
        Expr::Field { object, .. } | Expr::TupleIndex { object, .. } => {
            collect_free_names_expr(object, bound, names);
        }
        Expr::Call { callee, args, .. } => {
            collect_free_names_expr(callee, bound, names);
            for arg in args {
                match &arg.value {
                    ori_ast::expr::ArgValue::Expr(expr) | ori_ast::expr::ArgValue::Spread(expr) => {
                        collect_free_names_expr(expr, bound, names);
                    }
                }
            }
        }
        Expr::Index { object, index, .. } => {
            collect_free_names_expr(object, bound, names);
            match index {
                ori_ast::expr::IndexExpr::Single(index) => {
                    collect_free_names_expr(index, bound, names);
                }
                ori_ast::expr::IndexExpr::Range { start, end } => {
                    if let Some(start) = start {
                        collect_free_names_expr(start, bound, names);
                    }
                    if let Some(end) = end {
                        collect_free_names_expr(end, bound, names);
                    }
                }
            }
        }
        Expr::Pipe { value, func, .. } => {
            collect_free_names_expr(value, bound, names);
            collect_free_names_expr(func, bound, names);
        }
        Expr::IfExpr {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            collect_free_names_expr(condition, bound, names);
            collect_free_names_expr(then_expr, bound, names);
            collect_free_names_expr(else_expr, bound, names);
        }
        Expr::MatchExpr {
            scrutinee, arms, ..
        } => {
            collect_free_names_expr(scrutinee, bound, names);
            for arm in arms {
                // Pattern bindings are local to the arm: shadow them for the
                // guard and body so they are not reported as captures.
                let mut nested = bound.clone();
                if let Some(pattern) = &arm.pattern {
                    bind_pattern_names(pattern, &mut nested);
                }
                if let Some(guard) = &arm.guard {
                    collect_free_names_expr(guard, &mut nested, names);
                }
                collect_free_names_expr(&arm.body, &mut nested, names);
            }
        }
        Expr::FStrLit { parts, .. } => {
            for part in parts {
                if let FStrPart::Interpolated(expr) = part {
                    collect_free_names_expr(expr, bound, names);
                }
            }
        }
        Expr::StructUpdate { base, updates, .. } => {
            collect_free_names_expr(base, bound, names);
            for update in updates {
                collect_free_names_expr(&update.value, bound, names);
            }
        }
        Expr::IsCheck { value, .. } => collect_free_names_expr(value, bound, names),
        Expr::Closure(_) => {}
        Expr::BoolLit(..)
        | Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::StrLit { .. }
        | Expr::BytesLit { .. }
        | Expr::None(_)
        | Expr::EnumVariantUnit { .. }
        | Expr::QualifiedIdent(_) => {}
    }
}

fn bind_pattern_names(pattern: &ori_ast::pattern::Pattern, bound: &mut HashSet<SmolStr>) {
    use ori_ast::pattern::Pattern;
    match pattern {
        Pattern::Binding(name) => {
            bound.insert(name.text.clone());
        }
        // Alternatives cannot bind (checker-enforced), but walking them keeps
        // this function correct if that rule is ever relaxed.
        Pattern::Or(alternatives, _) => {
            for alternative in alternatives {
                bind_pattern_names(alternative, bound);
            }
        }
        Pattern::Some(inner, _) | Pattern::Ok(inner, _) | Pattern::Err(inner, _) => {
            bind_pattern_names(inner, bound);
        }
        Pattern::VariantNamed { fields, .. } => {
            for field in fields {
                bind_pattern_names(&field.pattern, bound);
            }
        }
        Pattern::Tuple(items, _) => {
            for item in items {
                bind_pattern_names(item, bound);
            }
        }
        Pattern::Wildcard(_)
        | Pattern::Literal(_)
        | Pattern::None(_)
        | Pattern::VariantUnit { .. } => {}
    }
}

fn lower_pattern(
    pat: &ori_ast::pattern::Pattern,
    scr_ty: &Ty,
    enum_sigs: &[EnumSig],
) -> HirPattern {
    use ori_ast::pattern::Pattern;
    match pat {
        Pattern::Wildcard(_) => HirPattern::Wildcard,
        Pattern::Or(alternatives, _) => HirPattern::Or(
            alternatives
                .iter()
                .map(|alt| lower_pattern(alt, scr_ty, enum_sigs))
                .collect(),
        ),
        Pattern::Binding(n) => {
            let mut resolved_def_id = None;
            if let Ty::Named(def_id, _) = scr_ty {
                resolved_def_id = Some(*def_id);
            }
            if let Some(def_id) = resolved_def_id {
                if let Some(sig) = enum_sigs.iter().find(|sig| sig.def_id == def_id) {
                    if let Some(variant_sig) = sig.variants.iter().find(|v| v.name == n.text) {
                        if variant_sig.fields.is_empty() {
                            return HirPattern::Variant {
                                def_id,
                                variant: n.text.clone(),
                                fields: Vec::new(),
                            };
                        }
                    }
                }
            }
            HirPattern::Binding(n.text.clone(), scr_ty.clone())
        }
        Pattern::None(_) => HirPattern::None_,
        Pattern::Some(p, _) => {
            let inner_ty = if let Ty::Optional(inner) = scr_ty {
                &**inner
            } else {
                &Ty::Infer(0)
            };
            HirPattern::Some_(Box::new(lower_pattern(p, inner_ty, enum_sigs)))
        }
        Pattern::Ok(p, _) => {
            let ok_ty = if let Ty::Result(ok, _) = scr_ty {
                &**ok
            } else {
                &Ty::Infer(0)
            };
            HirPattern::Ok_(Box::new(lower_pattern(p, ok_ty, enum_sigs)))
        }
        Pattern::Err(p, _) => {
            let err_ty = if let Ty::Result(_, err) = scr_ty {
                &**err
            } else {
                &Ty::Infer(0)
            };
            HirPattern::Err_(Box::new(lower_pattern(p, err_ty, enum_sigs)))
        }
        Pattern::Literal(e) => match e.as_ref() {
            Expr::BoolLit(b, _) => HirPattern::BoolLit(*b),
            Expr::IntLit { raw, .. } => HirPattern::IntLit(parse_int_lit(raw)),
            Expr::StrLit { value, .. } => HirPattern::StrLit(value.clone()),
            _ => HirPattern::Wildcard,
        },
        Pattern::Tuple(pats, _) => {
            let elem_tys = if let Ty::Tuple(elems) = scr_ty {
                elems.clone()
            } else {
                vec![Ty::Infer(0); pats.len()]
            };
            HirPattern::Tuple(
                pats.iter()
                    .zip(elem_tys.iter())
                    .map(|(pat, ty)| lower_pattern(pat, ty, enum_sigs))
                    .collect(),
            )
        }
        Pattern::VariantUnit { name, .. } => {
            let mut resolved_def_id = None;
            if let Ty::Named(def_id, _) = scr_ty {
                resolved_def_id = Some(*def_id);
            } else if let Ty::Infer(_) = scr_ty {
                if let Some(sig) = enum_sigs
                    .iter()
                    .find(|sig| sig.variants.iter().any(|variant| variant.name == name.text))
                {
                    resolved_def_id = Some(sig.def_id);
                }
            }

            if let Some(def_id) = resolved_def_id {
                HirPattern::Variant {
                    def_id,
                    variant: name.text.clone(),
                    fields: Vec::new(),
                }
            } else {
                HirPattern::Binding(name.text.clone(), Ty::Infer(0))
            }
        }
        Pattern::VariantNamed { name, fields, .. } => {
            let mut resolved_def_id = None;
            if let Ty::Named(def_id, _) = scr_ty {
                resolved_def_id = Some(*def_id);
            } else if let Ty::Infer(_) = scr_ty {
                if let Some(sig) = enum_sigs
                    .iter()
                    .find(|sig| sig.variants.iter().any(|variant| variant.name == name.text))
                {
                    resolved_def_id = Some(sig.def_id);
                }
            }

            if let Some(def_id) = resolved_def_id {
                let variant_sig =
                    enum_sigs
                        .iter()
                        .find(|sig| sig.def_id == def_id)
                        .and_then(|sig| {
                            sig.variants
                                .iter()
                                .find(|variant| variant.name == name.text)
                        });
                HirPattern::Variant {
                    def_id,
                    variant: name.text.clone(),
                    fields: fields
                        .iter()
                        .map(|field| {
                            let field_ty = variant_sig
                                .and_then(|variant| {
                                    variant
                                        .fields
                                        .iter()
                                        .find(|(field_name, _)| *field_name == field.name.text)
                                })
                                .map(|(_, ty)| ty.clone())
                                .unwrap_or(Ty::Infer(0));
                            (
                                field.name.text.clone(),
                                lower_pattern(&field.pattern, &field_ty, enum_sigs),
                            )
                        })
                        .collect(),
                }
            } else {
                HirPattern::Wildcard
            }
        }
    }
}

fn bind_hir_pattern_scope(lowerer: &mut Lowerer<'_>, pat: &HirPattern) {
    match pat {
        HirPattern::Binding(name, ty) => lowerer.bind(name.clone(), ty.clone()),
        HirPattern::Some_(inner) | HirPattern::Ok_(inner) | HirPattern::Err_(inner) => {
            bind_hir_pattern_scope(lowerer, inner);
        }
        HirPattern::Variant { fields, .. } => {
            for (_, pat) in fields {
                bind_hir_pattern_scope(lowerer, pat);
            }
        }
        HirPattern::Tuple(patterns) => {
            for pat in patterns {
                bind_hir_pattern_scope(lowerer, pat);
            }
        }
        _ => {}
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn qualified_prefix(q: &ori_ast::common::QualifiedName) -> Option<String> {
    if q.parts.len() < 2 {
        return None;
    }
    Some(
        q.parts[..q.parts.len() - 1]
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn parse_int_lit(raw: &str) -> i64 {
    parse_int_literal(raw)
        .map(|parsed| parsed.value)
        .unwrap_or_else(|err| {
            eprintln!(
                "ori: ICE: invalid integer literal reached HIR lowering: {}",
                err.message
            );
            0
        })
}

fn parse_float_lit(raw: &str) -> ori_types::literal::ParsedFloatLiteral {
    parse_float_literal(raw).unwrap_or_else(|err| {
        eprintln!(
            "ori: ICE: invalid float literal reached HIR lowering: {}",
            err.message
        );
        ori_types::literal::ParsedFloatLiteral {
            value: 0.0,
            ty: Ty::Float,
        }
    })
}

fn for_second_binding_ty(ty: &Ty) -> Ty {
    match ty {
        Ty::Map(_, value) => *value.clone(),
        _ => Ty::Int,
    }
}

fn index_result_ty(ty: &Ty) -> Ty {
    match ty {
        Ty::List(t) | Ty::Set(t) => *t.clone(),
        Ty::Map(_, v) => *v.clone(),
        Ty::String => Ty::String,
        Ty::Tuple(elems) => elems.first().cloned().unwrap_or(Ty::Infer(0)),
        _ => Ty::Infer(0),
    }
}

fn apply_expected_expr_ty(expr: &mut HirExpr, expected: &Ty) {
    match (&mut expr.kind, expected) {
        (HirExprKind::None_, Ty::Optional(_))
        | (HirExprKind::Some_(_), Ty::Optional(_))
        | (HirExprKind::Ok_(_), Ty::Result(_, _))
        | (HirExprKind::Err_(_), Ty::Result(_, _)) => {
            expr.ty = expected.clone();
        }
        (HirExprKind::StructLit { def_id, .. }, Ty::Named(expected_def_id, _))
            if *def_id == DefId(u32::MAX) =>
        {
            *def_id = *expected_def_id;
            expr.ty = expected.clone();
        }
        _ => {}
    }
}

fn unwrap_ty(ty: &Ty) -> Ty {
    match ty {
        Ty::Optional(t) => *t.clone(),
        Ty::Result(ok, _) => *ok.clone(),
        _ => ty.clone(),
    }
}

/// Resolve `@c_export` / `@c_export("sym")` for free functions (cdylib exports).
fn c_export_name_from_attrs(attrs: &[Attr], default_name: &str) -> Option<SmolStr> {
    for attr in attrs {
        if attr.name.text.as_str() != "c_export" {
            continue;
        }
        match attr.args.as_slice() {
            [] => return Some(SmolStr::new(default_name)),
            [AttrArg::String(name, _)] if !name.is_empty() => return Some(name.clone()),
            _ => return Some(SmolStr::new(default_name)),
        }
    }
    None
}

fn async_return_ty(is_async: bool, inner: Ty) -> Ty {
    if is_async {
        Ty::Future(Box::new(inner))
    } else {
        inner
    }
}

fn binary_result_ty(op: ori_ast::expr::BinaryOp, lty: &Ty, _rty: &Ty) -> Ty {
    use ori_ast::expr::BinaryOp::*;
    match op {
        Add | Sub | Mul | Div | Rem => lty.clone(),
        Eq | Ne | Lt | Le | Gt | Ge | And | Or => Ty::Bool,
    }
}

fn operator_trait_method(op: BinaryOp) -> Option<(&'static str, &'static str)> {
    match op {
        BinaryOp::Add => Some(("Addable", "add")),
        BinaryOp::Sub => Some(("Subtractable", "subtract")),
        BinaryOp::Mul => Some(("Multiplicable", "multiply")),
        BinaryOp::Div => Some(("Divisible", "divide")),
        BinaryOp::Eq | BinaryOp::Ne => Some(("Equatable", "equals")),
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            Some(("Comparable", "compare"))
        }
        _ => None,
    }
}

fn lower_lvalue(lv: &ori_ast::stmt::LValue, lowerer: &mut Lowerer, tp: &[SmolStr]) -> HirLValue {
    use ori_ast::stmt::LValue;
    match lv {
        LValue::Ident(n) => {
            if lowerer.lookup_var(&n.text).is_some() {
                HirLValue::Var(n.text.clone())
            } else if let Some(path) = lowerer.resolve_def_path(&n.text) {
                HirLValue::Var(path)
            } else {
                HirLValue::Var(n.text.clone())
            }
        }
        LValue::Field { base, field, .. } => HirLValue::Field {
            base: Box::new(lower_lvalue(base, lowerer, tp)),
            field: field.text.clone(),
        },
        LValue::Index { base, index, .. } => HirLValue::Index {
            base: Box::new(lower_lvalue(base, lowerer, tp)),
            index: Box::new(lowerer.lower_expr(index, tp)),
        },
    }
}

fn has_explicit_self_param(params: &[ori_ast::item::Param]) -> bool {
    params
        .first()
        .is_some_and(|param| param.name.text.as_str() == "self")
}

fn lvalue_to_expr(lv: &HirLValue, span: Span) -> HirExpr {
    match lv {
        HirLValue::Var(name) => HirExpr {
            kind: HirExprKind::Var(name.clone()),
            ty: Ty::Infer(0),
            span,
        },
        HirLValue::Field { base, field } => {
            let obj = lvalue_to_expr(base, span);
            HirExpr {
                kind: HirExprKind::Field {
                    object: Box::new(obj),
                    field: field.clone(),
                },
                ty: Ty::Infer(0),
                span,
            }
        }
        HirLValue::Index { base, index } => {
            let obj = lvalue_to_expr(base, span);
            HirExpr {
                kind: HirExprKind::Index {
                    object: Box::new(obj),
                    index: index.clone(),
                },
                ty: Ty::Infer(0),
                span,
            }
        }
    }
}

fn compound_op_to_binary(op: ori_ast::stmt::CompoundOp) -> ori_ast::expr::BinaryOp {
    use ori_ast::expr::BinaryOp;
    use ori_ast::stmt::CompoundOp;
    match op {
        CompoundOp::Add => BinaryOp::Add,
        CompoundOp::Sub => BinaryOp::Sub,
        CompoundOp::Mul => BinaryOp::Mul,
        CompoundOp::Div => BinaryOp::Div,
    }
}

/// Resolve the type after `is` in an `IsCheck` expression.
///
/// Primitive builtin type names (e.g. `int`, `string`, `bool`) are not
/// registered in the `DefMap`, so we map them directly to their `Ty` variants
/// instead of calling `lower_ast_ty` which would emit a spurious error.
fn lower_is_target_ty(
    name: &ori_ast::common::QualifiedName,
    l: &mut Lowerer,
    tp: &[SmolStr],
) -> Ty {
    if name.is_single() {
        let s = name.last().text.as_str();
        match s {
            "bool" => return Ty::Bool,
            "int" => return Ty::Int,
            "int8" => return Ty::Int8,
            "int16" => return Ty::Int16,
            "int32" => return Ty::Int32,
            "int64" => return Ty::Int64,
            "u8" => return Ty::U8,
            "u16" => return Ty::U16,
            "u32" => return Ty::U32,
            "u64" => return Ty::U64,
            "float" => return Ty::Float,
            "float32" => return Ty::Float32,
            "float64" => return Ty::Float64,
            "string" => return Ty::String,
            "bytes" => return Ty::Bytes,
            _ => {}
        }
    }
    let ty_node = ori_ast::ty::Type::Named(name.clone());
    l.lower_ast_ty(&ty_node, tp)
}

/// Desugar `.or(fallback)` on `optional[T]` or `result[T,E]`.
/// Emits an internal intrinsic call handled directly by native and C codegen.
fn lower_optional_or_result_or(obj: HirExpr, fallback: HirExpr, span: Span) -> HirExpr {
    let inner_ty = match &obj.ty {
        Ty::Optional(t) => *t.clone(),
        Ty::Result(ok, _) => *ok.clone(),
        _ => Ty::Error,
    };
    HirExpr {
        kind: HirExprKind::Call {
            callee: Box::new(HirExpr {
                kind: HirExprKind::Var(SmolStr::new("__ori_builtin_or")),
                ty: Ty::Func {
                    params: vec![obj.ty.clone(), fallback.ty.clone()],
                    ret: Box::new(inner_ty.clone()),
                },
                span: Span::DUMMY,
            }),
            args: vec![
                HirArg {
                    label: None,
                    spread: false,
                    value: obj,
                },
                HirArg {
                    label: None,
                    spread: false,
                    value: fallback,
                },
            ],
        },
        ty: inner_ty,
        span,
    }
}

/// Desugar `.or_wrap(context)` on `result[T, string]`.
/// Emits an internal intrinsic call handled directly by native and C codegen.
fn lower_result_or_wrap(obj: HirExpr, context: HirExpr, span: Span) -> HirExpr {
    let result_ty = obj.ty.clone();
    HirExpr {
        kind: HirExprKind::Call {
            callee: Box::new(HirExpr {
                kind: HirExprKind::Var(SmolStr::new("__ori_builtin_or_wrap")),
                ty: Ty::Func {
                    params: vec![obj.ty.clone(), context.ty.clone()],
                    ret: Box::new(result_ty.clone()),
                },
                span: Span::DUMMY,
            }),
            args: vec![
                HirArg {
                    label: None,
                    spread: false,
                    value: obj,
                },
                HirArg {
                    label: None,
                    spread: false,
                    value: context,
                },
            ],
        },
        ty: result_ty,
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_types::stdlib::stdlib_runtime_functions;

    #[test]
    fn stdlib_manifest_paths_lower_to_declared_runtime_symbols() {
        for entry in stdlib_runtime_functions() {
            assert_eq!(
                stdlib_c_name(entry.canonical_path),
                Some(entry.runtime_symbol),
                "{}",
                entry.canonical_path
            );
            for alias in entry.aliases {
                assert_eq!(stdlib_c_name(alias), Some(entry.runtime_symbol), "{alias}");
            }
            assert!(
                matches!(
                    stdlib_c_func_ty(entry.runtime_symbol, None),
                    Ty::Func { .. }
                ),
                "{} -> {} has no HIR stdlib function type",
                entry.canonical_path,
                entry.runtime_symbol
            );
        }
    }
}
