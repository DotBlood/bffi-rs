//! # bffi-macro-support
//!
//! Shared model, classification, codegen and diagnostics for the
//! proc-macro crates of
//! [bffi-rs](https://github.com/DotBlood/bffi-rs): `bffi-macros`
//! (the `#[bffi]` attribute) and `bffi-class`
//! (`#[bffi_class]`/`#[bffi_impl]`).
//!
//! This is a tooling crate: it holds no runtime code and knows
//! nothing about the C ABI. It exists to remove the duplication
//! between the two proc-macro crates, which would otherwise drift
//! apart. The split of responsibilities:
//!
//! - The proc-macro crates keep their attribute entry points, their
//!   item parsing (`ItemFn` vs `ItemStruct`/`ItemImpl`) and their
//!   diagnostics constructors with the specific E-codes and exact
//!   message texts (`E001`-`E004` in `bffi-macros`, `E005`-`E008` in
//!   `bffi-class`; the trybuild goldens there pin those texts).
//! - This crate keeps everything both crates would otherwise copy:
//!   the boundary kind model ([`kind`]), the `syn::Type`
//!   classification and the TypeScript kind mapping ([`classify`]),
//!   the shim transport token generators ([`codegen`]), the
//!   configurable crate-root paths ([`paths`]), the diagnostic
//!   renderer ([`diagnostics`]) and the small parse helpers
//!   ([`util`]).
//!
//! Classifiers reject neutrally: they return a
//! [`classify::Unsupported`] (span plus the offending type) and each
//! consumer renders its own diagnostic text from it, keeping the
//! published error messages byte-identical.
//!
//! The generated tokens name the runtime crates through a
//! [`paths::PathCtx`]: the default is the absolute direct-dependency
//! paths (`::bffi_core`, `::bffi_types`, `::bffi_dts`,
//! `::bffi_object`, `::bffi_build`); the `crate = "<name>"` option
//! redirects them to `::<name>::{core, types, dts, object, build}`
//! for facade-only mode. The expansion lands in the user crate,
//! which must provide whichever roots are named.

// The workspace restriction lints (expect/unwrap/panic) target production
// code; tests assert invariants and intentionally trigger panics.
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod classify;
pub mod codegen;
pub mod diagnostics;
pub mod kind;
pub mod paths;
pub mod util;
