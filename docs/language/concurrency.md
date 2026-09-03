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
- native frame emission verifies slot bounds/layout and zero-initializes managed await bindings before scheduling; full branch-sensitive HIR ownership proof remains future work;
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
contract. A closure cannot capture a mutable `var` binding or directly read or
write a top-level mutable `var` for `task.spawn`. The checker reports
`concurrency.global_mutable_capture`. The checker also follows calls to
same-module helpers and imported named helpers with conservative fixed-point
passes. Receiver methods and `any[Trait]` dispatch use a conservative
method-name summary, so a method that may touch a mutable global is rejected
at the boundary. A direct named function value is accepted when its definition
summary proves no mutable-global effect. A closure stored in a local binding
is also accepted when the checker records only transferable captures. Unknown
function environments and nested function captures remain conservative.
Resource handles (`fs.File`, `io.Input`, `io.Output`, `net.Connection`,
`net.Listener`, and `net.UdpSocket`) are never transferable; they borrow
process/OS state. `task.CancelToken` is the explicit exception because it is an
atomic cancellation flag intended for cross-task coordination.
Channels are typed and must
be closed by the owner when no more values will be sent. `channel.create`
creates an unbounded FIFO. `channel.create_bounded(capacity)` returns an
`optional[channel.Channel[T]]`: positive capacities create a bounded FIFO,
while zero or negative capacities return `none`. A full bounded channel makes
`send` wait until a receiver removes an item; `close` wakes blocked senders and
they receive `err(...)`. `atomic.AtomicInt`
provides atomic load/store/add operations; it is not a general-purpose mutex.

Managed channel payloads are supported when their type is `Transferable`; the
runtime retains queued values and releases them on receive, close, or teardown.
Network readiness operations retain their handles and synchronize close, so a
handle may be closed while an operation is pending; the operation completes
with its documented result instead of touching a freed resource.

The complete runnable example is [`examples/concurrency/main.orl`](../../examples/concurrency/main.orl).

## Cancellation tokens and thread-transfer helpers

`ori.cancel` currently wraps cancellation tokens. It does not yet own/join a
tree of child tasks, so this is not a complete structured-concurrency scope:

```ori
import ori.cancel = cancel

const scope: cancel.CancelScope = cancel.create_scope()
if cancel.is_cancelled(scope)
    return
end
cancel.cancel(scope)
```

`cancel.defer_cancel` is asynchronous: await it when using it as a timeout. It
waits for the requested delay before cancelling the scope. Child-task lifetime
and cancel-on-scope-exit remain open under `ASYNC-STRUCT-1`.

The helpers in `ori.concurrent` (`transfer_int`, `transfer_string`,
`transfer_list_string`) copy selected values. They do not define a complete
type-level transfer or ownership model for arbitrary managed values.

`task.block_on` is an explicit synchronous bridge; it waits for a future and
drains executor continuations. `ori_reactor_poll` currently waits on the
executor queue; Unix network readiness also uses a separate single `poll`
worker. It is not inserted implicitly by `await`.

Blocking filesystem, connect, and TLS operations do not create one thread per
request. They share a bounded native I/O pool with up to four workers and a
256-job FIFO queue. When the queue is full, submission waits for capacity. If
the runtime is shutting down or cannot create a worker, the future completes as
a failure instead of leaving an unowned job behind.

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
