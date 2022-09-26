use crate::archetype::{Archetype, ArchetypeLayout, TypeInfo};
use crate::utils::{HashMap, HashSet};
use std::any::TypeId;
use std::collections::hash_map;
use std::ops::Range;
use std::{mem, ptr};

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
        ArchetypeBuilder {
            layout: self,
            type_ids: HashSet::with_capacity(8),
        }
    }
}

/// A container of entities.
pub struct EntityStorage {
    archetypes: Vec<Archetype>,
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

        EntityStorage { archetypes }
    }

    /// Builds a new entity and returns its identifier.
    ///
    /// # Panics
    ///
    /// - if specified archetype is not found;
    /// - if specified component types != archetype component types;
    pub fn add_entity<const N: usize, const D: usize>(
        &mut self,
        archetype_id: u32,
        state: EntityState<N, D>,
    ) -> EntityId {
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

        let entity_id = if let Some(arch) = self.archetypes.get_mut(archetype_id as usize) {
            if state.offsets.len() != arch.components.len() {
                assert_failed2(state.offsets.len(), arch.components.len());
            }

            let id = if let Some(free_slot) = arch.free_slots.iter().next() {
                arch.free_slots.remove(free_slot);
                free_slot
            } else {
                arch.total_slot_count += 1;
                arch.total_slot_count - 1
            };

            for info in state.offsets {
                if let Some((_, data)) = arch.components.get_mut(&info.type_id) {
                    if id == (arch.total_slot_count - 1) {
                        data.extend(&state.data[info.range]);
                    } else {
                        let comp_size = info.range.len();
                        unsafe {
                            ptr::copy_nonoverlapping(
                                state.data.as_ptr().add(info.range.start),
                                data.as_mut_ptr().add(id * comp_size),
                                comp_size,
                            );
                        }
                    }
                } else {
                    assert_failed3(archetype_id);
                }
            }

            id as u32
        } else {
            assert_failed(archetype_id);
        };

        EntityId {
            archetype_id,
            id: entity_id,
        }
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
        self
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

pub struct ComponentInfo {
    pub type_id: TypeId,
    pub range: Range<usize>,
}

/// An entity state comprising of different components.
pub struct EntityState<const N: usize, const DATA_SIZE: usize> {
    data: [u8; DATA_SIZE],
    offsets: [ComponentInfo; N],
}

impl<const N: usize, const D: usize> EntityState<N, D> {
    pub fn from_raw(data: [u8; D], offsets: [ComponentInfo; N]) -> Self {
        Self { data, offsets }
    }
}
