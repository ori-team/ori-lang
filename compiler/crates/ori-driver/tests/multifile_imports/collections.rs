use super::*;

#[test]
fn compile_runs_more_collection_stdlib() {
    let dir = TestDir::new("compile_more_collection_stdlib");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.set = sets

main()
    var values: list[int] = [3, 1, 2]
    lists.insert(values, 1, 7)
    lists.remove(values, 2)
    io.print(string(lists.index_of(values, 7)))
    if lists.contains(values, 2)
        io.print("contains")
    end
    lists.sort(values)
    lists.reverse(values)
    const chunk: list[int] = lists.slice(values, 1, 3)
    io.print(string(lists.pop(chunk)))
    io.print(string(chunk[0] + lists.len(values)))

    const seen: set[int] = sets.new()
    sets.add(seen, 1)
    sets.add(seen, 2)
    sets.remove(seen, 1)
    io.print(string(sets.len(seen)))

    const scores: map[int, int] = maps.new()
    maps.set(scores, 1, 10)
    maps.set(scores, 2, 20)
    maps.set(scores, 3, 30)
    maps.remove(scores, 2)
    const keys: list[int] = maps.keys(scores)
    const vals: list[int] = maps.values(scores)
    io.print(string(lists.len(keys) + lists.len(vals)))
    io.print(string(keys[0] + keys[1] + vals[0] + vals[1]))
end
"#,
    );

    let stdout = compile_and_run(&dir, "more_collection_stdlib");
    assert_eq!(stdout, "1\ncontains\n2\n6\n1\n4\n44\n");
}

#[test]
fn compile_runs_list_capacity_reserve_with_capacity_native() {
    let dir = TestDir::new("compile_list_capacity_reserve_with_capacity_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists

main()
    var empty: list[int] = lists.with_capacity(64)
    if lists.capacity(empty) >= 64
        io.print("with-cap")
    end
    lists.reserve(empty, 128)
    if lists.capacity(empty) >= 128
        io.print("reserved")
    end
    var i: int = 0
    while i < 100
        lists.push(empty, i)
        i = i + 1
    end
    io.print(string(lists.len(empty)))
    io.print(string(empty[0] + empty[99]))
end
"#,
    );

    let stdout = compile_and_run(&dir, "list_capacity_reserve_with_capacity");
    assert_eq!(stdout, "with-cap\nreserved\n100\n99\n");
}

#[test]
fn compile_runs_map_set_capacity_reserve_clear_native() {
    let dir = TestDir::new("compile_map_set_capacity_reserve_clear_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps
import ori.set = sets

main()
    const seen: set[int] = sets.new()
    sets.reserve(seen, 32)
    if sets.capacity(seen) >= 32
        io.print("set-reserved")
    end
    sets.add(seen, 1)
    sets.add(seen, 2)
    sets.clear(seen)
    io.print(string(sets.len(seen)))
    sets.add(seen, 3)
    if sets.contains(seen, 3)
        io.print("set-reused")
    end

    const labels: set[string] = sets.new()
    sets.add(labels, "old")
    sets.clear(labels)
    sets.add(labels, "new")
    if sets.contains(labels, "new")
        io.print("string-set-reused")
    end

    const scores: map[int, int] = maps.new()
    maps.reserve(scores, 32)
    if maps.capacity(scores) >= 32
        io.print("map-reserved")
    end
    maps.set(scores, 1, 10)
    maps.set(scores, 2, 20)
    maps.clear(scores)
    io.print(string(maps.len(scores)))
    maps.set(scores, 3, 30)
    io.print(string(maps.get(scores, 3)))

    const counts: map[string, int] = maps.new()
    maps.set(counts, "old", 1)
    maps.clear(counts)
    maps.set(counts, "new", 2)
    io.print(string(maps.get(counts, "new")))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "map_set_capacity_reserve_clear.exe"
    } else {
        "map_set_capacity_reserve_clear"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "set-reserved\n0\nset-reused\nstring-set-reused\nmap-reserved\n0\n30\n2\n"
    );
}

#[test]
fn compile_runs_deque_queue_stack_stdlib_native() {
    let dir = TestDir::new("compile_deque_queue_stack_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.deque = deque
import ori.io = io
import ori.queue = queue
import ori.stack = stack

main()
    const d: deque.Deque[int] = deque.new()
    deque.push_back(d, 2)
    deque.push_front(d, 1)
    deque.push_back(d, 3)
    match deque.front(d)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    match deque.pop_back(d)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    const d_items: list[int] = deque.to_list(d)
    io.print(string(d_items[0] + d_items[1]))
    deque.clear(d)
    if deque.is_empty(d)
        io.print("deque-empty")
    end
    match deque.pop_front(d)
        case some(value):
            io.print(string(value))
        case none:
            io.print("deque-none")
    end

    const q: queue.Queue[string] = queue.new()
    queue.enqueue(q, "first")
    queue.enqueue(q, "second")
    match queue.dequeue(q)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    match queue.peek(q)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    match queue.dequeue(q)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    match queue.dequeue(q)
        case some(value):
            io.print(value)
        case none:
            io.print("queue-none")
    end

    const s: stack.Stack[int] = stack.new()
    stack.push(s, 10)
    stack.push(s, 20)
    match stack.peek(s)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    match stack.pop(s)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    io.print(string(stack.len(s)))

    const words: stack.Stack[string] = stack.new()
    stack.push(words, "managed")
    match stack.pop(words)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    match stack.pop(words)
        case some(value):
            io.print(value)
        case none:
            io.print("stack-none")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "deque_queue_stack_stdlib.exe"
    } else {
        "deque_queue_stack_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "1\n3\n3\ndeque-empty\ndeque-none\nfirst\nsecond\nsecond\nqueue-none\n20\n20\n1\nmanaged\nstack-none\n"
    );
}

#[test]
fn check_preserves_opaque_collection_type_display() {
    let dir = TestDir::new("opaque_collection_type_display");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.list = lists
import ori.queue = queue

main()
    const values: queue.Queue[int] = queue.new()
    lists.push(values, 1)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected opaque/list mismatch");
    let rendered = out
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("queue.Queue[int]"),
        "expected opaque display in diagnostics, got: {rendered}"
    );
    assert!(
        rendered.contains("list[_#") || rendered.contains("list[int]"),
        "expected list expectation in diagnostics, got: {rendered}"
    );
}

#[test]
fn compile_runs_linked_list_stdlib_native() {
    let dir = TestDir::new("compile_linked_list_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.doubly_linked_list = dll
import ori.io = io
import ori.linked_list = ll

main()
    const names: ll.LinkedList[string] = ll.new()
    ll.push_back(names, "beta")
    ll.push_front(names, "alpha")
    match ll.front(names)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    match ll.pop_front(names)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    const names_snapshot: list[string] = ll.to_list(names)
    io.print(names_snapshot[0])
    ll.push_back(names, "gamma")
    match ll.find(names, "gamma")
        case some(cursor):
            io.print(string(cursor))
        case none:
            io.print("missing")
    end
    match ll.cursor_front(names)
        case some(cursor):
            io.print(string(cursor))
        case none:
            io.print("missing")
    end
    io.print(string(ll.insert_after(names, 0, "inserted")))
    match ll.value_at(names, 1)
        case some(value):
            io.print(value)
        case none:
            io.print("missing")
    end
    match ll.remove_at(names, 1)
        case some(value):
            io.print(value)
        case none:
            io.print("missing")
    end
    ll.clear(names)
    if ll.is_empty(names)
        io.print("linked-empty")
    end
    match ll.pop_front(names)
        case some(value):
            io.print(value)
        case none:
            io.print("linked-none")
    end

    const ints: dll.DoublyLinkedList[int] = dll.new()
    dll.push_front(ints, 2)
    dll.push_front(ints, 1)
    dll.push_back(ints, 3)
    match dll.front(ints)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    match dll.back(ints)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    match dll.pop_front(ints)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    match dll.pop_back(ints)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    io.print(string(dll.len(ints)))
    const ints_snapshot: list[int] = dll.to_list(ints)
    io.print(string(ints_snapshot[0]))
    match dll.cursor_back(ints)
        case some(cursor):
            io.print(string(cursor))
        case none:
            io.print("missing")
    end
    io.print(string(dll.insert_before(ints, 0, 9)))
    io.print(string(dll.insert_after(ints, 0, 8)))
    match dll.value_at(ints, 1)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    match dll.find(ints, 2)
        case some(cursor):
            io.print(string(cursor))
        case none:
            io.print("missing")
    end
    match dll.remove_at(ints, 1)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    var direct_sum: int = 0
    for value in ints
        direct_sum = direct_sum + value
    end
    io.print(string(direct_sum))
    dll.clear(ints)
    if dll.is_empty(ints)
        io.print("doubly-empty")
    end

    const words: dll.DoublyLinkedList[string] = dll.new()
    dll.push_back(words, "managed")
    match dll.pop_back(words)
        case some(value):
            io.print(value)
        case none:
            io.print("empty")
    end
    match dll.pop_back(words)
        case some(value):
            io.print(value)
        case none:
            io.print("doubly-none")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "linked_list_stdlib.exe"
    } else {
        "linked_list_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "alpha\nalpha\nbeta\n1\n0\ntrue\ninserted\ninserted\nlinked-empty\nlinked-none\n1\n3\n1\n3\n1\n2\n0\ntrue\ntrue\n8\n2\n8\n11\ndoubly-empty\nmanaged\ndoubly-none\n"
    );
}

#[test]
fn compile_runs_doubly_linked_list_many_nodes_native() {
    let dir = TestDir::new("compile_doubly_linked_list_many_nodes_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.doubly_linked_list = dll
import ori.io = io

main()
    const values: dll.DoublyLinkedList[int] = dll.new()

    var i: int = 0
    while i < 200
        dll.push_back(values, i)
        i = i + 1
    end

    var front_sum: int = 0
    var front_count: int = 0
    while front_count < 50
        match dll.pop_front(values)
            case some(value):
                front_sum = front_sum + value
            case none:
                io.print("front-empty")
        end
        front_count = front_count + 1
    end

    var back_sum: int = 0
    var back_count: int = 0
    while back_count < 50
        match dll.pop_back(values)
            case some(value):
                back_sum = back_sum + value
            case none:
                io.print("back-empty")
        end
        back_count = back_count + 1
    end

    io.print(string(front_sum))
    io.print(string(back_sum))
    io.print(string(dll.len(values)))

    match dll.front(values)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing-front")
    end

    match dll.back(values)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing-back")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "doubly_linked_list_many_nodes.exe"
    } else {
        "doubly_linked_list_many_nodes"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "1225\n8725\n100\n50\n149\n");
}

#[test]
fn compile_runs_tree_stdlib_native() {
    let dir = TestDir::new("compile_tree_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists
import ori.tree = tree

main()
    const t: tree.Tree[string] = tree.new("root")
    const root: tree.NodeId = tree.root(t)
    const left: tree.NodeId = tree.add_child(t, root, "left")
    const right: tree.NodeId = tree.add_child(t, root, "right")
    const leaf: tree.NodeId = tree.add_child(t, left, "leaf")

    io.print(tree.value(t, root))
    const kids: list[tree.NodeId] = tree.children(t, root)
    io.print(string(lists.len(kids)))
    match tree.parent(t, root)
        case some(parent):
            io.print(tree.value(t, parent))
        case none:
            io.print("root-parent-none")
    end
    match tree.parent(t, leaf)
        case some(parent):
            io.print(tree.value(t, parent))
        case none:
            io.print("leaf-parent-none")
    end
    io.print(string(tree.depth(t, leaf)))

    const pre: list[tree.NodeId] = tree.pre_order(t)
    io.print(tree.value(t, pre[0]))
    io.print(tree.value(t, pre[1]))
    io.print(tree.value(t, pre[2]))
    io.print(tree.value(t, pre[3]))

    const post: list[tree.NodeId] = tree.post_order(t)
    io.print(tree.value(t, post[0]))
    io.print(tree.value(t, post[1]))
    io.print(tree.value(t, post[2]))
    io.print(tree.value(t, post[3]))

    const breadth: list[tree.NodeId] = tree.breadth_first(t)
    io.print(tree.value(t, breadth[0]))
    io.print(tree.value(t, breadth[1]))
    io.print(tree.value(t, breadth[2]))
    io.print(tree.value(t, breadth[3]))

    tree.remove_subtree(t, left)
    io.print(string(tree.len(t)))
    const remaining: list[tree.NodeId] = tree.children(t, root)
    io.print(string(lists.len(remaining)))
    io.print(tree.value(t, remaining[0]))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "tree_stdlib.exe"
    } else {
        "tree_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "root\n2\nroot-parent-none\nleft\n2\nroot\nleft\nleaf\nright\nleaf\nleft\nright\nroot\nroot\nleft\nright\nleaf\n2\n1\nright\n"
    );
}

#[test]
fn compile_runs_tree_invalid_node_id_runtime_error() {
    let dir = TestDir::new("tree_invalid_node_id_runtime_error");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.tree = tree

main()
    const t: tree.Tree[int] = tree.new(1)
    io.print(string(tree.value(t, 999)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "tree_invalid_node.exe"
    } else {
        "tree_invalid_node"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ori tree node id is invalid"),
        "stderr was: {stderr}"
    );
}

#[test]
fn compile_runs_hash_table_stdlib_native() {
    let dir = TestDir::new("compile_hash_table_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.hash_table = hash_table
import ori.io = io
import ori.list = lists

main()
    const table: hash_table.HashTable[int, string] = hash_table.with_capacity(2)
    hash_table.set(table, 1, "one")
    hash_table.set(table, 17, "seventeen")
    hash_table.set(table, 33, "thirty-three")
    hash_table.reserve(table, 16)

    match hash_table.get(table, 17)
        case some(value):
            io.print(value)
        case none:
            io.print("missing")
    end
    io.print(string(hash_table.contains(table, 33)))
    io.print(string(hash_table.len(table)))
    io.print(string(hash_table.capacity(table)))

    const keys: list[int] = hash_table.keys(table)
    const values: list[string] = hash_table.values(table)
    const entries: list[tuple[int, string]] = hash_table.entries(table)
    io.print(string(lists.len(keys)))
    io.print(values[0])
    io.print(string(lists.len(entries)))

    match hash_table.remove(table, 1)
        case some(value):
            io.print(value)
        case none:
            io.print("remove-missing")
    end
    match hash_table.get(table, 1)
        case some(value):
            io.print(value)
        case none:
            io.print("gone")
    end

    const labels: hash_table.HashTable[string, int] = hash_table.new()
    hash_table.set(labels, "alpha", 10)
    hash_table.set(labels, "beta", 20)
    match hash_table.get(labels, "beta")
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    match hash_table.remove(labels, "alpha")
        case some(value):
            io.print(string(value))
        case none:
            io.print("remove-missing")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "hash_table_stdlib.exe"
    } else {
        "hash_table_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.replace("\r\n", "\n");
    let mut lines = lines.lines();
    assert_eq!(lines.next(), Some("seventeen"));
    assert_eq!(lines.next(), Some("true"));
    assert_eq!(lines.next(), Some("3"));
    let capacity: i64 = lines.next().unwrap().parse().unwrap();
    assert!(capacity >= 16, "capacity was {capacity}");
    assert_eq!(lines.next(), Some("3"));
    assert_eq!(lines.next(), Some("one"));
    assert_eq!(lines.next(), Some("3"));
    assert_eq!(lines.next(), Some("one"));
    assert_eq!(lines.next(), Some("gone"));
    assert_eq!(lines.next(), Some("20"));
    assert_eq!(lines.next(), Some("10"));
    assert_eq!(lines.next(), None);
}

#[test]
fn check_accepts_hash_table_user_defined_hashable_equatable_key() {
    let dir = TestDir::new("hash_table_user_defined_key");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.hash_table = hash_table

struct Resource
    id: int
end

apply Resource use core.Hashable
end

apply Resource use core.Equatable
    equals(self, other: Resource) -> bool
        return self.id == other.id
    end
end

main()
    const cache: hash_table.HashTable[Resource, int] = hash_table.new()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "got: {:?}", out.diagnostics);
}

#[test]
fn compile_runs_graph_stdlib_native() {
    let dir = TestDir::new("compile_graph_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.graph = graph
import ori.io = io
import ori.list = lists

main()
    const dag: graph.Graph[int] = graph.new(true)
    graph.add_edge(dag, 1, 2)
    graph.add_edge(dag, 1, 3)
    graph.add_edge(dag, 2, 4)

    io.print(string(graph.has_node(dag, 3)))
    io.print(string(graph.has_edge(dag, 1, 2)))
    io.print(string(graph.has_edge(dag, 2, 1)))

    const neighbors: list[int] = graph.neighbors(dag, 1)
    io.print(string(lists.len(neighbors)))
    io.print(string(neighbors[0]))
    io.print(string(neighbors[1]))

    const bfs_order: list[int] = graph.bfs(dag, 1)
    io.print(string(bfs_order[0]))
    io.print(string(bfs_order[1]))
    io.print(string(bfs_order[2]))
    io.print(string(bfs_order[3]))

    const dfs_order: list[int] = graph.dfs(dag, 1)
    io.print(string(dfs_order[0]))
    io.print(string(dfs_order[1]))
    io.print(string(dfs_order[2]))
    io.print(string(dfs_order[3]))

    const topo: list[int] = graph.topological_sort(dag)
    io.print(string(topo[0]))
    io.print(string(topo[1]))
    io.print(string(topo[2]))
    io.print(string(topo[3]))

    const edges: list[tuple[int, int]] = graph.edges(dag)
    io.print(string(lists.len(edges)))
    graph.remove_edge(dag, 1, 3)
    io.print(string(graph.has_edge(dag, 1, 3)))
    graph.remove_node(dag, 2)
    io.print(string(graph.has_node(dag, 2)))

    const network: graph.Graph[string] = graph.new(false)
    graph.add_edge(network, "a", "b")
    graph.add_edge(network, "b", "c")
    io.print(string(graph.has_edge(network, "b", "a")))
    const network_bfs: list[string] = graph.bfs(network, "a")
    io.print(network_bfs[0])
    io.print(network_bfs[1])
    io.print(network_bfs[2])
    graph.remove_node(network, "b")
    io.print(string(graph.has_edge(network, "a", "b")))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "graph_stdlib.exe"
    } else {
        "graph_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "true\ntrue\nfalse\n2\n2\n3\n1\n2\n3\n4\n1\n2\n4\n3\n1\n2\n3\n4\n3\nfalse\nfalse\ntrue\na\nb\nc\nfalse\n"
    );
}

#[test]
fn compile_runs_graph_cycle_stress_native() {
    let dir = TestDir::new("compile_graph_cycle_stress_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.graph = graph
import ori.io = io
import ori.list = lists

main()
    const cyclic: graph.Graph[int] = graph.new(true)

    var node: int = 1
    while node < 60
        graph.add_edge(cyclic, node, node + 1)
        node = node + 1
    end
    graph.add_edge(cyclic, 60, 1)

    const cyclic_topo: list[int] = graph.topological_sort(cyclic)
    io.print(string(lists.len(cyclic_topo)))

    const bfs_order: list[int] = graph.bfs(cyclic, 1)
    const dfs_order: list[int] = graph.dfs(cyclic, 1)
    io.print(string(lists.len(bfs_order)))
    io.print(string(lists.len(dfs_order)))
    io.print(string(bfs_order[0]))
    io.print(string(dfs_order[0]))

    graph.remove_edge(cyclic, 60, 1)
    const acyclic_topo: list[int] = graph.topological_sort(cyclic)
    io.print(string(lists.len(acyclic_topo)))
    io.print(string(acyclic_topo[0]))
    io.print(string(acyclic_topo[59]))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "graph_cycle_stress.exe"
    } else {
        "graph_cycle_stress"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "0\n60\n60\n1\n1\n60\n1\n60\n");
}

#[test]
fn check_accepts_graph_user_defined_hashable_equatable_node() {
    let dir = TestDir::new("graph_user_defined_node");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.graph = graph

struct Resource
    id: int
end

apply Resource use core.Hashable
end

apply Resource use core.Equatable
    equals(self, other: Resource) -> bool
        return self.id == other.id
    end
end

main()
    const links: graph.Graph[Resource] = graph.new(false)
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(!out.has_errors, "got: {:?}", out.diagnostics);
}

#[test]
fn compile_runs_heap_stdlib_native() {
    let dir = TestDir::new("compile_heap_stdlib_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.heap = heap
import ori.io = io

struct Score
    value: int
end

apply Score use core.Comparable
    compare(self, other: Score) -> int
        return self.value - other.value
    end
end

main()
    const numbers: heap.Heap[int] = heap.new()
    heap.push(numbers, 4)
    heap.push(numbers, 1)
    heap.push(numbers, 3)

    match heap.peek(numbers)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    io.print(string(heap.len(numbers)))
    io.print(string(heap.is_empty(numbers)))

    match heap.pop(numbers)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    match heap.pop(numbers)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    match heap.pop(numbers)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    match heap.pop(numbers)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end

    const words: heap.Heap[string] = heap.new()
    heap.push(words, "pear")
    heap.push(words, "apple")
    heap.push(words, "orange")
    match heap.pop(words)
        case some(value):
            io.print(value)
        case none:
            io.print("missing")
    end

    const scores: heap.Heap[Score] = heap.new()
    heap.push(scores, Score {value: 5})
    heap.push(scores, Score {value: 2})
    heap.push(scores, Score {value: 7})
    match heap.pop(scores)
        case some(score):
            io.print(string(score.value))
        case none:
            io.print("missing")
    end
    match heap.pop(scores)
        case some(score):
            io.print(string(score.value))
        case none:
            io.print("missing")
    end
    match heap.pop(scores)
        case some(score):
            io.print(string(score.value))
        case none:
            io.print("missing")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "heap_stdlib.exe"
    } else {
        "heap_stdlib"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "1\n3\nfalse\n1\n3\n4\nempty\napple\n2\n5\n7\n"
    );
}

#[test]
fn compile_runs_heap_managed_pop_and_peek_after_heap_scope_native() {
    let dir = TestDir::new("heap_managed_pop_peek_after_scope");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.heap = heap
import ori.io = io

struct Score
    value: int
end

apply Score use core.Comparable
    compare(self, other: Score) -> int
        return self.value - other.value
    end
end

main()
    const pop_scores: heap.Heap[Score] = heap.new()
    heap.push(pop_scores, Score {value: 5})
    heap.push(pop_scores, Score {value: 2})
    match heap.pop(pop_scores)
        case some(score):
            heap.clear(pop_scores)
            io.print(string(score.value))
        case none:
            io.print("missing")
    end

    const peek_scores: heap.Heap[Score] = heap.new()
    heap.push(peek_scores, Score {value: 8})
    heap.push(peek_scores, Score {value: 3})
    match heap.peek(peek_scores)
        case some(score):
            heap.clear(peek_scores)
            io.print(string(score.value))
        case none:
            io.print("missing")
    end
end
"#,
    );

    let stdout = compile_and_run(&dir, "heap_managed_pop_peek_after_scope");
    assert_eq!(stdout, "2\n3\n");
}

#[test]
fn compile_runs_completed_collection_gap_apis_native() {
    let dir = TestDir::new("compile_completed_collection_gap_apis_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.graph = graph
import ori.heap = heap
import ori.io = io
import ori.list = lists
import ori.map = maps
import ori.set = sets
import ori.tree = tree

main()
    const values: list[int] = lists.from_list([10, 20, 30])
    const copied: list[int] = lists.clone(values)
    match lists.try_get(copied, 1)
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    io.print(string(lists.try_remove(copied, 0)))
    match lists.try_pop(copied)
        case some(value):
            io.print(string(value))
        case none:
            io.print("empty")
    end
    lists.clear(copied)
    io.print(string(lists.is_empty(copied)))

    const labels: set[string] = sets.from_list(["a", "b", "a"])
    const labels_copy: set[string] = sets.clone(labels)
    io.print(string(sets.len(labels_copy)))
    io.print(string(sets.try_remove(labels_copy, "a")))
    const label_items: list[string] = sets.to_list(labels_copy)
    io.print(label_items[0])

    const entries: list[tuple[string, int]] = [tuple("a", 1), tuple("b", 2)]
    const table: map[string, int] = maps.from_entries(entries)
    match maps.try_get(table, "b")
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    match maps.try_remove(table, "a")
        case some(value):
            io.print(string(value))
        case none:
            io.print("missing")
    end
    io.print(string(maps.is_empty(table)))
    const table_copy: map[string, int] = maps.clone(table)
    io.print(string(maps.len(table_copy)))

    const outline: tree.Tree[string] = tree.new("root")
    const root: tree.NodeId = tree.root(outline)
    const left: tree.NodeId = tree.add_child(outline, root, "left")
    const right: tree.NodeId = tree.add_child(outline, root, "right")
    const leaf: tree.NodeId = tree.add_child(outline, left, "leaf")
    io.print(string(tree.contains_node(outline, leaf)))
    io.print(string(tree.set_value(outline, leaf, "leaf2")))
    match tree.try_value(outline, leaf)
        case some(value):
            io.print(value)
        case none:
            io.print("missing")
    end
    io.print(string(tree.move_subtree(outline, leaf, right)))
    match tree.find(outline, "leaf2")
        case some(node):
            io.print(tree.value(outline, node))
        case none:
            io.print("missing")
    end
    const branch: tree.Tree[string] = tree.clone_subtree(outline, right)
    io.print(tree.value(branch, tree.root(branch)))
    const outline_copy: tree.Tree[string] = tree.clone(outline)
    io.print(string(tree.len(outline_copy)))

    const dag: graph.Graph[int] = graph.new(true)
    graph.add_edge(dag, 1, 2)
    graph.add_edge(dag, 2, 4)
    graph.add_edge(dag, 1, 3)
    graph.add_edge(dag, 3, 4)
    io.print(string(graph.is_directed(dag)))
    io.print(string(graph.len(dag)))
    io.print(string(graph.edge_len(dag)))
    io.print(string(graph.has_cycle(dag)))
    match graph.try_topological_sort(dag)
        case some(order):
            io.print(string(lists.len(order)))
        case none:
            io.print("no-topo")
    end
    match graph.shortest_path(dag, 1, 4)
        case some(path):
            io.print(string(path[0] + path[1] + path[2]))
        case none:
            io.print("no-path")
    end
    graph.add_weighted_edge(dag, 1, 3, 10)
    match graph.edge_weight(dag, 1, 3)
        case some(weight):
            io.print(string(weight))
        case none:
            io.print("no-weight")
    end
    match graph.shortest_weighted_path(dag, 1, 4)
        case some(path):
            io.print(string(path[0] + path[1] + path[2]))
        case none:
            io.print("no-weighted-path")
    end
    const closure: graph.Graph[int] = graph.transitive_closure(dag)
    io.print(string(graph.has_edge(closure, 1, 4)))
    const dag_copy: graph.Graph[int] = graph.clone(dag)
    io.print(string(graph.edge_len(dag_copy)))
    var dag_sum: int = 0
    for node in dag_copy
        dag_sum = dag_sum + node
    end
    io.print(string(dag_sum))

    const routes: graph.Graph[string] = graph.new(true)
    graph.add_weighted_edge(routes, "start", "slow", 9)
    graph.add_weighted_edge(routes, "start", "fast", 1)
    graph.add_weighted_edge(routes, "fast", "end", 1)
    graph.add_weighted_edge(routes, "slow", "end", 1)
    match graph.edge_weight(routes, "start", "slow")
        case some(weight):
            io.print(string(weight))
        case none:
            io.print("no-weight")
    end
    match graph.shortest_weighted_path(routes, "start", "end")
        case some(path):
            io.print(path[1])
        case none:
            io.print("no-route")
    end

    const cyclic: graph.Graph[int] = graph.new(true)
    graph.add_edge(cyclic, 1, 2)
    graph.add_edge(cyclic, 2, 1)
    io.print(string(graph.has_cycle(cyclic)))
    match graph.try_topological_sort(cyclic)
        case some(order):
            io.print(string(lists.len(order)))
        case none:
            io.print("cycle")
    end
    const scc: list[list[int]] = graph.strongly_connected_components(cyclic)
    io.print(string(lists.len(scc)))

    const network: graph.Graph[int] = graph.new(false)
    graph.add_edge(network, 10, 11)
    graph.add_edge(network, 20, 21)
    const components: list[list[int]] = graph.components(network)
    io.print(string(lists.len(components)))

    const source: list[int] = [9, 2, 5]
    const from_list: heap.Heap[int] = heap.from_list(source)
    io.print(string(heap.remove(from_list, 5)))
    const sorted: list[int] = heap.into_sorted_list(from_list)
    io.print(string(sorted[0]))
    io.print(string(sorted[1]))
    const extra: heap.Heap[int] = heap.from_list([1, 7])
    const merged: heap.Heap[int] = heap.merge(from_list, extra)
    io.print(string(heap.len(merged)))
    const heap_copy: heap.Heap[int] = heap.clone(merged)
    const heap_items: list[int] = heap.to_list(heap_copy)
    io.print(string(lists.len(heap_items)))
    var heap_sum: int = 0
    for item in heap_copy
        heap_sum = heap_sum + item
    end
    io.print(string(heap_sum))
    heap.clear(heap_copy)
    io.print(string(heap.is_empty(heap_copy)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "completed_collection_gap_apis.exe"
    } else {
        "completed_collection_gap_apis"
    });
    let out = run_compile_with_options(
        &dir.path("main.orl"),
        Path::new(&exe),
        CompileOptions { native_raw: true , lib: false },
    )
    .unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "20\ntrue\n30\ntrue\n2\ntrue\nb\n2\n1\nfalse\n1\ntrue\ntrue\nleaf2\ntrue\nleaf2\nright\n4\ntrue\n4\n4\nfalse\n4\n7\n10\n7\ntrue\n4\n10\n9\nfast\ntrue\ncycle\n1\n2\ntrue\n2\n9\n4\n4\n19\ntrue\n"
    );
}

#[test]
fn compile_runs_managed_values_in_all_collection_stdlibs_native() {
    let dir = TestDir::new("compile_managed_values_all_collections_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.deque = deque
import ori.doubly_linked_list = dll
import ori.graph = graph
import ori.hash_table = hash_table
import ori.heap = heap
import ori.io = io
import ori.linked_list = ll
import ori.list = lists
import ori.map = maps
import ori.queue = queue
import ori.set = sets
import ori.stack = stack
import ori.tree = tree

main()
    var list_values: list[string] = ["list-a"]
    lists.push(list_values, "list-b")
    io.print(list_values[1])

    const map_values: map[string, string] = maps.new()
    maps.set(map_values, "map-key", "map-value")
    io.print(maps.get(map_values, "map-key"))

    const set_values: set[string] = sets.new()
    sets.add(set_values, "set-value")
    io.print(if sets.contains(set_values, "set-value") then "set-ok" else "set-missing")

    const deque_values: deque.Deque[string] = deque.new()
    deque.push_back(deque_values, "deque-value")
    match deque.pop_front(deque_values)
        case some(value):
            io.print(value)
        case none:
            io.print("deque-missing")
    end

    const queue_values: queue.Queue[string] = queue.new()
    queue.enqueue(queue_values, "queue-value")
    match queue.dequeue(queue_values)
        case some(value):
            io.print(value)
        case none:
            io.print("queue-missing")
    end

    const stack_values: stack.Stack[string] = stack.new()
    stack.push(stack_values, "stack-value")
    match stack.pop(stack_values)
        case some(value):
            io.print(value)
        case none:
            io.print("stack-missing")
    end

    const linked_values: ll.LinkedList[string] = ll.new()
    ll.push_back(linked_values, "linked-value")
    match ll.pop_front(linked_values)
        case some(value):
            io.print(value)
        case none:
            io.print("linked-missing")
    end

    const doubly_values: dll.DoublyLinkedList[string] = dll.new()
    dll.push_back(doubly_values, "doubly-value")
    match dll.pop_back(doubly_values)
        case some(value):
            io.print(value)
        case none:
            io.print("doubly-missing")
    end

    const tree_values: tree.Tree[string] = tree.new("tree-root")
    const root: tree.NodeId = tree.root(tree_values)
    const leaf: tree.NodeId = tree.add_child(tree_values, root, "tree-leaf")
    io.print(tree.value(tree_values, leaf))

    const hash_values: hash_table.HashTable[string, string] = hash_table.new()
    hash_table.set(hash_values, "hash-key", "hash-value")
    match hash_table.get(hash_values, "hash-key")
        case some(value):
            io.print(value)
        case none:
            io.print("hash-missing")
    end

    const graph_values: graph.Graph[string] = graph.new(false)
    graph.add_edge(graph_values, "graph-a", "graph-b")
    const graph_walk: list[string] = graph.bfs(graph_values, "graph-a")
    io.print(graph_walk[1])

    const heap_values: heap.Heap[string] = heap.new()
    heap.push(heap_values, "heap-value")
    match heap.pop(heap_values)
        case some(value):
            io.print(value)
        case none:
            io.print("heap-missing")
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "managed_values_all_collections.exe"
    } else {
        "managed_values_all_collections"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "list-b\nmap-value\nset-ok\ndeque-value\nqueue-value\nstack-value\nlinked-value\ndoubly-value\ntree-leaf\nhash-value\ngraph-b\nheap-value\n"
    );
}

#[test]
fn check_rejects_heap_without_comparable_element() {
    let dir = TestDir::new("heap_rejects_missing_comparable");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.heap = heap

struct Score
    value: int
end

main()
    const scores: heap.Heap[Score] = heap.new()
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected comparable diagnostic");
    assert!(
        diagnostic_codes(&out).contains(&"type.collection_comparable_unsupported"),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn compile_runs_for_loop_over_map() {
    let dir = TestDir::new("compile_for_map");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps

main()
    const labels: map[int, string] = maps.new()
    maps.set(labels, 1, "alpha")
    maps.set(labels, 2, "beta")

    var key_total: int = 0
    for key in labels
        key_total = key_total + key
    end
    io.print(string(key_total))

    for key, label in labels
        io.print(string(key))
        io.print(label)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "for_map.exe"
    } else {
        "for_map"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "3\n1\nalpha\n2\nbeta\n");
}

#[test]
fn compile_runs_custom_iterable_native() {
    let dir = TestDir::new("custom_iterable_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.core = core

struct Countdown
    current: int
    min: int
end

apply Countdown use core.Iterable
    mut next() -> optional[int]
        if self.current < self.min
            return none
        end
        const value: int = self.current
        self.current = self.current - 1
        return some(value)
    end
end

main()
    var total: int = 0
    for value, index in Countdown {current: 3, min: 1}
        total = total + value + index
    end
    io.print(string(total))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "custom_iterable_native.exe"
    } else {
        "custom_iterable_native"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "9\n");
}

#[test]
fn check_reports_non_iterable_for_loop() {
    let dir = TestDir::new("non_iterable_for_loop");
    dir.write(
        "main.orl",
        r#"module app.main

main()
    for value in 1
    end
end
"#,
    );

    let out = run_check(&dir.path("main.orl")).unwrap();
    assert!(out.has_errors, "expected non-iterable diagnostic");
    assert!(diagnostic_codes(&out).contains(&"type.not_iterable"));
}

#[test]
fn compile_runs_string_keyed_map_native() {
    let dir = TestDir::new("string_keyed_map_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps

main()
    const labels: map[string, int] = { "alpha": 1, "beta": 2 }
    maps.set(labels, "alpha", 10)
    io.print(string(maps.get(labels, "alpha")))
    io.print(if maps.contains(labels, "beta") then "yes" else "no")
    maps.remove(labels, "beta")
    maps.set(labels, "gamma", 30)
    io.print(string(maps.len(labels)))

    const keys: list[string] = maps.keys(labels)
    const values: list[int] = maps.values(labels)
    io.print(keys[0])
    io.print(string(values[0] + values[1]))

    var total: int = 0
    for label, score in labels
        if label == "gamma"
            total = total + score
        end
    end
    io.print(string(total))

    const entries: list[tuple[string, int]] = maps.entries(labels)
    const first: tuple[string, int] = entries[0]
    io.print(first.0)
    io.print(string(first.1))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "string_keyed_map.exe"
    } else {
        "string_keyed_map"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.replace("\r\n", "\n"),
        "10\nyes\n2\nalpha\n40\n30\nalpha\n10\n"
    );
}

#[test]
fn compile_runs_string_set_native() {
    let dir = TestDir::new("string_set_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.set = sets

main()
    const primary: set[string] = set { "red", "blue", "red" }
    io.print(string(sets.len(primary)))
    io.print(if sets.contains(primary, "red") then "yes" else "no")
    sets.remove(primary, "red")
    io.print(if sets.contains(primary, "red") then "yes" else "no")
    sets.add(primary, "green")

    const other: set[string] = set { "green", "yellow" }
    const merged: set[string] = sets.union(primary, other)
    io.print(string(sets.len(merged)))

    const both: set[string] = sets.intersection(primary, other)
    io.print(string(sets.len(both)))

    const only_other: set[string] = sets.difference(merged, primary)
    io.print(string(sets.len(only_other)))

    var found_green: string = "no"
    for item in both
        if item == "green"
            found_green = "yes"
        end
    end
    io.print(found_green)
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "string_set.exe"
    } else {
        "string_set"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "2\nyes\nno\n3\n1\n1\nyes\n");
}

#[test]
fn compile_runs_trait_gated_user_defined_map_and_set_native() {
    let dir = TestDir::new("trait_gated_user_defined_map_set");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.core = core
import ori.io = io
import ori.map = maps
import ori.set = sets

struct Resource
    id: int
end

apply Resource use core.Hashable
end

apply Resource use core.Equatable
    equals(self, other: Resource) -> bool
        return self.id == other.id
    end
end

main()
    const resource: Resource = Resource {id: 7}
    const labels: map[Resource, int] = maps.new()
    maps.set(labels, resource, 42)
    io.print(string(maps.get(labels, resource)))

    const seen: set[Resource] = sets.new()
    sets.add(seen, resource)
    io.print(if sets.contains(seen, resource) then "yes" else "no")
    sets.remove(seen, resource)
    io.print(string(sets.len(seen)))
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "trait_gated_user_defined_map_set.exe"
    } else {
        "trait_gated_user_defined_map_set"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.replace("\r\n", "\n"), "42\nyes\n0\n");
}

#[test]
fn compile_runs_concurrent_modification_list_runtime_error() {
    let dir = TestDir::new("list_concurrent_modification");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.list = lists

main()
    const values: list[int] = [1, 2, 3]
    for x in values
        lists.push(values, 4)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "list_concurrent_modification.exe"
    } else {
        "list_concurrent_modification"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("concurrent modification during iteration"),
        "stderr was: {stderr}"
    );
}

#[test]
fn compile_runs_concurrent_modification_map_runtime_error() {
    let dir = TestDir::new("map_concurrent_modification");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.map = maps

main()
    const scores: map[int, int] = maps.new()
    maps.set(scores, 1, 10)
    maps.set(scores, 2, 20)
    for k, v in scores
        maps.set(scores, 3, 30)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "map_concurrent_modification.exe"
    } else {
        "map_concurrent_modification"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("concurrent modification during iteration"),
        "stderr was: {stderr}"
    );
}

#[test]
fn compile_runs_concurrent_modification_deque_runtime_error() {
    let dir = TestDir::new("deque_concurrent_modification");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.deque = deque
import ori.io = io

main()
    const d: deque.Deque[int] = deque.new()
    deque.push_back(d, 1)
    deque.push_back(d, 2)
    for x in d
        deque.push_back(d, 3)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "deque_concurrent_modification.exe"
    } else {
        "deque_concurrent_modification"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(!output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("concurrent modification during iteration"),
        "stderr was: {stderr}"
    );
}

#[test]
fn compile_runs_structured_json_api_native() {
    let dir = TestDir::new("structured_json_api_native");
    dir.write(
        "main.orl",
        r#"module app.main

import ori.io = io
import ori.json = json
import ori.list = lists
import ori.map = maps
import ori.math = math

main()
    const input: string = """{
        "name": "Ada",
        "age": 42.0,
        "active": true,
        "scores": [10.0, 20.0],
        "meta": null
    }"""
    match json.parse(input)
    case ok(val):
        match val
        case Object(fields):
            match maps.get(fields, "name")
            case String(value):
                io.println(value)
            case else:
                io.println("no name")
            end

            match maps.get(fields, "age")
            case Number(value):
                io.println(string(math.round(value)))
            case else:
                io.println("no age")
            end

            match maps.get(fields, "active")
            case Bool(value):
                io.println(if value then "true" else "false")
            case else:
                io.println("no active")
            end

            match maps.get(fields, "scores")
            case Array(items):
                io.println(string(lists.len(items)))
                match items[0]
                case Number(value):
                    io.println(string(math.round(value)))
                case else:
                    io.println("no first score")
                end
            case else:
                io.println("no scores")
            end

            match maps.get(fields, "meta")
            case Null:
                io.println("null ok")
            case else:
                io.println("no null")
            end

            -- test stringification
            const output: string = json.stringify(val)
            io.println(output)
        case else:
            io.println("not object")
        end
    case err(err):
        io.println(err)
    end
end
"#,
    );

    let exe = dir.path(if cfg!(windows) {
        "structured_json_api.exe"
    } else {
        "structured_json_api"
    });
    let out = run_compile(&dir.path("main.orl"), Path::new(&exe)).unwrap();
    assert!(!out.has_errors, "{:?}", out.diagnostics);

    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 7);
    assert_eq!(lines[0], "Ada");
    assert_eq!(lines[1], "42");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "2");
    assert_eq!(lines[4], "10");
    assert_eq!(lines[5], "null ok");

    // The stringified JSON should contain key fields
    let stringified = lines[6];
    assert!(stringified.contains("\"name\""));
    assert!(stringified.contains("\"Ada\""));
    assert!(stringified.contains("\"age\""));
    assert!(stringified.contains("42"));
}
