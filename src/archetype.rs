use crate::component_storage::ComponentStorageRef;
use crate::private::ComponentInfo;
use crate::{ArchetypeState, EntityId, HashMap};
use bitvec::vec::BitVec;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::hash::{Hash, Hasher};
use std::{ptr, slice};

pub trait Component: Send + Sync + 'static {}

impl<T> Component for T where T: Send + Sync + 'static {}

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
    pub(crate) components: Vec<(ComponentInfo, UnsafeCell<Vec<u8>>)>,
    pub(crate) components_by_types: HashMap<TypeId, usize>,
    pub(crate) occupied_slots: BitVec,
    pub(crate) components_need_drops: bool,
}

const fn const_min(a: usize, b: usize) -> usize {
    [a, b][(a < b) as usize]
}

impl ArchetypeStorage {
    const MAX_BITVEC_BITS: usize = bitvec::slice::BitSlice::<usize, bitvec::order::Lsb0>::MAX_BITS;
    const MAX_SLOTS: usize = u32::MAX as usize - 1;

    pub const MAX_ENTITIES: u32 = const_min(Self::MAX_SLOTS, Self::MAX_BITVEC_BITS) as u32;

    pub(crate) fn new(comp_infos: &[ComponentInfo]) -> Self {
        let components: Vec<_> = comp_infos
            .iter()
            .map(|info| (info.clone(), UnsafeCell::new(vec![])))
            .collect();

        let components_by_types: HashMap<_, _> = comp_infos
            .iter()
            .enumerate()
            .map(|(i, info)| (info.type_id, i))
            .collect();

        let components_need_drops = comp_infos.iter().any(|info| info.needs_drop);

        ArchetypeStorage {
            components,
            components_by_types,
            occupied_slots: Default::default(),
            components_need_drops,
        }
    }

    fn allocate_slot(&mut self) -> EntityId {
        #[cold]
        #[inline(never)]
        fn assert_failed() -> ! {
            panic!("Archetype: out of slots. A maximum number of entities (2^32-1) is reached.");
        }

        if let Some(free_slot) = self.occupied_slots.iter_zeros().next() {
            self.occupied_slots.set(free_slot, true);
            free_slot as EntityId
        } else if (self.occupied_slots.len() as u32) < Self::MAX_ENTITIES {
            self.occupied_slots.push(true);
            (self.occupied_slots.len() - 1) as EntityId
        } else {
            assert_failed();
        }
    }

    /// Safety: `S` must be of the same component layout as the archetype.
    pub(crate) unsafe fn add_entity_raw(&mut self, state_ptr: *const u8) -> u32 {
        let entity_id = self.allocate_slot();

        for (info, storage) in &mut self.components {
            let component_data = state_ptr.add(info.range.start);
            let comp_size = info.range.len();

            let dst_idx = entity_id as usize * comp_size;

            if dst_idx == storage.get_mut().len() {
                let slice = slice::from_raw_parts(component_data, comp_size);
                storage.get_mut().extend(slice);
            } else {
                let dst_ptr = storage.get_mut().as_mut_ptr().add(dst_idx);
                ptr::copy_nonoverlapping(component_data, dst_ptr, comp_size);
            }
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
    pub fn contains(&self, entity_id: EntityId) -> bool {
        self.occupied_slots
            .get(entity_id as usize)
            .map_or(false, |v| *v)
    }

    #[inline]
    pub fn component<C: Component>(&self) -> Option<ComponentStorageRef<C>> {
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let (_, data) = self.components.get(id)?;

        Some(ComponentStorageRef {
            archetype: self,
            data,
            _ty: Default::default(),
        })
    }

    #[inline]
    pub fn component_mut<C: Component>(&mut self) -> Option<ComponentStorageRef<C>> {
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let (_, data) = self.components.get(id)?;

        Some(ComponentStorageRef {
            archetype: self,
            data,
            _ty: Default::default(),
        })
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get<C: Component>(&self, entity_id: EntityId) -> Option<&C> {
        let component = self.component::<C>()?;
        component.get(entity_id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut<C: Component>(&mut self, entity_id: EntityId) -> Option<&mut C> {
        let mut component = self.component_mut::<C>()?;
        component.get_mut(entity_id)
    }

    /// Removes an entity from the archetype. Returns `true` if the entity was present in the archetype.
    pub(crate) fn remove(&mut self, entity_id: EntityId) -> bool {
        if entity_id >= self.occupied_slots.len() as EntityId {
            return false;
        }

        let is_present = self.occupied_slots.replace(entity_id as usize, false);

        if is_present && self.components_need_drops {
            let id = entity_id as usize;
            for (type_info, data) in &mut self.components {
                if type_info.needs_drop {
                    unsafe {
                        let ptr = data.get_mut().as_mut_ptr().add(id * type_info.range.len());
                        (type_info.drop_func)(ptr);
                    }
                }
            }
        }

        is_present
    }

    /// Returns the number of entities in the archetype.
    pub fn count_entities(&self) -> usize {
        self.occupied_slots.count_ones()
    }
}

impl Drop for ArchetypeStorage {
    fn drop(&mut self) {
        if !self.components_need_drops {
            return;
        }
        for (type_info, data) in &mut self.components {
            if !type_info.needs_drop {
                continue;
            }
            for id in self.occupied_slots.iter_ones() {
                let ptr = unsafe {
                    data.get_mut()
                        .as_mut_ptr()
                        .add(id as usize * type_info.range.len())
                };
                (type_info.drop_func)(ptr);
            }
        }
    }
}
