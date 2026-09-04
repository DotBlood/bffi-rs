# bffi-types

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/rust-toolchain.toml)

Type conversion between JavaScript values and Rust for
[bffi-rs](https://github.com/DotBlood/bffi-rs) - the Bun-only native binding
framework. Built on [`bffi-core`](../bffi-core) primitives.

This is the **policy** layer: it defines what a conversion _means_ (copy vs
zero-copy, strict vs coercing) over plain safe Rust types. The **syntax**
layer - `extern "C"` entry points, raw-pointer validation, `bun:ffi`
declarations - belongs to `bffi-build` (P2).

| Concern                              | Home              |
| ------------------------------------ | ----------------- |
| Conversion semantics (this crate)    | `bffi-types`      |
| FFI entry points, pointer validation | `bffi-build` (P2) |
| JS `Error` shapes for failures       | `bffi-error`      |
| Error transport (`ErrorCode`, TLS)   | `bffi-core`       |

## Numbers: three explicit policies

A JS number is an `f64`; converting to a fixed-width integer can lose data.
`JsNumber` never converts implicitly:

| Policy                             | `3e9` to `i32` | Use when                         |
| ---------------------------------- | -------------- | -------------------------------- |
| strict - `try_into_i32()`          | `Err`          | default; losses must be visible  |
| saturating - `to_i32_saturating()` | `i32::MAX`     | clamping is acceptable           |
| JS - `to_i32_js()`                 | `-1294967296`  | matching `x \| 0` / typed arrays |

Rust → JS: lossless `From` for `i8..i32`, `u8..u32`, `f32` (their full
ranges fit an `f64` exactly); `i64`/`u64` use checked
(`try_from_i64`/`try_from_u64`, |v| ≤ 2^53) or explicitly lossy
(`from_i64_lossy`) helpers.

## Strings: copy by default

- `bytes_to_string(&[u8]) -> Result<String, BffiError>` - validates UTF-8
  (`ErrorCode::InvalidUtf8` on failure) and **copies** into an owned
  `String`.
- `string_to_bytes(&str) -> Vec<u8>` - the symmetric owned copy.
- `unsafe_zero_copy::str_view(&[u8]) -> Result<ZeroCopyStr<'_>, _>` - the
  explicitly named zero-copy path (see below). UTF-8 is still validated:
  zero-copy means _no copy_, never _no checks_.

## Buffers: owned by default, borrowing only behind `unsafe_zero_copy`

- `CopiedBuf` - the owned buffer type; every construction copies or takes
  ownership, never aliases.
- `unsafe_zero_copy::buf_view(&[u8]) -> ZeroCopyBuf<'_>` / `ZeroCopyStr` -
  borrowed views for hot paths. Their lifetimes mirror the input, so a
  view cannot outlive the FFI call that produced the buffer. Never store
  one in Rust state - JS may keep mutating its memory between calls.
- `CopiedBuf::from(&view)` - the explicit, visible copy-out operation.

Per DESIGN §6.3, zero-copy exists **only** behind the `unsafe_zero_copy`
module name; nothing in this crate produces a borrowed view implicitly.

## Example

```rust
use bffi_types::{bytes_to_string, CopiedBuf, JsNumber};

let n = JsNumber::new(3.0e9);
assert!(n.try_into_i32().is_err());          // strict: the default
assert_eq!(n.to_i32_saturating(), i32::MAX); // clamping
assert_eq!(n.to_i32_js(), -1_294_967_296);   // JS: 3e9 | 0

let text = bytes_to_string("привет 🚀".as_bytes())?;
assert_eq!(text, "привет 🚀");

let mut source = vec![1_u8, 2, 3];
let copied = CopiedBuf::from_slice(&source);
source[0] = 99;
assert_eq!(copied.as_slice(), [1, 2, 3]);
# Ok::<(), bffi_core::BffiError>(())
```

## Testing

```sh
cargo test -p bffi-types
```

Every converter has its own suite: numeric boundaries and ECMAScript
reference vectors (`tests/num.rs`), UTF-8 validity categories and copy
independence (`tests/string.rs`), buffer aliasing guarantees
(`tests/buffer.rs`).

## License

MIT - see [LICENSE](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE).
