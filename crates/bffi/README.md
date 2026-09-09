# bffi

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/z2net/bffi-rs/blob/main/rust-toolchain.toml)

The public facade of [bffi-rs](https://github.com/z2net/bffi-rs/blob/main/README.md) - the Bun-only native
binding framework. One dependency re-exporting the whole stack
(DESIGN.md §5), with
[`bffi::unsafe_zero_copy`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi/src/lib.rs)
as the single zero-copy door (DESIGN.md §6.3).

**Status:** P2 complete - flat re-exports of core/types/error/object/
callback/dts/event-loop/build, the `#[bffi]`, `#[bffi_class]`,
`#[bffi_impl]`, `#[bffi_constructor]` macros and the
`bffi_runtime_abi!` generator, plus the `core`/`types`/`dts`/
`object`/`build` namespaces for facade-only mode.

---

## Usage

Default mode (direct dependencies; the macro expansions name
`::bffi_core` / `::bffi_types` / `::bffi_dts` / `::bffi_object` /
`::bffi_build` absolute paths, which resolve only through DIRECT
dependencies):

```toml
[dependencies]
bffi = { path = "../crates/bffi" }

bffi-core = { path = "../crates/bffi-core" }
bffi-types = { path = "../crates/bffi-types" }
bffi-dts = { path = "../crates/bffi-dts" }
bffi-object = { path = "../crates/bffi-object" }   # classes
bffi-build = { path = "../crates/bffi-build" }     # buffer/Result returns, runtime ABI
```

Facade-only mode (one dependency): annotate with
`#[bffi(crate = "bffi")]` (and `crate = "bffi"` on `#[bffi_class]` /
`#[bffi_impl]`) - the expansions then name `::bffi::core`,
`::bffi::types`, `::bffi::dts`, `::bffi::object`, `::bffi::build`,
the namespaces re-exported by this crate:

```toml
[dependencies]
bffi = { path = "../crates/bffi" }
```

Note: `bffi_runtime_abi!` always needs `bffi-build` as a direct
dependency (its paths are `$crate`-relative to `bffi-build`).

```rust
use bffi::{CopiedBuf, ErrorCode, Handle, ObjectWrap, TypeTag};

const SESSION: TypeTag = TypeTag(0x0160);

let wrap = ObjectWrap::<u32>::new(SESSION).expect("tag claimed once");
let handle = wrap.wrap(42).expect("room");
assert_eq!(*wrap.get(handle).expect("live"), 42);
let copied = CopiedBuf::from_slice(b"copy by default");
let _ = (handle, copied.as_slice(), ErrorCode::Ok);
```

## The zero-copy door

Everything in the facade copies by default. Zero-copy exists only as:

- `bffi::str_view(bytes) -> Result<ZeroCopyStr, BffiError>` and
  `bffi::buf_view(bytes) -> ZeroCopyBuf` at the root, next to the
  copying converters;
- the view TYPES (`ZeroCopyStr`/`ZeroCopyBuf`) only through
  `bffi::unsafe_zero_copy` - the module name is the warning label.

The genuinely unsafe `(ptr, len) -> &[u8]` step at the ABI belongs to
the generated shims (`bffi-macros` / `bffi-class` / `bffi_runtime_abi!`
in `bffi-build`), never to user code.

## What does _not_ belong here

| Concern | Home crate |
| ------- | ---------- |
| any new functionality | the individual `bffi-*` crates |
| runtime tables / ABI exports | `bffi-build` |
| event loop implementation | `bffi-event-loop` |

The facade re-exports; it does not implement. A name missing here is a
bug in the audit (`tests/reexports.rs`), not an invitation to add
logic.

## Testing

```sh
cargo test -p bffi
```

`tests/reexports.rs` is the surface audit: every documented name,
both proc macros in use, and the zero-copy door.

## The JS side

JavaScript consumes this stack through
[`packages/bffi`](https://github.com/z2net/bffi-rs/blob/main/packages/bffi)
(`@z2net/bffi`): the `bffi codegen` CLI (packages/bffi-cli) turns the
aggregated `ModuleDef` (canonical loader JSON from
`bffi_build::loader_json`) into a typed TS module whose `ApiOf<>`
derives exact signatures from the descriptors - see the root README,
"Generated TypeScript API".

## Requirements

- Rust 1.98.0 (pinned via `rust-toolchain.toml`)
- Bun >= 1.4.0 for anything that loads a cdylib

## License

[MIT](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
