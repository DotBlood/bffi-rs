//! Type classification and TypeScript kind mapping.
//!
//! [`classify_param`] and [`classify_return`] turn a `syn::Type` into
//! the boundary kinds from [`crate::model`], rejecting anything outside
//! the accepted set; [`TsKind`] is the typed bridge to the
//! `bffi-dts` IR (the descriptor generator quotes its variants
//! directly - no string round-trip).

use crate::errors::MacroDiagnostic;
use crate::model::{BigIntTy, BufferTy, FnReturn, PrimTy, ShimKind};
use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

/// The TypeScript type of an accepted boundary item, as a
/// [`::bffi_dts::TsType`] variant token stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsKind {
    /// `number`
    Number,
    /// `bigint`
    BigInt,
    /// `boolean`
    Boolean,
    /// `string`
    String,
    /// `Uint8Array`
    Uint8Array,
    /// `void`
    Void,
}

impl TsKind {
    /// The [`::bffi_dts::TsType`] variant tokens for this kind.
    pub(crate) fn tokens(self) -> TokenStream {
        match self {
            TsKind::Number => quote! { ::bffi_dts::TsType::Number },
            TsKind::BigInt => quote! { ::bffi_dts::TsType::BigInt },
            TsKind::Boolean => quote! { ::bffi_dts::TsType::Boolean },
            TsKind::String => quote! { ::bffi_dts::TsType::String },
            TsKind::Uint8Array => quote! { ::bffi_dts::TsType::Uint8Array },
            TsKind::Void => quote! { ::bffi_dts::TsType::Void },
        }
    }
}

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
/// generic-free segment (`u32`). Qualified paths (`std::u32`), paths
/// with arguments (`Vec<u8>`) and non-path types are `None`.
fn path_kind(ty: &syn::Type) -> Option<PathKind> {
    let (name, has_arguments) = path_ident(ty)?;
    if has_arguments {
        return None;
    }
    if let Some(prim) = prim_from_str(&name) {
        return Some(PathKind::Prim(prim));
    }
    bigint_from_str(&name).map(PathKind::BigInt)
}

/// The single-segment identifier of a plain path type, plus whether it
/// carries generic arguments. Non-path, qualified (`::x`/`std::x`)
/// and multi-segment paths are `None`.
fn path_ident(ty: &syn::Type) -> Option<(String, bool)> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    let has_arguments = !segment.arguments.is_none();
    Some((segment.ident.to_string(), has_arguments))
}

/// Whether `ty` is the bare `str` path type (`str` parses as a
/// single-segment path, not a dedicated variant).
fn is_str_type(ty: &syn::Type) -> bool {
    matches!(path_ident(ty), Some((name, false)) if name == "str")
}

/// The buffer payload behind `String` / `Vec<u8>` / `CopiedBuf`.
fn buffer_from_str(name: &str) -> Option<BufferTy> {
    match name {
        "String" => Some(BufferTy::String),
        "CopiedBuf" => Some(BufferTy::CopiedBuf),
        _ => None,
    }
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
    Err(MacroDiagnostic::param_type(ty.span(), ty, name))
}

/// Classifies a return type: the plain primitives plus `i64`/`u64`,
/// the empty tuple, the owned byte payloads (`String` / `Vec<u8>` /
/// `CopiedBuf`), `Option` of a payload, and `Result<T, E>` over any of
/// those. Everything else is rejected.
pub(crate) fn classify_return(ty: &syn::Type) -> syn::Result<FnReturn> {
    if let syn::Type::Tuple(tuple) = ty
        && tuple.elems.is_empty()
    {
        return Ok(FnReturn::Unit);
    }
    if let Some(kind) = path_kind(ty) {
        return Ok(match kind {
            PathKind::Prim(prim) => FnReturn::Prim(prim),
            PathKind::BigInt(bigint) => FnReturn::BigInt(bigint),
        });
    }
    if let Some((name, false)) = path_ident(ty)
        && let Some(buffer) = buffer_from_str(&name)
    {
        return Ok(FnReturn::Buffer(buffer));
    }
    if let Some((name, true)) = path_ident(ty) {
        let args = generic_args(ty);
        match (name.as_str(), args.as_slice()) {
            ("Vec", [inner]) if is_u8(inner) => return Ok(FnReturn::Buffer(BufferTy::ByteVec)),
            ("Option", [inner]) => {
                if let Some(buffer) = classify_buffer_only(inner) {
                    return Ok(FnReturn::Nullable(buffer));
                }
            }
            ("Result", [ok, err]) => {
                let inner = classify_return(ok)?;
                // Shape-check only: the trait obligations on `E`
                // (`Error + Send + Sync + 'static`) surface as a
                // regular trait-bound error in the expansion, where
                // rustc names the exact missing impl.
                if path_ident(err).is_none() {
                    return Err(MacroDiagnostic::return_type(ty.span(), ty));
                }
                return Ok(FnReturn::Result(Box::new(inner)));
            }
            _ => {}
        }
    }
    Err(MacroDiagnostic::return_type(ty.span(), ty))
}

/// Classifies `ty` when it is exactly a buffer payload; `None` when it
/// is another type (the caller rejects with the help line).
fn classify_buffer_only(ty: &syn::Type) -> Option<BufferTy> {
    if let Some((name, false)) = path_ident(ty) {
        return buffer_from_str(&name);
    }
    if let Some((name, true)) = path_ident(ty)
        && name == "Vec"
    {
        let args = generic_args(ty);
        if args.len() == 1 && is_u8(args[0]) {
            return Some(BufferTy::ByteVec);
        }
    }
    None
}

/// The generic argument list of a single-segment path type (empty for
/// argument-free paths).
fn generic_args(ty: &syn::Type) -> Vec<&syn::Type> {
    let syn::Type::Path(path) = ty else {
        return Vec::new();
    };
    let Some(segment) = path.path.segments.last() else {
        return Vec::new();
    };
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Vec::new();
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect()
}

/// Whether `ty` is the bare `u8` path.
fn is_u8(ty: &syn::Type) -> bool {
    matches!(path_ident(ty), Some((name, false)) if name == "u8")
}

/// TypeScript kind of a small primitive: every numeric type is
/// [`TsKind::Number`], `bool` is [`TsKind::Boolean`].
fn ts_prim(prim: PrimTy) -> TsKind {
    match prim {
        PrimTy::I8
        | PrimTy::I16
        | PrimTy::I32
        | PrimTy::U8
        | PrimTy::U16
        | PrimTy::U32
        | PrimTy::F32
        | PrimTy::F64 => TsKind::Number,
        PrimTy::Bool => TsKind::Boolean,
    }
}

/// TypeScript kind of an accepted parameter kind.
pub(crate) fn ts_type(kind: &ShimKind) -> TsKind {
    match kind {
        ShimKind::Prim(prim) => ts_prim(*prim),
        ShimKind::BigInt(_) => TsKind::BigInt,
        ShimKind::Str => TsKind::String,
    }
}

/// TypeScript kind of an accepted return type. `Nullable` renders as
/// its payload kind - the nullability is carried by the descriptor's
/// auto-generated doc line (see `meta`).
pub(crate) fn ts_return(ret: &FnReturn) -> TsKind {
    match ret {
        FnReturn::Unit => TsKind::Void,
        FnReturn::Prim(prim) => ts_prim(*prim),
        FnReturn::BigInt(_) => TsKind::BigInt,
        FnReturn::Buffer(BufferTy::String) => TsKind::String,
        FnReturn::Buffer(_) => TsKind::Uint8Array,
        FnReturn::Nullable(inner) => ts_return(&FnReturn::Buffer(*inner)),
        FnReturn::Result(inner) => ts_return(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BigIntTy, BufferTy, FnReturn, PrimTy, ShimKind, TsKind, classify_param, classify_return,
        ts_prim, ts_return, ts_type,
    };
    use crate::model::FnModel;
    use quote::quote;

    /// Parses a type source, panicking in tests only (allowed by the
    /// crate-root `cfg_attr(test)` escape hatch).
    fn ty(src: &str) -> syn::Type {
        syn::parse_str(src).expect("valid type source")
    }

    #[test]
    fn accepted_primitives_map_to_ts_kinds() {
        let cases = [
            (PrimTy::I8, TsKind::Number),
            (PrimTy::I16, TsKind::Number),
            (PrimTy::I32, TsKind::Number),
            (PrimTy::U8, TsKind::Number),
            (PrimTy::U16, TsKind::Number),
            (PrimTy::U32, TsKind::Number),
            (PrimTy::F32, TsKind::Number),
            (PrimTy::F64, TsKind::Number),
            (PrimTy::Bool, TsKind::Boolean),
        ];
        for (prim, expected) in cases {
            assert_eq!(ts_prim(prim), expected);
        }
    }

    #[test]
    fn ts_kind_tokens_quote_the_ir_variant() {
        assert_eq!(
            TsKind::Number.tokens().to_string(),
            ":: bffi_dts :: TsType :: Number"
        );
        assert_eq!(
            TsKind::Uint8Array.tokens().to_string(),
            ":: bffi_dts :: TsType :: Uint8Array"
        );
        assert_eq!(
            TsKind::Void.tokens().to_string(),
            ":: bffi_dts :: TsType :: Void"
        );
    }

    #[test]
    fn accepted_param_types_map_to_ts_kinds() {
        let cases = [
            ("i8", TsKind::Number),
            ("i32", TsKind::Number),
            ("f64", TsKind::Number),
            ("bool", TsKind::Boolean),
            ("i64", TsKind::BigInt),
            ("u64", TsKind::BigInt),
            ("&str", TsKind::String),
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
            assert_eq!(ts_type(&kind), TsKind::String, "param type `{src}`");
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
    fn buffer_returns_classify_with_their_ts_kinds() {
        let cases = [
            ("String", FnReturn::Buffer(BufferTy::String), TsKind::String),
            (
                "Vec<u8>",
                FnReturn::Buffer(BufferTy::ByteVec),
                TsKind::Uint8Array,
            ),
            (
                "CopiedBuf",
                FnReturn::Buffer(BufferTy::CopiedBuf),
                TsKind::Uint8Array,
            ),
        ];
        for (src, expected, ts) in cases {
            let ret = classify_return(&ty(src)).expect("accepted");
            assert_eq!(ret, expected, "return type `{src}`");
            assert_eq!(ts_return(&ret), ts, "ts kind for `{src}`");
        }
    }

    #[test]
    fn option_buffer_returns_classify_nullable() {
        for src in ["Option<String>", "Option<Vec<u8>>", "Option<CopiedBuf>"] {
            let ret = classify_return(&ty(src)).expect("accepted");
            assert!(
                matches!(ret, FnReturn::Nullable(_)),
                "`{src}` must classify as Nullable"
            );
        }
        // Nullable renders as its payload kind; nullability is a doc
        // line (see meta).
        assert_eq!(
            ts_return(&classify_return(&ty("Option<String>")).expect("accepted")),
            TsKind::String
        );
    }

    #[test]
    fn result_returns_wrap_any_supported_inner() {
        for src in [
            "Result<u32, MyError>",
            "Result<(), MyError>",
            "Result<String, MyError>",
            "Result<Option<CopiedBuf>, MyError>",
        ] {
            let ret = classify_return(&ty(src)).expect("accepted");
            assert!(
                matches!(ret, FnReturn::Result(_)),
                "`{src}` must classify as Result"
            );
        }
        assert_eq!(
            ts_return(&classify_return(&ty("Result<u32, MyError>")).expect("accepted")),
            TsKind::Number
        );
    }

    #[test]
    fn option_over_primitives_is_rejected() {
        for src in ["Option<i32>", "Option<u64>", "Option<bool>"] {
            let err = classify_return(&ty(src)).expect_err("rejected");
            assert!(
                err.to_string().contains("bffi[E003]"),
                "`{src}` must carry E003"
            );
        }
    }

    #[test]
    fn vec_non_u8_and_single_arg_result_are_rejected() {
        assert!(classify_return(&ty("Vec<u32>")).is_err());
        assert!(classify_return(&ty("Vec<i8>")).is_err());
        assert!(classify_return(&ty("Result<u32>")).is_err());
    }

    #[test]
    fn unit_return_maps_to_void() {
        let ret = classify_return(&ty("()")).expect("accepted");
        assert_eq!(ts_return(&ret), TsKind::Void);
    }

    #[test]
    fn unsupported_param_type_is_rejected_with_documented_message() {
        let err = classify_param(&ty("Vec<u8>"), "data").expect_err("rejected");
        let text = err.to_string();
        assert!(text.starts_with("bffi[E002]: unsupported type `Vec < u8 >` for parameter `data`"));
        assert!(
            text.contains(
                "  = help: supported: i8|i16|i32|i64|u8|u16|u32|u64|f32|f64|bool|&str|()"
            )
        );
        assert!(text.contains("DESIGN.md"));
    }

    #[test]
    fn unsupported_return_type_is_rejected() {
        assert!(classify_return(&ty("char")).is_err());
        assert!(classify_return(&ty("&str")).is_err());
        assert!(
            classify_return(&ty("std::string::String")).is_err(),
            "qualified paths stay rejected"
        );
    }

    #[test]
    fn rejected_param_types_fail_classification() {
        let cases = [
            ("&mut str", "mutability would break the copy guarantee"),
            ("&u32", "only `&str` may be borrowed"),
            ("str", "bare `str` is unsized"),
            ("i128", "integer width outside the matrix"),
            ("usize", "integer width outside the matrix"),
            ("char", "not in the matrix"),
            ("&[u8]", "buffer parameters are future work"),
            ("String", "owned strings are return-only"),
        ];
        for (src, why) in cases {
            let result = classify_param(&ty(src), "x");
            assert!(result.is_err(), "`{src}` must be rejected: {why}");
            // Rendering contract: type rejections carry the E002 code.
            let text = result.expect_err("checked above").to_string();
            assert!(text.contains("bffi[E002]"), "`{src}` must carry E002");
        }
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
                .starts_with("bffi[E004]: this attribute takes no options")
        );
    }
}
