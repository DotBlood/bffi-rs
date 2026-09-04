//! Integration tests for the buffer policy types.

// Tests assert invariants; the workspace restriction lints target
// production code.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_types::CopiedBuf;
use bffi_types::unsafe_zero_copy::buf_view;

#[test]
fn copied_buf_never_aliases_its_source() {
    let mut source = vec![1_u8, 2, 3];
    let copied = CopiedBuf::from_slice(&source);
    source[0] = 99;
    assert_eq!(copied.as_slice(), [1, 2, 3], "copy must be independent");

    let owned = CopiedBuf::from_vec(source);
    assert_eq!(owned.as_slice(), [99, 2, 3], "from_vec takes ownership");
}

#[test]
fn copied_buf_unwraps_into_owned_vec() {
    let copied = CopiedBuf::from_slice(b"payload");
    let vec = copied.into_vec();
    assert_eq!(vec, b"payload".to_vec());
}

#[test]
fn from_impls_cover_slice_vec_and_zero_copy() {
    let from_slice = CopiedBuf::from(&[7_u8, 8][..]);
    assert_eq!(from_slice.as_slice(), [7, 8]);

    let from_vec = CopiedBuf::from(vec![9_u8]);
    assert_eq!(from_vec.as_slice(), [9]);

    let borrowed = vec![5_u8, 6];
    let view = buf_view(&borrowed);
    let copied = CopiedBuf::from(&view);
    assert_eq!(copied.as_slice(), [5, 6], "explicit copy out of a view");
}

#[test]
fn copied_buf_behaves_like_a_slice_via_deref() {
    let copied = CopiedBuf::from_slice(&[10_u8, 20, 30]);
    assert_eq!(copied.len(), 3);
    assert!(!copied.is_empty());
    assert_eq!(copied[1], 20);
    assert!(copied.starts_with(&[10, 20]));

    assert!(CopiedBuf::default().is_empty());
}
