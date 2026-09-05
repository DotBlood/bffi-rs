//! Concurrency stress for ObjectWrap (kanboard 4.1: roundtrip + stress).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_core::{Handle, TypeTag};
use bffi_object::ObjectWrap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

struct Payload {
    value: u64,
}

const TAG_MIXED: TypeTag = TypeTag(0x0120);
const TAG_RACE: TypeTag = TypeTag(0x0121);

const THREADS: usize = 8;
const ROUNDS: usize = 500;

#[test]
fn mixed_wrap_get_release_stress() {
    let wrap = ObjectWrap::<Payload>::new(TAG_MIXED).expect("unique tag");

    // Handles parked for reuse; a handle sits in the pool at most once,
    // so pop -> get -> release never races on the same handle.
    let pool: Arc<Mutex<Vec<Handle>>> = Arc::new(Mutex::new(Vec::new()));

    thread::scope(|scope| {
        for _ in 0..THREADS {
            let pool = Arc::clone(&pool);
            scope.spawn(move || {
                // ObjectWrap is Copy: the move closure captures it by value.
                for round in 0..ROUNDS {
                    if round % 3 == 0 {
                        let handle = wrap
                            .wrap(Payload {
                                value: round as u64,
                            })
                            .expect("table has room");
                        pool.lock().expect("pool").push(handle);
                    } else if let Some(handle) = pool.lock().expect("pool").pop() {
                        let value = wrap.get(handle).expect("live while pooled");
                        assert!(value.value < ROUNDS as u64);
                        wrap.release(handle).expect("live while pooled");
                    }
                }
            });
        }
    });

    // Drain, then verify emptiness OBSERVABLY (frozen API has no len):
    // every handle ever issued must be stale now (kanboard "len == 0").
    let mut drained = Vec::new();
    for handle in pool.lock().expect("pool").drain(..) {
        wrap.release(handle).expect("live while pooled");
        drained.push(handle);
    }
    for handle in drained {
        assert!(
            wrap.get(handle).is_err(),
            "no handle may resolve after release"
        );
    }
}

#[test]
fn release_concurrent_with_get_never_resurrects() {
    let wrap = ObjectWrap::<Payload>::new(TAG_RACE).expect("unique tag");
    let handle = wrap.wrap(Payload { value: 1 }).expect("room");

    let done = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(Barrier::new(THREADS + 1));

    thread::scope(|scope| {
        for _ in 0..THREADS {
            let done = Arc::clone(&done);
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                // ObjectWrap is Copy: the move closure captures it by value.
                barrier.wait();
                // Before release: Ok; after: clean InvalidHandle. A panic
                // in a getter fails the test via the scope join; a released
                // handle must never come back to life persistently.
                while !done.load(Ordering::Acquire) {
                    let _ = wrap.get(handle);
                }
            });
        }
        barrier.wait();
        wrap.release(handle).expect("live handle");
        done.store(true, Ordering::Release);
    });

    assert!(
        wrap.get(handle).is_err(),
        "a released handle must stay dead"
    );
}
