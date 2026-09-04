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
| Text and bytes | `ori.string`, `ori.string_view`, `ori.bytes`, `ori.convert` | parsing, formatting, zero-copy views, encoding |
| Collections & memory | `ori.list`, `ori.map`, `ori.set`, `ori.queue`, `ori.stack`, `ori.deque`, `ori.heap`, `ori.linked_list`, `ori.doubly_linked_list`, `ori.buffer`, `ori.slotmap`, `ori.span` | containers, contiguous buffers, generational slots |
| Graphics & images | `ori.image` | BMP/PPM 24-bit uncompressed image generation helpers |
| Errors & diagnostics | `ori.err_trace` | Zero-cost error return traces and formatting |
| Data formats | `ori.json`, `ori.validate` | JSON and validation helpers |
| Time and randomness | `ori.time`, `ori.random`, `ori.format` | time, sampling, display |
| Processes and environment | `ori.args`, `ori.config`, `ori.os`, `ori.process` | command-line tools and host state |
| Networking | `ori.net` | TCP, TLS, UDP, synchronous and async helpers |
| Concurrency & async | `ori.task`, `ori.channel`, `ori.atomic`, `ori.concurrent`, `ori.cancel` | futures, tasks, channels, atomics, thread transfers, cancellation scopes |
| Security | `ori.crypto` | password and TOTP helpers |
| Testing | `ori.test` | assertions, skips, leak checks, doctests |
| Domain structures | `ori.graph`, `ori.tree` | graph and tree algorithms |

Import styles:

```ori
import ori.io as io
import ori.fs (read_text_or)

main()
    const text: string = read_text_or("notes.txt", "")
    io.println(text)
end
```

The complete signatures and backend status are in [12-stdlib.md](../spec/12-stdlib.md).
The generated website data comes from `ori doc export`; run `ori doc check` to
validate inline docs and `.oridoc` sidecars.

Async filesystem, connect, and TLS helpers use a shared bounded native pool
(up to four workers and 256 queued jobs), so blocking work does not create one
thread per request.

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
