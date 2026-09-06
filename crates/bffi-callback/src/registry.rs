//! The callback table and the JS -> Rust lifecycle: [`register`],
//! [`invoke`], [`revoke`].
//!
//! A Rust closure is registered explicitly and addressed by an opaque
//! [`Handle`] (criterion 5.1: registration = `register`), invoked with
//! a runtime-checked signature, and revoked terminally by [`revoke`] -
//! the single removal point for BOTH callback directions (criterion
//! 5.1: revocation = `revoke`): the registry's type-erased `remove`
//! routes by the handle's tag to either table.
//!
//! A revoked handle never resurrects: revocation drops the registry
//! slot, and the generational handle scheme keeps the stale value dead
//! even if the slot is later reused (criterion 5.2).
//!
//! Storage lives in the process-wide [`Registry`] under two
//! crate-owned tags: `0x0200` for [`NativeEntry`] (this module) and
//! `0x0201` for [`JsEntry`] (populated by the Rust -> JS direction).

use std::sync::{Arc, OnceLock};

use bffi_core::{Handle, Registry, TypeTag};

use crate::error::CallbackError;
use crate::thread::ensure_js_thread;
use crate::value::{CallbackSig, Value};

/// The type tag of the native (JS -> Rust) callback table.
const NATIVE_TAG: TypeTag = TypeTag(0x0200);

/// The type tag of the JS-side (Rust -> JS) callback table.
const JS_TAG: TypeTag = TypeTag(0x0201);

/// The body of a registered native callback: receives the (already
/// signature-checked) arguments and returns the callback's value.
type NativeBody = dyn Fn(&[Value]) -> Value + Send + Sync;

/// A registered native callback: its declared signature plus the Rust
/// closure invoked on every call.
struct NativeEntry {
    sig: CallbackSig,
    f: Arc<NativeBody>,
}

/// A registered JS-side callback (Rust -> JS direction): its declared
/// signature plus the raw trampoline slot handed over by the JS side.
/// Declared now so both tables are born at one initialization point;
/// the Rust -> JS direction populates the table.
// The fields stay unread until the Rust -> JS direction lands; the
// declaration itself is what reserves the table's type identity.
#[allow(dead_code)]
struct JsEntry {
    sig: CallbackSig,
    ptr: usize,
}

/// Declares both callback tables in the global registry, exactly once
/// per process.
///
/// The memoized `Result` inside the [`OnceLock`] makes the outcome
/// sticky: every later caller gets the same result as the first
/// initializer, whether `Ok` or `Err`. A `RegistryError::
/// TagAlreadyRegistered` maps to [`CallbackError::TagInUse`] for the
/// failing tag - the tags are crate-owned, so contention means two
/// initializers raced, and the `OnceLock` keeps only one outcome.
///
/// If the [`NativeEntry`] declare succeeds but the [`JsEntry`] declare
/// fails, that partial state (native table only) is permanent: the
/// registry has no undeclare, and the memoized `Err` stops all later
/// retries.
fn tables() -> Result<(), CallbackError> {
    static TABLES: OnceLock<Result<(), CallbackError>> = OnceLock::new();
    TABLES
        .get_or_init(|| {
            Registry::global()
                .declare::<NativeEntry>(NATIVE_TAG)
                .map_err(|_| CallbackError::TagInUse(NATIVE_TAG))?;
            Registry::global()
                .declare::<JsEntry>(JS_TAG)
                .map_err(|_| CallbackError::TagInUse(JS_TAG))?;
            Ok(())
        })
        .clone()
}

/// Registers a native callback (JS -> Rust direction) and returns its
/// fresh, opaque [`Handle`].
///
/// The declared [`CallbackSig`] is enforced on every [`invoke`]: the
/// call is rejected with [`CallbackError::SignatureMismatch`] unless
/// the arguments satisfy it.
///
/// # Errors
///
/// [`CallbackError::TableFull`] when the native table has no free
/// slot, or [`CallbackError::TagInUse`] if table initialization ever
/// failed earlier in the process (the memoized outcome).
pub fn register(sig: CallbackSig, f: Arc<NativeBody>) -> Result<Handle, CallbackError> {
    tables()?;
    Registry::global()
        .insert(NATIVE_TAG, Arc::new(NativeEntry { sig, f }))
        .map_err(|_| {
            // `tables()` declared NATIVE_TAG for `NativeEntry` and the
            // registry has no undeclare, so `NotRegistered` is
            // unreachable after declare; the remaining condition is a
            // full table. `RegistryError` is `#[non_exhaustive]`, so
            // future variants are grouped defensively into the
            // wildcard (same pattern as bffi-object's wrap.rs).
            CallbackError::TableFull
        })
}

/// Invokes the native callback behind `handle` with `args`.
///
/// Check order: table init -> null handle -> table lookup (wrong tag,
/// stale, or unknown handles all land in
/// [`CallbackError::InvalidHandle`]) -> signature
/// ([`CallbackError::SignatureMismatch`]) -> JS-thread gate ->
/// the call itself.
///
/// The closure's panic is deliberately NOT caught here: debug builds
/// abort (easier debugging) and release catches at the P2 event-loop
/// trampoline (`run_extern_body`, DESIGN.md §6.5).
///
/// # Errors
///
/// [`CallbackError::InvalidHandle`] for a null, revoked, stale, or
/// foreign-kind handle; [`CallbackError::SignatureMismatch`] when
/// `args` do not satisfy the declared signature;
/// [`CallbackError::WrongThread`] on a bound process when called off
/// the JS thread; [`CallbackError::TagInUse`] if table initialization
/// ever failed earlier in the process.
pub fn invoke(handle: Handle, args: &[Value]) -> Result<Value, CallbackError> {
    tables()?;
    if handle.is_null() {
        return Err(CallbackError::InvalidHandle(handle));
    }
    let entry = Registry::global()
        .get_typed::<NativeEntry>(handle)
        .ok_or(CallbackError::InvalidHandle(handle))?;
    if !entry.sig.matches(args) {
        return Err(CallbackError::SignatureMismatch {
            expected: entry.sig.clone(),
            got: args.iter().map(|v| v.ty()).collect(),
        });
    }
    ensure_js_thread()?;
    Ok((entry.f)(args))
}

/// Revokes the callback behind `handle`, whatever direction it
/// belongs to.
///
/// This is the single removal point for both callback directions
/// (criterion 5.1: revocation = `revoke`): the registry's type-erased
/// `remove` routes by the handle's tag to the native OR the JS table.
/// Revocation is terminal - the handle never resurrects, even if its
/// slot is reused (criterion 5.2).
///
/// Returns `true` iff a live callback slot was removed. Table
/// initialization failure is ignored gracefully (nothing was ever
/// stored, so there is nothing to remove), as is the null handle.
pub fn revoke(handle: Handle) -> bool {
    if tables().ok().is_none() {
        return false;
    }
    if handle.is_null() {
        return false;
    }
    Registry::global().remove(handle)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bffi_core::Handle;

    use super::{invoke, register, revoke};
    use crate::error::CallbackError;
    use crate::value::{CallbackSig, Value, ValueType};

    // NOTE (test isolation): these tests run in the lib test binary
    // where the process stays UNBOUND - they must never call
    // `set_js_thread` (see `src/thread.rs`). `ensure_js_thread` admits
    // every caller while unbound, so `invoke` needs no binding ritual.

    #[test]
    fn register_invoke_returns_the_closure_result() {
        let sig = CallbackSig::new(ValueType::I32, &[ValueType::I32, ValueType::I32]);
        let handle = register(
            sig,
            Arc::new(|args: &[Value]| {
                let mut sum = 0;
                for arg in args {
                    if let Value::I32(n) = arg {
                        sum += *n;
                    }
                }
                Value::I32(sum)
            }),
        )
        .unwrap();

        let result = invoke(handle, &[Value::I32(2), Value::I32(3)]).unwrap();
        assert_eq!(result, Value::I32(5));
    }

    #[test]
    fn invoke_after_revoke_is_invalid_handle() {
        let sig = CallbackSig::new(ValueType::Bool, &[]);
        let handle = register(sig, Arc::new(|_| Value::Bool(true))).unwrap();

        assert!(revoke(handle));
        assert_eq!(
            invoke(handle, &[]).err(),
            Some(CallbackError::InvalidHandle(handle))
        );
    }

    #[test]
    fn revoke_returns_true_exactly_once() {
        let sig = CallbackSig::new(ValueType::Bool, &[]);
        let handle = register(sig, Arc::new(|_| Value::Bool(false))).unwrap();

        assert!(revoke(handle));
        assert!(!revoke(handle));
    }

    #[test]
    fn invoke_rejects_wrong_arity_and_types() {
        let expected = CallbackSig::new(ValueType::I32, &[ValueType::I32, ValueType::I32]);
        let handle = register(expected.clone(), Arc::new(|_| Value::I32(0))).unwrap();

        let arity_error = invoke(handle, &[Value::I32(1)]).unwrap_err();
        assert_eq!(
            arity_error,
            CallbackError::SignatureMismatch {
                expected: expected.clone(),
                got: vec![ValueType::I32],
            }
        );

        let type_error = invoke(handle, &[Value::I32(1), Value::I64(2)]).unwrap_err();
        assert_eq!(
            type_error,
            CallbackError::SignatureMismatch {
                expected,
                got: vec![ValueType::I32, ValueType::I64],
            }
        );
    }

    #[test]
    fn invoke_rejects_null_handle() {
        assert_eq!(
            invoke(Handle::NULL, &[]).err(),
            Some(CallbackError::InvalidHandle(Handle::NULL))
        );
        assert!(!revoke(Handle::NULL));
    }
}
