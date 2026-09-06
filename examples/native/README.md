# bffi-example-native

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/rust-toolchain.toml)

Example native module of [bffi-rs](https://github.com/DotBlood/bffi-rs/blob/main/README.md):
the first real cdylib that Bun loads through `bun:ffi` (P2, criterion
"build a native artifact from a bun script"). It closes the P1-SPEC
§10.5 risk - the out-parameter, cstring and panic ABI guesses are now
verified by a real host.

## What it exports

| Symbol                                      | Covers                                     |
| ------------------------------------------- | ------------------------------------------ |
| `bffi_add(a, b, __ret)`                     | primitives + out-parameter                 |
| `bffi_greet(name)` / `bffi_greet_len()`     | the cstring (`&str`) parameter path        |
| `bffi_boom()`                               | the release panic path (`ErrorCode::Panic`) |
| `example_echo_buffer(ptr, len, __ret)`      | hand-written `ptr, len` parameter sketch   |
| `bffi_error_*`, `bffi_buffer`, `bffi_types_free` | `bffi_runtime_abi!()` runtime exports  |

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
  functions + one hand-written export (the reference sketch for the
  future `ptr, len` parameter convention).
- `js/load.ts` - `dlopen` declarations + `takeError()` /
  `readBuffer()` helpers over the C ABI
  ([CALLING-CONVENTION.md](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi-build/CALLING-CONVENTION.md)).
- `js/native.test.ts` - the e2e suite (bun test).

## Requirements

- Rust 1.98.0 (pinned), Bun >= 1.4.0

## License

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
