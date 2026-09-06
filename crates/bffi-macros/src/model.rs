//! The parsed and validated function model.
//!
//! [`FnModel::parse`] turns the annotated item into a normalized model
//! (identity, visibility, docs, params, return) that the shim and
//! descriptor generators in later stages consume. Every input outside
//! the P1 boundary rules is rejected here with a spanned error, so the
//! downstream stages can rely on the shape being valid.

use crate::{errors, mapping};
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{FnArg, ItemFn, Pat, ReturnType};

/// A 64-bit integer crossing the boundary (`i64`/`u64`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BigIntTy {
    /// `i64`
    I64,
    /// `u64`
    U64,
}

/// A small primitive accepted at the boundary: number-ish integers,
/// floats, or `bool`.
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

/// The kind of one parameter (or return) at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShimKind {
    /// A small primitive (`number`-ish types and `bool`).
    Prim(PrimTy),
    /// A 64-bit integer (`i64`/`u64`).
    BigInt(BigIntTy),
    /// A borrowed `&str` copied across the boundary.
    Str,
}

/// The return side of a validated function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FnReturn {
    /// No return value (`()`).
    Unit,
    /// A small primitive.
    Prim(PrimTy),
    /// A 64-bit integer (`i64`/`u64`).
    BigInt(BigIntTy),
}

/// One validated parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FnParam {
    /// Parameter name as written (identifier or rendered pattern).
    pub name: String,
    /// Boundary kind of the parameter type.
    pub kind: ShimKind,
}

/// A validated `#[bffi]` function: everything later stages need to
/// generate the C ABI shim and the `bffi_meta` descriptor.
// The fields are consumed by the shim/meta generators landing in
// Tasks 3-4; until then the non-test build sees them as dead.
#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct FnModel {
    /// Function name.
    pub ident: syn::Ident,
    /// Function visibility.
    pub vis: syn::Visibility,
    /// Doc-comment lines with exactly one leading space trimmed
    /// (`/// Adds.` becomes `Adds.`).
    pub docs: Vec<String>,
    /// Parameters in declaration order (receivers are rejected).
    pub params: Vec<FnParam>,
    /// Validated return type.
    pub ret: FnReturn,
}

impl FnModel {
    /// Parses and validates the annotated item against the P1
    /// boundary rules.
    ///
    /// `attrs` must be empty (the attribute takes no options); `item`
    /// must be a plain, non-generic, non-async, safe `fn` over the P1
    /// type set. Rejections are spanned on the offending tokens and
    /// carry the documented help lines.
    pub(crate) fn parse(attrs: &TokenStream, item: TokenStream) -> syn::Result<FnModel> {
        if !attrs.is_empty() {
            return Err(errors::no_options(attrs));
        }
        let func: ItemFn = syn::parse2(item)?;
        validate_shape(&func.sig)?;

        let mut params = Vec::new();
        for arg in &func.sig.inputs {
            // Receivers are rejected by `validate_shape`, so every
            // remaining argument is a typed parameter.
            let FnArg::Typed(arg) = arg else { continue };
            let name = match &*arg.pat {
                Pat::Ident(pat) => pat.ident.to_string(),
                // Non-identifier patterns cannot be named; keep the
                // rendered pattern so diagnostics stay lossless.
                other => other.to_token_stream().to_string(),
            };
            let kind = mapping::classify_param(&arg.ty, &name)?;
            params.push(FnParam { name, kind });
        }

        let ret = match &func.sig.output {
            ReturnType::Default => FnReturn::Unit,
            ReturnType::Type(_, ty) => mapping::classify_return(ty)?,
        };

        Ok(FnModel {
            ident: func.sig.ident,
            vis: func.vis,
            docs: extract_docs(&func.attrs),
            params,
            ret,
        })
    }
}

/// Rejects every non-plain function shape, earliest violation first,
/// with the error spanned on the offending token.
fn validate_shape(sig: &syn::Signature) -> syn::Result<()> {
    if let Some(tokens) = &sig.asyncness {
        return Err(errors::unsupported_shape("async function", tokens));
    }
    if !sig.generics.params.is_empty() {
        return Err(errors::unsupported_shape(
            "generic function",
            &sig.generics.params,
        ));
    }
    if let Some(where_clause) = &sig.generics.where_clause {
        return Err(errors::unsupported_shape("generic function", where_clause));
    }
    if let Some(tokens) = &sig.unsafety {
        return Err(errors::unsupported_shape("unsafe function", tokens));
    }
    if let Some(FnArg::Receiver(recv)) = sig.inputs.first() {
        return Err(errors::unsupported_shape("method (self receiver)", recv));
    }
    if let Some(tokens) = &sig.variadic {
        return Err(errors::unsupported_shape("variadic function", tokens));
    }
    if let Some(tokens) = &sig.abi {
        return Err(errors::unsupported_shape("extern abi function", tokens));
    }
    if let Some(tokens) = &sig.constness {
        return Err(errors::unsupported_shape("const function", tokens));
    }
    Ok(())
}

/// Collects `///` doc-comment lines, trimming exactly one leading
/// space (`/// Adds.` becomes `Adds.`).
fn extract_docs(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(expr) = &meta.value else {
            continue;
        };
        let syn::Lit::Str(lit) = &expr.lit else {
            continue;
        };
        let mut doc = lit.value();
        if doc.starts_with(' ') {
            doc = doc[1..].to_owned();
        }
        docs.push(doc);
    }
    docs
}
