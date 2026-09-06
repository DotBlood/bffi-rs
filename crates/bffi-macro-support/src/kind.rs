//! The boundary kind model shared by the proc-macro crates.
//!
//! These enums are the normalized vocabulary both `#[bffi]` and
//! `#[bffi_class]`/`#[bffi_impl]` speak after parsing: every accepted
//! parameter and return type is classified into exactly one of them,
//! and the codegen layer consumes only these kinds. [`TsKind`] is the
//! typed bridge to the `bffi-dts` IR - its [`TsKind::tokens`] quote
//! the IR variants directly, no string round-trip.

use proc_macro2::TokenStream;
use quote::quote;

/// A 64-bit integer crossing the boundary (`i64`/`u64`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BigIntTy {
    /// `i64`
    I64,
    /// `u64`
    U64,
}

/// A small primitive accepted at the boundary: number-ish integers,
/// floats, or `bool`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimTy {
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
pub enum ShimKind {
    /// A small primitive (`number`-ish types and `bool`).
    Prim(PrimTy),
    /// A 64-bit integer (`i64`/`u64`).
    BigInt(BigIntTy),
    /// A borrowed `&str` copied across the boundary.
    Str,
}

/// An owned byte-carrying return type: stored in the `bffi-build`
/// transient-buffer table and handed to JS as a handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferTy {
    /// `String` - UTF-8 bytes; rendered as `string`.
    String,
    /// `Vec<u8>` - raw bytes; rendered as `Uint8Array`.
    ByteVec,
    /// `CopiedBuf` - raw bytes; rendered as `Uint8Array`.
    CopiedBuf,
}

/// The return side of a validated function or method.
// Not `Copy`: `Result` boxes its inner return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetKind {
    /// No return value (`()`).
    Unit,
    /// A small primitive.
    Prim(PrimTy),
    /// A 64-bit integer (`i64`/`u64`).
    BigInt(BigIntTy),
    /// An owned byte payload returned as a transient-buffer handle.
    Buffer(BufferTy),
    /// `Option` of a buffer payload: `None` writes the `0` handle.
    Nullable(BufferTy),
    /// `Result<T, E>`: `Ok` transports `T`, `Err` reports the domain
    /// error through the last-error channel (`ErrorCode::DomainError`).
    Result(Box<RetKind>),
}

/// The TypeScript type of an accepted boundary item, as a
/// `::bffi_dts::TsType` variant token stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsKind {
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
    /// The `::bffi_dts::TsType` variant tokens for this kind.
    pub fn tokens(self) -> TokenStream {
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

#[cfg(test)]
mod tests {
    use super::TsKind;

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
}
