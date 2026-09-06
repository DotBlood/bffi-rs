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

mod errors;
mod mapping;
mod model;
mod shim;

use proc_macro::TokenStream;

/// Marks a function for bffi-rs code generation.
///
/// The signature is parsed and validated against the P1 boundary rules
/// (plain `fn`s over primitives, `&str` and `()` only - see
/// [DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md)).
/// Inputs outside the rules produce a spanned compile error with the
/// documented help lines. A validated item expands into its original
/// tokens plus an `extern "C"` shim (`bffi_<name>`) generated under
/// the boundary policy; the `bffi_meta` descriptor lands in Task 4.
#[proc_macro_attribute]
pub fn bffi(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = proc_macro2::TokenStream::from(attrs);
    let item = proc_macro2::TokenStream::from(item);
    // The item already parsed successfully inside `FnModel::parse`
    // once; parsing it here too gives the shim generator the typed
    // item to echo unchanged.
    match syn::parse2::<syn::ItemFn>(item.clone()) {
        Err(err) => err.to_compile_error().into(),
        Ok(func) => match model::FnModel::parse(&attrs, item.clone()) {
            Ok(model_) => shim::expand(&model_, &func).into(),
            Err(err) => err.to_compile_error().into(),
        },
    }
}
