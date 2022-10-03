use crate::private::ComponentInfo;
use crate::HashMap;
use bit_set::BitSet;
use std::any::TypeId;
use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::{ptr, slice};

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
pub struct Archetype {
    pub(crate) components: Vec<(ComponentInfo, Vec<u8>)>,
    pub(crate) components_by_types: HashMap<TypeId, usize>,
    pub(crate) free_slots: BitSet,
    pub(crate) total_slot_count: u32,
    pub(crate) components_need_drops: bool,
    /// `ArchetypeState` may not be `Sync`, hence `Archetype` may not be either.
    _unsync: PhantomData<Cell<()>>,
}

impl Archetype {
    pub const MAX_ENTITIES: u32 = u32::MAX - 1;

    pub(crate) fn new(comp_infos: &[ComponentInfo]) -> Self {
        let components: Vec<_> = comp_infos
            .iter()
            .map(|info| (info.clone(), vec![]))
            .collect();

        let components_by_types: HashMap<_, _> = comp_infos
            .iter()
            .enumerate()
            .map(|(i, info)| (info.type_id, i))
            .collect();

        let components_need_drops = comp_infos.iter().any(|info| info.needs_drop);

        Archetype {
            components,
            components_by_types,
            free_slots: Default::default(),
            total_slot_count: 0,
            components_need_drops,
            _unsync: Default::default(),
        }
    }

    fn allocate_slot(&mut self) -> u32 {
        #[cold]
        #[inline(never)]
        fn assert_failed() -> ! {
            panic!("Archetype: out of slots. A maximum number of entities (2^32-1) is reached.");
        }

        if let Some(free_slot) = self.free_slots.iter().next() {
            self.free_slots.remove(free_slot);
            free_slot as u32
        } else if self.total_slot_count < Self::MAX_ENTITIES {
            self.total_slot_count += 1;
            self.total_slot_count - 1
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

            if entity_id == (self.total_slot_count - 1) {
                let slice = slice::from_raw_parts(component_data, comp_size);
                storage.extend(slice);
            } else {
                let dst_ptr = storage.as_mut_ptr().add(entity_id as usize * comp_size);
                ptr::copy_nonoverlapping(component_data, dst_ptr, comp_size);
            }
        }

        entity_id
    }

    pub fn is_present(&self, entity_id: u32) -> bool {
        entity_id < self.total_slot_count && !self.free_slots.contains(entity_id as usize)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get<C: 'static>(&self, entity_id: u32) -> Option<&C> {
        if !self.is_present(entity_id) {
            return None;
        }
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let (_, data) = self.components.get(id)?;
        unsafe {
            let ptr = (data.as_ptr() as *const C).offset(entity_id as isize);
            Some(&*ptr)
        }
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut<C: 'static>(&mut self, entity_id: u32) -> Option<&mut C> {
        if !self.is_present(entity_id) {
            return None;
        }
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let (_, data) = self.components.get_mut(id)?;
        unsafe {
            let ptr = (data.as_mut_ptr() as *mut C).offset(entity_id as isize);
            Some(&mut *ptr)
        }
    }

    /// Removes an entity from the archetype. Returns `true` if the entity was present in the archetype.
    pub fn remove(&mut self, entity_id: u32) -> bool {
        let mut is_present = entity_id < self.total_slot_count;

        is_present &= !self.free_slots.insert(entity_id as usize);

        if is_present && self.components_need_drops {
            let id = entity_id as usize;
            for (type_info, data) in &mut self.components {
                if type_info.needs_drop {
                    unsafe {
                        let ptr = data.as_mut_ptr().add(id * type_info.range.len());
                        (type_info.drop_func)(ptr);
                    }
                }
            }
        }

        is_present
    }

    /// Returns the number of entities in the archetype.
    pub fn len(&self) -> usize {
        self.total_slot_count as usize - self.free_slots.len()
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        if !self.components_need_drops {
            return;
        }
        for (type_info, data) in &mut self.components {
            if !type_info.needs_drop {
                continue;
            }
            for id in 0..self.total_slot_count {
                if !self.free_slots.contains(id as usize) {
                    let ptr = unsafe { data.as_mut_ptr().add(id as usize * type_info.range.len()) };
                    (type_info.drop_func)(ptr);
                }
            }
        }
    }
}
