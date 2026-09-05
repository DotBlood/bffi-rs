//! # bffi-object
//!
//! Object ownership for the bffi-rs framework: native bindings for
//! [Bun](https://bun.sh). Rust values live as `Arc<T>` inside the
//! process-wide `Registry` (see `bffi-core`); JavaScript only ever
//! receives the opaque `u64` handle (DESIGN.md §6.2).
//!
//! - [`ObjectWrap`] ties one [`TypeTag`](bffi_core::TypeTag) to one
//!   object type `T` and wraps/gets/releases values by handle.
//! - [`ObjectError`] is the crate's domain error, convertible into
//!   `bffi_core::BffiError` on existing error codes.
//!
//! ## Use-after-free protection
//!
//! Every lookup passes three barriers; any failure yields
//! `ObjectError::InvalidHandle` (kanboard 4.2):
//!
//! 1. the null handle never resolves;
//! 2. the handle's type tag must match the wrap's table (type spoofing);
//! 3. the slot generation must match (a stale handle stays dead after
//!    release, even when the slot is reused).

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod error;

pub use error::ObjectError;
