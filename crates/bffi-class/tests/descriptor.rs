//! Acceptance tests for the generated `ClassDef` descriptors
//! (kanboard P2: integration with bffi-object and bffi-dts): the
//! metadata split across the two macro expansions assembles into one
//! `ClassDef` that renders through `bffi-dts`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_dts::{ClassDef, MethodDef, ModuleDef, ParamDef, TsType};

#[bffi_class::bffi_class(tag = 0x0160)]
/// A wallet.
pub struct Wallet {
    /// The balance.
    pub balance: u64,
}

#[bffi_class::bffi_impl]
impl Wallet {
    #[bffi_class::bffi_constructor]
    /// Creates a wallet.
    pub fn new(start: u64) -> Self {
        Self { balance: start }
    }

    /// Adds an amount.
    pub fn topped(&self, amount: u64) -> u64 {
        self.balance + amount
    }
}

#[test]
fn class_descriptor_matches_the_expected_literal() {
    assert_eq!(bffi_meta_wallet::TAG, 0x0160);
    assert_eq!(bffi_meta_wallet::FIELDS.len(), 1);
    assert_eq!(
        bffi_meta_wallet_impl::CLASS,
        ClassDef {
            js_name: "wallet",
            docs: &["A wallet."],
            constructor: MethodDef {
                js_name: "constructor",
                export_name: "bffi_wallet_new",
                docs: &["Creates a wallet."],
                params: &[ParamDef {
                    name: "start",
                    ty: TsType::BigInt,
                }],
                ret: TsType::BigInt,
            },
            fields: &[bffi_dts::FieldDef {
                js_name: "balance",
                docs: &[],
                ty: TsType::BigInt,
            }],
            methods: &[MethodDef {
                js_name: "topped",
                export_name: "bffi_wallet_topped",
                docs: &["Adds an amount."],
                params: &[ParamDef {
                    name: "amount",
                    ty: TsType::BigInt,
                }],
                ret: TsType::BigInt,
            }],
        }
    );
}

#[test]
fn class_descriptor_renders_through_bffi_dts() {
    static CLASSES: &[ClassDef] = &[bffi_meta_wallet_impl::CLASS];
    let module = ModuleDef {
        name: "bank",
        fns: &[],
        classes: CLASSES,
    };
    let rendered = bffi_dts::render(&module);
    assert!(rendered.contains("/** A wallet. */"));
    assert!(rendered.contains("export class wallet {"));
    assert!(rendered.contains("  constructor(start: bigint);"));
    assert!(rendered.contains("  get balance(): bigint;"));
    assert!(rendered.contains("  topped(amount: bigint): bigint;"));
    assert!(rendered.ends_with("}\n"));
}
