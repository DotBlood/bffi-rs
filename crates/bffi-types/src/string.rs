//! UTF-8 string conversion, copy by default (DESIGN §6.3).
//!
//! Byte sequences handed over from the JS side (e.g. a `cstring` argument)
//! are valid only for the duration of the call; [`bytes_to_string`]
//! therefore **copies** into an owned [`String`] after validating UTF-8.
//! The zero-copy counterpart lives behind [`crate::unsafe_zero_copy`].

use bffi_core::{BffiError, ErrorCode};

/// Validates `bytes` as UTF-8 and returns an owned copy.
///
/// Copy by default: the result never aliases `bytes`, so the caller may
/// free or reuse the source buffer immediately after the call.
///
/// # Errors
///
/// [`ErrorCode::InvalidUtf8`] (as [`BffiError`]) when the sequence is not
/// valid UTF-8.
pub fn bytes_to_string(bytes: &[u8]) -> Result<String, BffiError> {
    String::from_utf8(bytes.to_vec())
        .map_err(|_| BffiError::new(ErrorCode::InvalidUtf8, "byte sequence is not valid UTF-8"))
}

/// Copies a `&str` into an owned byte vector.
///
/// The symmetric partner of [`bytes_to_string`]: the result never aliases
/// the input, ready to cross the boundary (e.g. handed to the JS side with
/// transferred ownership).
#[must_use]
pub fn string_to_bytes(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}
