//! The value a completed task delivers to JavaScript.
//!
//! Every variant encodes into a transient-buffer payload
//! (`[tag: u8][payload]`) so the resolve callback keeps ONE shape:
//! a `u64` handle JS reads through the `bffi_buffer` pair and frees
//! with `bffi_types_free`. Exactness is preserved for `i64`/`u64`
//! (no `f64` narrowing).

use bffi_types::CopiedBuf;

/// The output value of a spawned task.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AsyncValue {
    /// No value.
    Unit,
    /// A 32-bit signed integer.
    I32(i32),
    /// A 64-bit signed integer.
    I64(i64),
    /// A double.
    F64(f64),
    /// A boolean.
    Bool(bool),
    /// A UTF-8 string (copied into the payload).
    Str(String),
    /// Raw bytes (copied into the payload).
    Bytes(CopiedBuf),
}

/// The payload tag byte shared with the JS decoder (`load.ts`).
pub(crate) mod tag {
    pub(crate) const UNIT: u8 = 0;
    pub(crate) const I32: u8 = 1;
    pub(crate) const I64: u8 = 2;
    pub(crate) const F64: u8 = 3;
    pub(crate) const BOOL: u8 = 4;
    pub(crate) const STR: u8 = 5;
    pub(crate) const BYTES: u8 = 6;
}

impl AsyncValue {
    /// Encodes the value into the transient-buffer payload:
    /// `[tag][payload...]`. `Str` payloads are UTF-8 with a `u32`
    /// length prefix; `Bytes` payloads are raw bytes with a `u32`
    /// length prefix.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Self::Unit => out.push(tag::UNIT),
            Self::I32(v) => {
                out.push(tag::I32);
                out.extend_from_slice(&v.to_le_bytes());
            }
            Self::I64(v) => {
                out.push(tag::I64);
                out.extend_from_slice(&v.to_le_bytes());
            }
            Self::F64(v) => {
                out.push(tag::F64);
                out.extend_from_slice(&v.to_le_bytes());
            }
            Self::Bool(v) => {
                out.push(tag::BOOL);
                out.push(u8::from(*v));
            }
            Self::Str(text) => {
                out.push(tag::STR);
                out.extend_from_slice(&(text.len() as u32).to_le_bytes());
                out.extend_from_slice(text.as_bytes());
            }
            Self::Bytes(bytes) => {
                out.push(tag::BYTES);
                out.extend_from_slice(&(bytes.as_slice().len() as u32).to_le_bytes());
                out.extend_from_slice(bytes.as_slice());
            }
        }
        out
    }
}

/// Lossless conversions feeding the `#[bffi_async]` codegen: an
/// async fn's return value is turned into an [`AsyncValue`] with
/// `.into()` at the spawn boundary.
///
/// `u8`/`u16` widen into [`AsyncValue::I32`]; `u32` widens into
/// [`AsyncValue::I64`] (its range exceeds `i32`).
macro_rules! impl_from_into_i32 {
    ($($ty:ty),* $(,)?) => {$(
        impl From<$ty> for AsyncValue {
            fn from(value: $ty) -> Self {
                Self::I32(i32::from(value))
            }
        }
    )*};
}

impl_from_into_i32!(i8, i16, i32, u8, u16);

macro_rules! impl_from_into_i64 {
    ($($ty:ty),* $(,)?) => {$(
        impl From<$ty> for AsyncValue {
            fn from(value: $ty) -> Self {
                Self::I64(i64::from(value))
            }
        }
    )*};
}

impl_from_into_i64!(u32);

impl From<i64> for AsyncValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for AsyncValue {
    fn from(value: u64) -> Self {
        Self::I64(value as i64)
    }
}

impl From<f64> for AsyncValue {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<f32> for AsyncValue {
    fn from(value: f32) -> Self {
        Self::F64(f64::from(value))
    }
}

impl From<bool> for AsyncValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<String> for AsyncValue {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl From<&'static str> for AsyncValue {
    fn from(value: &'static str) -> Self {
        Self::Str(value.to_owned())
    }
}

impl From<Vec<u8>> for AsyncValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(CopiedBuf::from_vec(value))
    }
}

impl From<CopiedBuf> for AsyncValue {
    fn from(value: CopiedBuf) -> Self {
        Self::Bytes(value)
    }
}

impl From<()> for AsyncValue {
    fn from((): ()) -> Self {
        Self::Unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_encodes_as_a_single_tag() {
        assert_eq!(AsyncValue::Unit.encode(), vec![tag::UNIT]);
    }

    #[test]
    fn primitives_encode_little_endian_after_the_tag() {
        assert_eq!(
            AsyncValue::I32(-2).encode(),
            vec![tag::I32, 0xFE, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            AsyncValue::I64(1).encode(),
            vec![tag::I64, 1, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(AsyncValue::Bool(true).encode(), vec![tag::BOOL, 1]);
    }

    #[test]
    fn strings_and_bytes_carry_a_u32_length_prefix() {
        let encoded = AsyncValue::Str("hey".to_owned()).encode();
        assert_eq!(encoded, vec![tag::STR, 3, 0, 0, 0, b'h', b'e', b'y']);

        let encoded = AsyncValue::Bytes(CopiedBuf::from_slice(&[9, 8])).encode();
        assert_eq!(encoded, vec![tag::BYTES, 2, 0, 0, 0, 9, 8]);
    }
}
