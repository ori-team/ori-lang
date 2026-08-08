# Async and concurrency

> **Audience:** users building programs that wait, schedule work, or share values
> **Native reference:** `ori run`, `ori compile`, and `ori test`
> **Portuguese:** [concurrency.pt-BR.md](concurrency.pt-BR.md)
> **Normative source:** [07-functions.md](../spec/07-functions.md), [10-memory.md](../spec/10-memory.md), and [12-stdlib.md](../spec/12-stdlib.md)

The native backend supports a deliberately small async model. The language
does not expose raw OS threads or raw pointers. Tasks, channels, futures, and
atomics are runtime-backed values with explicit types.

## `async` and `await`

An async function returns `future[T]` to its caller. Inside the body, `return`
still returns a `T`, and `await` turns a `future[T]` into a `T`:

```ori
module app.async_demo

imports
    ori.io = io
    ori.task = task
end

async delayed_answer() -> int
    await task.sleep(1)
    return 42
end

async main()
    const answer: int = await delayed_answer()
    io.println(f"answer: {answer}")
end
```

Rules:

- `await` is valid only inside an `async` function;
- the awaited value must be a `future[T]`;
- managed values that remain live across suspension are retained by the async frame;
- `using` is allowed inside async functions and disposes on normal return, failure, cancellation, `try`, and loop exit;
- the C/debug backend rejects async code with an actionable diagnostic;
- unsupported async shapes are rejected before native code generation rather than silently changing semantics.

The runnable source is [`examples/async_demo/main.orl`](../../examples/async_demo/main.orl).

## Tasks, channels, and atomics

`task.spawn` runs a no-argument function or closure. `task.join` returns a
`result`, so failure is explicit:

```ori
const job: task.Job[int] = task.spawn(() => 41)
match task.join(job)
    case ok(value):
        io.println(f"{value}")
    case err(_):
        io.println("join-error")
end
```

Values crossing a task or channel must satisfy the runtime's `Transferable`
contract. A closure cannot capture a mutable `var` binding for `task.spawn`.
Channels are typed and must be closed by the owner when no more values will be
sent. `atomic.AtomicInt` provides atomic load/store/add operations; it is not a
general-purpose mutex.

The complete runnable example is [`examples/concurrency/main.orl`](../../examples/concurrency/main.orl).

## Cancellation and synchronous bridges

`task.create_token`, `task.cancel`, and `task.associate` connect cancellation to
a future. `task.block_on` is an explicit synchronous bridge; it waits for a
future and drains executor continuations while waiting. It is not inserted
implicitly by `await`.

## Iter generators are different

`iter name(...) -> T` plus `suspend value` creates an inline generator consumed
directly by a `for` loop. It is not a storable callable value and currently has
deliberate restrictions around generics and consumption. For a persistent,
passable iterator, use an explicit state type implementing the stdlib iterable
contract.

## Backend boundary

Native AOT/JIT is the semantic reference for async and concurrency. The C
backend is a debug/transpile route and intentionally rejects async, tasks,
channels, atomics, and native networking. See the feature matrix in
[14-backend-support.md](../spec/14-backend-support.md).
