//! Integration tests for the error-to-JS mapping and the last-error drain.

// Tests assert invariants; the workspace restriction lints target
// production code.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use bffi_core::{BffiError, ErrorCode, catch_panic, set_last_error};
use bffi_error::{JsErrorExt, JsErrorName, JsErrorShape, take_last_error_shape};

#[test]
fn drained_shape_reflects_code_and_message() {
    set_last_error(BffiError::new(
        ErrorCode::NumberOutOfRange,
        "3e9 does not fit in i32",
    ));

    let shape = take_last_error_shape().expect("an error was stored");
    assert_eq!(shape.name, JsErrorName::RangeError);
    assert_eq!(shape.message, "3e9 does not fit in i32");

    assert!(
        take_last_error_shape().is_none(),
        "drain must clear the slot"
    );
}

#[test]
fn empty_slot_drains_to_none() {
    assert!(take_last_error_shape().is_none());
}

#[test]
fn panic_caught_at_the_boundary_maps_to_generic_error() {
    // The FFI boundary stores caught panics as last errors; they must
    // surface as a plain Error with the panic message.
    let outcome = catch_panic(|| panic!("boom"));
    let error = outcome.unwrap_err();
    assert_eq!(error.code, ErrorCode::Panic);

    set_last_error(error);
    let shape = take_last_error_shape().expect("panic error was stored");
    assert_eq!(shape.name, JsErrorName::Error);
    assert_eq!(shape.message, "boom");
}

#[test]
fn display_is_constructor_plus_message() {
    let shape = JsErrorShape::new(JsErrorName::TypeError, "not valid UTF-8");
    assert_eq!(shape.to_string(), "TypeError: not valid UTF-8");
}

#[test]
fn shapes_compare_by_value() {
    let a = BffiError::new(ErrorCode::NullPointer, "out was null").to_js_shape();
    let b = BffiError::new(ErrorCode::NullPointer, "out was null").to_js_shape();
    assert_eq!(a, b);

    let c = BffiError::new(ErrorCode::NullPointer, "different").to_js_shape();
    assert_ne!(a, c);
}
