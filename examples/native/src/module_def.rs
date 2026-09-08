//! THE single descriptor aggregation of the example crate: every
//! `#[bffi]` / `#[bffi_async]` function and every `#[bffi_class]`
//! class, in one explicit [`ModuleDef`] (the standing decision - no
//! inventory, no linkme).
//!
//! Consumers:
//!
//! - `tests/dts.rs` renders the committed `.d.ts` golden from [`MODULE`];
//! - `src/bin/emit_json.rs` materializes the loader JSON for the JS
//!   codegen from the same [`MODULE`] - one source, two artifacts.

use bffi_dts::{ClassDef, FunctionDef, ModuleDef};

/// The `#[bffi]` / `#[bffi_async]` functions of this example, in
/// aggregation order (the `.d.ts` golden order).
pub const FUNCTIONS: &[FunctionDef] = &[
    crate::bffi_meta_add::FUNCTION,
    crate::bffi_meta_greet::FUNCTION,
    crate::bffi_meta_greet_len::FUNCTION,
    crate::bffi_meta_shout::FUNCTION,
    crate::bffi_meta_echo_buffer::FUNCTION,
    crate::bffi_meta_checked_div::FUNCTION,
    crate::bffi_meta_boom::FUNCTION,
    crate::bffi_meta_mirror_i64::FUNCTION,
    crate::bffi_meta_mirror_u64::FUNCTION,
    crate::bffi_meta_is_even::FUNCTION,
    crate::bffi_meta_facade_probe::FUNCTION,
    crate::bffi_meta_example_compute::FUNCTION,
];

/// The `#[bffi_class]` classes of this example.
pub const CLASSES: &[ClassDef] = &[crate::bffi_meta_counter_impl::CLASS];

/// The full example module: name `api` (the committed `js/api.d.ts`
/// golden name).
pub const MODULE: ModuleDef = ModuleDef {
    name: "api",
    fns: FUNCTIONS,
    classes: CLASSES,
};
