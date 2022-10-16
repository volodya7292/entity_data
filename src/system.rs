use crate::entity_storage::component::{ComponentGlobalAccess, GenericComponentGlobalAccess};
use crate::entity_storage::AllEntities;
use crate::{Component, EntityStorage, HashMap};
use std::any::TypeId;
use std::cell::{Ref, RefCell, RefMut};
use std::fmt::Debug;

#[derive(Debug, Copy, Clone)]
pub struct ComponentMutability(bool);

pub trait SystemHandler: Send + Sync {
    fn run(&mut self, data: SystemData);
}

/// A system context.
pub struct System<'a> {
    handler: Box<dyn SystemHandler + 'a>,
    components: HashMap<TypeId, ComponentMutability>,
}

impl<'a> System<'a> {
    /// Creates a system with data handler.
    pub fn new(handler: impl SystemHandler + 'a) -> Self {
        Self {
            handler: Box::new(handler),
            components: Default::default(),
        }
    }

    /// Makes component accessible from the system.
    pub fn with<C: Component>(mut self) -> Self {
        self.components
            .insert(TypeId::of::<C>(), ComponentMutability(false));
        self
    }

    /// Makes component mutably accessible from the system.
    pub fn with_mut<C: Component>(mut self) -> Self {
        self.components
            .insert(TypeId::of::<C>(), ComponentMutability(true));
        self
    }
}

/// Represents all available components to a system.
pub struct SystemData<'a> {
    entities: AllEntities<'a>,
    global_components: HashMap<TypeId, RefCell<GenericComponentGlobalAccess<'a>>>,
}

impl<'a> SystemData<'a> {
    fn get_or_create_component(
        &mut self,
        ty: TypeId,
    ) -> &mut RefCell<GenericComponentGlobalAccess<'a>> {
        self.global_components.entry(ty).or_insert_with(|| {
            RefCell::new(GenericComponentGlobalAccess {
                filtered_archetype_ids: &[],
                all_archetypes: &[],
                all_entities: self.entities,
                // Safety: true is allowed here because there's nothing to modify.
                mutable: true,
            })
        })
    }

    /// Borrows the component.
    /// Panics if the component is mutably borrowed.
    pub fn component<C: Component>(
        &mut self,
    ) -> ComponentGlobalAccess<C, Ref<GenericComponentGlobalAccess<'a>>, &()> {
        let generic = self.get_or_create_component(TypeId::of::<C>());

        ComponentGlobalAccess {
            generic: RefCell::borrow(generic),
            _ty: Default::default(),
            _mutability: Default::default(),
        }
    }

    /// Mutably borrows the component.
    /// Panics if the component is already borrowed.
    pub fn component_mut<C: Component>(
        &mut self,
    ) -> ComponentGlobalAccess<C, RefMut<GenericComponentGlobalAccess<'a>>, &mut ()> {
        let generic = self.get_or_create_component(TypeId::of::<C>());
        let generic = RefCell::borrow_mut(generic);

        if !generic.mutable {
            panic!("Specified component is not allowed to be mutated");
        }

        ComponentGlobalAccess {
            generic,
            _ty: Default::default(),
            _mutability: Default::default(),
        }
    }
}

#[cfg(feature = "rayon")]
mod parallel {
    use std::any::TypeId;
    use std::collections::hash_map;
    use std::mem;
    use crate::{HashMap, System};
    use crate::system::ComponentMutability;

    #[derive(Debug)]
    pub struct ParallelSystems {
        pub systems: Vec<usize>,
        pub all_components: HashMap<TypeId, ComponentMutability>,
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
                        if !a_mutable.0 {
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
        a_components: &HashMap<TypeId, ComponentMutability>,
        b_components: &HashMap<TypeId, ComponentMutability>,
    ) -> bool {
        a_components.iter().any(|(ty, mutable_a)| {
            b_components
                .get(ty)
                .map_or(false, |mutable_b| mutable_a.0 || mutable_b.0)
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

                    let conflicting = systems_do_conflict(&sys.all_components, &sys2.all_components);

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
    unsafe fn get_system_data(&self, system: &System) -> SystemData {
        let global_components = system
            .components
            .iter()
            .map(|(&ty, mutable)| (ty, RefCell::new(self.global_component_by_id(ty, mutable.0))))
            .collect();

        SystemData {
            entities: self.entities(),
            global_components,
        }
    }

    /// Dispatches systems sequentially. For parallel execution,
    /// see [dispatch_par](Self::dispatch_par) (requires `rayon` feature).
    ///
    /// # Example
    /// ```
    /// use entity_data::{EntityId, EntityStorage, SystemHandler};
    /// use entity_data::system::SystemData;
    /// use macros::Archetype;
    ///
    /// #[derive(Default, Debug, Clone)]
    /// struct Position {
    ///     x: f32,
    ///     y: f32,
    /// }
    ///
    /// #[derive(Clone, Archetype)]
    /// struct Dog {
    ///     pos: Position,
    /// }
    ///
    /// let mut storage = EntityStorage::new();
    /// let dog0 = storage.add_entity(Dog { pos: Default::default() });
    /// let dog1 = storage.add_entity(Dog { pos: Position { x: 3.0, y: 5.0 } });
    ///
    /// struct PositionsPrintSystem {}
    ///
    /// impl SystemHandler for PositionsPrintSystem {
    ///     fn run(&mut self, mut data: SystemData) {
    ///         let positions = data.component::<Position>();
    ///         for pos in positions {
    ///             println!("{:?}", pos);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn dispatch(&self, systems: &mut [&mut System]) {
        for sys in systems {
            let data = unsafe { self.get_system_data(sys) };
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
                        let data = unsafe { self.get_system_data(system) };
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
        fn run(&mut self, _: SystemData) {}
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

    let test_sys = TestSystem {};
    let c0 = (TypeId::of::<i8>(), ComponentMutability(true));
    let c1 = (TypeId::of::<i16>(), ComponentMutability(true));
    let c2 = (TypeId::of::<i32>(), ComponentMutability(true));
    let c3 = (TypeId::of::<i64>(), ComponentMutability(true));
    let c4 = (TypeId::of::<u64>(), ComponentMutability(true));

    let sys0 = System {
        handler: Box::new(test_sys),
        components: [c1].into_iter().collect(),
    };
    let sys1 = System {
        handler: Box::new(test_sys),
        components: [c2, c3].into_iter().collect(),
    };
    let sys2 = System {
        handler: Box::new(test_sys),
        components: [c1, c4].into_iter().collect(),
    };
    let sys3 = System {
        handler: Box::new(test_sys),
        components: [c0, c3].into_iter().collect(),
    };
    let sys4 = System {
        handler: Box::new(test_sys),
        components: [c0, c1, c4].into_iter().collect(),
    };

    let systems = [sys0, sys1, sys2, sys3, sys4];
    let parallel_runs = parallel::partition_parallel_systems(&systems);

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
                parallel::systems_do_conflict(&systems[*sys0_id].components, &systems[*sys1_id].components)
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
        entity: crate::EntityId,
    }

    impl SystemHandler for TestSystem {
        fn run(&mut self, mut data: SystemData) {
            let mut comp = data.component_mut::<i16>();

            let e_comp = comp.get_mut(self.entity).unwrap();
            assert_eq!(*e_comp, 123);
            *e_comp = 321;
        }
    }

    let mut storage = EntityStorage::new();

    let entity = storage.add_entity(Arch { comp: 123 });
    storage.component::<u16>().get(entity);

    let test_sys = TestSystem { entity };
    let c1 = (TypeId::of::<i16>(), ComponentMutability(true));

    let mut sys0 = System {
        handler: Box::new(test_sys),
        components: [c1].into_iter().collect(),
    };

    storage.dispatch(&mut [&mut sys0]);

    assert_eq!(*storage.get::<i16>(&entity).unwrap(), 321);
}
