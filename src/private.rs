pub use memoffset::offset_of;
use std::any::TypeId;
use std::ops::Range;

#[derive(Clone)]
pub struct ComponentInfo {
    pub type_id: TypeId,
    pub range: Range<usize>,
    pub needs_drop: bool,
    pub drop_func: fn(*mut u8),
}
