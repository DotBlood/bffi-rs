//! Integration tests for the FFI boundary: the `bffi_extern!` wrapper and
//! the panic-to-error conversion.

// Tests assert invariants and intentionally trigger panics; the workspace
// restriction lints target production code.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::atomic::{AtomicU32, Ordering};

use bffi_core::bffi_extern;
use bffi_core::error::ErrorCode;

static LAST_SUM: AtomicU32 = AtomicU32::new(0);

bffi_extern! {
    /// Adds two numbers and records the sum.
    pub extern "C" fn bffi_sum(a: u32, b: u32) -> ErrorCode {
        LAST_SUM.store(a.wrapping_add(b), Ordering::Relaxed);
        ErrorCode::Ok
    }
}

#[test]
fn wrapped_extern_fn_runs_its_body_and_returns_ok() {
    assert_eq!(bffi_sum(20, 22), ErrorCode::Ok);
    assert_eq!(LAST_SUM.load(Ordering::Relaxed), 42);
}

#[test]
fn extern_bodies_can_signal_failures_through_codes() {
    let code = bffi_core::run_extern_body(|| ErrorCode::InvalidHandle);
    assert_eq!(code, ErrorCode::InvalidHandle);
    assert!(bffi_core::take_last_error().is_none());
}

#[test]
fn run_extern_body_converts_panics_into_codes_and_last_error() {
    let code = bffi_core::run_extern_body(|| panic!("outer boom"));
    assert_eq!(code, ErrorCode::Panic);

    let error = bffi_core::take_last_error().expect("last error must be stored");
    assert_eq!(error.code, ErrorCode::Panic);
    assert_eq!(error.message, "outer boom");
    assert!(
        bffi_core::take_last_error().is_none(),
        "take clears the slot"
    );
}

#[test]
fn last_error_stays_thread_local_across_the_boundary() {
    let code = bffi_core::run_extern_body(|| panic!("main-thread only"));
    assert_eq!(code, ErrorCode::Panic);

    let seen_from_thread = std::thread::spawn(bffi_core::take_last_error)
        .join()
        .expect("thread must not panic");
    assert!(seen_from_thread.is_none(), "other threads see no error");

    let own = bffi_core::take_last_error().expect("owner thread sees the error");
    assert_eq!(own.message, "main-thread only");
}
