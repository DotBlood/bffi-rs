//! Lock-free generational handle tables mapping opaque [`Handle`]s to
//! `Arc<T>` values.
//!
//! Rust-side objects live as `Arc<T>` inside a [`HandleTable`]; callers on
//! the other side of the C ABI only ever hold the opaque [`Handle`]
//! (DESIGN §6.2). The generation part of a handle makes staleness
//! unambiguous: freeing a slot bumps its generation, so a handle issued
//! before the free can never resolve to a later occupant of the same slot
//! (ABA protection).
//!
//! # Concurrency model
//!
//! The table is lock-free on every fast path (`get`, `contains`, `insert`,
//! `len`); `remove` is lock-free except for an amortized hazard-scan
//! critical section that runs only while readers hold the removed value
//! in flight.
//!
//! - **Cells** live in pinned two-level pages (4096 cells per page,
//!   installed once via CAS, addresses stable for the table lifetime) so
//!   readers can atomically load a `(generation, value)` pair without
//!   tearing. The generation and the value pointer must be validated as a
//!   pair, which is why they share a cell rather than sitting in separate
//!   arrays.
//! - **Readers** (`get`) publish the value pointer as a hazard
//!   ([`crate::hazard`]), re-validate the pair, then take an owned
//!   refcount - removers scan hazard slots before freeing, so a validated
//!   reader can never touch freed memory.
//! - **Freelist** is a Treiber stack on a single tagged word
//!   (`tag:u32 | index:u32`); the tag increments on every push and pop,
//!   which removes the classic ABA problem.
//!
//! Tables can be stored in a `static` or behind an `Arc` and shared across
//! threads; tables stored in the process-wide [`crate::catalog::Registry`]
//! are created and looked up by [`TypeTag`].
//!
//! # Unsafe policy
//!
//! This module contains audited `unsafe` blocks (refcount surgery, pinned
//! page dereferences). Public API remains 100% safe; see the crate
//! documentation for the invariant.

use crate::handle::{Handle, MAX_GENERATION, MAX_INDEX, TypeTag};
use crate::hazard;
use std::fmt;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Cells per page. Pages are allocated atomically and pinned, so a full
/// page is one contiguous 4096-cell slab whose address never changes.
const PAGE: usize = 4096;

/// Sentinel index inside the freelist word meaning "no more entries".
const FREE_END: u32 = u32::MAX;

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

/// One pinned slot: an atomically validated `(generation, value)` pair
/// plus the freelist link.
struct SlotCell<T> {
    generation: AtomicU32,
    value: AtomicPtr<T>,
    next_free: AtomicU32,
}

impl<T> SlotCell<T> {
    fn new() -> Self {
        Self {
            generation: AtomicU32::new(0),
            value: AtomicPtr::new(std::ptr::null_mut()),
            next_free: AtomicU32::new(FREE_END),
        }
    }
}

type Page<T> = [SlotCell<T>; PAGE];

/// Recovers from a poisoned lock; used by the registry's directory and
/// the retire-list mutex, which are off the lock-free fast paths.
pub(crate) fn recover<T>(result: std::sync::LockResult<T>) -> T {
    result.unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A lock-free, generational slot arena mapping [`Handle`]s to `Arc<T>`.
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
/// All operations take `&self` and are lock-free on the fast path; see the
/// module documentation. `T: Send + Sync` is required because values can
/// be reached from any thread that holds a handle.
pub struct HandleTable<T: Send + Sync + 'static> {
    tag: TypeTag,
    capacity: usize,
    pages: Box<[AtomicPtr<Page<T>>]>,
    free: AtomicU64,
    next_index: AtomicU32,
    live: AtomicUsize,
    /// Deferred table-owned references (as addresses; `usize` keeps the
    /// table `Send + Sync`). Entries are exclusively owned strong counts.
    retired: Mutex<Vec<usize>>,
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
    /// exceeds 2^24 slots. The page directory is allocated up front
    /// (8 bytes per page of 4096 slots); pages themselves are allocated
    /// lazily on first use.
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
        let pages = capacity.div_ceil(PAGE);
        Self {
            tag,
            capacity,
            pages: (0..pages)
                .map(|_| AtomicPtr::new(std::ptr::null_mut()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            free: AtomicU64::new(FREE_END as u64),
            next_index: AtomicU32::new(0),
            live: AtomicUsize::new(0),
            retired: Mutex::new(Vec::new()),
        }
    }

    /// Returns the tag stamped into all handles issued by this table.
    #[must_use]
    pub fn tag(&self) -> TypeTag {
        self.tag
    }

    /// Returns the number of live objects in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    /// Returns `true` if the table holds no live objects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if `handle` refers to a live object in this table.
    #[must_use]
    pub fn contains(&self, handle: Handle) -> bool {
        let (handle_tag, generation, index) = handle.parts();
        if handle_tag != self.tag {
            return false;
        }
        let Some(cell) = self.cell(index) else {
            return false;
        };
        let current = cell.generation.load(Ordering::SeqCst);
        current == generation && !cell.value.load(Ordering::SeqCst).is_null()
    }

    /// Stores `value` and returns a fresh handle for it.
    ///
    /// # Errors
    ///
    /// Returns [`TableError::Full`] when no slot is free.
    pub fn insert(&self, value: Arc<T>) -> Result<Handle, TableError> {
        let index = match self.pop_free() {
            Some(index) => index,
            None => {
                let index = self.next_index.fetch_add(1, Ordering::Relaxed);
                if index as usize >= self.capacity {
                    return Err(TableError::Full);
                }
                index
            }
        };
        let cell = self.cell_create(index);
        debug_assert!(
            cell.value.load(Ordering::Relaxed).is_null(),
            "a freelist or fresh slot must not hold a value"
        );
        // SAFETY: the table now owns this strong count; it is released
        // either by `remove` (via the hazard scan) or by `Drop`.
        let ptr = Arc::into_raw(value) as *mut T;
        cell.value.store(ptr, Ordering::Release);
        self.live.fetch_add(1, Ordering::Relaxed);
        Ok(Handle::new(
            self.tag,
            cell.generation.load(Ordering::Relaxed),
            index,
        ))
    }

    /// Clones the `Arc<T>` behind `handle`, if it is live and belongs to
    /// this table.
    #[must_use]
    pub fn get(&self, handle: Handle) -> Option<Arc<T>> {
        let (handle_tag, generation, index) = handle.parts();
        if handle_tag != self.tag {
            return None;
        }
        let cell = self.cell(index)?;
        loop {
            let g1 = cell.generation.load(Ordering::SeqCst);
            let p1 = cell.value.load(Ordering::SeqCst);
            if p1.is_null() || g1 != generation {
                return None;
            }
            // Publish, then re-validate: if the pair survived, the remover
            // that unlinks it will observe our hazard and defer the free.
            hazard::publish(p1.cast());
            let g2 = cell.generation.load(Ordering::SeqCst);
            let p2 = cell.value.load(Ordering::SeqCst);
            if g1 == g2 && p1 == p2 {
                // SAFETY: the hazard is published and the pair validated,
                // so no remover can free `p1` before it scans the hazard
                // slots; taking one more strong count is therefore sound.
                unsafe { Arc::increment_strong_count(p1) };
                hazard::clear();
                // SAFETY: we own exactly one strong count from the line
                // above.
                return Some(unsafe { Arc::from_raw(p1) });
            }
            hazard::clear();
        }
    }

    /// Removes the object behind `handle` and returns it, if it is live.
    ///
    /// The slot's generation is bumped, so the handle stays invalid even
    /// after the slot is reused. Slots whose generation reached
    /// [`MAX_GENERATION`] are retired instead of reused.
    ///
    /// The table's own reference is released under the hazard scan: if a
    /// reader is still validating the value, that reference is deferred to
    /// the retire list instead of being dropped.
    pub fn remove(&self, handle: Handle) -> Option<Arc<T>> {
        let (handle_tag, generation, index) = handle.parts();
        if handle_tag != self.tag {
            return None;
        }
        let cell = self.cell(index)?;
        loop {
            let g1 = cell.generation.load(Ordering::SeqCst);
            if g1 != generation {
                return None;
            }
            let p1 = cell.value.load(Ordering::SeqCst);
            if p1.is_null() {
                return None;
            }
            match cell.value.compare_exchange(
                p1,
                std::ptr::null_mut(),
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Unlink succeeded; this thread now owns the slot.
                    if let Some(next_gen) = next_generation(g1) {
                        cell.generation.store(next_gen, Ordering::Release);
                        self.push_free(cell, index);
                    }
                    // else: generation exhausted - the slot is retired and
                    // never reused; its generation intentionally stays at
                    // MAX so every stale handle keeps failing.
                    self.live.fetch_sub(1, Ordering::SeqCst);

                    // Hand one strong count to the caller.
                    //
                    // SAFETY: the table still holds its own reference, so
                    // `p1` is alive; both calls below only move/return the
                    // counts we own.
                    unsafe { Arc::increment_strong_count(p1) };
                    // SAFETY: we own exactly one strong count from the
                    // increment above.
                    let caller = unsafe { Arc::from_raw(p1) };

                    // Release the table's reference under the hazard scan:
                    // a reader still validating `p1` forces a deferral.
                    if hazard::is_protected(p1.cast()) {
                        self.retired
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .push(p1 as usize);
                    } else {
                        // SAFETY: no hazard points at `p1` and the caller
                        // holds a separate strong count, so dropping the
                        // table's count cannot free it.
                        drop(unsafe { Arc::from_raw(p1) });
                    }
                    self.drain_retired();
                    return Some(caller);
                }
                Err(_) => {
                    // Unlinked concurrently with the same handle; the loop
                    // re-checks the (already bumped) generation.
                }
            }
        }
    }

    /// Pops an index from the Treiber freelist, if any.
    fn pop_free(&self) -> Option<u32> {
        loop {
            let word = self.free.load(Ordering::SeqCst);
            let index = word as u32;
            if index == FREE_END {
                return None;
            }
            let tag = (word >> 32) as u32;
            let next = self.cell(index)?.next_free.load(Ordering::SeqCst);
            let updated = ((tag as u64 + 1) << 32) | next as u64;
            if self
                .free
                .compare_exchange(word, updated, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Some(index);
            }
        }
    }

    /// Pushes an index onto the Treiber freelist. `cell` is the slot for
    /// `index` (the CAS winner already holds it); its `next_free` field
    /// links to the previous stack top.
    fn push_free(&self, cell: &SlotCell<T>, index: u32) {
        loop {
            let word = self.free.load(Ordering::SeqCst);
            let tag = (word >> 32) as u32;
            cell.next_free.store(word as u32, Ordering::SeqCst);
            let updated = ((tag as u64 + 1) << 32) | index as u64;
            if self
                .free
                .compare_exchange(word, updated, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return;
            }
        }
    }

    /// Returns the cell for `index`, if its page has been allocated.
    fn cell(&self, index: u32) -> Option<&SlotCell<T>> {
        let page_idx = index as usize / PAGE;
        let page = self.pages.get(page_idx)?.load(Ordering::Acquire);
        if page.is_null() {
            return None;
        }
        // SAFETY: pages are allocated once, installed via CAS and freed
        // only in `Drop`, which excludes all other accesses.
        Some(&unsafe { &*page }[index as usize % PAGE])
    }

    /// Returns the cell for `index`, allocating its page if needed.
    /// `index` must be below the capacity.
    fn cell_create(&self, index: u32) -> &SlotCell<T> {
        let page_idx = index as usize / PAGE;
        let offset = index as usize % PAGE;
        let page_slot = &self.pages[page_idx];
        let existing = page_slot.load(Ordering::Acquire);
        if !existing.is_null() {
            // SAFETY: installed pages live until `Drop`.
            return &unsafe { &*existing }[offset];
        }
        // SAFETY: `array::from_fn` fully initialises the page; cells hold
        // no `T` values yet, so nothing is leaked on the losing CAS path.
        let page = Box::into_raw(Box::new(std::array::from_fn::<_, PAGE, _>(|_| {
            SlotCell::new()
        })));
        match page_slot.compare_exchange(
            std::ptr::null_mut(),
            page,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // SAFETY: we just installed this page; it lives until Drop.
                let installed = unsafe { &*page };
                &installed[offset]
            }
            Err(winner) => {
                // Another thread installed a page first; ours is empty.
                // SAFETY: we allocated `page` above and nobody installed
                // it, so this frees exactly our own allocation.
                drop(unsafe { Box::from_raw(page) });
                // SAFETY: the winning page lives until Drop.
                let installed = unsafe { &*winner };
                &installed[offset]
            }
        }
    }

    /// Drops the table-owned reference behind `ptr` unless a reader is
    /// still validating it; deferred pointers drain once the retire list
    /// grows past twice the hazard-slot count.
    fn drain_retired(&self) {
        let mut list = self.retired.lock().unwrap_or_else(|p| p.into_inner());
        if list.len() < 2 * hazard::count() {
            return;
        }
        self.scan_retired(&mut list);
    }

    /// Frees every retired entry whose pointer is no longer published as
    /// a hazard; protected entries stay for a later pass.
    fn scan_retired(&self, list: &mut Vec<usize>) {
        list.retain(|entry| {
            // SAFETY: entries are owned table references installed by
            // `remove`; this pass verifies no hazard points at them.
            let ptr = (*entry as *mut T) as *const u8;
            if hazard::is_protected(ptr) {
                true
            } else {
                // SAFETY: entries are owned table references installed by
                // `remove`; the hazard scan just cleared this one.
                drop(unsafe { Arc::from_raw(*entry as *mut T) });
                false
            }
        });
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

impl<T: Send + Sync + 'static> Drop for HandleTable<T> {
    fn drop(&mut self) {
        for page_slot in self.pages.iter() {
            let page = page_slot.load(Ordering::SeqCst);
            if page.is_null() {
                continue;
            }
            // SAFETY: at Drop time this table has exclusive access; pages
            // were allocated by `cell_create` and never freed elsewhere.
            let page_ref = unsafe { &*page };
            for cell in page_ref.iter() {
                let value = cell.value.swap(std::ptr::null_mut(), Ordering::SeqCst);
                if !value.is_null() {
                    // SAFETY: the table owned this strong count since the
                    // matching insert; no other reference can exist here.
                    drop(unsafe { Arc::from_raw(value) });
                }
            }
            // SAFETY: the page was allocated by `Box::into_raw`.
            drop(unsafe { Box::from_raw(page) });
        }
        // No reader can hold a hazard into a table that is being dropped:
        // readers borrow the table for the duration of their call.
        let list = self.retired.get_mut().unwrap_or_else(|p| p.into_inner());
        for entry in list.drain(..) {
            // SAFETY: retired entries are owned table references.
            drop(unsafe { Arc::from_raw(entry as *mut T) });
        }
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
        arena.remove(stale);

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

    #[test]
    fn exhausted_slots_are_retired_forever() {
        let arena = fresh::<u32>(TypeTag(0x8208));
        let handle = arena.insert(Arc::new(1_u32)).expect("insert");
        let index = handle.index();

        // Drive the slot's generation to MAX directly (same-crate access):
        // after the next removal the slot must be retired, never reused.
        let cell = arena.cell_create(index);
        cell.generation.store(MAX_GENERATION, Ordering::SeqCst);

        assert!(
            arena
                .remove(Handle::new(TypeTag(0x8208), MAX_GENERATION, index))
                .is_some(),
            "the value at MAX generation is still removable"
        );

        let next = arena
            .insert(Arc::new(2_u32))
            .expect("fresh index allocated");
        assert_ne!(
            next.index(),
            index,
            "a retired slot must never re-enter the freelist"
        );
        assert!(
            arena
                .get(Handle::new(TypeTag(0x8208), MAX_GENERATION, index))
                .is_none()
        );
    }

    #[test]
    fn values_are_freed_exactly_once_on_table_drop() {
        use std::sync::atomic::AtomicUsize;
        static LIVE: AtomicUsize = AtomicUsize::new(0);

        struct Tracked;
        impl Tracked {
            fn new() -> Self {
                LIVE.fetch_add(1, Ordering::Relaxed);
                Self
            }
        }
        impl Drop for Tracked {
            fn drop(&mut self) {
                LIVE.fetch_sub(1, Ordering::Relaxed);
            }
        }

        {
            let arena = fresh::<Tracked>(TypeTag(0x8209));
            let mut handles = Vec::new();
            for _ in 0..64 {
                handles.push(arena.insert(Arc::new(Tracked::new())).expect("insert"));
            }
            assert_eq!(LIVE.load(Ordering::Relaxed), 64);

            // Removing hands ownership to the caller; dropping the returned
            // Arcs exercises the table's reference release path.
            for handle in handles.drain(..32) {
                assert!(arena.remove(handle).is_some());
            }
            assert_eq!(LIVE.load(Ordering::Relaxed), 32);
        }
        assert_eq!(
            LIVE.load(Ordering::Relaxed),
            0,
            "dropping the table must free every remaining value exactly once"
        );
    }

    #[test]
    fn removed_value_is_deferred_while_hazard_is_published() {
        use std::sync::atomic::AtomicUsize;
        static LIVE: AtomicUsize = AtomicUsize::new(0);

        struct Tracked;
        impl Tracked {
            fn new() -> Self {
                LIVE.fetch_add(1, Ordering::Relaxed);
                Self
            }
        }
        impl Drop for Tracked {
            fn drop(&mut self) {
                LIVE.fetch_sub(1, Ordering::Relaxed);
            }
        }

        let arena = fresh::<Tracked>(TypeTag(0x820A));
        let handle = arena.insert(Arc::new(Tracked::new())).expect("insert");
        let ptr = {
            let value = arena.get(handle).expect("live");
            Arc::as_ptr(&value) as *const u8
        };

        // A reader publishing this exact pointer forces remove to defer
        // the table's reference into the retire list.
        hazard::publish(ptr);
        let removed = arena.remove(handle).expect("remove");
        assert_eq!(
            LIVE.load(Ordering::Relaxed),
            1,
            "the value must be deferred, not freed"
        );
        assert!(
            !arena.retired.lock().unwrap().is_empty(),
            "the table's reference must sit on the retire list"
        );

        hazard::clear();
        drop(removed);
        assert_eq!(
            LIVE.load(Ordering::Relaxed),
            1,
            "the deferred reference still keeps the value alive"
        );

        // Draining is threshold-based in production; here we invoke the
        // scan directly now that the hazard is cleared.
        let mut list = arena.retired.lock().unwrap();
        assert!(!list.is_empty(), "the deferred entry must be on the list");
        arena.scan_retired(&mut list);
        drop(list);
        assert_eq!(
            LIVE.load(Ordering::Relaxed),
            0,
            "retired values must be freed once hazards clear"
        );
        assert!(arena.is_empty());
    }
}
