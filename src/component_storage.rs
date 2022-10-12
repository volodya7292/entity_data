use crate::{ArchetypeStorage, Component, EntityId};
use std::cell::UnsafeCell;
use std::marker::PhantomData;

pub struct ComponentStorageRef<'a, C> {
    pub(crate) archetype: &'a ArchetypeStorage,
    pub(crate) data: &'a UnsafeCell<Vec<u8>>,
    pub(crate) _ty: PhantomData<C>,
}

impl<'a, C: Component> ComponentStorageRef<'a, C> {
    /// Returns `true` if the archetype contains the specified entity.
    pub fn contains(&self, entity_id: EntityId) -> bool {
        self.archetype.contains(entity_id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    /// # Safety:
    /// To not cause any undefined behavior, the following conditions must be met:
    /// * Entity at `entity_id` must exist.
    /// * `&mut C` must always be unique.
    pub unsafe fn get_unsafe(&self, entity_id: EntityId) -> &'a mut C {
        let ptr = ((&*self.data.get()).as_ptr() as *mut C).offset(entity_id as isize);
        &mut *ptr
    }

    /// Returns a reference to the component `C` of the specified entity id.
    /// Safety: component at `entity_id` must exist.
    pub unsafe fn get_unchecked(&self, entity_id: EntityId) -> &'a C {
        // Safety: the method does not mutate `self`
        self.get_unsafe(entity_id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    /// Safety: component at `entity_id` must exist.
    pub unsafe fn get_mut_unchecked(&mut self, entity_id: EntityId) -> &'a mut C {
        self.get_unsafe(entity_id)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get(&self, entity_id: EntityId) -> Option<&'a C> {
        if !self.contains(entity_id) {
            return None;
        }
        unsafe { Some(self.get_unchecked(entity_id)) }
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut(&mut self, entity_id: EntityId) -> Option<&'a mut C> {
        if !self.contains(entity_id) {
            return None;
        }
        unsafe { Some(self.get_mut_unchecked(entity_id)) }
    }

    /// Returns an iterator over all components.
    pub fn iter(&self) -> impl Iterator<Item = &'a C> + '_ {
        self.archetype
            .occupied_slots
            .iter_zeros()
            .map(|id| unsafe { self.get_unchecked(id as EntityId) })
    }

    /// Returns an iterator that allows to modifying each component.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &'a mut C> + '_ {
        self.archetype
            .occupied_slots
            .iter_zeros()
            .map(|id| unsafe { self.get_mut_unchecked(id as EntityId) })
    }

    /// Returns an iterator over all entities present in this storage.
    pub fn entities(&self) -> impl Iterator + '_ {
        self.archetype
            .occupied_slots
            .iter_zeros()
            .map(|id| id as EntityId)
    }
}
