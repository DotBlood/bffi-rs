//! # bffi-callback
//!
//! Callback plumbing for the [bffi-rs](https://github.com/DotBlood/bffi-rs)
//! framework: explicit registration and revocation of thread-safe callbacks
//! in both directions (Rust -> JavaScript and JavaScript -> Rust).
//!
//! ## Mission (P1-SPEC §6)
//!
//! - Registration is explicit: a callback exists only after the host
//!   registers it, and it is addressed by an opaque `u64` handle.
//! - Revocation is explicit and terminal: a revoked handle is dead -
//!   invocation through it is rejected, and the handle never resurrects
//!   even if its registry slot is reused.
//! - Wrong-thread invocation is rejected: only the thread that owns the
//!   callback may invoke it in P1; marshalling across threads is
//!   deferred to P2 (the event-loop trampoline).
//! - Storage lives in the process-wide `Registry` (see `bffi-core`)
//!   under the type tags `0x0200` (callback table) and `0x0201`
//!   (callback instances).
//! - Panics raised inside a callback body are not this crate's concern:
//!   catching and converting them into JS errors is the P2 trampoline's
//!   job (DESIGN.md §6.5).

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod value;

pub use value::{CallbackSig, Value, ValueType};
