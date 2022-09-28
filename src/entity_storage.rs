use crate::archetype::{Archetype, ArchetypeLayout};
use crate::private::{ArchetypeImpl, IsArchetype};
use crate::HashMap;
use std::any::TypeId;
use std::collections::hash_map;
use std::{mem, ptr, slice};

/// An entity identifier.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct EntityId {
    pub archetype_id: u32,
    pub id: u32,
}

impl EntityId {
    pub const NULL: Self = EntityId {
        archetype_id: u32::MAX,
        id: u32::MAX,
    };

    pub fn new(archetype_id: u32, id: u32) -> EntityId {
        EntityId { archetype_id, id }
    }
}

impl Default for EntityId {
    fn default() -> Self {
        EntityId::NULL
    }
}

/// A container of entities.
pub struct EntityStorage {
    archetypes: Vec<Archetype>,
    archetypes_by_types: HashMap<TypeId, usize>,
    archetypes_by_layout: HashMap<ArchetypeLayout, usize>,
}

impl EntityStorage {
    /// Creates an empty `EntityStorage`.
    pub fn new() -> EntityStorage {
        EntityStorage {
            archetypes: Vec::new(),
            archetypes_by_types: Default::default(),
            archetypes_by_layout: Default::default(),
        }
    }

    /// Creates a new entity and returns its identifier.
    pub fn add_entity<const N: usize, S>(&mut self, state: S) -> EntityId
    where
        S: ArchetypeImpl<N> + 'static,
    {
        let arch_id = match self.archetypes_by_types.entry(TypeId::of::<S>()) {
            hash_map::Entry::Vacant(e) => {
                let layout = ArchetypeLayout::new(S::component_type_ids().to_vec());

                let arch_id = match self.archetypes_by_layout.entry(layout) {
                    hash_map::Entry::Vacant(e) => {
                        let new_arch_id = self.archetypes.len();
                        e.insert(new_arch_id);
                        new_arch_id
                    }
                    hash_map::Entry::Occupied(e) => *e.get(),
                };

                e.insert(arch_id);
                self.archetypes.push(Archetype::new::<N, S>());
                arch_id
            }
            hash_map::Entry::Occupied(e) => *e.get(),
        };

        // Safety: archetype at `arch_id` exists because it is created above if not present.
        let arch = unsafe { self.archetypes.get_unchecked_mut(arch_id) };

        let entity_id = arch.allocate_slot();

        for info in S::component_infos() {
            let (_, all_components_data) = arch.components.get_mut(&info.type_id).unwrap();
            let component_data = unsafe { (&state as *const _ as *const u8).add(info.range.start) };
            let comp_size = info.range.len();

            if entity_id == (arch.total_slot_count - 1) {
                let slice = unsafe { slice::from_raw_parts(component_data, comp_size) };
                all_components_data.extend(slice);
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(
                        component_data,
                        all_components_data.as_mut_ptr().add(entity_id * comp_size),
                        comp_size,
                    );
                }
            }
        }

        mem::forget(state);

        EntityId {
            archetype_id: arch_id as u32,
            id: entity_id as u32,
        }
    }

    /// Returns a reference to the specified archetype.
    pub fn get_archetype<A: IsArchetype + 'static>(&self) -> Option<&Archetype> {
        // Safety: if archetype id is present in id map, then is must definitely exist.
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        unsafe { Some(self.archetypes.get_unchecked(arch_id)) }
    }

    /// Returns a mutable reference to the specified archetype.
    pub fn get_archetype_mut<A: IsArchetype + 'static>(&mut self) -> Option<&mut Archetype> {
        // Safety: if archetype id is present in id map, then is must definitely exist.
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        unsafe { Some(self.archetypes.get_unchecked_mut(arch_id)) }
    }

    /// Returns a reference to the component `C` of the specified entity.
    pub fn get<C: 'static>(&self, entity: &EntityId) -> Option<&C> {
        self.archetypes
            .get(entity.archetype_id as usize)
            .map(|arch| arch.get(entity.id))
            .flatten()
    }

    /// Returns a mutable reference to the component `C` of the specified entity.
    pub fn get_mut<C: 'static>(&mut self, entity: &EntityId) -> Option<&mut C> {
        self.archetypes
            .get_mut(entity.archetype_id as usize)
            .map(|arch| arch.get_mut(entity.id))
            .flatten()
    }

    /// Removes an entity from the storage. Returns `true` if the entity was present in the storage.
    pub fn remove(&mut self, entity: &EntityId) -> bool {
        if let Some(arch) = self.archetypes.get_mut(entity.archetype_id as usize) {
            arch.remove(entity.id)
        } else {
            false
        }
    }

    /// Returns the number of entities in the storage.
    pub fn len(&mut self) -> usize {
        self.archetypes.iter().fold(0, |acc, arch| acc + arch.len())
    }
}
