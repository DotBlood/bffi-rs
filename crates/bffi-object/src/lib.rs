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
//!
//! ## Quick start
//!
//! ```
//! use std::sync::OnceLock;
//!
//! use bffi_core::TypeTag;
//! use bffi_object::ObjectWrap;
//!
//! struct Counter {
//!     n: u32,
//! }
//!
//! // One tag per type per process; claim it once in a static.
//! static COUNTERS: OnceLock<ObjectWrap<Counter>> = OnceLock::new();
//! const COUNTER: TypeTag = TypeTag(0x0142);
//!
//! let wrap = COUNTERS.get_or_init(|| ObjectWrap::new(COUNTER).expect("tag 0x0142 free"));
//! let handle = wrap.wrap(Counter { n: 7 }).expect("room");
//! assert_eq!(wrap.get(handle).expect("live").n, 7);
//! let released = wrap.release(handle).expect("live handle");
//! assert_eq!(released.n, 7);          // the Arc outlives the slot
//! assert!(wrap.get(handle).is_err()); // the handle is stale now
//! ```

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod error;
pub mod wrap;

pub use error::ObjectError;
pub use wrap::{ObjectWrap, TAG_MAX, TAG_MIN, tag_in_range};
