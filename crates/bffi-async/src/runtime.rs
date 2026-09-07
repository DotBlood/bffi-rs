//! The tokio runtime wiring (opt-in, feature `tokio`).
//!
//! A lazily initialized multi-thread runtime polls futures spawned via
//! [`spawn_on_tokio`](crate::spawn_on_tokio); completion is recorded on
//! the task record and delivered through the event loop exactly like
//! the built-in executor's path. The Bun-support invariant is
//! unchanged: tokio threads never touch JavaScript.
//!
//! Panics inside a future are caught by the [`CatchPanic`] adapter
//! before tokio sees them - a tokio task panic would bypass our
//! outcome recording (the wrapper never resumes) and leak the live
//! task counter.

use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::{Context, Poll};

use bffi_core::Handle;

use crate::executor::{self, BoxFuture};
use crate::task;

/// Hard cap on tokio worker threads.
const MAX_TOKIO_WORKERS: usize = 8;

fn runtime() -> Result<&'static tokio::runtime::Runtime, crate::AsyncError> {
    static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
    let initialized = RUNTIME.get_or_init(|| {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .clamp(1, MAX_TOKIO_WORKERS);
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .enable_all()
            .build()
            .map_err(|error| error.to_string())
    });
    match initialized {
        Ok(runtime) => Ok(runtime),
        Err(message) => {
            bffi_core::set_last_error(bffi_core::BffiError::new(
                bffi_core::ErrorCode::Error,
                format!("tokio runtime is unavailable: {message}"),
            ));
            Err(crate::AsyncError::RuntimeUnavailable)
        }
    }
}

/// The executor mode selected with
/// [`set_executor_kind`](crate::set_executor_kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExecutorKind {
    /// The built-in N-worker executor (the default).
    #[default]
    Internal,
    /// A tokio multi-thread runtime (feature `tokio`).
    Tokio,
}

static EXECUTOR_KIND: AtomicU8 = AtomicU8::new(0);

const KIND_INTERNAL: u8 = 0;
const KIND_TOKIO: u8 = 1;

pub(crate) fn set_kind(kind: ExecutorKind) {
    let raw = match kind {
        ExecutorKind::Internal => KIND_INTERNAL,
        ExecutorKind::Tokio => KIND_TOKIO,
    };
    EXECUTOR_KIND.store(raw, Ordering::Release);
}

pub(crate) fn kind() -> ExecutorKind {
    match EXECUTOR_KIND.load(Ordering::Acquire) {
        KIND_TOKIO => ExecutorKind::Tokio,
        _ => ExecutorKind::Internal,
    }
}

/// A future adapter that converts a panic inside the inner future into
/// `Err(BffiError)` (code `Panic`), mirroring the boundary policy.
pub(crate) struct CatchPanic<F> {
    inner: Pin<Box<F>>,
}

impl<F> CatchPanic<F> {
    pub(crate) fn new(future: F) -> Self {
        Self {
            inner: Box::pin(future),
        }
    }
}

impl<F: Future> Future for CatchPanic<F> {
    type Output = Result<F::Output, bffi_core::BffiError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY (assertion): the panic payload is only observed via
        // `panic_message` (a fresh `BffiError`), which touches none of
        // the future's state; the future itself is resumed or dropped
        // by the caller.
        match catch_unwind(AssertUnwindSafe(|| self.inner.as_mut().poll(cx))) {
            Ok(Poll::Ready(outcome)) => Poll::Ready(Ok(outcome)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(bffi_core::BffiError::new(
                bffi_core::ErrorCode::Panic,
                bffi_core::panic_message(&*payload),
            ))),
        }
    }
}

/// Spawns `future` on the tokio runtime.
///
/// The wrapper records the outcome on the task record (winning the
/// same first-transition-wins protocol as the built-in executor) and
/// enqueues the delivery; an aborter closure registered on the handle
/// lets `cancel` abort the tokio task immediately.
pub(crate) fn launch_on_tokio(handle: Handle, future: BoxFuture) {
    let Ok(runtime) = runtime() else {
        // Sticky runtime failure: record the failure so `cancel`/pending
        // bookkeeping stays consistent (the live count drops).
        executor::task_finished(handle);
        return;
    };

    let join = runtime.spawn(async move {
        let Some(record) = task::record(handle) else {
            executor::task_finished(handle);
            return;
        };
        let outcome = CatchPanic::new(future).await;
        let won = match outcome {
            Ok(Ok(value)) => record.mark_completed(value),
            Ok(Err(error)) => record.mark_failed(error),
            Err(error) => record.mark_failed(error),
        };
        if won {
            executor::task_finished(handle);
            executor::try_deliver(&record);
        }
        // Lost transition (cancel won earlier): cancel already ran the
        // terminal bookkeeping and delivered.
    });

    // `JoinHandle::abort` takes `&self`, so the closure keeps the
    // handle alive and aborts on call.
    executor::register_aborter(handle, Box::new(move || join.abort()));
}

/// Whether `spawn` should route to tokio (the process-global mode).
pub(crate) fn spawn_routes_to_tokio() -> bool {
    kind() == ExecutorKind::Tokio
}
