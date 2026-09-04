//! The mapping table: [`ErrorCode`] to [`JsErrorName`], plus helpers that
//! drain the thread-local last error straight into a [`JsErrorShape`].

use bffi_core::{BffiError, ErrorCode, take_last_error};

use crate::js_error::{JsErrorName, JsErrorShape};

/// Classifies an [`ErrorCode`] into the JavaScript error constructor that
/// must raise it.
///
/// The rule of thumb:
///
/// - **wrong kind or shape of a value** -> [`JsErrorName::TypeError`]:
///   `InvalidUtf8`, `NullPointer`, `InvalidArgument`;
/// - **value out of an allowed range** -> [`JsErrorName::RangeError`]:
///   `NumberOutOfRange`, `BufferTooSmall`;
/// - **everything else** (generic failures, caught panics, stale handles,
///   exhausted tables) -> [`JsErrorName::Error`].
#[must_use]
pub const fn js_error_name(code: ErrorCode) -> JsErrorName {
    match code {
        ErrorCode::InvalidUtf8 | ErrorCode::NullPointer | ErrorCode::InvalidArgument => {
            JsErrorName::TypeError
        }
        ErrorCode::NumberOutOfRange | ErrorCode::BufferTooSmall => JsErrorName::RangeError,
        _ => JsErrorName::Error,
    }
}

/// Converts bffi failures into JavaScript error shapes.
pub trait JsErrorExt {
    /// Builds the [`JsErrorShape`] describing the JS error to raise.
    #[must_use]
    fn to_js_shape(&self) -> JsErrorShape;

    /// Returns only the constructor name for this failure.
    #[must_use]
    fn to_js_name(&self) -> JsErrorName;
}

impl JsErrorExt for ErrorCode {
    fn to_js_shape(&self) -> JsErrorShape {
        JsErrorShape::new(js_error_name(*self), self.to_string())
    }

    fn to_js_name(&self) -> JsErrorName {
        js_error_name(*self)
    }
}

impl JsErrorExt for BffiError {
    fn to_js_shape(&self) -> JsErrorShape {
        JsErrorShape::new(js_error_name(self.code), self.message.clone())
    }

    fn to_js_name(&self) -> JsErrorName {
        js_error_name(self.code)
    }
}

/// Drains the thread-local [last error](bffi_core) and maps it into a
/// [`JsErrorShape`], ready for the JS-facing layer to raise.
///
/// Returns `None` when no error is stored; like [`bffi_core::take_last_error`]
/// this clears the slot, so a second call on the same thread returns `None`.
#[must_use]
pub fn take_last_error_shape() -> Option<JsErrorShape> {
    take_last_error().map(|error| error.to_js_shape())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js_error::JsErrorName;

    #[test]
    fn every_code_maps_to_the_documented_name() {
        let expected: &[(ErrorCode, JsErrorName)] = &[
            (ErrorCode::Ok, JsErrorName::Error),
            (ErrorCode::Error, JsErrorName::Error),
            (ErrorCode::Panic, JsErrorName::Error),
            (ErrorCode::NullHandle, JsErrorName::Error),
            (ErrorCode::InvalidHandle, JsErrorName::Error),
            (ErrorCode::TableFull, JsErrorName::Error),
            (ErrorCode::InvalidTag, JsErrorName::Error),
            (ErrorCode::InvalidUtf8, JsErrorName::TypeError),
            (ErrorCode::NullPointer, JsErrorName::TypeError),
            (ErrorCode::InvalidArgument, JsErrorName::TypeError),
            (ErrorCode::NumberOutOfRange, JsErrorName::RangeError),
            (ErrorCode::BufferTooSmall, JsErrorName::RangeError),
        ];
        for (code, name) in expected {
            assert_eq!(&js_error_name(*code), name, "code {code:?}");
            assert_eq!(&code.to_js_name(), name, "code {code:?}");
        }
    }

    #[test]
    fn code_shape_uses_the_code_text_as_message() {
        let shape = ErrorCode::TableFull.to_js_shape();
        assert_eq!(shape.name, JsErrorName::Error);
        assert_eq!(shape.message, "handle table is full");
    }

    #[test]
    fn error_shape_keeps_the_original_message() {
        let error = BffiError::new(ErrorCode::InvalidUtf8, "byte 0xFF at position 2");
        let shape = error.to_js_shape();
        assert_eq!(shape.name, JsErrorName::TypeError);
        assert_eq!(shape.message, "byte 0xFF at position 2");
    }
}
