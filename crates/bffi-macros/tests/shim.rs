//! Acceptance tests for the generated `extern "C"` shims
//! (kanboard 7.1): every shim validates its pointers, copies `&str`
//! inputs through the cstring path, and transports results via
//! out-parameters under the boundary policy.
//!
//! The shims are safe functions: pointer *parameters* do not make an
//! `fn` unsafe, and every dereference inside the shim is guarded by
//! the shim's own checks.

#![allow(clippy::expect_used, clippy::unwrap_used)]
// The generated `pub extern "C"` shims carry no doc comments.
#![allow(missing_docs)]

use bffi_core::{ErrorCode, take_last_error};

#[bffi_macros::bffi]
/// Adds two numbers.
fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[bffi_macros::bffi]
fn widen(x: i32) -> i64 {
    x as i64
}

#[bffi_macros::bffi]
fn flip(x: u64) -> bool {
    x == 0
}

#[bffi_macros::bffi]
fn shout(phrase: &str) -> u32 {
    phrase.len() as u32
}

#[bffi_macros::bffi]
/// Touches nothing.
fn touch(x: u32) {
    let _ = x;
}

/// Converts `bytes` into a NUL-terminated cstring pointer for the
/// generated `&str` shim parameters. `into_raw` leaks on purpose:
/// test allocations are never reclaimed.
fn cstring(bytes: &[u8]) -> *const std::os::raw::c_char {
    std::ffi::CString::new(bytes)
        .expect("test bytes contain no interior NUL")
        .into_raw()
        .cast_const()
}

#[test]
fn shim_writes_out_param_and_returns_ok() {
    let mut out = 0_u32;
    let code = bffi_add(1, 2, &mut out);
    assert_eq!(code, ErrorCode::Ok);
    assert_eq!(out, 3);
    assert!(
        take_last_error().is_none(),
        "success must not store a last error"
    );
}

#[test]
fn shim_rejects_null_out_pointer() {
    let code = bffi_add(1, 2, std::ptr::null_mut());
    assert_eq!(code, ErrorCode::NullPointer);
    let error = take_last_error().expect("null out-pointer must store a last error");
    assert_eq!(error.code, ErrorCode::NullPointer);
}

#[test]
fn shim_handles_bigint_paths() {
    let mut wide = 0_i64;
    assert_eq!(bffi_widen(-5, &mut wide), ErrorCode::Ok);
    assert_eq!(wide, -5);

    let mut flag = false;
    assert_eq!(bffi_flip(0, &mut flag), ErrorCode::Ok);
    assert!(flag);
}

#[test]
fn shim_converts_cstrings_and_rejects_invalid_utf8() {
    let mut len = 0_u32;
    let hello = cstring(b"hello");
    assert_eq!(bffi_shout(hello, &mut len), ErrorCode::Ok);
    assert_eq!(len, 5);

    let invalid = cstring(&[0xFF_u8]);
    assert_eq!(bffi_shout(invalid, &mut len), ErrorCode::InvalidUtf8);
    let error = take_last_error().expect("invalid UTF-8 must store a last error");
    assert_eq!(error.code, ErrorCode::InvalidUtf8);
}

#[test]
fn shim_without_return_has_no_out_parameter() {
    // Two arguments only: the signature itself proves there is no
    // out-parameter.
    assert_eq!(bffi_touch(7), ErrorCode::Ok);
}
