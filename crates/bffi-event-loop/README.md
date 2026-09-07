# bffi-event-loop

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/rust-toolchain.toml)

Event-loop crate of [bffi-rs](https://github.com/DotBlood/bffi-rs/blob/main/README.md) - the Bun-only
native binding framework. Native code cannot hook Bun's real loop, so
this is the honest next thing: a thread-safe job queue with two drains -
a blocking one ([`run`]) and a non-blocking one ([`pump`]), per
DESIGN.md §7.

**Status:** P2 core complete - [`enqueue`] / [`marshal`] from any
thread, [`run`] as the blocking executor with per-job panic
containment, sticky [`stop`], [`pending`] / [`executed_total`] /
[`is_running`] introspection, and [`pump`] as a non-blocking drain.
The Bun-tick integration (calling `pump` periodically from a JS
loader) is loader-side work.

---

## Contents

| Module | Provides |
| ------ | -------- |
| [`lib`] | `enqueue` / `marshal` / `run` / `stop` / `pending` / `executed_total` / `is_running` / `pump`, [`EventLoopError`] |

## The marshalling story (criterion 5.3)

P1 made wrong-thread `invoke` calls REJECT
(`bffi_callback::ensure_js_thread` -> `CallbackError::WrongThread`).
P2 adds the delivery half: background threads hand work to the JS
thread through [`marshal`], the loop executes it on the runner thread,
where the `ensure_js_thread` gate passes.

- `marshal` without a runner -> [`EventLoopError::NotRunning`] ->
  `ErrorCode::WrongThread` (12) - the caller asked for marshalling and
  there was no one to marshal to.
- The loop does NOT bind the JS thread automatically: the first
  `set_js_thread` binder wins process-wide, and an automatic bind from
  a worker would be wrong with no way to undo it. Bind the runner
  yourself before/when starting it (see `tests/threading.rs` for the
  reference pattern).

## Semantics

| Call | Behavior |
| ---- | -------- |
| `enqueue(job)` | any thread; fails with `Stopped` after `stop` (sticky) |
| `marshal(job)` | `enqueue` + a runner check; `NotRunning` when no runner is active |
| `run()` | blocks the calling thread until `stop`; executes every job under `run_extern_body` (panic -> last error, loop lives); returns THIS runner's executed count |
| `stop()` | wakes all runners (current job finishes, queued jobs stay queued); sticky; idempotent |
| `pump()` | non-blocking drain: executes queued jobs without waiting (never touches the condvar); returns THIS call's executed count; safe alongside `run()` |

Extra runners (nested or parallel `run` calls) are allowed but
discouraged: they drain and exit on an empty queue while the first
runner keeps waiting for `stop`.

The queue is `Mutex<VecDeque>` + `Condvar` on purpose: the lock-free
machinery of P0 serves handle tables (CAS traffic on every FFI call);
a job queue sees a handful of transitions per job and does not need it.

## Quick start

```rust
use std::sync::atomic::{AtomicU32, Ordering};

static RESULT: AtomicU32 = AtomicU32::new(0);

bffi_event_loop::enqueue(Box::new(|| RESULT.store(42, Ordering::Relaxed)))
    .expect("not stopped");

let runner = std::thread::spawn(bffi_event_loop::run);
while RESULT.load(Ordering::Relaxed) == 0 {
    std::thread::yield_now(); // stop() is sticky - let the job run first
}
bffi_event_loop::stop();
assert_eq!(runner.join().ok(), Some(1));
```

## What does _not_ belong here

| Concern | Home crate |
| ------- | ---------- |
| callback registration and the JS-thread gate | `bffi-callback` |
| panic containment primitives | `bffi-core` |
| JS-facing exports | `bffi-build` |

## Testing

```sh
cargo test -p bffi-event-loop
```

`tests/loop.rs` - the full lifecycle in one sequential test (stop is
process-global and sticky, so phases, not separate tests), including
the `run()`-vs-`pump()` exactly-once race. 
`tests/threading.rs` - the reference pattern for binding a helper
"JS thread" and marshalling `invoke` calls onto it, including the
wrong-thread reject -> `ErrorCode::WrongThread` mapping.

## Requirements

- Rust 1.98.0 (pinned via `rust-toolchain.toml`)
- No Bun needed for the Rust-side tests

## License

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
