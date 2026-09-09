//! # bffi-build
//!
//! The runtime C ABI layer of the bffi-rs framework: the JS-facing
//! exports that Bun calls through `bun:ffi` (see `docs/DESIGN.md` §8,
//! HANDOFF §3.6).
//!
//! `#[bffi]` (P1) already generates per-function shims in the user
//! crate. This crate provides everything those shims cannot:
//!
//! - [`dts`] - the build-time half of the `.d.ts` story (kanboard
//!   criterion 6.2): [`dts::write_to_file`] renders a
//!   [`bffi_dts::ModuleDef`] with the deterministic
//!   [`bffi_dts::render`] and writes the bytes to disk in one call
//!   (descriptor aggregation stays the caller's explicit job);
//! - [`loader_json`] - the loader-schema materializer: the same
//!   aggregated [`ModuleDef`] rendered into the canonical JSON the
//!   `bffi` codegen CLI turns into the typed JS loader;
//! - [`runtime`] - safe Rust storage for **transient buffers**
//!   (`String`/`Vec<u8>`/`CopiedBuf` returned to JS) and for **drained
//!   last errors**, both addressed by opaque handles in the
//!   process-wide registry (tags `0x0400`/`0x0401`);
//! - [`bffi_runtime_abi!`] - the declarative generator of the eight
//!   JS-facing exports (`bffi_error_*`, `bffi_buffer`,
//!   `bffi_buffer_length`, `bffi_types_free`), expanded **in the user
//!   crate** so the linker cannot drop them from the cdylib;
//! - [`BuildError`] - the crate's domain error with the unified
//!   `From<BuildError> for BffiError` conversion.
//!
//! ## What does NOT belong here
//!
//! | Concern | Home |
//! |---|---|
//! | per-function `extern "C"` shims for `#[bffi]` fns | `bffi-macros` |
//! | TypeScript descriptors / `.d.ts` rendering | `bffi-dts` (the [`dts`] module here only materializes the rendered bytes to disk) |
//! | zero-copy view policy (`str_view`, `buf_view`) | `bffi-types` |
//! | code -> JS constructor mapping | `bffi-error` |
//! | the cargo/bun build wiring (task of the repo `package.json`) | repo scripts |
//!
//! ## Example
//!
//! The user (cdylib) crate expands the generator once and then works
//! with the safe runtime API:
//!
//! ```
//! use bffi_types::CopiedBuf;
//!
//! // in the cdylib crate root (not in a doctest):
//! // bffi_build::bffi_runtime_abi!();
//!
//! // Returning bytes to JS: store a copy, hand out the handle.
//! let handle = bffi_build::runtime::store_bytes(CopiedBuf::from_slice(b"payload"))
//!     .expect("buffer table has room");
//! assert!(!bffi_build::runtime::buffer_ptr(handle).is_null());
//! assert_eq!(bffi_build::runtime::buffer_len(handle), 7);
//! assert!(bffi_build::runtime::free_buffer(handle));
//! ```
//!
//! The JS side reads the same buffer through the generated exports:
//! `bffi_buffer(handle)` -> pointer, `bffi_buffer_length(handle)` ->
//! length, `bffi_types_free(handle)` -> release. See
//! `CALLING-CONVENTION.md` for the full ABI contract.
//!
//! ## Safety model
//!
//! The crate contains **no** `extern "C"` functions and no `unsafe`:
//! the generated exports are thin forwarders into this safe API, and
//! the build-policy plumbing (debug bare / release
//! `run_extern_body`/`run_extern_body_or`) comes from `bffi-core` in
//! the macro expansion. Pointers handed out by the runtime stay valid
//! until the owning handle is released - the JS-side lifetime contract
//! is documented on [`runtime`].

// The workspace restriction lints (expect/unwrap/panic) target production
// code; tests assert invariants and intentionally trigger panics.
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod abi;
pub mod dts;
pub mod error;
pub mod loader_json;
pub mod runtime;

pub use error::BuildError;
