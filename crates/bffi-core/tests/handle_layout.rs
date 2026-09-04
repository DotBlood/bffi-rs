//! Integration tests for the handle bit layout defined in DESIGN §6.2.

use bffi_core::{Handle, MAX_GENERATION, MAX_INDEX, TypeTag};

#[test]
fn layout_matches_the_design_formula() {
    // u64 = (type_tag << 48) | (generation << 24) | index
    let handle = Handle::new(TypeTag(0x0201), 3, 42);
    let expected = (0x0201_u64) << 48 | (3_u64) << 24 | 42;
    assert_eq!(handle.as_u64(), expected);
}

#[test]
fn every_field_fills_exactly_its_own_bits() {
    let handle = Handle::new(TypeTag(0xFFFF), MAX_GENERATION, MAX_INDEX);
    assert_eq!(handle.as_u64(), u64::MAX);
    assert_eq!(handle.parts(), (TypeTag(0xFFFF), MAX_GENERATION, MAX_INDEX));
}

#[test]
fn fields_do_not_bleed_into_each_other() {
    let handle = Handle::new(TypeTag(0x8002), 0, 0);
    assert_eq!(handle.as_u64(), 0x8002_u64 << 48);

    let handle = Handle::new(TypeTag(0x0000), MAX_GENERATION, 0);
    assert_eq!(handle.as_u64(), 0xFF_FFFF_u64 << 24);

    let handle = Handle::new(TypeTag(0x0000), 0, MAX_INDEX);
    assert_eq!(handle.as_u64(), 0xFF_FFFF_u64);
}

#[test]
fn null_handle_crosses_the_boundary_as_zero() {
    assert_eq!(Handle::NULL.as_u64(), 0);
    assert_eq!(Handle::from(0_u64), Handle::NULL);
    assert!(Handle::NULL.is_null());
    assert_eq!(Handle::NULL.tag(), TypeTag::NULL);
}

#[test]
fn raw_roundtrip_preserves_the_value() {
    let handle = Handle::new(TypeTag(0x0104), 9, 77);
    assert_eq!(Handle::from_raw(handle.as_u64()), handle);
    assert_eq!(u64::from(handle), handle.as_u64());
}

#[test]
fn display_shows_all_three_fields() {
    let handle = Handle::new(TypeTag(0x0201), 3, 42);
    assert_eq!(handle.to_string(), "Handle(tag:0x0201, gen:3, idx:42)");
    assert_eq!(format!("{handle:?}"), "Handle(tag:0x0201, gen:3, idx:42)");
}
