use crate::entity::ArchEntityId;
use index_pool::IndexPool;

#[derive(Default)]
pub struct ArchetypeEntities {
    occupied_ids: IndexPool,
}

impl<'a> ArchetypeEntities {
    pub const MAX_ENTITIES: usize = u32::MAX as usize - 1;

    pub(crate) fn allocate_slot(&mut self) -> ArchEntityId {
        if self.occupied_ids.in_use() >= Self::MAX_ENTITIES {
            panic!(
                "Out of slots. A maximum number of entities ({}) is reached.",
                ArchetypeEntities::MAX_ENTITIES
            );
        }

        let new_id = self.occupied_ids.new_id();
        new_id as ArchEntityId
    }

    /// Returns `true` if the entity was present.
    pub(crate) fn free(&mut self, entity_id: ArchEntityId) -> bool {
        let result = self.occupied_ids.return_id(entity_id as usize);
        result != Err(index_pool::AlreadyReturned)
    }

    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: ArchEntityId) -> bool {
        !self.occupied_ids.is_free(entity_id as usize)
    }

    /// Returns an iterator over all entities of the archetype.
    pub fn iter(&'a self) -> EntitiesIter {
        EntitiesIter(self.occupied_ids.all_indices().into_iter())
    }

    /// Returns the number of entities in the archetype.
    pub fn count(&self) -> usize {
        self.occupied_ids.in_use()
    }
}

#[derive(Clone)]
pub struct EntitiesIter<'a>(index_pool::iter::IndexIter<'a>);

impl Iterator for EntitiesIter<'_> {
    type Item = ArchEntityId;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.next()? as ArchEntityId)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
