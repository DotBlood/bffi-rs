//! Buffer conversion policy types: owned copies by default, explicit
//! borrowing for the zero-copy path (DESIGN §6.3).

use crate::unsafe_zero_copy::ZeroCopyBuf;
use std::ops::Deref;

/// An owned, heap-allocated byte buffer - the default buffer type.
///
/// Constructing a `CopiedBuf` always copies (or takes ownership of an
/// existing `Vec`); it never aliases a caller's memory. This is the type
/// every non-zero-copy buffer path produces and consumes.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CopiedBuf(Vec<u8>);

impl CopiedBuf {
    /// Copies `bytes` into a new buffer.
    #[must_use]
    pub fn from_slice(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    /// Takes ownership of an existing `Vec<u8>` (no copy).
    #[must_use]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self(vec)
    }

    /// The contained bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Unwraps into the owned `Vec<u8>` (e.g. to transfer ownership to
    /// the JS side through the `bffi-build` allocation contract).
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl Deref for CopiedBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&[u8]> for CopiedBuf {
    fn from(bytes: &[u8]) -> Self {
        Self::from_slice(bytes)
    }
}

impl From<Vec<u8>> for CopiedBuf {
    fn from(vec: Vec<u8>) -> Self {
        Self::from_vec(vec)
    }
}

/// Explicit copy out of a zero-copy view: the one place where borrowed
/// buffer data becomes owned. Writing `CopiedBuf::from(&view)` makes the
/// copying step visible at the call site, per DESIGN §6.3.
impl From<&ZeroCopyBuf<'_>> for CopiedBuf {
    fn from(view: &ZeroCopyBuf<'_>) -> Self {
        Self(view.as_slice().to_vec())
    }
}
