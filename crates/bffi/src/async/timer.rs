//! The timer thread: deadlines for [`sleep`](super::sleep) and
//! [`timeout`](super::timeout).
//!
//! One background thread owns a deadline table; producers register
//! `(deadline, waker)` entries, the thread wakes due entries on tick
//! and parks between them. v1 scans the table on every tick - entry
//! counts per process are small; a heap/wheel would be premature.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A per-future shared fired-flag: the timer thread sets it, the
/// future's poll observes it through the same atomics.
#[derive(Default)]
pub(crate) struct SharedFired {
    fired: AtomicBool,
}

impl SharedFired {
    pub(crate) fn fire(&self) {
        self.fired.store(true, Ordering::Release);
    }

    pub(crate) fn is_fired(&self) -> bool {
        self.fired.load(Ordering::Acquire)
    }
}

struct Entry {
    deadline: Instant,
    waker: std::task::Waker,
    fired: Arc<SharedFired>,
    /// Set when the owner detached (timeout raced / dropped) - the
    /// timer thread must not wake a dead entry.
    detached: Arc<AtomicBool>,
}

struct TimerState {
    entries: Mutex<HashMap<u64, Entry>>,
    signal: Condvar,
    next_seq: AtomicU64,
}

fn timers() -> &'static TimerState {
    static TIMERS: OnceLock<TimerState> = OnceLock::new();
    TIMERS.get_or_init(|| TimerState {
        entries: Mutex::new(HashMap::new()),
        signal: Condvar::new(),
        next_seq: AtomicU64::new(1),
    })
}

/// The timer thread: parks until the next deadline or a new
/// registration, wakes due entries.
fn timer_loop() {
    loop {
        let state = timers();
        let mut entries = state
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        let mut due: Vec<u64> = Vec::new();
        let mut earliest: Option<Instant> = None;
        for (seq, entry) in entries.iter() {
            if entry.deadline <= now {
                due.push(*seq);
            } else {
                earliest = Some(match earliest {
                    Some(known) if known <= entry.deadline => known,
                    _ => entry.deadline,
                });
            }
        }
        for seq in &due {
            if let Some(entry) = entries.remove(seq)
                && !entry.detached.load(Ordering::Acquire)
            {
                entry.fired.fire();
                entry.waker.wake();
            }
        }
        match earliest {
            // Parks until the earliest deadline or a new registration
            // (notify_all) - whichever comes first.
            Some(deadline) => {
                let wait = deadline.saturating_duration_since(Instant::now());
                let _unused = state
                    .signal
                    .wait_timeout_while(entries, wait, |_| false)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            None => {
                let _unused = state
                    .signal
                    .wait(entries)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
}

/// A registered deadline: keep it while interested, drop it to detach.
pub(crate) struct Registration {
    seq: u64,
    fired: Arc<SharedFired>,
    detached: Arc<AtomicBool>,
}

impl Registration {
    /// Whether the deadline already fired.
    pub(crate) fn is_fired(&self) -> bool {
        self.fired.is_fired()
    }

    /// The waker slot for re-registration on later polls.
    pub(crate) fn update_waker(&self, waker: std::task::Waker) {
        let mut entries = timers()
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = entries.get_mut(&self.seq) {
            entry.waker = waker;
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.detached.store(true, Ordering::Release);
        let _unused = timers()
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.seq);
        timers().signal.notify_all();
    }
}

/// Registers a one-shot deadline for the given waker.
pub(crate) fn register(deadline: Instant, waker: std::task::Waker) -> Registration {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_ok() {
        let _unused = std::thread::Builder::new()
            .name("bffi-async-timer".to_owned())
            .spawn(timer_loop);
    }

    let seq = timers().next_seq.fetch_add(1, Ordering::AcqRel);
    let registration = Registration {
        seq,
        fired: Arc::new(SharedFired::default()),
        detached: Arc::new(AtomicBool::new(false)),
    };
    let _unused = timers()
        .entries
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            seq,
            Entry {
                deadline,
                waker,
                fired: Arc::clone(&registration.fired),
                detached: Arc::clone(&registration.detached),
            },
        );
    timers().signal.notify_all();
    registration
}

/// A future that completes after `dur`.
///
/// Implemented over the timer thread: no I/O reactor involved. Under
/// the `tokio` feature [`super::sleep`] switches to
/// `tokio::time::sleep` instead.
pub struct Sleep {
    deadline: Instant,
    registration: Option<Registration>,
}

impl Sleep {
    pub(crate) fn new(dur: Duration) -> Self {
        Self {
            deadline: Instant::now() + dur,
            registration: None,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        if self
            .registration
            .as_ref()
            .is_some_and(|registration| registration.is_fired())
        {
            self.registration = None;
            return std::task::Poll::Ready(());
        }
        if self.registration.is_none() {
            self.registration = Some(register(self.deadline, cx.waker().clone()));
            // Lost-wakeup guard: the timer may have fired between the
            // registration and the first check.
            if self
                .registration
                .as_ref()
                .is_some_and(|registration| registration.is_fired())
            {
                self.registration = None;
                return std::task::Poll::Ready(());
            }
        } else if let Some(registration) = &self.registration {
            // Update the waker: the future may be polled from a
            // different context than the one that registered.
            registration.update_waker(cx.waker().clone());
        }
        std::task::Poll::Pending
    }
}
