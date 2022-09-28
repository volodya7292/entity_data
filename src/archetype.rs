use crate::private::ArchetypeImpl;
use crate::HashMap;
use bit_set::BitSet;
use std::any::TypeId;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone)]
pub(crate) struct TypeInfo {
    pub(crate) size: usize,
    pub(crate) needs_drop: bool,
    pub(crate) drop_func: fn(*mut u8),
}

#[derive(Clone, Eq)]
pub(crate) struct ArchetypeLayout {
    sorted_type_ids: Vec<TypeId>,
    hash_val: u64,
}

impl ArchetypeLayout {
    pub fn new(mut type_ids: Vec<TypeId>) -> ArchetypeLayout {
        type_ids.sort();

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
pub struct Archetype {
    pub(crate) components: HashMap<TypeId, (TypeInfo, Vec<u8>)>,
    pub(crate) free_slots: BitSet,
    pub(crate) total_slot_count: usize,
}

impl Archetype {
    pub(crate) fn new<const N: usize, A: ArchetypeImpl<N>>() -> Self {
        let components: HashMap<_, _> = A::component_infos()
            .into_iter()
            .map(|info| {
                (
                    info.type_id,
                    (
                        TypeInfo {
                            size: info.range.len(),
                            needs_drop: info.needs_drop,
                            drop_func: info.drop_func,
                        },
                        vec![],
                    ),
                )
            })
            .collect();

        Archetype {
            components,
            free_slots: Default::default(),
            total_slot_count: 0,
        }
    }

    pub(crate) fn allocate_slot(&mut self) -> usize {
        if let Some(free_slot) = self.free_slots.iter().next() {
            self.free_slots.remove(free_slot);
            free_slot
        } else {
            self.total_slot_count += 1;
            self.total_slot_count - 1
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

        is_present
    }

    /// Returns the number of entities in the archetype.
    pub fn len(&self) -> usize {
        self.total_slot_count - self.free_slots.len()
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        for (_, (type_info, data)) in &mut self.components {
            if !type_info.needs_drop {
                continue;
            }
            for id in 0..self.total_slot_count {
                if !self.free_slots.contains(id) {
                    let ptr = unsafe { data.as_mut_ptr().add(id * type_info.size) };
                    (type_info.drop_func)(ptr);
                }
            }
        }
    }
}
