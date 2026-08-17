# Standard library reference map

> **Audience:** users choosing an `ori.*` module
> **Portuguese:** [stdlib-reference.pt-BR.md](stdlib-reference.pt-BR.md)
> **Normative contracts:** [spec/12-stdlib.md](../spec/12-stdlib.md)

The standard library has three layers:

1. Layer 1: Rust runtime primitives registered in the compiler manifest;
2. Layer 2: safe `.orl` wrappers;
3. Layer 3: algorithms written in Ori.

Use the canonical parent module first. Compatibility paths such as
`ori.string.utils` and `ori.map.algorithms` remain available where documented,
but new code should prefer `ori.string`, `ori.map`, and the other parents.

## Module map

| Domain | Canonical modules | Typical use |
|---|---|---|
| I/O | `ori.io`, `ori.fs`, `ori.path` | streams, files, paths |
| Text and bytes | `ori.string`, `ori.bytes`, `ori.convert` | parsing, formatting, encoding |
| Collections | `ori.list`, `ori.map`, `ori.set`, `ori.queue`, `ori.stack`, `ori.deque`, `ori.heap`, `ori.linked_list`, `ori.doubly_linked_list` | containers and algorithms |
| Data formats | `ori.json`, `ori.validate` | JSON and validation helpers |
| Time and randomness | `ori.time`, `ori.random`, `ori.format` | time, sampling, display |
| Processes and environment | `ori.args`, `ori.config`, `ori.os`, `ori.process` | command-line tools and host state |
| Networking | `ori.net` | TCP, TLS, UDP, synchronous and async helpers |
| Concurrency | `ori.task`, `ori.channel`, `ori.atomic`, `ori.concurrent` | futures, tasks, channels, atomics |
| Security | `ori.crypto` | password and TOTP helpers |
| Testing | `ori.test` | assertions, skips, leak checks |
| Domain structures | `ori.graph`, `ori.tree` | graph and tree algorithms |

Import styles:

```ori
import ori.io = io
import ori.fs (read_text_or)

main()
    const text: string = read_text_or("notes.txt", "")
    io.println(text)
end
```

The complete signatures and backend status are in [12-stdlib.md](../spec/12-stdlib.md).
The generated website data comes from `ori doc export`; run `ori doc check` to
validate inline docs and `.oridoc` sidecars.

## Text positions and bytes

`string` stores valid UTF-8. `len(text)`, `text.len()`, slicing, indexing,
`index_of`, `chars()`, and direct `for` iteration use Unicode scalar positions.
They do not expose UTF-8 byte offsets. One visible grapheme can still contain
multiple scalars; use `bytes` and `string.to_bytes` when a protocol requires
the encoded bytes. Grapheme segmentation and normalization are planned but are
not current stdlib APIs.

## Backend and error conventions

Native runtime functions are the semantic reference. Filesystem operations that
can fail return `result[...]`; `ori.io.read_line` returns `optional[string]` at
EOF or when the input is not valid UTF-8. Networking has both blocking wrappers and async helpers, but the async
reactor is not a separate OS-level event loop yet.

Opaque collections expose methods through their module contracts rather than a
public memory layout. Do not pass their internal representation through FFI.
