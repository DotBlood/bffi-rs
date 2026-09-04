//! # bffi-error
//!
//! Unified mapping of bffi errors to JavaScript error shapes.
//!
//! The transport of a failure across the C ABI lives in `bffi-core`
//! ([`ErrorCode`] return values plus the thread-local *last error*). This
//! crate adds the missing piece: **which kind of JavaScript error** a
//! failure must become and **what message** it carries.
//!
//! The taxonomy is deliberately small — JavaScript has exactly three error
//! constructors worth targeting:
//!
//! - [`JsErrorName::TypeError`] — the *value* has the wrong kind or shape
//!   (bad UTF-8, null pointer, contract-violating argument);
//! - [`JsErrorName::RangeError`] — the value is out of an allowed range
//!   (numeric overflow, output buffer too small);
//! - [`JsErrorName::Error`] — everything else (generic failures, caught
//!   panics, stale handles, exhausted tables).
//!
//! JS-side instantiation (constructing the actual `Error` objects) and the
//! `extern "C"` exports that expose this mapping are the job of
//! `bffi-build` / the public facade (P2); this crate is a pure-Rust policy
//! library and contains no FFI surface.
//!
//! ## Example
//!
//! ```
//! use bffi_core::{BffiError, ErrorCode};
//! use bffi_error::{JsErrorExt, JsErrorName};
//!
//! let error = BffiError::new(ErrorCode::NumberOutOfRange, "3e9 does not fit in i32");
//! let shape = error.to_js_shape();
//!
//! assert_eq!(shape.name, JsErrorName::RangeError);
//! assert_eq!(shape.message, "3e9 does not fit in i32");
//! ```

pub mod js_error;
pub mod map;

pub use js_error::{JsErrorName, JsErrorShape};
pub use map::{JsErrorExt, js_error_name, take_last_error_shape};
