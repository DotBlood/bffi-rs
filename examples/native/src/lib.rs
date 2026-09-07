//! Example native module for bffi-rs: the first real cdylib Bun loads
//! through `bun:ffi`.
//!
//! It exercises the full P1 shim contract plus the P2 runtime exports:
//!
//! - [`add`] - primitives and the `__ret` out-parameter;
//! - [`greet`] / [`greet_len`] - the cstring (`&str`) parameter path;
//! - [`echo_buffer`] - the borrowed `&[u8]` parameter path (a
//!   `(ptr, len)` pair per CALLING-CONVENTION.md §3) combined with a
//!   `CopiedBuf` return;
//! - [`boom`] - the release panic path (debug builds abort by design,
//!   so the e2e suite only ever loads the release artifact);
//! - [`facade_probe`] - the facade-only mode: annotated with
//!   `#[bffi(crate = "bffi")]`, its shim and descriptor resolve
//!   through the `bffi` facade namespaces while every other function
//!   here stays in the default direct-dependency mode;
//! - the verification exports (`mirror_*`, `is_even`,
//!   `example_callback_*`, `example_js_callback_*`,
//!   `example_set_js_thread`, `example_loop_*`,
//!   `example_last_invoked`) - the P1 surface exercised with real
//!   bun:ffi by `js/callbacks.test.ts` and `js/numbers.test.ts`;
//! - `bffi_runtime_abi!()` - the eight JS-facing runtime exports.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::Duration;

use bffi::{CopiedBuf, ErrorCode};
use bffi_async::{
    AsyncValue, bffi_async_abi, sleep as async_sleep, spawn as async_spawn,
    timeout as async_timeout,
};
use bffi_build::bffi_runtime_abi;
use bffi_callback::{
    CallbackSig, Value, ValueType, bind_js_callback, invoke, js_callback, register, revoke,
    set_js_thread,
};
use bffi_class::{bffi_class, bffi_constructor, bffi_impl};
use bffi_core::{BffiError, Handle, bffi_extern, set_last_error};
use bffi_event_loop::{Job, executed_total, marshal, pending, pump, run, stop};
use bffi_macros::{bffi, bffi_async};

// The eight JS-facing runtime exports (bffi_error_*, bffi_buffer pair,
// bffi_types_free) generated into this cdylib.
bffi_runtime_abi!();

// The two JS-facing async exports (bffi_async_attach/bffi_async_cancel)
// generated into this cdylib.
bffi_async_abi!();

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

/// Copies the borrowed bytes into a fresh buffer and returns its
/// handle: the borrowed `&[u8]` parameter travels as a `(ptr, len)`
/// pair (CALLING-CONVENTION.md §3) and stays valid only for the
/// duration of the call, so the documented copy-on-return policy
/// applies.
#[bffi]
pub fn echo_buffer(data: &[u8]) -> CopiedBuf {
    CopiedBuf::from_slice(data)
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

/// Facade-only mode probe: `crate = "bffi"` makes the generated shim
/// and descriptor resolve through the `bffi` facade namespaces
/// (`::bffi::core`, `::bffi::dts`, ...) instead of the direct
/// dependencies. The other functions above stay in default mode, so
/// both modes compile side by side in one crate.
#[bffi(crate = "bffi")]
pub fn facade_probe(x: u32) -> u32 {
    x * 3
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
    /// `pump()` non-blocking drain result into `__ret` (the number of
    /// jobs executed by this call - `0` on an empty queue).
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
    /// Debug helper (P3): the number of live async tasks.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_async_pending(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        // SAFETY: `__ret` is non-null (checked above) and valid for
        // one `u64` write per the bun:ffi out-parameter contract.
        unsafe { ::std::ptr::write(__ret, bffi_async::pending_tasks()) };
        ErrorCode::Ok
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

/// The shared spawn epilogue of the async verification exports:
/// writes the task handle to `__ret` or stores the spawn error.
fn spawn_to_ret(
    __ret: *mut u64,
    future: impl std::future::Future<Output = Result<AsyncValue, BffiError>> + Send + 'static,
) -> ErrorCode {
    if !out_ok(__ret) {
        return ErrorCode::NullPointer;
    }
    match async_spawn(future) {
        Ok(handle) => {
            // SAFETY: `__ret` is non-null (checked above) and valid for
            // one `u64` write per the bun:ffi out-parameter contract.
            unsafe { ::std::ptr::write(__ret, handle.as_u64()) };
            ErrorCode::Ok
        }
        Err(error) => store_error(error.into()),
    }
}

fn ok_value(value: AsyncValue) -> Result<AsyncValue, BffiError> {
    Ok(value)
}

bffi_extern! {
    /// Verification export (P3): spawns a task doubling `x` after a
    /// short sleep; `__ret` receives the task handle.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_async_double(x: u32, __ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        spawn_to_ret(
            __ret,
            async move {
                async_sleep(Duration::from_millis(15)).await;
                ok_value(AsyncValue::I64((i64::from(x)) * 2))
            },
        )
    }
}

bffi_extern! {
    /// Verification export (P3): spawns a task returning a string
    /// (delivered through the transient-buffer pair). The string
    /// parameter follows the cstring convention (CALLING-CONVENTION.md
    /// section 3).
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_async_shout(
        name_ptr: *const std::os::raw::c_char,
        __ret: *mut u64,
    ) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        if name_ptr.is_null() {
            return store_error(BffiError::new(
                ErrorCode::NullPointer,
                "string argument pointer is null",
            ));
        }
        // SAFETY: bun:ffi hands out NUL-terminated cstrings for string
        // parameters (DESIGN.md 6.3); the pointer is null-checked
        // above.
        let bytes = unsafe { ::std::ffi::CStr::from_ptr(name_ptr) }.to_bytes();
        let view = match bffi_types::unsafe_zero_copy::str_view(bytes) {
            Ok(v) => v,
            Err(error) => return store_error(error),
        };
        let owned = view.as_str().to_owned();
        spawn_to_ret(
            __ret,
            async move { ok_value(AsyncValue::Str(format!("HELLO {owned}!"))) },
        )
    }
}

bffi_extern! {
    /// Verification export (P3): spawns a failing task; the promise
    /// rejects with the domain message.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_async_fail(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        spawn_to_ret(
            __ret,
            async move {
                Err(BffiError::new(
                    ErrorCode::DomainError,
                    "domain failure",
                ))
            },
        )
    }
}

bffi_extern! {
    /// Verification export (P3): spawns a panicking task; the release
    /// boundary turns the panic into a rejection.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref, clippy::panic)]
    pub extern "C" fn example_async_panic(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        spawn_to_ret(
            __ret,
            async move {
                panic!("async boom");
                #[allow(unreachable_code)]
                ok_value(AsyncValue::Unit)
            },
        )
    }
}

bffi_extern! {
    /// Verification export (P3): spawns a task that never completes
    /// on its own under a 50 ms timeout; the promise rejects with
    /// "task timed out" and the inner future is dropped.
    #[unsafe(no_mangle)]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub extern "C" fn example_async_timeout(__ret: *mut u64) -> ErrorCode {
        if !out_ok(__ret) {
            return ErrorCode::NullPointer;
        }
        spawn_to_ret(
            __ret,
            async_timeout(
                Duration::from_millis(50),
                async move {
                    async_sleep(Duration::from_secs(60)).await;
                    ok_value(AsyncValue::Unit)
                },
            ),
        )
    }
}

/// The `#[bffi_async]` macro: the spawn shim and `Promise<u32>`
/// descriptor are generated; `js/async.test.ts` awaits the value.
#[bffi_async]
pub async fn example_compute(x: u32) -> u32 {
    async_sleep(Duration::from_millis(15)).await;
    x * 2
}
