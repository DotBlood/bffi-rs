//! **Unsafe-by-name zero-copy views - read every word of this module.**
//!
//! Everything else in this crate copies (DESIGN §6.3). Zero-copy is only
//! allowed through this module, and the module name is the warning label:
//! a value produced here borrows the caller's buffer instead of owning a
//! copy, which is exactly the dangerous direction.
//!
//! # Contracts
//!
//! A `ZeroCopyStr` / `ZeroCopyBuf` view:
//!
//! 1. borrows memory the *caller* controls - it must not outlive the FFI
//!    call that produced the buffer. The `'a` lifetime enforces this at
//!    compile time as long as the raw pointer is only dereferenced inside
//!    the boundary layer (`bffi-build`) for the duration of the call;
//! 2. aliases memory that JS may legally keep mutating between calls -
//!    never store a view in Rust state, never spawn a thread with it;
//! 3. still validates UTF-8 for [`str_view`] - zero-copy means *no copy*,
//!    never *no checks*.
//!
//! The constructors here are safe because they take a borrowed slice; the
//! genuinely unsafe step (turning a raw `(ptr, len)` pair from the ABI
//! into a `&[u8]`) lives in `bffi-build`, immediately above this module.

use bffi_core::{BffiError, ErrorCode};
use std::ops::Deref;

/// A borrowed `&str` view over caller-owned UTF-8 bytes - no copy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ZeroCopyStr<'a>(&'a str);

/// A borrowed `&[u8]` view over caller-owned bytes - no copy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ZeroCopyBuf<'a>(&'a [u8]);

/// Borrows `bytes` as a UTF-8 string view without copying.
///
/// # Errors
///
/// [`ErrorCode::InvalidUtf8`] (as [`BffiError`]) when the bytes are not
/// valid UTF-8 - the check is mandatory in the zero-copy path too.
pub fn str_view(bytes: &[u8]) -> Result<ZeroCopyStr<'_>, BffiError> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(ZeroCopyStr(text)),
        Err(_) => Err(BffiError::new(
            ErrorCode::InvalidUtf8,
            "byte sequence is not valid UTF-8",
        )),
    }
}

/// Borrows `bytes` as a byte view without copying (infallible).
#[must_use]
pub fn buf_view(bytes: &[u8]) -> ZeroCopyBuf<'_> {
    ZeroCopyBuf(bytes)
}

impl ZeroCopyStr<'_> {
    /// The borrowed string.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0
    }
}

impl Deref for ZeroCopyStr<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl ZeroCopyBuf<'_> {
    /// The borrowed bytes.
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        self.0
    }
}

impl Deref for ZeroCopyBuf<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
