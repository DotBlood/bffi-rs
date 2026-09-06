//! # bffi-macros
//!
//! The `#[bffi]` attribute macro of the bffi-rs framework: native
//! bindings for [Bun](https://bun.sh) that target `bun:ffi` and a
//! thin C ABI layer (see
//! [DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md)).
//!
//! ## Mission
//!
//! - A single ABI generator: one macro owns everything that crosses
//!   the FFI boundary - no hand-written shims, no drift between the
//!   Rust signature and the generated C ABI.
//! - Each `#[bffi]` expansion produces exactly two artifacts:
//!   - an `extern "C"` shim that enforces the boundary policy
//!     (copy by default, panics converted into JS errors), and
//!   - a `bffi_meta` const descriptor consumed by `bffi-dts` for
//!     TypeScript `.d.ts` generation.
//! - The user crate must depend on `bffi-core`, `bffi-types`, and
//!   `bffi-dts` so the expansion can name their items without
//!   ambiguity.
//! - Generated symbols use unique, derived names to avoid collisions
//!   between multiple annotated functions.

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

#[allow(unused_extern_crates)]
extern crate proc_macro;

use proc_macro::TokenStream;

/// Marks a function for bffi-rs code generation.
///
/// Expands to a C ABI shim plus a `bffi_meta` descriptor; the full
/// expansion contract lands in Task 5.
#[proc_macro_attribute]
pub fn bffi(_attrs: TokenStream, item: TokenStream) -> TokenStream {
    item
}
