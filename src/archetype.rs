use crate::utils::{HashMap, HashSet};
use bit_set::BitSet;
use std::any::TypeId;
use std::hash::{Hash, Hasher};

#[derive(Clone, Eq)]
pub(crate) struct ArchetypeLayout {
    pub(crate) type_ids: HashSet<TypeId>,
    hash_val: u64,
}

impl ArchetypeLayout {
    pub(crate) fn new(type_ids: HashSet<TypeId>) -> ArchetypeLayout {
        let mut hasher = ahash::AHasher::default();
        let mut v: Vec<TypeId> = type_ids.iter().cloned().collect();

        v.sort();
        v.hash(&mut hasher);

        let hash_val = hasher.finish();

        ArchetypeLayout { type_ids, hash_val }
    }
}

impl PartialEq for ArchetypeLayout {
    fn eq(&self, other: &Self) -> bool {
        self.type_ids == other.type_ids
    }
}

impl Hash for ArchetypeLayout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_val.hash(state);
    }
}

#[derive(Copy, Clone)]
pub(crate) struct TypeInfo {
    pub(crate) size: usize,
    pub(crate) needs_drop: bool,
    pub(crate) drop_func: fn(*mut u8),
}

/// A collection of entities with layout of single combination of components.
pub struct Archetype {
    pub(crate) components: HashMap<TypeId, (TypeInfo, Vec<u8>)>,
    pub(crate) free_slots: BitSet,
    pub(crate) total_slot_count: usize,
}

impl Archetype {
    pub(crate) fn new() -> Self {
        Archetype {
            components: Default::default(),
            free_slots: Default::default(),
            total_slot_count: 0,
        }
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get<C: 'static>(&self, entity_id: u32) -> Option<&C> {
        self.components
            .get(&TypeId::of::<C>())
            .map(|(_, data)| unsafe { &*((data.as_ptr() as *const C).offset(entity_id as isize)) })
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut<C: 'static>(&mut self, entity_id: u32) -> Option<&mut C> {
        self.components
            .get_mut(&TypeId::of::<C>())
            .map(|(_, data)| unsafe {
                &mut *((data.as_mut_ptr() as *mut C).offset(entity_id as isize))
            })
    }

    /// Removes an entity from the archetype. Returns `true` if the entity was present in the archetype.
    pub fn remove(&mut self, entity_id: u32) -> bool {
        let id = entity_id as usize;
        let is_present = id < self.total_slot_count && !self.free_slots.insert(id);

        if is_present {
            for (_, (type_info, data)) in &mut self.components {
                if type_info.needs_drop {
                    unsafe {
                        let ptr = data.as_mut_ptr().add(id * type_info.size);
                        (type_info.drop_func)(ptr);
                    }
                }
            }
        }

        return is_present;
    }

    /// Returns the number of entities in the archetype.
    pub fn len(&self) -> usize {
        self.total_slot_count - self.free_slots.len()
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        for (_, (type_info, data)) in &mut self.components {
            if type_info.needs_drop {
                for id in 0..self.total_slot_count {
                    if !self.free_slots.contains(id) {
                        let ptr = unsafe { data.as_mut_ptr().add(id * type_info.size) };
                        (type_info.drop_func)(ptr);
                    }
                }
            }
        }
    }
}
