//! Integration tests for the register / invoke / revoke lifecycle
//! (kanboard 5.1 + 5.2).
//!
//! Isolation model: the callback type tags (`0x0200` native, `0x0201`
//! JS) are CRATE-OWNED constants - tests never reserve their own tags.
//! Both tables are declared once per process through the memoized
//! initializer on the first `register` / `invoke` / `revoke` call, so
//! every test in this binary (and in the lib test binary) shares the
//! same two registry tables. Each test registers its own callback and
//! owns the returned handle, so tests stay order-independent and
//! parallel-safe.
//!
//! This binary never calls `set_js_thread`: the process stays UNBOUND,
//! which is exactly the pure-Rust usage mode - `invoke` admits every
//! thread while no JS thread is bound (see `src/thread.rs`).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use std::thread;

use bffi_callback::{
    CallbackError, CallbackSig, Value, ValueType, bind_js_callback, invoke, js_callback, register,
    revoke,
};

/// The signature of the shared test callback: `i32(i32, i32)`.
fn sum_sig() -> CallbackSig {
    CallbackSig::new(ValueType::I32, &[ValueType::I32, ValueType::I32])
}

/// The shared test callback body: sums the `i32` arguments.
fn sum_body(args: &[Value]) -> Value {
    let mut sum = 0;
    for arg in args {
        if let Value::I32(n) = arg {
            sum += *n;
        }
    }
    Value::I32(sum)
}

#[test]
fn lifecycle_register_invoke_revoke() {
    let handle = register(sum_sig(), Arc::new(sum_body)).unwrap();

    let result = invoke(handle, &[Value::I32(2), Value::I32(3)]).unwrap();
    assert_eq!(result, Value::I32(5));

    assert!(revoke(handle));
}

#[test]
fn revoke_revokes_exactly_once() {
    let handle = register(sum_sig(), Arc::new(sum_body)).unwrap();

    assert!(revoke(handle));
    assert!(!revoke(handle));
}

#[test]
fn invoke_after_revoke_is_invalid_handle() {
    let handle = register(sum_sig(), Arc::new(sum_body)).unwrap();

    assert!(revoke(handle));
    assert_eq!(
        invoke(handle, &[Value::I32(1), Value::I32(1)]).err(),
        Some(CallbackError::InvalidHandle(handle))
    );
}

#[test]
fn signature_mismatch_reports_expected_and_got() {
    let expected = sum_sig();
    let handle = register(expected.clone(), Arc::new(sum_body)).unwrap();

    let arity_error = invoke(handle, &[Value::I32(1)]).unwrap_err();
    assert_eq!(
        arity_error,
        CallbackError::SignatureMismatch {
            expected: expected.clone(),
            got: vec![ValueType::I32],
        }
    );

    let type_error = invoke(handle, &[Value::I64(1), Value::I32(2)]).unwrap_err();
    assert_eq!(
        type_error,
        CallbackError::SignatureMismatch {
            expected,
            got: vec![ValueType::I64, ValueType::I32],
        }
    );
}

#[test]
fn invoke_works_from_any_thread_while_unbound() {
    let handle = register(sum_sig(), Arc::new(sum_body)).unwrap();

    thread::scope(|scope| {
        let mut joins = Vec::new();
        for _ in 0..4 {
            joins.push(scope.spawn(|| {
                let result = invoke(handle, &[Value::I32(1), Value::I32(2)]).unwrap();
                assert_eq!(result, Value::I32(3));
            }));
        }
        for join in joins {
            join.join().unwrap();
        }
    });
}

#[test]
fn bind_js_callback_lifecycle() {
    let sig = CallbackSig::new(ValueType::Bool, &[ValueType::I32]);
    let ptr = 0xfeed_face_usize;
    let handle = bind_js_callback(sig, ptr).unwrap();

    let info = js_callback(handle).unwrap();
    assert_eq!(
        info.sig,
        CallbackSig::new(ValueType::Bool, &[ValueType::I32])
    );
    assert_eq!(info.ptr, ptr);

    assert!(revoke(handle));
    assert_eq!(
        js_callback(handle).err(),
        Some(CallbackError::InvalidHandle(handle))
    );

    assert_eq!(
        invoke(handle, &[]).err(),
        Some(CallbackError::InvalidHandle(handle)),
        "a js slot must not respond to invoke (type routing)"
    );
}
