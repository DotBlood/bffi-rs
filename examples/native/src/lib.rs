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
//! - the verification exports (`mirror_*`, `is_even`,
//!   `example_callback_*`, `example_js_callback_*`,
//!   `example_set_js_thread`, `example_loop_*`,
//!   `example_last_invoked`) - the P1 surface exercised with real
//!   bun:ffi by `js/callbacks.test.ts` and `js/numbers.test.ts`;
//! - `bffi_runtime_abi!()` - the eight JS-facing runtime exports.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use bffi::{CopiedBuf, ErrorCode};
use bffi_build::bffi_runtime_abi;
use bffi_callback::{
    CallbackSig, Value, ValueType, bind_js_callback, invoke, js_callback, register, revoke,
    set_js_thread,
};
use bffi_class::{bffi_class, bffi_constructor, bffi_impl};
use bffi_core::{BffiError, Handle, bffi_extern, set_last_error};
use bffi_event_loop::{Job, executed_total, marshal, pending, pump, run, stop};
use bffi_macros::bffi;

// The eight JS-facing runtime exports (bffi_error_*, bffi_buffer pair,
// bffi_types_free) generated into this cdylib.
bffi_runtime_abi!();

static LAST_GREET_LEN: AtomicU32 = AtomicU32::new(0);

/// The value stored by the last executed marshal job (read through
/// `example_last_invoked`; criterion 5.3).
static LAST_INVOKED: AtomicI32 = AtomicI32::new(0);

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

/// Verification export (P1 type matrix): mirrors an `i64` through the
/// bigint parameter and out-parameter paths.
#[bffi]
pub fn mirror_i64(x: i64) -> i64 {
    x
}

/// Verification export (P1 type matrix): mirrors a `u64` across the
/// full unsigned range, including values beyond
/// `Number.MAX_SAFE_INTEGER`.
#[bffi]
pub fn mirror_u64(x: u64) -> u64 {
    x
}

/// Verification export (P1 type matrix): reports the parity of a
/// `u64` through the bool parameter and return paths.
#[bffi]
pub fn is_even(x: u64) -> bool {
    x.is_multiple_of(2)
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

// ---------------------------------------------------------------------------
// P1-E2E verification exports (kanboard 5.1, 5.2, 5.3)
//
// Every export below is a hand-written verification export: its doc
// comment names the criterion it exercises. All of them
//
// - run the body through `bffi_core::bffi_extern!` (debug: bare;
//   release: a panic becomes `ErrorCode::Panic` plus a last error);
// - return `ErrorCode` on the C ABI and transport results through a
//   trailing `__ret` out-parameter (the generated-shim convention);
// - map callback-layer failures through `BffiError::from`:
//   SignatureMismatch -> InvalidArgument (11), dead handles ->
//   InvalidHandle (4), wrong-thread calls -> WrongThread (12).
// ---------------------------------------------------------------------------

/// The fixed signature of the verification callbacks: `i32(i32)`.
fn verification_sig() -> CallbackSig {
    CallbackSig::new(ValueType::I32, &[ValueType::I32])
}

/// The fixed verification body: doubles its single `i32` argument.
/// `invoke` checks the signature before calling, so the fallback arm
/// is unreachable but required for exhaustiveness.
fn doubling_body(args: &[Value]) -> Value {
    match args.first() {
        Some(Value::I32(a)) => Value::I32(a.wrapping_mul(2)),
        _ => Value::I32(0),
    }
}

/// Stores `error` as the last error and returns its code: the shared
/// failure epilogue of the verification exports.
fn store_error(error: BffiError) -> ErrorCode {
    let code = error.code;
    set_last_error(error);
    code
}

/// The shared out-parameter guard: stores a `NullPointer` last error
/// and returns `false` when `ptr` is null.
fn out_ok<T>(ptr: *mut T) -> bool {
    if ptr.is_null() {
        store_error(BffiError::new(
            ErrorCode::NullPointer,
            "output pointer is null",
        ));
        return false;
    }
    true
}

/// The shared revoke epilogue: a successful removal is `Ok`; `false`
/// (already revoked, unknown, or foreign handle - the crate's revoke
/// is an idempotent no-op) surfaces as `InvalidHandle` plus a last
/// error so the terminal-revocation contract is observable across the
/// ABI.
fn revoke_status(revoked: bool) -> ErrorCode {
    if revoked {
        return ErrorCode::Ok;
    }
    store_error(BffiError::new(
        ErrorCode::InvalidHandle,
        "callback handle is already revoked or unknown",
    ))
}

bffi_extern! {
    /// Verification export (criterion 5.1, callback lifecycle):
    /// registers the fixed doubling callback `i32(i32)` and writes
    /// its fresh opaque handle into `__ret`.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_callback_register(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        match register(verification_sig(), Arc::new(doubling_body)) {
            Ok(handle) => {
                // SAFETY: `__ret` is non-null (checked above) and
                // valid for one `u64` write per the bun:ffi
                // out-parameter contract.
                unsafe { ::std::ptr::write(__ret, handle.as_u64()) };
                ErrorCode::Ok
            }
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criteria 5.1-5.3, callback lifecycle):
    /// invokes the callback behind `handle` with one `i32` and writes
    /// the result into `__ret`. A signature mismatch maps to
    /// `InvalidArgument` (11), a dead handle to `InvalidHandle` (4),
    /// a wrong-thread call to `WrongThread` (12).
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_callback_invoke(handle: u64, a: i32, __ret: *mut i32) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        match invoke(Handle::from_raw(handle), &[Value::I32(a)]) {
            Ok(Value::I32(value)) => {
                // SAFETY: `__ret` is non-null (checked above) and
                // valid for one `i32` write per the bun:ffi
                // out-parameter contract.
                unsafe { ::std::ptr::write(__ret, value) };
                ErrorCode::Ok
            }
            Ok(_) => store_error(BffiError::new(
                ErrorCode::Error,
                "callback returned an unexpected value kind",
            )),
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criterion 5.2, terminal revocation):
    /// invokes the callback behind `handle` with an EMPTY argument
    /// slice - a deliberate arity mismatch against the registered
    /// `i32(i32)` signature - so the JS side can observe
    /// `SignatureMismatch` as `InvalidArgument` (11) plus the
    /// expected/got message.
    #[unsafe(no_mangle)]
    pub extern "C" fn example_callback_invoke_mismatched(handle: u64) -> ErrorCode {
        match invoke(Handle::from_raw(handle), &[]) {
            Ok(_) => ErrorCode::Ok,
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criteria 5.1/5.2, terminal revocation):
    /// revokes the callback behind `handle` (both directions route
    /// through the same removal). A second revoke reports
    /// `InvalidHandle` (4).
    #[unsafe(no_mangle)]
    pub extern "C" fn example_callback_revoke(handle: u64) -> ErrorCode {
        revoke_status(revoke(Handle::from_raw(handle)))
    }
}

bffi_extern! {
    /// Verification export (criterion 5.1, Rust -> JS ownership):
    /// stores `ptr` - an opaque, handle-sized JS callback token (the
    /// `JSCallback` pointer bun:ffi hands out) - as a JS-side callback
    /// and writes the fresh handle into `__ret`. The token is stored
    /// verbatim and never dereferenced.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_js_callback_bind(ptr: u64, __ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        match bind_js_callback(verification_sig(), ptr as usize) {
            Ok(handle) => {
                // SAFETY: `__ret` is non-null (checked above) and
                // valid for one `u64` write per the bun:ffi
                // out-parameter contract.
                unsafe { ::std::ptr::write(__ret, handle.as_u64()) };
                ErrorCode::Ok
            }
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criterion 5.1, Rust -> JS ownership):
    /// reads the JS-side callback behind `handle` and writes its
    /// stored pointer token into `__ret` - the same value that was
    /// passed to `example_js_callback_bind`.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_js_callback_get(handle: u64, __ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        match js_callback(Handle::from_raw(handle)) {
            Ok(info) => {
                // SAFETY: `__ret` is non-null (checked above) and
                // valid for one `u64` write per the bun:ffi
                // out-parameter contract.
                unsafe { ::std::ptr::write(__ret, info.ptr as u64) };
                ErrorCode::Ok
            }
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criteria 5.1/5.2): revokes the JS-side
    /// callback behind `handle`; afterwards `example_js_callback_get`
    /// reports `InvalidHandle` (4).
    #[unsafe(no_mangle)]
    pub extern "C" fn example_js_callback_revoke(handle: u64) -> ErrorCode {
        revoke_status(revoke(Handle::from_raw(handle)))
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, JS-thread binding): binds
    /// the CURRENT thread as the process-wide JS thread. The first
    /// binder wins; a later attempt from any other thread maps to
    /// `WrongThread` (12). The binding is sticky for the process
    /// lifetime.
    #[unsafe(no_mangle)]
    pub extern "C" fn example_set_js_thread() -> ErrorCode {
        match set_js_thread() {
            Ok(()) => ErrorCode::Ok,
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, wrong-thread delivery):
    /// blocks the calling thread draining the job queue until
    /// `example_loop_stop`, executing every job under the boundary
    /// policy. Writes the number of jobs executed by THIS runner into
    /// `__ret`. Intended for the worker thread of the phase-C test.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_loop_run(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        // SAFETY: `__ret` is non-null (checked above) and valid for
        // one `u64` write per the bun:ffi out-parameter contract.
        unsafe { ::std::ptr::write(__ret, run()) };
        ErrorCode::Ok
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3): stops the loop for good
    /// (sticky): `example_loop_run` returns and later
    /// `example_loop_marshal_invoke` calls report `WrongThread` (12).
    #[unsafe(no_mangle)]
    pub extern "C" fn example_loop_stop() -> ErrorCode {
        stop();
        ErrorCode::Ok
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, loop probes): writes the
    /// number of jobs waiting for execution into `__ret`.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_loop_pending(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        // SAFETY: `__ret` is non-null (checked above) and valid for
        // one `u64` write per the bun:ffi out-parameter contract.
        unsafe { ::std::ptr::write(__ret, pending()) };
        ErrorCode::Ok
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, loop probes): writes the
    /// number of jobs executed since process start into `__ret`.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_loop_executed(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        // SAFETY: `__ret` is non-null (checked above) and valid for
        // one `u64` write per the bun:ffi out-parameter contract.
        unsafe { ::std::ptr::write(__ret, executed_total()) };
        ErrorCode::Ok
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, loop probes): writes the
    /// documented `pump()` mock result into `__ret` (always `0` - the
    /// real non-blocking drain is a later phase).
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_loop_pump(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        // SAFETY: `__ret` is non-null (checked above) and valid for
        // one `u64` write per the bun:ffi out-parameter contract.
        unsafe { ::std::ptr::write(__ret, pump()) };
        ErrorCode::Ok
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, marshal delivery): puts a
    /// job on the loop queue; the job invokes the callback behind
    /// `handle` with `a` ON THE RUNNER THREAD (where
    /// `example_set_js_thread` bound the JS thread) and stores the
    /// result into the process-wide `LAST_INVOKED` slot, readable
    /// through `example_last_invoked`. Without a running runner this
    /// maps to `WrongThread` (12).
    #[unsafe(no_mangle)]
    pub extern "C" fn example_loop_marshal_invoke(handle: u64, a: i32) -> ErrorCode {
        let job: Job = Box::new(move || {
            if let Ok(Value::I32(value)) = invoke(Handle::from_raw(handle), &[Value::I32(a)]) {
                LAST_INVOKED.store(value, Ordering::Relaxed);
            }
        });
        match marshal(job) {
            Ok(()) => ErrorCode::Ok,
            Err(error) => store_error(BffiError::from(error)),
        }
    }
}

bffi_extern! {
    /// Verification export (criterion 5.3, marshal delivery): writes
    /// the value stored by the last executed marshal job into `__ret`
    /// (`0` before the first executed job).
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_last_invoked(__ret: *mut i32) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        // SAFETY: `__ret` is non-null (checked above) and valid for
        // one `i32` write per the bun:ffi out-parameter contract.
        unsafe { ::std::ptr::write(__ret, LAST_INVOKED.load(Ordering::Relaxed)) };
        ErrorCode::Ok
    }
}
