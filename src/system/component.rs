use crate::{ArchetypeStorage, Component, EntityId};
use std::cell::{Ref, RefMut};
use std::marker::PhantomData;

pub(crate) type CompMutability = bool;

#[derive(Clone)]
pub struct GenericComponentGlobalAccess<'a> {
    pub(crate) filtered_archetype_ids: Vec<usize>,
    pub(crate) all_archetypes: &'a [ArchetypeStorage],
    pub(crate) mutable: bool,
}

impl GenericComponentGlobalAccess<'_> {
    fn count_entities(&self) -> usize {
        self.filtered_archetype_ids
            .iter()
            .map(|v| self.all_archetypes[*v].entities.count())
            .sum::<usize>()
    }
}

pub struct GlobalComponentAccess<'a, C> {
    pub(crate) generic: Ref<'a, GenericComponentGlobalAccess<'a>>,
    pub(crate) _ty: PhantomData<C>,
}

impl<'a, C: Component> GlobalComponentAccess<'a, C> {
    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: &EntityId) -> bool {
        self.generic
            .all_archetypes
            .get(entity_id.archetype_id as usize)
            .and_then(|v| Some(v.contains(entity_id.id)))
            .unwrap_or(false)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get(&self, entity_id: &EntityId) -> Option<&C> {
        self.generic
            .all_archetypes
            .get(entity_id.archetype_id as usize)?
            .get(entity_id.id)
    }

    /// Returns total number of entities with the component `C`.
    pub fn count_entities(&self) -> usize {
        self.generic.count_entities()
    }
}

pub struct GlobalComponentAccessMut<'a, 'b, C> {
    pub(crate) generic: RefMut<'b, GenericComponentGlobalAccess<'a>>,
    pub(crate) _ty: PhantomData<C>,
}

impl<'a, 'b, C: Component> GlobalComponentAccessMut<'a, 'b, C> {
    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: &EntityId) -> bool {
        self.generic
            .all_archetypes
            .get(entity_id.archetype_id as usize)
            .and_then(|v| Some(v.contains(entity_id.id)))
            .unwrap_or(false)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get(&self, entity_id: &EntityId) -> Option<&C> {
        self.generic
            .all_archetypes
            .get(entity_id.archetype_id as usize)?
            .get(entity_id.id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut(&mut self, entity_id: &EntityId) -> Option<&mut C> {
        let comp = self
            .generic
            .all_archetypes
            .get(entity_id.archetype_id as usize)?
            .component::<C>()?;
        comp.contains(entity_id.id)
            .then(|| unsafe { comp.get_mut_unsafe(entity_id.id) })
    }

    /// Returns total number of entities with the component `C`.
    pub fn count_entities(&self) -> usize {
        self.generic.count_entities()
    }
}
