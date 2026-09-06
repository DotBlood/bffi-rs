//! `extern "C"` shim codegen under the boundary policy (DESIGN §6.1,
//! §6.3, §6.5).
//!
//! [`expand`] renders exactly one shim per validated function: the
//! symbol the JS side links against (`bffi_<name>`). The shim copies
//! inputs (never zero-copy), validates every pointer, validates UTF-8
//! on the cstring path, and transports the result through an
//! out-parameter, returning an [`ErrorCode`]-style status from
//! `bffi-core`. Panics follow the per-build policy: debug builds let
//! the panic escape (the process aborts, easier debugging), release
//! builds run the body through `bffi_core::boundary::run_extern_body`,
//! which converts the panic into `ErrorCode::Panic` plus a stored
//! thread-local last error.

use crate::model::{BigIntTy, FnModel, FnReturn, PrimTy, ShimKind};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// Renders the full expansion of a validated model: the original item
/// tokens first (docs and attributes unchanged), then the
/// `extern "C"` shim.
pub(crate) fn expand(model: &FnModel, item: &syn::ItemFn) -> TokenStream {
    let shim_ident = format_ident!("bffi_{}", model.ident);
    let params: Vec<TokenStream> = model
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| shim_param(param, index))
        .collect();
    let out = out_param(model.ret);
    let body = body(model);

    // Variant A (dev): the body runs directly - a panic escapes and
    // aborts, which keeps stack traces intact while debugging
    // (DESIGN §6.5).
    let debug_shim = quote! {
        #[cfg(debug_assertions)]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn #shim_ident(#(#params,)* #(#out)*) -> ::bffi_core::ErrorCode {
            #body
        }
    };
    // Variant B (prod): the entire body runs inside the boundary
    // policy, so a panic becomes `ErrorCode::Panic` plus a stored last
    // error instead of unwinding into the host.
    let release_shim = quote! {
        #[cfg(not(debug_assertions))]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn #shim_ident(#(#params,)* #(#out)*) -> ::bffi_core::ErrorCode {
            ::bffi_core::boundary::run_extern_body(move || { #body })
        }
    };

    quote! { #item #debug_shim #release_shim }
}

/// Declares one shim parameter.
///
/// Primitives and bigints keep their Rust type; a `&str` parameter
/// arrives as a NUL-terminated cstring pointer per the `bun:ffi`
/// convention (DESIGN §6.3).
fn shim_param(param: &crate::model::FnParam, index: usize) -> TokenStream {
    let name = param_ident(&param.name, index);
    match param.kind {
        ShimKind::Str => {
            let ptr = format_ident!("{name}_ptr");
            quote! { #ptr: *const ::std::os::raw::c_char }
        }
        ShimKind::Prim(prim) => {
            let ty = prim_ty(prim);
            quote! { #name: #ty }
        }
        ShimKind::BigInt(big) => {
            let ty = bigint_ty(big);
            quote! { #name: #ty }
        }
    }
}

/// Declares the out-parameter that carries the return value across the
/// C ABI (`()` returns have none).
fn out_param(ret: FnReturn) -> Vec<TokenStream> {
    match ret {
        FnReturn::Unit => Vec::new(),
        FnReturn::Prim(prim) => {
            let ty = prim_ty(prim);
            vec![quote! { __ret: *mut #ty }]
        }
        FnReturn::BigInt(big) => {
            let ty = bigint_ty(big);
            vec![quote! { __ret: *mut #ty }]
        }
    }
}

/// Renders the shim body shared by both cfg variants: pointer
/// validation first, cstring conversion per `&str` parameter, then the
/// call and the out-parameter write.
fn body(model: &FnModel) -> TokenStream {
    let mut body = TokenStream::new();

    if !matches!(model.ret, FnReturn::Unit) {
        body.extend(quote! {
            if __ret.is_null() {
                let error = ::bffi_core::BffiError::new(
                    ::bffi_core::ErrorCode::NullPointer,
                    "output pointer is null",
                );
                ::bffi_core::set_last_error(error);
                return ::bffi_core::ErrorCode::NullPointer;
            }
        });
    }

    for (index, param) in model.params.iter().enumerate() {
        if let ShimKind::Str = param.kind {
            let name = param_ident(&param.name, index);
            let ptr = format_ident!("{name}_ptr");
            let bytes = format_ident!("{name}_bytes");
            let view = format_ident!("{name}_view");
            body.extend(quote! {
                if #ptr.is_null() {
                    let error = ::bffi_core::BffiError::new(
                        ::bffi_core::ErrorCode::NullPointer,
                        "string argument pointer is null",
                    );
                    ::bffi_core::set_last_error(error);
                    return ::bffi_core::ErrorCode::NullPointer;
                }
                // SAFETY: bun:ffi hands out NUL-terminated cstrings for `&str`
                // parameters (DESIGN.md §6.3); the pointer is null-checked above.
                let #bytes = unsafe { ::std::ffi::CStr::from_ptr(#ptr) }.to_bytes();
                let #view = match ::bffi_types::unsafe_zero_copy::str_view(#bytes) {
                    ::std::result::Result::Ok(v) => v,
                    ::std::result::Result::Err(e) => {
                        ::bffi_core::set_last_error(e);
                        return ::bffi_core::ErrorCode::InvalidUtf8;
                    }
                };
            });
        }
    }

    let ident = &model.ident;
    let args = model.params.iter().enumerate().map(|(index, param)| {
        let name = param_ident(&param.name, index);
        match param.kind {
            // `ZeroCopyStr` derefs to `str`.
            ShimKind::Str => {
                let view = format_ident!("{name}_view");
                quote! { &#view }
            }
            ShimKind::Prim(_) | ShimKind::BigInt(_) => quote! { #name },
        }
    });

    match model.ret {
        FnReturn::Unit => body.extend(quote! {
            #ident(#(#args,)*);
            ::bffi_core::ErrorCode::Ok
        }),
        FnReturn::Prim(_) | FnReturn::BigInt(_) => body.extend(quote! {
            let __value = #ident(#(#args,)*);
            // SAFETY: `__ret` is non-null (checked above) and valid for one
            // `T` write per the bun:ffi out-parameter contract.
            unsafe { ::std::ptr::write(__ret, __value); }
            ::bffi_core::ErrorCode::Ok
        }),
    }

    body
}

/// Sanitizes a model parameter name into a usable shim identifier.
///
/// `FnParam::name` mirrors the source pattern and is not guaranteed to
/// be a plain identifier (non-ident patterns fall back to rendered
/// tokens, e.g. `_`), so unusable names deterministically fall back to
/// the positional `__arg<index>`.
fn param_ident(name: &str, index: usize) -> Ident {
    syn::parse_str::<Ident>(name).unwrap_or_else(|_| format_ident!("__arg{index}"))
}

/// The Rust primitive type of a small boundary primitive.
fn prim_ty(prim: PrimTy) -> TokenStream {
    match prim {
        PrimTy::I8 => quote! { i8 },
        PrimTy::I16 => quote! { i16 },
        PrimTy::I32 => quote! { i32 },
        PrimTy::U8 => quote! { u8 },
        PrimTy::U16 => quote! { u16 },
        PrimTy::U32 => quote! { u32 },
        PrimTy::F32 => quote! { f32 },
        PrimTy::F64 => quote! { f64 },
        PrimTy::Bool => quote! { bool },
    }
}

/// The Rust type of a 64-bit boundary integer.
fn bigint_ty(big: BigIntTy) -> TokenStream {
    match big {
        BigIntTy::I64 => quote! { i64 },
        BigIntTy::U64 => quote! { u64 },
    }
}

#[cfg(test)]
mod tests {
    use super::{expand, param_ident};
    use crate::model::FnModel;
    use quote::quote;

    #[test]
    fn wildcard_pattern_name_falls_back_to_positional() {
        assert_eq!(param_ident("_", 2).to_string(), "__arg2");
    }

    #[test]
    fn valid_ident_passes_through() {
        assert_eq!(param_ident("phrase", 0).to_string(), "phrase");
    }

    #[test]
    fn keyword_falls_back_to_positional() {
        assert_eq!(param_ident("fn", 1).to_string(), "__arg1");
    }

    #[test]
    fn expansion_keeps_item_and_adds_both_cfg_shims() {
        let item = quote! {
            /// Adds two numbers.
            fn add(a: u32, b: u32) -> u32 {
                a + b
            }
        };
        let attrs = proc_macro2::TokenStream::new();
        let model = FnModel::parse(&attrs, item.clone()).expect("accepted");
        let func = syn::parse2::<syn::ItemFn>(item).expect("valid fn");

        let tokens = expand(&model, &func).to_string();

        assert!(tokens.contains("fn add"), "original item is emitted first");
        assert!(tokens.contains("Adds two numbers"), "docs stay on the item");
        assert!(tokens.contains("fn bffi_add"));
        assert!(tokens.contains("no_mangle"));
        assert!(
            tokens.contains("cfg (debug_assertions)"),
            "exactly one dev variant"
        );
        assert!(
            tokens.contains("cfg (not (debug_assertions))"),
            "exactly one release variant"
        );
        assert!(tokens.contains("run_extern_body"));
        assert!(tokens.contains("not_unsafe_ptr_arg_deref"));
        assert!(tokens.contains("NullPointer"));
    }
}
