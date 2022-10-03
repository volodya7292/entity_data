pub use memoffset::offset_of;
pub use smallvec::smallvec;
pub use smallvec::SmallVec;
use std::any::TypeId;
use std::ops::Range;

pub const MAX_INFOS_ON_STACK: usize = 32;

#[derive(Clone)]
pub struct ComponentInfo {
    pub type_id: TypeId,
    pub range: Range<usize>,
    pub needs_drop: bool,
    pub drop_func: fn(*mut u8),
}

#[derive(Copy, Clone)]
pub struct ArchetypeMetadata {
    pub component_type_ids: fn() -> SmallVec<[TypeId; MAX_INFOS_ON_STACK]>,
    pub component_infos: fn() -> SmallVec<[ComponentInfo; MAX_INFOS_ON_STACK]>,
    pub needs_drop: bool,
    pub clone_func: fn(*const u8) -> Vec<u8>,
    pub drop_func: fn(*mut u8),
}
