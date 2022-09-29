use crate::archetype::{AnyState, Archetype, ArchetypeLayout};
use crate::private::ComponentInfo;
use crate::{ArchetypeImpl, HashMap, IsArchetype};
use std::any::TypeId;
use std::collections::hash_map;
use std::mem;

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

    fn get_or_create_archetype_by_layout(
        archetypes_by_layout: &mut HashMap<ArchetypeLayout, usize>,
        archetypes: &mut Vec<Archetype>,
        component_type_ids: Vec<TypeId>,
        component_infos: &[ComponentInfo],
    ) -> usize {
        let layout = ArchetypeLayout::new(component_type_ids);

        match archetypes_by_layout.entry(layout) {
            hash_map::Entry::Vacant(e) => {
                let new_arch_id = archetypes.len();
                archetypes.push(Archetype::new(component_infos));

                e.insert(new_arch_id);
                new_arch_id
            }
            hash_map::Entry::Occupied(e) => *e.get(),
        }
    }

    fn get_or_create_archetype<const N: usize, S: ArchetypeImpl<N> + 'static>(&mut self) -> usize {
        match self.archetypes_by_types.entry(TypeId::of::<S>()) {
            hash_map::Entry::Vacant(e) => {
                let arch_id = Self::get_or_create_archetype_by_layout(
                    &mut self.archetypes_by_layout,
                    &mut self.archetypes,
                    S::component_type_ids().to_vec(),
                    &S::component_infos(),
                );
                e.insert(arch_id);
                arch_id
            }
            hash_map::Entry::Occupied(e) => *e.get(),
        }
    }

    fn get_or_create_archetype_any(&mut self, state: &AnyState) -> usize {
        match self.archetypes_by_types.entry(state.ty) {
            hash_map::Entry::Vacant(e) => {
                let arch_id = Self::get_or_create_archetype_by_layout(
                    &mut self.archetypes_by_layout,
                    &mut self.archetypes,
                    (state.component_type_ids)(),
                    &(state.component_infos)(),
                );
                e.insert(arch_id);
                arch_id
            }
            hash_map::Entry::Occupied(e) => *e.get(),
        }
    }

    /// Creates a new entity and returns its identifier.
    pub fn add_entity<const N: usize, S>(&mut self, state: S) -> EntityId
    where
        S: ArchetypeImpl<N> + 'static,
    {
        let arch_id = self.get_or_create_archetype::<N, S>();
        // Safety: archetype at `arch_id` exists because it is created above if not present.
        let arch = unsafe { self.archetypes.get_unchecked_mut(arch_id) };

        // Safety: layout of the archetype is ensured by `get_or_create_archetype_any`.
        let entity_id = unsafe { arch.add_entity_raw(&state as *const _ as *const u8) };
        mem::forget(state);

        EntityId {
            archetype_id: arch_id as u32,
            id: entity_id,
        }
    }

    /// Creates a new entity and returns its identifier.
    pub fn add_entity_any(&mut self, state: AnyState) -> EntityId {
        let arch_id = self.get_or_create_archetype_any(&state);
        // Safety: archetype at `arch_id` exists because it is created if not present.
        let arch = unsafe { self.archetypes.get_unchecked_mut(arch_id) };

        // Safety: layout of the archetype is ensured by `get_or_create_archetype_any`.
        let entity_id = unsafe { arch.add_entity_raw(state.data.as_ptr()) };
        mem::forget(state);

        EntityId {
            archetype_id: arch_id as u32,
            id: entity_id,
        }
    }

    /// Returns a reference to the specified archetype.
    pub fn get_archetype<A: IsArchetype + 'static>(&self) -> Option<&Archetype> {
        // Safety: if archetype id is present in the id map, then is must definitely exist.
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        unsafe { Some(self.archetypes.get_unchecked(arch_id)) }
    }

    /// Returns a mutable reference to the specified archetype.
    pub fn get_archetype_mut<A: IsArchetype + 'static>(&mut self) -> Option<&mut Archetype> {
        // Safety: if archetype id is present in the id map, then is must definitely exist.
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        unsafe { Some(self.archetypes.get_unchecked_mut(arch_id)) }
    }

    /// Returns a reference to the component `C` of the specified entity.
    pub fn get<C: 'static>(&self, entity: &EntityId) -> Option<&C> {
        let arch = self.archetypes.get(entity.archetype_id as usize)?;
        arch.get(entity.id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity.
    pub fn get_mut<C: 'static>(&mut self, entity: &EntityId) -> Option<&mut C> {
        let arch = self.archetypes.get_mut(entity.archetype_id as usize)?;
        arch.get_mut(entity.id)
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
    pub fn n_archetypes(&mut self) -> usize {
        self.archetypes.len()
    }

    /// Returns the number of entities in the storage.
    pub fn count_entities(&mut self) -> usize {
        self.archetypes.iter().fold(0, |acc, arch| acc + arch.len())
    }
}
