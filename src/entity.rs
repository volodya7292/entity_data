/// An archetype identifier.
pub type ArchetypeId = u32;
/// An entity identifier within an archetype.
pub type ArchEntityId = u32;

/// An entity identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct EntityId {
    pub archetype_id: ArchetypeId,
    pub id: ArchEntityId,
}

impl EntityId {
    pub const NULL: Self = EntityId {
        archetype_id: u32::MAX,
        id: u32::MAX,
    };

    /// Constructs a new entity identifier.
    pub fn new(archetype_id: ArchetypeId, id: ArchEntityId) -> EntityId {
        EntityId { archetype_id, id }
    }
}

impl Default for EntityId {
    fn default() -> Self {
        EntityId::NULL
    }
}
