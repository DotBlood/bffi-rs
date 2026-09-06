# bffi-callback

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/rust-toolchain.toml)

Callback crate of [bffi-rs](https://github.com/DotBlood/bffi-rs/blob/main/README.md) - the Bun-only native
binding framework. Callbacks exist only because a host registered them:
Rust closures live as `Arc<dyn Fn>` behind opaque `u64` handles in the
process-wide [`Registry`](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi-core),
JS-side callbacks are stored as opaque bind-slots, per the design in
[docs/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md) §6.

**Status:** P1 core complete - explicit [`register`] / [`invoke`] /
[`revoke`] (JS -> Rust), [`bind_js_callback`] / [`js_callback`]
(Rust -> JS), and the process-wide JS-thread gate. Cross-thread
marshalling and the C ABI exports arrive with `bffi-event-loop` /
`bffi-macros` (P2) on top of these primitives.

---

## Contents

| Module      | Provides                                                                            |
| ----------- | ------------------------------------------------------------------------------------ |
| [`error`]   | [`CallbackError`] and the lossless `From<CallbackError> for BffiError` bridge         |
| [`registry`] | register / invoke / revoke + `bind_js_callback` / `js_callback` over the two tables |
| [`thread`]  | `set_js_thread` / `ensure_js_thread` - the process-wide JS-thread gate               |
| [`value`]   | [`Value`], [`ValueType`], [`CallbackSig`] - the callback boundary IR                 |

---

## Why explicit registration

Crossing the C ABI means neither side may hold a raw reference to the
other. The callback layer makes every crossing an explicit, owned
transaction:

- **Rust closures live under handles.** `register(sig, f)` stores the
  closure in the global [`Registry`](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi-core)
  and returns an opaque `u64` handle; nothing else is ever handed out.
- **JS hands out bind-slots.** `bind_js_callback(sig, ptr)` stores the
  opaque, handle-sized token JS gave us - never dereferenced here; the
  whole crate is zero-unsafe.
- **Explicit lifecycle.** `register` / `bind` create, `invoke` /
  `js_callback` read, `revoke` removes. After `revoke` the handle is
  dead - terminal, no resurrection, even if its registry slot is later
  reused (the generational handle scheme keeps the stale value dead).

Both tables are declared once per process in a `OnceLock`, whose
memoized outcome makes a failed tag claim sticky: two callback tables
(`0x0200` native, `0x0201` JS) are crate-owned and shared by every
user in the process.

## Two directions, one removal point

| Direction   | Create              | Read           | Remove   |
| ----------- | ------------------- | -------------- | -------- |
| JS -> Rust  | `register`          | `invoke`       | `revoke` |
| Rust -> JS  | `bind_js_callback`  | `js_callback`  | `revoke` |

`revoke` is THE removal point for both directions: the registry's
type-erased `remove` routes by the handle's tag (`0x0200` native,
`0x0201` JS) to the right table. Callers never need to know which kind
a handle belongs to.

## Thread policy

- `set_js_thread` binds the CURRENT thread as the process-wide JS
  thread; the first call wins, later calls are idempotent from the
  bound thread and rejected from any other.
- `ensure_js_thread` is the gate every `invoke` passes: unbound
  process -> `Ok` (pure-Rust usage and tests need no ritual); bound and
  current -> `Ok`; bound and foreign -> `Err(CallbackError::WrongThread)`.
- In P1 a wrong-thread call is REJECTED, not marshalled: delivering the
  call onto the JS thread arrives with `bffi-event-loop` in P2.

## Panic policy

`invoke` does NOT catch panics raised inside a callback body: today a
panic unwinds into the caller - DESIGN.md §6.5 allows debug builds to
abort instead, for easier debugging - and catching at the FFI boundary
is the P2 event-loop trampoline's job (`run_extern_body`). This crate
deliberately stays out of that business.

## Quick start

```rust
use std::sync::Arc;

use bffi_callback::{CallbackSig, Value, ValueType, invoke, register, revoke};

let sig = CallbackSig::new(ValueType::I32, &[ValueType::I32, ValueType::I32]);
let handle = register(
    sig,
    Arc::new(|args: &[Value]| match args {
        [Value::I32(a), Value::I32(b)] => Value::I32(a + b),
        _ => unreachable!("invoke checks the signature before calling"),
    }),
)
.expect("table has room");

let sum = invoke(handle, &[Value::I32(2), Value::I32(3)]).expect("live handle");
assert_eq!(sum, Value::I32(5));

assert!(revoke(handle));
assert!(invoke(handle, &[Value::I32(2), Value::I32(3)]).is_err());
```

## What does _not_ belong here

Per DESIGN §8-9 (one responsibility per crate):

| Concern                              | Home                                    |
| ------------------------------------ | ---------------------------------------- |
| Numbers / strings / buffers conversion | `bffi-types`                          |
| Object ownership (`ObjectWrap`)      | `bffi-object`                           |
| Event-loop marshalling onto the JS thread | `bffi-event-loop` (P2)             |
| `#[bffi]` and C ABI generation       | `bffi-macros`, `bffi-build`             |

## Testing

```sh
cargo test -p bffi-callback          # everything
cargo test -p bffi-callback --doc    # documentation examples
```

Suites: unit tests per module (`error`, `registry`, `thread`, `value`),
plus integration tests covering the register/invoke/revoke lifecycle
and the JS-callback slots (`tests/callback_lifecycle.rs`), the
process-global JS-thread binding (`tests/threading.rs`), and a
revoke-vs-parallel-invoke race (`tests/concurrency.rs`).

## Requirements

- Rust **1.98.0** (pinned workspace-wide via `rust-toolchain.toml`)
- No Bun and no other runtime is needed to use or test this crate

## License

MIT - see [LICENSE](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE).
