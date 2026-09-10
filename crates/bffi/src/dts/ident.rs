//! Deterministic identifier sanitization for generated TypeScript
//! declarations.
//!
//! The renderer ([`crate`]) must never emit a declaration that is not a
//! valid JavaScript identifier, and it must do so without ever failing:
//! sanitization is total, deterministic, and cannot panic. The same input
//! always produces the same output.
//!
//! # ASCII-only rule
//!
//! Validity follows the ASCII rule `[A-Za-z_$][A-Za-z0-9_$]*` plus a
//! reserved-word check. This is a deliberate decision: determinism comes
//! first, and JavaScript identifiers are technically Unicode-aware.
//! Unicode identifiers are YAGNI for generated bindings; names that do
//! not satisfy the ASCII rule are simply prefixed with `_` and otherwise
//! preserved verbatim.
//!
//! Use [`sanitize`] when rendering; use [`is_valid_js_identifier`] to
//! only *test* validity.

use std::borrow::Cow;

/// ECMAScript reserved words that are not valid as generated identifiers
/// on their own.
///
/// The list is sorted lexicographically, one word per line, so membership
/// checks can use binary search. Kept crate-private: callers use
/// [`is_valid_js_identifier`] / [`sanitize`] and never match on the list
/// directly.
pub(crate) const RESERVED: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Returns `true` when `name` is a valid JavaScript identifier under the
/// ASCII rule.
///
/// A valid identifier is non-empty, starts with `[A-Za-z_$]`, continues
/// with `[A-Za-z0-9_$]*`, and is not an ECMAScript reserved word (see
/// [`RESERVED`]). The empty string is not valid.
#[must_use]
pub fn is_valid_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let valid_first = first.is_ascii_alphabetic() || first == '_' || first == '$';
    if !valid_first || RESERVED.binary_search(&name).is_ok() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Sanitizes `name` into something safe to emit as a JavaScript
/// identifier.
///
/// Valid identifiers are passed through unchanged as [`Cow::Borrowed`]
/// (no allocation in the common case); invalid, reserved, or empty names
/// are returned as [`Cow::Owned`] with a `_` prefix. Deterministic: the
/// same input always yields the same output, and rendering never panics
/// or fails.
#[must_use]
pub fn sanitize(name: &str) -> Cow<'_, str> {
    if is_valid_js_identifier(name) {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("_{name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_list_is_sorted_for_the_binary_search() {
        // `binary_search` requires sorted input; a `windows(2)` sweep
        // catches an unsorted edit at test time instead of at runtime.
        assert!(
            RESERVED.windows(2).all(|pair| pair[0] < pair[1]),
            "RESERVED must stay strictly sorted"
        );
    }

    #[test]
    fn sanitize_passes_valid_identifiers_through() {
        for name in ["add", "_x", "$y", "a1"] {
            let sanitized = sanitize(name);
            assert!(
                matches!(&sanitized, Cow::Borrowed(_)),
                "expected Borrowed for {name:?}, got {sanitized:?}"
            );
            assert_eq!(sanitized, name);
        }
    }

    #[test]
    fn sanitize_prefixes_reserved_words() {
        for (input, expected) in [
            ("class", "_class"),
            ("delete", "_delete"),
            ("void", "_void"),
        ] {
            let sanitized = sanitize(input);
            assert!(
                matches!(&sanitized, Cow::Owned(_)),
                "expected Owned for {input:?}, got {sanitized:?}"
            );
            assert_eq!(sanitized, expected);
        }
    }

    #[test]
    fn sanitize_prefixes_invalid_names() {
        for (input, expected) in [("", "_"), ("2a", "_2a"), ("a-b", "_a-b"), ("имя", "_имя")]
        {
            let sanitized = sanitize(input);
            assert!(
                matches!(&sanitized, Cow::Owned(_)),
                "expected Owned for {input:?}, got {sanitized:?}"
            );
            assert_eq!(sanitized, expected);
        }
    }

    #[test]
    fn reserved_list_covers_expected_words() {
        for word in ["class", "delete", "void", "function", "export", "return"] {
            assert!(RESERVED.contains(&word), "RESERVED must contain {word:?}");
        }
    }

    #[test]
    fn validity_rules_reject_reserved() {
        assert!(!is_valid_js_identifier("class"));
        assert!(is_valid_js_identifier("add"));
        assert!(!is_valid_js_identifier(""));
    }
}
