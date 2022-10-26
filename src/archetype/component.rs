use crate::archetype::entities::{ArchetypeEntities, EntitiesIter};
use crate::entity::ArchEntityId;
use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Default)]
pub struct UnsafeVec(UnsafeCell<Vec<u8>>);

pub trait Component: Send + Sync + 'static {}

impl Deref for UnsafeVec {
    type Target = UnsafeCell<Vec<u8>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UnsafeVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Component for T where T: Send + Sync + 'static {}

pub struct ComponentStorage<'a, C, D> {
    pub(crate) entities: &'a ArchetypeEntities,
    pub(crate) data: D,
    pub(crate) _ty: PhantomData<C>,
}

pub type ComponentStorageRef<'a, C> = ComponentStorage<'a, C, &'a UnsafeVec>;
pub type ComponentStorageMut<'a, C> = ComponentStorage<'a, C, &'a mut UnsafeVec>;

impl<'a, C, D: Borrow<UnsafeVec> + Copy> Clone for ComponentStorage<'a, C, D> {
    fn clone(&self) -> Self {
        Self {
            entities: &self.entities,
            data: self.data,
            _ty: Default::default(),
        }
    }
}

impl<'a, C, D: Borrow<UnsafeVec> + Copy> Copy for ComponentStorage<'a, C, D> {}

impl<'a, C: Component, D: Borrow<UnsafeVec>> ComponentStorage<'a, C, D> {
    pub fn contains(&self, entity_id: ArchEntityId) -> bool {
        self.entities.contains(entity_id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    /// # Safety:
    /// To not cause any undefined behavior, the following conditions must be met:
    /// * Entity at `entity_id` must exist.
    /// * `&mut C` must always be unique.
    pub(crate) unsafe fn get_mut_unsafe(&self, entity_id: ArchEntityId) -> &'a mut C {
        let ptr = ((&*self.data.borrow().get()).as_ptr() as *mut C).offset(entity_id as isize);
        &mut *ptr
    }

    /// Returns an iterator over all components. Safety: see [get_mut_unsafe](Self::get_mut_unsafe).
    pub(crate) unsafe fn into_iter_mut_unsafe(self) -> IterMut<'a, C, Self> {
        IterMut {
            entities_iter: self.entities.iter(),
            data: self,
            _ty: Default::default(),
        }
    }

    /// Returns a reference to the component `C` of the specified entity.
    /// Safety: entity must exist.
    pub unsafe fn get_unchecked(&self, entity_id: ArchEntityId) -> &'a C {
        // Safety: the method does not mutate `self`
        self.get_mut_unsafe(entity_id)
    }

    /// Returns a reference to component `C` of the specified entity.
    pub fn get(&self, entity_id: ArchEntityId) -> Option<&'a C> {
        if !self.contains(entity_id) {
            return None;
        }
        unsafe { Some(self.get_unchecked(entity_id)) }
    }
}

impl<'a, C: Component> ComponentStorageRef<'a, C> {
    /// Returns an iterator over all components.
    pub fn iter(self) -> Iter<'a, C, Self> {
        Iter {
            entities_iter: self.entities.iter(),
            data: self,
            _ty: Default::default(),
        }
    }
}

impl<'a, C: Component + 'a> IntoIterator for ComponentStorageRef<'a, C> {
    type Item = &'a C;
    type IntoIter = Iter<'a, C, Self>;

    /// Returns an iterator over all components.
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, C: Component> ComponentStorageMut<'a, C> {
    /// Returns a mutable reference to the component `C` of the specified entity id.
    /// Safety: component at `entity_id` must exist.
    pub unsafe fn get_unchecked_mut(&mut self, entity_id: ArchEntityId) -> &'a mut C {
        self.get_mut_unsafe(entity_id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut(&mut self, entity_id: ArchEntityId) -> Option<&'a mut C> {
        if !self.contains(entity_id) {
            return None;
        }
        unsafe { Some(self.get_unchecked_mut(entity_id)) }
    }

    /// Returns an iterator over all components.
    pub fn iter_mut(&'a mut self) -> IterMut<'a, C, &mut Self> {
        IterMut {
            entities_iter: self.entities.iter(),
            data: self,
            _ty: Default::default(),
        }
    }
}

impl<'a, C: Component + 'a> IntoIterator for ComponentStorageMut<'a, C> {
    type Item = &'a mut C;
    type IntoIter = IterMut<'a, C, ComponentStorageRef<'a, C>>;

    /// Returns an iterator over all components.
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            entities_iter: self.entities.iter(),
            data: ComponentStorageRef {
                entities: self.entities,
                data: self.data,
                _ty: Default::default(),
            },
            _ty: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct Iter<'a, C, D> {
    pub(crate) entities_iter: EntitiesIter<'a>,
    pub(crate) data: D,
    pub(crate) _ty: PhantomData<C>,
}

impl<'a, C, D> Iterator for Iter<'a, C, D>
where
    C: Component + 'a,
    D: Borrow<ComponentStorageRef<'a, C>>,
{
    type Item = &'a C;

    fn next(&mut self) -> Option<Self::Item> {
        self.entities_iter
            .next()
            .map(|entity_id| unsafe { self.data.borrow().get_unchecked(entity_id) })
    }
}

pub struct IterMut<'a, C, D> {
    pub(crate) entities_iter: EntitiesIter<'a>,
    pub(crate) data: D,
    pub(crate) _ty: PhantomData<C>,
}

impl<'a, C, D> Iterator for IterMut<'a, C, D>
where
    C: Component + 'a,
    D: Borrow<ComponentStorageRef<'a, C>>,
{
    type Item = &'a mut C;

    fn next(&mut self) -> Option<Self::Item> {
        self.entities_iter
            .next()
            .map(|entity_id| unsafe { self.data.borrow().get_mut_unsafe(entity_id) })
    }
}
