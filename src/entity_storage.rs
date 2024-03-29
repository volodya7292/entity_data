use crate::archetype::component::Component;
use crate::archetype::entities::EntitiesIter;
use crate::archetype::{ArchetypeLayout, ArchetypeStorage};
use crate::entity::ArchetypeId;
use crate::entry::{Entry, EntryMut};
use crate::{ArchetypeState, StaticArchetype};
use crate::{EntityId, HashMap};
use std::any::TypeId;
use std::collections::hash_map;

/// A container of entities.
#[derive(Default)]
pub struct EntityStorage {
    pub(crate) archetypes: Vec<ArchetypeStorage>,
    pub(crate) archetypes_by_types: HashMap<TypeId, usize>,
    pub(crate) archetypes_by_layout: HashMap<ArchetypeLayout, usize>,
    pub(crate) component_to_archetypes_map: HashMap<TypeId, Vec<usize>>,
}

impl EntityStorage {
    /// Creates an empty `EntityStorage`.
    pub fn new() -> EntityStorage {
        EntityStorage {
            archetypes: Vec::new(),
            archetypes_by_types: Default::default(),
            archetypes_by_layout: Default::default(),
            component_to_archetypes_map: Default::default(),
        }
    }

    fn get_or_create_archetype<S: ArchetypeState>(&mut self, state: &S) -> usize {
        match self.archetypes_by_types.entry(state.ty()) {
            hash_map::Entry::Vacant(e) => {
                let meta = state.metadata();
                let layout = ArchetypeLayout::new((meta.component_type_ids)().into_vec());

                let arch_id = match self.archetypes_by_layout.entry(layout) {
                    hash_map::Entry::Vacant(e) => {
                        let new_arch_id = self.archetypes.len();
                        let archetype = ArchetypeStorage::new(meta);

                        // Map components to the new archetype
                        for info in &archetype.components {
                            self.component_to_archetypes_map
                                .entry(info.type_id)
                                .or_insert(Default::default())
                                .push(new_arch_id);
                        }

                        self.archetypes.push(archetype);

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
    pub fn add<S: ArchetypeState>(&mut self, state: S) -> EntityId {
        let arch_id = self.get_or_create_archetype::<S>(&state);

        // Safety: archetype at `arch_id` exists because it is created above if not present.
        let arch = unsafe { self.archetypes.get_unchecked_mut(arch_id) };

        // Safety: layout of the archetype is ensured by `get_or_create_archetype_any`.
        let entity_id = arch.add_entity(state);

        EntityId {
            archetype_id: arch_id as u32,
            id: entity_id,
        }
    }

    /// Returns a reference to the specified archetype.
    pub fn get_archetype<A: StaticArchetype>(&self) -> Option<&ArchetypeStorage> {
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        // Safety: if archetype id is present in the id map, then is must definitely exist.
        unsafe { Some(self.archetypes.get_unchecked(arch_id)) }
    }

    /// Returns a mutable reference to the specified archetype.
    pub fn get_archetype_mut<A: StaticArchetype>(&mut self) -> Option<&mut ArchetypeStorage> {
        let arch_id = *self.archetypes_by_types.get(&TypeId::of::<A>())?;
        // Safety: if archetype id is present in the id map, then is must definitely exist.
        unsafe { Some(self.archetypes.get_unchecked_mut(arch_id)) }
    }

    /// Maps the specified `TypeId` to respective `ArchetypeId`.
    /// If the storage doesn't contain an archetype of type `type_id`, it returns `None`.
    pub fn type_id_to_archetype_id(&self, type_id: &TypeId) -> Option<ArchetypeId> {
        self.archetypes_by_types.get(type_id).map(|id| *id as u32)
    }

    /// Returns a reference to the specified archetype.
    pub fn get_archetype_by_id(&self, id: ArchetypeId) -> Option<&ArchetypeStorage> {
        self.archetypes.get(id as usize)
    }

    /// Returns a mutable reference to the specified archetype.
    pub fn get_mut_archetype_by_id(&mut self, id: ArchetypeId) -> Option<&mut ArchetypeStorage> {
        self.archetypes.get_mut(id as usize)
    }

    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity: &EntityId) -> bool {
        self.entities().contains(entity)
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

    /// Returns a reference to the state at `entity_id`.
    /// Panics if `TypeId` of `S` is not equal to the type of the underlying archetype.
    pub fn get_state<S: StaticArchetype>(&self, entity_id: &EntityId) -> Option<&S> {
        let arch = self.archetypes.get(entity_id.archetype_id as usize)?;
        arch.get_state(entity_id.id)
    }

    /// Returns a mutable reference to the state at `entity_id`.
    /// Panics if `TypeId` of `S` is not equal to the type of the underlying archetype.
    pub fn get_state_mut<S: StaticArchetype>(&mut self, entity_id: &EntityId) -> Option<&mut S> {
        let arch = self.archetypes.get_mut(entity_id.archetype_id as usize)?;
        arch.get_state_mut(entity_id.id)
    }

    /// Returns an entry of `entity` in the corresponding archetype.
    pub fn entry(&self, entity: &EntityId) -> Option<Entry> {
        Some(Entry {
            arch: self.archetypes.get(entity.archetype_id as usize)?,
            entity: *entity,
        })
    }

    /// Returns a mutable entry of `entity` in the corresponding archetype.
    pub fn entry_mut(&mut self, entity: &EntityId) -> Option<EntryMut> {
        Some(EntryMut {
            arch: self.archetypes.get_mut(entity.archetype_id as usize)?,
            entity: *entity,
        })
    }

    /// Removes an entity from the storage. Returns `true` if the entity was present in the storage.
    pub fn remove(&mut self, entity: &EntityId) -> bool {
        if let Some(arch) = self.archetypes.get_mut(entity.archetype_id as usize) {
            arch.remove(entity.id)
        } else {
            false
        }
    }

    pub fn entities(&self) -> AllEntities {
        AllEntities {
            archetypes: &self.archetypes,
        }
    }

    /// Returns the number of entities in the storage.
    pub fn n_archetypes(&mut self) -> usize {
        self.archetypes.len()
    }

    /// Returns the number of entities in the storage.
    pub fn count_entities(&self) -> usize {
        self.entities().count()
    }
}

#[derive(Copy, Clone)]
pub struct AllEntities<'a> {
    pub(crate) archetypes: &'a [ArchetypeStorage],
}

impl AllEntities<'_> {
    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity: &EntityId) -> bool {
        self.archetypes
            .get(entity.archetype_id as usize)
            .map_or(false, |arch| arch.contains(entity.id))
    }

    /// Returns the number of entities in the storage.
    pub fn count(&self) -> usize {
        self.archetypes
            .iter()
            .fold(0, |acc, arch| acc + arch.count_entities())
    }

    pub fn iter(&self) -> AllEntitiesIter {
        AllEntitiesIter {
            remaining_entities: self.count(),
            archetypes: &self.archetypes,
            curr_arch_id: 0,
            curr_iter: self.archetypes.get(0).map(|arch| arch.entities.iter()),
        }
    }
}

#[derive(Clone)]
pub struct AllEntitiesIter<'a> {
    remaining_entities: usize,
    archetypes: &'a [ArchetypeStorage],
    curr_arch_id: ArchetypeId,
    curr_iter: Option<EntitiesIter<'a>>,
}

impl Iterator for AllEntitiesIter<'_> {
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(arch_entity_id) = self.curr_iter.as_mut().map(|v| v.next()).flatten() {
                self.remaining_entities -= 1;
                return Some(EntityId::new(self.curr_arch_id, arch_entity_id));
            } else {
                self.curr_arch_id += 1;
                let arch = self.archetypes.get(self.curr_arch_id as usize)?;
                self.curr_iter = Some(arch.entities.iter());
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining_entities, Some(self.remaining_entities))
    }
}
