# bffi-class

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/z2net/bffi-rs/blob/main/rust-toolchain.toml)

Class-declaration crate of [bffi-rs](https://github.com/z2net/bffi-rs/blob/main/README.md) - the Bun-only native
binding framework: declare a Rust struct, get the JS-facing
constructor/method/destructor shims plus a const TypeScript descriptor,
per DESIGN.md §6.2 and the `bffi-object` ownership model.

**Status:** P2 complete - `#[bffi_class]` / `#[bffi_impl]` /
`#[bffi_constructor]` over
[`ObjectWrap`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi-object)
(tags `0x0100-0x01FF`), with `ClassDef` descriptors rendered by
[`bffi-dts`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi-dts).
Diagnostics continue the stable series at `E005`-`E008`.

---

## Syntax v1

```rust
use bffi_class::{bffi_class, bffi_constructor, bffi_impl};

#[bffi_class(tag = 0x0142)]
/// A counter.
pub struct Counter {
    /// The current value.
    pub value: u32,
    secret: u8,          // private fields are never exported
}

#[bffi_impl]
impl Counter {
    #[bffi_constructor]
    /// Creates a counter.
    pub fn new(start: u32) -> Self { Self { value: start, secret: 0 } }

    /// Adds one and returns the new value.
    pub fn increment(&self) -> u32 { self.value + 1 }
}
```

Both `#[bffi_class]` and `#[bffi_impl]` accept one optional
`crate = "<name>"` option (facade-only mode): the generated paths then
resolve through `::<name>::{core, types, dts, object, build}` - the
re-export namespaces of the
[`bffi`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi)
facade - instead of the direct dependencies. Use the same value on
both macros of one class; anything else besides `tag` is rejected.

```rust
#[bffi_class(tag = 0x0142, crate = "bffi")]
pub struct FacadeCounter { pub value: u32 }

#[bffi_impl(crate = "bffi")]
impl FacadeCounter { /* ... */ }
```

The generated C ABI surface:

| Symbol | Role |
| ------ | ---- |
| `bffi_counter_new(params..., __ret: *mut u64)` | constructor: wraps `Self`, writes the handle |
| `bffi_counter_<field>_get(handle, __ret)` | read-only getter per `pub` field |
| `bffi_counter_<method>(handle, params...[, __ret])` | `&self` method call |
| `bffi_counter_release(handle)` | destructor: frees the slot (Arc-lenient) |

## The metadata split

Two macro expansions cannot write into one module, so the descriptor is
assembled from both:

- `#[bffi_class]` emits `bffi_meta_counter::{TAG, DOCS, FIELDS}`;
- `#[bffi_impl]` emits `bffi_meta_counter_impl::CLASS` - the complete
  [`::bffi_dts::ClassDef`] referencing the former by `super::` path.

Aggregation is explicit (no inventory/linkme):

```rust
static CLASSES: &[bffi_dts::ClassDef] = &[bffi_meta_counter_impl::CLASS];
let module = bffi_dts::ModuleDef { name: "app", fns: &[], classes: CLASSES };
```

## Rules v1

- The tag is a literal in `0x0100..=0x01FF`, checked at compile time
  (`E006`); one tag = one type per process.
- Methods take `&self` only (`E007` for `&mut self`/`self`):
  `ObjectWrap` stores `Arc<T>`, and the Arc-lenient ownership model has
  no safe mutation - use interior mutability (`Mutex`/atomics) inside
  the struct.
- `pub` fields export as read-only getters, primitive-typed only
  (`E007` on other types); `&str`/owned fields cannot be served.
- Exactly one `#[bffi_constructor]` returning `Self` per impl (`E008`);
  a JS class cannot be instantiated without one.
- Method parameters/returns follow the `#[bffi]` matrix, including
  the borrowed `&[u8]` parameters (a `(ptr, len)` pair at the ABI
  level, descriptor sees one `Uint8Array`), the P2 buffer payloads
  and `Result<T, E>` err channel. An `#[bffi_impl]` without a
  matching `#[bffi_class]` surfaces as a missing
  `__bffi_<name>_wrap` function.

## Diagnostics

| Code   | Meaning                                            |
| ------ | -------------------------------------------------- |
| `E005` | unsupported class shape (not a named struct/generic) |
| `E006` | bad attribute arguments (`tag`, `crate` option)    |
| `E007` | unsupported method/field shape or type             |
| `E008` | impl binding problems (constructor count/shape)    |

Same rendering as `bffi-macros`: `bffi[E0XX]:` + ` = help: ` +
` = note: ` lines; golden `.stderr` files lock the texts.

## What does _not_ belong here

| Concern | Home crate |
| ------- | ---------- |
| plain-function shims | `bffi-macros` |
| object ownership primitives | `bffi-object` |
| descriptor rendering | `bffi-dts` |
| JS-facing runtime exports | `bffi-build` |

## Testing

```sh
cargo test -p bffi-class
```

`tests/shim.rs` - direct shim calls: lifecycle, UAF barriers (null /
wrong-tag / forged generation), buffer and `Result` returns.
`tests/descriptor.rs` - the assembled `ClassDef` vs a literal, and its
`.d.ts` render. `tests/ui` - the `E005`-`E008` goldens.

## Requirements on the user crate

- Default mode: dependencies on `bffi-core`, `bffi-object`,
  `bffi-types`, `bffi-dts` (and `bffi-build` for buffer/`Result`
  returns): the expansion names `::bffi_core`, `::bffi_object`,
  `::bffi_types`, `::bffi_dts`, `::bffi_build` at the call site.
- Facade-only mode (`crate = "bffi"` on both `#[bffi_class]` and
  `#[bffi_impl]`): the
  [`bffi`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi)
  facade alone - the expansion names `::bffi::core`, `::bffi::object`,
  `::bffi::types`, `::bffi::dts`, `::bffi::build`.
- Rust **edition 2024** (`#[unsafe(no_mangle)]` shims).

## Requirements

- Rust 1.98.0 (pinned via `rust-toolchain.toml`)

## License

[MIT](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
