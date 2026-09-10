//! Concurrency stress for the callback lifecycle (criterion 5.2):
//! `revoke` racing parallel `invoke` traffic must never resurrect the
//! handle.
//!
//! Why the race is safe: an invoker's `invoke` call either lands BEFORE
//! the revocation (the slot is still live, so the call returns `Ok` -
//! or a clean domain error such as `SignatureMismatch`) or AFTER it
//! (the registry slot is gone, so the call returns
//! `CallbackError::InvalidHandle`). A call must never succeed against
//! a revoked handle: revocation is terminal, and the generational
//! handle scheme keeps the stale value dead even if its slot is later
//! reused.
//!
//! Isolation model: this binary never calls `set_js_thread`, so the
//! process stays UNBOUND and `invoke` admits every thread (the
//! pure-Rust usage mode, see `src/thread.rs`).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use bffi::bffi_callback::{CallbackError, CallbackSig, Value, ValueType, invoke, register, revoke};

/// The callback returns this constant on every successful call.
const SUM: i32 = 7;

const INVOKERS: usize = 8;

#[test]
fn revoker_racing_invokers_never_resurrects() {
    let sig = CallbackSig::new(ValueType::I32, &[ValueType::I32]);
    let handle = register(sig, Arc::new(|_| Value::I32(SUM))).expect("table has room");

    let done = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(Barrier::new(INVOKERS + 1));

    thread::scope(|scope| {
        let mut joins = Vec::new();
        for _ in 0..INVOKERS {
            let done = Arc::clone(&done);
            let barrier = Arc::clone(&barrier);
            joins.push(scope.spawn(move || {
                barrier.wait();
                // Fire-and-forget traffic: every call either lands
                // before the revoke (Ok) or after it (InvalidHandle).
                while !done.load(Ordering::Acquire) {
                    let _ = invoke(handle, &[Value::I32(1)]);
                }
            }));
        }

        // Release the invokers, revoke ONCE under full racing load,
        // then stop the traffic (Release pairs with Acquire above).
        barrier.wait();
        assert!(revoke(handle));
        done.store(true, Ordering::Release);

        for join in joins {
            // Joining asserts no invoker panicked mid-race.
            join.join().expect("invoker thread must not panic");
        }
    });

    // Revocation is terminal: the exact handle stays dead for good.
    let dead = handle;
    assert_eq!(
        invoke(dead, &[]).err(),
        Some(CallbackError::InvalidHandle(dead))
    );
}
