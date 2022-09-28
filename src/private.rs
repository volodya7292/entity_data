pub use memoffset::offset_of;
use std::any::TypeId;
use std::ops::Range;

pub struct ComponentInfo {
    pub type_id: TypeId,
    pub range: Range<usize>,
    pub needs_drop: bool,
    pub drop_func: fn(*mut u8),
}

pub trait IsArchetype {}

pub trait ArchetypeImpl<const N: usize>: IsArchetype {
    fn component_type_ids() -> [TypeId; N];
    fn component_infos() -> [ComponentInfo; N];
}
