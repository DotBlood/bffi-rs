//! Generational handle tables that map opaque [`Handle`]s to `Arc<T>`.
//!
//! Rust-side objects live as `Arc<T>` inside a [`HandleTable`]; callers on
//! the other side of the C ABI only ever hold the opaque [`Handle`]
//! (DESIGN §6.2). The generation part of a handle makes staleness
//! unambiguous: freeing a slot bumps its generation, so a handle issued
//! before the free can never resolve to a later occupant of the same slot
//! (ABA protection).
//!
//! All operations take `&self` (interior mutability via [`RwLock`]), so a
//! table can be stored in a `static` or behind an `Arc` and shared across
//! threads - required because callbacks may be invoked from non-JS threads.
//! Tables stored in the process-wide [`crate::catalog::Registry`] are
//! created and looked up by [`TypeTag`].
//!
//! If a thread panics while holding a table lock, the lock is *recovered*
//! rather than propagated: handle lookup still behaves correctly afterwards,
//! but the reported live count may have drifted by the number of panicked
//! operations. Panics inside table operations should not happen; the FFI
//! boundary ([`crate::boundary`]) is responsible for containing them.

use crate::handle::{Handle, MAX_GENERATION, MAX_INDEX, TypeTag};
use std::fmt;
use std::sync::{Arc, RwLock};

/// Error returned by [`HandleTable`] operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum TableError {
    /// The table has no free slot left: all indices up to the configured
    /// capacity are occupied by live (or retired) objects.
    Full,
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => f.write_str("handle table has no free slots"),
        }
    }
}

impl std::error::Error for TableError {}

/// Generation a slot gets when freed: `None` means retire it forever.
///
/// Slots whose generation would wrap past [`MAX_GENERATION`] are never
/// reused, so generations stay monotonically increasing for every live
/// index and stale handles can never collide with fresh ones.
#[must_use]
pub(crate) fn next_generation(current: u32) -> Option<u32> {
    if current < MAX_GENERATION {
        Some(current + 1)
    } else {
        None
    }
}

struct Slot<T> {
    generation: u32,
    value: Option<Arc<T>>,
}

struct Inner<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    live: usize,
}

/// Recovers from a poisoned lock; see the module docs for the trade-off.
pub(crate) fn recover<T>(result: std::sync::LockResult<T>) -> T {
    result.unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An owned, generational slot arena mapping [`Handle`]s to `Arc<T>`.
///
/// The table stamps its [`TypeTag`] into every handle it issues and rejects
/// handles carrying a different tag, so handles from different tables
/// (or different object kinds) never mix.
///
/// Dropping a table invalidates all handles it issued. If a replacement
/// table is created with the same tag, its generation-0 handles may be
/// numerically equal to stale handles from the old table - the previous
/// owner is responsible for ensuring no stale handles survive it.
///
/// # Concurrency
///
/// Safe to share (`&self` methods only). Reads ([`get`](Self::get),
/// [`contains`](Self::contains)) take a shared lock; mutations take an
/// exclusive lock.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use bffi_core::{HandleTable, TypeTag};
///
/// // user modules own tags in the 0x8000–0xFFFF range
/// let arena = HandleTable::<String>::new(TypeTag(0x8000));
/// let handle = arena.insert(Arc::new("hello".to_owned())).expect("room available");
///
/// let value = arena.get(handle).expect("live handle resolves");
/// assert_eq!(value.as_str(), "hello");
///
/// arena.remove(handle);
/// assert!(arena.get(handle).is_none(), "removed handles stay invalid");
/// ```
pub struct HandleTable<T: Send + Sync + 'static> {
    tag: TypeTag,
    capacity: usize,
    inner: RwLock<Inner<T>>,
}

impl<T: Send + Sync + 'static> HandleTable<T> {
    /// Creates a table with the maximum capacity of 2^24 slots.
    #[must_use]
    pub fn new(tag: TypeTag) -> Self {
        Self::with_capacity(tag, MAX_INDEX as usize + 1)
    }

    /// Creates a table holding at most `capacity` simultaneous objects.
    ///
    /// Useful for user modules that want to bound their own live-object
    /// count. The capacity can only shrink the address space; it never
    /// exceeds 2^24 slots.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` exceeds 2^24.
    #[must_use]
    pub fn with_capacity(tag: TypeTag, capacity: usize) -> Self {
        assert!(
            capacity <= MAX_INDEX as usize + 1,
            "capacity cannot exceed 2^24 slots"
        );
        Self {
            tag,
            capacity,
            inner: RwLock::new(Inner {
                slots: Vec::new(),
                free: Vec::new(),
                live: 0,
            }),
        }
    }

    /// Returns the tag stamped into all handles issued by this table.
    #[must_use]
    pub fn tag(&self) -> TypeTag {
        self.tag
    }

    /// Returns the number of live objects in the table.
    ///
    /// May drift after a panic inside a table operation; see module docs.
    #[must_use]
    pub fn len(&self) -> usize {
        recover(self.inner.read()).live
    }

    /// Returns `true` if the table holds no live objects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if `handle` refers to a live object in this table.
    #[must_use]
    pub fn contains(&self, handle: Handle) -> bool {
        let inner = recover(self.inner.read());
        Self::locate(&inner, handle, self.tag).is_some()
    }

    /// Stores `value` and returns a fresh handle for it.
    ///
    /// # Errors
    ///
    /// Returns [`TableError::Full`] when no slot is free.
    pub fn insert(&self, value: Arc<T>) -> Result<Handle, TableError> {
        let mut inner = recover(self.inner.write());
        let index = match inner.free.pop() {
            Some(index) => index,
            None => {
                let next = inner.slots.len();
                if next >= self.capacity {
                    return Err(TableError::Full);
                }
                inner.slots.push(Slot {
                    generation: 0,
                    value: None,
                });
                next as u32
            }
        };
        let slot = &mut inner.slots[index as usize];
        debug_assert!(slot.value.is_none(), "free-list slots must not hold values");
        let handle = Handle::new(self.tag, slot.generation, index);
        slot.value = Some(value);
        inner.live += 1;
        Ok(handle)
    }

    /// Clones the `Arc<T>` behind `handle`, if it is live and belongs to
    /// this table.
    #[must_use]
    pub fn get(&self, handle: Handle) -> Option<Arc<T>> {
        let inner = recover(self.inner.read());
        let index = Self::locate(&inner, handle, self.tag)?;
        inner.slots[index as usize].value.clone()
    }

    /// Removes the object behind `handle` and returns it, if it is live.
    ///
    /// The slot's generation is bumped, so the handle stays invalid even
    /// after the slot is reused. Slots whose generation reached
    /// [`MAX_GENERATION`] are retired instead of reused.
    #[must_use]
    pub fn remove(&self, handle: Handle) -> Option<Arc<T>> {
        let mut inner = recover(self.inner.write());
        let index = Self::locate(&inner, handle, self.tag)?;
        let (value, freed_generation) = {
            let slot = &mut inner.slots[index as usize];
            let value = slot.value.take()?;
            (value, next_generation(slot.generation))
        };
        inner.live -= 1;
        if let Some(generation) = freed_generation {
            inner.slots[index as usize].generation = generation;
            inner.free.push(index);
        }
        Some(value)
    }

    /// Resolves a handle to a live slot index, checking tag, generation
    /// and occupancy.
    fn locate(inner: &Inner<T>, handle: Handle, tag: TypeTag) -> Option<u32> {
        let (handle_tag, generation, index) = handle.parts();
        if handle_tag != tag {
            return None;
        }
        let slot = inner.slots.get(index as usize)?;
        if slot.generation != generation || slot.value.is_none() {
            return None;
        }
        Some(index)
    }
}

impl<T: Send + Sync + 'static> fmt::Debug for HandleTable<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleTable")
            .field("tag", &self.tag)
            .field("live", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a table for a test tag.
    fn fresh<T: Send + Sync + 'static>(tag: TypeTag) -> HandleTable<T> {
        HandleTable::new(tag)
    }

    #[test]
    fn next_generation_increments_until_max_then_retires() {
        assert_eq!(next_generation(0), Some(1));
        assert_eq!(next_generation(1), Some(2));
        assert_eq!(next_generation(MAX_GENERATION - 1), Some(MAX_GENERATION));
        assert_eq!(next_generation(MAX_GENERATION), None);
    }

    #[test]
    fn insert_get_remove_roundtrip() {
        let arena = fresh::<u64>(TypeTag(0x8200));
        assert!(arena.is_empty());

        let handle = arena.insert(Arc::new(11_u64)).expect("arena has room");
        assert_eq!(arena.len(), 1);
        assert!(arena.contains(handle));

        let value = arena.get(handle).expect("live handle resolves");
        assert_eq!(*value, 11);

        let removed = arena.remove(handle).expect("live handle removes");
        assert_eq!(*removed, 11);
        assert!(arena.is_empty());
        assert!(!arena.contains(handle));
    }

    #[test]
    fn double_remove_is_rejected() {
        let arena = fresh::<u8>(TypeTag(0x8201));
        let handle = arena.insert(Arc::new(1_u8)).expect("arena has room");
        assert!(arena.remove(handle).is_some());
        assert!(arena.remove(handle).is_none());
    }

    #[test]
    fn stale_handles_are_rejected_after_slot_reuse() {
        let arena = fresh::<u32>(TypeTag(0x8202));

        let stale = arena.insert(Arc::new(1_u32)).expect("arena has room");
        assert!(arena.remove(stale).is_some(), "live handle must remove");

        let fresh_handle = arena.insert(Arc::new(2_u32)).expect("arena has room");
        assert_eq!(
            fresh_handle.index(),
            stale.index(),
            "freed slot should be reused"
        );
        assert_eq!(fresh_handle.generation(), stale.generation() + 1);
        assert_ne!(fresh_handle, stale);

        assert!(
            arena.get(stale).is_none(),
            "stale handle must not resolve after slot reuse"
        );
        assert!(arena.remove(stale).is_none());
        assert_eq!(*arena.get(fresh_handle).expect("fresh handle is live"), 2);
    }

    #[test]
    fn foreign_tag_handles_are_rejected() {
        let ours = fresh::<u32>(TypeTag(0x8203));
        let theirs = fresh::<u32>(TypeTag(0x8204));

        let foreign = theirs.insert(Arc::new(1_u32)).expect("arena has room");
        assert!(ours.get(foreign).is_none());
        assert!(!ours.contains(foreign));
        assert!(ours.remove(foreign).is_none());
        assert_eq!(theirs.len(), 1, "foreign remove must not steal the value");
    }

    #[test]
    fn capacity_limit_produces_table_full() {
        let arena = HandleTable::<u16>::with_capacity(TypeTag(0x8205), 2);

        let first = arena.insert(Arc::new(1_u16)).expect("slot 0");
        let _second = arena.insert(Arc::new(2_u16)).expect("slot 1");
        assert_eq!(
            arena.insert(Arc::new(3_u16)),
            Err(TableError::Full),
            "third insert must exhaust the capacity"
        );

        assert!(arena.remove(first).is_some(), "live handle must remove");
        let third = arena
            .insert(Arc::new(3_u16))
            .expect("freed slot is reusable");
        assert_eq!(third.index(), first.index());
    }

    #[test]
    fn null_handle_is_never_resolved() {
        let arena = fresh::<u32>(TypeTag(0x8206));
        assert!(arena.get(Handle::NULL).is_none());
        assert!(!arena.contains(Handle::NULL));
        assert!(arena.remove(Handle::NULL).is_none());
    }

    #[test]
    fn arena_shows_in_debug_output() {
        let arena = fresh::<u32>(TypeTag(0x8207));
        let _handle = arena.insert(Arc::new(5_u32)).expect("arena has room");
        let debug = format!("{arena:?}");
        assert!(debug.contains("HandleTable"), "{debug}");
        assert!(debug.contains("live: 1"), "{debug}");
    }
}
