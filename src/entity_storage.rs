use crate::archetype::{Archetype, ArchetypeLayout, Component};
use crate::{HashMap};
use std::any::TypeId;
use std::collections::hash_map;
use crate::{ArchetypeState, StaticArchetype};

/// An entity identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
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

    fn get_or_create_archetype<S: ArchetypeState>(&mut self, state: &S) -> usize {
        match self.archetypes_by_types.entry(state.ty()) {
            hash_map::Entry::Vacant(e) => {
                let meta = state.metadata()();
                let layout = ArchetypeLayout::new((meta.component_type_ids)().into_vec());

                let arch_id = match self.archetypes_by_layout.entry(layout) {
                    hash_map::Entry::Vacant(e) => {
                        let new_arch_id = self.archetypes.len();
                        self.archetypes
                            .push(Archetype::new(&(meta.component_infos)()));

                        e.insert(new_arch_id);
                        new_arch_id
                    }
                    hash_map::Entry::Occupied(e) => *e.get(),
                };

                e.insert(arch_id);
                arch_id
            }
            hash_map::Entry::Occupied(e) => *e.get(),
        }
    }

    /// Creates a new entity and returns its identifier.
    pub fn add_entity<S>(&mut self, state: S) -> EntityId
    where
        S: ArchetypeState,
    {
        let arch_id = self.get_or_create_archetype::<S>(&state);
        // Safety: archetype at `arch_id` exists because it is created above if not present.
        let arch = unsafe { self.archetypes.get_unchecked_mut(arch_id) };

        // Safety: layout of the archetype is ensured by `get_or_create_archetype_any`.
        let entity_id = unsafe { arch.add_entity_raw(state.as_ptr()) };
        state.forget();

        EntityId {
            archetype_id: arch_id as u32,
            id: entity_id,
        }
    }

    /// Returns a reference to the specified archetype.
    pub fn get_archetype<A: StaticArchetype>(&self) -> Option<&Archetype> {
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        // Safety: if archetype id is present in the id map, then is must definitely exist.
        unsafe { Some(self.archetypes.get_unchecked(arch_id)) }
    }

    /// Returns a mutable reference to the specified archetype.
    pub fn get_archetype_mut<A: StaticArchetype>(&mut self) -> Option<&mut Archetype> {
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        // Safety: if archetype id is present in the id map, then is must definitely exist.
        unsafe { Some(self.archetypes.get_unchecked_mut(arch_id)) }
    }

    /// Returns a reference to the component `C` of the specified entity.
    pub fn get<C: Component>(&self, entity: &EntityId) -> Option<&C> {
        let arch = self.archetypes.get(entity.archetype_id as usize)?;
        arch.get(entity.id)
    }

    /// Returns a mutable reference to the component `C` of the specified entity.
    pub fn get_mut<C: Component>(&mut self, entity: &EntityId) -> Option<&mut C> {
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
