pub mod component;
pub mod entities;

use crate::archetype::component::{ComponentStorageMut, ComponentStorageRef, UnsafeVec};
use crate::entity::ArchEntityId;
use crate::private::{ArchetypeMetadata, ComponentInfo};
use crate::{ArchetypeState, HashMap, StaticArchetype};
use component::Component;
use entities::ArchetypeEntities;
use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::slice;

#[derive(Clone, Eq)]
pub(crate) struct ArchetypeLayout {
    sorted_type_ids: Vec<TypeId>,
    hash_val: u64,
}

impl ArchetypeLayout {
    pub fn new(mut type_ids: Vec<TypeId>) -> ArchetypeLayout {
        type_ids.sort_unstable();

        let mut hasher = ahash::AHasher::default();
        type_ids.hash(&mut hasher);
        let hash_val = hasher.finish();

        ArchetypeLayout {
            sorted_type_ids: type_ids,
            hash_val,
        }
    }
}

impl PartialEq for ArchetypeLayout {
    fn eq(&self, other: &Self) -> bool {
        self.sorted_type_ids == other.sorted_type_ids
    }
}

impl Hash for ArchetypeLayout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_val.hash(state);
    }
}

/// A collection of entities with unique combination of components.
/// An archetype can hold a maximum of 2^32-1 entities.
pub struct ArchetypeStorage {
    pub(crate) meta: ArchetypeMetadata,
    pub(crate) data: UnsafeVec,
    pub(crate) components: Vec<ComponentInfo>,
    pub(crate) components_by_types: HashMap<TypeId, usize>,
    pub(crate) entities: ArchetypeEntities,
}

impl ArchetypeStorage {
    pub(crate) fn new(meta: ArchetypeMetadata) -> Self {
        let component_infos = meta.component_infos();
        let components_by_types: HashMap<_, _> = component_infos
            .iter()
            .enumerate()
            .map(|(i, info)| (info.type_id, i))
            .collect();

        ArchetypeStorage {
            meta,
            data: Default::default(),
            components: component_infos.to_vec(),
            components_by_types,
            entities: Default::default(),
        }
    }

    fn allocate_slot(&mut self) -> ArchEntityId {
        self.entities.allocate_slot()
    }

    /// Safety: `S` must be of the same component layout as the archetype.
    pub(crate) unsafe fn add_entity_raw(&mut self, state_ptr: *const u8) -> u32 {
        let entity_id = self.allocate_slot();

        let data = self.data.get_mut();
        let offset = entity_id as usize * self.meta.size;

        if offset == data.len() {
            let slice = slice::from_raw_parts(state_ptr, self.meta.size);
            data.extend(slice);
        } else if offset < data.len() {
            let dst_ptr = data.as_mut_ptr().add(offset);
            dst_ptr.copy_from_nonoverlapping(state_ptr, self.meta.size);
        } else {
            unreachable!()
        }

        entity_id
    }

    /// Creates a new entity and returns its identifier.
    pub fn add_entity<S>(&mut self, state: S) -> u32
    where
        S: ArchetypeState,
    {
        let entity_id = unsafe { self.add_entity_raw(state.as_ptr()) };
        state.forget();
        entity_id
    }

    /// Returns `true` if the archetype contains the specified entity.
    pub fn contains(&self, entity_id: ArchEntityId) -> bool {
        self.entities.contains(entity_id)
    }

    #[inline]
    pub fn component<C: Component>(&self) -> Option<ComponentStorageRef<C>> {
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let info = self.components.get(id)?;

        Some(ComponentStorageRef {
            entities: &self.entities,
            step: self.meta.size,
            info,
            data: &self.data,
            _ty: Default::default(),
        })
    }

    #[inline]
    pub fn component_mut<C: Component>(&mut self) -> Option<ComponentStorageMut<C>> {
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let info = self.components.get_mut(id)?;

        Some(ComponentStorageMut {
            entities: &self.entities,
            step: self.meta.size,
            info,
            data: &mut self.data,
            _ty: Default::default(),
        })
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get<C: Component>(&self, entity_id: ArchEntityId) -> Option<&C> {
        let component = self.component::<C>()?;
        component.get(entity_id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut<C: Component>(&mut self, entity_id: ArchEntityId) -> Option<&mut C> {
        let mut component = self.component_mut::<C>()?;
        component.get_mut(entity_id)
    }

    /// Returns a reference to the state at `entity_id`.
    /// Panics if `TypeId` of `S` != `self.ty()`.
    pub fn get_state<S: StaticArchetype>(&self, entity_id: ArchEntityId) -> Option<&S> {
        if self.meta.type_id != TypeId::of::<S>() {
            panic!("invalid type");
        }
        if !self.entities.contains(entity_id) {
            return None;
        }
        unsafe {
            let obj = self.get_ptr(entity_id);
            Some(&*(obj as *const S))
        }
    }

    /// Returns a mutable reference to the state at `entity_id`.
    /// Panics if `TypeId` of `S` != `self.ty()`.
    pub fn get_state_mut<S: StaticArchetype>(&mut self, entity_id: ArchEntityId) -> Option<&mut S> {
        if self.meta.type_id != TypeId::of::<S>() {
            panic!("invalid type");
        }
        if !self.entities.contains(entity_id) {
            return None;
        }
        unsafe {
            let obj = self.get_ptr(entity_id);
            Some(&mut *(obj as *mut S))
        }
    }

    /// Returns a pointer to the entity object. `entity_id` must be valid.
    unsafe fn get_ptr(&self, entity_id: ArchEntityId) -> *mut u8 {
        let data = unsafe { &mut *self.data.get() };
        let offset = self.meta.size * entity_id as usize;
        unsafe { data.as_mut_ptr().add(offset) }
    }

    /// Removes an entity from the archetype. Returns `true` if the entity was present in the archetype.
    pub(crate) fn remove(&mut self, entity_id: ArchEntityId) -> bool {
        let was_present = self.entities.free(entity_id);

        if was_present && self.meta.needs_drop {
            unsafe {
                let ptr = self.get_ptr(entity_id);
                (self.meta.drop_fn)(ptr);
            }
        }

        was_present
    }

    /// Returns iterator of archetype constituent components.
    pub fn iter_component_infos(&self) -> impl Iterator<Item = &ComponentInfo> {
        self.components.iter()
    }

    /// Returns the number of entities in the archetype.
    pub fn count_entities(&self) -> usize {
        self.entities.count()
    }

    /// Returns the `TypeId` of a single state in this archetype.
    pub fn ty(&self) -> &TypeId {
        &self.meta.type_id
    }
}

impl Drop for ArchetypeStorage {
    fn drop(&mut self) {
        if !self.meta.needs_drop {
            return;
        }
        for entity_id in self.entities.iter() {
            unsafe {
                let ptr = self.get_ptr(entity_id);
                (self.meta.drop_fn)(ptr);
            }
        }
    }
}

unsafe impl Sync for ArchetypeStorage {}
