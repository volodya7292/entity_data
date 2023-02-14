pub(crate) mod component;

use crate::entity::ArchetypeId;
use crate::system::component::{
    CompMutability, GenericComponentGlobalAccess, GlobalComponentAccess, GlobalComponentAccessMut,
};
use crate::{Component, EntityStorage, HashMap};
use std::any::TypeId;
use std::cell::{RefCell, UnsafeCell};
use std::collections::hash_map;
use std::pin::Pin;
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

/// Represents all available components to a system.
pub struct SystemAccess<'a> {
    storage: &'a EntityStorage,
    /// Whether new components can be added to `global_components` from the `storage`.
    /// Safety: `storage` must be uniquely borrowed.
    new_components_allowed: bool,
    /// Maps component `TypeId`s to respective archetypes which contain this component.
    global_components:
        UnsafeCell<HashMap<TypeId, Pin<Box<RefCell<GenericComponentGlobalAccess<'a>>>>>>,
}

impl<'a> SystemAccess<'a> {
    fn get_component(&self, ty: TypeId) -> Option<&RefCell<GenericComponentGlobalAccess<'a>>> {
        let global_components = unsafe { &mut *self.global_components.get() };

        match global_components.entry(ty) {
            hash_map::Entry::Occupied(e) => Some(e.into_mut()),
            hash_map::Entry::Vacant(e) => {
                if !self.new_components_allowed {
                    return None;
                }

                // Modifying the hashmap is safe because referenced values are wrapped in Pin<Box<>>.
                let new = RefCell::new(GenericComponentGlobalAccess {
                    filtered_archetype_ids: self
                        .storage
                        .component_to_archetypes_map
                        .get(&ty)
                        .unwrap_or(&vec![])
                        .clone(),
                    all_archetypes: &self.storage.archetypes,
                    // Safety: mutability is allowed because `self.new_components_allowed` is true,
                    // therefore `self.storage` must be uniquely borrowed.
                    mutable: true,
                });

                Some(e.insert(Box::pin(new)))
            }
        }
    }

    /// Returns `ArchetypeId` corresponding to the specified `TypeId`.
    pub fn type_id_to_archetype_id(&self, type_id: &TypeId) -> Option<ArchetypeId> {
        self.storage.type_id_to_archetype_id(type_id)
    }

    /// Borrows the component.
    /// Panics if the component is mutably borrowed or not available to this system.
    pub fn component<C: Component>(&self) -> GlobalComponentAccess<C> {
        let ty = TypeId::of::<C>();

        // This is safe because the mutable reference gets dropped afterwards.
        let generic = self.get_component(ty).expect("Component must be available");

        GlobalComponentAccess {
            generic: generic
                .try_borrow()
                .expect("Component must not be mutably borrowed"),
            _ty: Default::default(),
        }
    }

    /// Mutably borrows the component.
    /// Panics if the component is already borrowed or not available to this system.
    pub fn component_mut<'b, C: Component>(&'b self) -> GlobalComponentAccessMut<'a, 'b, C> {
        let generic = self
            .get_component(TypeId::of::<C>())
            .expect("Component must be available");

        let guard = generic
            .try_borrow_mut()
            .expect("Component must not be borrowed");

        if !guard.mutable {
            panic!("Component is not allowed to be mutated");
        }

        GlobalComponentAccessMut {
            generic: guard,
            _ty: Default::default(),
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
                    Box::pin(RefCell::new(self.global_component_by_id(ty, *mutable))),
                )
            })
            .collect();

        SystemAccess {
            storage: self,
            // `self` is not uniquely borrowed, so restrict access only to specified components.
            new_components_allowed: false,
            global_components: UnsafeCell::new(global_components),
        }
    }

    /// Provides access to all components. Allows simultaneous mutable access to multiple components.
    pub fn access(&mut self) -> SystemAccess {
        SystemAccess {
            storage: self,
            // Safety: `self` is &mut, therefore this is valid.
            new_components_allowed: true,
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
    /// struct PositionsPrintSystem {
    ///     to_process: Vec<EntityId>,
    /// }
    ///
    /// impl SystemHandler for PositionsPrintSystem {
    ///     fn run(&mut self, data: SystemAccess) {
    ///         let positions = data.component::<Position>();
    ///         for entity in &self.to_process {
    ///             println!("{:?}", positions.get(entity));
    ///         }
    ///     }
    /// }
    ///
    /// let mut sys = PositionsPrintSystem {
    ///     to_process: vec![dog0, dog1]
    /// };
    /// storage.dispatch(&mut [System::new(&mut sys).with::<Position>()]);
    /// ```
    pub fn dispatch<'a>(&self, mut systems: impl AsMut<[System<'a>]>) {
        for sys in systems.as_mut() {
            let data = unsafe { self.get_system_data(&sys.components) };
            sys.handler.run(data);
        }
    }

    /// Dispatches systems in parallel if possible. Two systems won't execute in parallel if they
    /// access the same component and one of the systems mutates this component.
    #[cfg(feature = "rayon")]
    pub fn dispatch_par<'a>(&self, mut systems: impl AsMut<[System<'a>]>) {
        let systems = systems.as_mut();

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
    use crate::EntityId;

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

            let e_comp = comp.get_mut(&self.entity).unwrap();
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
