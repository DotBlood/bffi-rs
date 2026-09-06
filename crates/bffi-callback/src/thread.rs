//! The JS-thread policy of the callback layer.
//!
//! P1 rule: a callback is invoked only on the JS thread; cross-thread
//! marshalling is deferred to P2 (the event-loop trampoline,
//! DESIGN.md §6.5). The process designates exactly ONE thread as the
//! JS thread: the first [`set_js_thread`] call binds its own thread,
//! and later attempts from any other thread are rejected. The binding
//! is process-global state held in a single lock-free atomic word.
//!
//! [`ensure_js_thread`] is the gate every invocation path must pass
//! (Task 5's `invoke` calls it). While the process has NOT bound a JS
//! thread it admits every caller, so pure-Rust usage and unit tests
//! need no binding ritual.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::CallbackError;

/// The bound JS thread's numeric id, or `0` while the process is
/// unbound.
///
/// Ids start at 1, so `0` is a safe unbound sentinel. The single
/// compare_exchange on first use is the whole synchronization story:
/// concurrent first binders race once and one word decides the winner.
static JS_THREAD: AtomicU64 = AtomicU64::new(0);

/// Source of per-thread numeric ids; starts at 0 so the first assigned
/// id is 1 and `0` stays free as the unbound sentinel.
static NEXT_THREAD_SEQ: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// This thread's stable numeric id, assigned on first use.
    static THREAD_SEQ: Cell<u64> = const { Cell::new(0) };
}

/// Binds the CURRENT thread as the process-wide JS thread.
///
/// The first call wins the binding. Later calls are idempotent from
/// the bound thread and rejected with [`CallbackError::WrongThread`]
/// from any other thread - a binding attempt from a wrong thread never
/// overwrites the existing binding.
pub fn set_js_thread() -> Result<(), CallbackError> {
    let id = current_thread_id();
    if JS_THREAD.load(Ordering::SeqCst) == 0 {
        // First binder wins the 0 -> id exchange; a loser either raced
        // another binder or re-observed its own binding - every path
        // falls through to the SAME bound-check as `ensure_js_thread`.
        let _ = JS_THREAD.compare_exchange(0, id, Ordering::SeqCst, Ordering::SeqCst);
    }
    ensure_js_thread()
}

/// Ensures the CURRENT thread is the bound JS thread.
///
/// - Unbound process: `Ok` (pure-Rust usage and unit tests).
/// - Bound and current == bound: `Ok`.
/// - Bound and current != bound: `Err(CallbackError::WrongThread)`.
pub fn ensure_js_thread() -> Result<(), CallbackError> {
    let bound = JS_THREAD.load(Ordering::SeqCst);
    if bound == 0 {
        return Ok(());
    }
    if bound == current_thread_id() {
        Ok(())
    } else {
        Err(CallbackError::WrongThread)
    }
}

/// Returns a stable nonzero numeric id for the calling thread.
///
/// `std::thread::ThreadId` has no stable numeric accessor
/// (`ThreadId::as_u64` is feature-gated `thread_id_value`), so ids are
/// assigned per thread on first use from a process-wide counter -
/// monotonic and never reused, matching `ThreadId` semantics.
fn current_thread_id() -> u64 {
    THREAD_SEQ.with(|seq| {
        let assigned = seq.get();
        if assigned == 0 {
            let fresh = NEXT_THREAD_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
            seq.set(fresh);
            fresh
        } else {
            assigned
        }
    })
}

#[cfg(test)]
mod tests {
    // NOTE (test isolation): the JS-thread binding is process-global.
    // This lib test binary must contain NO other binder: unit tests of
    // later tasks must NOT call `set_js_thread` here, so this single
    // test can rely on observing the UNBOUND state. Wrong-thread
    // behavior is covered by `tests/threading.rs`, a separate process.
    use super::ensure_js_thread;

    #[test]
    fn unbound_ensure_is_ok() {
        assert!(ensure_js_thread().is_ok());
    }
}
