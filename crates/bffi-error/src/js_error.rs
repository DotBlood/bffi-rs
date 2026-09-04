//! The JavaScript error taxonomy and the data shape shared with the JS side.
//!
//! The future `bffi-build` layer turns a [`JsErrorShape`] into an actual
//! `new TypeError(message)` / `new RangeError(message)` / `new Error(message)`
//! on the JS side; the shape itself is produced here, from Rust data only.

use std::fmt;

/// The JavaScript error constructor a failure must be raised with.
///
/// Mirrors the three relevant built-ins; deliberately `#[non_exhaustive]`
/// so additional kinds (if any are ever justified) do not break matches
/// downstream.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum JsErrorName {
    /// The generic `Error` constructor: internal failures, caught panics,
    /// stale handles, exhausted tables.
    Error,
    /// `TypeError`: the value has the wrong kind or shape (bad UTF-8, null
    /// pointer, contract-violating argument).
    TypeError,
    /// `RangeError`: the value is out of an allowed range (numeric
    /// overflow, output buffer too small).
    RangeError,
}

impl JsErrorName {
    /// Returns the constructor name as it is written in JavaScript.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::TypeError => "TypeError",
            Self::RangeError => "RangeError",
        }
    }
}

impl fmt::Display for JsErrorName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The complete description of a JavaScript error to raise: which
/// constructor and with which message.
///
/// This is the unit of exchange between the Rust side (which produces it
/// from [`bffi_core::BffiError`] data) and the future JS-facing layer
/// (which raises it). `message` is already the final, human-readable text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JsErrorShape {
    /// The error constructor to invoke on the JS side.
    pub name: JsErrorName,
    /// Final message text; passed to the constructor verbatim.
    pub message: String,
}

impl JsErrorShape {
    /// Creates a shape from a name and a message.
    #[must_use]
    pub fn new(name: JsErrorName, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
        }
    }
}

impl fmt::Display for JsErrorShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}
