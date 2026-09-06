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
}

#[test]
fn descriptors_render_through_bffi_dts() {
    static FNS: &[FunctionDef] = &[bffi_meta_add::FUNCTION, bffi_meta_greet::FUNCTION];
    let module = ModuleDef {
        name: "math",
        fns: FNS,
    };
    let rendered = bffi_dts::render(&module);
    assert!(rendered.contains("/** Adds two numbers. */"));
    assert!(rendered.contains("export function add(a: number, b: number): number;"));
    assert!(rendered.contains("export function greet(who: string): number;"));
}
