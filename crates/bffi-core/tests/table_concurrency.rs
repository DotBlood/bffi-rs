//! Concurrency stress tests for handle tables and the global registry.

// Tests assert invariants; the workspace restriction lints target
// production code.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use bffi_core::{Handle, HandleTable, Registry, TypeTag};

const THREADS: usize = 8;
const ITERATIONS: usize = 2_000;

/// Builds a table for a test tag.
fn table_for<T: Send + Sync + 'static>(tag: TypeTag) -> HandleTable<T> {
    HandleTable::new(tag)
}

#[test]
fn concurrent_insert_get_remove_never_loses_objects() {
    let arena = Arc::new(table_for::<u64>(TypeTag(0x8F00)));

    let handles = thread::scope(|scope| {
        let mut joiners = Vec::new();
        for worker in 0..THREADS {
            let arena = Arc::clone(&arena);
            joiners.push(scope.spawn(move || {
                let mut local = Vec::with_capacity(ITERATIONS);
                for i in 0..ITERATIONS {
                    let payload = (worker * ITERATIONS + i) as u64;
                    let handle = arena
                        .insert(Arc::new(payload))
                        .expect("insert must succeed");
                    let stored = arena.get(handle).expect("fresh handle resolves");
                    assert_eq!(*stored, payload);
                    local.push(handle);
                }
                local
            }));
        }
        joiners
            .into_iter()
            .flat_map(|joiner| joiner.join().expect("worker must not panic"))
            .collect::<Vec<_>>()
    });

    assert_eq!(arena.len(), THREADS * ITERATIONS);
    assert!(handles.iter().all(|h| arena.contains(*h)));

    // Every handle issued during the race must be distinct.
    let mut sorted: Vec<u64> = handles.iter().map(|h| h.as_u64()).collect();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), THREADS * ITERATIONS);

    let removed = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                for handle in &handles {
                    if arena.remove(*handle).is_some() {
                        removed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    assert_eq!(
        removed.load(Ordering::Relaxed),
        THREADS * ITERATIONS,
        "every handle must be removable exactly once"
    );
    assert!(arena.is_empty());
}

#[test]
fn stale_handles_are_invisible_across_threads() {
    let arena = table_for::<u32>(TypeTag(0x8F01));
    let handles: Vec<_> = (0..100_u32)
        .map(|i| arena.insert(Arc::new(i)).expect("arena has room"))
        .collect();
    for handle in &handles {
        assert!(
            arena.remove(*handle).is_some(),
            "every live handle must be removable"
        );
    }

    thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                for handle in &handles {
                    assert!(arena.get(*handle).is_none());
                    assert!(!arena.contains(*handle));
                }
            });
        }
    });
}

#[test]
fn registry_survives_concurrent_traffic() {
    const SLOT: TypeTag = TypeTag(0x8F02);
    let registry = Registry::global();
    if registry.declare::<u64>(SLOT).is_ok() {
        // first declaration in this process; later runs of the same test
        // binary reuse the already-declared table
    }

    let successes = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                for i in 0..ITERATIONS {
                    let handle = registry
                        .insert(SLOT, Arc::new(i as u64))
                        .expect("registry insert must succeed");
                    let value = registry
                        .get_typed::<u64>(handle)
                        .expect("registry handle resolves");
                    assert_eq!(*value, i as u64);
                    assert!(registry.remove(handle));
                    successes.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    assert_eq!(successes.load(Ordering::Relaxed), THREADS * ITERATIONS);
    assert!(registry.get_typed::<u64>(Handle::NULL).is_none());
}
