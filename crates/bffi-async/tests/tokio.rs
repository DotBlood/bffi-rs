//! Integration tests for the tokio runtime wiring (feature `tokio`).
//!
//! Completion of a tokio-spawned task is observed through
//! `pending_tasks` (the executor-agnostic live counter) and through
//! side effects the future performs before returning. Delivery to
//! resolver trampolines is exercised by the real bun:ffi e2e suite.
#![cfg(feature = "tokio")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bffi_async::{AsyncValue, cancel, pending_tasks, spawn_on_tokio};

fn ok(value: AsyncValue) -> Result<AsyncValue, bffi_core::BffiError> {
    Ok(value)
}

fn wait_for(what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn tokio_spawns_run_and_record_the_outcome() {
    let done = Arc::new(AtomicU32::new(0));
    for _ in 0..8 {
        let done = Arc::clone(&done);
        spawn_on_tokio(async move {
            done.fetch_add(1, Ordering::SeqCst);
            ok(AsyncValue::Unit)
        })
        .expect("task table has room");
    }
    wait_for("8 tokio tasks", || done.load(Ordering::SeqCst) == 8);
    wait_for("live count drops to zero", || pending_tasks() == 0);
}

#[test]
fn tokio_panicking_futures_are_recorded_as_failures() {
    struct Panicky;
    impl Future for Panicky {
        type Output = Result<AsyncValue, bffi_core::BffiError>;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            panic!("tokio boom");
        }
    }

    spawn_on_tokio(Panicky).expect("room");
    wait_for("panicking tokio task finished", || pending_tasks() == 0);
    // The executor (tokio runtime) survived: a healthy task still runs.
    let done = Arc::new(AtomicBool::new(false));
    let done2 = Arc::clone(&done);
    spawn_on_tokio(async move {
        done2.store(true, Ordering::SeqCst);
        ok(AsyncValue::Unit)
    })
    .expect("room");
    wait_for("healthy tokio task after the panic", || {
        done.load(Ordering::SeqCst)
    });
}

use std::pin::Pin;
use std::task::{Context, Poll};

#[test]
fn cancel_aborts_a_tokio_task_and_drops_the_future() {
    static INNER_DROPPED: AtomicBool = AtomicBool::new(false);
    struct DropOnAbort;
    impl Drop for DropOnAbort {
        fn drop(&mut self) {
            INNER_DROPPED.store(true, Ordering::SeqCst);
        }
    }

    struct PendingHeld(DropOnAbort);
    impl Future for PendingHeld {
        type Output = Result<AsyncValue, bffi_core::BffiError>;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    let task = spawn_on_tokio(PendingHeld(DropOnAbort)).expect("room");
    assert!(cancel(task), "cancel wins while the tokio task runs");
    wait_for("the aborted future dropped", || {
        INNER_DROPPED.load(Ordering::SeqCst)
    });
    wait_for("live count drops to zero", || pending_tasks() == 0);
}

#[test]
fn executor_mode_switch_routes_spawn_to_tokio() {
    bffi_async::set_executor_kind(bffi_async::ExecutorKind::Tokio);
    let done = Arc::new(AtomicBool::new(false));
    let done2 = Arc::clone(&done);
    bffi_async::spawn(async move {
        done2.store(true, Ordering::SeqCst);
        ok(AsyncValue::Unit)
    })
    .expect("room");
    wait_for("routed task executed on tokio", || {
        done.load(Ordering::SeqCst)
    });
    bffi_async::set_executor_kind(bffi_async::ExecutorKind::Internal);
    wait_for("internal executor idle again", || pending_tasks() == 0);
}
