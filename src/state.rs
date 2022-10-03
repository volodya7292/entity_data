use crate::private::ArchetypeMetadata;
use std::any::{Any, TypeId};
use std::mem::MaybeUninit;
use std::{mem, ptr};

/// Defines archetype objects (entity states) with definite components.
pub trait ArchetypeState: Send + Sync + 'static {
    fn ty(&self) -> TypeId;
    fn as_ptr(&self) -> *const u8;
    fn forget(self);
    fn metadata(&self) -> fn() -> ArchetypeMetadata;
}

/// Defines archetype objects (entity states).
pub trait StaticArchetype: ArchetypeState {
    const N_COMPONENTS: usize;

    fn into_any(self) -> AnyState
    where
        Self: Sized + 'static,
    {
        let ty = self.type_id();
        let size = mem::size_of::<Self>();
        let metadata = self.metadata();

        let mut data = Vec::<u8>::with_capacity(size);
        unsafe {
            ptr::write(data.as_mut_ptr() as *mut Self, self);
            data.set_len(size);
        }

        AnyState { ty, data, metadata }
    }
}

/// Entity state with arbitrary components.
pub struct AnyState {
    pub(crate) ty: TypeId,
    pub(crate) data: Vec<u8>,
    pub(crate) metadata: fn() -> ArchetypeMetadata,
}

impl AnyState {
    pub fn into_static<S: StaticArchetype>(mut self) -> Option<S> {
        if TypeId::of::<S>() != self.ty {
            return None;
        }

        let mut result = MaybeUninit::<S>::uninit();

        unsafe {
            let src_ptr = self.data.as_ptr() as *const S;
            src_ptr.copy_to_nonoverlapping(result.as_mut_ptr(), mem::size_of::<S>());

            (&mut self.data as *mut Vec<_>).drop_in_place();
            mem::forget(self);

            Some(result.assume_init())
        }
    }
}

impl ArchetypeState for AnyState {
    fn ty(&self) -> TypeId {
        self.ty
    }

    fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    fn forget(mut self) {
        unsafe { (&mut self.data as *mut Vec<_>).drop_in_place() };
        mem::forget(self);
    }

    fn metadata(&self) -> fn() -> ArchetypeMetadata {
        self.metadata
    }
}

impl<T: StaticArchetype> From<T> for AnyState {
    fn from(state: T) -> Self {
        state.into_any()
    }
}

impl Clone for AnyState {
    fn clone(&self) -> Self {
        let data = (self.metadata()().clone_func)(self.data.as_ptr());
        Self {
            ty: self.ty,
            data,
            metadata: self.metadata,
        }
    }
}

impl Drop for AnyState {
    fn drop(&mut self) {
        (self.metadata()().drop_func)(self.data.as_mut_ptr());
    }
}
