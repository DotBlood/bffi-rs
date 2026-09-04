//! Panic containment for `extern "C"` functions (DESIGN §6.1, §6.5).
//!
//! Every FFI entry point must be thin, and a Rust panic must never unwind
//! into the Bun host. The policy differs per build (DESIGN §6.5):
//!
//! | Build | Behavior                                                     |
//! |-------|--------------------------------------------------------------|
//! | dev   | the panic escapes and aborts the process (easier debugging) |
//! | prod  | the panic is caught and converted into an [`ErrorCode`]     |
//!
//! [`catch_panic`] is the always-catching primitive (usable and testable in
//! any profile), [`run_extern_body`] adds the error-code/last-error
//! plumbing, and the [`bffi_extern!`](crate::bffi_extern) macro wires the
//! policy choice into `extern "C"` declarations. The procedural `#[bffi]`
//! attribute (`bffi-macros`, P1) will expand to the same shape.

use crate::error::{BffiError, ErrorCode, set_last_error};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Runs `f`, converting a panic into a [`BffiError`].
///
/// This is the low-level containment primitive: it catches in every build
/// configuration, so it is testable in debug builds. [`run_extern_body`] is
/// the policy-aware wrapper used by FFI entry points.
///
/// `f` is wrapped in [`AssertUnwindSafe`] because unwind safety does not
/// apply at an FFI boundary: the closure's state does not outlive the call,
/// and the only observer of the failure is the freshly created
/// [`BffiError`], which touches none of that state.
///
/// # Examples
///
/// ```
/// use bffi_core::boundary::catch_panic;
///
/// let ok = catch_panic(|| 1 + 1);
/// assert!(matches!(ok, Ok(2)));
///
/// let panicked = catch_panic(|| panic!("boom"));
/// assert_eq!(panicked.unwrap_err().message, "boom");
/// ```
pub fn catch_panic<T, F>(f: F) -> Result<T, BffiError>
where
    F: FnOnce() -> T,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => Ok(value),
        // `&*payload` (not `&payload`): the Box must be explicitly
        // dereferenced so `panic_message` receives the payload object
        // itself, not a reference to the box.
        Err(payload) => Err(BffiError::new(ErrorCode::Panic, panic_message(&*payload))),
    }
}

/// Extracts a human-readable message from a panic payload.
///
/// `panic!("literal")` payloads are `&'static str`, formatted panics are
/// `String`, and `Box<dyn Any>` payloads from other sources fall back to a
/// fixed description.
#[must_use]
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_owned();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "panic payload was not a string".to_owned()
}

/// Executes the body of an `extern "C"` function under the production
/// boundary policy: panics become [`ErrorCode::Panic`] plus a stored
/// [last error](crate::error).
///
/// This function always catches (even in debug builds); the
/// [`bffi_extern!`](crate::bffi_extern) macro decides *whether* to use it -
/// debug builds let the panic escape instead (DESIGN §6.5).
#[must_use]
pub fn run_extern_body<F>(f: F) -> ErrorCode
where
    F: FnOnce() -> ErrorCode,
{
    match catch_panic(f) {
        Ok(code) => code,
        Err(error) => {
            set_last_error(error);
            ErrorCode::Panic
        }
    }
}

/// Declares an `extern "C"` function whose body runs under the FFI safety
/// policy.
///
/// The wrapped function must return [`ErrorCode`] (the numeric status that
/// crosses the C ABI); additional results travel through out-parameters,
/// which the `bffi-types` layer (P1) will provide. The body must not use
/// `?` or early `return` of a non-[`ErrorCode`] value.
///
/// * In **debug** builds the body runs directly: a panic aborts the process
///   (`extern "C"` unwinding aborts by definition), which keeps stack
///   traces intact while debugging.
/// * In **release** builds the body runs through [`run_extern_body`]: a
///   panic is converted into [`ErrorCode::Panic`] and a thread-local last
///   error.
///
/// # Examples
///
/// ```
/// use bffi_core::{bffi_extern, error::ErrorCode};
/// use std::sync::atomic::{AtomicU32, Ordering};
///
/// static LAST_SUM: AtomicU32 = AtomicU32::new(0);
///
/// bffi_extern! {
///     /// Adds two numbers and records the result.
///     pub extern "C" fn my_add(a: u32, b: u32) -> ErrorCode {
///         LAST_SUM.store(a.wrapping_add(b), Ordering::Relaxed);
///         ErrorCode::Ok
///     }
/// }
///
/// assert_eq!(my_add(2, 3), ErrorCode::Ok);
/// assert_eq!(LAST_SUM.load(Ordering::Relaxed), 5);
/// ```
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! bffi_extern {
    ($(#[$attr:meta])* $vis:vis extern "C" fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty { $($body:tt)* }) => {
        $(#[$attr])*
        $vis extern "C" fn $name($($arg: $ty),*) -> $ret {
            $($body)*
        }
    };
}

/// Declares an `extern "C"` function whose body runs under the FFI safety
/// policy (release variant: panics are caught, see the debug variant).
#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! bffi_extern {
    ($(#[$attr:meta])* $vis:vis extern "C" fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty { $($body:tt)* }) => {
        $(#[$attr])*
        $vis extern "C" fn $name($($arg: $ty),*) -> $ret {
            $crate::boundary::run_extern_body(move || { $($body)* })
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::take_last_error;

    #[test]
    fn catches_string_panic_messages() {
        let outcome = catch_panic(|| panic!("boom"));
        assert_eq!(outcome.unwrap_err().message, "boom");
    }

    #[test]
    fn catches_formatted_panic_messages() {
        let outcome = catch_panic(|| panic!("bad value: {}", 7));
        assert_eq!(outcome.unwrap_err().message, "bad value: 7");
    }

    #[test]
    fn catches_non_string_payloads() {
        use std::panic::resume_unwind;
        let outcome = catch_panic::<(), _>(|| resume_unwind(Box::new(42_u32)));
        assert_eq!(
            outcome.unwrap_err().message,
            "panic payload was not a string"
        );
    }

    #[test]
    fn passes_values_through_when_no_panic_occurs() {
        assert!(matches!(catch_panic(|| 40 + 2), Ok(42)));
    }

    #[test]
    fn panic_errors_carry_the_panic_code() {
        let error = catch_panic(|| panic!("nope")).unwrap_err();
        assert_eq!(error.code, ErrorCode::Panic);
    }

    #[test]
    fn extern_body_returns_success_code_untouched() {
        assert_eq!(run_extern_body(|| ErrorCode::Ok), ErrorCode::Ok);
    }

    #[test]
    fn extern_body_converts_panic_into_code_and_last_error() {
        let code = run_extern_body(|| panic!("boundary!"));
        assert_eq!(code, ErrorCode::Panic);
        let error = take_last_error().expect("last error must be stored");
        assert_eq!(error.code, ErrorCode::Panic);
        assert_eq!(error.message, "boundary!");
    }
}
