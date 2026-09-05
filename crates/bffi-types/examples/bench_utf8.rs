//! Benchmark: UTF-8 validation throughput (SIMD dispatch vs scalar DFA).
//!
//! Run with: `cargo run --release -p bffi-types --example bench_utf8`
//!
//! The SIMD path is internal, so this benchmark compares the public
//! `bytes_to_string` (SIMD validation + copy) against `std::str::from_utf8`
//! on the same payload.

use std::time::Instant;

fn main() {
    // Realistic mixed payload: ASCII + Cyrillic + emoji, ~1.5 bytes per
    // character on average.
    let unit = "bffi: привет 🚀 - fast utf8 ünïcödé check ".as_bytes();
    let mut bytes = Vec::with_capacity(unit.len() * 64_000);
    for _ in 0..64_000 {
        bytes.extend_from_slice(unit);
    }
    let mib = bytes.len() as f64 / (1024.0 * 1024.0);
    println!("payload: {:.1} MiB ({} bytes)", mib, bytes.len());

    // warm-up
    for _ in 0..3 {
        std::hint::black_box(bffi_types::bytes_to_string(&bytes).ok());
        std::hint::black_box(std::str::from_utf8(&bytes).ok());
    }

    let rounds = 20;
    let mut bffi_total = std::time::Duration::ZERO;
    let mut std_total = std::time::Duration::ZERO;
    for _ in 0..rounds {
        let start = Instant::now();
        std::hint::black_box(bffi_types::bytes_to_string(&bytes).ok());
        bffi_total += start.elapsed();

        let start = Instant::now();
        std::hint::black_box(std::str::from_utf8(&bytes).ok());
        std_total += start.elapsed();
    }

    let bffi_gib_s = rounds as f64 * mib / 1024.0 / bffi_total.as_secs_f64();
    let std_gib_s = rounds as f64 * mib / 1024.0 / std_total.as_secs_f64();
    println!(
        "bffi-types (SIMD validate + copy): {:>10.2} GiB/s",
        bffi_gib_s
    );
    println!(
        "std::str::from_utf8 (validate only): {:>8.2} GiB/s",
        std_gib_s
    );
}
