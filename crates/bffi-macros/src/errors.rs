//! Spanned error builders for `#[bffi]` rejections.
//!
//! Every diagnostic follows the documented format: a `bffi:` first line
//! followed by two-space-indented `= help:` lines that name the P1 type
//! set and point at DESIGN.md. The exact wording is part of the macro's
//! public contract and is locked by the trybuild goldens in `tests/ui`.

use quote::ToTokens;

/// Help line listing the types accepted in P1 (shared by all type
/// rejections).
const P1_TYPES_HELP: &str =
    "  = help: supported in P1: i8|i16|i32|i64|u8|u16|u32|u64|f32|f64|bool|&str|()";
/// Help line naming what arrives in P2 via `bffi-build`.
const P2_HELP: &str = "  = help: buffers, Option, structs and Result arrive with bffi-build (P2)";
/// Help line pointing at the boundary rules in DESIGN.md.
const DESIGN_HELP: &str = "  = help: boundary rules: DESIGN.md (https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md)";
/// Help line for shape rejections.
const PLAIN_FN_HELP: &str = "  = help: plain `fn`s only in P1";

/// Builds the rejection for a parameter whose type is outside the P1
/// boundary set. Spanned on the offending type.
pub(crate) fn unsupported_param_type<T: ToTokens>(ty: &T, name: &str) -> syn::Error {
    syn::Error::new_spanned(
        ty,
        format!(
            "bffi: unsupported type `{}` for parameter `{}`\n{P1_TYPES_HELP}\n{P2_HELP}\n{DESIGN_HELP}",
            ty.to_token_stream(),
            name,
        ),
    )
}

/// Builds the rejection for a return type outside the P1 boundary set.
/// Spanned on the offending type.
pub(crate) fn unsupported_return_type<T: ToTokens>(ty: &T) -> syn::Error {
    syn::Error::new_spanned(
        ty,
        format!(
            "bffi: unsupported type `{}` for the return type\n{P1_TYPES_HELP}\n{P2_HELP}\n{DESIGN_HELP}",
            ty.to_token_stream(),
        ),
    )
}

/// Builds the rejection for a non-plain function shape (`async fn`,
/// generic `fn`, ...). `what` names the shape; the error is spanned on
/// the offending tokens.
pub(crate) fn unsupported_shape<T: ToTokens>(what: &str, tokens: &T) -> syn::Error {
    syn::Error::new_spanned(
        tokens,
        format!("bffi: unsupported function shape: {what}\n{PLAIN_FN_HELP}\n{DESIGN_HELP}"),
    )
}

/// Builds the rejection for an `#[bffi(...)]` attribute that carries
/// options. Spanned on the attribute tokens.
pub(crate) fn no_options<T: ToTokens>(tokens: &T) -> syn::Error {
    syn::Error::new_spanned(
        tokens,
        format!("bffi: this attribute takes no options\n{PLAIN_FN_HELP}\n{DESIGN_HELP}"),
    )
}
