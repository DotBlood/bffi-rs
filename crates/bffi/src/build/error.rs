//! Domain errors of the build/runtime layer and their conversion to
//! [`BffiError`].
//!
//! Display texts are part of the contract: they travel across the C ABI
//! as `BffiError.message` and must stay deterministic.

// Internal module aliases (the pre-merge crate names).
use crate::bffi_core;
use std::fmt;

use bffi_core::{BffiError, ErrorCode, TypeTag};

/// Everything that can go wrong in the `bffi-build` runtime tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// The runtime table has no free slots.
    TableFull(TypeTag),
    /// The crate-owned type tag is already declared - one tag serves one
    /// table per process. Content here means two initializers raced; the
    /// memoized [`std::result::Result`] inside the table initializer keeps
    /// the outcome sticky.
    TagInUse(TypeTag),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TableFull(tag) => write!(f, "bffi-build table for tag {tag} is full"),
            Self::TagInUse(tag) => {
                write!(f, "bffi-build type tag {tag} is already declared")
            }
        }
    }
}

impl std::error::Error for BuildError {}

/// Unified-format conversion: table exhaustion maps to
/// [`ErrorCode::TableFull`], an already-declared tag to
/// [`ErrorCode::InvalidTag`]; the domain error is preserved as the source.
impl From<BuildError> for BffiError {
    fn from(error: BuildError) -> Self {
        let code = match &error {
            BuildError::TableFull(_) => ErrorCode::TableFull,
            BuildError::TagInUse(_) => ErrorCode::InvalidTag,
        };
        BffiError::with_source(code, error.to_string(), error)
    }
}

#[cfg(test)]
mod tests {
    use crate::bffi_core::{ErrorCode, TypeTag};

    use super::BuildError;

    #[test]
    fn display_names_the_table_and_the_cause() {
        assert_eq!(
            BuildError::TableFull(TypeTag(0x0401)).to_string(),
            "bffi-build table for tag 0x0401 is full"
        );
        assert_eq!(
            BuildError::TagInUse(TypeTag(0x0400)).to_string(),
            "bffi-build type tag 0x0400 is already declared"
        );
    }

    #[test]
    fn converts_to_bffi_error_on_existing_codes() {
        let cases = [
            (BuildError::TableFull(TypeTag(0x0401)), ErrorCode::TableFull),
            (BuildError::TagInUse(TypeTag(0x0400)), ErrorCode::InvalidTag),
        ];
        for (error, code) in cases {
            let converted = crate::bffi_core::BffiError::from(error);
            assert_eq!(converted.code, code);
            assert!(
                converted.source.is_some(),
                "the domain error must survive as source"
            );
        }
    }
}
