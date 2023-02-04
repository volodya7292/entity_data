use crate::archetype::component;
use crate::archetype::component::ComponentStorageRef;
use crate::entity_storage::AllEntities;
use crate::{ArchetypeStorage, Component, EntityId};
use std::borrow::Borrow;
use std::cell::{Ref, RefMut};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::slice;

pub(crate) type CompMutability = bool;

pub(crate) struct OwningRef<R, U> {
    _r: Pin<Box<R>>,
    data: ManuallyDrop<U>,
}

impl<'a, R: 'a, U: 'a> OwningRef<R, U> {
    /// Safety: any reference to `R` must not be used outside `map` function.
    pub unsafe fn new(r: R, map: fn(&'a R) -> U) -> Self {
        let r = Box::pin(r);

        let r_ptr: *const R = &*r as *const _;
        let data = map(&*r_ptr);

        Self {
            _r: r,
            data: ManuallyDrop::new(data),
        }
    }
}

impl<R, U> Deref for OwningRef<R, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<R, U> DerefMut for OwningRef<R, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<R, U> Drop for OwningRef<R, U> {
    fn drop(&mut self) {
        // Safety: drop `self.data`, which depends on `self.r`, and only then drop `self`.
        unsafe {
            ManuallyDrop::drop(&mut self.data);
        }
    }
}

pub(crate) struct ComponentGlobalIterInner<'a, C> {
    filtered_archetypes_iter: slice::Iter<'a, usize>,
    all_archetypes: &'a [ArchetypeStorage],
    curr_iter: component::Iter<'a, C, ComponentStorageRef<'a, C>>,
}

impl<'a, C: Component> ComponentGlobalIterInner<'a, C> {
    pub fn new(generic: &'a GenericComponentGlobalAccess) -> Self {
        let mut filtered_archetypes_iter = generic.filtered_archetype_ids.iter();
        let curr_id = *filtered_archetypes_iter.next().unwrap();
        let curr_arch = generic.all_archetypes.get(curr_id as usize).unwrap();
        let curr_iter = curr_arch.component::<C>().unwrap().iter();

        Self {
            filtered_archetypes_iter,
            all_archetypes: &generic.all_archetypes,
            curr_iter,
        }
    }
}

pub struct GlobalComponentIter<'a, 'b, C> {
    pub(crate) inner:
        OwningRef<Ref<'b, GenericComponentGlobalAccess<'a>>, ComponentGlobalIterInner<'b, C>>,
}

impl<'a, 'b, C: Component> GlobalComponentIter<'a, 'b, C> {}

impl<'a, 'b: 'a, C: Component> Iterator for GlobalComponentIter<'a, 'b, C> {
    type Item = &'a C;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.inner.curr_iter.next();

            if res.is_some() {
                return res;
            } else {
                let next_arch_id = *self.inner.filtered_archetypes_iter.next()?;
                self.inner.curr_iter = self.inner.all_archetypes[next_arch_id]
                    .component::<C>()?
                    .iter();
            }
        }
    }
}

pub(crate) struct ComponentGlobalIterMutInner<'a, C> {
    filtered_archetypes_iter: slice::Iter<'a, usize>,
    all_archetypes: &'a [ArchetypeStorage],
    curr_iter: component::IterMut<'a, C, ComponentStorageRef<'a, C>>,
}

impl<'a, C: Component> ComponentGlobalIterMutInner<'a, C> {
    pub fn new(generic: &'a GenericComponentGlobalAccess) -> Self {
        let mut filtered_archetypes_iter = generic.filtered_archetype_ids.iter();
        let curr_id = *filtered_archetypes_iter.next().unwrap();
        let curr_arch = generic.all_archetypes.get(curr_id as usize).unwrap();
        let curr_iter = unsafe { curr_arch.component::<C>().unwrap().into_iter_mut_unsafe() };

        Self {
            filtered_archetypes_iter,
            all_archetypes: &generic.all_archetypes,
            curr_iter,
        }
    }
}

pub struct GlobalComponentIterMut<
    'a: 'b,
    'b: 'c,
    'c,
    B: Borrow<RefMut<'b, GenericComponentGlobalAccess<'a>>>,
    C,
> {
    pub(crate) inner: OwningRef<B, ComponentGlobalIterMutInner<'c, C>>,
    pub(crate) _l: PhantomData<(&'a (), &'b ())>,
}

impl<'a, 'b, 'c, B: Borrow<RefMut<'b, GenericComponentGlobalAccess<'a>>> + 'c, C: Component>
    Iterator for GlobalComponentIterMut<'a, 'b, 'c, B, C>
{
    type Item = &'c mut C;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.inner.curr_iter.next();

            if res.is_some() {
                return res;
            } else {
                let next_arch_id = *self.inner.filtered_archetypes_iter.next()?;
                let next_comp =
                    self.inner.all_archetypes[next_arch_id as usize].component::<C>()?;
                self.inner.curr_iter = unsafe { next_comp.into_iter_mut_unsafe() };
            }
        }
    }
}

#[derive(Clone)]
pub struct GenericComponentGlobalAccess<'a> {
    pub(crate) filtered_archetype_ids: Vec<usize>,
    pub(crate) all_archetypes: &'a [ArchetypeStorage],
    pub(crate) all_entities: AllEntities<'a>,
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

pub struct GlobalComponentAccess<C, G> {
    pub(crate) generic: G,
    pub(crate) _ty: PhantomData<C>,
}

impl<'a, C: Component> GlobalComponentAccess<C, Ref<'a, GenericComponentGlobalAccess<'a>>> {
    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: &EntityId) -> bool {
        self.generic.all_entities.contains(entity_id)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get(&self, entity_id: &EntityId) -> Option<&C> {
        self.generic.all_archetypes[entity_id.archetype_id as usize].get(entity_id.id)
    }

    /// Returns total number of entities with the component `C`.
    pub fn count_entities(&self) -> usize {
        self.generic.count_entities()
    }

    /// Returns an iterator over the components.
    pub fn iter<'b: 'a>(&self) -> GlobalComponentIter<'a, 'b, C> {
        GlobalComponentIter {
            inner: unsafe {
                OwningRef::new(Ref::clone(&self.generic), |generic| {
                    ComponentGlobalIterInner::new(generic)
                })
            },
        }
    }
}

impl<'a, 'b: 'a, C: Component + 'a> IntoIterator
    for GlobalComponentAccess<C, Ref<'b, GenericComponentGlobalAccess<'a>>>
{
    type Item = &'a C;
    type IntoIter = GlobalComponentIter<'a, 'b, C>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, 'b, C: Component>
    GlobalComponentAccess<C, RefMut<'b, GenericComponentGlobalAccess<'a>>>
{
    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: &EntityId) -> bool {
        self.generic.all_entities.contains(entity_id)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get(&self, entity_id: &EntityId) -> Option<&C> {
        self.generic.all_archetypes[entity_id.archetype_id as usize].get(entity_id.id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut(&mut self, entity_id: &EntityId) -> Option<&mut C> {
        let comp = self.generic.all_archetypes[entity_id.archetype_id as usize].component::<C>()?;
        comp.contains(entity_id.id)
            .then(|| unsafe { comp.get_mut_unsafe(entity_id.id) })
    }

    /// Returns total number of entities with the component `C`.
    pub fn count_entities(&self) -> usize {
        self.generic.count_entities()
    }

    /// Returns a mutable iterator over the components.
    pub fn iter<'c>(
        &'c mut self,
    ) -> GlobalComponentIterMut<'a, 'b, 'c, &'c mut RefMut<'b, GenericComponentGlobalAccess<'a>>, C>
    {
        GlobalComponentIterMut {
            inner: unsafe {
                OwningRef::new(&mut self.generic, |generic| {
                    ComponentGlobalIterMutInner::new(generic)
                })
            },
            _l: Default::default(),
        }
    }
}

impl<'a, 'b, C: Component + 'a> IntoIterator
    for GlobalComponentAccess<C, RefMut<'b, GenericComponentGlobalAccess<'a>>>
{
    type Item = &'b mut C;
    type IntoIter =
        GlobalComponentIterMut<'a, 'b, 'b, RefMut<'b, GenericComponentGlobalAccess<'a>>, C>;

    fn into_iter(self) -> Self::IntoIter {
        GlobalComponentIterMut {
            inner: unsafe {
                OwningRef::new(self.generic, |generic| {
                    ComponentGlobalIterMutInner::new(generic)
                })
            },
            _l: Default::default(),
        }
    }
}
