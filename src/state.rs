use crate::private::ArchetypeMetadata;
use smallvec::SmallVec;
use std::alloc;
use std::any::{Any, TypeId};
use std::ops::Deref;

/// Defines archetype objects (entity states) with definite components.
pub trait ArchetypeState: Send + Sync + 'static {
    fn ty(&self) -> TypeId;
    fn as_ptr(&self) -> *const u8;
    fn forget(self);
    fn metadata(&self) -> fn() -> ArchetypeMetadata;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn num_components(&self) -> usize;

    fn component_ids(&self) -> SmallVec<[TypeId; 32]> {
        let meta = (self.metadata())();
        (meta.component_type_ids)()
    }
}

/// Defines archetype objects (entity states).
pub trait StaticArchetype: Sized + ArchetypeState {
    const N_COMPONENTS: usize;

    fn metadata() -> fn() -> ArchetypeMetadata;

    fn into_any(self) -> AnyState {
        AnyState(Box::new(self))
    }
}

pub struct AnyState(Box<dyn ArchetypeState>);

/// Entity state with arbitrary components.
impl AnyState {
    /// Returns `&dyn` reference to the contained state.
    pub fn downcast_ref<T: ArchetypeState>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref()
    }

    /// Returns `&mut dyn` reference to the contained state.
    pub fn downcast_mut<T: ArchetypeState>(&mut self) -> Option<&mut T> {
        self.0.as_any_mut().downcast_mut()
    }

    /// Returns the contained state.
    pub fn downcast<T: ArchetypeState>(self) -> Option<T> {
        if let Some(val) = self.downcast_ref::<T>() {
            let val = unsafe { (val as *const T).read() };
            self.forget();
            Some(val)
        } else {
            None
        }
    }
}

impl<T: StaticArchetype> From<T> for AnyState {
    fn from(state: T) -> Self {
        state.into_any()
    }
}

impl ArchetypeState for AnyState {
    fn ty(&self) -> TypeId {
        self.0.ty()
    }

    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    fn forget(self) {
        let val: &dyn ArchetypeState = self.0.deref();
        let layout = alloc::Layout::for_value(val);
        let ptr = Box::into_raw(self.0);

        // Deallocate `Box` without dropping the state itself.
        if layout.size() != 0 {
            unsafe {
                assert!(!ptr.is_null());
                alloc::dealloc(ptr as *mut u8, layout);
            };
        }
    }

    fn metadata(&self) -> fn() -> ArchetypeMetadata {
        self.0.metadata()
    }

    fn as_any(&self) -> &dyn Any {
        self.0.as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.0.as_any_mut()
    }

    fn num_components(&self) -> usize {
        self.0.num_components()
    }
}
