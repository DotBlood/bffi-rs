//! Domain errors of the object layer and their conversion to
//! [`BffiError`].
//!
//! One variant per failure mode of [`crate::wrap::ObjectWrap`]; the
//! conversion targets EXISTING `ErrorCode`s only (P1 rule: no new codes
//! in P1).

use bffi_core::{BffiError, ErrorCode, Handle, TypeTag};
use std::fmt;

/// Everything that can go wrong in the object layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObjectError {
    /// The tag is outside the `bffi-object` range `0x0100-0x01FF`.
    TagOutOfRange(TypeTag),
    /// The tag is already declared - one tag serves one type per process.
    TagInUse(TypeTag),
    /// The handle is null, stale (already released), or belongs to
    /// another type.
    InvalidHandle(Handle),
    /// The table declared for the tag has no free slots.
    TableFull(TypeTag),
}

impl fmt::Display for ObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TagOutOfRange(tag) => {
                write!(
                    f,
                    "type tag {tag} is outside the bffi-object range 0x0100-0x01FF"
                )
            }
            Self::TagInUse(tag) => {
                write!(
                    f,
                    "type tag {tag} is already declared (one tag = one type per process)"
                )
            }
            Self::InvalidHandle(handle) => {
                write!(
                    f,
                    "object handle {handle} is stale (already released) or belongs to another type"
                )
            }
            Self::TableFull(tag) => {
                write!(f, "object table for type tag {tag} is full")
            }
        }
    }
}

impl std::error::Error for ObjectError {}

/// Unified-format conversion on existing codes: tag problems map to
/// `InvalidTag`, stale handles to `InvalidHandle`, a full table to
/// `TableFull`; the domain error is preserved as the source.
impl From<ObjectError> for BffiError {
    fn from(error: ObjectError) -> Self {
        let code = match &error {
            ObjectError::TagOutOfRange(_) | ObjectError::TagInUse(_) => ErrorCode::InvalidTag,
            ObjectError::InvalidHandle(_) => ErrorCode::InvalidHandle,
            ObjectError::TableFull(_) => ErrorCode::TableFull,
        };
        BffiError::with_source(code, error.to_string(), error)
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectError;
    use bffi_core::{BffiError, ErrorCode, Handle, TypeTag};

    #[test]
    fn invalid_handle_display_names_the_handle_and_cause() {
        let handle = Handle::new(TypeTag(0x0101), 3, 7);
        let error = ObjectError::InvalidHandle(handle);
        assert_eq!(
            error.to_string(),
            "object handle Handle(tag:0x0101, gen:3, idx:7) is stale (already released) or belongs to another type"
        );
    }

    #[test]
    fn tag_errors_display_the_range_and_uniqueness_rule() {
        assert_eq!(
            ObjectError::TagOutOfRange(TypeTag(0x0200)).to_string(),
            "type tag 0x0200 is outside the bffi-object range 0x0100-0x01FF"
        );
        assert_eq!(
            ObjectError::TagInUse(TypeTag(0x0100)).to_string(),
            "type tag 0x0100 is already declared (one tag = one type per process)"
        );
    }

    #[test]
    fn converts_to_bffi_error_on_existing_codes() {
        let tag = TypeTag(0x0100);
        let cases = [
            (ObjectError::TagOutOfRange(tag), ErrorCode::InvalidTag),
            (ObjectError::TagInUse(tag), ErrorCode::InvalidTag),
            (
                ObjectError::InvalidHandle(Handle::NULL),
                ErrorCode::InvalidHandle,
            ),
            (ObjectError::TableFull(tag), ErrorCode::TableFull),
        ];
        for (error, code) in cases {
            let converted = BffiError::from(error);
            assert_eq!(converted.code, code);
            assert!(
                converted.source.is_some(),
                "the domain error must survive as source"
            );
        }
    }
}
