//! Type classification and TypeScript name mapping.
//!
//! [`classify_param`] and [`classify_return`] turn a `syn::Type` into
//! the boundary kinds from [`crate::model`], rejecting anything outside
//! the P1 set; the `ts_*` helpers render the TypeScript name (`number`,
//! `bigint`, `boolean`, `string`, `void`) that the descriptor generator
//! quotes into the expansion.

use crate::{
    errors,
    model::{BigIntTy, FnReturn, PrimTy, ShimKind},
};

/// Kind of a plain path type: a small primitive or a 64-bit integer.
enum PathKind {
    /// A small primitive (`i32`, `f64`, `bool`, ...).
    Prim(PrimTy),
    /// A 64-bit integer (`i64`/`u64`).
    BigInt(BigIntTy),
}

/// Resolves a small-primitive name.
fn prim_from_str(name: &str) -> Option<PrimTy> {
    match name {
        "i8" => Some(PrimTy::I8),
        "i16" => Some(PrimTy::I16),
        "i32" => Some(PrimTy::I32),
        "u8" => Some(PrimTy::U8),
        "u16" => Some(PrimTy::U16),
        "u32" => Some(PrimTy::U32),
        "f32" => Some(PrimTy::F32),
        "f64" => Some(PrimTy::F64),
        "bool" => Some(PrimTy::Bool),
        _ => None,
    }
}

/// Resolves a 64-bit integer name.
fn bigint_from_str(name: &str) -> Option<BigIntTy> {
    match name {
        "i64" => Some(BigIntTy::I64),
        "u64" => Some(BigIntTy::U64),
        _ => None,
    }
}

/// Classifies `ty` when it is a plain, unqualified path over a single
/// generic-free segment (`u32`), i.e. the shape accepted in P1.
/// Qualified paths (`std::u32`), paths with arguments (`Vec<u8>`) and
/// non-path types are `None`.
fn path_kind(ty: &syn::Type) -> Option<PathKind> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    if !segment.arguments.is_none() {
        return None;
    }
    let name = segment.ident.to_string();
    if let Some(prim) = prim_from_str(&name) {
        return Some(PathKind::Prim(prim));
    }
    bigint_from_str(&name).map(PathKind::BigInt)
}

/// Whether `ty` is the bare `str` path type (`str` parses as a
/// single-segment path, not a dedicated variant).
fn is_str_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == "str"
        && path.path.segments[0].arguments.is_none()
}

/// Classifies a parameter type: plain primitives and `i64`/`u64` as
/// their kinds, `&str` (borrowed, not `mut`; lifetimes ignored) as
/// [`ShimKind::Str`], everything else rejected.
pub(crate) fn classify_param(ty: &syn::Type, name: &str) -> syn::Result<ShimKind> {
    if let Some(kind) = path_kind(ty) {
        return Ok(match kind {
            PathKind::Prim(prim) => ShimKind::Prim(prim),
            PathKind::BigInt(bigint) => ShimKind::BigInt(bigint),
        });
    }
    if let syn::Type::Reference(reference) = ty
        && reference.mutability.is_none()
        && is_str_type(&reference.elem)
    {
        return Ok(ShimKind::Str);
    }
    Err(errors::unsupported_param_type(ty, name))
}

/// Classifies a return type: the plain primitives plus `i64`/`u64` and
/// the empty tuple. `&str` and everything else is rejected.
pub(crate) fn classify_return(ty: &syn::Type) -> syn::Result<FnReturn> {
    if let syn::Type::Tuple(tuple) = ty
        && tuple.elems.is_empty()
    {
        return Ok(FnReturn::Unit);
    }
    match path_kind(ty) {
        Some(PathKind::Prim(prim)) => Ok(FnReturn::Prim(prim)),
        Some(PathKind::BigInt(bigint)) => Ok(FnReturn::BigInt(bigint)),
        None => Err(errors::unsupported_return_type(ty)),
    }
}

/// TypeScript name of a small primitive: every numeric type is
/// `number`, `bool` is `boolean`.
// Quoted into descriptors by the generator in Task 4.
#[allow(dead_code)]
pub(crate) fn ts_prim(prim: PrimTy) -> &'static str {
    match prim {
        PrimTy::I8
        | PrimTy::I16
        | PrimTy::I32
        | PrimTy::U8
        | PrimTy::U16
        | PrimTy::U32
        | PrimTy::F32
        | PrimTy::F64 => "number",
        PrimTy::Bool => "boolean",
    }
}

/// TypeScript name of an accepted parameter kind.
// Quoted into descriptors by the generator in Task 4.
#[allow(dead_code)]
pub(crate) fn ts_type(kind: &ShimKind) -> &'static str {
    match kind {
        ShimKind::Prim(prim) => ts_prim(*prim),
        ShimKind::BigInt(_) => "bigint",
        ShimKind::Str => "string",
    }
}

/// TypeScript name of an accepted return type.
// Quoted into descriptors by the generator in Task 4.
#[allow(dead_code)]
pub(crate) fn ts_return(ret: &FnReturn) -> &'static str {
    match ret {
        FnReturn::Unit => "void",
        FnReturn::Prim(prim) => ts_prim(*prim),
        FnReturn::BigInt(_) => "bigint",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BigIntTy, FnReturn, PrimTy, ShimKind, classify_param, classify_return, ts_prim, ts_return,
        ts_type,
    };
    use crate::model::FnModel;
    use quote::quote;

    /// Parses a type source, panicking in tests only (allowed by the
    /// crate-root `cfg_attr(test)` escape hatch).
    fn ty(src: &str) -> syn::Type {
        syn::parse_str(src).expect("valid type source")
    }

    #[test]
    fn accepted_primitives_map_to_ts_names() {
        let cases = [
            (PrimTy::I8, "number"),
            (PrimTy::I16, "number"),
            (PrimTy::I32, "number"),
            (PrimTy::U8, "number"),
            (PrimTy::U16, "number"),
            (PrimTy::U32, "number"),
            (PrimTy::F32, "number"),
            (PrimTy::F64, "number"),
            (PrimTy::Bool, "boolean"),
        ];
        for (prim, expected) in cases {
            assert_eq!(ts_prim(prim), expected);
        }
    }

    #[test]
    fn accepted_param_types_map_to_ts_names() {
        let cases = [
            ("i8", "number"),
            ("i16", "number"),
            ("i32", "number"),
            ("u8", "number"),
            ("u16", "number"),
            ("u32", "number"),
            ("f32", "number"),
            ("f64", "number"),
            ("bool", "boolean"),
            ("i64", "bigint"),
            ("u64", "bigint"),
        ];
        for (src, expected) in cases {
            let kind = classify_param(&ty(src), "x").expect("accepted");
            assert_eq!(ts_type(&kind), expected, "param type `{src}`");
        }
    }

    #[test]
    fn str_params_accept_lifetimes() {
        for src in ["&str", "&'a str"] {
            let kind = classify_param(&ty(src), "x").expect("accepted");
            assert_eq!(ts_type(&kind), "string", "param type `{src}`");
        }
    }

    #[test]
    fn bigints_classify_distinctly() {
        assert_eq!(
            classify_param(&ty("i64"), "x").expect("accepted"),
            ShimKind::BigInt(BigIntTy::I64)
        );
        assert_eq!(
            classify_param(&ty("u64"), "x").expect("accepted"),
            ShimKind::BigInt(BigIntTy::U64)
        );
    }

    #[test]
    fn unit_return_maps_to_void() {
        let ret = classify_return(&ty("()")).expect("accepted");
        assert_eq!(ts_return(&ret), "void");
    }

    #[test]
    fn unsupported_param_type_is_rejected_with_documented_message() {
        let err = classify_param(&ty("Vec<u8>"), "data").expect_err("rejected");
        let text = err.to_string();
        assert!(text.starts_with("bffi: unsupported type `Vec < u8 >` for parameter `data`"));
        assert!(text.contains(
            "  = help: supported in P1: i8|i16|i32|i64|u8|u16|u32|u64|f32|f64|bool|&str|()"
        ));
        assert!(text.contains("DESIGN.md"));
    }

    #[test]
    fn unsupported_return_type_is_rejected() {
        assert!(classify_return(&ty("String")).is_err());
        assert!(classify_return(&ty("&str")).is_err());
    }

    #[test]
    fn happy_path_model_parse() {
        let item = quote! {
            #[doc = " Adds."]
            fn add(a: u32, b: u32) -> u32 {
                a + b
            }
        };
        let attrs = proc_macro2::TokenStream::new();
        let model = FnModel::parse(&attrs, item).expect("accepted");
        assert_eq!(model.ident, "add");
        assert_eq!(model.docs, ["Adds."]);
        assert_eq!(model.params.len(), 2);
        assert_eq!(model.params[0].name, "a");
        assert_eq!(model.params[0].kind, ShimKind::Prim(PrimTy::U32));
        assert_eq!(model.params[1].name, "b");
        assert_eq!(model.params[1].kind, ShimKind::Prim(PrimTy::U32));
        assert_eq!(model.ret, FnReturn::Prim(PrimTy::U32));
    }

    #[test]
    fn attribute_options_are_rejected() {
        let item = quote! { fn f(x: u32) {} };
        let result = FnModel::parse(&quote! { rename = "x" }, item);
        assert!(result.is_err());
        let err = result.err().expect("rejected");
        assert!(
            err.to_string()
                .starts_with("bffi: this attribute takes no options")
        );
    }
}
