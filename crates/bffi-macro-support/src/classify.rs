//! Type classification: `syn::Type` -> boundary kinds.
//!
//! [`classify_param`] and [`classify_return`] turn a `syn::Type` into
//! the boundary kinds from [`crate::kind`], rejecting anything outside
//! the accepted set. Rejections are neutral: the classifiers return an
//! [`Unsupported`] carrying the span and the offending type, and each
//! proc-macro crate converts it into its own diagnostic (its own
//! E-code and exact help lines). The TypeScript mapping
//! ([`ts_prim`]/[`ts_type`]/[`ts_return`]) complements the
//! classifiers for the descriptor generators.

use crate::kind::{BigIntTy, BufferTy, PrimTy, RetKind, ShimKind, TsKind};
use proc_macro2::Span;
use syn::spanned::Spanned;

/// A type rejected by a classifier: the neutral form each proc-macro
/// crate maps onto its own diagnostic (E-code plus exact help and
/// note lines).
pub struct Unsupported<'a> {
    /// Span of the offending type.
    pub span: Span,
    /// The offending type.
    pub ty: &'a syn::Type,
}

// Manual impl: `syn::Type` only implements `Debug` behind the
// `extra-traits` feature, which this crate does not enable.
impl std::fmt::Debug for Unsupported<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Unsupported").finish_non_exhaustive()
    }
}

/// Kind of a plain path type: a small primitive or a 64-bit integer.
pub enum PathKind {
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

/// Resolves the buffer payload behind `String` / `Vec<u8>` / `CopiedBuf`.
fn buffer_from_str(name: &str) -> Option<BufferTy> {
    match name {
        "String" => Some(BufferTy::String),
        "CopiedBuf" => Some(BufferTy::CopiedBuf),
        _ => None,
    }
}

/// The single-segment identifier of a plain path type, plus whether it
/// carries generic arguments. Non-path, qualified (`::x`/`std::x`)
/// and multi-segment paths are `None`.
pub fn path_ident(ty: &syn::Type) -> Option<(String, bool)> {
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

/// Classifies `ty` when it is a plain, unqualified path over a single
/// generic-free segment (`u32`). Qualified paths (`std::u32`), paths
/// with arguments (`Vec<u8>`) and non-path types are `None`.
pub fn path_kind(ty: &syn::Type) -> Option<PathKind> {
    let (name, has_arguments) = path_ident(ty)?;
    if has_arguments {
        return None;
    }
    if let Some(prim) = prim_from_str(&name) {
        return Some(PathKind::Prim(prim));
    }
    bigint_from_str(&name).map(PathKind::BigInt)
}

/// Whether `ty` is the bare `str` path type (`str` parses as a
/// single-segment path, not a dedicated variant).
pub fn is_str_type(ty: &syn::Type) -> bool {
    matches!(path_ident(ty), Some((name, false)) if name == "str")
}

/// Whether `ty` is the bare `u8` path.
pub fn is_u8(ty: &syn::Type) -> bool {
    matches!(path_ident(ty), Some((name, false)) if name == "u8")
}

/// Whether `ty` is a slice of exactly `u8` (`[u8]`).
fn is_u8_slice(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Slice(slice) if is_u8(&slice.elem))
}

/// The generic argument list of a single-segment path type (empty for
/// argument-free paths).
pub fn generic_args(ty: &syn::Type) -> Vec<&syn::Type> {
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

/// Classifies a parameter type: plain primitives and `i64`/`u64` as
/// their kinds, `&str` (borrowed, not `mut`; lifetimes ignored) as
/// [`ShimKind::Str`], `&[u8]` (borrowed, not `mut`; lifetimes
/// ignored) as [`ShimKind::BufferView`], everything else rejected.
pub fn classify_param(ty: &syn::Type) -> Result<ShimKind, Unsupported<'_>> {
    if let Some(kind) = path_kind(ty) {
        return Ok(match kind {
            PathKind::Prim(prim) => ShimKind::Prim(prim),
            PathKind::BigInt(bigint) => ShimKind::BigInt(bigint),
        });
    }
    if let syn::Type::Reference(reference) = ty
        && reference.mutability.is_none()
    {
        if is_str_type(&reference.elem) {
            return Ok(ShimKind::Str);
        }
        if is_u8_slice(&reference.elem) {
            return Ok(ShimKind::BufferView);
        }
    }
    Err(Unsupported {
        span: ty.span(),
        ty,
    })
}

/// Classifies a return type: the plain primitives plus `i64`/`u64`,
/// the empty tuple, the owned byte payloads (`String` / `Vec<u8>` /
/// `CopiedBuf`), `Option` of a payload, and `Result<T, E>` over any of
/// those. Everything else is rejected.
pub fn classify_return(ty: &syn::Type) -> Result<RetKind, Unsupported<'_>> {
    if let syn::Type::Tuple(tuple) = ty
        && tuple.elems.is_empty()
    {
        return Ok(RetKind::Unit);
    }
    if let Some(kind) = path_kind(ty) {
        return Ok(match kind {
            PathKind::Prim(prim) => RetKind::Prim(prim),
            PathKind::BigInt(bigint) => RetKind::BigInt(bigint),
        });
    }
    if let Some((name, false)) = path_ident(ty)
        && let Some(buffer) = buffer_from_str(&name)
    {
        return Ok(RetKind::Buffer(buffer));
    }
    if let Some((name, true)) = path_ident(ty) {
        let args = generic_args(ty);
        match (name.as_str(), args.as_slice()) {
            ("Vec", [inner]) if is_u8(inner) => return Ok(RetKind::Buffer(BufferTy::ByteVec)),
            ("Option", [inner]) => {
                if let Some(buffer) = classify_buffer_only(inner) {
                    return Ok(RetKind::Nullable(buffer));
                }
            }
            ("Result", [ok, err]) => {
                let inner = classify_return(ok)?;
                // Shape-check only: the trait obligations on `E`
                // (`Error + Send + Sync + 'static`) surface as a
                // regular trait-bound error in the expansion, where
                // rustc names the exact missing impl.
                if path_ident(err).is_none() {
                    return Err(Unsupported {
                        span: ty.span(),
                        ty,
                    });
                }
                return Ok(RetKind::Result(Box::new(inner)));
            }
            _ => {}
        }
    }
    Err(Unsupported {
        span: ty.span(),
        ty,
    })
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

/// TypeScript kind of a small primitive: every numeric type is
/// [`TsKind::Number`], `bool` is [`TsKind::Boolean`].
pub fn ts_prim(prim: PrimTy) -> TsKind {
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
pub fn ts_type(kind: &ShimKind) -> TsKind {
    match kind {
        ShimKind::Prim(prim) => ts_prim(*prim),
        ShimKind::BigInt(_) => TsKind::BigInt,
        ShimKind::Str => TsKind::String,
        // The descriptor sees ONE `Uint8Array` parameter: the
        // `(ptr, len)` C pair is ABI-level only.
        ShimKind::BufferView => TsKind::Uint8Array,
    }
}

/// TypeScript kind of an accepted return type. `Nullable` payloads
/// map to the dedicated `NullableString` / `NullableUint8Array`
/// kinds, whose `as_str` carries the `| null` contract.
pub fn ts_return(ret: &RetKind) -> TsKind {
    match ret {
        RetKind::Unit => TsKind::Void,
        RetKind::Prim(prim) => ts_prim(*prim),
        RetKind::BigInt(_) => TsKind::BigInt,
        RetKind::Buffer(BufferTy::String) => TsKind::String,
        RetKind::Buffer(_) => TsKind::Uint8Array,
        RetKind::Nullable(BufferTy::String) => TsKind::NullableString,
        RetKind::Nullable(_) => TsKind::NullableUint8Array,
        RetKind::Result(inner) => ts_return(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PathKind, classify_param, classify_return, is_str_type, is_u8, path_ident, path_kind,
        ts_prim, ts_return, ts_type,
    };
    use crate::kind::{BigIntTy, BufferTy, PrimTy, RetKind, ShimKind, TsKind};

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
    fn accepted_param_types_map_to_ts_kinds() {
        let cases = [
            ("i8", TsKind::Number),
            ("i32", TsKind::Number),
            ("f64", TsKind::Number),
            ("bool", TsKind::Boolean),
            ("i64", TsKind::BigInt),
            ("u64", TsKind::BigInt),
            ("&str", TsKind::String),
            ("&[u8]", TsKind::Uint8Array),
        ];
        for (src, expected) in cases {
            let kind = classify_param(&ty(src)).expect("accepted");
            assert_eq!(ts_type(&kind), expected, "param type `{src}`");
        }
    }

    #[test]
    fn str_params_accept_lifetimes() {
        for src in ["&str", "&'a str"] {
            let kind = classify_param(&ty(src)).expect("accepted");
            assert_eq!(ts_type(&kind), TsKind::String, "param type `{src}`");
        }
    }

    #[test]
    fn buffer_view_params_accept_lifetimes_and_classify_distinctly() {
        for src in ["&[u8]", "&'a [u8]"] {
            let kind = classify_param(&ty(src)).expect("accepted");
            assert_eq!(kind, ShimKind::BufferView, "param type `{src}`");
            assert_eq!(ts_type(&kind), TsKind::Uint8Array, "param type `{src}`");
        }
    }

    #[test]
    fn bigints_classify_distinctly() {
        assert_eq!(
            classify_param(&ty("i64")).expect("accepted"),
            ShimKind::BigInt(BigIntTy::I64)
        );
        assert_eq!(
            classify_param(&ty("u64")).expect("accepted"),
            ShimKind::BigInt(BigIntTy::U64)
        );
    }

    #[test]
    fn buffer_returns_classify_with_their_ts_kinds() {
        let cases = [
            ("String", RetKind::Buffer(BufferTy::String), TsKind::String),
            (
                "Vec<u8>",
                RetKind::Buffer(BufferTy::ByteVec),
                TsKind::Uint8Array,
            ),
            (
                "CopiedBuf",
                RetKind::Buffer(BufferTy::CopiedBuf),
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
        let cases = [
            ("Option<String>", TsKind::NullableString),
            ("Option<Vec<u8>>", TsKind::NullableUint8Array),
            ("Option<CopiedBuf>", TsKind::NullableUint8Array),
        ];
        for (src, ts) in cases {
            let ret = classify_return(&ty(src)).expect("accepted");
            assert!(
                matches!(ret, RetKind::Nullable(_)),
                "`{src}` must classify as Nullable"
            );
            assert_eq!(ts_return(&ret), ts, "ts kind for `{src}`");
        }
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
                matches!(ret, RetKind::Result(_)),
                "`{src}` must classify as Result"
            );
        }
        assert_eq!(
            ts_return(&classify_return(&ty("Result<u32, MyError>")).expect("accepted")),
            TsKind::Number
        );
        assert_eq!(
            ts_return(
                &classify_return(&ty("Result<Option<CopiedBuf>, MyError>")).expect("accepted")
            ),
            TsKind::NullableUint8Array
        );
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
    fn unsupported_return_types_are_rejected_neutrally() {
        let parsed = ty("char");
        let err = classify_return(&parsed).expect_err("rejected");
        assert!(std::ptr::eq(err.ty, &parsed), "carries the offending type");
        assert!(classify_return(&ty("&str")).is_err());
        assert!(
            classify_return(&ty("std::string::String")).is_err(),
            "qualified paths stay rejected"
        );
        for src in ["Option<i32>", "Option<u64>", "Option<bool>"] {
            assert!(
                classify_return(&ty(src)).is_err(),
                "`{src}` is not a buffer"
            );
        }
    }

    #[test]
    fn rejected_param_types_fail_classification() {
        let cases = [
            "&mut str",
            "&mut [u8]",
            "&[i32]",
            "&u32",
            "str",
            "i128",
            "usize",
            "char",
            "String",
            "Vec<u8>",
            "*const u8",
        ];
        for src in cases {
            let parsed = ty(src);
            let result = classify_param(&parsed);
            assert!(result.is_err(), "`{src}` must be rejected");
        }
    }

    #[test]
    fn path_helpers_respect_their_contracts() {
        assert_eq!(path_ident(&ty("u32")), Some(("u32".to_owned(), false)));
        assert_eq!(path_ident(&ty("Vec<u8>")), Some(("Vec".to_owned(), true)));
        assert_eq!(path_ident(&ty("std::string::String")), None);
        assert_eq!(path_ident(&ty("()")), None);
        assert!(matches!(
            path_kind(&ty("bool")),
            Some(PathKind::Prim(PrimTy::Bool))
        ));
        assert!(matches!(
            path_kind(&ty("u64")),
            Some(PathKind::BigInt(BigIntTy::U64))
        ));
        assert!(path_kind(&ty("Vec<u8>")).is_none());
        assert!(is_str_type(&ty("str")));
        assert!(!is_str_type(&ty("u8")));
        assert!(is_u8(&ty("u8")));
        assert!(!is_u8(&ty("i8")));
    }
}
