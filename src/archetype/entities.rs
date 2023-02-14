use crate::entity::ArchEntityId;
use bitvec::vec::BitVec;

#[derive(Default)]
pub struct ArchetypeEntities {
    occupied_ids: BitVec,
}

const fn const_min(a: usize, b: usize) -> usize {
    [a, b][(a < b) as usize]
}

impl<'a> ArchetypeEntities {
    const MAX_BITVEC_BITS: usize = bitvec::slice::BitSlice::<usize, bitvec::order::Lsb0>::MAX_BITS;
    const MAX_SLOTS: usize = u32::MAX as usize - 1;

    pub const MAX_ENTITIES: usize = const_min(Self::MAX_SLOTS, Self::MAX_BITVEC_BITS);

    pub(crate) fn allocate_slot(&mut self) -> ArchEntityId {
        #[cold]
        #[inline(never)]
        fn assert_failed() -> ! {
            panic!(
                "Out of slots. A maximum number of entities ({}) is reached.",
                ArchetypeEntities::MAX_ENTITIES
            );
        }

        let free_slot = self.occupied_ids.iter_zeros().next();

        if let Some(free_slot) = free_slot {
            self.occupied_ids.set(free_slot, true);
            free_slot as ArchEntityId
        } else if self.occupied_ids.len() < Self::MAX_ENTITIES {
            self.occupied_ids.push(true);
            (self.occupied_ids.len() - 1) as ArchEntityId
        } else {
            assert_failed();
        }
    }

    /// Returns `true` if the entity was present.
    pub(crate) fn free(&mut self, entity_id: ArchEntityId) -> bool {
        if entity_id >= self.occupied_ids.len() as ArchEntityId {
            return false;
        }
        self.occupied_ids.replace(entity_id as usize, false)
    }

    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: ArchEntityId) -> bool {
        self.occupied_ids
            .get(entity_id as usize)
            .map_or(false, |v| *v)
    }

    /// Returns an iterator over all entities of the archetype.
    pub fn iter(&'a self) -> EntitiesIter {
        EntitiesIter(self.occupied_ids.iter_ones())
    }

    /// Returns the number of entities in the archetype.
    pub fn count(&self) -> usize {
        self.occupied_ids.count_ones() as usize
    }
}

#[derive(Copy, Clone)]
pub struct EntitiesIter<'a>(bitvec::slice::IterOnes<'a, usize, bitvec::order::Lsb0>);

impl Iterator for EntitiesIter<'_> {
    type Item = ArchEntityId;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.next()? as ArchEntityId)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
