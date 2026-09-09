# bffi-async

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/z2net/bffi-rs/blob/main/rust-toolchain.toml)

Promise / async support for [bffi-rs](https://github.com/z2net/bffi-rs/blob/main/README.md) -
the Bun-only native binding framework. Rust futures spawned by
`#[bffi_async]` run on an N-worker executor and resolve attached
JavaScript Promises through the event loop, per DESIGN.md §7 and the
[`bffi-event-loop`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi-event-loop)
delivery contract.

**Status:** P3 core complete - [`spawn`] with cooperative [`cancel`],
[`timeout`] / [`sleep`] over a dedicated timer thread, resolver
attachment (`bffi_async_attach` via [`bffi_async_abi!`]), the
`AsyncValue` codec, and the opt-in `tokio` feature. Descriptors:
`#[bffi_async]` in
[`bffi-macros`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi-macros)
emits `Promise<T>` types from
[`bffi-dts`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi-dts).

## Threading model

| Thread | Role |
| ------ | ---- |
| JS thread | the only thread that touches JavaScript; drains the event loop (`pump()` / `run()`) |
| executor workers (N, machine parallelism capped at 4) | poll the spawned futures |
| timer thread | deadlines for `sleep` / `timeout` |

The Bun-support invariant: workers and the timer never touch
JavaScript. A completed future is delivered by enqueuing a job onto the
event loop; the JS thread runs it while draining. **Promises settle
only while the JS side pumps the loop** - the loader pattern is a
periodic `pump()`.

## Cancellation and timeouts

- [`cancel`] is cooperative: the future is dropped at the next poll
  boundary; the promise rejects with "task cancelled". The first
  terminal transition (cancel vs completion) wins; the loser is a
  no-op.
- [`timeout`] wraps a future with a deadline; on expiry the inner
  future is dropped and the task fails with "task timed out".

## Value codec

`AsyncValue` encodes into `[tag][payload]` buffers (i32/i64/f64/bool/
string/bytes), keeping `i64` exact (no `f64` narrowing). JavaScript
decodes the tag in the loader (`decodeValue`) and frees the buffer with
`bffi_types_free`.

## Tokio (opt-in)

```toml
bffi-async = { features = ["tokio"] }
```

With the feature, two things unlock:

- `spawn_on_tokio` polls a future on a lazily built multi-thread
  tokio runtime (worker threads = machine parallelism, capped at 8) -
  sockets and tokio timers work inside the future;
- `set_executor_kind(ExecutorKind::Tokio)` switches the process-wide
  mode, so `spawn` (and therefore every `#[bffi_async]` shim) routes
  to tokio.

Cancellation aborts the tokio task (`JoinHandle::abort`) - the future
is dropped at its next await point, the promise rejects with "task
cancelled". Panicking futures are caught by the completion wrapper
before tokio sees them: the task fails with the panic message and the
runtime survives. Set the mode once at startup, before the first
spawn; `spawn_on_tokio` works regardless of the mode.

The default build carries no tokio dependency - the built-in N-worker
executor and the timer thread cover sleep/timeout without it.

## What does _not_ belong here

| Concern | Home crate |
| ------- | ---------- |
| the event loop itself | `bffi-event-loop` |
| the runtime ABI transport (`bffi_buffer` pair) | `bffi-build` |
| the `#[bffi_async]` proc macro | `bffi-macros` |
| TypeScript rendering of `Promise<T>` | `bffi-dts` |

## Testing

```sh
cargo test -p bffi-async
```

Core suites cover ready/pending-woken futures, cancellation races,
timeout expiry dropping the inner future, panicking futures (the
executor survives), and the exactly-once execution guarantee under
mixed runner/pumper load.

## Requirements

- Rust 1.98.0 (pinned via `rust-toolchain.toml`)
- Bun >= 1.4.0 for the e2e suite

## License

[MIT](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
