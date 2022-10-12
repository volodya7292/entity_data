pub type ArchetypeId = u32;
pub type EntityId = u32;

/// An entity identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Entity {
    pub archetype_id: ArchetypeId,
    pub id: EntityId,
}

impl Entity {
    pub const NULL: Self = Entity {
        archetype_id: u32::MAX,
        id: u32::MAX,
    };

    /// Constructs a new entity identifier.
    pub fn new(archetype_id: u32, id: u32) -> Entity {
        Entity { archetype_id, id }
    }
}

impl Default for Entity {
    fn default() -> Self {
        Entity::NULL
    }
}
