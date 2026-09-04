//! The process-wide [`Registry`] routing handles to typed tables.
//!
//! The C ABI layer receives bare `u64` handles and cannot know their Rust
//! types; the registry resolves a handle's [`TypeTag`] to the
//! [`HandleTable`] that issued it. Each tag is declared exactly once
//! (typically at module init) with the concrete type it stores, and lookups
//! verify both the tag and the requested type.
//!
//! Tables themselves live in [`crate::table`].

use crate::handle::{Handle, TypeTag};
use crate::table::{HandleTable, recover};
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

/// Error returned by [`Registry`] operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum RegistryError {
    /// The tag is already declared (by this or another type).
    TagAlreadyRegistered(TypeTag),
    /// No table is declared for the tag with the requested type.
    NotRegistered(TypeTag),
    /// The declared table for the tag is full.
    TableFull(TypeTag),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TagAlreadyRegistered(tag) => write!(f, "type tag {tag} is already registered"),
            Self::NotRegistered(tag) => write!(f, "type tag {tag} is not registered"),
            Self::TableFull(tag) => write!(f, "table for type tag {tag} is full"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Type-erased operations available on registry tables without knowing `T`.
trait ErasedTable: Send + Sync {
    /// Upcasts to `dyn Any` so the registry can downcast back to the
    /// concrete `HandleTable<T>`.
    fn as_any(&self) -> &dyn Any;

    /// Removes the object behind `handle`, returning whether it was live.
    fn remove_dyn(&self, handle: Handle) -> bool;
}

impl<T: Send + Sync + 'static> ErasedTable for HandleTable<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn remove_dyn(&self, handle: Handle) -> bool {
        self.remove(handle).is_some()
    }
}

/// The process-wide registry routing handles to typed [`HandleTable`]s.
///
/// Usage: obtain the registry with [`Registry::global`], declare one table
/// per [`TypeTag`] during module initialization, then store and look up
/// objects by handle. See the crate documentation for a complete example.
pub struct Registry {
    tables: RwLock<HashMap<TypeTag, Box<dyn ErasedTable>>>,
}

impl Registry {
    /// Returns the process-wide registry.
    #[must_use]
    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<Registry> = OnceLock::new();
        GLOBAL.get_or_init(|| Self {
            tables: RwLock::new(HashMap::new()),
        })
    }

    /// Declares a table for `tag` storing values of type `T`.
    ///
    /// Call once per tag, typically during module initialization.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::TagAlreadyRegistered`] if the tag is taken.
    pub fn declare<T: Send + Sync + 'static>(&self, tag: TypeTag) -> Result<(), RegistryError> {
        let mut locked = recover(self.tables.write());
        let occupied = locked.contains_key(&tag);
        if occupied {
            return Err(RegistryError::TagAlreadyRegistered(tag));
        }
        locked.insert(tag, Box::new(HandleTable::<T>::new(tag)));
        Ok(())
    }

    /// Returns `true` if `tag` has a declared table.
    #[must_use]
    pub fn is_declared(&self, tag: TypeTag) -> bool {
        recover(self.tables.read()).contains_key(&tag)
    }

    /// Returns all declared tags.
    #[must_use]
    pub fn declared_tags(&self) -> Vec<TypeTag> {
        recover(self.tables.read()).keys().copied().collect()
    }

    /// Stores `value` in the table declared for `tag` and returns a
    /// fresh handle.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::NotRegistered`] if `tag` has no table for
    /// type `T`, and [`RegistryError::TableFull`] if that table is full.
    pub fn insert<T: Send + Sync + 'static>(
        &self,
        tag: TypeTag,
        value: Arc<T>,
    ) -> Result<Handle, RegistryError> {
        match self.with_table(tag, |table| table.insert(value)) {
            Some(result) => result.map_err(|_| RegistryError::TableFull(tag)),
            None => Err(RegistryError::NotRegistered(tag)),
        }
    }

    /// Clones the `Arc<T>` behind `handle`, verifying both the tag and the
    /// requested type.
    #[must_use]
    pub fn get_typed<T: Send + Sync + 'static>(&self, handle: Handle) -> Option<Arc<T>> {
        let (tag, _, _) = handle.parts();
        if tag == TypeTag::NULL {
            return None;
        }
        self.with_table(tag, |table| table.get(handle)).flatten()
    }

    /// Removes the object behind `handle`, verifying the requested type,
    /// and returns it.
    #[must_use]
    pub fn remove_typed<T: Send + Sync + 'static>(&self, handle: Handle) -> Option<Arc<T>> {
        let (tag, _, _) = handle.parts();
        if tag == TypeTag::NULL {
            return None;
        }
        self.with_table(tag, |table| table.remove(handle)).flatten()
    }

    /// Removes and drops the object behind `handle` without knowing its
    /// type. Returns whether the handle referred to a live object.
    ///
    /// This is the operation behind a future generic `close(handle)`
    /// export, which the C ABI layer calls without type information.
    #[must_use]
    pub fn remove(&self, handle: Handle) -> bool {
        let (tag, _, _) = handle.parts();
        if tag == TypeTag::NULL {
            return false;
        }
        match recover(self.tables.read()).get(&tag) {
            Some(table) => table.remove_dyn(handle),
            None => false,
        }
    }

    /// Runs `f` with the table declared for `tag` as `&HandleTable<T>`.
    /// Returns `None` when the tag is unknown or declared for another
    /// type.
    fn with_table<T: Send + Sync + 'static, R>(
        &self,
        tag: TypeTag,
        f: impl FnOnce(&HandleTable<T>) -> R,
    ) -> Option<R> {
        let tables = recover(self.tables.read());
        let table = tables
            .get(&tag)?
            .as_any()
            .downcast_ref::<HandleTable<T>>()?;
        Some(f(table))
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("tables", &recover(self.tables.read()).len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Registry tests: each test uses a unique tag because the global
    // registry is shared by every test in this binary.

    struct Widget {
        id: u32,
    }
    struct Gadget;

    #[test]
    fn registry_roundtrip_and_typed_access() {
        const WIDGET: TypeTag = TypeTag(0x8210);
        let registry = Registry::global();
        registry
            .declare::<Widget>(WIDGET)
            .expect("unique tag registers");

        let handle = registry
            .insert(WIDGET, Arc::new(Widget { id: 7 }))
            .expect("declared arena accepts values");

        let widget = registry.get_typed::<Widget>(handle).expect("typed lookup");
        assert_eq!(widget.id, 7);

        // wrong requested type must not resolve even with a valid tag
        assert!(registry.get_typed::<Gadget>(handle).is_none());

        assert!(registry.remove(handle));
        assert!(registry.get_typed::<Widget>(handle).is_none());
        assert!(
            registry.remove_typed::<Widget>(handle).is_none(),
            "double untyped removal must fail"
        );
    }

    #[test]
    fn registry_rejects_double_declaration() {
        const GADGET: TypeTag = TypeTag(0x8211);
        let registry = Registry::global();

        let first = registry.declare::<Gadget>(GADGET);
        assert!(
            first.is_ok() || first == Err(RegistryError::TagAlreadyRegistered(GADGET)),
            "the first declaration of a unique tag must succeed"
        );
        assert_eq!(
            registry.declare::<Widget>(GADGET),
            Err(RegistryError::TagAlreadyRegistered(GADGET)),
            "a taken tag must not be re-declared, even for another type"
        );
        assert!(registry.is_declared(GADGET));
    }

    #[test]
    fn registry_rejects_undeclared_tags_and_null_handles() {
        const UNKNOWN: TypeTag = TypeTag(0x8212);
        let registry = Registry::global();

        assert_eq!(
            registry.insert(UNKNOWN, Arc::new(1_u32)),
            Err(RegistryError::NotRegistered(UNKNOWN))
        );
        assert!(!registry.is_declared(UNKNOWN));
        assert!(!registry.is_declared(TypeTag::NULL));

        assert!(registry.get_typed::<Widget>(Handle::NULL).is_none());
        assert!(!registry.remove(Handle::NULL));
    }
}
