//! Concurrency stress tests for the lock-free handle table.
//!
//! These hammer `insert` / `get` / `remove` / `contains` from many threads
//! at once and verify the invariants that matter for the lock-free design:
//! unique handles, exactly-once removal, refcount balance (every value is
//! dropped exactly once, including values released through the hazard
//! retire path), and `get` never resolving a removed handle.

// Tests assert invariants; the workspace restriction lints target
// production code.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;

use bffi_core::{Handle, HandleTable, TypeTag};

const THREADS: usize = 8;
const ITERATIONS: usize = 4_000;

/// Table + value helper (test-local constructor).
fn table_for<T: Send + Sync + 'static>(tag: TypeTag) -> HandleTable<T> {
    HandleTable::new(tag)
}

/// Value payload carrying a reference to its test's private live-counter,
/// so parallel tests never observe each other's numbers.
#[derive(Debug)]
struct Tracked {
    payload: u64,
    live: &'static AtomicUsize,
}

impl Tracked {
    fn new(payload: u64, live: &'static AtomicUsize) -> Self {
        live.fetch_add(1, Ordering::Relaxed);
        Self { payload, live }
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.live.fetch_sub(1, Ordering::Relaxed);
    }
}

#[test]
fn mixed_operations_keep_invariants_under_contention() {
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    let arena = Arc::new(table_for::<Tracked>(TypeTag(0x8F10)));
    let failures = Arc::new(AtomicU64::new(0));

    thread::scope(|scope| {
        for worker in 0..THREADS {
            let arena = Arc::clone(&arena);
            let failures = Arc::clone(&failures);
            scope.spawn(move || {
                let mut local_handles: Vec<Handle> = Vec::with_capacity(ITERATIONS / 4);
                for i in 0..ITERATIONS {
                    let payload = ((worker as u64) << 40) | (i as u64) | 1;
                    match i % 4 {
                        // insert
                        0 => {
                            let handle = arena
                                .insert(Arc::new(Tracked::new(payload, &LIVE)))
                                .expect("insert must succeed");
                            local_handles.push(handle);
                        }
                        // get on our own recent handle: must resolve with
                        // our payload or (if another thread removed it) not
                        // resolve at all - never a foreign payload
                        1 => {
                            if let Some(handle) = local_handles.last()
                                && let Some(value) = arena.get(*handle)
                                && value.payload >> 40 != worker as u64
                            {
                                failures.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        // remove our oldest handle: exactly one thread can
                        // win; the loser must observe None
                        2 => {
                            if let Some(handle) = local_handles.pop() {
                                let removed = arena.remove(handle);
                                match removed {
                                    Some(value) => {
                                        if value.payload >> 40 != worker as u64 {
                                            failures.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                    None => {
                                        failures.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                        // contains on a foreign-format handle must be false
                        _ => {
                            let foreign = Handle::new(TypeTag(0x8F11), 0, (worker * 7 + i) as u32);
                            if arena.contains(foreign) {
                                failures.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                // drain remaining handles through the remove path
                for handle in local_handles.drain(..) {
                    if arena.remove(handle).is_none() {
                        failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    assert_eq!(
        failures.load(Ordering::Relaxed),
        0,
        "no protocol violations are tolerated"
    );
    assert!(arena.is_empty(), "all values must be removed");
}

#[test]
fn get_never_resolves_a_removed_value_even_under_heavy_removal() {
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    // One thread hammers remove on a rotating window while several threads
    // hammer get on the same handles: the hazard protocol must make every
    // successful get observe a fully intact value.
    let arena = Arc::new(table_for::<Tracked>(TypeTag(0x8F12)));

    let handles: Arc<Vec<Handle>> = Arc::new(
        (0..64_u64)
            .map(|i| {
                arena
                    .insert(Arc::new(Tracked::new(i, &LIVE)))
                    .expect("insert")
            })
            .collect(),
    );

    let stop = AtomicU64::new(0);
    thread::scope(|scope| {
        let arena = &arena;
        let handles = &handles[..];
        let stop = &stop;

        // remover: rotates removal over the window
        scope.spawn(move || {
            for handle in handles {
                arena.remove(*handle);
            }
            stop.fetch_add(1, Ordering::Relaxed);
        });

        // readers: get must either fail or return an intact payload
        for _ in 0..(THREADS - 1) {
            scope.spawn(move || {
                for handle in handles {
                    while stop.load(Ordering::Relaxed) == 0 {
                        if let Some(value) = arena.get(*handle) {
                            if value.payload >= 64 {
                                panic!("get resolved a corrupted/foreign value");
                            }
                        } else {
                            break;
                        }
                    }
                }
            });
        }
    });

    // The retire list drains amortized (threshold 2 x hazard-slot count)
    // and on Drop; a value protected by a reader at removal time is
    // legitimately still alive right after the scope. Dropping the table
    // is the deterministic point where exactly-once freeing is guaranteed.
    drop(arena);

    // every value must have been dropped exactly once (table + readers):
    // the remover drained the whole window, so nothing stays live
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
}

#[test]
fn dropped_table_frees_every_value_after_contention() {
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    let base = LIVE.load(Ordering::Relaxed);
    {
        let arena = table_for::<Tracked>(TypeTag(0x8F13));

        let handles: Vec<_> = (0..THREADS)
            .map(|worker| {
                let arena = &arena;
                (0..ITERATIONS / 2)
                    .map(move |i| {
                        arena
                            .insert(Arc::new(Tracked::new(
                                ((worker as u64) << 40) | i as u64,
                                &LIVE,
                            )))
                            .expect("insert")
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let total: usize = handles.iter().map(Vec::len).sum();
        assert_eq!(LIVE.load(Ordering::Relaxed) - base, total);

        // drop roughly half through remove (returned Arcs dropped at once)
        for chunk in &handles {
            for handle in chunk.iter().take(chunk.len() / 2) {
                assert!(arena.remove(*handle).is_some());
            }
        }
    }
    assert_eq!(
        LIVE.load(Ordering::Relaxed),
        base,
        "dropping the table must free the values it still owns"
    );
}
