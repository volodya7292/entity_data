use crate::archetype::{Archetype, ArchetypeLayout, TypeInfo};
use crate::utils::{HashMap, HashSet};
use std::any::{Any, TypeId};
use std::collections::hash_map;
use std::{mem, ptr, slice};

/// An opaque entity identifier.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Entity {
    archetype_id: u32,
    id: u32,
}

impl Entity {
    pub fn archetype_id(&self) -> u32 {
        self.archetype_id
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

/// A layout of the `EntityStorage`.
/// Can be used to crate multiple `EntityStorage`s with the same archetypes.
#[derive(Clone, Default)]
pub struct EntityStorageLayout {
    archetype_layouts: HashMap<ArchetypeLayout, u32>,
    type_infos: HashMap<TypeId, TypeInfo>,
}

impl EntityStorageLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_archetype(&mut self) -> ArchetypeBuilder {
        return ArchetypeBuilder {
            layout: self,
            type_ids: HashSet::with_capacity(8),
        };
    }
}

/// A container of entities.
pub struct EntityStorage {
    archetypes: Vec<Archetype>,
    temp_data: Vec<u8>,
    temp_offsets: HashMap<TypeId, (usize, usize)>,
}

impl EntityStorage {
    /// Creates an empty `EntityStorage`.
    pub fn new(layout: &EntityStorageLayout) -> EntityStorage {
        let mut archetypes: Vec<Archetype> = (0..layout.archetype_layouts.len())
            .map(|_| Archetype::new())
            .collect();

        for (arch_layout, id) in &layout.archetype_layouts {
            archetypes[*id as usize].components.extend(
                arch_layout
                    .type_ids
                    .iter()
                    .map(|type_id| (*type_id, (layout.type_infos[type_id], Default::default()))),
            );
        }

        EntityStorage {
            archetypes,
            temp_data: vec![],
            temp_offsets: Default::default(),
        }
    }

    /// Returns `EntityBuilder` of the new entity.
    pub fn add_entity(&mut self, archetype_id: u32) -> EntityBuilder {
        return EntityBuilder {
            container: self,
            archetype_id,
        };
    }

    /// Returns a reference to the specified archetype.
    pub fn get_archetype(&self, id: u32) -> Option<&Archetype> {
        self.archetypes.get(id as usize)
    }

    /// Returns a mutable reference to the specified archetype.
    pub fn get_archetype_mut(&mut self, id: u32) -> Option<&mut Archetype> {
        self.archetypes.get_mut(id as usize)
    }

    /// Returns a reference to the component `C` of the specified entity.
    pub fn get<C: 'static>(&self, entity: &Entity) -> Option<&C> {
        self.archetypes
            .get(entity.archetype_id as usize)
            .map(|arch| arch.get(entity.id))
            .flatten()
    }

    /// Returns a mutable reference to the component `C` of the specified entity.
    pub fn get_mut<C: 'static>(&mut self, entity: &Entity) -> Option<&mut C> {
        self.archetypes
            .get_mut(entity.archetype_id as usize)
            .map(|arch| arch.get_mut(entity.id))
            .flatten()
    }

    /// Removes an entity from the storage. Returns `true` if the entity was present in the storage.
    pub fn remove(&mut self, entity: &Entity) -> bool {
        return if let Some(arch) = self.archetypes.get_mut(entity.archetype_id as usize) {
            arch.remove(entity.id)
        } else {
            false
        };
    }

    /// Returns the number of entities in the storage.
    pub fn len(&mut self) -> usize {
        self.archetypes.iter().fold(0, |acc, arch| acc + arch.len())
    }
}

/// An archetype builder.
pub struct ArchetypeBuilder<'a> {
    layout: &'a mut EntityStorageLayout,
    type_ids: HashSet<TypeId>,
}

impl<'a> ArchetypeBuilder<'a> {
    /// Adds a component type to the archetype layout. Adding already present component type will have no effect.
    pub fn with<C: 'static>(mut self) -> Self {
        let type_id = TypeId::of::<C>();
        if let hash_map::Entry::Vacant(e) = self.layout.type_infos.entry(type_id) {
            let drop_func = |p: *mut u8| unsafe { ptr::drop_in_place(p as *mut C) };
            e.insert(TypeInfo {
                size: mem::size_of::<C>(),
                needs_drop: mem::needs_drop::<C>(),
                drop_func,
            });
        }

        self.type_ids.insert(type_id);

        return self;
    }

    /// Returns id of the archetype. Equivalent archetypes have identical ids.
    pub fn build(self) -> u32 {
        let layout = ArchetypeLayout::new(self.type_ids);
        let next_id = self.layout.archetype_layouts.len() as u32;

        match self.layout.archetype_layouts.entry(layout) {
            hash_map::Entry::Vacant(e) => {
                e.insert(next_id);
                next_id
            }
            hash_map::Entry::Occupied(e) => *e.get(),
        }
    }
}

/// An entity builder.
pub struct EntityBuilder<'a> {
    container: &'a mut EntityStorage,
    archetype_id: u32,
}

impl EntityBuilder<'_> {
    /// Adds a component to the entity. Adding already present component will have no effect.
    pub fn with<C: 'static>(self, component: C) -> Self {
        let bytes = unsafe {
            slice::from_raw_parts(&component as *const C as *const u8, mem::size_of::<C>())
        };
        let offset = self.container.temp_data.len();

        if let hash_map::Entry::Vacant(e) = self.container.temp_offsets.entry(component.type_id()) {
            self.container.temp_data.extend(bytes);
            e.insert((offset, mem::size_of_val(&component)));
            mem::forget(component);
        }

        return self;
    }

    /// Builds a new entity and returns its identifier.
    ///
    /// # Panics
    ///
    /// - if specified archetype is not found;
    /// - if specified component types != archetype component types;
    pub fn build(self) -> Entity {
        #[cold]
        #[inline(never)]
        fn assert_failed(id: u32) -> ! {
            panic!("archetype (id {}) not found", id);
        }

        #[cold]
        #[inline(never)]
        fn assert_failed2(c1: usize, c2: usize) -> ! {
            panic!(
                "entity component count ({}) != archetype component count ({})",
                c1, c2
            );
        }

        #[cold]
        #[inline(never)]
        fn assert_failed3(id: u32) -> ! {
            panic!("TypeId doesn't exist in archetype {}", id);
        }

        let temp_offsets = &mut self.container.temp_offsets;
        let temp_data = &self.container.temp_data;
        let archetype_id = self.archetype_id;

        let entity_id = if let Some(arch) = self
            .container
            .archetypes
            .get_mut(self.archetype_id as usize)
        {
            let entity_comp_count = temp_offsets.len();
            let arch_comp_count = arch.components.len();
            if entity_comp_count != arch_comp_count {
                assert_failed2(entity_comp_count, arch_comp_count);
            }

            let id = if let Some(free_slot) = arch.free_slots.iter().next() {
                arch.free_slots.remove(free_slot);
                free_slot
            } else {
                arch.total_slot_count += 1;
                arch.total_slot_count - 1
            };

            temp_offsets.drain().for_each(|(type_id, (offset, size))| {
                if let Some((_, data)) = arch.components.get_mut(&type_id) {
                    if id == (arch.total_slot_count - 1) {
                        data.extend(&temp_data[offset..(offset + size)]);
                    } else {
                        unsafe {
                            ptr::copy_nonoverlapping(
                                temp_data.as_ptr().add(offset),
                                data.as_mut_ptr().add(id * size),
                                size,
                            );
                        }
                    }
                } else {
                    assert_failed3(archetype_id);
                }
            });

            id as u32
        } else {
            assert_failed(archetype_id);
        };

        self.container.temp_data.clear();

        Entity {
            archetype_id,
            id: entity_id,
        }
    }
}
