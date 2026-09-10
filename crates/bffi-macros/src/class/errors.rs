//! Structured diagnostics for the `bffi-class` macros.
//!
//! Every diagnostic is rendered by the shared
//! [`crate::support::diagnostics::MacroDiagnostic`]; this module
//! keeps this crate's constructors with their stable codes and exact
//! message texts. Same scheme as `bffi-macros` (`bffi[E0XX]:` +
//! ` = help: ` + ` = note: `); the numbering CONTINUES after the
//! frozen `E001`-`E004` of `#[bffi]` - do not renumber either series
//! (the UI goldens in `tests/ui` lock them):
//!
//! | Code   | Meaning                                                  |
//! |--------|----------------------------------------------------------|
//! | `E005` | unsupported class shape (not a named struct / generic)   |
//! | `E006` | bad attribute arguments (tag, crate option)              |
//! | `E007` | unsupported method/field shape (`&mut self`, async, ...) |
//! | `E008` | impl binding problems (constructor count/shape)          |

use proc_macro2::Span;
use quote::ToTokens;

// Re-exported so the model/codegen stages can name the shared
// diagnostic type; the constructors below carry this crate's codes.
pub(crate) use crate::support::diagnostics::{DESIGN_NOTE, MacroDiagnostic};

/// The field/method parameter set accepted by `bffi-class` v1.
const FIELD_TYPES: &str = "supported fields: i8|i16|i32|u8|u16|u32|f32|f64|bool (getters only)";
/// The method parameter set (the `#[bffi]` parameter matrix).
const PARAM_TYPES: &str = "supported: i8|i16|i32|i64|u8|u16|u32|u64|f32|f64|bool|&str|&[u8]";
/// The method return set (the `#[bffi]` return matrix, P2 included).
const RETURN_TYPES: &str = "supported returns: parameters|String|Vec<u8>|CopiedBuf|Option<buffer>|Result<T, E: Error + Send + Sync>";

/// `E005` - the annotated item is not a supported class
/// declaration. `what` names the violation.
pub(crate) fn class_shape(span: Span, what: &str) -> syn::Error {
    MacroDiagnostic::new("E005", format!("unsupported class shape: {what}"))
        .with_help("`#[bffi_class]` annotates a non-generic named struct")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
}

/// `E006` - the `tag = ...` attribute argument is missing, not a
/// literal, or outside `0x0100..=0x01FF`. The `crate = "..."` option
/// problems on `#[bffi_class]` funnel through here too (the parse
/// errors carry the specifics).
pub(crate) fn tag(span: Span, what: impl Into<String>) -> syn::Error {
    MacroDiagnostic::new("E006", format!("invalid class tag: {}", what.into()))
        .with_help("use `#[bffi_class(tag = 0x0142)]` with a literal in 0x0100..=0x01FF (bffi-object range)")
        .with_note("one tag = one type per process (bffi-object/Registry)")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
}

/// `E006` - the attribute carries unsupported options. The only
/// option besides `tag` is `crate = "<name>"` (facade-only mode).
/// Anchored at the attribute tokens' span.
pub(crate) fn attr_options(span: Span) -> syn::Error {
    MacroDiagnostic::new(
        "E006",
        "unknown option; only `crate = \"...\"` is supported",
    )
    .with_help("use `#[bffi_impl]` or `#[bffi_impl(crate = \"bffi\")]`")
    .with_note(DESIGN_NOTE)
    .to_compile_error(span)
}

/// `E007` - a method or field outside the accepted matrix. `what`
/// names the violation.
pub(crate) fn method_shape(span: Span, what: &str) -> syn::Error {
    MacroDiagnostic::new("E007", format!("unsupported member shape: {what}"))
        .with_help(format!(
            "methods take `&self` only; {PARAM_TYPES}; {RETURN_TYPES}"
        ))
        .with_help(FIELD_TYPES)
        .with_note("ObjectWrap stores Arc<T>: mutation goes through interior mutability")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
}

/// `E008` - the impl block binding is wrong: zero or multiple
/// `#[bffi_constructor]` fns, a constructor with a receiver, or a
/// constructor not returning `Self`.
pub(crate) fn impl_binding(span: Span, what: &str) -> syn::Error {
    MacroDiagnostic::new("E008", format!("invalid impl binding: {what}"))
        .with_help("exactly one `#[bffi_constructor] pub fn new(...) -> Self` per `#[bffi_impl]`")
        .with_note("an `#[bffi_impl]` without a matching `#[bffi_class]` surfaces as a missing `__bffi_<name>_wrap` function")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
}

/// `E007` for field types, with the field-focused help only.
pub(crate) fn field_type<T: ToTokens>(span: Span, ty_tokens: &T, name: &str) -> syn::Error {
    MacroDiagnostic::new(
        "E007",
        format!(
            "unsupported type `{}` for field `{}`",
            ty_tokens.to_token_stream(),
            name,
        ),
    )
    .with_help(FIELD_TYPES)
    .with_note("only `pub` fields are exported, as read-only getters")
    .with_note(DESIGN_NOTE)
    .to_compile_error(span)
}

/// `E002`-style method parameter rejection (shares the parameter
/// semantics of `#[bffi]` but carries `E007` so class rejections
/// are greppable as one family).
pub(crate) fn method_param<T: ToTokens>(span: Span, ty_tokens: &T, name: &str) -> syn::Error {
    MacroDiagnostic::new(
        "E007",
        format!(
            "unsupported type `{}` for parameter `{}`",
            ty_tokens.to_token_stream(),
            name,
        ),
    )
    .with_help(PARAM_TYPES)
    .with_note(DESIGN_NOTE)
    .to_compile_error(span)
}

/// `E003`-style method return rejection carrying `E007`.
pub(crate) fn method_return<T: ToTokens>(span: Span, ty_tokens: &T) -> syn::Error {
    MacroDiagnostic::new(
        "E007",
        format!(
            "unsupported type `{}` for the return type",
            ty_tokens.to_token_stream(),
        ),
    )
    .with_help(RETURN_TYPES)
    .with_note(DESIGN_NOTE)
    .to_compile_error(span)
}
