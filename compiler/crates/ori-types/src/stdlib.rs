use crate::{DefId, OpaqueTy, Ty};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibRuntimeFunction {
    pub canonical_path: &'static str,
    pub aliases: &'static [&'static str],
    pub runtime_symbol: &'static str,
    pub native_runtime: bool,
    pub c_backend_runtime: bool,
}

fn opaque_collection(kind: OpaqueTy) -> Ty {
    Ty::Opaque {
        kind,
        args: vec![Ty::Infer(0)],
    }
}

fn tree_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Tree,
        args: vec![Ty::Infer(0)],
    }
}

fn node_id_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::NodeId,
        args: vec![],
    }
}

fn hash_table_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::HashTable,
        args: vec![Ty::Infer(0), Ty::Infer(1)],
    }
}

fn graph_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Graph,
        args: vec![Ty::Infer(0)],
    }
}

fn heap_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Heap,
        args: vec![Ty::Infer(0)],
    }
}

fn file_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::File,
        args: vec![],
    }
}

fn cancel_token_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::CancelToken,
        args: vec![],
    }
}

fn connection_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Connection,
        args: vec![],
    }
}

fn listener_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Listener,
        args: vec![],
    }
}

fn udp_socket_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::UdpSocket,
        args: vec![],
    }
}

fn input_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Input,
        args: vec![],
    }
}

fn output_ty() -> Ty {
    Ty::Opaque {
        kind: OpaqueTy::Output,
        args: vec![],
    }
}

fn list_backed_collection_kind(path: &str, ops: &[&str]) -> Option<OpaqueTy> {
    let candidates = [
        ("ori.deque.", OpaqueTy::Deque),
        ("ori.queue.", OpaqueTy::Queue),
        ("ori.stack.", OpaqueTy::Stack),
        ("ori.linked_list.", OpaqueTy::LinkedList),
        ("ori.doubly_linked_list.", OpaqueTy::DoublyLinkedList),
    ];

    candidates.iter().find_map(|(prefix, kind)| {
        path.strip_prefix(prefix)
            .filter(|op| ops.iter().any(|candidate| candidate == op))
            .map(|_| *kind)
    })
}

macro_rules! stdlib {
    ($path:literal => $symbol:literal) => {
        StdlibRuntimeFunction {
            canonical_path: $path,
            aliases: &[],
            runtime_symbol: $symbol,
            native_runtime: true,
            c_backend_runtime: false,
        }
    };
    ($path:literal => $symbol:literal, c_backend) => {
        StdlibRuntimeFunction {
            canonical_path: $path,
            aliases: &[],
            runtime_symbol: $symbol,
            native_runtime: true,
            c_backend_runtime: true,
        }
    };
    ($path:literal, [$($alias:literal),* $(,)?] => $symbol:literal) => {
        StdlibRuntimeFunction {
            canonical_path: $path,
            aliases: &[$($alias),*],
            runtime_symbol: $symbol,
            native_runtime: true,
            c_backend_runtime: false,
        }
    };
    ($path:literal, [$($alias:literal),* $(,)?] => $symbol:literal, c_backend) => {
        StdlibRuntimeFunction {
            canonical_path: $path,
            aliases: &[$($alias),*],
            runtime_symbol: $symbol,
            native_runtime: true,
            c_backend_runtime: true,
        }
    };
}

pub const STDLIB_RUNTIME_FUNCTIONS: &[StdlibRuntimeFunction] = &[
    stdlib!("ori.io.print" => "ori_io_print", c_backend),
    stdlib!("ori.io.println" => "ori_io_print", c_backend),
    stdlib!("ori.mem.string_as_ptr" => "ori_mem_string_as_ptr", c_backend),
    stdlib!("ori.mem.string_len" => "ori_mem_string_len", c_backend),
    stdlib!("ori.handle.null" => "ori_handle_null"),
    stdlib!("ori.handle.is_null" => "ori_handle_is_null", c_backend),
    stdlib!("ori.io.eprint" => "ori_io_eprint", c_backend),
    stdlib!("ori.io.eprintln" => "ori_io_eprint", c_backend),
    stdlib!("ori.io.read_line" => "ori_io_read_line", c_backend),
    stdlib!("ori.io.stdin" => "ori_io_stdin"),
    stdlib!("ori.io.stdout" => "ori_io_stdout"),
    stdlib!("ori.io.stderr" => "ori_io_stderr"),
    stdlib!("ori.io.open_input" => "ori_io_open_input"),
    stdlib!("ori.io.open_output" => "ori_io_open_output"),
    stdlib!("ori.io.read" => "ori_io_read"),
    stdlib!("ori.io.write" => "ori_io_write"),
    stdlib!("ori.io.flush" => "ori_io_flush"),
    stdlib!("ori.io.close_input" => "ori_io_close_input"),
    stdlib!("ori.io.close_output" => "ori_io_close_output"),
    stdlib!("ori.string.len" => "ori_string_len", c_backend),
    stdlib!("ori.string.concat" => "ori_string_concat", c_backend),
    stdlib!("ori.string.split" => "ori_string_split", c_backend),
    stdlib!("ori.string.slice" => "ori_string_slice", c_backend),
    stdlib!("ori.string.contains" => "ori_string_contains", c_backend),
    stdlib!("ori.string.starts_with" => "ori_string_starts_with", c_backend),
    stdlib!("ori.string.ends_with" => "ori_string_ends_with", c_backend),
    stdlib!("ori.string.trim" => "ori_string_trim", c_backend),
    stdlib!("ori.string.trim_start" => "ori_string_trim_start", c_backend),
    stdlib!("ori.string.trim_end" => "ori_string_trim_end", c_backend),
    stdlib!("ori.string.to_upper" => "ori_string_to_upper", c_backend),
    stdlib!("ori.string.to_lower" => "ori_string_to_lower", c_backend),
    stdlib!("ori.string.is_ascii" => "ori_string_is_ascii", c_backend),
    stdlib!("ori.string.case_fold" => "ori_string_case_fold", c_backend),
    stdlib!("ori.string.replace" => "ori_string_replace", c_backend),
    stdlib!("ori.string.chars" => "ori_string_chars", c_backend),
    stdlib!("ori.err_trace.push", ["err_trace.push"] => "ori_err_trace_push", c_backend),
    stdlib!("ori.err_trace.format", ["err_trace.format"] => "ori_err_trace_format", c_backend),
    stdlib!("string", [] => "ori_to_string", c_backend),
    stdlib!("int", [] => "ori_to_int", c_backend),
    stdlib!("float", [] => "ori_to_float", c_backend),
    stdlib!("len", [] => "ori_len", c_backend),
    stdlib!("ori.list.new", ["list.new"] => "ori_list_new"),
    stdlib!("ori.list.with_capacity", ["list.with_capacity"] => "ori_list_with_capacity"),
    stdlib!("ori.list.push", ["list.push"] => "ori_list_push"),
    stdlib!("ori.list.get", ["list.get"] => "ori_list_get"),
    // `slice[T]` — a read-only **window**, distinct from `ori.list.slice`,
    // which copies. The name says which one you get.
    stdlib!("ori.list.window", ["list.window"] => "ori_slice_new"),
    stdlib!("ori.slice.len", ["slice.len"] => "ori_slice_len"),
    stdlib!("ori.slice.get", ["slice.get"] => "ori_slice_get"),
    stdlib!("ori.list.try_get", ["list.try_get"] => "ori_list_try_get"),
    stdlib!("ori.list.set", ["list.set"] => "ori_list_set"),
    stdlib!("ori.list.len", ["list.len"] => "ori_list_len"),
    stdlib!("ori.list.capacity", ["list.capacity"] => "ori_list_capacity"),
    stdlib!("ori.list.reserve", ["list.reserve"] => "ori_list_reserve"),
    stdlib!("ori.list.is_empty", ["list.is_empty"] => "ori_list_is_empty"),
    stdlib!("ori.list.clear", ["list.clear"] => "ori_list_clear"),
    stdlib!("ori.list.clone", ["list.clone"] => "ori_list_clone"),
    stdlib!("ori.list.to_list", ["list.to_list"] => "ori_list_to_list"),
    stdlib!("ori.list.from_list", ["list.from_list"] => "ori_list_from_list"),
    stdlib!("ori.list.free", ["list.free"] => "ori_list_free"),
    stdlib!("ori.buffer.new", ["buffer.new"] => "ori_buffer_new"),
    stdlib!("ori.buffer.len", ["buffer.len"] => "ori_buffer_len"),
    stdlib!("ori.buffer.is_empty", ["buffer.is_empty"] => "ori_buffer_is_empty"),
    stdlib!("ori.buffer.get", ["buffer.get"] => "ori_buffer_get"),
    stdlib!("ori.buffer.set", ["buffer.set"] => "ori_buffer_set"),
    stdlib!("ori.buffer.fill", ["buffer.fill"] => "ori_buffer_fill"),
    stdlib!("ori.buffer.as_slice", ["buffer.as_slice"] => "ori_buffer_as_slice"),
    stdlib!("ori.deque.new", ["deque.new"] => "ori_deque_new"),
    stdlib!("ori.deque.push_front", ["deque.push_front"] => "ori_deque_push_front"),
    stdlib!("ori.deque.push_back", ["deque.push_back"] => "ori_deque_push_back"),
    stdlib!("ori.deque.pop_front", ["deque.pop_front"] => "ori_deque_pop_front"),
    stdlib!("ori.deque.pop_back", ["deque.pop_back"] => "ori_deque_pop_back"),
    stdlib!("ori.deque.front", ["deque.front"] => "ori_deque_front"),
    stdlib!("ori.deque.back", ["deque.back"] => "ori_deque_back"),
    stdlib!("ori.deque.len", ["deque.len"] => "ori_deque_len"),
    stdlib!("ori.deque.is_empty", ["deque.is_empty"] => "ori_deque_is_empty"),
    stdlib!("ori.deque.clear", ["deque.clear"] => "ori_deque_clear"),
    stdlib!("ori.deque.clone", ["deque.clone"] => "ori_deque_clone"),
    stdlib!("ori.deque.to_list", ["deque.to_list"] => "ori_deque_to_list"),
    stdlib!("ori.queue.new", ["queue.new"] => "ori_queue_new"),
    stdlib!("ori.queue.enqueue", ["queue.enqueue"] => "ori_queue_enqueue"),
    stdlib!("ori.queue.dequeue", ["queue.dequeue"] => "ori_queue_dequeue"),
    stdlib!("ori.queue.peek", ["queue.peek"] => "ori_queue_peek"),
    stdlib!("ori.queue.len", ["queue.len"] => "ori_queue_len"),
    stdlib!("ori.queue.is_empty", ["queue.is_empty"] => "ori_queue_is_empty"),
    stdlib!("ori.queue.clear", ["queue.clear"] => "ori_queue_clear"),
    stdlib!("ori.queue.clone", ["queue.clone"] => "ori_queue_clone"),
    stdlib!("ori.queue.to_list", ["queue.to_list"] => "ori_queue_to_list"),
    stdlib!("ori.stack.new", ["stack.new"] => "ori_stack_new"),
    stdlib!("ori.stack.push", ["stack.push"] => "ori_stack_push"),
    stdlib!("ori.stack.pop", ["stack.pop"] => "ori_stack_pop"),
    stdlib!("ori.stack.peek", ["stack.peek"] => "ori_stack_peek"),
    stdlib!("ori.stack.len", ["stack.len"] => "ori_stack_len"),
    stdlib!("ori.stack.is_empty", ["stack.is_empty"] => "ori_stack_is_empty"),
    stdlib!("ori.stack.clear", ["stack.clear"] => "ori_stack_clear"),
    stdlib!("ori.stack.clone", ["stack.clone"] => "ori_stack_clone"),
    stdlib!("ori.stack.to_list", ["stack.to_list"] => "ori_stack_to_list"),
    stdlib!("ori.linked_list.new", ["linked_list.new"] => "ori_linked_list_new"),
    stdlib!("ori.linked_list.push_front", ["linked_list.push_front"] => "ori_linked_list_push_front"),
    stdlib!("ori.linked_list.push_back", ["linked_list.push_back"] => "ori_linked_list_push_back"),
    stdlib!("ori.linked_list.pop_front", ["linked_list.pop_front"] => "ori_linked_list_pop_front"),
    stdlib!("ori.linked_list.front", ["linked_list.front"] => "ori_linked_list_front"),
    stdlib!("ori.linked_list.cursor_front", ["linked_list.cursor_front"] => "ori_linked_list_cursor_front"),
    stdlib!("ori.linked_list.cursor_back", ["linked_list.cursor_back"] => "ori_linked_list_cursor_back"),
    stdlib!("ori.linked_list.value_at", ["linked_list.value_at"] => "ori_linked_list_value_at"),
    stdlib!("ori.linked_list.insert_after", ["linked_list.insert_after"] => "ori_linked_list_insert_after"),
    stdlib!("ori.linked_list.remove_at", ["linked_list.remove_at"] => "ori_linked_list_remove_at"),
    stdlib!("ori.linked_list.find", ["linked_list.find"] => "ori_linked_list_find"),
    stdlib!("ori.linked_list.len", ["linked_list.len"] => "ori_linked_list_len"),
    stdlib!("ori.linked_list.is_empty", ["linked_list.is_empty"] => "ori_linked_list_is_empty"),
    stdlib!("ori.linked_list.clear", ["linked_list.clear"] => "ori_linked_list_clear"),
    stdlib!("ori.linked_list.clone", ["linked_list.clone"] => "ori_linked_list_clone"),
    stdlib!("ori.linked_list.to_list", ["linked_list.to_list"] => "ori_linked_list_to_list"),
    stdlib!("ori.doubly_linked_list.new", ["doubly_linked_list.new"] => "ori_doubly_linked_list_new"),
    stdlib!("ori.doubly_linked_list.push_front", ["doubly_linked_list.push_front"] => "ori_doubly_linked_list_push_front"),
    stdlib!("ori.doubly_linked_list.push_back", ["doubly_linked_list.push_back"] => "ori_doubly_linked_list_push_back"),
    stdlib!("ori.doubly_linked_list.pop_front", ["doubly_linked_list.pop_front"] => "ori_doubly_linked_list_pop_front"),
    stdlib!("ori.doubly_linked_list.pop_back", ["doubly_linked_list.pop_back"] => "ori_doubly_linked_list_pop_back"),
    stdlib!("ori.doubly_linked_list.front", ["doubly_linked_list.front"] => "ori_doubly_linked_list_front"),
    stdlib!("ori.doubly_linked_list.back", ["doubly_linked_list.back"] => "ori_doubly_linked_list_back"),
    stdlib!("ori.doubly_linked_list.cursor_front", ["doubly_linked_list.cursor_front"] => "ori_doubly_linked_list_cursor_front"),
    stdlib!("ori.doubly_linked_list.cursor_back", ["doubly_linked_list.cursor_back"] => "ori_doubly_linked_list_cursor_back"),
    stdlib!("ori.doubly_linked_list.value_at", ["doubly_linked_list.value_at"] => "ori_doubly_linked_list_value_at"),
    stdlib!("ori.doubly_linked_list.insert_after", ["doubly_linked_list.insert_after"] => "ori_doubly_linked_list_insert_after"),
    stdlib!("ori.doubly_linked_list.insert_before", ["doubly_linked_list.insert_before"] => "ori_doubly_linked_list_insert_before"),
    stdlib!("ori.doubly_linked_list.remove_at", ["doubly_linked_list.remove_at"] => "ori_doubly_linked_list_remove_at"),
    stdlib!("ori.doubly_linked_list.find", ["doubly_linked_list.find"] => "ori_doubly_linked_list_find"),
    stdlib!("ori.doubly_linked_list.len", ["doubly_linked_list.len"] => "ori_doubly_linked_list_len"),
    stdlib!("ori.doubly_linked_list.is_empty", ["doubly_linked_list.is_empty"] => "ori_doubly_linked_list_is_empty"),
    stdlib!("ori.doubly_linked_list.clear", ["doubly_linked_list.clear"] => "ori_doubly_linked_list_clear"),
    stdlib!("ori.doubly_linked_list.clone", ["doubly_linked_list.clone"] => "ori_doubly_linked_list_clone"),
    stdlib!("ori.doubly_linked_list.to_list", ["doubly_linked_list.to_list"] => "ori_doubly_linked_list_to_list"),
    stdlib!("ori.tree.new", ["tree.new"] => "ori_tree_new"),
    stdlib!("ori.tree.root", ["tree.root"] => "ori_tree_root"),
    stdlib!("ori.tree.value", ["tree.value"] => "ori_tree_value"),
    stdlib!("ori.tree.try_value", ["tree.try_value"] => "ori_tree_try_value"),
    stdlib!("ori.tree.contains_node", ["tree.contains_node"] => "ori_tree_contains_node"),
    stdlib!("ori.tree.set_value", ["tree.set_value"] => "ori_tree_set_value"),
    stdlib!("ori.tree.add_child", ["tree.add_child"] => "ori_tree_add_child"),
    stdlib!("ori.tree.children", ["tree.children"] => "ori_tree_children"),
    stdlib!("ori.tree.parent", ["tree.parent"] => "ori_tree_parent"),
    stdlib!("ori.tree.remove_subtree", ["tree.remove_subtree"] => "ori_tree_remove_subtree"),
    stdlib!("ori.tree.move_subtree", ["tree.move_subtree"] => "ori_tree_move_subtree"),
    stdlib!("ori.tree.find", ["tree.find"] => "ori_tree_find"),
    stdlib!("ori.tree.len", ["tree.len"] => "ori_tree_len"),
    stdlib!("ori.tree.depth", ["tree.depth"] => "ori_tree_depth"),
    stdlib!("ori.tree.pre_order", ["tree.pre_order"] => "ori_tree_pre_order"),
    stdlib!("ori.tree.post_order", ["tree.post_order"] => "ori_tree_post_order"),
    stdlib!("ori.tree.breadth_first", ["tree.breadth_first"] => "ori_tree_breadth_first"),
    stdlib!("ori.tree.clone", ["tree.clone"] => "ori_tree_clone"),
    stdlib!("ori.tree.clone_subtree", ["tree.clone_subtree"] => "ori_tree_clone_subtree"),
    stdlib!("ori.set.new", ["set.new"] => "ori_set_new"),
    stdlib!("ori.set.add", ["set.add"] => "ori_set_add"),
    stdlib!("ori.set.contains", ["set.contains"] => "ori_set_contains"),
    stdlib!("ori.set.len", ["set.len"] => "ori_set_len"),
    stdlib!("ori.set.is_empty", ["set.is_empty"] => "ori_set_is_empty"),
    stdlib!("ori.set.capacity", ["set.capacity"] => "ori_set_capacity"),
    stdlib!("ori.set.reserve", ["set.reserve"] => "ori_set_reserve"),
    stdlib!("ori.set.clear", ["set.clear"] => "ori_set_clear"),
    stdlib!("ori.set.clone", ["set.clone"] => "ori_set_clone"),
    stdlib!("ori.set.to_list", ["set.to_list"] => "ori_set_to_list"),
    stdlib!("ori.set.from_list", ["set.from_list"] => "ori_set_from_list"),
    stdlib!("ori.set.free", ["set.free"] => "ori_set_free"),
    stdlib!("ori.map.new", ["map.new"] => "ori_map_new"),
    stdlib!("ori.map.set", ["map.set"] => "ori_map_set"),
    stdlib!("ori.map.get", ["map.get"] => "ori_map_get"),
    stdlib!("ori.map.try_get", ["map.try_get"] => "ori_map_try_get"),
    stdlib!("ori.map.contains", ["map.contains"] => "ori_map_contains"),
    stdlib!("ori.map.len", ["map.len"] => "ori_map_len"),
    stdlib!("ori.map.is_empty", ["map.is_empty"] => "ori_map_is_empty"),
    stdlib!("ori.map.capacity", ["map.capacity"] => "ori_map_capacity"),
    stdlib!("ori.map.reserve", ["map.reserve"] => "ori_map_reserve"),
    stdlib!("ori.map.clear", ["map.clear"] => "ori_map_clear"),
    stdlib!("ori.map.clone", ["map.clone"] => "ori_map_clone"),
    stdlib!("ori.map.from_entries", ["map.from_entries"] => "ori_map_from_entries"),
    stdlib!("ori.map.free", ["map.free"] => "ori_map_free"),
    stdlib!("ori.hash_table.new", ["hash_table.new"] => "ori_hash_table_new"),
    stdlib!("ori.hash_table.with_capacity", ["hash_table.with_capacity"] => "ori_hash_table_with_capacity"),
    stdlib!("ori.hash_table.set", ["hash_table.set"] => "ori_hash_table_set"),
    stdlib!("ori.hash_table.get", ["hash_table.get"] => "ori_hash_table_get"),
    stdlib!("ori.hash_table.remove", ["hash_table.remove"] => "ori_hash_table_remove"),
    stdlib!("ori.hash_table.contains", ["hash_table.contains"] => "ori_hash_table_contains"),
    stdlib!("ori.hash_table.len", ["hash_table.len"] => "ori_hash_table_len"),
    stdlib!("ori.hash_table.is_empty", ["hash_table.is_empty"] => "ori_hash_table_is_empty"),
    stdlib!("ori.hash_table.capacity", ["hash_table.capacity"] => "ori_hash_table_capacity"),
    stdlib!("ori.hash_table.reserve", ["hash_table.reserve"] => "ori_hash_table_reserve"),
    stdlib!("ori.hash_table.clear", ["hash_table.clear"] => "ori_hash_table_clear"),
    stdlib!("ori.hash_table.clone", ["hash_table.clone"] => "ori_hash_table_clone"),
    stdlib!("ori.hash_table.from_entries", ["hash_table.from_entries"] => "ori_hash_table_from_entries"),
    stdlib!("ori.hash_table.keys", ["hash_table.keys"] => "ori_hash_table_keys"),
    stdlib!("ori.hash_table.values", ["hash_table.values"] => "ori_hash_table_values"),
    stdlib!("ori.hash_table.entries", ["hash_table.entries"] => "ori_hash_table_entries"),
    stdlib!("ori.graph.new", ["graph.new"] => "ori_graph_new"),
    stdlib!("ori.graph.add_node", ["graph.add_node"] => "ori_graph_add_node"),
    stdlib!("ori.graph.remove_node", ["graph.remove_node"] => "ori_graph_remove_node"),
    stdlib!("ori.graph.add_edge", ["graph.add_edge"] => "ori_graph_add_edge"),
    stdlib!("ori.graph.add_weighted_edge", ["graph.add_weighted_edge"] => "ori_graph_add_weighted_edge"),
    stdlib!("ori.graph.remove_edge", ["graph.remove_edge"] => "ori_graph_remove_edge"),
    stdlib!("ori.graph.has_node", ["graph.has_node"] => "ori_graph_has_node"),
    stdlib!("ori.graph.has_edge", ["graph.has_edge"] => "ori_graph_has_edge"),
    stdlib!("ori.graph.edge_weight", ["graph.edge_weight"] => "ori_graph_edge_weight"),
    stdlib!("ori.graph.neighbors", ["graph.neighbors"] => "ori_graph_neighbors"),
    stdlib!("ori.graph.nodes", ["graph.nodes"] => "ori_graph_nodes"),
    stdlib!("ori.graph.edges", ["graph.edges"] => "ori_graph_edges"),
    stdlib!("ori.graph.bfs", ["graph.bfs"] => "ori_graph_bfs"),
    stdlib!("ori.graph.dfs", ["graph.dfs"] => "ori_graph_dfs"),
    stdlib!("ori.graph.topological_sort", ["graph.topological_sort"] => "ori_graph_topological_sort"),
    stdlib!("ori.graph.try_topological_sort", ["graph.try_topological_sort"] => "ori_graph_try_topological_sort"),
    stdlib!("ori.graph.is_directed", ["graph.is_directed"] => "ori_graph_is_directed"),
    stdlib!("ori.graph.len", ["graph.len"] => "ori_graph_len"),
    stdlib!("ori.graph.edge_len", ["graph.edge_len"] => "ori_graph_edge_len"),
    stdlib!("ori.graph.has_cycle", ["graph.has_cycle"] => "ori_graph_has_cycle"),
    stdlib!("ori.graph.components", ["graph.components"] => "ori_graph_components"),
    stdlib!("ori.graph.strongly_connected_components", ["graph.strongly_connected_components"] => "ori_graph_strongly_connected_components"),
    stdlib!("ori.graph.transitive_closure", ["graph.transitive_closure"] => "ori_graph_transitive_closure"),
    stdlib!("ori.graph.shortest_path", ["graph.shortest_path"] => "ori_graph_shortest_path"),
    stdlib!("ori.graph.shortest_weighted_path", ["graph.shortest_weighted_path"] => "ori_graph_shortest_weighted_path"),
    stdlib!("ori.graph.clone", ["graph.clone"] => "ori_graph_clone"),
    stdlib!("ori.heap.new", ["heap.new"] => "ori_heap_new"),
    stdlib!("ori.heap.push", ["heap.push"] => "ori_heap_push"),
    stdlib!("ori.heap.pop", ["heap.pop"] => "ori_heap_pop"),
    stdlib!("ori.heap.peek", ["heap.peek"] => "ori_heap_peek"),
    stdlib!("ori.heap.len", ["heap.len"] => "ori_heap_len"),
    stdlib!("ori.heap.is_empty", ["heap.is_empty"] => "ori_heap_is_empty"),
    stdlib!("ori.heap.clear", ["heap.clear"] => "ori_heap_clear"),
    stdlib!("ori.heap.clone", ["heap.clone"] => "ori_heap_clone"),
    stdlib!("ori.heap.to_list", ["heap.to_list"] => "ori_heap_to_list"),
    stdlib!("ori.heap.from_list", ["heap.from_list"] => "ori_heap_from_list"),
    stdlib!("ori.heap.merge", ["heap.merge"] => "ori_heap_merge"),
    stdlib!("ori.heap.remove", ["heap.remove"] => "ori_heap_remove"),
    stdlib!("ori.heap.into_sorted_list", ["heap.into_sorted_list"] => "ori_heap_into_sorted_list"),
    stdlib!("ori.math.sqrt" => "ori_math_sqrt", c_backend),
    stdlib!("ori.math.abs" => "ori_math_abs", c_backend),
    stdlib!("ori.math.min" => "ori_math_min", c_backend),
    stdlib!("ori.math.max" => "ori_math_max", c_backend),
    stdlib!("ori.math.clamp" => "ori_math_clamp", c_backend),
    stdlib!("ori.math.pow" => "ori_math_pow", c_backend),
    stdlib!("ori.math.floor" => "ori_math_floor", c_backend),
    stdlib!("ori.math.ceil" => "ori_math_ceil", c_backend),
    stdlib!("ori.math.round" => "ori_math_round", c_backend),
    stdlib!("ori.math.log" => "ori_math_log", c_backend),
    stdlib!("ori.math.log2" => "ori_math_log2", c_backend),
    stdlib!("ori.math.sin" => "ori_math_sin", c_backend),
    stdlib!("ori.math.cos" => "ori_math_cos", c_backend),
    stdlib!("ori.math.tan" => "ori_math_tan", c_backend),
    stdlib!("ori.math.is_nan" => "ori_math_is_nan", c_backend),
    stdlib!("ori.math.is_infinite" => "ori_math_is_infinite", c_backend),
    stdlib!("ori.time.now", ["time.now"] => "ori_time_now", c_backend),
    stdlib!("ori.time.sleep", ["time.sleep"] => "ori_time_sleep", c_backend),
    stdlib!(
        "ori.time.duration_ms",
        ["time.duration_ms"] => "ori_time_duration_ms",
        c_backend
    ),
    stdlib!("ori.format.number", ["format.number"] => "ori_format_number", c_backend),
    stdlib!(
        "ori.format.percent",
        ["format.percent"] => "ori_format_percent",
        c_backend
    ),
    stdlib!("ori.format.hex", ["format.hex"] => "ori_format_hex", c_backend),
    stdlib!(
        "ori.format.binary",
        ["format.binary"] => "ori_format_binary",
        c_backend
    ),
    stdlib!("ori.format.date", ["format.date"] => "ori_format_date", c_backend),
    stdlib!(
        "ori.format.datetime",
        ["format.datetime"] => "ori_format_datetime",
        c_backend
    ),
    stdlib!(
        "ori.format.bytes_size",
        ["format.bytes_size"] => "ori_format_bytes_size",
        c_backend
    ),
    stdlib!("ori.os.args", ["os.args"] => "ori_os_args", c_backend),
    stdlib!("ori.os.env", ["os.env"] => "ori_os_env", c_backend),
    stdlib!("ori.os.exit", ["os.exit"] => "ori_os_exit", c_backend),
    stdlib!("ori.os.pid", ["os.pid"] => "ori_os_pid", c_backend),
    stdlib!("ori.os.platform", ["os.platform"] => "ori_os_platform", c_backend),
    stdlib!("ori.os.arch", ["os.arch"] => "ori_os_arch", c_backend),
    stdlib!("ori.random.int", ["random.int"] => "ori_random_int", c_backend),
    stdlib!(
        "ori.random.float",
        ["random.float"] => "ori_random_float",
        c_backend
    ),
    stdlib!(
        "ori.random.bool",
        ["random.bool"] => "ori_random_bool",
        c_backend
    ),
    stdlib!(
        "ori.random.choice",
        ["random.choice"] => "ori_random_choice",
        c_backend
    ),
    stdlib!(
        "ori.random.shuffle",
        ["random.shuffle"] => "ori_random_shuffle",
        c_backend
    ),
    stdlib!("ori.json.parse", ["json.parse"] => "ori_json_parse"),
    stdlib!(
        "ori.json.stringify",
        ["json.stringify"] => "ori_json_stringify"
    ),
    stdlib!(
        "ori.json.stringify_pretty",
        ["json.stringify_pretty"] => "ori_json_stringify_pretty"
    ),
    StdlibRuntimeFunction {
        canonical_path: "ori.lazy.once",
        aliases: &["lazy.once"],
        runtime_symbol: "ori_lazy_once",
        native_runtime: false,
        c_backend_runtime: false,
    },
    StdlibRuntimeFunction {
        canonical_path: "ori.lazy.force",
        aliases: &["lazy.force"],
        runtime_symbol: "ori_lazy_force",
        native_runtime: false,
        c_backend_runtime: false,
    },
    stdlib!("ori.task.spawn", ["task.spawn"] => "ori_task_spawn"),
    stdlib!("ori.task.join", ["task.join"] => "ori_task_join"),
    stdlib!("ori.task.detach", ["task.detach"] => "ori_task_detach"),
    stdlib!("ori.task.block_on", ["task.block_on"] => "ori_task_block_on"),
    stdlib!("ori.task.run_blocking", ["task.run_blocking"] => "ori_task_spawn"),
    stdlib!("ori.task.sleep", ["task.sleep"] => "ori_task_sleep"),
    stdlib!("ori.task.create_token", ["task.create_token"] => "ori_task_create_token"),
    stdlib!("ori.task.cancel", ["task.cancel"] => "ori_task_cancel"),
    stdlib!("ori.task.is_cancelled", ["task.is_cancelled"] => "ori_task_is_cancelled"),
    stdlib!("ori.task.associate", ["task.associate"] => "ori_task_associate"),
    stdlib!("ori.channel.create", ["channel.create"] => "ori_channel_create"),
    stdlib!("ori.channel.create_bounded" => "ori_channel_create_bounded"),
    stdlib!("ori.channel.send", ["channel.send"] => "ori_channel_send"),
    stdlib!("ori.channel.__send_managed" => "ori_channel_send_managed"),
    stdlib!("ori.channel.receive", ["channel.receive"] => "ori_channel_receive"),
    stdlib!("ori.channel.close", ["channel.close"] => "ori_channel_close"),
    stdlib!("ori.atomic.new", ["atomic.new"] => "ori_atomic_new"),
    stdlib!("ori.atomic.load", ["atomic.load"] => "ori_atomic_load"),
    stdlib!("ori.atomic.store", ["atomic.store"] => "ori_atomic_store"),
    stdlib!("ori.atomic.add", ["atomic.add"] => "ori_atomic_add"),
    stdlib!("ori.test.assert", ["test.assert"] => "ori_test_assert", c_backend),
    stdlib!("ori.test.assert_eq", ["test.assert_eq"] => "ori_test_assert_eq", c_backend),
    stdlib!("ori.test.assert_ne", ["test.assert_ne"] => "ori_test_assert_ne", c_backend),
    stdlib!("ori.test.fail", ["test.fail"] => "ori_test_fail", c_backend),
    stdlib!("ori.test.live_allocations", ["test.live_allocations"] => "ori_test_live_allocations"),
    stdlib!("ori.test.collect_cycles", ["test.collect_cycles"] => "ori_test_collect_cycles"),
    stdlib!("ori.test.assert_no_leaks", ["test.assert_no_leaks"] => "ori_test_assert_no_leaks"),
    stdlib!("ori.panic" => "ori_panic"),
    stdlib!("ori.list.pop", ["list.pop"] => "ori_list_pop"),
    stdlib!("ori.list.try_pop", ["list.try_pop"] => "ori_list_try_pop"),
    stdlib!("ori.list.remove", ["list.remove"] => "ori_list_remove"),
    stdlib!("ori.list.try_remove", ["list.try_remove"] => "ori_list_try_remove"),
    stdlib!("ori.list.insert", ["list.insert"] => "ori_list_insert"),
    stdlib!("ori.list.contains", ["list.contains"] => "ori_list_contains"),
    stdlib!("ori.list.index_of", ["list.index_of"] => "ori_list_index_of"),
    stdlib!("ori.list.sort", ["list.sort"] => "ori_list_sort"),
    stdlib!("ori.list.reverse", ["list.reverse"] => "ori_list_reverse"),
    stdlib!("ori.list.slice", ["list.slice"] => "ori_list_slice"),
    stdlib!("ori.map.remove", ["map.remove"] => "ori_map_remove"),
    stdlib!("ori.map.try_remove", ["map.try_remove"] => "ori_map_try_remove"),
    stdlib!("ori.map.keys", ["map.keys"] => "ori_map_keys"),
    stdlib!("ori.map.values", ["map.values"] => "ori_map_values"),
    stdlib!("ori.map.entries", ["map.entries"] => "ori_map_entries"),
    stdlib!("ori.set.remove", ["set.remove"] => "ori_set_remove"),
    stdlib!("ori.set.try_remove", ["set.try_remove"] => "ori_set_try_remove"),
    stdlib!("ori.set.union", ["set.union"] => "ori_set_union"),
    stdlib!("ori.set.intersection", ["set.intersection"] => "ori_set_intersection"),
    stdlib!("ori.set.difference", ["set.difference"] => "ori_set_difference"),
    stdlib!("ori.list.map", ["list.map"] => "ori_list_map"),
    stdlib!("ori.list.filter", ["list.filter"] => "ori_list_filter"),
    stdlib!("ori.iter.map", ["iter.map"] => "ori_list_map", c_backend),
    stdlib!("ori.iter.filter", ["iter.filter"] => "ori_list_filter", c_backend),
    stdlib!("ori.iter.any", ["iter.any"] => "ori_iter_any", c_backend),
    stdlib!("ori.iter.all", ["iter.all"] => "ori_iter_all", c_backend),
    stdlib!(
        "ori.iter.count_where",
        ["iter.count_where"] => "ori_iter_count_where",
        c_backend
    ),
    stdlib!("ori.iter.take", ["iter.take"] => "ori_iter_take", c_backend),
    stdlib!("ori.iter.skip", ["iter.skip"] => "ori_iter_skip", c_backend),
    stdlib!("ori.iter.reverse", ["iter.reverse"] => "ori_iter_reverse", c_backend),
    stdlib!("ori.iter.reduce", ["iter.reduce"] => "ori_iter_reduce", c_backend),
    stdlib!("ori.iter.find", ["iter.find"] => "ori_iter_find", c_backend),
    stdlib!("ori.iter.sort", ["iter.sort"] => "ori_iter_sort", c_backend),
    stdlib!("ori.iter.sort_by", ["iter.sort_by"] => "ori_iter_sort_by", c_backend),
    stdlib!("ori.iter.unique", ["iter.unique"] => "ori_iter_unique", c_backend),
    stdlib!("ori.iter.flat_map", ["iter.flat_map"] => "ori_iter_flat_map", c_backend),
    stdlib!("ori.iter.zip", ["iter.zip"] => "ori_iter_zip", c_backend),
    stdlib!(
        "ori.iter.partition",
        ["iter.partition"] => "ori_iter_partition",
        c_backend
    ),
    stdlib!(
        "ori.iter.group_by",
        ["iter.group_by"] => "ori_iter_group_by",
        c_backend
    ),
    stdlib!("ori.iter.flatten", ["iter.flatten"] => "ori_iter_flatten", c_backend),
    stdlib!("ori.string.index_of", ["string.index_of"] => "ori_string_index_of", c_backend),
    stdlib!("ori.string.join", ["string.join"] => "ori_string_join", c_backend),
    stdlib!("ori.string.repeat", ["string.repeat"] => "ori_string_repeat", c_backend),
    stdlib!("ori.string.pad_left", ["string.pad_left"] => "ori_string_pad_left", c_backend),
    stdlib!("ori.string.pad_right", ["string.pad_right"] => "ori_string_pad_right", c_backend),
    stdlib!("ori.string.parse_int", ["string.parse_int"] => "ori_string_parse_int"),
    stdlib!("ori.string.parse_float", ["string.parse_float"] => "ori_string_parse_float"),
    stdlib!("ori.string.to_bytes", ["string.to_bytes"] => "ori_string_to_bytes"),
    stdlib!("ori.string.from_bytes", ["string.from_bytes"] => "ori_string_from_bytes"),
    stdlib!("ori.bytes.len", ["bytes.len"] => "ori_bytes_len"),
    stdlib!("ori.bytes.concat", ["bytes.concat"] => "ori_bytes_concat"),
    stdlib!("ori.bytes.slice", ["bytes.slice"] => "ori_bytes_slice"),
    stdlib!("ori.bytes.to_hex", ["bytes.to_hex"] => "ori_bytes_to_hex"),
    stdlib!("ori.bytes.from_hex", ["bytes.from_hex"] => "ori_bytes_from_hex"),
    stdlib!("ori.bytes.decode_utf8", ["bytes.decode_utf8"] => "ori_bytes_decode_utf8"),
    stdlib!("ori.bytes.get", ["bytes.get"] => "ori_bytes_get"),
    stdlib!("ori.convert.float_to_string", ["float_to_string"] => "ori_float_to_string", c_backend),
    stdlib!("ori.convert.bool_to_string", ["bool_to_string"] => "ori_bool_to_string", c_backend),
    stdlib!("ori.convert.string_to_int", ["string_to_int"] => "ori_string_to_int", c_backend),
    // float optional return type is emitted by C codegen; body stays native/extern.
    stdlib!("ori.convert.string_to_float", ["string_to_float"] => "ori_string_to_float"),
    stdlib!(
        "ori.fs.read_text",
        ["fs.read_text", "ori.files.read_text", "files.read_text"] => "ori_files_read_text"
    ),
    stdlib!(
        "ori.fs.read_text_async",
        ["fs.read_text_async", "ori.files.read_text_async", "files.read_text_async"] => "ori_files_read_text_async"
    ),
    stdlib!(
        "ori.fs.write_text",
        ["fs.write_text", "ori.files.write_text", "files.write_text"] => "ori_files_write_text"
    ),
    stdlib!(
        "ori.fs.write_text_async",
        ["fs.write_text_async", "ori.files.write_text_async", "files.write_text_async"] => "ori_files_write_text_async"
    ),
    stdlib!(
        "ori.fs.read_bytes",
        ["fs.read_bytes", "ori.files.read_bytes", "files.read_bytes"] => "ori_files_read_bytes"
    ),
    stdlib!(
        "ori.fs.write_bytes",
        ["fs.write_bytes", "ori.files.write_bytes", "files.write_bytes"] => "ori_files_write_bytes"
    ),
    stdlib!(
        "ori.fs.read_all",
        ["fs.read_all", "ori.files.read_all", "files.read_all"] => "ori_files_read_all"
    ),
    stdlib!(
        "ori.fs.append_text",
        ["fs.append_text", "ori.files.append_text", "files.append_text"] => "ori_files_append_text"
    ),
    stdlib!(
        "ori.fs.exists",
        ["fs.exists", "ori.files.exists", "files.exists"] => "ori_files_exists"
    ),
    stdlib!(
        "ori.fs.delete",
        ["fs.delete", "ori.files.delete", "files.delete"] => "ori_files_delete"
    ),
    stdlib!(
        "ori.fs.list_dir",
        ["fs.list_dir", "ori.files.list_dir", "files.list_dir"] => "ori_files_list_dir"
    ),
    stdlib!(
        "ori.fs.create_dir",
        ["fs.create_dir", "ori.files.create_dir", "files.create_dir"] => "ori_files_create_dir"
    ),
    stdlib!(
        "ori.fs.is_file",
        ["fs.is_file", "ori.files.is_file", "files.is_file"] => "ori_files_is_file"
    ),
    stdlib!(
        "ori.fs.is_dir",
        ["fs.is_dir", "ori.files.is_dir", "files.is_dir"] => "ori_files_is_dir"
    ),
    stdlib!(
        "ori.fs.copy",
        ["fs.copy", "ori.files.copy", "files.copy"] => "ori_files_copy"
    ),
    stdlib!(
        "ori.fs.rename",
        ["fs.rename", "ori.files.rename", "files.rename"] => "ori_files_rename"
    ),
    stdlib!(
        "ori.fs.open_read",
        ["fs.open_read", "ori.files.open_read", "files.open_read"] => "ori_files_open_read"
    ),
    stdlib!(
        "ori.fs.open_write",
        ["fs.open_write", "ori.files.open_write", "files.open_write"] => "ori_files_open_write"
    ),
    stdlib!(
        "ori.fs.read",
        ["fs.read", "ori.files.read", "files.read"] => "ori_files_read"
    ),
    stdlib!(
        "ori.fs.write",
        ["fs.write", "ori.files.write", "files.write"] => "ori_files_write"
    ),
    stdlib!(
        "ori.fs.close",
        ["fs.close", "ori.files.close", "files.close"] => "ori_files_close"
    ),
    stdlib!(
        "ori.fs.create_dir_all",
        ["fs.create_dir_all", "ori.files.create_dir_all", "files.create_dir_all"] =>
            "ori_files_create_dir"
    ),
    stdlib!(
        "ori.fs.file_size",
        ["fs.file_size", "fs.size"] => "ori_files_file_size"
    ),
    stdlib!(
        "ori.fs.modified_at",
        ["fs.modified_at"] => "ori_files_modified_at"
    ),
    stdlib!(
        "ori.fs.created_at",
        ["fs.created_at"] => "ori_files_created_at"
    ),
    stdlib!("ori.os.current_dir", ["os.current_dir"] => "ori_os_current_dir", c_backend),
    stdlib!("ori.os.change_dir", ["os.change_dir"] => "ori_os_change_dir", c_backend),
    stdlib!("ori.random.seed", ["random.seed"] => "ori_random_seed", c_backend),
    // Password hashing (argon2id PHC) — web C10 / SEC9
    stdlib!(
        "ori.crypto.password_hash",
        ["crypto.password_hash"] => "ori_password_hash"
    ),
    stdlib!(
        "ori.crypto.password_verify",
        ["crypto.password_verify"] => "ori_password_verify"
    ),
    // TOTP (RFC 6238) — web C3 / ori-web-auth
    stdlib!(
        "ori.crypto.totp_generate_secret",
        ["crypto.totp_generate_secret"] => "ori_totp_generate_secret"
    ),
    stdlib!(
        "ori.crypto.totp_code",
        ["crypto.totp_code"] => "ori_totp_code"
    ),
    stdlib!(
        "ori.crypto.totp_verify",
        ["crypto.totp_verify"] => "ori_totp_verify"
    ),
    stdlib!("ori.test.skip", ["test.skip"] => "ori_test_skip", c_backend),
    stdlib!("ori.bytes.from_list", ["bytes.from_list"] => "ori_bytes_from_list"),
    stdlib!("ori.bytes.to_list", ["bytes.to_list"] => "ori_bytes_to_list"),
    stdlib!("ori.process.run", ["process.run"] => "ori_process_run"),
    stdlib!(
        "ori.process.run_capture",
        ["process.run_capture"] => "ori_process_run_capture"
    ),
    stdlib!(
        "ori.process.run_output_raw",
        ["process.run_output_raw"] => "ori_process_run_output"
    ),
    stdlib!("ori.net.connect", ["net.connect"] => "ori_net_connect"),
    stdlib!("ori.net.connect_tls", ["net.connect_tls"] => "ori_net_connect_tls"),
    stdlib!(
        "ori.net.connect_async",
        ["net.connect_async"] => "ori_net_connect_async"
    ),
    stdlib!(
        "ori.net.connect_tls_async",
        ["net.connect_tls_async"] => "ori_net_connect_tls_async"
    ),
    stdlib!("ori.net.listen", ["net.listen"] => "ori_net_listen"),
    stdlib!("ori.net.accept", ["net.accept"] => "ori_net_accept"),
    stdlib!(
        "ori.net.accept_async",
        ["net.accept_async"] => "ori_net_accept_async"
    ),
    stdlib!("ori.net.close_listener", ["net.close_listener"] => "ori_net_close_listener"),
    stdlib!("ori.net.listener_port", ["net.listener_port"] => "ori_net_listener_port"),
    stdlib!("ori.net.udp_bind", ["net.udp_bind"] => "ori_net_udp_bind"),
    stdlib!("ori.net.udp_send_to", ["net.udp_send_to"] => "ori_net_udp_send_to"),
    stdlib!(
        "ori.net.udp_send_to_async",
        ["net.udp_send_to_async"] => "ori_net_udp_send_to_async"
    ),
    stdlib!("ori.net.udp_recv_from", ["net.udp_recv_from"] => "ori_net_udp_recv_from"),
    stdlib!(
        "ori.net.udp_recv_from_async",
        ["net.udp_recv_from_async"] => "ori_net_udp_recv_from_async"
    ),
    stdlib!("ori.net.udp_close", ["net.udp_close"] => "ori_net_udp_close"),
    stdlib!("ori.net.udp_local_port", ["net.udp_local_port"] => "ori_net_udp_local_port"),
    stdlib!("ori.net.read_some", ["net.read_some"] => "ori_net_read_some"),
    stdlib!(
        "ori.net.read_some_async",
        ["net.read_some_async"] => "ori_net_read_some_async"
    ),
    stdlib!(
        "ori.net.set_read_timeout_ms",
        ["net.set_read_timeout_ms"] => "ori_net_set_read_timeout_ms"
    ),
    stdlib!(
        "ori.net.set_write_timeout_ms",
        ["net.set_write_timeout_ms"] => "ori_net_set_write_timeout_ms"
    ),
    stdlib!("ori.net.write_all", ["net.write_all"] => "ori_net_write_all"),
    stdlib!(
        "ori.net.write_all_async",
        ["net.write_all_async"] => "ori_net_write_all_async"
    ),
    stdlib!("ori.net.close", ["net.close"] => "ori_net_close"),
    stdlib!("ori.net.is_closed", ["net.is_closed"] => "ori_net_is_closed"),
    stdlib!("ori.math.trunc", ["math.trunc"] => "ori_math_trunc", c_backend),
    stdlib!("ori.math.ln", ["math.ln"] => "ori_math_ln", c_backend),
    stdlib!("ori.math.exp", ["math.exp"] => "ori_math_exp", c_backend),
    stdlib!("ori.math.asin", ["math.asin"] => "ori_math_asin", c_backend),
    stdlib!("ori.math.acos", ["math.acos"] => "ori_math_acos", c_backend),
    stdlib!("ori.math.atan", ["math.atan"] => "ori_math_atan", c_backend),
    stdlib!("ori.math.atan2", ["math.atan2"] => "ori_math_atan2", c_backend),
    stdlib!("ori.math.log10", ["math.log10"] => "ori_math_log10", c_backend),
    stdlib!("ori.math.is_finite", ["math.is_finite"] => "ori_math_is_finite", c_backend),
    StdlibRuntimeFunction {
        canonical_path: "ori.lazy.is_consumed",
        aliases: &["lazy.is_consumed"],
        runtime_symbol: "ori_lazy_is_consumed",
        native_runtime: false,
        c_backend_runtime: false,
    },
];

pub fn stdlib_runtime_functions() -> &'static [StdlibRuntimeFunction] {
    STDLIB_RUNTIME_FUNCTIONS
}

/// Iterator over the runtime symbol names that must be resolved at link/JIT
/// time. Yields the `runtime_symbol` for every entry where `native_runtime`
/// is true (i.e. the symbol is provided by `ori-runtime` and consumed via
/// FFI). Used by the JIT backend to register symbols in `JITBuilder` and by
/// the AOT link path to validate that all imported symbols exist in the
/// runtime library.
pub fn stdlib_runtime_symbols() -> impl Iterator<Item = &'static str> {
    STDLIB_RUNTIME_FUNCTIONS
        .iter()
        .filter(|entry| entry.native_runtime)
        .map(|entry| entry.runtime_symbol)
}

pub fn stdlib_runtime_symbol(path: &str) -> Option<&'static str> {
    stdlib_entry_for_path(path).map(|entry| entry.runtime_symbol)
}

/// Returns true if the given stdlib path has native runtime FFI support.
pub fn stdlib_native_runtime_available(path: &str) -> bool {
    stdlib_entry_for_path(path)
        .map(|entry| entry.native_runtime)
        .unwrap_or(false)
}

/// Returns true if the native Cranelift backend can codegen this stdlib call.
/// Some functions (e.g. `lazy.once`) use inline codegen instead of runtime FFI.
pub fn stdlib_native_codegen_available(path: &str) -> bool {
    if stdlib_native_runtime_available(path) {
        return true;
    }
    matches!(
        canonical_stdlib_path(path).unwrap_or(path),
        "ori.lazy.once"
            | "ori.lazy.force"
            | "ori.lazy.is_consumed"
            | "ori.mem.size_of"
            | "ori.mem.align_of"
    )
}

pub fn canonical_stdlib_path(path: &str) -> Option<&'static str> {
    stdlib_entry_for_path(path).map(|entry| entry.canonical_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibNativeAbiTy {
    Ptr,
    I64,
    I32,
    I8,
    F64,
}

pub fn stdlib_func_sig(path: &str) -> Option<(Vec<Ty>, Ty)> {
    let path = canonical_stdlib_path(path).unwrap_or(path);
    let sig = match path {
        "ori.io.print" | "ori.io.println" | "ori.io.eprint" | "ori.io.eprintln" => {
            (vec![Ty::String], Ty::Void)
        }
        "ori.io.read_line" => (vec![], Ty::Optional(Box::new(Ty::String))),
        "ori.io.stdin" => (vec![], input_ty()),
        "ori.io.stdout" | "ori.io.stderr" => (vec![], output_ty()),
        "ori.io.open_input" => (
            vec![Ty::String],
            Ty::Result(Box::new(input_ty()), Box::new(Ty::String)),
        ),
        "ori.io.open_output" => (
            vec![Ty::String],
            Ty::Result(Box::new(output_ty()), Box::new(Ty::String)),
        ),
        "ori.io.read" => (
            vec![input_ty(), Ty::Int],
            Ty::Result(
                Box::new(Ty::Optional(Box::new(Ty::Bytes))),
                Box::new(Ty::String),
            ),
        ),
        "ori.io.write" => (
            vec![output_ty(), Ty::Bytes],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori.io.flush" => (
            vec![output_ty()],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.io.close_input" => (vec![input_ty()], Ty::Void),
        "ori.io.close_output" => (vec![output_ty()], Ty::Void),
        "ori.string.len" => (vec![Ty::String], Ty::Int),
        "ori.string.concat" => (vec![Ty::String, Ty::String], Ty::String),
        "ori.string.split" => (vec![Ty::String, Ty::String], Ty::List(Box::new(Ty::String))),
        "ori.string.slice" => (vec![Ty::String, Ty::Int, Ty::Int], Ty::String),
        "ori.string.contains" | "ori.string.starts_with" | "ori.string.ends_with" => {
            (vec![Ty::String, Ty::String], Ty::Bool)
        }
        "ori.string.trim"
        | "ori.string.trim_start"
        | "ori.string.trim_end"
        | "ori.string.to_upper"
        | "ori.string.to_lower"
        | "ori.string.case_fold" => (vec![Ty::String], Ty::String),
        "ori.err_trace.push" => (vec![Ty::String, Ty::Int, Ty::String], Ty::String),
        "ori.err_trace.format" => (vec![Ty::String], Ty::String),
        "ori.string.is_ascii" => (vec![Ty::String], Ty::Bool),
        "ori.string.replace" => (vec![Ty::String, Ty::String, Ty::String], Ty::String),
        "ori.string.chars" => (vec![Ty::String], Ty::List(Box::new(Ty::String))),
        "ori.string.index_of" => (vec![Ty::String, Ty::String], Ty::Int),
        "ori.string.join" => (vec![Ty::List(Box::new(Ty::String)), Ty::String], Ty::String),
        "ori.string.repeat" => (vec![Ty::String, Ty::Int], Ty::String),
        "ori.string.pad_left" | "ori.string.pad_right" => {
            (vec![Ty::String, Ty::Int, Ty::String], Ty::String)
        }
        "ori.string.parse_int" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori.string.parse_float" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Float), Box::new(Ty::String)),
        ),
        "ori.string.to_bytes" => (vec![Ty::String], Ty::Bytes),
        "ori.string.from_bytes" => (
            vec![Ty::Bytes],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.bytes.len" => (vec![Ty::Bytes], Ty::Int),
        "ori.bytes.concat" => (vec![Ty::Bytes, Ty::Bytes], Ty::Bytes),
        "ori.bytes.slice" => (vec![Ty::Bytes, Ty::Int, Ty::Int], Ty::Bytes),
        "ori.bytes.to_hex" => (vec![Ty::Bytes], Ty::String),
        "ori.bytes.from_hex" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori.bytes.decode_utf8" => (
            vec![Ty::Bytes],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.bytes.get" => (vec![Ty::Bytes, Ty::Int], Ty::U8),
        "ori.mem.size_of" | "ori.mem.align_of" => (vec![Ty::Infer(0)], Ty::Int),
        "ori.mem.string_as_ptr" | "ori.mem.string_len" => (vec![Ty::String], Ty::Int),
        "ori.handle.null" => (vec![], Ty::Handle(Box::new(Ty::Infer(0)))),
        "ori.handle.is_null" => (vec![Ty::Handle(Box::new(Ty::Infer(0)))], Ty::Bool),
        "ori.time.now" => (vec![], Ty::Int),
        "ori.time.sleep" => (vec![Ty::Int], Ty::Void),
        "ori.time.duration_ms" => (vec![Ty::Int, Ty::Int], Ty::Int),
        "ori.format.number" | "ori.format.percent" => (vec![Ty::Float, Ty::Int], Ty::String),
        "ori.format.hex" | "ori.format.binary" => (vec![Ty::Int], Ty::String),
        "ori.format.date" => (vec![Ty::Int, Ty::String], Ty::String),
        "ori.format.datetime" => (vec![Ty::Int, Ty::String, Ty::String], Ty::String),
        "ori.format.bytes_size" => (vec![Ty::Int, Ty::String], Ty::String),
        "ori.os.args" => (vec![], Ty::List(Box::new(Ty::String))),
        "ori.os.env" => (vec![Ty::String], Ty::Optional(Box::new(Ty::String))),
        "ori.os.exit" => (vec![Ty::Int], Ty::Void),
        "ori.os.pid" => (vec![], Ty::Int),
        "ori.os.platform" | "ori.os.arch" => (vec![], Ty::String),
        "ori.random.int" => (vec![Ty::Int, Ty::Int], Ty::Int),
        "ori.random.float" => (vec![Ty::Float, Ty::Float], Ty::Float),
        "ori.random.bool" => (vec![], Ty::Bool),
        "ori.crypto.password_hash" => (vec![Ty::String], Ty::String),
        "ori.crypto.password_verify" => (vec![Ty::String, Ty::String], Ty::Bool),
        "ori.crypto.totp_generate_secret" => (vec![], Ty::String),
        "ori.crypto.totp_code" => (vec![Ty::String, Ty::Int], Ty::String),
        "ori.crypto.totp_verify" => (vec![Ty::String, Ty::String, Ty::Int, Ty::Int], Ty::Bool),
        "ori.random.choice" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::Optional(Box::new(Ty::Infer(0))),
        ),
        "ori.random.shuffle" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.lazy.once" => (
            vec![Ty::Func {
                params: vec![],
                ret: Box::new(Ty::Infer(0)),
            }],
            Ty::Lazy(Box::new(Ty::Infer(0))),
        ),
        "ori.lazy.force" => (vec![Ty::Lazy(Box::new(Ty::Infer(0)))], Ty::Infer(0)),
        "ori.lazy.is_consumed" => (vec![Ty::Lazy(Box::new(Ty::Infer(0)))], Ty::Bool),
        "ori.task.spawn" => (
            vec![Ty::Func {
                params: vec![],
                ret: Box::new(Ty::Infer(0)),
            }],
            Ty::TaskJob(Box::new(Ty::Infer(0))),
        ),
        "ori.task.join" => (
            vec![Ty::TaskJob(Box::new(Ty::Infer(0)))],
            Ty::Result(Box::new(Ty::Infer(0)), Box::new(Ty::TaskJoinError)),
        ),
        "ori.task.detach" => (vec![Ty::TaskJob(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.task.block_on" => (vec![Ty::Future(Box::new(Ty::Infer(0)))], Ty::Infer(0)),
        "ori.task.run_blocking" => (
            vec![Ty::Func {
                params: vec![],
                ret: Box::new(Ty::Infer(0)),
            }],
            Ty::TaskJob(Box::new(Ty::Infer(0))),
        ),
        "ori.task.sleep" => (vec![Ty::Int], Ty::Future(Box::new(Ty::Void))),
        "ori.task.create_token" => (vec![], cancel_token_ty()),
        "ori.task.cancel" => (vec![cancel_token_ty()], Ty::Void),
        "ori.task.is_cancelled" => (vec![cancel_token_ty()], Ty::Bool),
        "ori.task.associate" => (
            vec![cancel_token_ty(), Ty::Future(Box::new(Ty::Infer(0)))],
            Ty::Void,
        ),
        "ori.channel.create" => (vec![], Ty::Channel(Box::new(Ty::Infer(0)))),
        "ori.channel.create_bounded" => (
            vec![Ty::Int],
            Ty::Optional(Box::new(Ty::Channel(Box::new(Ty::Infer(0))))),
        ),
        "ori.channel.send" => (
            vec![Ty::Channel(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::ChannelSendError)),
        ),
        "ori.channel.__send_managed" => (
            vec![Ty::Channel(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::ChannelSendError)),
        ),
        "ori.channel.receive" => (
            vec![Ty::Channel(Box::new(Ty::Infer(0)))],
            Ty::Result(Box::new(Ty::Infer(0)), Box::new(Ty::ChannelReceiveError)),
        ),
        "ori.channel.close" => (vec![Ty::Channel(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.atomic.new" => (vec![Ty::Int], Ty::AtomicInt),
        "ori.atomic.load" => (vec![Ty::AtomicInt], Ty::Int),
        "ori.atomic.store" => (vec![Ty::AtomicInt, Ty::Int], Ty::Void),
        "ori.atomic.add" => (vec![Ty::AtomicInt, Ty::Int], Ty::Int),
        "ori.test.assert" => (vec![Ty::Bool, Ty::String], Ty::Void),
        "ori.test.assert_eq" | "ori.test.assert_ne" => (vec![Ty::Infer(0), Ty::Infer(0)], Ty::Void),
        "ori.test.fail" => (vec![Ty::String], Ty::Never),
        "ori.test.live_allocations" => (vec![], Ty::Int),
        "ori.test.collect_cycles" => (vec![], Ty::Int),
        "ori.test.assert_no_leaks" => (vec![Ty::String], Ty::Int),
        "ori.panic" => (vec![Ty::String], Ty::Never),
        "ori.list.new" => (vec![], Ty::List(Box::new(Ty::Infer(0)))),
        "ori.list.with_capacity" => (vec![Ty::Int], Ty::List(Box::new(Ty::Infer(0)))),
        "ori.list.push" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.list.get" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int],
            Ty::Infer(0),
        ),
        "ori.list.window" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Int],
            Ty::Slice(Box::new(Ty::Infer(0))),
        ),
        "ori.slice.len" => (vec![Ty::Slice(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori.slice.get" => (
            vec![Ty::Slice(Box::new(Ty::Infer(0))), Ty::Int],
            Ty::Infer(0),
        ),
        "ori.list.try_get" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int],
            Ty::Optional(Box::new(Ty::Infer(0))),
        ),
        "ori.list.set" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.list.len" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori.list.capacity" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori.list.reserve" => (vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int], Ty::Void),
        "ori.list.is_empty" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Bool),
        "ori.list.clear" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.list.clone" | "ori.list.to_list" | "ori.list.from_list" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.list.free" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.buffer.new" => (vec![Ty::Int], Ty::Buffer(Box::new(Ty::Infer(0)))),
        "ori.buffer.len" => (vec![Ty::Buffer(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori.buffer.is_empty" => (vec![Ty::Buffer(Box::new(Ty::Infer(0)))], Ty::Bool),
        "ori.buffer.get" => (
            vec![Ty::Buffer(Box::new(Ty::Infer(0))), Ty::Int],
            Ty::Infer(0),
        ),
        "ori.buffer.set" => (
            vec![Ty::Buffer(Box::new(Ty::Infer(0))), Ty::Int, Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.buffer.fill" => (
            vec![Ty::Buffer(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.buffer.as_slice" => (
            vec![Ty::Buffer(Box::new(Ty::Infer(0)))],
            Ty::Slice(Box::new(Ty::Infer(0))),
        ),
        "ori.list.pop" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Infer(0)),
        "ori.list.try_pop" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::Optional(Box::new(Ty::Infer(0))),
        ),
        "ori.list.remove" => (vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int], Ty::Void),
        "ori.list.try_remove" => (vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int], Ty::Bool),
        "ori.list.insert" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.list.contains" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Bool,
        ),
        "ori.list.index_of" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Int,
        ),
        "ori.list.sort" | "ori.list.reverse" => (vec![Ty::List(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.list.slice" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int, Ty::Int],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.list.map" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Infer(1)),
                },
            ],
            Ty::List(Box::new(Ty::Infer(1))),
        ),
        "ori.list.filter" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.map" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Infer(0)),
                },
            ],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.filter" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.any" | "ori.iter.all" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::Bool,
        ),
        "ori.iter.count_where" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::Int,
        ),
        "ori.iter.take" | "ori.iter.skip" => (
            vec![Ty::List(Box::new(Ty::Infer(0))), Ty::Int],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.reverse" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.reduce" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Infer(0),
                Ty::Func {
                    params: vec![Ty::Infer(0), Ty::Infer(0)],
                    ret: Box::new(Ty::Infer(0)),
                },
            ],
            Ty::Infer(0),
        ),
        "ori.iter.find" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::Optional(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.flat_map" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::List(Box::new(Ty::Infer(0)))),
                },
            ],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.sort" | "ori.iter.unique" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.sort_by" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0), Ty::Infer(0)],
                    ret: Box::new(Ty::Int),
                },
            ],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.iter.zip" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::List(Box::new(Ty::Infer(0))),
            ],
            Ty::List(Box::new(Ty::Tuple(vec![Ty::Infer(0), Ty::Infer(0)]))),
        ),
        "ori.iter.partition" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Bool),
                },
            ],
            Ty::Tuple(vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::List(Box::new(Ty::Infer(0))),
            ]),
        ),
        "ori.iter.group_by" => (
            vec![
                Ty::List(Box::new(Ty::Infer(0))),
                Ty::Func {
                    params: vec![Ty::Infer(0)],
                    ret: Box::new(Ty::Infer(0)),
                },
            ],
            Ty::Map(
                Box::new(Ty::Infer(0)),
                Box::new(Ty::List(Box::new(Ty::Infer(0)))),
            ),
        ),
        "ori.iter.flatten" => (
            vec![Ty::List(Box::new(Ty::List(Box::new(Ty::Infer(0)))))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.json.parse" => (
            vec![Ty::String],
            Ty::Result(
                Box::new(Ty::Named(DefId::SYNTHETIC_JSON_VALUE, Vec::new())),
                Box::new(Ty::String),
            ),
        ),
        "ori.json.stringify" => (
            vec![Ty::Named(DefId::SYNTHETIC_JSON_VALUE, Vec::new())],
            Ty::String,
        ),
        "ori.json.stringify_pretty" => (
            vec![Ty::Named(DefId::SYNTHETIC_JSON_VALUE, Vec::new())],
            Ty::String,
        ),
        path if list_backed_collection_kind(path, &["new"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["new"]).unwrap();
            (vec![], opaque_collection(kind))
        }
        path if list_backed_collection_kind(
            path,
            &["push_front", "push_back", "enqueue", "push"],
        )
        .is_some() =>
        {
            let kind =
                list_backed_collection_kind(path, &["push_front", "push_back", "enqueue", "push"])
                    .unwrap();
            (vec![opaque_collection(kind), Ty::Infer(0)], Ty::Void)
        }
        path if list_backed_collection_kind(path, &["insert_after", "insert_before"]).is_some() => {
            let kind =
                list_backed_collection_kind(path, &["insert_after", "insert_before"]).unwrap();
            (
                vec![opaque_collection(kind), Ty::Int, Ty::Infer(0)],
                Ty::Bool,
            )
        }
        path if list_backed_collection_kind(
            path,
            &[
                "pop_front",
                "pop_back",
                "front",
                "back",
                "dequeue",
                "peek",
                "pop",
            ],
        )
        .is_some() =>
        {
            let kind = list_backed_collection_kind(
                path,
                &[
                    "pop_front",
                    "pop_back",
                    "front",
                    "back",
                    "dequeue",
                    "peek",
                    "pop",
                ],
            )
            .unwrap();
            (
                vec![opaque_collection(kind)],
                Ty::Optional(Box::new(Ty::Infer(0))),
            )
        }
        path if list_backed_collection_kind(path, &["value_at", "remove_at"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["value_at", "remove_at"]).unwrap();
            (
                vec![opaque_collection(kind), Ty::Int],
                Ty::Optional(Box::new(Ty::Infer(0))),
            )
        }
        path if list_backed_collection_kind(path, &["cursor_front", "cursor_back"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["cursor_front", "cursor_back"]).unwrap();
            (
                vec![opaque_collection(kind)],
                Ty::Optional(Box::new(Ty::Int)),
            )
        }
        path if list_backed_collection_kind(path, &["find"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["find"]).unwrap();
            (
                vec![opaque_collection(kind), Ty::Infer(0)],
                Ty::Optional(Box::new(Ty::Int)),
            )
        }
        path if list_backed_collection_kind(path, &["len"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["len"]).unwrap();
            (vec![opaque_collection(kind)], Ty::Int)
        }
        path if list_backed_collection_kind(path, &["is_empty"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["is_empty"]).unwrap();
            (vec![opaque_collection(kind)], Ty::Bool)
        }
        path if list_backed_collection_kind(path, &["clear"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["clear"]).unwrap();
            (vec![opaque_collection(kind)], Ty::Void)
        }
        path if list_backed_collection_kind(path, &["clone"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["clone"]).unwrap();
            (vec![opaque_collection(kind)], opaque_collection(kind))
        }
        path if list_backed_collection_kind(path, &["to_list"]).is_some() => {
            let kind = list_backed_collection_kind(path, &["to_list"]).unwrap();
            (
                vec![opaque_collection(kind)],
                Ty::List(Box::new(Ty::Infer(0))),
            )
        }
        "ori.tree.new" => (vec![Ty::Infer(0)], tree_ty()),
        "ori.tree.root" => (vec![tree_ty()], node_id_ty()),
        "ori.tree.value" => (vec![tree_ty(), node_id_ty()], Ty::Infer(0)),
        "ori.tree.try_value" => (
            vec![tree_ty(), node_id_ty()],
            Ty::Optional(Box::new(Ty::Infer(0))),
        ),
        "ori.tree.contains_node" => (vec![tree_ty(), node_id_ty()], Ty::Bool),
        "ori.tree.set_value" => (vec![tree_ty(), node_id_ty(), Ty::Infer(0)], Ty::Bool),
        "ori.tree.add_child" => (vec![tree_ty(), node_id_ty(), Ty::Infer(0)], node_id_ty()),
        "ori.tree.children" => (
            vec![tree_ty(), node_id_ty()],
            Ty::List(Box::new(node_id_ty())),
        ),
        "ori.tree.parent" => (
            vec![tree_ty(), node_id_ty()],
            Ty::Optional(Box::new(node_id_ty())),
        ),
        "ori.tree.remove_subtree" => (vec![tree_ty(), node_id_ty()], Ty::Void),
        "ori.tree.move_subtree" => (vec![tree_ty(), node_id_ty(), node_id_ty()], Ty::Bool),
        "ori.tree.find" => (
            vec![tree_ty(), Ty::Infer(0)],
            Ty::Optional(Box::new(node_id_ty())),
        ),
        "ori.tree.len" => (vec![tree_ty()], Ty::Int),
        "ori.tree.depth" => (vec![tree_ty(), node_id_ty()], Ty::Int),
        "ori.tree.pre_order" | "ori.tree.post_order" | "ori.tree.breadth_first" => {
            (vec![tree_ty()], Ty::List(Box::new(node_id_ty())))
        }
        "ori.tree.clone" => (vec![tree_ty()], tree_ty()),
        "ori.tree.clone_subtree" => (vec![tree_ty(), node_id_ty()], tree_ty()),
        "ori.set.new" => (vec![], Ty::Set(Box::new(Ty::Infer(0)))),
        "ori.set.add" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.set.contains" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Bool,
        ),
        "ori.set.len" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori.set.is_empty" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Bool),
        "ori.set.capacity" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Int),
        "ori.set.reserve" => (vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Int], Ty::Void),
        "ori.set.clear" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.set.clone" => (
            vec![Ty::Set(Box::new(Ty::Infer(0)))],
            Ty::Set(Box::new(Ty::Infer(0))),
        ),
        "ori.set.to_list" => (
            vec![Ty::Set(Box::new(Ty::Infer(0)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.set.from_list" => (
            vec![Ty::List(Box::new(Ty::Infer(0)))],
            Ty::Set(Box::new(Ty::Infer(0))),
        ),
        "ori.set.free" => (vec![Ty::Set(Box::new(Ty::Infer(0)))], Ty::Void),
        "ori.set.remove" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Void,
        ),
        "ori.set.try_remove" => (
            vec![Ty::Set(Box::new(Ty::Infer(0))), Ty::Infer(0)],
            Ty::Bool,
        ),
        "ori.set.union" | "ori.set.intersection" | "ori.set.difference" => (
            vec![
                Ty::Set(Box::new(Ty::Infer(0))),
                Ty::Set(Box::new(Ty::Infer(0))),
            ],
            Ty::Set(Box::new(Ty::Infer(0))),
        ),
        "ori.map.new" => (
            vec![],
            Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
        ),
        "ori.map.set" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Infer(0),
                Ty::Infer(1),
            ],
            Ty::Void,
        ),
        "ori.map.get" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Infer(0),
            ],
            Ty::Infer(1),
        ),
        "ori.map.try_get" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Infer(0),
            ],
            Ty::Optional(Box::new(Ty::Infer(1))),
        ),
        "ori.map.contains" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Infer(0),
            ],
            Ty::Bool,
        ),
        "ori.map.len" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::Int,
        ),
        "ori.map.is_empty" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::Bool,
        ),
        "ori.map.capacity" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::Int,
        ),
        "ori.map.reserve" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Int,
            ],
            Ty::Void,
        ),
        "ori.map.clear" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::Void,
        ),
        "ori.map.clone" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
        ),
        "ori.map.from_entries" => (
            vec![Ty::List(Box::new(Ty::Tuple(vec![
                Ty::Infer(0),
                Ty::Infer(1),
            ])))],
            Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
        ),
        "ori.map.free" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::Void,
        ),
        "ori.map.remove" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Infer(0),
            ],
            Ty::Void,
        ),
        "ori.map.try_remove" => (
            vec![
                Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1))),
                Ty::Infer(0),
            ],
            Ty::Optional(Box::new(Ty::Infer(1))),
        ),
        "ori.map.keys" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.map.values" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::List(Box::new(Ty::Infer(1))),
        ),
        "ori.map.entries" => (
            vec![Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(1)))],
            Ty::List(Box::new(Ty::Tuple(vec![Ty::Infer(0), Ty::Infer(1)]))),
        ),
        "ori.hash_table.new" => (vec![], hash_table_ty()),
        "ori.hash_table.with_capacity" => (vec![Ty::Int], hash_table_ty()),
        "ori.hash_table.set" => (vec![hash_table_ty(), Ty::Infer(0), Ty::Infer(1)], Ty::Void),
        "ori.hash_table.get" | "ori.hash_table.remove" => (
            vec![hash_table_ty(), Ty::Infer(0)],
            Ty::Optional(Box::new(Ty::Infer(1))),
        ),
        "ori.hash_table.contains" => (vec![hash_table_ty(), Ty::Infer(0)], Ty::Bool),
        "ori.hash_table.len" | "ori.hash_table.capacity" => (vec![hash_table_ty()], Ty::Int),
        "ori.hash_table.is_empty" => (vec![hash_table_ty()], Ty::Bool),
        "ori.hash_table.reserve" => (vec![hash_table_ty(), Ty::Int], Ty::Void),
        "ori.hash_table.clear" => (vec![hash_table_ty()], Ty::Void),
        "ori.hash_table.clone" => (vec![hash_table_ty()], hash_table_ty()),
        "ori.hash_table.from_entries" => (
            vec![Ty::List(Box::new(Ty::Tuple(vec![
                Ty::Infer(0),
                Ty::Infer(1),
            ])))],
            hash_table_ty(),
        ),
        "ori.hash_table.keys" => (vec![hash_table_ty()], Ty::List(Box::new(Ty::Infer(0)))),
        "ori.hash_table.values" => (vec![hash_table_ty()], Ty::List(Box::new(Ty::Infer(1)))),
        "ori.hash_table.entries" => (
            vec![hash_table_ty()],
            Ty::List(Box::new(Ty::Tuple(vec![Ty::Infer(0), Ty::Infer(1)]))),
        ),
        "ori.graph.new" => (vec![Ty::Bool], graph_ty()),
        "ori.graph.add_node" | "ori.graph.remove_node" => {
            (vec![graph_ty(), Ty::Infer(0)], Ty::Void)
        }
        "ori.graph.add_edge" | "ori.graph.remove_edge" => {
            (vec![graph_ty(), Ty::Infer(0), Ty::Infer(0)], Ty::Void)
        }
        "ori.graph.add_weighted_edge" => (
            vec![graph_ty(), Ty::Infer(0), Ty::Infer(0), Ty::Int],
            Ty::Void,
        ),
        "ori.graph.has_node" => (vec![graph_ty(), Ty::Infer(0)], Ty::Bool),
        "ori.graph.has_edge" => (vec![graph_ty(), Ty::Infer(0), Ty::Infer(0)], Ty::Bool),
        "ori.graph.edge_weight" => (
            vec![graph_ty(), Ty::Infer(0), Ty::Infer(0)],
            Ty::Optional(Box::new(Ty::Int)),
        ),
        "ori.graph.neighbors" | "ori.graph.bfs" | "ori.graph.dfs" => (
            vec![graph_ty(), Ty::Infer(0)],
            Ty::List(Box::new(Ty::Infer(0))),
        ),
        "ori.graph.nodes" | "ori.graph.topological_sort" => {
            (vec![graph_ty()], Ty::List(Box::new(Ty::Infer(0))))
        }
        "ori.graph.try_topological_sort"
        | "ori.graph.shortest_path"
        | "ori.graph.shortest_weighted_path" => (
            if path == "ori.graph.shortest_path" || path == "ori.graph.shortest_weighted_path" {
                vec![graph_ty(), Ty::Infer(0), Ty::Infer(0)]
            } else {
                vec![graph_ty()]
            },
            Ty::Optional(Box::new(Ty::List(Box::new(Ty::Infer(0))))),
        ),
        "ori.graph.is_directed" | "ori.graph.has_cycle" => (vec![graph_ty()], Ty::Bool),
        "ori.graph.len" | "ori.graph.edge_len" => (vec![graph_ty()], Ty::Int),
        "ori.graph.components" | "ori.graph.strongly_connected_components" => (
            vec![graph_ty()],
            Ty::List(Box::new(Ty::List(Box::new(Ty::Infer(0))))),
        ),
        "ori.graph.transitive_closure" | "ori.graph.clone" => (vec![graph_ty()], graph_ty()),
        "ori.graph.edges" => (
            vec![graph_ty()],
            Ty::List(Box::new(Ty::Tuple(vec![Ty::Infer(0), Ty::Infer(0)]))),
        ),
        "ori.heap.new" => (vec![], heap_ty()),
        "ori.heap.push" => (vec![heap_ty(), Ty::Infer(0)], Ty::Void),
        "ori.heap.pop" | "ori.heap.peek" => (vec![heap_ty()], Ty::Optional(Box::new(Ty::Infer(0)))),
        "ori.heap.len" => (vec![heap_ty()], Ty::Int),
        "ori.heap.is_empty" => (vec![heap_ty()], Ty::Bool),
        "ori.heap.clear" => (vec![heap_ty()], Ty::Void),
        "ori.heap.clone" => (vec![heap_ty()], heap_ty()),
        "ori.heap.to_list" | "ori.heap.into_sorted_list" => {
            (vec![heap_ty()], Ty::List(Box::new(Ty::Infer(0))))
        }
        "ori.heap.from_list" => (vec![Ty::List(Box::new(Ty::Infer(0)))], heap_ty()),
        "ori.heap.merge" => (vec![heap_ty(), heap_ty()], heap_ty()),
        "ori.heap.remove" => (vec![heap_ty(), Ty::Infer(0)], Ty::Bool),
        "ori.math.sqrt" => (vec![Ty::Float], Ty::Float),
        "ori.math.abs" => (vec![Ty::Int], Ty::Int),
        "ori.math.min" | "ori.math.max" => (vec![Ty::Int, Ty::Int], Ty::Int),
        "ori.math.clamp" => (vec![Ty::Int, Ty::Int, Ty::Int], Ty::Int),
        "ori.math.pow" => (vec![Ty::Float, Ty::Float], Ty::Float),
        "ori.math.floor" | "ori.math.ceil" | "ori.math.round" => (vec![Ty::Float], Ty::Int),
        "ori.math.log" | "ori.math.log2" | "ori.math.sin" | "ori.math.cos" | "ori.math.tan" => {
            (vec![Ty::Float], Ty::Float)
        }
        "ori.math.is_nan" | "ori.math.is_infinite" => (vec![Ty::Float], Ty::Bool),
        "ori.convert.float_to_string" => (vec![Ty::Float], Ty::String),
        "ori.convert.bool_to_string" => (vec![Ty::Bool], Ty::String),
        "ori.convert.string_to_int" => (vec![Ty::String], Ty::Optional(Box::new(Ty::Int))),
        "ori.convert.string_to_float" => (vec![Ty::String], Ty::Optional(Box::new(Ty::Float))),
        "string" => (vec![Ty::Int], Ty::String),
        "int" => (vec![Ty::Int], Ty::Int),
        "float" => (vec![Ty::Int], Ty::Float),
        "len" => (vec![Ty::String], Ty::Int),
        "ori.fs.read_text" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.fs.read_text_async" => (
            vec![Ty::String],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::String),
                Box::new(Ty::String),
            ))),
        ),
        "ori.fs.write_text" => (
            vec![Ty::String, Ty::String],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.fs.write_text_async" => (
            vec![Ty::String, Ty::String],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::String),
                Box::new(Ty::String),
            ))),
        ),
        "ori.fs.read_bytes" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori.fs.write_bytes" => (
            vec![Ty::String, Ty::Bytes],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.fs.read_all" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.fs.append_text" => (
            vec![Ty::String, Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.fs.exists" | "ori.fs.is_file" | "ori.fs.is_dir" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Bool), Box::new(Ty::String)),
        ),
        "ori.fs.delete" | "ori.fs.create_dir" | "ori.fs.create_dir_all" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.fs.copy" | "ori.fs.rename" => (
            vec![Ty::String, Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.fs.list_dir" => (
            vec![Ty::String],
            Ty::Result(
                Box::new(Ty::List(Box::new(Ty::String))),
                Box::new(Ty::String),
            ),
        ),
        "ori.fs.open_read" | "ori.fs.open_write" => (
            vec![Ty::String],
            Ty::Result(Box::new(file_ty()), Box::new(Ty::String)),
        ),
        "ori.fs.read" => (
            vec![file_ty(), Ty::Int],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori.fs.write" => (
            vec![file_ty(), Ty::Bytes],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori.fs.close" => (vec![file_ty()], Ty::Void),
        "ori.fs.file_size" | "ori.fs.modified_at" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori.fs.created_at" => (
            vec![Ty::String],
            Ty::Result(
                Box::new(Ty::Optional(Box::new(Ty::Int))),
                Box::new(Ty::String),
            ),
        ),
        "ori.os.current_dir" => (
            vec![],
            Ty::Result(Box::new(Ty::String), Box::new(Ty::String)),
        ),
        "ori.os.change_dir" => (
            vec![Ty::String],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.random.seed" => (vec![Ty::Int], Ty::Void),
        "ori.test.skip" => (vec![Ty::String], Ty::Void),
        "ori.bytes.from_list" => (vec![Ty::List(Box::new(Ty::Int))], Ty::Bytes),
        "ori.bytes.to_list" => (vec![Ty::Bytes], Ty::List(Box::new(Ty::Int))),
        "ori.process.run" => (
            vec![Ty::String, Ty::List(Box::new(Ty::String))],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori.process.run_capture" => (
            vec![Ty::String, Ty::List(Box::new(Ty::String))],
            Ty::Result(
                Box::new(Ty::Map(Box::new(Ty::String), Box::new(Ty::String))),
                Box::new(Ty::String),
            ),
        ),
        "ori.process.run_output_raw" => (
            vec![Ty::String, Ty::List(Box::new(Ty::String))],
            Ty::Result(
                Box::new(Ty::Tuple(vec![Ty::Int, Ty::Bytes, Ty::Bytes])),
                Box::new(Ty::String),
            ),
        ),
        "ori.net.connect" => (
            vec![Ty::String, Ty::Int, Ty::Int],
            Ty::Result(Box::new(connection_ty()), Box::new(Ty::String)),
        ),
        "ori.net.connect_tls" => (
            vec![Ty::String, Ty::Int, Ty::Int],
            Ty::Result(Box::new(connection_ty()), Box::new(Ty::String)),
        ),
        "ori.net.connect_async" | "ori.net.connect_tls_async" => (
            vec![Ty::String, Ty::Int, Ty::Int],
            Ty::Future(Box::new(Ty::Result(
                Box::new(connection_ty()),
                Box::new(Ty::String),
            ))),
        ),
        "ori.net.listen" => (
            vec![Ty::String, Ty::Int],
            Ty::Result(Box::new(listener_ty()), Box::new(Ty::String)),
        ),
        "ori.net.accept" => (
            vec![listener_ty()],
            Ty::Result(Box::new(connection_ty()), Box::new(Ty::String)),
        ),
        "ori.net.accept_async" => (
            vec![listener_ty()],
            Ty::Future(Box::new(Ty::Result(
                Box::new(connection_ty()),
                Box::new(Ty::String),
            ))),
        ),
        "ori.net.close_listener" => (vec![listener_ty()], Ty::Void),
        "ori.net.listener_port" => (vec![listener_ty()], Ty::Int),
        "ori.net.udp_bind" => (
            vec![Ty::String, Ty::Int],
            Ty::Result(Box::new(udp_socket_ty()), Box::new(Ty::String)),
        ),
        "ori.net.udp_send_to" => (
            vec![udp_socket_ty(), Ty::String, Ty::Int, Ty::Bytes],
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
        ),
        "ori.net.udp_send_to_async" => (
            vec![udp_socket_ty(), Ty::String, Ty::Int, Ty::Bytes],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Int),
                Box::new(Ty::String),
            ))),
        ),
        "ori.net.udp_recv_from" => (
            vec![udp_socket_ty(), Ty::Int],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori.net.udp_recv_from_async" => (
            vec![udp_socket_ty(), Ty::Int],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Bytes),
                Box::new(Ty::String),
            ))),
        ),
        "ori.net.udp_close" => (vec![udp_socket_ty()], Ty::Void),
        "ori.net.udp_local_port" => (vec![udp_socket_ty()], Ty::Int),
        "ori.net.read_some" => (
            vec![connection_ty(), Ty::Int],
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
        ),
        "ori.net.read_some_async" => (
            vec![connection_ty(), Ty::Int],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Bytes),
                Box::new(Ty::String),
            ))),
        ),
        "ori.net.set_read_timeout_ms" | "ori.net.set_write_timeout_ms" => (
            vec![connection_ty(), Ty::Int],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.net.write_all" => (
            vec![connection_ty(), Ty::Bytes],
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
        ),
        "ori.net.write_all_async" => (
            vec![connection_ty(), Ty::Bytes],
            Ty::Future(Box::new(Ty::Result(
                Box::new(Ty::Int),
                Box::new(Ty::String),
            ))),
        ),
        "ori.net.close" => (vec![connection_ty()], Ty::Void),
        "ori.net.is_closed" => (vec![connection_ty()], Ty::Bool),
        "ori.math.trunc" | "ori.math.ln" | "ori.math.exp" | "ori.math.asin" | "ori.math.acos"
        | "ori.math.atan" => (vec![Ty::Float], Ty::Float),
        "ori.math.atan2" => (vec![Ty::Float, Ty::Float], Ty::Float),
        "ori.math.log10" => (vec![Ty::Float], Ty::Float),
        "ori.math.is_finite" => (vec![Ty::Float], Ty::Bool),
        _ => return None,
    };
    Some(sig)
}

pub fn stdlib_native_abi(
    runtime_symbol: &str,
) -> Option<(Vec<StdlibNativeAbiTy>, Option<StdlibNativeAbiTy>)> {
    use StdlibNativeAbiTy::*;
    let sig = match runtime_symbol {
        "ori_io_print" | "ori_io_eprint" => (vec![Ptr, I64], None),
        "ori_io_read_line" => (vec![], Some(Ptr)),
        "ori_io_stdin" | "ori_io_stdout" | "ori_io_stderr" => (vec![], Some(Ptr)),
        "ori_io_open_input" | "ori_io_open_output" => (vec![Ptr], Some(Ptr)),
        "ori_io_read" => (vec![Ptr, I64], Some(Ptr)),
        "ori_io_write" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_io_flush" => (vec![Ptr], Some(Ptr)),
        "ori_io_close_input" | "ori_io_close_output" => (vec![Ptr], None),
        "ori_string_len" | "ori_len" => (vec![Ptr], Some(I64)),
        "ori_handle_null" => (vec![], Some(Ptr)),
        "ori_handle_is_null" => (vec![Ptr], Some(I8)),
        "ori_string_concat" | "ori_string_split" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_string_slice" => (vec![Ptr, I64, I64], Some(Ptr)),
        "ori_string_contains" | "ori_string_starts_with" | "ori_string_ends_with" => {
            (vec![Ptr, Ptr], Some(I8))
        }
        "ori_string_is_ascii" => (vec![Ptr], Some(I8)),
        "ori_string_trim"
        | "ori_string_trim_start"
        | "ori_string_trim_end"
        | "ori_string_to_upper"
        | "ori_string_to_lower"
        | "ori_string_case_fold"
        | "ori_string_chars"
        | "ori_bytes_to_hex"
        | "ori_bytes_from_hex"
        | "ori_bytes_decode_utf8"
        | "ori_string_to_bytes"
        | "ori_string_from_bytes"
        | "ori_string_to_int"
        | "ori_string_to_float"
        | "ori_string_parse_int"
        | "ori_string_parse_float"
        | "ori_files_read_text"
        | "ori_files_read_text_async"
        | "ori_files_read_bytes"
        | "ori_files_read_all"
        | "ori_files_list_dir"
        | "ori_err_trace_format" => (vec![Ptr], Some(Ptr)),
        "ori_err_trace_push" => (vec![Ptr, I64, Ptr], Some(Ptr)),
        "ori_bytes_len" => (vec![Ptr], Some(I64)),
        "ori_string_replace" => (vec![Ptr, Ptr, Ptr], Some(Ptr)),
        "ori_string_index_of" => (vec![Ptr, Ptr], Some(I64)),
        "ori_string_join" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_string_repeat" => (vec![Ptr, I64], Some(Ptr)),
        "ori_string_pad_left" | "ori_string_pad_right" => (vec![Ptr, I64, Ptr], Some(Ptr)),
        "ori_to_string" => (vec![I64], Some(Ptr)),
        "ori_to_int" => (vec![I64], Some(I64)),
        "ori_to_float" => (vec![I64], Some(F64)),
        "ori_float_to_string" => (vec![F64], Some(Ptr)),
        "ori_bool_to_string" => (vec![I8], Some(Ptr)),
        "ori_bytes_concat" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_bytes_slice" => (vec![Ptr, I64, I64], Some(Ptr)),
        "ori_bytes_get" => (vec![Ptr, I64], Some(I8)),
        "ori_list_new"
        | "ori_deque_new"
        | "ori_queue_new"
        | "ori_stack_new"
        | "ori_linked_list_new"
        | "ori_doubly_linked_list_new"
        | "ori_set_new"
        | "ori_map_new"
        | "ori_os_args" => (vec![], Some(Ptr)),
        "ori_list_clone"
        | "ori_list_to_list"
        | "ori_list_from_list"
        | "ori_deque_clone"
        | "ori_queue_clone"
        | "ori_stack_clone"
        | "ori_linked_list_clone"
        | "ori_doubly_linked_list_clone"
        | "ori_set_clone"
        | "ori_map_clone" => (vec![Ptr], Some(Ptr)),
        "ori_list_push"
        | "ori_deque_push_front"
        | "ori_deque_push_back"
        | "ori_queue_enqueue"
        | "ori_stack_push"
        | "ori_linked_list_push_front"
        | "ori_linked_list_push_back"
        | "ori_doubly_linked_list_push_front"
        | "ori_doubly_linked_list_push_back"
        | "ori_set_add" => (vec![Ptr, I64], None),
        "ori_linked_list_insert_after"
        | "ori_doubly_linked_list_insert_after"
        | "ori_doubly_linked_list_insert_before" => (vec![Ptr, I64, I64], Some(I8)),
        "ori_list_get" | "ori_map_get" => (vec![Ptr, I64], Some(I64)),
        "ori_slice_new" => (vec![Ptr, I64, I64], Some(Ptr)),
        "ori_slice_len" => (vec![Ptr], Some(I64)),
        "ori_slice_get" => (vec![Ptr, I64], Some(I64)),
        "ori_list_try_get" | "ori_map_try_get" | "ori_map_try_remove" => {
            (vec![Ptr, I64], Some(Ptr))
        }
        "ori_linked_list_value_at"
        | "ori_linked_list_remove_at"
        | "ori_linked_list_find"
        | "ori_doubly_linked_list_value_at"
        | "ori_doubly_linked_list_remove_at"
        | "ori_doubly_linked_list_find" => (vec![Ptr, I64], Some(Ptr)),
        "ori_list_try_pop" => (vec![Ptr], Some(Ptr)),
        "ori_list_pop" => (vec![Ptr], Some(I64)),
        "ori_deque_pop_front"
        | "ori_deque_pop_back"
        | "ori_deque_front"
        | "ori_deque_back"
        | "ori_queue_dequeue"
        | "ori_queue_peek"
        | "ori_stack_pop"
        | "ori_stack_peek"
        | "ori_linked_list_pop_front"
        | "ori_linked_list_front"
        | "ori_linked_list_cursor_front"
        | "ori_linked_list_cursor_back"
        | "ori_doubly_linked_list_pop_front"
        | "ori_doubly_linked_list_pop_back"
        | "ori_doubly_linked_list_front"
        | "ori_doubly_linked_list_back"
        | "ori_doubly_linked_list_cursor_front"
        | "ori_doubly_linked_list_cursor_back" => (vec![Ptr], Some(Ptr)),
        "ori_list_set" | "ori_list_insert" | "ori_map_set" => (vec![Ptr, I64, I64], None),
        "ori_list_len"
        | "ori_list_capacity"
        | "ori_deque_len"
        | "ori_queue_len"
        | "ori_stack_len"
        | "ori_linked_list_len"
        | "ori_doubly_linked_list_len"
        | "ori_set_len"
        | "ori_map_len"
        | "ori_set_capacity"
        | "ori_map_capacity" => (vec![Ptr], Some(I64)),
        "ori_list_is_empty" | "ori_set_is_empty" | "ori_map_is_empty" => (vec![Ptr], Some(I8)),
        "ori_deque_is_empty"
        | "ori_queue_is_empty"
        | "ori_stack_is_empty"
        | "ori_linked_list_is_empty"
        | "ori_doubly_linked_list_is_empty" => (vec![Ptr], Some(I8)),
        "ori_list_free"
        | "ori_set_free"
        | "ori_map_free"
        | "ori_list_clear"
        | "ori_set_clear"
        | "ori_map_clear"
        | "ori_deque_clear"
        | "ori_queue_clear"
        | "ori_stack_clear"
        | "ori_list_sort"
        | "ori_linked_list_clear"
        | "ori_doubly_linked_list_clear"
        | "ori_list_reverse" => (vec![Ptr], None),
        "ori_deque_to_list"
        | "ori_queue_to_list"
        | "ori_stack_to_list"
        | "ori_linked_list_to_list"
        | "ori_doubly_linked_list_to_list" => (vec![Ptr], Some(Ptr)),
        "ori_tree_new" => (vec![I64], Some(Ptr)),
        "ori_tree_root" | "ori_tree_len" => (vec![Ptr], Some(I64)),
        "ori_tree_value" | "ori_tree_depth" => (vec![Ptr, I64], Some(I64)),
        "ori_tree_try_value" | "ori_tree_parent" | "ori_tree_find" => (vec![Ptr, I64], Some(Ptr)),
        "ori_tree_contains_node" => (vec![Ptr, I64], Some(I8)),
        "ori_tree_add_child" => (vec![Ptr, I64, I64], Some(I64)),
        "ori_tree_set_value" => (vec![Ptr, I64, I64], Some(I8)),
        "ori_tree_move_subtree" => (vec![Ptr, I64, I64], Some(I8)),
        "ori_tree_clone" => (vec![Ptr], Some(Ptr)),
        "ori_tree_clone_subtree" => (vec![Ptr, I64], Some(Ptr)),
        "ori_tree_children" | "ori_tree_remove_subtree" => {
            let ret = if runtime_symbol == "ori_tree_remove_subtree" {
                None
            } else {
                Some(Ptr)
            };
            (vec![Ptr, I64], ret)
        }
        "ori_tree_pre_order" | "ori_tree_post_order" | "ori_tree_breadth_first" => {
            (vec![Ptr], Some(Ptr))
        }
        "ori_list_reserve" | "ori_set_reserve" | "ori_map_reserve" => (vec![Ptr, I64], None),
        "ori_list_with_capacity" => (vec![I64], Some(Ptr)),
        "ori_list_remove" | "ori_set_remove" | "ori_map_remove" => (vec![Ptr, I64], None),
        "ori_list_try_remove" | "ori_set_try_remove" => (vec![Ptr, I64], Some(I8)),
        "ori_list_contains" | "ori_list_index_of" | "ori_set_contains" | "ori_map_contains" => (
            vec![Ptr, I64],
            Some(if runtime_symbol.ends_with("index_of") {
                I64
            } else {
                I8
            }),
        ),
        "ori_list_slice" => (vec![Ptr, I64, I64], Some(Ptr)),
        "ori_buffer_new" => (vec![I64], Some(Ptr)),
        "ori_buffer_len" => (vec![Ptr], Some(I64)),
        "ori_buffer_is_empty" => (vec![Ptr], Some(I8)),
        "ori_buffer_get" => (vec![Ptr, I64], Some(I64)),
        "ori_buffer_set" => (vec![Ptr, I64, I64], None),
        "ori_buffer_fill" => (vec![Ptr, I64], None),
        "ori_buffer_as_slice" => (vec![Ptr], Some(Ptr)),
        "ori_set_union" | "ori_set_intersection" | "ori_set_difference" => {
            (vec![Ptr, Ptr], Some(Ptr))
        }
        "ori_set_to_list" => (vec![Ptr], Some(Ptr)),
        "ori_set_from_list" => (vec![Ptr], Some(Ptr)),
        "ori_map_from_entries" => (vec![Ptr], Some(Ptr)),
        "ori_hash_table_new" => (vec![], Some(Ptr)),
        "ori_hash_table_with_capacity" => (vec![I64], Some(Ptr)),
        "ori_hash_table_set" => (vec![Ptr, I64, I64], None),
        "ori_hash_table_get" | "ori_hash_table_remove" => (vec![Ptr, I64], Some(Ptr)),
        "ori_hash_table_contains" => (vec![Ptr, I64], Some(I8)),
        "ori_hash_table_len" | "ori_hash_table_capacity" => (vec![Ptr], Some(I64)),
        "ori_hash_table_is_empty" => (vec![Ptr], Some(I8)),
        "ori_hash_table_reserve" => (vec![Ptr, I64], None),
        "ori_hash_table_clear" => (vec![Ptr], None),
        "ori_hash_table_clone" => (vec![Ptr], Some(Ptr)),
        "ori_hash_table_from_entries" => (vec![Ptr], Some(Ptr)),
        "ori_hash_table_keys" | "ori_hash_table_values" | "ori_hash_table_entries" => {
            (vec![Ptr], Some(Ptr))
        }
        "ori_graph_new" => (vec![I8], Some(Ptr)),
        "ori_graph_add_node" | "ori_graph_remove_node" => (vec![Ptr, I64], None),
        "ori_graph_add_edge" | "ori_graph_remove_edge" => (vec![Ptr, I64, I64], None),
        "ori_graph_add_weighted_edge" => (vec![Ptr, I64, I64, I64], None),
        "ori_graph_has_node" => (vec![Ptr, I64], Some(I8)),
        "ori_graph_has_edge" => (vec![Ptr, I64, I64], Some(I8)),
        "ori_graph_edge_weight" => (vec![Ptr, I64, I64], Some(Ptr)),
        "ori_graph_neighbors" | "ori_graph_bfs" | "ori_graph_dfs" => (vec![Ptr, I64], Some(Ptr)),
        "ori_graph_nodes" | "ori_graph_edges" | "ori_graph_topological_sort" => {
            (vec![Ptr], Some(Ptr))
        }
        "ori_graph_try_topological_sort" => (vec![Ptr], Some(Ptr)),
        "ori_graph_is_directed" | "ori_graph_has_cycle" => (vec![Ptr], Some(I8)),
        "ori_graph_len" | "ori_graph_edge_len" => (vec![Ptr], Some(I64)),
        "ori_graph_components"
        | "ori_graph_strongly_connected_components"
        | "ori_graph_transitive_closure"
        | "ori_graph_clone" => (vec![Ptr], Some(Ptr)),
        "ori_graph_shortest_path" | "ori_graph_shortest_weighted_path" => {
            (vec![Ptr, I64, I64], Some(Ptr))
        }
        "ori_heap_new" => (vec![], Some(Ptr)),
        "ori_heap_push" => (vec![Ptr, I64], None),
        "ori_heap_pop" | "ori_heap_peek" => (vec![Ptr], Some(Ptr)),
        "ori_heap_len" => (vec![Ptr], Some(I64)),
        "ori_heap_is_empty" => (vec![Ptr], Some(I8)),
        "ori_heap_clear" => (vec![Ptr], None),
        "ori_heap_clone" | "ori_heap_to_list" | "ori_heap_into_sorted_list" => {
            (vec![Ptr], Some(Ptr))
        }
        "ori_heap_from_list" => (vec![Ptr], Some(Ptr)),
        "ori_heap_merge" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_heap_remove" => (vec![Ptr, I64], Some(I8)),
        "ori_list_map" | "ori_list_filter" | "ori_iter_flat_map" => {
            (vec![Ptr, Ptr, Ptr], Some(Ptr))
        }
        "ori_iter_any" | "ori_iter_all" => (vec![Ptr, Ptr, Ptr], Some(I8)),
        "ori_iter_count_where" => (vec![Ptr, Ptr, Ptr], Some(I64)),
        "ori_iter_take" | "ori_iter_skip" => (vec![Ptr, I64], Some(Ptr)),
        "ori_iter_reverse" => (vec![Ptr], Some(Ptr)),
        "ori_iter_reduce" => (vec![Ptr, I64, Ptr, Ptr], Some(I64)),
        "ori_iter_find" => (vec![Ptr, Ptr, Ptr], Some(Ptr)),
        "ori_iter_partition" => (vec![Ptr, Ptr, Ptr], Some(Ptr)),
        "ori_iter_group_by" => (vec![Ptr, Ptr, Ptr], Some(Ptr)),
        "ori_iter_sort_by" => (vec![Ptr, Ptr, Ptr], Some(Ptr)),
        "ori_iter_zip" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_iter_sort" | "ori_iter_unique" | "ori_iter_flatten" => (vec![Ptr], Some(Ptr)),
        "ori_map_keys" | "ori_map_values" | "ori_map_entries" => (vec![Ptr], Some(Ptr)),
        "ori_math_sqrt" | "ori_math_pow" | "ori_math_log" | "ori_math_log2" | "ori_math_sin"
        | "ori_math_cos" | "ori_math_tan" => {
            if runtime_symbol == "ori_math_pow" {
                (vec![F64, F64], Some(F64))
            } else {
                (vec![F64], Some(F64))
            }
        }
        "ori_math_abs" => (vec![I64], Some(I64)),
        "ori_math_min" | "ori_math_max" => (vec![I64, I64], Some(I64)),
        "ori_math_clamp" => (vec![I64, I64, I64], Some(I64)),
        "ori_math_floor" | "ori_math_ceil" | "ori_math_round" => (vec![F64], Some(I64)),
        "ori_math_is_nan" | "ori_math_is_infinite" => (vec![F64], Some(I8)),
        "ori_time_now" | "ori_os_pid" => (vec![], Some(I64)),
        "ori_time_sleep" | "ori_os_exit" => (vec![I64], None),
        "ori_time_duration_ms" => (vec![I64, I64], Some(I64)),
        "ori_format_number" | "ori_format_percent" => (vec![F64, I64], Some(Ptr)),
        "ori_format_hex" | "ori_format_binary" => (vec![I64], Some(Ptr)),
        "ori_format_date" | "ori_format_bytes_size" => (vec![I64, Ptr], Some(Ptr)),
        "ori_format_datetime" => (vec![I64, Ptr, Ptr], Some(Ptr)),
        "ori_os_env" => (vec![Ptr], Some(Ptr)),
        "ori_os_platform" | "ori_os_arch" => (vec![], Some(Ptr)),
        "ori_random_int" => (vec![I64, I64], Some(I64)),
        "ori_random_float" => (vec![F64, F64], Some(F64)),
        "ori_random_bool" => (vec![], Some(I8)),
        "ori_random_choice" => (vec![Ptr], Some(Ptr)),
        "ori_random_shuffle" => (vec![Ptr], Some(Ptr)),
        "ori_password_hash" => (vec![Ptr], Some(Ptr)),
        "ori_password_verify" => (vec![Ptr, Ptr], Some(I8)),
        "ori_totp_generate_secret" => (vec![], Some(Ptr)),
        "ori_totp_code" => (vec![Ptr, I64], Some(Ptr)),
        "ori_totp_verify" => (vec![Ptr, Ptr, I64, I64], Some(I8)),
        "ori_json_parse" => (vec![Ptr], Some(Ptr)),
        "ori_json_stringify" => (vec![Ptr], Some(Ptr)),
        "ori_json_stringify_pretty" => (vec![Ptr], Some(Ptr)),
        "ori_task_spawn" => (vec![Ptr], Some(Ptr)),
        "ori_task_join" => (vec![Ptr], Some(Ptr)),
        "ori_task_detach" => (vec![Ptr], None),
        "ori_task_block_on" => (vec![Ptr], Some(I64)),
        "ori_task_sleep" => (vec![I64], Some(Ptr)),
        "ori_task_create_token" => (vec![], Some(Ptr)),
        "ori_task_cancel" => (vec![Ptr], None),
        "ori_task_is_cancelled" => (vec![Ptr], Some(I8)),
        "ori_task_associate" => (vec![Ptr, Ptr], None),
        "ori_channel_create" => (vec![], Some(Ptr)),
        "ori_channel_create_bounded" => (vec![I64], Some(Ptr)),
        "ori_channel_send" | "ori_channel_send_managed" => (vec![Ptr, I64], Some(Ptr)),
        "ori_channel_receive" => (vec![Ptr], Some(Ptr)),
        "ori_channel_close" => (vec![Ptr], None),
        "ori_atomic_new" => (vec![I64], Some(Ptr)),
        "ori_atomic_load" => (vec![Ptr], Some(I64)),
        "ori_atomic_store" => (vec![Ptr, I64], None),
        "ori_atomic_add" => (vec![Ptr, I64], Some(I64)),
        "ori_test_assert" => (vec![I8, Ptr], None),
        "ori_test_assert_eq" | "ori_test_assert_ne" => (vec![I64, I64], None),
        "ori_test_fail" => (vec![Ptr], None),
        "ori_test_live_allocations" => (vec![], Some(I64)),
        "ori_test_collect_cycles" => (vec![], Some(I64)),
        "ori_test_assert_no_leaks" => (vec![Ptr], Some(I64)),
        "ori_panic" => (vec![Ptr], None),
        "ori_files_write_text" | "ori_files_write_text_async" | "ori_files_write_bytes" => {
            (vec![Ptr, Ptr], Some(Ptr))
        }
        "ori_files_append_text" | "ori_files_copy" | "ori_files_rename" => {
            (vec![Ptr, Ptr], Some(Ptr))
        }
        "ori_files_exists"
        | "ori_files_delete"
        | "ori_files_create_dir"
        | "ori_files_is_file"
        | "ori_files_is_dir" => (vec![Ptr], Some(Ptr)),
        "ori_files_open_read" | "ori_files_open_write" => (vec![Ptr], Some(Ptr)),
        "ori_files_read" => (vec![Ptr, I64], Some(Ptr)),
        "ori_files_write" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_files_close" => (vec![Ptr], None),
        "ori_files_file_size" | "ori_files_modified_at" | "ori_files_created_at" => {
            (vec![Ptr], Some(Ptr))
        }
        "ori_os_current_dir" => (vec![], Some(Ptr)),
        "ori_os_change_dir" => (vec![Ptr], Some(Ptr)),
        "ori_mem_string_as_ptr" | "ori_mem_string_len" => (vec![Ptr], Some(I64)),
        "ori_random_seed" => (vec![I64], None),
        "ori_test_skip" => (vec![Ptr], None),
        "ori_bytes_from_list" => (vec![Ptr], Some(Ptr)),
        "ori_bytes_to_list" => (vec![Ptr], Some(Ptr)),
        "ori_process_run" | "ori_process_run_capture" | "ori_process_run_output" => {
            (vec![Ptr, Ptr], Some(Ptr))
        }
        "ori_net_connect"
        | "ori_net_connect_tls"
        | "ori_net_connect_async"
        | "ori_net_connect_tls_async" => (vec![Ptr, I64, I64], Some(Ptr)),
        "ori_net_listen" | "ori_net_udp_bind" => (vec![Ptr, I64], Some(Ptr)),
        "ori_net_accept" | "ori_net_accept_async" => (vec![Ptr], Some(Ptr)),
        "ori_net_close_listener" | "ori_net_udp_close" => (vec![Ptr], None),
        "ori_net_listener_port" | "ori_net_udp_local_port" => (vec![Ptr], Some(I64)),
        "ori_net_udp_send_to" | "ori_net_udp_send_to_async" => {
            (vec![Ptr, Ptr, I64, Ptr], Some(Ptr))
        }
        "ori_net_udp_recv_from" | "ori_net_udp_recv_from_async" => (vec![Ptr, I64], Some(Ptr)),
        "ori_net_read_some" | "ori_net_read_some_async" => (vec![Ptr, I64], Some(Ptr)),
        "ori_net_set_read_timeout_ms" | "ori_net_set_write_timeout_ms" => {
            (vec![Ptr, I64], Some(Ptr))
        }
        "ori_net_write_all" | "ori_net_write_all_async" => (vec![Ptr, Ptr], Some(Ptr)),
        "ori_net_close" => (vec![Ptr], None),
        "ori_net_is_closed" => (vec![Ptr], Some(I8)),
        "ori_math_trunc" | "ori_math_ln" | "ori_math_exp" | "ori_math_asin" | "ori_math_acos"
        | "ori_math_atan" | "ori_math_log10" => (vec![F64], Some(F64)),
        "ori_math_atan2" => (vec![F64, F64], Some(F64)),
        "ori_math_is_finite" => (vec![F64], Some(I8)),
        _ => return None,
    };
    Some(sig)
}

pub fn stdlib_entry_for_path(path: &str) -> Option<&'static StdlibRuntimeFunction> {
    STDLIB_RUNTIME_FUNCTIONS
        .iter()
        .find(|entry| entry.canonical_path == path || entry.aliases.contains(&path))
}

/// Stdlib paths that are importable as modules but have no runtime function
/// entry in `STDLIB_RUNTIME_FUNCTIONS`. There are three reasons a module lives
/// here instead of the manifest:
///
/// - Types or traits registered directly in the `DefMap`
///   (e.g. `ori.Error` struct, `ori.core.Hashable` trait, the `ori` root).
/// - Inline codegen intrinsics with no runtime FFI symbol
///   (e.g. `ori.mem.size_of` / `ori.mem.align_of` compile to constants).
/// - Umbrella modules that only re-export other modules' APIs
///   (e.g. `ori.concurrent` points to `ori.task`, `ori.channel`,
///   `ori.atomic`).
///
/// This list is the only hand-maintained addition to the manifest-derived
/// module set; keep it small and document why each entry cannot be derived
/// from `STDLIB_RUNTIME_FUNCTIONS`.
const STDLIB_MODULE_ONLY_PATHS: &[&str] = &[
    "ori",
    "ori.core",
    "ori.Error",
    "ori.mem",
    "ori.handle",
    "ori.concurrent",
    "ori.buffer",
    "ori.span",
    "ori.slotmap",
    "ori.image",
    "ori.string_view",
    "ori.cancel",
    "ori.err_trace",
];

/// Extracts the module prefix from a canonical stdlib path.
/// `"ori.io.print"` -> `Some("ori.io")`. Returns `None` for unqualified names
/// like `"string"` or `"len"` that have no module component.
fn module_of_canonical_path(canonical: &str) -> Option<&str> {
    canonical.rsplit_once('.').map(|(module, _)| module)
}

/// Returns true if `import` is an implemented `ori.*` module path.
///
/// This is the single source of truth for stdlib import classification,
/// derived from `STDLIB_RUNTIME_FUNCTIONS` plus `STDLIB_MODULE_ONLY_PATHS`.
/// Module prefixes come from both canonical paths and `ori.*` aliases, so
/// compatibility aliases like `ori.files` (alias of `ori.fs`) are accepted.
/// Callers (e.g. `classify_stdlib_import` in the driver) must not keep
/// parallel hardcoded module lists.
pub fn is_implemented_stdlib_module(import: &str) -> bool {
    if STDLIB_MODULE_ONLY_PATHS.contains(&import) {
        return true;
    }
    STDLIB_RUNTIME_FUNCTIONS.iter().any(|entry| {
        module_of_canonical_path(entry.canonical_path) == Some(import)
            || entry
                .aliases
                .iter()
                .filter(|alias| alias.starts_with("ori."))
                .any(|alias| module_of_canonical_path(alias) == Some(import))
    })
}

/// Returns the sorted list of implemented `ori.*` module paths, derived from
/// the manifest plus the module-only allowlist. Module prefixes come from
/// both canonical paths and `ori.*` aliases, so compatibility aliases like
/// `ori.files` are included. Useful for diagnostics, documentation, and
/// parity tests.
pub fn implemented_stdlib_modules() -> Vec<&'static str> {
    let mut modules: std::collections::BTreeSet<&'static str> =
        STDLIB_MODULE_ONLY_PATHS.iter().copied().collect();
    for entry in STDLIB_RUNTIME_FUNCTIONS {
        if let Some(module) = module_of_canonical_path(entry.canonical_path) {
            modules.insert(module);
        }
        for alias in entry.aliases {
            if alias.starts_with("ori.") {
                if let Some(module) = module_of_canonical_path(alias) {
                    modules.insert(module);
                }
            }
        }
    }
    modules.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn manifest_paths_and_aliases_are_unique() {
        let mut seen = HashSet::new();
        for entry in STDLIB_RUNTIME_FUNCTIONS {
            assert!(
                seen.insert(entry.canonical_path),
                "duplicate stdlib path {}",
                entry.canonical_path
            );
            for alias in entry.aliases {
                assert!(seen.insert(*alias), "duplicate stdlib alias {alias}");
            }
        }
    }

    #[test]
    fn manifest_resolves_aliases_to_runtime_symbols() {
        assert_eq!(
            stdlib_runtime_symbol("ori.string.trim"),
            Some("ori_string_trim")
        );
        assert_eq!(
            stdlib_runtime_symbol("string.pad_left"),
            Some("ori_string_pad_left")
        );
        assert_eq!(
            stdlib_runtime_symbol("ori.files.read_text"),
            Some("ori_files_read_text")
        );
        assert_eq!(stdlib_runtime_symbol("iter.map"), Some("ori_list_map"));
        assert_eq!(canonical_stdlib_path("files.rename"), Some("ori.fs.rename"));
    }

    #[test]
    fn manifest_runtime_entries_have_type_and_native_abi_metadata() {
        let mut missing_type_sig = Vec::new();
        let mut missing_native_abi = Vec::new();
        let mut checked_native_symbols = HashSet::new();

        for entry in STDLIB_RUNTIME_FUNCTIONS {
            if stdlib_func_sig(entry.canonical_path).is_none() {
                missing_type_sig.push(entry.canonical_path);
            }
            if entry.native_runtime
                && checked_native_symbols.insert(entry.runtime_symbol)
                && stdlib_native_abi(entry.runtime_symbol).is_none()
            {
                missing_native_abi.push(entry.runtime_symbol);
            }
        }

        assert!(
            missing_type_sig.is_empty(),
            "stdlib manifest entries missing semantic type signatures: {missing_type_sig:#?}"
        );
        assert!(
            missing_native_abi.is_empty(),
            "stdlib manifest entries missing native ABI metadata: {missing_native_abi:#?}"
        );
    }

    /// Spec cap. 12 sanity check: the documented `ori.fs.File` and `ori.json`
    /// contracts must match the actual `stdlib_func_sig` return values. This
    /// guards against the spec drifting from the implementation.
    #[test]
    fn spec_fs_and_json_contracts_match_stdlib_sig() {
        // fs.open_read / fs.open_write: (string) -> result[File, string]
        for path in ["ori.fs.open_read", "ori.fs.open_write"] {
            let (params, ret) = stdlib_func_sig(path).expect("fs file open sig");
            assert_eq!(params, vec![Ty::String], "{path} params");
            assert_eq!(
                ret,
                Ty::Result(Box::new(file_ty()), Box::new(Ty::String)),
                "{path} return"
            );
        }

        // fs.read: (File, int) -> result[bytes, string]
        let (params, ret) = stdlib_func_sig("ori.fs.read").expect("fs.read sig");
        assert_eq!(params, vec![file_ty(), Ty::Int], "fs.read params");
        assert_eq!(
            ret,
            Ty::Result(Box::new(Ty::Bytes), Box::new(Ty::String)),
            "fs.read return"
        );

        // fs.write: (File, bytes) -> result[int, string]
        let (params, ret) = stdlib_func_sig("ori.fs.write").expect("fs.write sig");
        assert_eq!(params, vec![file_ty(), Ty::Bytes], "fs.write params");
        assert_eq!(
            ret,
            Ty::Result(Box::new(Ty::Int), Box::new(Ty::String)),
            "fs.write return"
        );

        // fs.close: (File) -> void
        let (params, ret) = stdlib_func_sig("ori.fs.close").expect("fs.close sig");
        assert_eq!(params, vec![file_ty()], "fs.close params");
        assert_eq!(ret, Ty::Void, "fs.close return");

        // fs.exists / fs.delete: result-based Layer 1 (post-migration)
        let (params, ret) = stdlib_func_sig("ori.fs.exists").expect("fs.exists sig");
        assert_eq!(params, vec![Ty::String], "fs.exists params");
        assert_eq!(
            ret,
            Ty::Result(Box::new(Ty::Bool), Box::new(Ty::String)),
            "fs.exists return"
        );
        let (params, ret) = stdlib_func_sig("ori.fs.delete").expect("fs.delete sig");
        assert_eq!(params, vec![Ty::String], "fs.delete params");
        assert_eq!(
            ret,
            Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)),
            "fs.delete return"
        );

        // io.read_line: () -> optional[string]
        let (params, ret) = stdlib_func_sig("ori.io.read_line").expect("io.read_line sig");
        assert_eq!(params, Vec::<Ty>::new(), "io.read_line params");
        assert_eq!(
            ret,
            Ty::Optional(Box::new(Ty::String)),
            "io.read_line return"
        );

        // json.parse: (string) -> result[Value, string]
        let json_value = Ty::Named(DefId::SYNTHETIC_JSON_VALUE, Vec::new());
        let (params, ret) = stdlib_func_sig("ori.json.parse").expect("json.parse sig");
        assert_eq!(params, vec![Ty::String], "json.parse params");
        assert_eq!(
            ret,
            Ty::Result(Box::new(json_value.clone()), Box::new(Ty::String)),
            "json.parse return"
        );

        // json.stringify / json.stringify_pretty: (Value) -> string
        for path in ["ori.json.stringify", "ori.json.stringify_pretty"] {
            let (params, ret) = stdlib_func_sig(path).expect("json stringify sig");
            assert_eq!(params, vec![json_value.clone()], "{path} params");
            assert_eq!(ret, Ty::String, "{path} return");
        }
    }

    /// Spec cap. 14 sanity check: the C/debug backend stdlib matrix documents
    /// which modules carry the `c_backend` flag. This test asserts the flag
    /// value for representative entries from each matrix row, guarding against
    /// the spec drifting from the manifest.
    #[test]
    fn spec_c_backend_matrix_matches_manifest_flags() {
        fn flagged(path: &str) -> bool {
            stdlib_entry_for_path(path)
                .expect("manifest entry exists")
                .c_backend_runtime
        }

        // Rows documented as "yes" (every function carries the flag).
        assert!(flagged("ori.io.print"), "io.print should be c_backend");
        assert!(flagged("ori.io.println"), "io.println should be c_backend");
        assert!(
            flagged("ori.mem.string_as_ptr"),
            "mem.string_as_ptr should be c_backend"
        );
        assert!(
            flagged("ori.mem.string_len"),
            "mem.string_len should be c_backend"
        );
        assert!(flagged("ori.math.sqrt"), "math should be c_backend");
        assert!(flagged("ori.time.now"), "time should be c_backend");
        assert!(flagged("ori.format.number"), "format should be c_backend");
        assert!(flagged("ori.os.args"), "os should be c_backend");
        assert!(flagged("ori.random.int"), "random should be c_backend");
        assert!(
            flagged("ori.test.assert"),
            "test.assert should be c_backend"
        );
        assert!(flagged("ori.iter.map"), "iter should be c_backend");
        assert!(flagged("string"), "string builtin should be c_backend");
        assert!(flagged("int"), "int builtin should be c_backend");
        assert!(flagged("float"), "float builtin should be c_backend");

        // LANG-2: io eprint/read_line + string + convert + len now ship C bodies.
        assert!(flagged("ori.io.eprint"), "io.eprint should be c_backend");
        assert!(
            flagged("ori.io.read_line"),
            "io.read_line should be c_backend"
        );
        assert!(flagged("ori.string.len"), "string.len should be c_backend");
        assert!(
            flagged("ori.string.contains"),
            "string.contains should be c_backend"
        );
        assert!(
            flagged("ori.convert.float_to_string"),
            "convert.float_to_string should be c_backend"
        );
        assert!(
            flagged("ori.convert.bool_to_string"),
            "convert.bool_to_string should be c_backend"
        );
        assert!(
            flagged("ori.convert.string_to_int"),
            "convert.string_to_int should be c_backend"
        );
        assert!(
            !flagged("ori.convert.string_to_float"),
            "string_to_float stays native (optional[float] typedef order)"
        );
        assert!(flagged("len"), "len builtin should be c_backend");

        // Rows documented as "no" (native-only or extern stubs).
        assert!(!flagged("ori.bytes.len"), "bytes.* should NOT be c_backend");
        assert!(!flagged("ori.list.new"), "list.* should NOT be c_backend");
        assert!(!flagged("ori.map.new"), "map.* should NOT be c_backend");
        assert!(!flagged("ori.set.new"), "set.* should NOT be c_backend");
        assert!(!flagged("ori.tree.new"), "tree.* should NOT be c_backend");
        assert!(!flagged("ori.json.parse"), "json should NOT be c_backend");
        assert!(!flagged("ori.fs.read_text"), "fs.* should NOT be c_backend");
        assert!(!flagged("ori.task.spawn"), "task.* should NOT be c_backend");
        assert!(
            !flagged("ori.channel.create"),
            "channel.* should NOT be c_backend"
        );
        assert!(
            !flagged("ori.atomic.new"),
            "atomic.* should NOT be c_backend"
        );
        assert!(
            !flagged("ori.test.live_allocations"),
            "test leak checks NOT c_backend"
        );
        assert!(!flagged("ori.panic"), "panic should NOT be c_backend");
    }

    /// Parity test: every module prefix derived from the manifest must be
    /// classified as implemented. This guards against future manifest entries
    /// whose module prefix is not recognized by `is_implemented_stdlib_module`.
    #[test]
    fn manifest_module_prefixes_are_all_implemented() {
        let modules = implemented_stdlib_modules();
        for module in &modules {
            assert!(
                is_implemented_stdlib_module(module),
                "implemented_stdlib_modules returned {module} but is_implemented_stdlib_module rejects it"
            );
        }
    }

    /// Regression test: the set of modules accepted by the manifest-derived
    /// classifier must cover every module that the old hardcoded
    /// `classify_stdlib_import` list accepted. If a module is added or removed
    /// from the manifest, this test catches the change explicitly.
    #[test]
    fn implemented_stdlib_modules_covers_legacy_hardcoded_list() {
        let modules = implemented_stdlib_modules();
        let legacy: &[&str] = &[
            "ori",
            "ori.core",
            "ori.io",
            "ori.fs",
            "ori.files",
            "ori.string",
            "ori.bytes",
            "ori.list",
            "ori.map",
            "ori.set",
            "ori.deque",
            "ori.queue",
            "ori.stack",
            "ori.linked_list",
            "ori.doubly_linked_list",
            "ori.tree",
            "ori.hash_table",
            "ori.graph",
            "ori.heap",
            "ori.math",
            "ori.convert",
            "ori.mem",
            "ori.time",
            "ori.format",
            "ori.os",
            "ori.random",
            "ori.iter",
            "ori.lazy",
            "ori.concurrent",
            "ori.task",
            "ori.channel",
            "ori.atomic",
            "ori.test",
            "ori.json",
            "ori.Error",
        ];
        for &expected in legacy {
            assert!(
                modules.contains(&expected),
                "legacy stdlib module {expected} missing from implemented_stdlib_modules: {modules:?}"
            );
            assert!(
                is_implemented_stdlib_module(expected),
                "legacy stdlib module {expected} rejected by is_implemented_stdlib_module"
            );
        }
    }

    /// Negative test: unknown `ori.*` modules must be rejected so the driver
    /// can emit `bind.stdlib_module_unknown`.
    #[test]
    fn unknown_stdlib_modules_are_rejected() {
        assert!(!is_implemented_stdlib_module("ori.nope"));
        assert!(!is_implemented_stdlib_module("ori.io.sub"));
        assert!(!is_implemented_stdlib_module("ori.future"));
        assert!(!is_implemented_stdlib_module("notori.io"));
        assert!(!is_implemented_stdlib_module(""));
    }
}
