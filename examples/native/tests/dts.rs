//! `.d.ts` verification (kanboard criteria 6.1/6.2): the TypeScript
//! surface of this example is rendered from the crate's SINGLE
//! descriptor aggregation (`src/module_def.rs`), written to disk via
//! `bffi_build::dts::write_to_file`, pinned to the committed golden
//! `js/api.d.ts` and consumed type-level by `js/api-usage.ts` under
//! real tsc (`bun run typecheck`).
//!
//! The same `module_def::MODULE` also feeds the loader JSON
//! (`src/bin/emit_json.rs`) - one aggregation, two artifacts.

#![allow(clippy::expect_used)]

use bffi_example_native::module_def::MODULE;

#[test]
fn render_is_deterministic_lf_only_and_matches_the_golden() {
    let first = bffi_dts::render(&MODULE);
    let second = bffi_dts::render(&MODULE);
    assert_eq!(
        first, second,
        "rendering the same module must be deterministic"
    );
    assert!(
        !first.contains('\r'),
        "the renderer must emit LF endings only"
    );

    // The build-time half of criterion 6.2: the same bytes reach the
    // disk through the one-call helper (parent dirs included).
    let dir = std::env::temp_dir().join(format!("bffi-example-native-dts-{}", std::process::id()));
    let written = dir.join("js/api.d.ts");
    bffi_build::dts::write_to_file(&MODULE, &written).expect("write_to_file succeeds");
    let from_disk = std::fs::read_to_string(&written).expect("the written file exists");
    assert_eq!(
        first, from_disk,
        "write_to_file must persist the rendered bytes verbatim"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/js/api.d.ts");
    let committed = std::fs::read_to_string(golden_path).expect("golden js/api.d.ts exists");
    // CRLF -> LF: a Windows checkout may materialize the committed
    // file with CRLF line endings; the renderer contract is LF-only.
    let normalized = committed.replace("\r\n", "\n");
    assert_eq!(
        first, normalized,
        "golden drift: js/api.d.ts must match the rendered descriptor surface"
    );
}
