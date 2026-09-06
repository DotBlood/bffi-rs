//! Type classification and TypeScript kind mapping.
//!
//! A deliberately self-contained twin of the `bffi-macros` mapping
//! (proc-macro crates cannot share plain code without an extra common
//! crate; the duplication is a documented P2 trade-off). [`classify_param`]
//! and [`classify_return`] turn a `syn::Type` into the boundary kinds,
//! and [`TsKind`] quotes the `bffi-dts` IR variants directly.

use crate::errors::MacroDiagnostic;
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

/// A small primitive accepted at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrimTy {
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`
    I32,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `bool`
    Bool,
}

/// A 64-bit integer crossing the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BigIntTy {
    /// `i64`
    I64,
    /// `u64`
    U64,
}

/// An owned byte payload returned as a transient-buffer handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BufferTy {
    /// `String` - UTF-8 bytes.
    String,
    /// `Vec<u8>` - raw bytes.
    ByteVec,
    /// `CopiedBuf` - raw bytes.
    CopiedBuf,
}

/// The kind of one parameter at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShimKind {
    /// A small primitive.
    Prim(PrimTy),
    /// A 64-bit integer.
    BigInt(BigIntTy),
    /// A borrowed `&str` copied across the boundary.
    Str,
}

/// The return side of a validated method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RetKind {
    /// No return value (`()`).
    Unit,
    /// A small primitive.
    Prim(PrimTy),
    /// A 64-bit integer.
    BigInt(BigIntTy),
    /// An owned byte payload returned as a handle.
    Buffer(BufferTy),
    /// `Option` of a payload: `None` writes the `0` handle.
    Nullable(BufferTy),
    /// `Result<T, E>`: `Ok` transports `T`, `Err` -> `DomainError`.
    Result(Box<RetKind>),
}

/// Kind of a plain path type: a small primitive or a 64-bit integer.
pub(crate) enum PathKind {
    /// A small primitive.
    Prim(PrimTy),
    /// A 64-bit integer.
    BigInt(BigIntTy),
}

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

fn bigint_from_str(name: &str) -> Option<BigIntTy> {
    match name {
        "i64" => Some(BigIntTy::I64),
        "u64" => Some(BigIntTy::U64),
        _ => None,
    }
}

/// The single-segment identifier of a plain path type, plus whether it
/// carries generic arguments.
pub(crate) fn path_ident(ty: &syn::Type) -> Option<(String, bool)> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    Some((segment.ident.to_string(), !segment.arguments.is_none()))
}

/// Classifies `ty` when it is a generic-free single-segment path.
pub(crate) fn path_kind(ty: &syn::Type) -> Option<PathKind> {
    let (name, has_arguments) = path_ident(ty)?;
    if has_arguments {
        return None;
    }
    if let Some(prim) = prim_from_str(&name) {
        return Some(PathKind::Prim(prim));
    }
    bigint_from_str(&name).map(PathKind::BigInt)
}

/// Whether `ty` is the bare `str` path type.
fn is_str_type(ty: &syn::Type) -> bool {
    matches!(path_ident(ty), Some((name, false)) if name == "str")
}

fn buffer_from_str(name: &str) -> Option<BufferTy> {
    match name {
        "String" => Some(BufferTy::String),
        "CopiedBuf" => Some(BufferTy::CopiedBuf),
        _ => None,
    }
}

/// Classifies a parameter type (the `#[bffi]` parameter matrix).
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
    Err(MacroDiagnostic::method_param(ty.span(), ty, name))
}

/// Classifies a return type (the `#[bffi]` return matrix, P2 included).
pub(crate) fn classify_return(ty: &syn::Type) -> syn::Result<RetKind> {
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
                if path_ident(err).is_none() {
                    return Err(MacroDiagnostic::method_return(ty.span(), ty));
                }
                return Ok(RetKind::Result(Box::new(inner)));
            }
            _ => {}
        }
    }
    Err(MacroDiagnostic::method_return(ty.span(), ty))
}

/// Classifies `ty` when it is exactly a buffer payload.
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

/// The generic argument types of a single-segment path type.
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

/// TypeScript kind of a parameter kind.
pub(crate) fn ts_type(kind: &ShimKind) -> TsKind {
    match kind {
        ShimKind::Prim(prim) => ts_prim(*prim),
        ShimKind::BigInt(_) => TsKind::BigInt,
        ShimKind::Str => TsKind::String,
    }
}

/// TypeScript kind of a return kind (`Nullable` renders as its
/// payload; the nullability lives in the descriptor's doc line).
pub(crate) fn ts_return(ret: &RetKind) -> TsKind {
    match ret {
        RetKind::Unit => TsKind::Void,
        RetKind::Prim(prim) => ts_prim(*prim),
        RetKind::BigInt(_) => TsKind::BigInt,
        RetKind::Buffer(BufferTy::String) => TsKind::String,
        RetKind::Buffer(_) => TsKind::Uint8Array,
        RetKind::Nullable(inner) => ts_return(&RetKind::Buffer(*inner)),
        RetKind::Result(inner) => ts_return(inner),
    }
}
