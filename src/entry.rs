use crate::{ArchetypeStorage, Component, EntityId};

/// A immutable entry of an entity in an `ArchetypeStorage`.
/// Provides convenient and faster access to entity components.
pub struct Entry<'a> {
    pub(crate) arch: &'a ArchetypeStorage,
    pub(crate) entity: EntityId,
}

impl<'a> Entry<'a> {
    /// Returns underlying entity.
    pub fn entity(&self) -> &EntityId {
        &self.entity
    }

    /// Returns a reference to the component `C` of the specified entity.
    pub fn get<C: Component>(&self) -> Option<&C> {
        let comp = self.arch.component::<C>()?;
        Some(unsafe { comp.get_unchecked(self.entity.id) })
    }
}

/// A mutable entry of an entity in an `ArchetypeStorage`.
/// Provides convenient and faster access to entity components.
pub struct EntryMut<'a> {
    pub(crate) arch: &'a mut ArchetypeStorage,
    pub(crate) entity: EntityId,
}

impl EntryMut<'_> {
    /// Returns underlying entity.
    pub fn entity(&self) -> &EntityId {
        &self.entity
    }

    /// Returns a reference to the component `C` of the specified entity.
    pub fn get<C: Component>(&self) -> Option<&C> {
        let comp = self.arch.component::<C>()?;
        Some(unsafe { comp.get_unchecked(self.entity.id) })
    }

    /// Returns a mutable reference to the component `C` of the specified entity.
    pub fn get_mut<C: Component>(&mut self) -> Option<&mut C> {
        let mut comp = self.arch.component_mut::<C>()?;
        Some(unsafe { comp.get_unchecked_mut(self.entity.id) })
    }
}
