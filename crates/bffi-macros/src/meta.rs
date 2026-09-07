//! Const descriptor codegen for `bffi-dts` (DESIGN §6.1).
//!
//! [`expand`] appends one documented module per validated function:
//! `bffi_meta_<name>`, holding a single `FUNCTION` const of
//! [`::bffi_dts::FunctionDef`]. The descriptor is consumed by
//! `bffi-dts` (TypeScript `.d.ts` rendering) and `bffi-build` (linking
//! the `bffi_<name>` C ABI symbol), so the Rust signature, the shim
//! and the TypeScript surface all derive from one parsed model.

use crate::mapping;
use crate::model::FnModel;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Renders the descriptor module of a validated model: a documented
/// `bffi_meta_<name>` module exposing the `FUNCTION` const.
pub(crate) fn expand(model: &FnModel) -> TokenStream {
    let module = format_ident!("bffi_meta_{}", model.ident);
    let js_name = model.ident.to_string();
    let export_name = format!("bffi_{js_name}");
    let module_doc = format!(
        "Metadata for the `{js_name}` function descriptor (consumed by bffi-dts / bffi-build)."
    );
    let const_doc = format!("The [`::bffi_dts::FunctionDef`] descriptor for `{js_name}`.");

    let docs = model.docs.iter().map(|doc| quote! { #doc });

    let params = model.params.iter().map(|param| {
        let name = &param.name;
        let ty = mapping::ts_type(&param.kind).tokens();
        quote! { ::bffi_dts::ParamDef { name: #name, ty: #ty } }
    });
    let ret = mapping::ts_return(&model.ret).tokens();

    quote! {
        #[doc = #module_doc]
        pub mod #module {
            #[doc = #const_doc]
            pub const FUNCTION: ::bffi_dts::FunctionDef = ::bffi_dts::FunctionDef {
                js_name: #js_name,
                export_name: #export_name,
                docs: &[#(#docs),*],
                params: &[#(#params),*],
                ret: #ret,
            };
        }
    }
}
