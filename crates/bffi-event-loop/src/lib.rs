//! # bffi-event-loop
//!
//! The event loop abstraction of the bffi-rs framework (DESIGN §7:
//! "Start with `run()`; `pump()` is a mock for now").
//!
//! Native code cannot hook Bun's real event loop, so this crate is the
//! honest next thing: a thread-safe job queue plus a blocking drain -
//!
//! - background threads deliver work with [`enqueue`] / [`marshal`];
//! - one thread (typically the JS thread, started with
//!   `bffi_callback::set_js_thread()`) calls [`run`] and executes the
//!   jobs - each under [`bffi_core::run_extern_body`], so a panicking
//!   job becomes a stored last error and the loop lives on;
//! - [`stop`] ends the (sticky) loop; [`pump`] is a documented mock.
//!
//! The queue is a `Mutex<VecDeque>` + `Condvar` on purpose: the
//! lock-free machinery of P0 exists for handle tables (CAS traffic on
//! every FFI call); a job queue sees a handful of transitions per job
//! and does not need it (documented trade-off, see the README).
//!
//! ## Example
//!
//! ```
//! use std::sync::atomic::{AtomicU32, Ordering};
//!
//! static RESULT: AtomicU32 = AtomicU32::new(0);
//!
//! bffi_event_loop::enqueue(Box::new(|| {
//!     RESULT.store(41 + 1, Ordering::Relaxed);
//! }))
//! .expect("not stopped");
//! // No runner exists yet: the job is definitely still queued.
//! assert_eq!(bffi_event_loop::pending(), 1);
//!
//! let handle = std::thread::spawn(bffi_event_loop::run);
//! // Wait until the runner actually picked the job up (stop() is
//! // sticky - calling it before `run()` starts would skip the job).
//! while RESULT.load(Ordering::Relaxed) == 0 {
//!     std::thread::yield_now();
//! }
//! bffi_event_loop::stop();
//! assert_eq!(handle.join().ok(), Some(1));
//! assert_eq!(RESULT.load(Ordering::Relaxed), 42);
//! assert!(bffi_event_loop::enqueue(Box::new(|| {})).is_err());
//! ```

// The workspace restriction lints (expect/unwrap/panic) target production
// code; tests assert invariants and intentionally trigger panics.
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// A unit of work delivered to the loop; executed on the thread that
/// runs [`run`].
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// Everything that can go wrong at the event-loop surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventLoopError {
    /// No runner is active, so the job could not be delivered to the
    /// JS thread. Maps to `ErrorCode::WrongThread` - the caller asked
    /// for marshalling and there was no one to marshal to.
    NotRunning,
    /// The loop has been stopped; it is sticky and never restarts.
    /// Maps to `ErrorCode::Error`.
    Stopped,
}

impl fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning => write!(f, "no event loop is running to marshal onto"),
            Self::Stopped => write!(f, "the event loop has been stopped"),
        }
    }
}

impl std::error::Error for EventLoopError {}

/// Unified-format conversion: a marshal attempt without a runner maps
/// to the dedicated `WrongThread` code (P2), a stop to `Error`; the
/// domain error is preserved as the source.
impl From<EventLoopError> for bffi_core::BffiError {
    fn from(error: EventLoopError) -> Self {
        use bffi_core::{BffiError, ErrorCode};
        let code = match &error {
            EventLoopError::NotRunning => ErrorCode::WrongThread,
            EventLoopError::Stopped => ErrorCode::Error,
        };
        BffiError::with_source(code, error.to_string(), error)
    }
}

static STOPPED: AtomicBool = AtomicBool::new(false);
static RUNNERS: AtomicUsize = AtomicUsize::new(0);
static EXECUTED: AtomicU64 = AtomicU64::new(0);

fn queue() -> &'static (Mutex<VecDeque<Job>>, Condvar) {
    static QUEUE: std::sync::OnceLock<(Mutex<VecDeque<Job>>, Condvar)> = std::sync::OnceLock::new();
    QUEUE.get_or_init(|| (Mutex::new(VecDeque::new()), Condvar::new()))
}

/// Locks the queue, recovering from a poisoned mutex: jobs never run
/// while the lock is held (they execute outside it), so poisoning is
/// theoretically impossible - recovery keeps that claim airtight.
fn lock_queue() -> MutexGuard<'static, VecDeque<Job>> {
    queue().0.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Puts `job` on the queue for the active runner.
///
/// Deliberately infallible w.r.t. capacity (the queue is unbounded)
/// and callable from any thread.
///
/// # Errors
///
/// [`EventLoopError::Stopped`] once [`stop`] has been called (sticky).
pub fn enqueue(job: Job) -> Result<(), EventLoopError> {
    if STOPPED.load(Ordering::Acquire) {
        return Err(EventLoopError::Stopped);
    }
    lock_queue().push_back(job);
    queue().1.notify_one();
    Ok(())
}

/// Marshals `job` onto the running loop: the wrong-thread delivery
/// path (criterion 5.3, P2 half - the P1 half was the reject inside
/// `bffi_callback::invoke`).
///
/// # Errors
///
/// [`EventLoopError::NotRunning`] when no runner is active - the
/// caller decides how to surface it (retry, reject, log); the
/// conversion to [`bffi_core::BffiError`] maps it to
/// `ErrorCode::WrongThread`. [`EventLoopError::Stopped`] like
/// [`enqueue`].
pub fn marshal(job: Job) -> Result<(), EventLoopError> {
    if RUNNERS.load(Ordering::Acquire) == 0 {
        return Err(EventLoopError::NotRunning);
    }
    enqueue(job)
}

/// Blocks the calling thread and drains the queue until [`stop`] is
/// called. Every job executes under [`bffi_core::run_extern_body`]: a
/// panic becomes a stored last error and the loop continues.
///
/// The calling thread is the executor. If the jobs call back into
/// `bffi_callback::invoke`, bind this thread as the JS thread first
/// (`set_js_thread`); this crate never binds automatically - the first
/// binder wins process-wide, and an automatic bind from a worker would
/// be wrong with no way to undo it.
///
/// Nested/parallel `run` calls are allowed but discouraged: additional
/// runners only drain the queue and exit once it is empty, while the
/// first runner keeps waiting for `stop`. `run` after `stop` returns
/// `0` immediately (stop is sticky). The return value is the number of
/// jobs executed by THIS runner (panicking jobs included).
#[must_use]
pub fn run() -> u64 {
    if STOPPED.load(Ordering::Acquire) {
        return 0;
    }
    RUNNERS.fetch_add(1, Ordering::AcqRel);
    let primary = RUNNERS.load(Ordering::Acquire) == 1;
    let mut executed = 0_u64;
    loop {
        let job = {
            let mut q = lock_queue();
            loop {
                if let Some(job) = q.pop_front() {
                    break Some(job);
                }
                if STOPPED.load(Ordering::Acquire) {
                    break None;
                }
                // Additional runners are drain-only: they exit on an
                // empty queue instead of waiting forever.
                if !primary {
                    break None;
                }
                q = queue().1.wait(q).unwrap_or_else(PoisonError::into_inner);
            }
        };
        let Some(job) = job else { break };
        executed += 1;
        EXECUTED.fetch_add(1, Ordering::Relaxed);
        let _ = bffi_core::boundary::run_extern_body(|| {
            job();
            bffi_core::ErrorCode::Ok
        });
    }
    RUNNERS.fetch_sub(1, Ordering::AcqRel);
    executed
}

/// Stops the loop for good: wakes every runner (the current job
/// finishes, queued jobs stay queued), makes [`enqueue`] fail with
/// [`EventLoopError::Stopped`] and turns [`run`] into a no-op.
/// Idempotent.
pub fn stop() {
    STOPPED.store(true, Ordering::Release);
    queue().1.notify_all();
}

/// Number of jobs waiting for execution (a monotonic snapshot, for
/// monitoring and tests).
#[must_use]
pub fn pending() -> u64 {
    lock_queue().len() as u64
}

/// Number of jobs executed since process start, across all runners
/// (for monitoring and tests).
#[must_use]
pub fn executed_total() -> u64 {
    EXECUTED.load(Ordering::Relaxed)
}

/// Whether at least one runner is inside [`run`].
#[must_use]
pub fn is_running() -> bool {
    RUNNERS.load(Ordering::Acquire) > 0
}

/// MOCK per DESIGN §7: a non-blocking drain attempt. Always returns
/// `0` and executes nothing; the real integration with Bun's tick
/// arrives in a later phase. Callers must not build behavior on it.
#[must_use]
pub fn pump() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pump_is_the_documented_mock() {
        assert_eq!(pump(), 0);
    }

    #[test]
    fn display_texts_are_stable() {
        assert_eq!(
            EventLoopError::NotRunning.to_string(),
            "no event loop is running to marshal onto"
        );
        assert_eq!(
            EventLoopError::Stopped.to_string(),
            "the event loop has been stopped"
        );
    }

    #[test]
    fn errors_map_to_wrong_thread_and_error_codes() {
        use bffi_core::{BffiError, ErrorCode};
        let not_running = BffiError::from(EventLoopError::NotRunning);
        assert_eq!(not_running.code, ErrorCode::WrongThread);
        assert!(not_running.source.is_some());

        let stopped = BffiError::from(EventLoopError::Stopped);
        assert_eq!(stopped.code, ErrorCode::Error);
        assert!(stopped.source.is_some());
    }

    #[test]
    fn empty_queue_reports_zero_pending() {
        // Nothing in this binary's unit tests enqueues, so the
        // process-wide queue is empty; the full lifecycle with real
        // enqueue/drain lives in tests/loop.rs.
        assert_eq!(pending(), 0);
    }
}
