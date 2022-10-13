use crate::EntityId;
use bitvec::vec::BitVec;

#[derive(Default)]
pub struct Entities {
    occupied_ids: BitVec,
}

const fn const_min(a: usize, b: usize) -> usize {
    [a, b][(a < b) as usize]
}

impl Entities {
    const MAX_BITVEC_BITS: usize = bitvec::slice::BitSlice::<usize, bitvec::order::Lsb0>::MAX_BITS;
    const MAX_SLOTS: usize = u32::MAX as usize - 1;

    pub const MAX_ENTITIES: usize = const_min(Self::MAX_SLOTS, Self::MAX_BITVEC_BITS);

    pub(crate) fn allocate_slot(&mut self) -> EntityId {
        #[cold]
        #[inline(never)]
        fn assert_failed() -> ! {
            panic!(
                "Out of slots. A maximum number of entities ({}) is reached.",
                Entities::MAX_ENTITIES
            );
        }

        let free_slot = self.occupied_ids.iter_zeros().next();

        if let Some(free_slot) = free_slot {
            self.occupied_ids.set(free_slot, true);
            free_slot as EntityId
        } else if self.occupied_ids.len() < Self::MAX_ENTITIES {
            self.occupied_ids.push(true);
            (self.occupied_ids.len() - 1) as EntityId
        } else {
            assert_failed();
        }
    }

    /// Returns `true` if the entity was present.
    pub(crate) fn free(&mut self, entity_id: EntityId) -> bool {
        if entity_id >= self.occupied_ids.len() as EntityId {
            return false;
        }
        self.occupied_ids.replace(entity_id as usize, false)
    }

    /// Returns `true` if the storage contains the specified entity.
    pub fn contains(&self, entity_id: EntityId) -> bool {
        self.occupied_ids
            .get(entity_id as usize)
            .map_or(false, |v| *v)
    }

    /// Returns an iterator over all available entities.
    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.occupied_ids.iter_ones().map(|id| id as EntityId)
    }

    /// Returns the number of available entities.
    pub fn count(&self) -> usize {
        self.occupied_ids.count_ones() as usize
    }
}
