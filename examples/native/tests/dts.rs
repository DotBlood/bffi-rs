//! `.d.ts` verification (kanboard criteria 6.1/6.2): the TypeScript
//! surface of this example is rendered from the same `bffi_meta`
//! descriptors the shims are generated from, pinned to the committed
//! golden `js/api.d.ts` and consumed type-level by `js/api-usage.ts`
//! under real tsc (`bun run typecheck`).
//!
//! Aggregation is explicit (the standing decision: no inventory, no
//! linkme): every `#[bffi]` function of this example is listed below
//! by its descriptor module.

#![allow(clippy::expect_used)]

use bffi_dts::{FunctionDef, ModuleDef};

/// The `#[bffi]` functions of this example, in declaration order.
const FUNCTIONS: &[FunctionDef] = &[
    bffi_example_native::bffi_meta_add::FUNCTION,
    bffi_example_native::bffi_meta_greet::FUNCTION,
    bffi_example_native::bffi_meta_greet_len::FUNCTION,
    bffi_example_native::bffi_meta_shout::FUNCTION,
    bffi_example_native::bffi_meta_echo_buffer::FUNCTION,
    bffi_example_native::bffi_meta_checked_div::FUNCTION,
    bffi_example_native::bffi_meta_boom::FUNCTION,
    bffi_example_native::bffi_meta_mirror_i64::FUNCTION,
    bffi_example_native::bffi_meta_mirror_u64::FUNCTION,
    bffi_example_native::bffi_meta_is_even::FUNCTION,
];

/// The module definition the golden file is rendered from.
const MODULE: ModuleDef = ModuleDef {
    name: "api",
    fns: FUNCTIONS,
    classes: &[],
};

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
