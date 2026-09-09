//! The generic callback C ABI: wire-encoded arguments and results for
//! the JS-facing exports, plus the declarative generator of the four
//! exports themselves.
//!
//! The layout mirrors `bffi-build`'s `bffi_runtime_abi!` / `bffi-async`'s
//! `bffi_async_abi!`: the generator is a `macro_rules!` defined here and
//! expanded - IN THE USER crate, not in this rlib - so the linker
//! cannot drop the `#[no_mangle]` symbols from the final cdylib.
//!
//! # Generated exports (CALLING-CONVENTION.md, "Callback ABI exports")
//!
//! | Symbol | Signature | Notes |
//! | --- | --- | --- |
//! | `bffi_callback_set_thread` | `() -> ErrorCode` | binds the calling thread as the process-wide JS thread (`WrongThread` when already bound elsewhere) |
//! | `bffi_callback_bind` | `(sig_ret: u8, sig_params_ptr: *const u8, sig_params_len: u64, js_ptr: u64, __ret: *mut u64) -> ErrorCode` | Rust -> JS: stores the opaque JSCallback pointer under a fresh handle |
//! | `bffi_callback_invoke` | `(handle: u64, args_ptr: *const u8, args_len: u64, __ret: *mut u64) -> ErrorCode` | JS -> Rust: invokes the native body; the result travels as a transient-buffer handle holding one wire record |
//! | `bffi_callback_revoke` | `(handle: u64) -> u32` | terminal; `ErrorCode::InvalidHandle` on a second revoke |
//!
//! Signature and argument bytes use the framework-wide wire codec
//! (`bffi_types::wire`): a signature is the return tag byte followed by
//! one parameter tag byte each (`I32`=1, `I64`=2, `F64`=3, `Bool`=4);
//! arguments are concatenated `[tag][payload]` records. Malformed
//! bytes surface as `ErrorCode::InvalidArgument` with a stored message.
//!
//! # User-crate requirements
//!
//! The expansion names `::bffi_core`, `::bffi_callback` (`$crate`) and
//! `::bffi_build` / `::bffi_types` (the result buffer): the user
//! cdylib crate depends on all four.
//!
//! # Panics
//!
//!
//! The bodies run through the same build policy as every bffi export
//! (DESIGN §6.5): debug builds run bare; release builds wrap in
//! `run_extern_body_or` with the documented sentinel (a `Panic` status
//! for the status-returning exports, `0` for the out-value exports).

use bffi_core::{BffiError, ErrorCode, Handle, set_last_error};
use bffi_types::CopiedBuf;
use bffi_types::wire;

use crate::error::CallbackError;
use crate::registry::{bind_js_callback, invoke, revoke};
use crate::thread::set_js_thread;
use crate::value::{CallbackSig, Value, ValueType};

/// Stores `error` as the thread-local last error and returns its code:
/// the shared failure epilogue of every export body.
fn store(error: BffiError) -> ErrorCode {
    let code = error.code;
    set_last_error(error);
    code
}

/// Maps a [`CallbackError`] onto the last-error channel and returns
/// its code.
fn store_callback(error: CallbackError) -> ErrorCode {
    store(BffiError::from(error))
}

/// The `out_ok` guard: stores a `NullPointer` last error and returns
/// `false` when `ptr` is null.
fn out_ok<T>(ptr: *mut T) -> bool {
    if ptr.is_null() {
        store(BffiError::new(
            ErrorCode::NullPointer,
            "output pointer is null",
        ));
        return false;
    }
    true
}

/// Builds the `("ptr", "len")` slice the shim received: an empty view
/// for `len == 0` (the null-pointer contract), the raw borrow
/// otherwise.
///
/// Only the `bffi_callback_abi!()` expansion calls this (from the user
/// crate), which the compiler cannot see - hence the permit below.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes for the duration of the
/// call (the `bun:ffi` TypedArray-pointer contract, CALLING-CONVENTION.md
/// §3); a null `ptr` is only allowed when `len == 0`.
#[doc(hidden)]
pub unsafe fn arg_slice<'a>(ptr: *const u8, len: u64) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        // SAFETY: the caller contract (see above) guarantees `ptr` is
        // non-null and valid for exactly `len` readable bytes.
        unsafe { ::std::slice::from_raw_parts(ptr, len as usize) }
    }
}

/// Decodes the concatenated `[tag][payload]` argument records into
/// values. An unknown tag or a truncated payload is `InvalidArgument`
/// with the exact byte position.
///
/// # Errors
///
/// [`CallbackError::SignatureMismatch`] is never produced here; only
/// the malformed-shape [`BffiError`] (returned, not stored).
pub fn decode_args(bytes: &[u8]) -> Result<Vec<Value>, BffiError> {
    let mut values = Vec::new();
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let tag = bytes[offset];
        let value = match tag {
            wire::TAG_I32 => wire::read_i32_le(bytes, offset + 1).map(Value::I32),
            wire::TAG_I64 => wire::read_i64_le(bytes, offset + 1).map(Value::I64),
            wire::TAG_F64 => wire::read_f64_le(bytes, offset + 1).map(Value::F64),
            wire::TAG_BOOL => wire::read_bool(bytes, offset + 1).map(Value::Bool),
            _ => None,
        };
        let value = value.ok_or_else(|| {
            BffiError::new(
                ErrorCode::InvalidArgument,
                format!(
                    "malformed callback arguments: unknown or truncated value tag {tag} at byte {offset}"
                ),
            )
        })?;
        offset += 1 + payload_len(tag);
        values.push(value);
    }
    Ok(values)
}

/// The payload byte length of a fixed-width callback value record.
fn payload_len(tag: u8) -> usize {
    match tag {
        wire::TAG_I32 => 4,
        wire::TAG_I64 | wire::TAG_F64 => 8,
        wire::TAG_BOOL => 1,
        // Unknown tags never reach this function through `decode_args`
        // (they error out before the length is consulted).
        _ => 0,
    }
}

/// Encodes the callback result into the wire record stored in the
/// transient-buffer table.
#[must_use]
pub fn encode_result(value: Value) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode_into(&mut out);
    out
}

/// The body behind `bffi_callback_set_thread`.
pub fn set_thread_body() -> ErrorCode {
    match set_js_thread() {
        Ok(()) => ErrorCode::Ok,
        Err(error) => store_callback(error),
    }
}

/// The body behind `bffi_callback_bind`: decodes the signature bytes,
/// stores the opaque JS pointer, writes the fresh handle.
/// The body behind `bffi_callback_bind`: decodes the signature bytes,
/// stores the opaque JS pointer, writes the fresh handle. The raw-
/// pointer write is guarded by `out_ok` (a null `__ret` reports
/// `NullPointer`, never UB); the lint allowance mirrors the generated
/// shims.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn bind_body(sig_ret: u8, sig_params: &[u8], js_ptr: u64, __ret: *mut u64) -> ErrorCode {
    if !out_ok(__ret) {
        return ErrorCode::NullPointer;
    }
    let ret = match ValueType::from_wire_tag(sig_ret) {
        Some(ret) => ret,
        None => {
            return store(BffiError::new(
                ErrorCode::InvalidArgument,
                format!("unknown callback return tag {sig_ret}"),
            ));
        }
    };
    let mut params = Vec::with_capacity(sig_params.len());
    for (index, tag) in sig_params.iter().enumerate() {
        match ValueType::from_wire_tag(*tag) {
            Some(ty) => params.push(ty),
            None => {
                return store(BffiError::new(
                    ErrorCode::InvalidArgument,
                    format!("unknown callback parameter tag {tag} at index {index}"),
                ));
            }
        }
    }
    match bind_js_callback(CallbackSig::new(ret, &params), js_ptr as usize) {
        Ok(handle) => {
            // SAFETY: `__ret` is non-null (checked above) and valid for
            // one `u64` write per the bun:ffi out-parameter contract.
            unsafe { ::std::ptr::write(__ret, handle.as_u64()) };
            ErrorCode::Ok
        }
        Err(error) => store_callback(error),
    }
}

/// The body behind `bffi_callback_invoke`: decodes the arguments,
/// invokes the native body, stores the encoded result in the
/// transient-buffer table and writes its handle.
/// The body behind `bffi_callback_invoke`: decodes the arguments,
/// invokes the native body, stores the encoded result in the
/// transient-buffer table and writes its handle. Same `out_ok` guard
/// and lint allowance as [`bind_body`].
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn invoke_body(handle: u64, args: &[u8], __ret: *mut u64) -> ErrorCode {
    if !out_ok(__ret) {
        return ErrorCode::NullPointer;
    }
    let values = match decode_args(args) {
        Ok(values) => values,
        Err(error) => return store(error),
    };
    match invoke(Handle::from_raw(handle), &values) {
        Ok(value) => {
            let bytes = encode_result(value);
            match bffi_build::runtime::store_bytes(CopiedBuf::from_vec(bytes)) {
                Ok(buffer) => {
                    // SAFETY: `__ret` is non-null (checked above) and
                    // valid for one `u64` write per the bun:ffi
                    // out-parameter contract.
                    unsafe { ::std::ptr::write(__ret, buffer.as_u64()) };
                    ErrorCode::Ok
                }
                Err(error) => store(BffiError::from(error)),
            }
        }
        Err(error) => store_callback(error),
    }
}

/// The body behind `bffi_callback_revoke`: `Ok` on a live removal,
/// `InvalidHandle` for an already-revoked/unknown/foreign handle.
#[must_use]
pub fn revoke_body(handle: u64) -> u32 {
    if revoke(Handle::from_raw(handle)) {
        ErrorCode::Ok.as_u32()
    } else {
        store(BffiError::new(
            ErrorCode::InvalidHandle,
            "callback handle is already revoked or unknown",
        ))
        .as_u32()
    }
}

/// Generates the four JS-facing callback exports.
///
/// Call once, at the root of the cdylib crate:
///
/// ```text
/// bffi_callback::bffi_callback_abi!();
/// ```
///
/// # Generated exports
///
/// | Symbol | Signature | Sentinel on panic |
/// |---|---|---|
/// | `bffi_callback_set_thread` | `() -> ErrorCode` | `ErrorCode::Panic` |
/// | `bffi_callback_bind` | `(sig_ret: u8, sig_params_ptr: *const u8, sig_params_len: u64, js_ptr: u64, __ret: *mut u64) -> ErrorCode` | `ErrorCode::Panic` (`__ret` untouched) |
/// | `bffi_callback_invoke` | `(handle: u64, args_ptr: *const u8, args_len: u64, __ret: *mut u64) -> ErrorCode` | `ErrorCode::Panic` (`__ret` untouched) |
/// | `bffi_callback_revoke` | `(handle: u64) -> u32` | `ErrorCode::Panic` value |
///
/// The user crate depends on `bffi-core`, `bffi-callback`, `bffi-build`
/// and `bffi-types` (the expansion names all four by absolute path).
#[macro_export]
macro_rules! bffi_callback_abi {
    () => {
        /// C ABI export generated by `bffi_callback::bffi_callback_abi!()`:
        /// binds the calling thread as the process-wide JS thread
        /// (sticky; `ErrorCode::WrongThread` when already bound elsewhere).
        #[unsafe(no_mangle)]
        pub extern "C" fn bffi_callback_set_thread() -> ::bffi_core::ErrorCode {
            #[cfg(debug_assertions)]
            {
                $crate::abi::set_thread_body()
            }
            #[cfg(not(debug_assertions))]
            {
                ::bffi_core::boundary::run_extern_body_or(
                    || $crate::abi::set_thread_body(),
                    ::bffi_core::ErrorCode::Panic,
                )
            }
        }

        /// C ABI export generated by `bffi_callback::bffi_callback_abi!()`:
        /// Rust -> JS binding. `sig_ret` is one wire tag byte,
        /// `sig_params` is the parameter tag bytes, `js_ptr` is the
        /// opaque JSCallback pointer token; `__ret` receives the fresh
        /// callback handle.
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn bffi_callback_bind(
            sig_ret: u8,
            sig_params_ptr: *const u8,
            sig_params_len: u64,
            js_ptr: u64,
            __ret: *mut u64,
        ) -> ::bffi_core::ErrorCode {
            #[cfg(debug_assertions)]
            {
                // SAFETY: bun:ffi keeps the TypedArray pointer valid for
                // the duration of the call; `len == 0` takes the empty
                // slice branch (CALLING-CONVENTION.md §3).
                let params = unsafe { $crate::abi::arg_slice(sig_params_ptr, sig_params_len) };
                $crate::abi::bind_body(sig_ret, params, js_ptr, __ret)
            }
            #[cfg(not(debug_assertions))]
            {
                ::bffi_core::boundary::run_extern_body_or(
                    || {
                        // SAFETY: see the debug branch.
                        let params =
                            unsafe { $crate::abi::arg_slice(sig_params_ptr, sig_params_len) };
                        $crate::abi::bind_body(sig_ret, params, js_ptr, __ret)
                    },
                    ::bffi_core::ErrorCode::Panic,
                )
            }
        }

        /// C ABI export generated by `bffi_callback::bffi_callback_abi!()`:
        /// JS -> Rust invocation. `args` are the concatenated wire
        /// records; on success `__ret` receives the transient-buffer
        /// handle of the encoded result.
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn bffi_callback_invoke(
            handle: u64,
            args_ptr: *const u8,
            args_len: u64,
            __ret: *mut u64,
        ) -> ::bffi_core::ErrorCode {
            #[cfg(debug_assertions)]
            {
                // SAFETY: bun:ffi keeps the TypedArray pointer valid for
                // the duration of the call; `len == 0` takes the empty
                // slice branch (CALLING-CONVENTION.md §3).
                let args = unsafe { $crate::abi::arg_slice(args_ptr, args_len) };
                $crate::abi::invoke_body(handle, args, __ret)
            }
            #[cfg(not(debug_assertions))]
            {
                ::bffi_core::boundary::run_extern_body_or(
                    || {
                        // SAFETY: see the debug branch.
                        let args = unsafe { $crate::abi::arg_slice(args_ptr, args_len) };
                        $crate::abi::invoke_body(handle, args, __ret)
                    },
                    ::bffi_core::ErrorCode::Panic,
                )
            }
        }

        /// C ABI export generated by `bffi_callback::bffi_callback_abi!()`:
        /// terminal revocation in both directions. Returns the
        /// [`ErrorCode`](::bffi_core::ErrorCode) value (`0` = Ok, `4` =
        /// InvalidHandle for a second revoke).
        #[unsafe(no_mangle)]
        pub extern "C" fn bffi_callback_revoke(handle: u64) -> u32 {
            #[cfg(debug_assertions)]
            {
                $crate::abi::revoke_body(handle)
            }
            #[cfg(not(debug_assertions))]
            {
                ::bffi_core::boundary::run_extern_body_or(
                    || $crate::abi::revoke_body(handle),
                    ::bffi_core::ErrorCode::Panic.as_u32(),
                )
            }
        }
    };
}
