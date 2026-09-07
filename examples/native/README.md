# bffi-example-native

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/rust-toolchain.toml)

Example native module of [bffi-rs](https://github.com/DotBlood/bffi-rs/blob/main/README.md):
the first real cdylib that Bun loads through `bun:ffi` (P2, criterion
"build a native artifact from a bun script"). It closes the P1-SPEC
§10.5 risk - the out-parameter, cstring and panic ABI guesses are now
verified by a real host - and verifies the unfinished P1 surface
(callbacks, wrong-thread, marshal, bigint/bool matrix, `.d.ts` to real
tsc) with Bun 1.4.

## What it exports

| Symbol                                      | Covers                                     |
| ------------------------------------------- | ------------------------------------------ |
| `bffi_add(a, b, __ret)`                     | primitives + out-parameter                 |
| `bffi_greet(name)` / `bffi_greet_len()`     | the cstring (`&str`) parameter path        |
| `bffi_echo_buffer(ptr, len, __ret)`         | the borrowed `&[u8]` parameter path (real `#[bffi]` shim, `(ptr, len)` pair) |
| `bffi_boom()`                               | the release panic path (`ErrorCode::Panic`) |
| `bffi_mirror_i64` / `bffi_mirror_u64` / `bffi_is_even` | bigint + bool paths of the P1 type matrix |
| `example_callback_register` / `_invoke` / `_invoke_mismatched` / `_revoke` | JS -> Rust callback lifecycle (register/invoke/revoke, signature mismatch, terminal revocation) |
| `example_js_callback_bind` / `_get` / `_revoke` | Rust -> JS callback ownership (pointer roundtrip) |
| `example_set_js_thread`                     | the process-wide JS-thread binding         |
| `example_loop_run` / `_stop` / `_pending` / `_executed` / `_pump` / `_marshal_invoke`, `example_last_invoked` | event-loop probes + wrong-thread marshal delivery |
| `bffi_error_*`, `bffi_buffer`, `bffi_types_free` | `bffi_runtime_abi!()` runtime exports  |

## Verified surface

| Kanboard criterion | Surface                                              | Verified by                                     |
| ------------------ | ---------------------------------------------------- | ----------------------------------------------- |
| 5.1                | callback register/invoke/revoke; Rust -> JS bind/get | `js/callbacks.test.ts` phases A and B           |
| 5.2                | signature mismatch -> 11; revocation is terminal     | `js/callbacks.test.ts` phase A                  |
| 5.3                | wrong-thread reject (12) + marshal to the bound thread | `js/callbacks.test.ts` phase C (real Worker)  |
| 5.3 (probes)       | pending/executed counters; `pump()` drain probe      | `js/callbacks.test.ts` phase C                  |
| P1 type matrix     | i64/u64 bigint-exact roundtrips; bool path           | `js/numbers.test.ts`                            |
| 6.1                | `.d.ts` rendered from the `bffi_meta` IR (deterministic, LF) | `tests/dts.rs` (golden `js/api.d.ts`)    |
| 6.2                | the generated declarations compile under real tsc    | `js/api-usage.ts` via `bun run typecheck`       |

## Run

```sh
bun run build        # cargo build --release -p bffi-example-native
bun test examples/native
```

or both at once: `bun run test:native`. The tests skip when the release
artifact is missing; debug artifacts are never loaded (the panic test
relies on the release boundary policy - debug aborts by design).

## Layout

- `src/lib.rs` - the cdylib: `bffi_runtime_abi!()` + `#[bffi]`
  functions (including `echo_buffer`, the borrowed `&[u8]` parameter
  path) + hand-written verification exports (`bffi_extern!`-based
  wrappers over `bffi-callback`/`bffi-event-loop`, each documented
  with the criterion it exercises).
- `js/load.ts` - `dlopen` declarations + `takeError()` /
  `readBuffer()` helpers over the C ABI
  ([CALLING-CONVENTION.md](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi-build/CALLING-CONVENTION.md)).
- `js/native.test.ts` - the original e2e suite (bun test).
- `js/numbers.test.ts` - the P1 type matrix e2e (bigint/bool).
- `js/callbacks.test.ts` - the phased callback/lifecycle/marshal e2e
  (one test: the JS-thread binding is process-global and sticky);
  `js/worker.ts` is its worker half.
- `js/api.d.ts` - the committed golden, generated from the `bffi_meta`
  descriptors via `bffi_build::dts::write_to_file` (the build-time
  half of criterion 6.2); `js/api-usage.ts` type-checks it under tsc.
- `tests/dts.rs` - pins the render to the golden (determinism, LF) and
  exercises the `write_to_file` round-trip.

## Requirements

- Rust 1.98.0 (pinned), Bun >= 1.4.0

## License

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
