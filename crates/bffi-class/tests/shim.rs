//! Acceptance tests for the generated class shims: full lifecycle
//! (create -> getter -> method -> release) plus UAF barriers, in the
//! `#[bffi]` shim testing model (direct Rust calls, no Bun).
//!
//! Test tags (0x0150/0x0151) are unique per class: one binary shares
//! the process-wide Registry.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_core::{ErrorCode, Handle, take_last_error};

#[bffi_class::bffi_class(tag = 0x0150)]
/// A counter.
pub struct Counter {
    /// The current value.
    pub value: u32,
    /// Private fields are never exported (kept unreachable here).
    #[allow(dead_code)]
    secret: u8,
}

#[bffi_class::bffi_impl]
impl Counter {
    #[bffi_class::bffi_constructor]
    /// Creates a counter.
    pub fn new(start: u32) -> Self {
        Self {
            value: start,
            secret: 0,
        }
    }

    /// Adds one and returns the new value.
    pub fn increment(&self) -> u32 {
        self.value + 1
    }

    /// Scales by a factor.
    pub fn scaled(&self, factor: u32) -> u32 {
        self.value * factor
    }

    /// Greets, returning an owned string.
    pub fn greet(&self, prefix: &str) -> String {
        format!("{prefix}-{}", self.value)
    }

    /// Falls back to the err channel on zero.
    pub fn divided(&self, divisor: u32) -> Result<u32, DivError> {
        self.value.checked_div(divisor).ok_or(DivError { divisor })
    }
}

/// The `E` side of the err channel.
#[derive(Debug)]
pub struct DivError {
    divisor: u32,
}

impl std::fmt::Display for DivError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by {divisor}", divisor = self.divisor)
    }
}

impl std::error::Error for DivError {}

/// A second class for wrong-tag tests (its own tag, own table).
#[bffi_class::bffi_class(tag = 0x0151)]
/// A gate.
pub struct Gate {
    /// Open or closed.
    pub open: bool,
}

#[bffi_class::bffi_impl]
impl Gate {
    #[bffi_class::bffi_constructor]
    /// Creates a gate.
    pub fn new(open: bool) -> Self {
        Self { open }
    }
}

fn cstring(bytes: &[u8]) -> *const std::os::raw::c_char {
    std::ffi::CString::new(bytes)
        .expect("test bytes contain no interior NUL")
        .into_raw()
        .cast_const()
}

#[test]
fn constructor_getter_method_release_roundtrip() {
    let mut handle = 0_u64;
    assert_eq!(bffi_counter_new(41, &mut handle), ErrorCode::Ok);
    assert_ne!(handle, 0);

    let mut value = 0_u32;
    assert_eq!(bffi_counter_value_get(handle, &mut value), ErrorCode::Ok);
    assert_eq!(value, 41);

    let mut out = 0_u32;
    assert_eq!(bffi_counter_increment(handle, &mut out), ErrorCode::Ok);
    assert_eq!(out, 42);

    assert_eq!(bffi_counter_scaled(handle, 3, &mut out), ErrorCode::Ok);
    assert_eq!(out, 123);

    // The destructor frees the slot: the handle goes stale.
    assert_eq!(bffi_counter_release(handle), ErrorCode::Ok);
    assert_eq!(bffi_counter_release(handle), ErrorCode::InvalidHandle);
    assert_eq!(
        bffi_counter_value_get(handle, &mut value),
        ErrorCode::InvalidHandle
    );
    let error = take_last_error().expect("the stale getter must store an error");
    assert_eq!(error.code, ErrorCode::InvalidHandle);
}

#[test]
fn string_and_result_method_returns_travel_the_p2_channels() {
    let mut handle = 0_u64;
    assert_eq!(bffi_counter_new(7, &mut handle), ErrorCode::Ok);

    let mut out_handle = 0_u64;
    assert_eq!(
        bffi_counter_greet(handle, cstring(b"val"), &mut out_handle),
        ErrorCode::Ok
    );
    // SAFETY: `buffer_ptr` handed out the pointer to exactly
    // `buffer_len(handle)` owned bytes; the handle is still live.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            bffi_build::runtime::buffer_ptr(bffi_core::Handle::from_raw(out_handle)),
            bffi_build::runtime::buffer_len(bffi_core::Handle::from_raw(out_handle)) as usize,
        )
    };
    assert_eq!(bytes, b"val-7");
    assert!(bffi_build::runtime::free_buffer(
        bffi_core::Handle::from_raw(out_handle)
    ));

    let mut out = 0_u32;
    assert_eq!(bffi_counter_divided(handle, 3, &mut out), ErrorCode::Ok);
    assert_eq!(out, 2);
    assert_eq!(
        bffi_counter_divided(handle, 0, &mut out),
        ErrorCode::DomainError
    );
    let error = take_last_error().expect("an Err must store the domain error");
    assert_eq!(error.message, "division by 0");

    bffi_counter_release(handle);
}

#[test]
fn invalid_and_foreign_handles_are_rejected_with_clear_errors() {
    // Null handle.
    let mut value = 0_u32;
    assert_eq!(
        bffi_counter_value_get(0, &mut value),
        ErrorCode::InvalidHandle
    );
    let error = take_last_error().expect("an invalid handle must store an error");
    assert_eq!(error.code, ErrorCode::InvalidHandle);

    // Live Gate handle used against Counter: the tag barrier rejects.
    let mut gate = 0_u64;
    assert_eq!(bffi_gate_new(true, &mut gate), ErrorCode::Ok);
    assert_eq!(
        bffi_counter_value_get(gate, &mut value),
        ErrorCode::InvalidHandle
    );

    // Forged generation on a real Counter handle.
    let mut counter = 0_u64;
    assert_eq!(bffi_counter_new(1, &mut counter), ErrorCode::Ok);
    let live = Handle::from_raw(counter);
    let forged = Handle::new(live.tag(), live.generation() + 1, live.index());
    assert_eq!(
        bffi_counter_value_get(forged.as_u64(), &mut value),
        ErrorCode::InvalidHandle
    );

    bffi_gate_release(gate);
    bffi_counter_release(counter);
}

#[test]
fn constructor_null_out_pointer_is_rejected() {
    let code = bffi_counter_new(1, std::ptr::null_mut());
    assert_eq!(code, ErrorCode::NullPointer);
    let error = take_last_error().expect("null out-pointer must store a last error");
    assert_eq!(error.code, ErrorCode::NullPointer);
}
