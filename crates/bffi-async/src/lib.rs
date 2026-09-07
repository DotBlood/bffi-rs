//! # bffi-async
//!
//! Promise / async support for the bffi-rs framework (DESIGN §8, P3):
//! spawn Rust futures from JavaScript as Promises, with cancellation
//! and timeouts - built **on top of the event loop**.
//!
//! ## Threading model (the Bun-support invariant)
//!
//! Three roles, never mixed:
//!
//! 1. the **JS thread** - the only place legal to invoke resolver
//!    callbacks; it also drains the event loop (`pump()` / `run()`);
//! 2. **executor workers** (default: machine parallelism, capped at
//!    4) - poll the spawned futures, never touch JavaScript;
//! 3. the **timer thread** - deadlines for
//!    [`sleep`] / [`timeout`].
//!
//! A completed future is delivered by enqueuing a job onto the event
//! loop; the JS thread executes it while draining. Consequence:
//! **promises resolve only when the JS side drains the loop** (the
//! documented loader pattern is a periodic `pump()`), and stopping the
//! loop with tasks in flight loses their delivery.
//!
//! ## Cancellation and timeouts
//!
//! - [`cancel`] is cooperative: the future is dropped at the next
//!   poll boundary; blocking code inside it is not interrupted.
//! - [`timeout`] wraps a future with a deadline; on expiry the inner
//!   future is dropped and the task fails with the message
//!   "task timed out".
//!
//! ## Tokio (opt-in)
//!
//! With the `tokio` feature (`bffi-async = { features = ["tokio"] }`)
//! [`spawn_on_tokio`] polls futures on a tokio runtime instead of the
//! built-in workers - sockets and tokio timers work; the Bun-support
//! invariant is unchanged because tokio threads never touch JS.
//!
//! ## Example
//!
//! ```
//! use bffi_async::{spawn, AsyncValue};
//! use std::future::Future;
//! use std::pin::Pin;
//! use std::task::{Context, Poll};
//!
//! /// A future that stays pending forever (nothing wakes it).
//! struct PendingForever;
//!
//! impl Future for PendingForever {
//!     type Output = Result<AsyncValue, bffi_core::BffiError>;
//!     fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
//!         Poll::Pending
//!     }
//! }
//!
//! // Spawn registers the task; a cancel drops the future at the next
//! // poll boundary (here: immediately - the future never completes).
//! let task = spawn(PendingForever).expect("task table has room");
//! assert!(bffi_async::cancel(task));
//! assert_eq!(bffi_async::pending_tasks(), 0);
//! ```

// The workspace restriction lints (expect/unwrap/panic) target production
// code; tests assert invariants and intentionally trigger panics.
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod value;

mod executor;
mod task;
mod timer;

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use bffi_core::Handle;

pub use timer::Sleep;
pub use value::AsyncValue;

/// Everything that can go wrong at the async surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsyncError {
    /// The task table has no free slots.
    TableFull,
    /// The crate-owned tag is already declared (sticky init failure).
    TagInUse,
    /// The handle is null, stale or foreign.
    InvalidHandle,
    /// A resolver pair is already attached to this task.
    AlreadyAttached,
}

impl std::fmt::Display for AsyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TableFull => write!(f, "the bffi-async task table is full"),
            Self::TagInUse => write!(f, "the bffi-async task tag is already declared"),
            Self::InvalidHandle => write!(f, "unknown or stale task handle"),
            Self::AlreadyAttached => {
                write!(f, "a resolver pair is already attached to this task")
            }
        }
    }
}

impl std::error::Error for AsyncError {}

/// Unified-format conversion on existing codes; the domain error is
/// preserved as the source.
impl From<AsyncError> for bffi_core::BffiError {
    fn from(error: AsyncError) -> Self {
        use bffi_core::ErrorCode;
        let code = match &error {
            AsyncError::TableFull => ErrorCode::TableFull,
            AsyncError::TagInUse => ErrorCode::InvalidTag,
            AsyncError::InvalidHandle => ErrorCode::InvalidHandle,
            AsyncError::AlreadyAttached => ErrorCode::InvalidArgument,
        };
        bffi_core::BffiError::with_source(code, error.to_string(), error)
    }
}

/// Spawns a future on the built-in executor (N workers, N = machine
/// parallelism capped at 4).
///
/// The future must not block indefinitely: workers are shared. The
/// returned handle cancels ([`cancel`]) or attaches resolvers
/// ([`attach`]).
///
/// # Errors
///
/// [`AsyncError::TableFull`] when the task table has no free slot, or
/// [`AsyncError::TagInUse`] if table initialization ever failed.
pub fn spawn<F>(future: F) -> Result<Handle, AsyncError>
where
    F: Future<Output = Result<AsyncValue, bffi_core::BffiError>> + Send + 'static,
{
    executor::launch(Box::pin(future)).map_err(|error| match error {
        bffi_core::RegistryError::TableFull(_) => AsyncError::TableFull,
        _ => AsyncError::TagInUse,
    })
}

/// Spawns a future on a tokio runtime (requires the `tokio` feature)
/// instead of the built-in workers: sockets and tokio timers work,
/// while the Bun-support invariant stays intact - completion is still
/// delivered through the event loop on the JS thread.
///
/// # Errors
///
/// The same as [`spawn`].
#[cfg(feature = "tokio")]
pub fn spawn_on_tokio<F>(future: F) -> Result<Handle, AsyncError>
where
    F: Future<Output = Result<AsyncValue, bffi_core::BffiError>> + Send + 'static,
{
    let _ = future;
    Err(AsyncError::TableFull) // wired in the tokio follow-up of T2
}

/// Requests cancellation of the task behind `handle`.
///
/// Cooperative: the future is dropped at the next poll boundary. The
/// attached promise (if any) is rejected with "task cancelled".
/// Returns `false` when the handle is invalid or the task already
/// reached a terminal state.
pub fn cancel(handle: Handle) -> bool {
    let Some(record) = task::record(handle) else {
        return false;
    };
    if !record.mark_cancelled() {
        return false;
    }
    // The cancel won the transition: it owns the terminal bookkeeping
    // (the live-count drop and the parked-future drop).
    executor::task_finished(handle);
    executor::try_deliver(&record);
    true
}

/// Attaches the resolve/reject trampoline pair to a task.
///
/// `resolve` and `reject` are the raw pointer values of bun:ffi
/// `JSCallback` objects declared as `(u64) -> void` (resolve: the
/// value handle) and `(cstring) -> void` (reject: the message).
///
/// Attaching after completion delivers the stored outcome on the next
/// loop drain; a second attach is rejected.
///
/// # Errors
///
/// [`AsyncError::InvalidHandle`] or
/// [`AsyncError::AlreadyAttached`].
pub fn attach(handle: Handle, resolve: usize, reject: usize) -> Result<(), AsyncError> {
    let Some(record) = task::record(handle) else {
        return Err(AsyncError::InvalidHandle);
    };
    record
        .attach(resolve, reject)
        .map_err(|error| match error {
            task::AttachError::AlreadyAttached => AsyncError::AlreadyAttached,
            task::AttachError::InvalidHandle => AsyncError::InvalidHandle,
        })?;
    executor::try_deliver(&record);
    Ok(())
}

/// The number of tasks that are spawned but not finished (or
/// cancelled) yet.
#[must_use]
pub fn pending_tasks() -> u64 {
    executor::live_tasks()
}

/// A future that completes after `dur` (no value).
#[must_use]
pub fn sleep(dur: Duration) -> Sleep {
    Sleep::new(dur)
}

/// The timeout error: the inner future was dropped because its
/// deadline expired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task timed out")
    }
}

impl std::error::Error for TimeoutError {}

/// Wraps a future with a deadline: on expiry the inner future is
/// dropped (cancellation) and the output is
/// `Err(BffiError)` with the message "task timed out".
#[must_use]
pub fn timeout<F>(dur: Duration, future: F) -> Timeout<F>
where
    F: Future<Output = Result<AsyncValue, bffi_core::BffiError>>,
{
    Timeout {
        inner: Box::pin(future),
        deadline: std::time::Instant::now() + dur,
        registration: None,
    }
}

/// The [`timeout`] combinator future.
pub struct Timeout<F> {
    inner: Pin<Box<F>>,
    deadline: std::time::Instant,
    registration: Option<timer::Registration>,
}

impl<F> Future for Timeout<F>
where
    F: Future<Output = Result<AsyncValue, bffi_core::BffiError>>,
{
    type Output = Result<AsyncValue, bffi_core::BffiError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(registration) = &self.registration
            && registration.is_fired()
        {
            return std::task::Poll::Ready(Err(bffi_core::BffiError::new(
                bffi_core::ErrorCode::Error,
                TimeoutError.to_string(),
            )));
        }
        match self.inner.as_mut().poll(cx) {
            std::task::Poll::Ready(outcome) => std::task::Poll::Ready(outcome),
            std::task::Poll::Pending => match &self.registration {
                Some(registration) => {
                    registration.update_waker(cx.waker().clone());
                    if registration.is_fired() {
                        return std::task::Poll::Ready(Err(bffi_core::BffiError::new(
                            bffi_core::ErrorCode::Error,
                            TimeoutError.to_string(),
                        )));
                    }
                    std::task::Poll::Pending
                }
                None => {
                    self.registration = Some(timer::register(self.deadline, cx.waker().clone()));
                    // Lost-wakeup guard: the timer may have fired
                    // between the registration and the first check. A
                    // fired entry needs no bookkeeping - drop it.
                    let fired = self
                        .registration
                        .as_ref()
                        .is_some_and(|registration| registration.is_fired());
                    if fired {
                        self.registration = None;
                        return std::task::Poll::Ready(Err(bffi_core::BffiError::new(
                            bffi_core::ErrorCode::Error,
                            TimeoutError.to_string(),
                        )));
                    }
                    std::task::Poll::Pending
                }
            },
        }
    }
}
