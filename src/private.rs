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
}

#[derive(Copy, Clone)]
pub struct ArchetypeMetadata {
    pub type_id: TypeId,
    pub component_type_ids: fn() -> SmallVec<[TypeId; MAX_INFOS_ON_STACK]>,
    pub component_infos: fn() -> SmallVec<[ComponentInfo; MAX_INFOS_ON_STACK]>,
    pub size: usize,
    pub needs_drop: bool,
    pub drop_fn: unsafe fn(*mut u8),
}

impl ArchetypeMetadata {
    pub fn component_infos(&self) -> SmallVec<[ComponentInfo; MAX_INFOS_ON_STACK]> {
        (self.component_infos)()
    }
}
