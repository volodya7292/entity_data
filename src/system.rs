pub(crate) mod component;

use crate::entity::ArchetypeId;
use crate::entity_storage::AllEntities;
use crate::system::component::{CompMutability, ComponentGlobalIterInner, ComponentGlobalIterMutInner, GenericComponentGlobalAccess, GlobalComponentAccess, OwningRef};
use crate::{
    archetype, ArchetypeStorage, Component, EntityId, EntityStorage, GlobalComponentIter,
    GlobalComponentIterMut, HashMap, HashSet,
};
use std::any::TypeId;
use std::cell::{Ref, RefCell, RefMut, UnsafeCell};
use std::collections::hash_map;
use std::pin::Pin;
use std::rc::Rc;
use std::vec;

pub trait SystemHandler: Send + Sync {
    fn run(&mut self, data: SystemAccess);
}

impl<F: FnMut(SystemAccess) + Send + Sync> SystemHandler for F {
    fn run(&mut self, data: SystemAccess) {
        self(data);
    }
}

/// A system context.
pub struct System<'a> {
    handler: Box<&'a mut (dyn SystemHandler)>,
    components: HashMap<TypeId, CompMutability>,
}

impl<'a> System<'a> {
    /// Creates a system with data handler.
    pub fn new(handler: &'a mut impl SystemHandler) -> Self {
        Self {
            handler: Box::new(handler),
            components: Default::default(),
        }
    }

    /// Makes component accessible from the system.
    pub fn with<C: Component>(mut self) -> Self {
        self.components.insert(TypeId::of::<C>(), false);
        self
    }

    /// Makes component mutably accessible from the system.
    pub fn with_mut<C: Component>(mut self) -> Self {
        self.components.insert(TypeId::of::<C>(), true);
        self
    }
}

/// Describes binary component intersection with or without particular components.
#[derive(Clone, Eq, PartialEq)]
pub struct BCompSet {
    components: HashMap<TypeId, CompMutability>,
}

impl BCompSet {
    /// Creates a new binary component intersection descriptor.
    pub fn new() -> Self {
        Self {
            components: HashMap::with_capacity(32),
        }
    }

    /// Includes component `C` in the set.
    pub fn with<C: Component>(mut self) -> Self {
        self.components.insert(TypeId::of::<C>(), false);
        self
    }

    /// Includes component `C` in the set.
    pub fn with_mut<C: Component>(mut self) -> Self {
        self.components.insert(TypeId::of::<C>(), true);
        self
    }
}

pub struct ComponentSetAccess<'a, 'b> {
    _component_guards: Vec<Ref<'b, GenericComponentGlobalAccess<'a>>>,
    _component_mut_guards: Vec<RefMut<'b, GenericComponentGlobalAccess<'a>>>,
    components: HashMap<TypeId, RefCell<GenericComponentGlobalAccess<'a>>>,
    filtered_archetype_ids: Vec<usize>,
    system_access: &'b SystemAccess<'a>,
}

impl<'a, 'c> ComponentSetAccess<'a, 'c> {
    pub fn access(&self) -> &SystemAccess<'a> {
        self.system_access
    }

    pub fn iter<'b, C: Component>(&'b self) -> GlobalComponentIter<'a, 'b, C> {
        let generic = self
            .components
            .get(&TypeId::of::<C>())
            .expect("Component not found");

        let guard = generic.try_borrow().expect("Component is mutably borrowed");

        GlobalComponentIter {
            inner: unsafe {
                OwningRef::new(guard, |generic| ComponentGlobalIterInner::new(generic))
            },
        }
    }

    pub fn iter_mut<'b, C: Component>(
        &'b self,
    ) -> GlobalComponentIterMut<'a, 'b, 'b, RefMut<'b, GenericComponentGlobalAccess<'a>>, C> {
        let generic = self
            .components
            .get(&TypeId::of::<C>())
            .expect("Component not found");

        let guard =
            RefCell::try_borrow_mut(generic).expect("Component is already mutably borrowed");

        if !guard.mutable {
            panic!("Component is not allowed to be mutated");
        }

        GlobalComponentIterMut {
            inner: unsafe {
                OwningRef::new(guard, |generic| ComponentGlobalIterMutInner::new(generic))
            },
            _l: Default::default(),
        }
    }

    pub fn into_entities_iter(self) -> FilteredEntitiesIter<'a> {
        let mut filtered_archetype_ids = self.filtered_archetype_ids.into_iter();
        let curr_arch_id = filtered_archetype_ids.next().unwrap();

        FilteredEntitiesIter {
            filtered_archetype_ids,
            all_archetypes: self.system_access.all_archetypes,
            curr_arch_id,
            curr_iter: self.system_access.all_archetypes[curr_arch_id]
                .entities
                .iter(),
        }
    }
}

pub struct FilteredEntitiesIter<'a> {
    filtered_archetype_ids: vec::IntoIter<usize>,
    all_archetypes: &'a [ArchetypeStorage],
    curr_arch_id: usize,
    curr_iter: archetype::entities::EntitiesIter<'a>,
}

impl<'a> Iterator for FilteredEntitiesIter<'a> {
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.curr_iter.next();

            if let Some(arch_entity_id) = id {
                let entity_id = EntityId::new(self.curr_arch_id as ArchetypeId, arch_entity_id);
                return Some(entity_id);
            }

            let next_arch_id = self.filtered_archetype_ids.next()?;

            self.curr_iter = self.all_archetypes[next_arch_id].entities.iter();
        }
    }
}

/// Represents all available components to a system.
pub struct SystemAccess<'a> {
    entities: AllEntities<'a>,
    component_to_archetypes_map: &'a HashMap<TypeId, Vec<usize>>,
    all_archetypes: &'a [ArchetypeStorage],
    allowed_all_components: bool,
    global_components:
        UnsafeCell<HashMap<TypeId, Pin<Rc<RefCell<GenericComponentGlobalAccess<'a>>>>>>,
}

impl<'a> SystemAccess<'a> {
    fn get_component(
        &self,
        ty: TypeId,
    ) -> Option<&Pin<Rc<RefCell<GenericComponentGlobalAccess<'a>>>>> {
        let global_components = unsafe { &mut *self.global_components.get() };

        match global_components.entry(ty) {
            hash_map::Entry::Occupied(e) => Some(e.into_mut()),
            hash_map::Entry::Vacant(e) => {
                if !self.allowed_all_components {
                    return None;
                }

                // Modifying the hashmap is safe because referenced values are wrapped in Pin<Box<>>.
                let new = RefCell::new(GenericComponentGlobalAccess {
                    filtered_archetype_ids: self
                        .component_to_archetypes_map
                        .get(&ty)
                        .unwrap_or(&vec![])
                        .clone(),
                    all_archetypes: self.all_archetypes,
                    all_entities: self.entities,
                    // Safety: true is allowed here because there's nothing to modify.
                    mutable: true,
                });

                Some(e.insert(Rc::pin(new)))
            }
        }
    }

    fn archetype_set_for(&self, set: &BCompSet) -> HashSet<ArchetypeId> {
        let mut result = HashSet::<ArchetypeId>::with_capacity(64);
        let mut initialized = false;

        // Find archetypes with intersecting components from `set`
        for (comp_ty, _) in &set.components {
            let comp = self.get_component(*comp_ty);

            if comp.is_none() {
                panic!("Component not available")
            }

            let next_archetype_ids: HashSet<_> =
                if let Some(archetypes) = self.component_to_archetypes_map.get(comp_ty) {
                    archetypes.iter().map(|v| *v as ArchetypeId).collect()
                } else {
                    return HashSet::new();
                };

            if initialized {
                result.retain(|v| next_archetype_ids.contains(v));
            } else {
                result = next_archetype_ids;
                initialized = true;
            }
        }

        result
    }

    /// Borrows the component.
    /// Panics if the component is mutably borrowed or not available to this system.
    pub fn component<'b, C: Component>(
        &'b self,
    ) -> GlobalComponentAccess<C, Ref<'b, GenericComponentGlobalAccess<'a>>, &()> {
        let ty = TypeId::of::<C>();

        // This is safe because the mutable reference gets dropped afterwards.
        let generic = self.get_component(ty).expect("Component not available");

        GlobalComponentAccess {
            generic: generic.try_borrow().expect("Component is mutably borrowed"),
            _ty: Default::default(),
            _mutability: Default::default(),
        }
    }

    /// Mutably borrows the component.
    /// Panics if the component is already borrowed or not available to this system.
    pub fn component_mut<'b, C: Component>(
        &'b self,
    ) -> GlobalComponentAccess<C, RefMut<'b, GenericComponentGlobalAccess<'a>>, &'static ()> {
        let ty = TypeId::of::<C>();

        let generic = self.get_component(ty).expect("Component not available");

        let guard = generic
            .try_borrow_mut()
            .expect("Component is already borrowed");

        if !guard.mutable {
            panic!("Component is not allowed to be mutated");
        }

        GlobalComponentAccess {
            generic: guard,
            _ty: Default::default(),
            _mutability: Default::default(),
        }
    }

    /// Provides access to entities that contain specific components.
    /// Panics if any of specified components are already borrowed or not available to this system.
    pub fn component_set(&mut self, set: &BCompSet) -> ComponentSetAccess<'a, '_> {
        let filtered_archetype_ids: Vec<usize> = self
            .archetype_set_for(&set)
            .into_iter()
            .map(|v| v as usize)
            .collect();

        let mut components = HashMap::with_capacity(set.components.len());
        let mut _component_guards = Vec::with_capacity(set.components.len());
        let mut _component_mut_guards = Vec::with_capacity(set.components.len());

        for (ty, &mutable) in &set.components {
            let comp = self.get_component(*ty).unwrap();

            if mutable && !RefCell::borrow(comp).mutable {
                panic!("Component is not allowed to be mutated");
            }

            components.insert(
                *ty,
                RefCell::new(GenericComponentGlobalAccess {
                    filtered_archetype_ids: filtered_archetype_ids.clone(),
                    all_archetypes: self.all_archetypes,
                    all_entities: self.entities,
                    // Safety: true is allowed here because there's nothing to modify.
                    mutable,
                }),
            );

            if mutable {
                _component_mut_guards
                    .push(RefCell::try_borrow_mut(comp).expect("Component is already borrowed"));
            } else {
                _component_guards
                    .push(RefCell::try_borrow(comp).expect("Component is mutably borrowed"));
            }
        }

        ComponentSetAccess {
            _component_guards,
            _component_mut_guards,
            components,
            filtered_archetype_ids,
            system_access: self,
        }
    }
}

#[cfg(feature = "rayon")]
mod parallel {
    use crate::system::component::CompMutability;
    use crate::{HashMap, System};
    use std::any::TypeId;
    use std::collections::hash_map;
    use std::mem;

    #[derive(Debug)]
    pub struct ParallelSystems {
        pub systems: Vec<usize>,
        pub all_components: HashMap<TypeId, CompMutability>,
    }

    impl ParallelSystems {
        fn take(&mut self) -> Self {
            Self {
                systems: mem::replace(&mut self.systems, vec![]),
                all_components: mem::replace(&mut self.all_components, Default::default()),
            }
        }

        fn append(&mut self, other: Self) {
            self.systems.extend(other.systems);

            self.all_components.reserve(other.all_components.len());

            for (ty, b_mutable) in &other.all_components {
                match self.all_components.entry(*ty) {
                    hash_map::Entry::Occupied(mut e) => {
                        let a_mutable = e.get_mut();
                        if !*a_mutable {
                            e.insert(*b_mutable);
                        }
                    }
                    hash_map::Entry::Vacant(e) => {
                        e.insert(*b_mutable);
                    }
                }
            }
        }
    }

    pub fn systems_do_conflict(
        a_components: &HashMap<TypeId, CompMutability>,
        b_components: &HashMap<TypeId, CompMutability>,
    ) -> bool {
        a_components.iter().any(|(ty, mutable_a)| {
            b_components
                .get(ty)
                .map_or(false, |mutable_b| *mutable_a || *mutable_b)
        })
    }

    /// Partitions systems in parallel in such a way as to maximally utilize CPU.
    pub fn partition_parallel_systems(systems: &[System]) -> Vec<ParallelSystems> {
        // Component conflict resolution example:
        // Components (*) in rows are mutated concurrently.
        //
        // Initial state:
        //
        //      C0 C1 C2 C3 C4
        //  S0  -  *  -  -  -
        //  S1  -  -  *  *  -
        //  S2  -  *  -  -  *
        //  S3  *  -  -  *  -
        //  S4  *  *  -  -  *
        //
        //  potential permutations:
        //  (src) S0   S1   S2   S3   S4  \/
        // ------------------------------
        //  (dst) S1   S0   S1   S0   S1
        //        S3   S2   S3   S2
        //             S4
        //
        // 1. First step result:
        //
        //      C0 C1 C2 C3 C4
        //  S0       -  *  -  -  -
        //  (S1,S4)  *  *  *  *  *
        //  S2       -  *  -  -  *
        //  S3       *  -  -  *  -
        //
        //  potential permutations:
        //  S0   S1   S2   S3   S4
        // ------------------------
        //  S3        S3   S0
        //                 S2
        //
        // 2. Second step result (conflict resolution complete):
        //
        //      C0 C1 C2 C3 C4
        //  (S1,S4)  *  *  *  *  *
        //  S2       -  *  -  -  *
        //  (S3,S0)  *  *  -  *  -
        //
        //  no potential permutations left:
        //  S1   S2   S3   S4   S5
        // ------------------------

        fn extract_potential_moves(systems: &[ParallelSystems], moves: &mut [Vec<usize>]) {
            for ((i, sys), moves) in systems.iter().enumerate().zip(moves) {
                if sys.systems.is_empty() {
                    continue;
                }

                for (j, sys2) in systems.iter().enumerate() {
                    if j == i || sys2.systems.is_empty() {
                        continue;
                    }

                    let conflicting =
                        systems_do_conflict(&sys.all_components, &sys2.all_components);

                    if !conflicting {
                        moves.push(j);
                    }
                }
            }
        }

        let mut parallel_runs: Vec<_> = systems
            .iter()
            .enumerate()
            .map(|(i, sys)| ParallelSystems {
                systems: vec![i],
                all_components: sys.components.clone(),
            })
            .collect();

        let mut potential_moves = vec![Vec::<usize>::with_capacity(systems.len()); systems.len()];

        loop {
            for v in &mut potential_moves {
                v.clear();
            }
            extract_potential_moves(&parallel_runs, &mut potential_moves);

            if potential_moves.iter().all(|v| v.is_empty()) {
                break;
            }

            let (min_i, min_moves) = potential_moves
                .iter_mut()
                .enumerate()
                .filter(|(_, v)| !v.is_empty())
                .min_by_key(|(_, v)| v.len())
                .unwrap();

            let mv_from = min_i;
            let mv_to = min_moves.pop().unwrap();

            let mv_systems = parallel_runs[mv_from].take();
            parallel_runs[mv_to].append(mv_systems);
        }

        parallel_runs.retain(|v| !v.systems.is_empty());

        parallel_runs
    }
}

impl EntityStorage {
    /// Safety: mutable borrows must be unique.
    pub(crate) unsafe fn global_component_by_id(
        &self,
        ty: TypeId,
        mutable: bool,
    ) -> GenericComponentGlobalAccess {
        let filtered_archetype_ids: Vec<usize> = self
            .component_to_archetypes_map
            .get(&ty)
            .map_or(vec![], |v| v.clone());

        GenericComponentGlobalAccess {
            filtered_archetype_ids,
            all_archetypes: &self.archetypes,
            all_entities: self.entities(),
            mutable,
        }
    }

    /// Safety: the same component aren't allowed to be mutated on different threads simultaneously.
    unsafe fn get_system_data(&self, components: &HashMap<TypeId, CompMutability>) -> SystemAccess {
        let global_components = components
            .iter()
            .map(|(&ty, mutable)| {
                (
                    ty,
                    Rc::pin(RefCell::new(self.global_component_by_id(ty, *mutable))),
                )
            })
            .collect();

        SystemAccess {
            entities: self.entities(),
            component_to_archetypes_map: &self.component_to_archetypes_map,
            all_archetypes: &self.archetypes,
            allowed_all_components: false,
            global_components: UnsafeCell::new(global_components),
        }
    }

    /// Provides access to all components. Allows simultaneous mutable access to multiple components.
    pub fn access(&mut self) -> SystemAccess {
        SystemAccess {
            entities: self.entities(),
            component_to_archetypes_map: &self.component_to_archetypes_map,
            all_archetypes: &self.archetypes,
            allowed_all_components: true,
            global_components: UnsafeCell::new(HashMap::with_capacity(
                self.component_to_archetypes_map.len(),
            )),
        }
    }

    /// Dispatches systems sequentially. For parallel execution,
    /// see [dispatch_par](Self::dispatch_par) (requires `rayon` feature).
    ///
    /// # Example
    /// ```
    /// use entity_data::{EntityId, EntityStorage, System, SystemHandler};
    /// use entity_data::system::SystemAccess;
    /// use macros::Archetype;
    ///
    /// #[derive(Default, Debug)]
    /// struct Position {
    ///     x: f32,
    ///     y: f32,
    /// }
    ///
    /// #[derive(Archetype)]
    /// struct Dog {
    ///     pos: Position,
    /// }
    ///
    /// let mut storage = EntityStorage::new();
    /// let dog0 = storage.add(Dog { pos: Default::default() });
    /// let dog1 = storage.add(Dog { pos: Position { x: 3.0, y: 5.0 } });
    ///
    /// struct PositionsPrintSystem {}
    ///
    /// impl SystemHandler for PositionsPrintSystem {
    ///     fn run(&mut self, data: SystemAccess) {
    ///         let positions = data.component::<Position>();
    ///         for pos in positions {
    ///             println!("{:?}", pos);
    ///         }
    ///     }
    /// }
    ///
    /// let mut sys = PositionsPrintSystem {};
    /// storage.dispatch(&mut [System::new(&mut sys).with::<Position>()]);
    /// ```
    pub fn dispatch(&self, systems: &mut [System]) {
        for sys in systems {
            let data = unsafe { self.get_system_data(&sys.components) };
            sys.handler.run(data);
        }
    }

    /// Dispatches systems in parallel if possible. Two systems won't execute in parallel if they
    /// access the same component and one of them mutates this component.
    #[cfg(feature = "rayon")]
    pub fn dispatch_par(&self, systems: &mut [System]) {
        if systems.is_empty() {
            return;
        }

        let parallel_runs = parallel::partition_parallel_systems(systems);

        rayon::scope(|s| {
            for mut run in parallel_runs {
                for sys_i in &mut run.systems {
                    let system = &systems[*sys_i];

                    // The cast from *const to *mut is safe because the slice itself is &mut.
                    let system_mut: &mut System = unsafe { &mut *(system as *const _ as *mut _) };

                    s.spawn(|_| {
                        let data = unsafe { self.get_system_data(&system.components) };
                        system_mut.handler.run(data);
                    });
                }
            }
        });
    }
}

#[cfg(feature = "rayon")]
#[test]
fn test_optimization() {
    #[derive(Copy, Clone)]
    struct TestSystem {}

    impl SystemHandler for TestSystem {
        fn run(&mut self, _: SystemAccess) {}
    }

    // Initial:
    //      C0 C1 C2 C3 C4
    //  S0  -  *  -  -  -
    //  S1  -  -  *  *  -
    //  S2  -  *  -  -  *
    //  S3  *  -  -  *  -
    //  S4  *  *  -  -  *

    // Result:
    //           C0 C1 C2 C3 C4
    //  (S1,S4)  *  *  *  *  *
    //  S2       -  *  -  -  *
    //  (S3,S0)  *  *  -  *  -

    let mut test_sys0 = TestSystem {};
    let mut test_sys1 = TestSystem {};
    let mut test_sys2 = TestSystem {};
    let mut test_sys3 = TestSystem {};
    let mut test_sys4 = TestSystem {};

    let sys0 = System::new(&mut test_sys0).with_mut::<i16>();
    let sys1 = System::new(&mut test_sys1)
        .with_mut::<i32>()
        .with_mut::<i64>();
    let sys2 = System::new(&mut test_sys2)
        .with_mut::<i16>()
        .with_mut::<u64>();
    let sys3 = System::new(&mut test_sys3)
        .with_mut::<i8>()
        .with_mut::<i64>();
    let sys4 = System::new(&mut test_sys4)
        .with_mut::<i8>()
        .with_mut::<i16>()
        .with_mut::<u64>();

    let mut systems = [sys0, sys1, sys2, sys3, sys4];
    let parallel_runs = parallel::partition_parallel_systems(&mut systems);

    assert_eq!(systems.len(), 5);
    assert_eq!(parallel_runs.len(), 3);

    assert_eq!(
        &parallel_runs[0].systems.iter().cloned().collect::<Vec<_>>(),
        &[1, 4]
    );
    assert_eq!(
        &parallel_runs[1].systems.iter().cloned().collect::<Vec<_>>(),
        &[2]
    );
    assert_eq!(
        &parallel_runs[2].systems.iter().cloned().collect::<Vec<_>>(),
        &[3, 0]
    );

    for run in &parallel_runs {
        let conflicting = run.systems.iter().enumerate().any(|(i, sys0_id)| {
            run.systems.iter().enumerate().any(|(j, sys1_id)| {
                if i == j {
                    return false;
                }
                parallel::systems_do_conflict(
                    &systems[*sys0_id].components,
                    &systems[*sys1_id].components,
                )
            })
        });

        assert_eq!(conflicting, false);
    }
}

#[test]
fn test_system_data_access() {
    #[derive(Clone, crate::Archetype)]
    struct Arch {
        comp: i16,
    }

    #[derive(Copy, Clone)]
    struct TestSystem {
        entity: EntityId,
    }

    impl SystemHandler for TestSystem {
        fn run(&mut self, data: SystemAccess) {
            let mut comp = data.component_mut::<i16>();

            let e_comp = comp.get_mut(self.entity).unwrap();
            assert_eq!(*e_comp, 123);
            *e_comp = 321;
        }
    }

    let mut storage = EntityStorage::new();
    let entity = storage.add(Arch { comp: 123 });

    let mut test_sys = TestSystem { entity };
    let sys0 = System::new(&mut test_sys).with_mut::<i16>();

    storage.dispatch(&mut [sys0]);

    assert_eq!(*storage.get::<i16>(&entity).unwrap(), 321);
}
