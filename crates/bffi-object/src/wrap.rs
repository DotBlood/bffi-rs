//! [`ObjectWrap`] - typed object ownership over the global registry.

use crate::error::ObjectError;
use bffi_core::{Handle, Registry, RegistryError, TypeTag};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// Inclusive lower bound of the tag range owned by `bffi-object`.
pub const TAG_MIN: u16 = 0x0100;
/// Inclusive upper bound of the tag range owned by `bffi-object`.
pub const TAG_MAX: u16 = 0x01FF;

/// Returns `true` if `tag` belongs to the `bffi-object` range
/// `TAG_MIN..=TAG_MAX`.
#[must_use]
pub const fn tag_in_range(tag: TypeTag) -> bool {
    tag.0 >= TAG_MIN && tag.0 <= TAG_MAX
}

/// Typed view of the global [`Registry`] for one object type `T`.
///
/// One tag = one `T` per process: [`ObjectWrap::new`] declares the tag
/// and fails with [`ObjectError::TagInUse`] on a second claim. The wrap
/// itself is just `(tag, marker)` - `Copy`, `Send + Sync`; keep one per
/// process in a `OnceLock` static.
///
/// Rust owns `T` through the `Arc` stored in the registry; the JS side
/// only ever sees the opaque `u64` handle.
pub struct ObjectWrap<T> {
    tag: TypeTag,
    _marker: PhantomData<fn(&T)>,
}

// Manual trait impls instead of `#[derive]`: the wrap is just
// `(tag, marker)`, so none of these traits depend on `T` - a derive
// would wrongly require `T: Copy` / `T: Debug` / `T: PartialEq`. With
// `PhantomData<fn(&T)>` the wrap is also `Send + Sync` for every `T`.
impl<T> Clone for ObjectWrap<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ObjectWrap<T> {}

impl<T> PartialEq for ObjectWrap<T> {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag
    }
}

impl<T> Eq for ObjectWrap<T> {}

impl<T> fmt::Debug for ObjectWrap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectWrap")
            .field("tag", &self.tag)
            .finish()
    }
}

impl<T: Send + Sync + 'static> ObjectWrap<T> {
    /// Checks the tag range and declares the tag for `T` in the global
    /// registry.
    ///
    /// # Errors
    ///
    /// [`ObjectError::TagOutOfRange`] if the tag is outside
    /// `0x0100-0x01FF`; [`ObjectError::TagInUse`] if the tag is already
    /// declared.
    pub fn new(tag: TypeTag) -> Result<Self, ObjectError> {
        if !tag_in_range(tag) {
            return Err(ObjectError::TagOutOfRange(tag));
        }
        Registry::global()
            .declare::<T>(tag)
            .map_err(|_| ObjectError::TagInUse(tag))?;
        Ok(Self {
            tag,
            _marker: PhantomData,
        })
    }

    /// The tag this wrap owns.
    #[must_use]
    pub const fn tag(&self) -> TypeTag {
        self.tag
    }

    /// Takes ownership of `value` and returns a fresh handle for it.
    ///
    /// # Errors
    ///
    /// [`ObjectError::TableFull`] when the tag's table has no free slot.
    pub fn wrap(&self, value: T) -> Result<Handle, ObjectError> {
        Registry::global()
            .insert(self.tag, Arc::new(value))
            .map_err(|error| match error {
                // `new` declared the tag for `T` and the registry has no
                // undeclare, so `NotRegistered` is unreachable; both
                // remaining conditions collapse to `TableFull` (spec:
                // wrap -> TableFull). `RegistryError` is
                // `#[non_exhaustive]`, so future variants are grouped
                // defensively into the wildcard.
                RegistryError::TagAlreadyRegistered(_) => ObjectError::TagInUse(self.tag),
                _ => ObjectError::TableFull(self.tag),
            })
    }

    /// Clones the `Arc<T>` behind `handle`.
    ///
    /// # Errors
    ///
    /// [`ObjectError::InvalidHandle`] - the handle is null, stale
    /// (released), or belongs to another type.
    pub fn get(&self, handle: Handle) -> Result<Arc<T>, ObjectError> {
        // Barrier 1 (kanboard 4.2): the null handle never resolves.
        if handle.is_null() {
            return Err(ObjectError::InvalidHandle(handle));
        }
        // Barriers 2+3: the registry matches the tag (type spoofing),
        // the table matches the generation (a stale handle stays dead
        // across slot reuse).
        Registry::global()
            .get_typed::<T>(handle)
            .ok_or(ObjectError::InvalidHandle(handle))
    }

    /// Releases the slot behind `handle` and returns the last table
    /// `Arc<T>`. The handle goes stale; existing `Arc` holders keep the
    /// value alive (arc-lenient ownership, P1-SPEC §5).
    ///
    /// # Errors
    ///
    /// [`ObjectError::InvalidHandle`] - the handle is null, stale, or
    /// belongs to another type.
    pub fn release(&self, handle: Handle) -> Result<Arc<T>, ObjectError> {
        if handle.is_null() {
            return Err(ObjectError::InvalidHandle(handle));
        }
        Registry::global()
            .remove_typed::<T>(handle)
            .ok_or(ObjectError::InvalidHandle(handle))
    }
}

#[cfg(test)]
mod tests {
    use crate::error::ObjectError;
    use crate::wrap::ObjectWrap;
    use bffi_core::{Handle, TypeTag};

    struct Counter {
        n: u32,
    }

    struct Log;

    #[test]
    fn tag_outside_object_range_is_rejected() {
        assert_eq!(
            ObjectWrap::<Counter>::new(TypeTag(0x00FF)).err(),
            Some(ObjectError::TagOutOfRange(TypeTag(0x00FF)))
        );
        assert_eq!(
            ObjectWrap::<Counter>::new(TypeTag(0x0200)).err(),
            Some(ObjectError::TagOutOfRange(TypeTag(0x0200)))
        );
        assert_eq!(
            ObjectWrap::<Counter>::new(TypeTag::NULL).err(),
            Some(ObjectError::TagOutOfRange(TypeTag::NULL))
        );
    }

    #[test]
    fn second_new_with_same_tag_is_tag_in_use() {
        const TAG: TypeTag = TypeTag(0x0130);
        ObjectWrap::<Counter>::new(TAG).expect("first declaration of a unique tag");
        assert_eq!(
            ObjectWrap::<Counter>::new(TAG).err(),
            Some(ObjectError::TagInUse(TAG))
        );
        assert_eq!(
            ObjectWrap::<Log>::new(TAG).err(),
            Some(ObjectError::TagInUse(TAG))
        );
    }

    #[test]
    fn roundtrip_wrap_get_release() {
        const TAG: TypeTag = TypeTag(0x0131);
        let wrap = ObjectWrap::<Counter>::new(TAG).expect("unique tag registers");
        let handle = wrap.wrap(Counter { n: 7 }).expect("wrap stores the value");
        assert_eq!(handle.tag(), TAG);
        assert_eq!(wrap.get(handle).expect("live handle resolves").n, 7);
        assert_eq!(
            wrap.release(handle).expect("release returns the value").n,
            7
        );
    }

    #[test]
    fn get_and_release_after_release_are_invalid_handle() {
        const TAG: TypeTag = TypeTag(0x0132);
        let wrap = ObjectWrap::<Counter>::new(TAG).expect("unique tag registers");
        let handle = wrap.wrap(Counter { n: 1 }).expect("wrap stores the value");
        assert!(wrap.release(handle).is_ok(), "the first release succeeds");
        assert_eq!(
            wrap.get(handle).err(),
            Some(ObjectError::InvalidHandle(handle))
        );
        assert_eq!(
            wrap.release(handle).err(),
            Some(ObjectError::InvalidHandle(handle))
        );
    }

    #[test]
    fn null_handle_is_rejected() {
        const TAG: TypeTag = TypeTag(0x0133);
        let wrap = ObjectWrap::<Counter>::new(TAG).expect("unique tag registers");
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
    fn wrap_traits() {
        const COUNTER_TAG: TypeTag = TypeTag(0x0134);
        const LOG_TAG: TypeTag = TypeTag(0x0135);

        fn assert_copy_clone_send_sync_debug<T: Copy + Clone + Send + Sync + std::fmt::Debug>() {}
        assert_copy_clone_send_sync_debug::<ObjectWrap<Counter>>();
        assert_copy_clone_send_sync_debug::<ObjectWrap<Log>>();

        let counter = ObjectWrap::<Counter>::new(COUNTER_TAG).expect("unique tag registers");
        let log = ObjectWrap::<Log>::new(LOG_TAG).expect("unique tag registers");
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        assert_send_sync(&counter);
        assert_send_sync(&log);
    }
}
