//! Integration tests for UTF-8 string conversion (copy by default) and
//! the zero-copy views.

// Tests assert invariants; the workspace restriction lints target
// production code.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_core::ErrorCode;
use bffi_types::unsafe_zero_copy::{buf_view, str_view};
use bffi_types::{bytes_to_string, string_to_bytes};

#[test]
fn valid_utf8_roundtrips_through_a_copy() {
    let ascii = b"plain ascii";
    let text = bytes_to_string(ascii).expect("ascii is valid utf-8");
    assert_eq!(text, "plain ascii");

    let multibyte = "привет 🚀 — ünïcödé".as_bytes();
    let text = bytes_to_string(multibyte).expect("multibyte is valid utf-8");
    assert_eq!(text, "привет 🚀 — ünïcödé");

    // embedded NUL is legal inside a length-prefixed string
    let with_nul = b"a\x00b";
    assert_eq!(bytes_to_string(with_nul).as_deref(), Ok("a\0b"));

    assert_eq!(bytes_to_string(b"").as_deref(), Ok(""));
}

#[test]
fn every_invalid_utf8_category_is_rejected() {
    let invalid: &[&[u8]] = &[
        b"\xFF",                   // invalid byte
        &[0x80],                   // lone continuation byte
        &[0xC3],                   // truncated two-byte sequence
        &[0xE2, 0x82],             // truncated three-byte sequence
        &[0xF0, 0x9F, 0x9A],       // truncated four-byte sequence
        &[0xED, 0xA0, 0x80],       // UTF-8-encoded surrogate (CESU-8)
        &[0xC0, 0x80],             // overlong encoding of NUL
        &[0xF5, 0x80, 0x80, 0x80], // out-of-range lead byte
    ];
    for bytes in invalid {
        let error = bytes_to_string(bytes).expect_err("must be rejected");
        assert_eq!(error.code, ErrorCode::InvalidUtf8, "for {bytes:?}");
    }
}

#[test]
fn copies_do_not_alias_their_source() {
    let mut source = vec![104_u8, 105]; // "hi"
    let text = bytes_to_string(&source).expect("valid utf-8");
    source[0] = 88; // "Xi"
    assert_eq!(text, "hi", "the string must own its bytes");

    let bytes = string_to_bytes("абв");
    assert_eq!(bytes.len(), 6, "two bytes per Cyrillic letter");
    assert_eq!(bytes, "абв".as_bytes());
}

#[test]
fn zero_copy_str_view_validates_and_borrows() {
    let source = "🚀 zero-copy".as_bytes().to_vec();
    let view = str_view(&source).expect("valid utf-8");
    assert_eq!(view.as_str(), "🚀 zero-copy");
    assert_eq!(view.len(), source.len(), "deref exposes the same bytes");

    let invalid = vec![0xFF_u8];
    let error = str_view(&invalid).expect_err("invalid utf-8 is rejected");
    assert_eq!(error.code, ErrorCode::InvalidUtf8);
}

#[test]
fn zero_copy_buf_view_borrows_infallibly() {
    let source = vec![9_u8, 8, 7];
    let view = buf_view(&source);
    assert_eq!(view.as_slice(), &[9, 8, 7]);
    assert_eq!(view.len(), 3);
    assert!(!view.is_empty());

    assert!(buf_view(&[]).is_empty(), "empty input borrows fine");
}
