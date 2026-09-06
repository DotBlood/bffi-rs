//! Acceptance tests for the generated `bffi_meta_<name>` descriptors
//! (kanboard 6.1): every annotated function exposes a const
//! [`::bffi_dts::FunctionDef`] that matches the signature and renders
//! through `bffi-dts` into the expected TypeScript declarations.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_dts::{FunctionDef, ModuleDef, ParamDef, TsType};

#[bffi_macros::bffi]
/// Adds two numbers.
fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[bffi_macros::bffi]
/// Handles a name.
fn greet(who: &str) -> u32 {
    who.len() as u32
}

#[bffi_macros::bffi]
/// Builds a greeting.
fn build_greeting(who: &str) -> String {
    format!("hello {who}")
}

#[bffi_macros::bffi]
/// Reads a payload.
fn payload() -> Option<Vec<u8>> {
    None
}

#[test]
fn descriptor_matches_the_expected_literal() {
    assert_eq!(
        bffi_meta_add::FUNCTION,
        FunctionDef {
            js_name: "add",
            export_name: "bffi_add",
            docs: &["Adds two numbers."],
            params: &[
                ParamDef {
                    name: "a",
                    ty: TsType::Number
                },
                ParamDef {
                    name: "b",
                    ty: TsType::Number
                },
            ],
            ret: TsType::Number,
        }
    );
    assert_eq!(
        bffi_meta_greet::FUNCTION,
        FunctionDef {
            js_name: "greet",
            export_name: "bffi_greet",
            docs: &["Handles a name."],
            params: &[ParamDef {
                name: "who",
                ty: TsType::String
            }],
            ret: TsType::Number,
        }
    );
    // Buffer payloads: String -> `string`, Vec<u8> -> `Uint8Array`;
    // nullability is carried by the deterministic doc line.
    assert_eq!(
        bffi_meta_build_greeting::FUNCTION,
        FunctionDef {
            js_name: "build_greeting",
            export_name: "bffi_build_greeting",
            docs: &["Builds a greeting."],
            params: &[ParamDef {
                name: "who",
                ty: TsType::String
            }],
            ret: TsType::String,
        }
    );
    assert_eq!(
        bffi_meta_payload::FUNCTION.ret,
        TsType::Uint8Array,
        "Vec<u8> renders as Uint8Array"
    );
    assert_eq!(
        bffi_meta_payload::FUNCTION.docs,
        &[
            "Reads a payload.",
            "Returns the byte payload as an opaque handle; `0` means `None`."
        ],
        "Nullable returns carry the deterministic doc line"
    );
}

#[test]
fn descriptors_render_through_bffi_dts() {
    static FNS: &[FunctionDef] = &[
        bffi_meta_add::FUNCTION,
        bffi_meta_greet::FUNCTION,
        bffi_meta_build_greeting::FUNCTION,
        bffi_meta_payload::FUNCTION,
    ];
    let module = ModuleDef {
        name: "math",
        fns: FNS,
        classes: &[],
    };
    let rendered = bffi_dts::render(&module);
    assert!(rendered.contains("/** Adds two numbers. */"));
    assert!(rendered.contains("export function add(a: number, b: number): number;"));
    assert!(rendered.contains("export function greet(who: string): number;"));
    assert!(rendered.contains("export function build_greeting(who: string): string;"));
    assert!(rendered.contains("export function payload(): Uint8Array;"));
}
