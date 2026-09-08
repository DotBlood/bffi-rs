//! Type classification and TypeScript kind mapping.
//!
//! The pure classification lives in `bffi_macro_support::classify`;
//! this module keeps the local signatures the model stage uses and
//! maps neutral rejections onto this crate's `E007` diagnostics
//! (exact texts locked by the `tests/ui` goldens).

use crate::errors::{method_param, method_return};
use bffi_macro_support::classify as support;

pub(crate) use bffi_macro_support::abi;
pub(crate) use bffi_macro_support::classify::{PathKind, path_kind, ts_return, ts_type};
pub(crate) use bffi_macro_support::kind::{BigIntTy, PrimTy, RetKind, ShimKind, TsKind};

/// Classifies a parameter type (the `#[bffi]` parameter matrix).
pub(crate) fn classify_param(ty: &syn::Type, name: &str) -> syn::Result<ShimKind> {
    support::classify_param(ty).map_err(|u| method_param(u.span, u.ty, name))
}

/// Classifies a return type (the `#[bffi]` return matrix, P2 included).
pub(crate) fn classify_return(ty: &syn::Type) -> syn::Result<RetKind> {
    support::classify_return(ty).map_err(|u| method_return(u.span, u.ty))
}
