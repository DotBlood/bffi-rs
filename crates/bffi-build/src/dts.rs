//! The build-time `.d.ts` materialization helper: the one-call bridge
//! between the pure [`bffi_dts`] renderer and the filesystem.
//!
//! This is the build-time half of kanboard criterion 6.2 ("full dts
//! <-> build integration"): aggregate the `bffi_meta` descriptors
//! explicitly (the standing decision - no inventory, no linkme), then
//! materialize the declaration file with a single call.

use std::path::Path;

use bffi_dts::ModuleDef;

/// Renders `module` and writes the `.d.ts` bytes to `path`.
///
/// The bytes on disk are exactly [`bffi_dts::render`]'s output - the
/// render contract already guarantees LF line endings and a single
/// trailing newline, so nothing is transformed here. The output is a
/// function of the IR alone: writing the same module twice produces a
/// byte-identical file, which makes the generated `.d.ts` safe to
/// commit and diff.
///
/// Missing parent directories are created first
/// ([`std::fs::create_dir_all`]); io errors surface unchanged.
///
/// Aggregation is the caller's job: this helper renders the
/// [`ModuleDef`] it is handed and knows nothing about how the
/// descriptors were collected. The reference use is
/// `examples/native/tests/dts.rs`; the C ABI surface the declarations
/// describe is specified in this crate's `CALLING-CONVENTION.md`.
pub fn write_to_file(module: &ModuleDef, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bffi_dts::render(module).as_bytes())
}
