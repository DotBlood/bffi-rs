//! # bffi-class
//!
//! Class declaration macros of the bffi-rs framework: declare a Rust
//! struct once, get the JS-facing constructor/method/release shims
//! plus a const [`::bffi_dts::ClassDef`] descriptor (kanboard P2:
//! "макрос класса: поля, методы, деструктор"; integration with
//! `bffi-object` and `bffi-dts`).
//!
//! - `#[bffi_class(tag = 0x01xx)]` on a named struct claims the tag
//!   through the `bffi-object` `ObjectWrap` (0x0100-0x01FF), emits the
//!   `bffi_<name>_release` export (the destructor), read-only getters
//!   for `pub` primitive fields, and the `bffi_meta_<name>::{TAG,
//!   DOCS, FIELDS}` metadata;
//! - `#[bffi_impl]` on the `impl` block emits the constructor and
//!   `&self`-method shims plus the `bffi_meta_<name>_impl::CLASS`
//!   descriptor;
//! - `#[bffi_constructor]` marks the (single) constructor inside the
//!   impl block.
//!
//! The two expansions cannot write into one module, so the metadata
//! is split; see the README for the aggregation pattern.
//!
//! Both `#[bffi_class]` and `#[bffi_impl]` accept one optional
//! `crate = "<name>"` option (facade-only mode): the generated paths
//! then resolve through `::<name>::{core, types, dts, object, build}`
//! instead of the direct dependencies. Use the same value on both
//! macros of one class.
//!
//! ```ignore
//! // documentation-only snippet: attribute macros cannot run in doctests
//! #[bffi_class(tag = 0x0142)]
//! /// A counter.
//! pub struct Counter { pub value: u32 }
//!
//! #[bffi_impl]
//! impl Counter {
//!     #[bffi_constructor]
//!     /// Creates a counter.
//!     pub fn new(start: u32) -> Self { Self { value: start } }
//!     /// Adds one.
//!     pub fn increment(&self) -> u32 { self.value + 1 }
//! }
//! ```

// The workspace restriction lints (expect/unwrap/panic) target production
// code; tests assert invariants and intentionally trigger panics.
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

mod errors;
mod mapping;
mod meta;
mod model;
mod shim;

use quote::quote;

/// Declares a native class over a named struct.
///
/// Syntax: `#[bffi_class(tag = 0x01xx)]` with a literal tag in the
/// bffi-object range (`0x0100..=0x01FF`); one tag = one type per
/// process. An optional `crate = "<name>"` switches the generated
/// paths to the facade namespaces (facade-only mode); no other
/// option is accepted. Only `pub` primitive-typed fields are
/// exported, as read-only getters (`Arc<T>` ownership has no safe
/// setter).
///
/// Expands to: the struct unchanged, the `ObjectWrap` accessor, the
/// `bffi_<name>_release` destructor export, the field getters, and the
/// `bffi_meta_<name>` metadata module.
pub(crate) fn bffi_class(
    attrs: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let item2 = item.clone();
    match model::ClassModel::parse(&attrs, item) {
        Ok(model) => {
            let shims = shim::class_shims(&model);
            let meta = meta::class_meta(&model);
            quote! { #item2 #shims #meta }
        }
        Err(err) => err.to_compile_error(),
    }
}

/// Marks the `impl` block of a `#[bffi_class]` struct: generates the
/// constructor and `&self`-method shims plus the
/// `bffi_meta_<name>_impl::CLASS` descriptor.
///
/// Exactly one `#[bffi_constructor] pub fn new(...) -> Self` is
/// required; every other `fn` must take `&self` (methods with `&mut
/// self`/`self` cannot be served through the `Arc<T>` ownership
/// model). An optional `crate = "<name>"` switches the generated
/// paths to the facade namespaces (facade-only mode) - use the same
/// value as on the matching `#[bffi_class]`.
pub(crate) fn bffi_impl(
    attrs: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let item2 = item.clone();
    match model::ImplModel::parse(&attrs, item) {
        Ok(model) => {
            let shims = shim::impl_shims(&model);
            let meta = meta::impl_meta(&model);
            quote! { #item2 #shims #meta }
        }
        Err(err) => err.to_compile_error(),
    }
}

/// Marks the constructor inside a `#[bffi_impl]` block. A pure marker:
/// the impl macro reads the attribute; this macro echoes the item
/// unchanged so the annotation can also stand alone.
pub(crate) fn bffi_constructor(
    _attrs: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    item
}
