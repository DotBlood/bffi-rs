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
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

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
        let ty = ts_variant(mapping::ts_type(&param.kind));
        quote! { ::bffi_dts::ParamDef { name: #name, ty: #ty } }
    });
    let ret = ts_variant(mapping::ts_return(&model.ret));

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

/// The [`::bffi_dts::TsType`] variant token for a TypeScript name
/// produced by the mapping helpers (`"number"` -> `Number`, ...).
fn ts_variant(name: &str) -> TokenStream {
    let variant = match name {
        "number" => "Number",
        "bigint" => "BigInt",
        "boolean" => "Boolean",
        "string" => "String",
        "void" => "Void",
        // Unreachable: the mapping helpers only produce the five
        // names above. Instead of panicking, emit a nonexistent
        // variant so a bug surfaces as a compile error at the use
        // site.
        _ => "__BffiUnsupportedTsType",
    };
    let variant = Ident::new(variant, Span::call_site());
    quote! { ::bffi_dts::TsType::#variant }
}

#[cfg(test)]
mod tests {
    use super::ts_variant;

    #[test]
    fn ts_variant_covers_all_five_names() {
        assert_eq!(
            ts_variant("number").to_string(),
            ":: bffi_dts :: TsType :: Number"
        );
        assert_eq!(
            ts_variant("bigint").to_string(),
            ":: bffi_dts :: TsType :: BigInt"
        );
        assert_eq!(
            ts_variant("boolean").to_string(),
            ":: bffi_dts :: TsType :: Boolean"
        );
        assert_eq!(
            ts_variant("string").to_string(),
            ":: bffi_dts :: TsType :: String"
        );
        assert_eq!(
            ts_variant("void").to_string(),
            ":: bffi_dts :: TsType :: Void"
        );
    }
}
