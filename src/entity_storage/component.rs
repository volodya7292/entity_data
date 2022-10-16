use crate::archetype::component;
use crate::archetype::component::ComponentStorageRef;
use crate::entity_storage::AllEntities;
use crate::{ArchetypeStorage, Component, EntityId};
use std::borrow::{Borrow, BorrowMut};
use std::cell::{Ref, RefMut};
use std::marker::PhantomData;
use std::slice;

pub struct ComponentGlobalIter<'a, C> {
    filtered_archetype_ids: slice::Iter<'a, usize>,
    all_archetypes: &'a [ArchetypeStorage],
    curr_iter: component::Iter<'a, C, ComponentStorageRef<'a, C>>,
}

impl<'a, C: Component> Iterator for ComponentGlobalIter<'a, C> {
    type Item = &'a C;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.curr_iter.next();

            if res.is_some() {
                return res;
            } else {
                let next_arch_id = *self.filtered_archetype_ids.next()?;
                self.curr_iter = self.all_archetypes[next_arch_id].component::<C>()?.iter();
            }
        }
    }
}

pub struct ComponentGlobalIterMut<'a, C> {
    filtered_archetype_ids: slice::Iter<'a, usize>,
    all_archetypes: &'a [ArchetypeStorage],
    curr_iter: component::IterMut<'a, C, ComponentStorageRef<'a, C>>,
}

impl<'a, C: Component> Iterator for ComponentGlobalIterMut<'a, C> {
    type Item = &'a mut C;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.curr_iter.next();

            if res.is_some() {
                return res;
            } else {
                let next_arch_id = *self.filtered_archetype_ids.next()?;
                let next_comp = self.all_archetypes[next_arch_id].component::<C>()?;
                self.curr_iter = unsafe { next_comp.into_iter_mut_unsafe() };
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct GenericComponentGlobalAccess<'a> {
    pub(crate) filtered_archetype_ids: &'a [usize],
    pub(crate) all_archetypes: &'a [ArchetypeStorage],
    pub(crate) all_entities: AllEntities<'a>,
    pub(crate) mutable: bool,
}

pub trait AsGenericGlobalComponent<'a> {
    fn get(&self) -> &GenericComponentGlobalAccess<'a>;
}

impl<'a> AsGenericGlobalComponent<'a> for GenericComponentGlobalAccess<'a> {
    fn get(&self) -> &GenericComponentGlobalAccess<'a> {
        self
    }
}

impl<'a> AsGenericGlobalComponent<'a> for Ref<'_, GenericComponentGlobalAccess<'a>> {
    fn get(&self) -> &GenericComponentGlobalAccess<'a> {
        self
    }
}

impl<'a> AsGenericGlobalComponent<'a> for RefMut<'_, GenericComponentGlobalAccess<'a>> {
    fn get(&self) -> &GenericComponentGlobalAccess<'a> {
        self
    }
}

pub struct ComponentGlobalAccess<C, G, M> {
    pub(crate) generic: G,
    pub(crate) _ty: PhantomData<C>,
    pub(crate) _mutability: PhantomData<M>,
}

impl<'a, C: Component, G: AsGenericGlobalComponent<'a>, M: Borrow<()>>
    ComponentGlobalAccess<C, G, M>
{
    pub fn contains(&self, entity_id: EntityId) -> bool {
        self.generic.get().all_entities.contains(entity_id)
    }

    pub fn get(&self, entity_id: EntityId) -> Option<&'a C> {
        self.generic.get().all_archetypes[entity_id.archetype_id as usize].get(entity_id.id)
    }

    pub fn iter(&self) -> ComponentGlobalIter<'a, C> {
        let mut filtered_archetype_ids = self.generic.get().filtered_archetype_ids.iter();
        let curr_id = *filtered_archetype_ids.next().unwrap();
        let curr_arch = self.generic.get().all_archetypes.get(curr_id).unwrap();
        let curr_iter = curr_arch.component::<C>().unwrap().iter();

        ComponentGlobalIter {
            filtered_archetype_ids,
            all_archetypes: self.generic.get().all_archetypes,
            curr_iter,
        }
    }
}

impl<'a, C: Component + 'a> IntoIterator
    for ComponentGlobalAccess<C, GenericComponentGlobalAccess<'a>, &()>
{
    type Item = &'a C;
    type IntoIter = ComponentGlobalIter<'a, C>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, C: Component + 'a> IntoIterator
for ComponentGlobalAccess<C, Ref<'_, GenericComponentGlobalAccess<'a>>, &()>
{
    type Item = &'a C;
    type IntoIter = ComponentGlobalIter<'a, C>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, C: Component + 'a> IntoIterator
for ComponentGlobalAccess<C, RefMut<'_, GenericComponentGlobalAccess<'a>>, &()>
{
    type Item = &'a C;
    type IntoIter = ComponentGlobalIter<'a, C>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}


impl<'a, C: Component, G: AsGenericGlobalComponent<'a>, M: BorrowMut<()>>
    ComponentGlobalAccess<C, G, M>
{
    pub fn get_mut(&mut self, entity_id: EntityId) -> Option<&'a mut C> {
        let comp = self.generic.get().all_archetypes[entity_id.archetype_id as usize]
            .component::<C>()
            .unwrap();
        comp.contains(entity_id.id)
            .then(|| unsafe { comp.get_mut_unsafe(entity_id.id) })
    }

    pub fn iter_mut(&mut self) -> ComponentGlobalIterMut<'a, C> {
        let mut filtered_archetype_ids = self.generic.get().filtered_archetype_ids.iter();
        let curr_id = *filtered_archetype_ids.next().unwrap();
        let curr_arch = self.generic.get().all_archetypes.get(curr_id).unwrap();
        let curr_iter = unsafe { curr_arch.component::<C>().unwrap().into_iter_mut_unsafe() };

        ComponentGlobalIterMut {
            filtered_archetype_ids,
            all_archetypes: self.generic.get().all_archetypes,
            curr_iter,
        }
    }
}
