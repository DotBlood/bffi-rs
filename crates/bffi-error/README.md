# bffi-error

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/rust-toolchain.toml)

Unified mapping of bffi errors to JavaScript error shapes. Part of
[bffi-rs](https://github.com/DotBlood/bffi-rs) — the Bun-only native binding
framework.

`bffi-core` transports failures across the C ABI (`ErrorCode` return values
plus the thread-local *last error*). This crate answers the remaining
question: **which JavaScript error kind** a failure must become and with
**which message**.

| Concern                              | Home            |
|--------------------------------------|-----------------|
| Error transport (`ErrorCode`, TLS)   | `bffi-core`     |
| Error -> JS-kind policy (this crate) | `bffi-error`    |
| Raising the JS `Error` (ABI/JS glue) | `bffi-build` / facade (P2) |

## Mapping

| `ErrorCode`                                        | JS constructor |
|----------------------------------------------------|----------------|
| `InvalidUtf8`, `NullPointer`, `InvalidArgument`    | `TypeError`    |
| `NumberOutOfRange`, `BufferTooSmall`               | `RangeError`   |
| everything else (`Error`, `Panic`, handle/table…)  | `Error`        |

The rule of thumb: *wrong kind or shape of a value* is a `TypeError`,
*out of an allowed range* is a `RangeError`, everything internal is a plain
`Error`.

## Example

```rust
use bffi_core::{BffiError, ErrorCode};
use bffi_error::{JsErrorExt, JsErrorName};

let error = BffiError::new(ErrorCode::InvalidUtf8, "byte 0xFF at position 2");
let shape = error.to_js_shape();

assert_eq!(shape.name, JsErrorName::TypeError);
assert_eq!(shape.message, "byte 0xFF at position 2");

// On the FFI thread, after a non-OK status was returned:
// let shape = bffi_error::take_last_error_shape();  // drains the slot
```

The future `bffi-build` layer will raise `new TypeError(shape.message)` (and
friends) on the JS side from a [`JsErrorShape`]; this crate contains no FFI
surface by design — pure-Rust policy only.

## API

- [`JsErrorName`] — `Error` / `TypeError` / `RangeError` (`#[non_exhaustive]`).
- [`JsErrorShape`] — `{ name, message }`, the unit the JS-facing layer consumes.
- [`JsErrorExt`] — `to_js_shape()` / `to_js_name()` on `BffiError` and `ErrorCode`.
- [`js_error_name`] — the raw classification function.
- [`take_last_error_shape`] — drains the thread-local last error into a shape.

## Testing

```sh
cargo test -p bffi-error
```

## License

MIT — see [LICENSE](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE).
