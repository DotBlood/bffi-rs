//! Materializes the loader JSON for the reference library: writes
//! `.bffi/bffi.api.json` from the single `module_def::MODULE`
//! aggregation. The `@z2net/bffi` pipeline and the `packages/native`
//! main package consume that file.

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".bffi/bffi.api.json");
    if let Err(error) =
        bffi_build::loader_json::write_to_file(&bffi_native::module_def::MODULE, &path)
    {
        eprintln!("emit-json: {error}");
        std::process::exit(1);
    }
    println!("written {}", path.display());
}
