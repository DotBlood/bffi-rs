//! Lifecycle and use-after-free acceptance tests (kanboard 4.1 + 4.2).
//!
//! Tests of one file share the process-wide registry, so every test
//! reserves its own unique tag constants (P1-SPEC §5).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use bffi_core::{Handle, TypeTag};
use bffi_object::{ObjectError, ObjectWrap};

struct Session {
    id: u32,
}
struct Database {
    // Only used as a distinct type for tag/foreign-handle checks.
    #[allow(dead_code)]
    url: String,
}

// Unique tag per test, all inside the bffi-object range 0x0100-0x01FF.
const TAG_ROUNDTRIP: TypeTag = TypeTag(0x0110);
const TAG_STALE: TypeTag = TypeTag(0x0111);
const TAG_DOUBLE_RELEASE: TypeTag = TypeTag(0x0112);
const TAG_NULL: TypeTag = TypeTag(0x0113);
const TAG_FOREIGN_A: TypeTag = TypeTag(0x0114);
const TAG_FOREIGN_B: TypeTag = TypeTag(0x0115);
const TAG_FORGED: TypeTag = TypeTag(0x0116);
const TAG_DOUBLE_DECLARE: TypeTag = TypeTag(0x0117);
const TAG_ARC_OUTLIVES: TypeTag = TypeTag(0x0118);

#[test]
fn roundtrip_wrap_get_release() {
    let wrap = ObjectWrap::<Session>::new(TAG_ROUNDTRIP).expect("unique tag registers");
    let handle = wrap.wrap(Session { id: 7 }).expect("wrap stores the value");
    assert_eq!(handle.tag(), TAG_ROUNDTRIP);
    assert_eq!(wrap.get(handle).expect("live handle resolves").id, 7);
    assert_eq!(
        wrap.release(handle).expect("release returns the value").id,
        7
    );
}

#[test]
fn get_after_release_is_invalid_handle() {
    let wrap = ObjectWrap::<Session>::new(TAG_STALE).expect("unique tag registers");
    let handle = wrap.wrap(Session { id: 1 }).expect("wrap stores the value");
    wrap.release(handle).expect("the first release succeeds");
    assert_eq!(
        wrap.get(handle).err(),
        Some(ObjectError::InvalidHandle(handle))
    );
}

#[test]
fn double_release_is_invalid_handle() {
    let wrap = ObjectWrap::<Session>::new(TAG_DOUBLE_RELEASE).expect("unique tag registers");
    let handle = wrap.wrap(Session { id: 2 }).expect("wrap stores the value");
    assert!(wrap.release(handle).is_ok(), "the first release succeeds");
    assert_eq!(
        wrap.release(handle).err(),
        Some(ObjectError::InvalidHandle(handle))
    );
}

#[test]
fn null_handle_is_rejected() {
    let wrap = ObjectWrap::<Session>::new(TAG_NULL).expect("unique tag registers");
    assert_eq!(
        wrap.get(Handle::NULL).err(),
        Some(ObjectError::InvalidHandle(Handle::NULL))
    );
    assert_eq!(
        wrap.release(Handle::NULL).err(),
        Some(ObjectError::InvalidHandle(Handle::NULL))
    );
}

#[test]
fn foreign_type_handle_is_rejected() {
    let sessions = ObjectWrap::<Session>::new(TAG_FOREIGN_A).expect("unique tag registers");
    let databases = ObjectWrap::<Database>::new(TAG_FOREIGN_B).expect("unique tag registers");
    let handle = sessions
        .wrap(Session { id: 3 })
        .expect("wrap stores the value");
    assert_eq!(
        databases.get(handle).err(),
        Some(ObjectError::InvalidHandle(handle))
    );
    assert_eq!(
        databases.release(handle).err(),
        Some(ObjectError::InvalidHandle(handle))
    );
    assert_eq!(
        sessions
            .get(handle)
            .expect("the owning wrap still resolves")
            .id,
        3
    );
}

#[test]
fn forged_generation_is_rejected() {
    let wrap = ObjectWrap::<Session>::new(TAG_FORGED).expect("unique tag registers");
    let handle = wrap.wrap(Session { id: 4 }).expect("wrap stores the value");
    let forged = Handle::new(handle.tag(), handle.generation() + 1, handle.index());
    assert_eq!(
        wrap.get(forged).err(),
        Some(ObjectError::InvalidHandle(forged))
    );
    assert_eq!(wrap.get(handle).expect("the genuine handle resolves").id, 4);
}

#[test]
fn tag_outside_range_is_rejected() {
    for tag in [TypeTag(0x00FF), TypeTag(0x0200), TypeTag::NULL] {
        assert_eq!(
            ObjectWrap::<Session>::new(tag).err(),
            Some(ObjectError::TagOutOfRange(tag))
        );
    }
}

#[test]
fn second_declare_of_a_tag_is_tag_in_use() {
    ObjectWrap::<Session>::new(TAG_DOUBLE_DECLARE).expect("the first declaration succeeds");
    assert_eq!(
        ObjectWrap::<Session>::new(TAG_DOUBLE_DECLARE).err(),
        Some(ObjectError::TagInUse(TAG_DOUBLE_DECLARE))
    );
    assert_eq!(
        ObjectWrap::<Database>::new(TAG_DOUBLE_DECLARE).err(),
        Some(ObjectError::TagInUse(TAG_DOUBLE_DECLARE))
    );
}

#[test]
fn arc_outlives_release_and_reused_slot_stales_old_handle() {
    let wrap = ObjectWrap::<Session>::new(TAG_ARC_OUTLIVES).expect("unique tag registers");
    let first = wrap.wrap(Session { id: 7 }).expect("wrap stores the value");
    assert_eq!(
        wrap.release(first).expect("release returns the value").id,
        7
    );
    let second = wrap.wrap(Session { id: 6 }).expect("wrap reuses the slot");
    assert_eq!(second.index(), first.index(), "the slot is reused");
    assert_eq!(
        second.generation(),
        first.generation() + 1,
        "the reuse bumps the generation"
    );
    assert_eq!(
        wrap.get(first).err(),
        Some(ObjectError::InvalidHandle(first))
    );
    assert_eq!(wrap.get(second).expect("the new handle resolves").id, 6);
}
