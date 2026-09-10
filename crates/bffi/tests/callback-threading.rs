//! Integration tests for the process-global JS-thread binding.
//!
//! The binding is PROCESS-GLOBAL state: the first `set_js_thread`
//! call binds its own thread, repeats from the bound thread are
//! idempotent, and every other thread is rejected with
//! `CallbackError::WrongThread`.
//!
//! Isolation strategy: libtest runs every `#[test]` on its own thread,
//! so a test body cannot bind "the main test thread" - two tests would
//! bind two different harness threads and each would reject the other
//! (observed empirically before this harness was adopted). This binary
//! therefore funnels every call that must observe or establish the
//! binding through ONE dedicated helper thread - the test stand-in for
//! the JS thread, bound on first use - while foreign-thread rejection
//! is asserted from plain spawned threads. Tests stay
//! order-independent and parallel-safe: whichever test runs first
//! establishes the single, idempotent binding through the helper, and
//! every foreign-thread spawn happens only after a completed helper
//! call, so the binding always exists by then.
//!
//! The "unbound -> Ok" half of the contract is NOT covered here (the
//! binding may already exist by the time these tests run); it is
//! asserted by the `unbound_ensure_is_ok` unit test in
//! `src/thread.rs`, whose lib test binary has no other binder.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::thread;

use bffi::bffi_callback::{CallbackError, ensure_js_thread, set_js_thread};

type Job = Box<dyn FnOnce() + Send>;

/// Runs `f` on the shared JS-thread stand-in and waits for its result.
fn on_js_thread<R>(f: impl FnOnce() -> R + Send + 'static) -> R
where
    R: Send + 'static,
{
    static JS_THREAD: OnceLock<Sender<Job>> = OnceLock::new();
    let sender = JS_THREAD.get_or_init(|| {
        let (sender, receiver) = channel::<Job>();
        thread::Builder::new()
            .name("bffi-js-thread".to_owned())
            .spawn(move || {
                set_js_thread().expect("the JS-thread stand-in must bind itself");
                for job in receiver {
                    job();
                }
            })
            .expect("spawning the JS-thread stand-in must not fail");
        sender
    });
    let (reply_sender, reply_receiver) = channel();
    sender
        .send(Box::new(move || {
            let _ = reply_sender.send(f());
        }))
        .expect("the JS-thread stand-in must be alive");
    reply_receiver
        .recv()
        .expect("the JS-thread stand-in must answer")
}

#[test]
fn set_js_thread_is_idempotent_on_the_bound_thread() {
    on_js_thread(set_js_thread).unwrap();
    on_js_thread(set_js_thread).unwrap();
    on_js_thread(ensure_js_thread).unwrap();
}

#[test]
fn foreign_thread_is_rejected() {
    // Establish the binding first (blocking): from here on, foreign
    // threads must observe a BOUND process, never an unbound one.
    on_js_thread(set_js_thread).unwrap();

    // Invocation from a foreign thread must be rejected...
    let invoked = thread::spawn(ensure_js_thread).join().unwrap();
    assert_eq!(invoked.err(), Some(CallbackError::WrongThread));

    // ...and so must a binding ATTEMPT from a foreign thread.
    let rebound = thread::spawn(set_js_thread).join().unwrap();
    assert_eq!(rebound.err(), Some(CallbackError::WrongThread));

    // The binding is unchanged: the bound thread still admits itself.
    on_js_thread(ensure_js_thread).unwrap();
}
