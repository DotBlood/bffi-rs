//! Callback values and callback signatures.
//!
//! The signature IR of the callback layer: [`Value`] is a dynamically
//! typed payload crossing the boundary, [`ValueType`] is its static
//! classification, and [`CallbackSig`] validates that a slice of
//! [`Value`]s matches a declared callback signature (arity first, then
//! per-element types).

use bffi_types::wire;

/// The static classification of a [`Value`] payload crossing the
/// callback boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ValueType {
    /// 32-bit signed integer.
    I32,
    /// 64-bit signed integer.
    I64,
    /// 64-bit IEEE-754 floating-point number.
    F64,
    /// Boolean.
    Bool,
}

impl ValueType {
    /// The wire tag of this value type (`bffi_types::wire`, the
    /// framework-wide codec table).
    #[must_use]
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::I32 => wire::TAG_I32,
            Self::I64 => wire::TAG_I64,
            Self::F64 => wire::TAG_F64,
            Self::Bool => wire::TAG_BOOL,
        }
    }

    /// Parses a wire tag into this type; `None` for tags outside the
    /// callback value matrix.
    #[must_use]
    pub const fn from_wire_tag(tag: u8) -> Option<Self> {
        match tag {
            wire::TAG_I32 => Some(Self::I32),
            wire::TAG_I64 => Some(Self::I64),
            wire::TAG_F64 => Some(Self::F64),
            wire::TAG_BOOL => Some(Self::Bool),
            _ => None,
        }
    }
}

/// A dynamically typed payload passed to or from a callback.
///
/// Every variant pairs its payload with an implicit [`ValueType`]
/// available through [`Value::ty`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// A 32-bit signed integer.
    I32(i32),
    /// A 64-bit signed integer.
    I64(i64),
    /// A 64-bit IEEE-754 floating-point number.
    F64(f64),
    /// A boolean.
    Bool(bool),
}

impl Value {
    /// The static [`ValueType`] of this value.
    #[must_use]
    pub const fn ty(self) -> ValueType {
        match self {
            Self::I32(_) => ValueType::I32,
            Self::I64(_) => ValueType::I64,
            Self::F64(_) => ValueType::F64,
            Self::Bool(_) => ValueType::Bool,
        }
    }

    /// The wire bytes of this value: one `[tag][payload]` record in
    /// the framework-wide codec (`bffi_types::wire`).
    #[must_use]
    pub fn encode(self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(&mut out);
        out
    }

    /// Encodes this value into an existing buffer (the multi-argument
    /// form of [`Value::encode`]).
    pub fn encode_into(self, out: &mut Vec<u8>) {
        match self {
            Self::I32(v) => {
                out.push(wire::TAG_I32);
                wire::push_i32_le(out, v);
            }
            Self::I64(v) => {
                out.push(wire::TAG_I64);
                wire::push_i64_le(out, v);
            }
            Self::F64(v) => {
                out.push(wire::TAG_F64);
                wire::push_f64_le(out, v);
            }
            Self::Bool(v) => {
                out.push(wire::TAG_BOOL);
                wire::push_bool(out, v);
            }
        }
    }
}

/// The declared signature of a callback: a return type plus an ordered
/// list of parameter types.
///
/// Use [`CallbackSig::matches`] to check that a slice of [`Value`]
/// arguments satisfies the signature before invoking the callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackSig {
    ret: ValueType,
    params: Vec<ValueType>,
}

impl CallbackSig {
    /// Builds a signature returning `ret` and taking `params` (in
    /// declaration order).
    pub fn new(ret: ValueType, params: &[ValueType]) -> Self {
        Self {
            ret,
            params: params.to_vec(),
        }
    }

    /// The declared return type.
    #[must_use]
    pub fn ret(&self) -> ValueType {
        self.ret
    }

    /// The declared parameter types, in declaration order.
    #[must_use]
    pub fn params(&self) -> &[ValueType] {
        &self.params
    }

    /// Whether `args` satisfies this signature: the arity must agree
    /// and every element's [`Value::ty`] must equal the declared
    /// parameter type at the same position.
    #[must_use]
    pub fn matches(&self, args: &[Value]) -> bool {
        args.len() == self.params.len()
            && args
                .iter()
                .zip(self.params.iter())
                .all(|(arg, param)| arg.ty() == *param)
    }
}

#[cfg(test)]
mod tests {
    use super::{CallbackSig, Value, ValueType};

    #[test]
    fn value_matches_when_arity_and_types_align() {
        let sig = CallbackSig::new(ValueType::Bool, &[ValueType::I32, ValueType::F64]);
        let args = [Value::I32(1), Value::F64(2.0)];
        assert!(sig.matches(&args));
    }

    #[test]
    fn value_rejects_arity_mismatch() {
        let sig = CallbackSig::new(ValueType::Bool, &[ValueType::I32, ValueType::F64]);
        let shorter = [Value::I32(1)];
        let longer = [Value::I32(1), Value::F64(2.0), Value::Bool(true)];
        assert!(
            !sig.matches(&shorter),
            "fewer args than params must not match"
        );
        assert!(
            !sig.matches(&longer),
            "more args than params must not match"
        );
    }

    #[test]
    fn value_rejects_type_mismatch() {
        let sig = CallbackSig::new(ValueType::Bool, &[ValueType::I32]);
        let args = [Value::I64(1)];
        assert!(
            !sig.matches(&args),
            "I64 where I32 is declared must not match"
        );
    }

    #[test]
    fn value_empty_params_match_empty_args() {
        let sig = CallbackSig::new(ValueType::Bool, &[]);
        assert!(sig.matches(&[]));
    }

    #[test]
    fn value_ty_reports_the_variant() {
        assert_eq!(Value::I32(1).ty(), ValueType::I32);
        assert_eq!(Value::I64(2).ty(), ValueType::I64);
        assert_eq!(Value::F64(3.0).ty(), ValueType::F64);
        assert_eq!(Value::Bool(true).ty(), ValueType::Bool);
    }

    #[test]
    fn sig_accessors_roundtrip() {
        let params = [
            ValueType::I32,
            ValueType::I64,
            ValueType::F64,
            ValueType::Bool,
        ];
        let sig = CallbackSig::new(ValueType::F64, &params);
        assert_eq!(sig.ret(), ValueType::F64);
        assert_eq!(sig.params(), &params);
    }

    #[test]
    fn sig_types_are_copy_and_comparable() {
        fn assert_copy<T: Copy>() {}
        fn assert_clone<T: Clone>() {}
        fn assert_partial_eq<T: PartialEq>() {}

        assert_copy::<Value>();
        assert_copy::<ValueType>();
        assert_clone::<CallbackSig>();
        assert_partial_eq::<CallbackSig>();

        let params = [ValueType::I32, ValueType::Bool];
        let first = CallbackSig::new(ValueType::I64, &params);
        let second = CallbackSig::new(ValueType::I64, &params);
        assert_eq!(first, second, "identical signatures must compare equal");

        let different_ret = CallbackSig::new(ValueType::F64, &params);
        assert_ne!(
            first, different_ret,
            "different return types must not be equal"
        );

        let different_params = [ValueType::I32];
        let third = CallbackSig::new(ValueType::I64, &different_params);
        assert_ne!(first, third, "different parameter lists must not be equal");
    }
}
