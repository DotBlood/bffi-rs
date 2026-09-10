//! UTF-16/UTF-32 string conversion utilities (copy by default).
//!
//! These are UTILITIES for consumers of `bffi-types`: the bffi
//! boundary itself stays UTF-8 canonical (DESIGN §6.3/§7 - `bun:ffi`
//! cstrings and JS strings convert automatically). Nothing here is
//! wired into the ABI.
//!
//! Decoding is STRICT by default (an unpaired surrogate or a
//! non-scalar `u32` is an error, not U+FFFD); every decoder has a
//! `_lossy` twin that substitutes U+FFFD instead of failing.

// Internal module aliases (the pre-merge crate names).
use crate::bffi_core;
use bffi_core::{BffiError, ErrorCode};

fn invalid_utf16() -> BffiError {
    BffiError::new(
        ErrorCode::InvalidUtf8,
        "code unit sequence is not valid UTF-16 (unpaired surrogate)",
    )
}

fn invalid_utf32() -> BffiError {
    BffiError::new(
        ErrorCode::InvalidUtf8,
        "value sequence contains a value that is not a Unicode scalar",
    )
}

/// Copies a `&str` into UTF-16 code units (native endianness,
/// surrogate pairs for astral characters).
#[must_use]
pub fn string_to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// Validates UTF-16 code units and decodes them into an owned
/// [`String`]. An unpaired surrogate is an error (use
/// [`utf16_to_string_lossy`] for U+FFFD substitution).
///
/// # Errors
///
/// [`ErrorCode::InvalidUtf8`] (as [`BffiError`]) when the sequence
/// contains an unpaired surrogate.
pub fn utf16_to_string(units: &[u16]) -> Result<String, BffiError> {
    String::from_utf16(units).map_err(|_| invalid_utf16())
}

/// Lossy UTF-16 decode: unpaired surrogates become U+FFFD.
#[must_use]
pub fn utf16_to_string_lossy(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// Copies a `&str` into UTF-32 code units (one `u32` Unicode scalar
/// per character, native endianness).
#[must_use]
pub fn string_to_utf32(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

/// Validates UTF-32 code units (each value must be a Unicode scalar)
/// and decodes them into an owned [`String`].
///
/// # Errors
///
/// [`ErrorCode::InvalidUtf8`] (as [`BffiError`]) when any value is
/// not a Unicode scalar value (surrogates 0xD800..=0xDFFF or values
/// above 0x10FFFF).
pub fn utf32_to_string(units: &[u32]) -> Result<String, BffiError> {
    let mut out = String::with_capacity(units.len());
    for &unit in units {
        let ch = char::from_u32(unit).ok_or_else(invalid_utf32)?;
        out.push(ch);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{
        invalid_utf16, string_to_utf16, string_to_utf32, utf16_to_string, utf16_to_string_lossy,
        utf32_to_string,
    };
    use crate::bffi_core::ErrorCode;

    #[test]
    fn utf16_round_trips_ascii_bmp_and_astral() {
        for text in ["", "ascii", "кириллица", "🚀 mixed 🌍"] {
            let units = string_to_utf16(text);
            assert_eq!(utf16_to_string(&units).unwrap(), text);
            assert_eq!(utf16_to_string_lossy(&units), text);
        }
    }

    #[test]
    fn astral_chars_encode_as_surrogate_pairs() {
        // U+1F680 rocket = surrogate pair D83D DE80.
        assert_eq!(string_to_utf16("🚀"), vec![0xD83D, 0xDE80]);
    }

    #[test]
    fn utf16_strict_decode_rejects_unpaired_surrogates() {
        // Lone high surrogate D83D.
        let error = utf16_to_string(&[0xD83D]).expect_err("unpaired surrogate");
        assert_eq!(error.code, ErrorCode::InvalidUtf8);
        assert!(error.message.contains("unpaired surrogate"));
        assert_eq!(
            utf16_to_string(&[0xD83D, 0x0061])
                .expect_err("unpaired + char")
                .code,
            ErrorCode::InvalidUtf8
        );
        let _ = invalid_utf16();
    }

    #[test]
    fn utf16_lossy_substitutes_fffd() {
        assert_eq!(utf16_to_string_lossy(&[0xD83D, 0x0061]), "\u{FFFD}a");
    }

    #[test]
    fn utf32_round_trips_scalars() {
        for text in ["", "ascii", "кириллица", "🚀 mixed 🌍"] {
            let units = string_to_utf32(text);
            assert_eq!(utf32_to_string(&units).unwrap(), text);
        }
        // One scalar per char, no surrogate pairs.
        assert_eq!(string_to_utf32("🚀"), vec![0x1F680]);
    }

    #[test]
    fn utf32_rejects_surrogates_and_out_of_range() {
        assert_eq!(
            utf32_to_string(&[0xD800]).expect_err("surrogate").code,
            ErrorCode::InvalidUtf8
        );
        assert_eq!(
            utf32_to_string(&[0x110000]).expect_err("above max").code,
            ErrorCode::InvalidUtf8
        );
    }
}
