//! Golden tests for the deterministic `.d.ts` renderer.
//!
//! The files under `tests/golden/` are byte-normative: LF line endings
//! and exactly one trailing newline, per the format contract on
//! [`bffi_dts::render`]. Git may normalize working-tree files to CRLF
//! on Windows checkouts (`core.autocrlf`), so every textual comparison
//! normalizes `\r\n` back to `\n` first; the committed bytes are still
//! guarded by [`golden_files_contain_no_carriage_returns`], and the
//! repo-root `.gitattributes` forces `eol=lf` for `*.d.ts`.
//!
//! Module fixtures are declared as `static` descriptor arrays, proving
//! the const-descriptor pattern the `#[bffi]` macro (a later stage)
//! will emit.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_dts::{FunctionDef, ModuleDef, ParamDef, TsType, render::render};

static ADD_PARAMS: &[ParamDef] = &[
    ParamDef {
        name: "a",
        ty: TsType::Number,
    },
    ParamDef {
        name: "b",
        ty: TsType::Number,
    },
];

static MATH_FNS: &[FunctionDef] = &[FunctionDef {
    js_name: "add",
    export_name: "bffi_add",
    docs: &["Adds two numbers."],
    params: ADD_PARAMS,
    ret: TsType::Number,
}];

static MATH: ModuleDef = ModuleDef {
    name: "math",
    fns: MATH_FNS,
};

static KITCHEN_FNS: &[FunctionDef] = &[
    FunctionDef {
        js_name: "add",
        export_name: "bffi_add",
        docs: &["Adds two numbers."],
        params: ADD_PARAMS,
        ret: TsType::Number,
    },
    FunctionDef {
        js_name: "class",
        export_name: "bffi_class",
        docs: &["Stores a value.", "Returns nothing."],
        params: &[
            ParamDef {
                name: "delete",
                ty: TsType::BigInt,
            },
            ParamDef {
                name: "data",
                ty: TsType::Uint8Array,
            },
            ParamDef {
                name: "flag",
                ty: TsType::Boolean,
            },
        ],
        ret: TsType::Void,
    },
    FunctionDef {
        js_name: "greet",
        export_name: "bffi_greet",
        docs: &["Greets."],
        params: &[ParamDef {
            name: "name",
            ty: TsType::String,
        }],
        ret: TsType::String,
    },
    FunctionDef {
        js_name: "noop",
        export_name: "bffi_noop",
        docs: &[],
        params: &[],
        ret: TsType::Void,
    },
];

static KITCHEN: ModuleDef = ModuleDef {
    name: "kitchen",
    fns: KITCHEN_FNS,
};

static EMPTY: ModuleDef = ModuleDef {
    name: "empty",
    fns: &[],
};

/// Normalizes CRLF line endings to LF, undoing any `core.autocrlf`
/// normalization `include_str!` picked up from the working tree.
fn normalize_lf(contents: &str) -> String {
    contents.replace("\r\n", "\n")
}

#[test]
fn golden_math_matches() {
    let expected = normalize_lf(include_str!("golden/math.d.ts"));
    assert_eq!(render(&MATH), expected);
}

#[test]
fn golden_kitchen_matches() {
    let expected = normalize_lf(include_str!("golden/kitchen.d.ts"));
    assert_eq!(render(&KITCHEN), expected);
}

#[test]
fn golden_empty_matches() {
    let expected = normalize_lf(include_str!("golden/empty.d.ts"));
    assert_eq!(render(&EMPTY), expected);
}

#[test]
fn render_is_deterministic() {
    for module in [&MATH, &KITCHEN, &EMPTY] {
        let first = render(module);
        let second = render(module);
        assert_eq!(first, second, "re-rendering {module:?} must be identical");
    }
}

#[test]
fn golden_files_contain_no_carriage_returns() {
    for contents in [
        include_str!("golden/math.d.ts"),
        include_str!("golden/kitchen.d.ts"),
        include_str!("golden/empty.d.ts"),
    ] {
        assert!(!contents.contains('\r'), "golden file must be LF-only");
    }
}
