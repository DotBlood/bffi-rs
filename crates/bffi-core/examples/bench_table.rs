//! Benchmark: lock-free handle table operations.
//!
//! Run with: `cargo run --release -p bffi-core --example bench_table`

// Benchmarks abort on setup failures by design.
#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

use bffi_core::{HandleTable, TypeTag};

/// Builds the benchmark table.
fn bench_table(tag: TypeTag) -> Arc<HandleTable<u64>> {
    Arc::new(HandleTable::new(tag))
}

fn main() {
    let tag = TypeTag(0xBEEF);

    // single-threaded insert/get/remove
    const N: usize = 1_000_000;
    let arena = bench_table(tag);
    let start = Instant::now();
    let handles: Vec<_> = (0..N as u64)
        .map(|i| arena.insert(Arc::new(i)).expect("insert"))
        .collect();
    let insert = start.elapsed();

    let start = Instant::now();
    for handle in &handles {
        std::hint::black_box(arena.get(*handle));
    }
    let get = start.elapsed();

    let start = Instant::now();
    for handle in &handles {
        std::hint::black_box(arena.remove(*handle));
    }
    let remove = start.elapsed();

    println!(
        "single thread: insert {:>8.0} ops/s | get {:>8.0} ops/s | remove {:>8.0} ops/s",
        N as f64 / insert.as_secs_f64(),
        N as f64 / get.as_secs_f64(),
        N as f64 / remove.as_secs_f64(),
    );

    // multi-threaded mixed traffic (readers dominated)
    let arena = bench_table(tag);
    let seeds: Vec<_> = (0..4096_u64)
        .map(|i| arena.insert(Arc::new(i)).expect("insert"))
        .collect();
    let ops = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();
    thread::scope(|scope| {
        for _ in 0..8 {
            let arena = Arc::clone(&arena);
            let seeds = &seeds;
            let ops = Arc::clone(&ops);
            scope.spawn(move || {
                let mut local = 0_usize;
                for i in 0..1_000_000_usize {
                    let handle = seeds[i % seeds.len()];
                    if i % 8 == 0 {
                        let value = arena.get(handle);
                        std::hint::black_box(value);
                    } else {
                        std::hint::black_box(arena.contains(handle));
                    }
                    local += 1;
                }
                ops.fetch_add(local, Ordering::Relaxed);
            });
        }
    });
    let elapsed = start.elapsed();
    let total = ops.load(Ordering::Relaxed);
    println!(
        "8 threads read-heavy: {:>10.0} ops/s ({total} ops in {:.2}s)",
        total as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64()
    );
}
