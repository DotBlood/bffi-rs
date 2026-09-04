//! Opaque handles and type tags that identify objects across the C ABI.
//!
//! Design reference: `docs/DESIGN.md` §6.2 — ownership is expressed through
//! opaque 64-bit handles instead of raw references:
//!
//! ```text
//! u64 = (type_tag << 48) | (generation << 24) | index
//! ```
//!
//! See [`Handle`] for the bit layout and [`TypeTag`] for the tag ranges each
//! crate owns.

use std::fmt;

/// Number of bits occupied by the type tag (bits 48..64).
pub const TAG_BITS: u32 = 16;
/// Number of bits occupied by the generation (bits 24..48).
pub const GENERATION_BITS: u32 = 24;
/// Number of bits occupied by the slot index (bits 0..24).
pub const INDEX_BITS: u32 = 24;

/// Highest slot index a handle can address (`2^24 - 1`).
pub const MAX_INDEX: u32 = (1 << INDEX_BITS) - 1;
/// Highest generation value (`2^24 - 1`). Slots reaching it are retired
/// instead of reused, so generations never wrap around.
pub const MAX_GENERATION: u32 = (1 << GENERATION_BITS) - 1;

const _: () = assert!(TAG_BITS + GENERATION_BITS + INDEX_BITS == 64);

/// Identifies the kind of object stored behind a handle.
///
/// Tags make handles self-describing: given only a raw `u64` from the C ABI,
/// the runtime can tell which table must own the referenced object.
///
/// # Tag allocation
///
/// Tags are statically assigned ranges, one owner per range. New ranges are
/// reserved by documenting them here (explicit over magical):
///
/// | Range            | Owner                              |
/// |------------------|------------------------------------|
/// | `0x0000`         | null handle, never a valid object  |
/// | `0x0001`–`0x00FF`| reserved for `bffi-core`           |
/// | `0x0100`–`0x01FF`| `bffi-object`                      |
/// | `0x0200`–`0x02FF`| `bffi-callback`                    |
/// | `0x0300`–`0x03FF`| `bffi-class`                       |
/// | `0x0400`–`0x7FFF`| future `bffi-*` crates (reserve here) |
/// | `0x8000`–`0xFFFF`| user native modules                |
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct TypeTag(pub u16);

impl TypeTag {
    /// Tag of the null handle. Never a valid object tag.
    pub const NULL: Self = Self(0x0000);
    /// Reserved for core-owned error objects.
    pub const CORE_ERROR: Self = Self(0x0001);
}

impl fmt::Display for TypeTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#06x}", self.0)
    }
}

/// An opaque reference to a Rust-side object, safe to pass across the C ABI.
///
/// A handle is a 64-bit value with a fixed layout (DESIGN §6.2):
///
/// ```text
/// 63        48 47            24 23             0
/// ┌────────────┬────────────────┬────────────────┐
/// │  type tag  │   generation   │      index     │
/// │   16 bits  │     24 bits    │     24 bits    │
/// └────────────┴────────────────┴────────────────┘
/// ```
///
/// Handles are issued by [`crate::table::HandleTable`], which keeps the
/// object alive. Removing the object bumps the slot generation, so a stale
/// handle can never accidentally resolve to a new occupant of the same slot
/// (ABA protection).
///
/// The zero value is [`Handle::NULL`] and never resolves to an object.
///
/// [`Handle`] is `Copy`, `Send` and `Sync` — it is a plain integer with no
/// Rust-side validity requirements.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Handle(u64);

impl Handle {
    /// The null handle. Decodes to the null tag, generation 0, index 0 and
    /// is never issued by a table.
    pub const NULL: Self = Self(0);

    /// Assembles a handle from its parts.
    ///
    /// # Panics
    ///
    /// Panics if `generation` or `index` do not fit in 24 bits. Tables check
    /// ranges before constructing handles; this constructor is for
    /// round-tripping known-good values (and tests).
    #[must_use]
    pub const fn new(tag: TypeTag, generation: u32, index: u32) -> Self {
        assert!(
            generation <= MAX_GENERATION,
            "handle generation exceeds 24 bits"
        );
        assert!(index <= MAX_INDEX, "handle index exceeds 24 bits");
        Self(((tag.0 as u64) << 48) | ((generation as u64) << 24) | index as u64)
    }

    /// Reinterprets a raw 64-bit value received from the C ABI.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw 64-bit value to pass across the C ABI.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Splits the handle into `(tag, generation, index)`.
    #[must_use]
    pub const fn parts(self) -> (TypeTag, u32, u32) {
        let tag = TypeTag((self.0 >> 48) as u16);
        let generation = ((self.0 >> 24) as u32) & MAX_GENERATION;
        let index = (self.0 as u32) & MAX_INDEX;
        (tag, generation, index)
    }

    /// Returns the type tag of the referenced object.
    #[must_use]
    pub const fn tag(self) -> TypeTag {
        self.parts().0
    }

    /// Returns the generation part of the handle.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.parts().1
    }

    /// Returns the slot index part of the handle.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.parts().2
    }

    /// Returns `true` for [`Handle::NULL`].
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (tag, generation, index) = self.parts();
        write!(f, "Handle(tag:{tag}, gen:{generation}, idx:{index})")
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl From<u64> for Handle {
    fn from(raw: u64) -> Self {
        Self(raw)
    }
}

impl From<Handle> for u64 {
    fn from(handle: Handle) -> Self {
        handle.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_design_layout() {
        let handle = Handle::new(TypeTag(0x0201), 3, 42);
        assert_eq!(handle.as_u64(), (0x0201_u64) << 48 | (3_u64) << 24 | 42);
    }

    #[test]
    fn null_handle_is_zero() {
        assert_eq!(Handle::NULL.as_u64(), 0);
        assert!(Handle::NULL.is_null());
        assert_eq!(Handle::NULL.tag(), TypeTag::NULL);
        assert!(!Handle::new(TypeTag(0x8000), 0, 0).is_null());
    }

    #[test]
    fn roundtrips_extreme_values() {
        let handle = Handle::new(TypeTag(0xFFFF), MAX_GENERATION, MAX_INDEX);
        assert_eq!(handle.as_u64(), u64::MAX);
        assert_eq!(handle.parts(), (TypeTag(0xFFFF), MAX_GENERATION, MAX_INDEX));
    }

    #[test]
    fn accessors_match_parts() {
        let handle = Handle::new(TypeTag(0x0104), 9, 77);
        assert_eq!(handle.tag(), TypeTag(0x0104));
        assert_eq!(handle.generation(), 9);
        assert_eq!(handle.index(), 77);
    }

    #[test]
    fn raw_conversions_roundtrip() {
        let handle = Handle::new(TypeTag(0x8002), 1, 5);
        assert_eq!(Handle::from_raw(handle.as_u64()), handle);
        assert_eq!(u64::from(handle), handle.as_u64());
        assert_eq!(Handle::from(1234_u64).as_u64(), 1234);
    }

    #[test]
    fn display_and_debug_show_parts() {
        let handle = Handle::new(TypeTag(0x0201), 3, 42);
        let expected = "Handle(tag:0x0201, gen:3, idx:42)";
        assert_eq!(handle.to_string(), expected);
        assert_eq!(format!("{handle:?}"), expected);
    }

    #[test]
    #[should_panic(expected = "generation exceeds")]
    fn rejects_out_of_range_generation() {
        let _ = Handle::new(TypeTag(0x8000), MAX_GENERATION + 1, 0);
    }

    #[test]
    #[should_panic(expected = "index exceeds")]
    fn rejects_out_of_range_index() {
        let _ = Handle::new(TypeTag(0x8000), 0, MAX_INDEX + 1);
    }

    #[test]
    fn decoding_truncates_to_field_widths() {
        // A raw value with stray high bits in each field still decodes
        // within the 16/24/24 layout; the tag keeps all 16 bits.
        let raw: u64 = u64::MAX;
        let (tag, generation, index) = Handle::from_raw(raw).parts();
        assert_eq!(tag, TypeTag(0xFFFF));
        assert_eq!(generation, MAX_GENERATION);
        assert_eq!(index, MAX_INDEX);
    }
}
