//! Integration tests of the loop lifecycle.
//!
//! libtest runs every `#[test]` on its own thread, and `stop()` is
//! process-global and sticky - so the full enqueue/drain/stop
//! lifecycle lives in ONE sequential test with phases; the remaining
//! tests stay strictly stop-agnostic. A leaked runner (parked on the
//! condvar) never blocks process exit.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bffi_event_loop::{enqueue, executed_total, marshal, pending, pump, run, stop};

fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn loop_lifecycle() {
    // Phase 1: a runner drains FIFO-ordered jobs.
    let order = Arc::new(Mutex::new(VecDeque::new()));
    let (done_tx, done_rx) = std::sync::mpsc::channel::<u32>();
    for seq in 0_u32..8 {
        let order = Arc::clone(&order);
        let done_tx = done_tx.clone();
        enqueue(Box::new(move || {
            order.lock().unwrap().push_back(seq);
            done_tx.send(seq).expect("receiver alive");
        }))
        .expect("not stopped");
    }
    drop(done_tx);
    let runner = std::thread::spawn(run);
    let mut received: Vec<u32> = Vec::new();
    for seq in done_rx {
        received.push(seq);
    }
    assert_eq!(received, (0..8).collect::<Vec<_>>(), "FIFO order");
    assert_eq!(order.lock().unwrap().len(), 8);
    assert_eq!(pending(), 0, "all jobs drained");

    // Phase 2: concurrent producers through the same queue.
    let before = executed_total();
    const PRODUCERS: u64 = 4;
    const PER_PRODUCER: u64 = 32;
    let executed_counter = Arc::new(AtomicU32::new(0));
    std::thread::scope(|scope| {
        for _ in 0..PRODUCERS {
            let executed_counter = Arc::clone(&executed_counter);
            scope.spawn(move || {
                for _ in 0..PER_PRODUCER {
                    enqueue(Box::new(|| {})).expect("not stopped");
                    executed_counter.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    wait_for("producers' jobs", || {
        executed_total() - before >= PRODUCERS * PER_PRODUCER
    });
    assert_eq!(
        executed_counter.load(Ordering::Relaxed),
        (PRODUCERS * PER_PRODUCER) as u32,
        "every enqueue returned Ok"
    );

    // Phase 3: a panicking job becomes a stored last error and the
    // loop continues (run_extern_body catches in every profile).
    let (err_tx, err_rx) = std::sync::mpsc::channel::<bffi_core::BffiError>();
    enqueue(Box::new(|| panic!("loop boom"))).expect("not stopped");
    enqueue(Box::new(move || {
        let error = bffi_core::take_last_error().expect("the panic must be stored");
        err_tx.send(error).expect("receiver alive");
    }))
    .expect("not stopped");
    let panic_error = err_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("drained");
    assert_eq!(panic_error.code, bffi_core::ErrorCode::Panic);
    assert_eq!(panic_error.message, "loop boom");

    // Phase 4: marshal works while running; stop is sticky.
    marshal(Box::new(|| {})).expect("a runner is active");
    wait_for("marshalled job", || pending() == 0);

    // Phase 5: the runner (still inside run()) and two pump() threads
    // race for the same queue. A job is executed exactly once no
    // matter how the three drains interleave: the queue mutex hands
    // every job to exactly one popper, and only the popper runs it.
    const RACE_JOBS: u32 = 256;
    let race_counter = Arc::new(AtomicU32::new(0));
    let raced_before = executed_total();
    for _ in 0..RACE_JOBS {
        let race_counter = Arc::clone(&race_counter);
        enqueue(Box::new(move || {
            race_counter.fetch_add(1, Ordering::Relaxed);
        }))
        .expect("not stopped");
    }
    let pumps_done = Arc::new(AtomicBool::new(false));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let pumps_done = Arc::clone(&pumps_done);
            scope.spawn(move || {
                while !pumps_done.load(Ordering::Acquire) {
                    let _ = pump();
                }
            });
        }
        wait_for("the race jobs", || {
            race_counter.load(Ordering::Relaxed) == RACE_JOBS
        });
        pumps_done.store(true, Ordering::Release);
    });
    assert_eq!(
        race_counter.load(Ordering::Relaxed),
        RACE_JOBS,
        "every raced job executed exactly once (none lost, none doubled)"
    );
    assert_eq!(
        executed_total() - raced_before,
        u64::from(RACE_JOBS),
        "executed_total counts the raced jobs exactly"
    );

    stop();
    let executed = runner.join().expect("runner must not panic");
    assert!(executed >= 8, "this runner drained at least phase 1");

    assert!(matches!(
        enqueue(Box::new(|| {})),
        Err(bffi_event_loop::EventLoopError::Stopped)
    ));
    // marshal checks the runner first: with the runner gone (joined
    // above) the failure is NotRunning, not Stopped.
    assert!(matches!(
        marshal(Box::new(|| {})),
        Err(bffi_event_loop::EventLoopError::NotRunning)
    ));
    assert_eq!(run(), 0, "run after stop is a no-op");
    assert!(!bffi_event_loop::is_running());
}

#[test]
fn marshal_without_runner_is_not_running() {
    // Stop-agnostic: NotRunning fires regardless of the global state
    // (no runner in THIS test, and libtest owns the timing).
    if bffi_event_loop::is_running() {
        return; // another test's runner is alive; marshal would succeed
    }
    assert!(matches!(
        marshal(Box::new(|| {})),
        Err(bffi_event_loop::EventLoopError::NotRunning)
    ));
}
