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
