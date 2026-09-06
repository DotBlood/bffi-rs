//! Example native module for bffi-rs: the first real cdylib Bun loads
//! through `bun:ffi`.
//!
//! It exercises the full P1 shim contract plus the P2 runtime exports:
//!
//! - [`add`] - primitives and the `__ret` out-parameter;
//! - [`greet`] / [`greet_len`] - the cstring (`&str`) parameter path;
//! - [`boom`] - the release panic path (debug builds abort by design,
//!   so the e2e suite only ever loads the release artifact);
//! - [`example_echo_buffer`] - a hand-written buffer return, the
//!   reference sketch for `ptr, len` parameters (CALLING-CONVENTION.md
//!   §8);
//! - `bffi_runtime_abi!()` - the eight JS-facing runtime exports.

use std::sync::atomic::{AtomicU32, Ordering};

use bffi::{CopiedBuf, ErrorCode};
use bffi_build::bffi_runtime_abi;
use bffi_class::{bffi_class, bffi_constructor, bffi_impl};
use bffi_macros::bffi;

// The eight JS-facing runtime exports (bffi_error_*, bffi_buffer pair,
// bffi_types_free) generated into this cdylib.
bffi_runtime_abi!();

static LAST_GREET_LEN: AtomicU32 = AtomicU32::new(0);

/// Adds two numbers.
#[bffi]
pub fn add(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

/// Remembers the length of the greeting.
#[bffi]
pub fn greet(name: &str) {
    LAST_GREET_LEN.store(name.len() as u32, Ordering::Relaxed);
}

/// Returns the length of the last greeting.
#[bffi]
pub fn greet_len() -> u32 {
    LAST_GREET_LEN.load(Ordering::Relaxed)
}

/// Returns an uppercased greeting as a buffer handle.
#[bffi]
pub fn shout(name: &str) -> String {
    format!("HELLO {name}!")
}

/// The `E` side of the err channel.
#[derive(Debug)]
pub struct MathError {
    divisor: u32,
}

impl std::fmt::Display for MathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "division by {}", self.divisor)
    }
}

impl std::error::Error for MathError {}

/// Divides, reporting a domain error through the err channel.
#[bffi]
pub fn checked_div(a: u32, b: u32) -> Result<u32, MathError> {
    a.checked_div(b).ok_or(MathError { divisor: b })
}

/// A native class: exercised end-to-end through bun:ffi.
#[bffi_class(tag = 0x015A)]
/// A native counter.
pub struct Counter {
    /// The current value.
    pub value: u32,
}

#[bffi_impl]
impl Counter {
    #[bffi_constructor]
    /// Creates a counter.
    pub fn new(start: u32) -> Self {
        Self { value: start }
    }

    /// Adds one and returns the new value.
    pub fn increment(&self) -> u32 {
        self.value + 1
    }
}

/// Always panics: the release build must convert this into
/// `ErrorCode::Panic` plus a last error, not abort Bun.
#[bffi]
#[allow(clippy::panic)]
pub fn boom() -> u32 {
    panic!("boom")
}

/// Copies `ptr[0..len]` into the runtime buffer table and writes the
/// resulting handle into `__ret`.
///
/// Hand-written sketch of the future `ptr, len` parameter convention
/// (CALLING-CONVENTION.md §8): the caller passes a live
/// `TypedArray`/`ArrayBuffer` pointer valid for the duration of the
/// call.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes for the duration of the
/// call; `__ret` must be null or a valid `*mut u64`. These are the
/// caller's obligations under the bun:ffi pointer contract.
#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn example_echo_buffer(ptr: *const u8, len: u64, __ret: *mut u64) -> ErrorCode {
    #[cfg(debug_assertions)]
    {
        echo_buffer_body(ptr, len, __ret)
    }
    #[cfg(not(debug_assertions))]
    {
        bffi_core::boundary::run_extern_body(|| echo_buffer_body(ptr, len, __ret))
    }
}

fn echo_buffer_body(ptr: *const u8, len: u64, __ret: *mut u64) -> ErrorCode {
    if __ret.is_null() {
        let error =
            bffi_core::BffiError::new(bffi_core::ErrorCode::NullPointer, "output pointer is null");
        bffi_core::set_last_error(error);
        return bffi_core::ErrorCode::NullPointer;
    }
    if len > 0 && ptr.is_null() {
        let error =
            bffi_core::BffiError::new(bffi_core::ErrorCode::NullPointer, "buffer pointer is null");
        bffi_core::set_last_error(error);
        return bffi_core::ErrorCode::NullPointer;
    }
    // SAFETY: the caller contract (this function's doc comment)
    // guarantees `ptr` points to `len` readable bytes for the duration
    // of the call; `len == 0` short-circuits the pointer use above.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let view = bffi_types::buf_view(bytes);
    match bffi_build::runtime::store_bytes(CopiedBuf::from_slice(view.as_slice())) {
        Ok(handle) => {
            // SAFETY: `__ret` is non-null (checked above) and valid for
            // one `u64` write per the bun:ffi out-parameter contract.
            unsafe { ::std::ptr::write(__ret, handle.as_u64()) };
            ErrorCode::Ok
        }
        Err(error) => {
            bffi_core::set_last_error(error.into());
            ErrorCode::TableFull
        }
    }
}
