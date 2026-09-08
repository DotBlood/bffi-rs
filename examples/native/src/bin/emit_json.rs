//! Materializes the loader JSON for the example module: the single
//! `emit-json` binary writes `js/bffi.api.json` from the SAME
//! [`module_def::MODULE`] the `.d.ts` golden is rendered from, so the
//! two JS-side artifacts can never drift apart.
//!
//! Run as part of `bun run test:native`:
//! `cargo run -q -p bffi-example-native --bin emit-json`.

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("js/bffi.api.json");
    if let Err(error) =
        bffi_build::loader_json::write_to_file(&bffi_example_native::module_def::MODULE, &path)
    {
        eprintln!("emit-json: {error}");
        std::process::exit(1);
    }
    println!("written {}", path.display());
}
