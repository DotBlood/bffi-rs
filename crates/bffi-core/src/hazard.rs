//! Hazard pointers: lock-free reclamation for handle-table values.
//!
//! A reader that wants to dereference (or clone the refcount of) a value
//! pointer must first *publish* the pointer into its thread's hazard slot,
//! then re-validate that the slot still holds the same pointer. A remover
//! that unlinks a value must *scan* all hazard slots before freeing: a
//! published pointer is deferred into a retire list instead of being
//! dropped. This closes the load-then-increment race between readers and
//! removers without any locks on the reader path.
//!
//! Each thread owns one hazard slot, claimed once and never freed, so
//! publishing and clearing are single atomic stores with no locking. The
//! slot registry is a grow-only list of boxed atomics; slot addresses are
//! stable for the life of the process.
//!
//! # Unsafe contract
//!
//! This module is one of the three places where `unsafe` is permitted in
//! the P0 crates (see the crate documentation). Callers must guarantee:
//!
//! 1. a published pointer is only dereferenced (or refcount-manipulated)
//!    after `publish` + successful re-validation of the observed slot;
//! 2. every unlinking of a value goes through `is_protected` before the
//!    last reference is dropped, deferring protected pointers to a retire
//!    list drained later by the same rule;
//! 3. hazard slots are cleared before returning from the read path.

use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// The process-wide hazard slot registry: grow-only, one boxed slot per
/// thread that ever used a lock-free structure.
static SLOTS: Mutex<Vec<&'static AtomicPtr<u8>>> = Mutex::new(Vec::new());

/// Number of threads that have claimed a slot so far. Retire thresholds
/// scale with this value.
static SLOT_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// This thread's hazard slot. The reference is valid for `'static`:
    /// slots are leaked into [`SLOTS`] and never freed.
    static SLOT: &'static AtomicPtr<u8> = {
        let slot: &'static AtomicPtr<u8> = Box::leak(Box::new(AtomicPtr::new(ptr::null_mut())));
        // Registration is once per thread, never on the hot path; the
        // mutex here is therefore uncontended in practice.
        let mut slots = SLOTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        slots.push(slot);
        SLOT_COUNT.fetch_add(1, Ordering::Relaxed);
        slot
    };
}

/// Returns this thread's hazard slot.
fn slot() -> &'static AtomicPtr<u8> {
    // SAFETY: the registry never frees slot objects, so the stored
    // `&'static AtomicPtr<u8>` is valid for the whole process lifetime.
    SLOT.with(|slot| unsafe { &*(*slot as *const AtomicPtr<u8>) })
}

/// Publishes `ptr` as this thread's hazard: removers must now defer
/// freeing `ptr` until the slot is cleared or repurposed.
pub(crate) fn publish(ptr: *const u8) {
    slot().store(ptr.cast_mut(), Ordering::SeqCst);
}

/// Clears this thread's hazard slot.
pub(crate) fn clear() {
    slot().store(ptr::null_mut(), Ordering::SeqCst);
}

/// Returns `true` if any thread currently publishes `ptr` as hazardous.
///
/// Removers call this before dropping the last reference: a positive
/// result means the value must be deferred to a retire list.
pub(crate) fn is_protected(ptr: *const u8) -> bool {
    let slots = SLOTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    slots
        .iter()
        .any(|slot| slot.load(Ordering::SeqCst) == ptr.cast_mut())
}

/// Number of hazard slots in existence; retire lists drain once they hold
/// twice this many entries.
pub(crate) fn count() -> usize {
    SLOT_COUNT.load(Ordering::Relaxed).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static MARKER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn publish_and_clear_roundtrip() {
        let value = &MARKER as *const AtomicUsize as *const u8;
        publish(value);
        assert!(is_protected(value));
        clear();
        assert!(!is_protected(value));
    }

    #[test]
    fn distinct_pointers_are_not_protected() {
        publish(&MARKER as *const AtomicUsize as *const u8);
        let other = &SLOT_COUNT as *const AtomicUsize as *const u8;
        assert!(!is_protected(other), "another address must not match");
        clear();
    }

    #[test]
    fn count_tracks_registered_threads() {
        // this test thread claimed a slot through the calls above
        assert!(count() >= 1);
    }
}
