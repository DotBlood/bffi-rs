//! Configurable crate-root paths for the generated code.
//!
//! The generated tokens name the runtime crates by absolute path,
//! because the expansion lands in the user crate. By default those
//! roots are the direct dependencies `::bffi_core`, `::bffi_types`,
//! `::bffi_dts`, `::bffi_object` and `::bffi_build`.
//!
//! The `crate = "<name>"` attribute option (parsed by the
//! proc-macro crates, which own the diagnostics) swaps every root
//! for the namespaced re-export `::<name>::{core, types, dts,
//! object, build}`. That enables **facade-only mode**: a user crate
//! whose only dependency is the `bffi` facade, which re-exports the
//! stack under exactly those module names.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// The five crate roots the generated code names, plus the
/// `crate = "..."` mapping onto a facade's re-export namespaces.
///
/// [`PathCtx::default`] yields the direct-dependency roots; build
/// with it and the expansion is byte-identical to the hard-coded
/// paths. [`PathCtx::from_attr`] yields the namespaced roots of one
/// facade crate.
#[derive(Clone, Debug)]
pub struct PathCtx {
    /// Root for `bffi-core` (ErrorCode, BffiError, Handle, TypeTag,
    /// `boundary::run_extern_body`, `set_last_error`).
    pub core: TokenStream,
    /// Root for `bffi-types` (CopiedBuf, `str_view`, `buf_view`,
    /// `unsafe_zero_copy`).
    pub types: TokenStream,
    /// Root for `bffi-dts` (the descriptor IR types).
    pub dts: TokenStream,
    /// Root for `bffi-object` (ObjectWrap, ObjectError).
    pub object: TokenStream,
    /// Root for `bffi-build` (`runtime::store_bytes`).
    pub build: TokenStream,
}

impl Default for PathCtx {
    fn default() -> Self {
        Self {
            core: quote! { ::bffi_core },
            types: quote! { ::bffi_types },
            dts: quote! { ::bffi_dts },
            object: quote! { ::bffi_object },
            build: quote! { ::bffi_build },
        }
    }
}

impl PathCtx {
    /// Builds the context for a validated `crate = "<name>"` value:
    /// every root becomes `::<name>::<namespace>` (crate names with
    /// hyphens are referenced with underscores, as Rust requires).
    ///
    /// Validate the name with [`is_crate_name`] first; this
    /// constructor assumes a valid name.
    pub fn from_attr(name: &str) -> PathCtx {
        let root = format_ident!("{}", name.replace('-', "_"));
        Self {
            core: quote! { ::#root::core },
            types: quote! { ::#root::types },
            dts: quote! { ::#root::dts },
            object: quote! { ::#root::object },
            build: quote! { ::#root::build },
        }
    }
}

/// Whether `name` is a valid `crate = "<name>"` value: a non-empty
/// crate-ish identifier (ASCII letters, digits, `_`, `-`; not
/// starting with a digit or `-`) that does not shadow a language
/// crate (`core`/`std`/`alloc`) or a path keyword.
pub fn is_crate_name(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return false;
    }
    // Redirecting the generated absolute paths into the language
    // crates or a keyword would compile to nonsense; reject up front.
    !matches!(
        name,
        "core" | "std" | "alloc" | "crate" | "self" | "super" | "Self"
    )
}

#[cfg(test)]
mod tests {
    use super::{PathCtx, is_crate_name};

    #[test]
    fn default_tokens_are_the_direct_dependency_paths() {
        let ctx = PathCtx::default();
        assert_eq!(ctx.core.to_string(), ":: bffi_core");
        assert_eq!(ctx.types.to_string(), ":: bffi_types");
        assert_eq!(ctx.dts.to_string(), ":: bffi_dts");
        assert_eq!(ctx.object.to_string(), ":: bffi_object");
        assert_eq!(ctx.build.to_string(), ":: bffi_build");
    }

    #[test]
    fn from_attr_maps_onto_the_facade_namespaces() {
        let ctx = PathCtx::from_attr("bffi");
        assert_eq!(ctx.core.to_string(), ":: bffi :: core");
        assert_eq!(ctx.types.to_string(), ":: bffi :: types");
        assert_eq!(ctx.dts.to_string(), ":: bffi :: dts");
        assert_eq!(ctx.object.to_string(), ":: bffi :: object");
        assert_eq!(ctx.build.to_string(), ":: bffi :: build");
    }

    #[test]
    fn from_attr_normalizes_hyphens_to_underscores() {
        let ctx = PathCtx::from_attr("my-facade");
        assert_eq!(ctx.core.to_string(), ":: my_facade :: core");
    }

    #[test]
    fn crate_names_follow_the_crate_ident_rules() {
        assert!(is_crate_name("bffi"));
        assert!(is_crate_name("my-facade"));
        assert!(is_crate_name("my_facade"));
        assert!(is_crate_name("_private"));
        assert!(is_crate_name("a1"));
    }

    #[test]
    fn crate_names_reject_the_language_crates_and_junk() {
        for name in [
            "",
            "core",
            "std",
            "alloc",
            "crate",
            "self",
            "super",
            "Self",
            "1bad",
            "-bad",
            "has space",
            "has.dot",
            "has::path",
        ] {
            assert!(!is_crate_name(name), "`{name}` must be rejected");
        }
    }
}
