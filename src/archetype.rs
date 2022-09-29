use crate::private::ComponentInfo;
use crate::{private, HashMap};
use bit_set::BitSet;
use smallvec::SmallVec;
use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::{mem, ptr, slice};

#[derive(Clone, Eq)]
pub(crate) struct ArchetypeLayout {
    sorted_type_ids: Vec<TypeId>,
    hash_val: u64,
}

impl ArchetypeLayout {
    pub fn new(mut type_ids: Vec<TypeId>) -> ArchetypeLayout {
        type_ids.sort_unstable();

        let mut hasher = ahash::AHasher::default();
        type_ids.hash(&mut hasher);
        let hash_val = hasher.finish();

        ArchetypeLayout {
            sorted_type_ids: type_ids,
            hash_val,
        }
    }
}

impl PartialEq for ArchetypeLayout {
    fn eq(&self, other: &Self) -> bool {
        self.sorted_type_ids == other.sorted_type_ids
    }
}

impl Hash for ArchetypeLayout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_val.hash(state);
    }
}

/// A collection of entities with unique combination of components.
/// An archetype can hold a maximum of 2^32-1 entities.
pub struct Archetype {
    pub(crate) components: Vec<(ComponentInfo, Vec<u8>)>,
    pub(crate) components_by_types: HashMap<TypeId, usize>,
    pub(crate) free_slots: BitSet,
    pub(crate) total_slot_count: u32,
    pub(crate) components_need_drops: bool,
}

impl Archetype {
    pub const MAX_ENTITIES: u32 = u32::MAX - 1;

    pub(crate) fn new(comp_infos: &[ComponentInfo]) -> Self {
        let components: Vec<_> = comp_infos
            .iter()
            .map(|info| (info.clone(), vec![]))
            .collect();

        let components_by_types: HashMap<_, _> = comp_infos
            .iter()
            .enumerate()
            .map(|(i, info)| (info.type_id, i))
            .collect();

        let components_need_drops = comp_infos.iter().any(|info| info.needs_drop);

        Archetype {
            components,
            components_by_types,
            free_slots: Default::default(),
            total_slot_count: 0,
            components_need_drops,
        }
    }

    fn allocate_slot(&mut self) -> u32 {
        #[cold]
        #[inline(never)]
        fn assert_failed() -> ! {
            panic!("Archetype: out of slots. A maximum number of entities (2^32-1) is reached.");
        }

        if let Some(free_slot) = self.free_slots.iter().next() {
            self.free_slots.remove(free_slot);
            free_slot as u32
        } else if self.total_slot_count < Self::MAX_ENTITIES {
            self.total_slot_count += 1;
            self.total_slot_count - 1
        } else {
            assert_failed();
        }
    }

    /// Safety: `S` must be of the same component layout as the archetype.
    pub(crate) unsafe fn add_entity_raw(&mut self, state_ptr: *const u8) -> u32 {
        let entity_id = self.allocate_slot();

        for (info, storage) in &mut self.components {
            let component_data = state_ptr.add(info.range.start);
            let comp_size = info.range.len();

            if entity_id == (self.total_slot_count - 1) {
                let slice = slice::from_raw_parts(component_data, comp_size);
                storage.extend(slice);
            } else {
                let dst_ptr = storage.as_mut_ptr().add(entity_id as usize * comp_size);
                ptr::copy_nonoverlapping(component_data, dst_ptr, comp_size);
            }
        }

        entity_id
    }

    pub fn is_present(&self, entity_id: u32) -> bool {
        entity_id < self.total_slot_count && !self.free_slots.contains(entity_id as usize)
    }

    /// Returns a reference to the component `C` of the specified entity id.
    pub fn get<C: 'static>(&self, entity_id: u32) -> Option<&C> {
        if !self.is_present(entity_id) {
            return None;
        }
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let (_, data) = self.components.get(id)?;
        unsafe {
            let ptr = (data.as_ptr() as *const C).offset(entity_id as isize);
            Some(&*ptr)
        }
    }

    /// Returns a mutable reference to the component `C` of the specified entity id.
    pub fn get_mut<C: 'static>(&mut self, entity_id: u32) -> Option<&mut C> {
        if !self.is_present(entity_id) {
            return None;
        }
        let id = *self.components_by_types.get(&TypeId::of::<C>())?;
        let (_, data) = self.components.get_mut(id)?;
        unsafe {
            let ptr = (data.as_mut_ptr() as *mut C).offset(entity_id as isize);
            Some(&mut *ptr)
        }
    }

    /// Removes an entity from the archetype. Returns `true` if the entity was present in the archetype.
    pub fn remove(&mut self, entity_id: u32) -> bool {
        let mut is_present = entity_id < self.total_slot_count;

        is_present &= !self.free_slots.insert(entity_id as usize);

        if is_present && self.components_need_drops {
            let id = entity_id as usize;
            for (type_info, data) in &mut self.components {
                if type_info.needs_drop {
                    unsafe {
                        let ptr = data.as_mut_ptr().add(id * type_info.range.len());
                        (type_info.drop_func)(ptr);
                    }
                }
            }
        }

        is_present
    }

    /// Returns the number of entities in the archetype.
    pub fn len(&self) -> usize {
        self.total_slot_count as usize - self.free_slots.len()
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        if !self.components_need_drops {
            return;
        }
        for (type_info, data) in &mut self.components {
            if !type_info.needs_drop {
                continue;
            }
            for id in 0..self.total_slot_count {
                if !self.free_slots.contains(id as usize) {
                    let ptr = unsafe { data.as_mut_ptr().add(id as usize * type_info.range.len()) };
                    (type_info.drop_func)(ptr);
                }
            }
        }
    }
}

/// Defines archetype objects (entity states).
pub trait StaticArchetype: 'static {
    const N_COMPONENTS: usize;
}

/// Defines archetype objects (entity states) with definite components.
pub trait ArchetypeState: Sized + Clone + 'static {
    fn ty(&self) -> TypeId;
    fn as_ptr(&self) -> *const u8;
    fn forget(self);
    fn component_type_ids(&self) -> SmallVec<[TypeId; private::MAX_INFOS_ON_STACK]>;
    fn component_infos(&self) -> SmallVec<[ComponentInfo; private::MAX_INFOS_ON_STACK]>;

    fn into_any(self) -> AnyState
    where
        Self: 'static,
    {
        let ty = self.type_id();
        let size = mem::size_of_val(&self);
        let mut data = Vec::<u8>::with_capacity(size);

        let clone_func = |p: *const u8| {
            let size = mem::size_of::<Self>();
            let mut data = Vec::<u8>::with_capacity(size);
            unsafe {
                let clone = Self::clone(&*(p as *const Self));
                ptr::write(data.as_mut_ptr() as *mut Self, clone);
                data.set_len(size);
            }
            data
        };
        let drop_func = |p: *mut u8| unsafe { ptr::drop_in_place(p as *mut Self) };

        let component_type_ids = self.component_type_ids().to_vec();
        let component_infos = self.component_infos().to_vec();

        unsafe {
            ptr::write(data.as_mut_ptr() as *mut Self, self);
            data.set_len(size);
        }

        AnyState {
            ty,
            data,
            component_type_ids,
            component_infos,
            needs_drop: mem::needs_drop::<Self>(),
            clone_func,
            drop_func,
        }
    }
}

/// Entity state with arbitrary components.
pub struct AnyState {
    pub(crate) ty: TypeId,
    pub(crate) data: Vec<u8>,
    pub(crate) component_type_ids: Vec<TypeId>,
    pub(crate) component_infos: Vec<ComponentInfo>,
    pub(crate) needs_drop: bool,
    pub(crate) clone_func: fn(*const u8) -> Vec<u8>,
    pub(crate) drop_func: fn(*mut u8),
}

impl AnyState {
    pub fn into_definite<S: StaticArchetype>(mut self) -> Option<S> {
        if TypeId::of::<S>() != self.ty {
            return None;
        }
        self.needs_drop = false;

        let mut result = MaybeUninit::<S>::uninit();

        unsafe {
            let src_ptr = self.data.as_ptr() as *const S;
            src_ptr.copy_to_nonoverlapping(result.as_mut_ptr(), self.data.len());
            Some(result.assume_init())
        }
    }
}

impl<T: StaticArchetype + ArchetypeState> From<T> for AnyState {
    fn from(state: T) -> Self {
        state.into_any()
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
        self.needs_drop = false;
    }

    fn component_type_ids(&self) -> SmallVec<[TypeId; private::MAX_INFOS_ON_STACK]> {
        SmallVec::from_vec(self.component_type_ids.clone())
    }

    fn component_infos(&self) -> SmallVec<[ComponentInfo; private::MAX_INFOS_ON_STACK]> {
        SmallVec::from_vec(self.component_infos.clone())
    }

    fn into_any(self) -> AnyState {
        panic!("Cannot cast dynamic state.");
    }
}

impl Clone for AnyState {
    fn clone(&self) -> Self {
        Self {
            ty: self.ty,
            data: (self.clone_func)(self.data.as_ptr()),
            component_type_ids: self.component_type_ids.clone(),
            component_infos: self.component_infos.clone(),
            needs_drop: self.needs_drop,
            clone_func: self.clone_func,
            drop_func: self.drop_func,
        }
    }
}

impl Drop for AnyState {
    fn drop(&mut self) {
        if self.needs_drop {
            (self.drop_func)(self.data.as_mut_ptr());
        }
    }
}
