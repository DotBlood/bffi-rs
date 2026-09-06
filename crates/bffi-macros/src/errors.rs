//! Structured diagnostics for `#[bffi]` rejections.
//!
//! Every diagnostic is a [`MacroDiagnostic`]: a stable code plus a
//! message, actionable help lines and context notes, rendered into a
//! `compile_error!`. This is the compile-time counterpart of the
//! runtime `BffiError` scheme (code + message + source); the runtime
//! mapping of these errors to JS errors is `bffi-error`'s job, and the
//! macro never depends on it.
//!
//! Stable codes (do not renumber; UI goldens in `tests/ui` lock them):
//!
//! | Code   | Meaning                                                 |
//! |--------|---------------------------------------------------------|
//! | `E001` | unsupported function shape (async/generic/unsafe/...)   |
//! | `E002` | unsupported parameter type                              |
//! | `E003` | unsupported return type                                 |
//! | `E004` | unsupported attribute options                           |

use proc_macro2::Span;
use quote::ToTokens;

/// The P1 type set, listed in every type-rejection help line.
const P1_TYPES: &str = "supported in P1: i8|i16|i32|i64|u8|u16|u32|u64|f32|f64|bool|&str|()";
/// Note pointing at the boundary rules in DESIGN.md (shared by all
/// diagnostics).
const DESIGN_NOTE: &str =
    "boundary rules: DESIGN.md (https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md)";

/// One structured `#[bffi]` diagnostic: a stable code, a message,
/// actionable help lines and context notes. Rendered to a
/// `compile_error!` - the compile-time counterpart of the runtime
/// `BffiError` (code + message + source) scheme; runtime mapping to
/// JS errors is `bffi-error`'s job.
pub(crate) struct MacroDiagnostic {
    /// Stable code, `E001` .. `E004`.
    code: &'static str,
    /// First-line message (without the `bffi[code]: ` prefix).
    message: String,
    /// Actionable `  = help: ` lines.
    help: Vec<String>,
    /// Context `  = note: ` lines.
    notes: Vec<String>,
}

impl MacroDiagnostic {
    /// Starts a diagnostic with the given stable code and first-line
    /// message.
    fn new(code: &'static str, message: String) -> Self {
        Self {
            code,
            message,
            help: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Appends an actionable `  = help: ` line.
    fn with_help(mut self, line: impl Into<String>) -> Self {
        self.help.push(line.into());
        self
    }

    /// Appends a context `  = note: ` line.
    fn with_note(mut self, line: impl Into<String>) -> Self {
        self.notes.push(line.into());
        self
    }

    /// Adds the help/note lines shared by all shape-style diagnostics.
    fn with_shape_guidance(self) -> Self {
        self.with_help("plain `fn`s only in P1")
            .with_note(DESIGN_NOTE)
    }

    /// Renders into a `syn::Error` anchored at `span`: a
    /// `bffi[<code>]: <message>` first line, then `  = help: ` per
    /// help line, then `  = note: ` per note.
    // Consuming `self` (and the `to_*` name) is the documented contract
    // for this renderer; the diagnostic is single-use.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_compile_error(self, span: Span) -> syn::Error {
        let mut lines = vec![format!("bffi[{}]: {}", self.code, self.message)];
        lines.extend(self.help.iter().map(|h| format!("  = help: {h}")));
        lines.extend(self.notes.iter().map(|n| format!("  = note: {n}")));
        syn::Error::new(span, lines.join("\n"))
    }

    /// `E001` - the function shape is outside the P1 rules (`async`,
    /// generic, `unsafe`, method receiver, variadic, `extern`,
    /// `const`). `what` names the shape; anchored at the offending
    /// tokens' span.
    pub(crate) fn fn_shape(span: Span, what: &str) -> syn::Error {
        Self::new("E001", format!("unsupported function shape: {what}"))
            .with_shape_guidance()
            .to_compile_error(span)
    }

    /// `E001` - the parameter pattern is outside the P1 rules: only
    /// identifiers and `_` name a boundary parameter. Anchored at the
    /// offending pattern.
    pub(crate) fn param_pattern(span: Span) -> syn::Error {
        Self::new(
            "E001",
            "unsupported function shape: non-identifier parameter pattern".to_owned(),
        )
        .with_help("use `name: Type` or `_`: Type")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
    }

    /// `E002` - a parameter type outside the P1 boundary set, anchored
    /// at the offending type.
    pub(crate) fn param_type<T: ToTokens>(span: Span, ty_tokens: &T, name: &str) -> syn::Error {
        Self::new(
            "E002",
            format!(
                "unsupported type `{}` for parameter `{}`",
                ty_tokens.to_token_stream(),
                name,
            ),
        )
        .with_help(P1_TYPES)
        .with_note("buffers, Option, structs and Result arrive with bffi-build (P2)")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
    }

    /// `E003` - a return type outside the P1 boundary set (everything
    /// but primitives, `i64`/`u64` and `()`), anchored at the
    /// offending type.
    pub(crate) fn return_type<T: ToTokens>(span: Span, ty_tokens: &T) -> syn::Error {
        Self::new(
            "E003",
            format!(
                "unsupported type `{}` for the return type",
                ty_tokens.to_token_stream(),
            ),
        )
        .with_help(P1_TYPES)
        .with_note("buffers, Option, structs and Result arrive with bffi-build (P2)")
        .with_note(DESIGN_NOTE)
        .to_compile_error(span)
    }

    /// `E004` - the attribute carries options; none are supported yet.
    /// Anchored at the attribute tokens' span.
    pub(crate) fn attr_options(span: Span) -> syn::Error {
        Self::new("E004", "this attribute takes no options".to_owned())
            .with_shape_guidance()
            .to_compile_error(span)
    }
}

#[cfg(test)]
mod tests {
    use super::{MacroDiagnostic, Span};
    use quote::quote;

    #[test]
    fn param_type_diagnostic_renders_code_help_and_notes() {
        let err = MacroDiagnostic::param_type(Span::call_site(), &quote! { Vec<u8> }, "data");
        let text = err.to_string();
        assert!(
            text.contains("bffi[E002]: unsupported type `Vec < u8 >` for parameter `data`")
                && text.contains(
                    "  = help: supported in P1: i8|i16|i32|i64|u8|u16|u32|u64|f32|f64|bool|&str|()"
                )
                && text.contains(
                    "  = note: buffers, Option, structs and Result arrive with bffi-build (P2)"
                )
                && text.contains(
                    "  = note: boundary rules: DESIGN.md (https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md)"
                )
        );
    }
}
