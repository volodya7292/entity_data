use crate::{add_archetype, entity_state, EntityStorage, EntityStorageLayout};
use rand::Rng;
use std::convert::TryInto;

#[derive(Debug, Clone, Eq, PartialEq)]
struct Comp1 {
    a: u32,
    b: [u32; 4],
}

impl Comp1 {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        Comp1 {
            a: rng.gen(),
            b: [rng.gen(), rng.gen(), rng.gen(), rng.gen()],
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Comp2 {
    a: Vec<Comp1>,
    b: [usize; 123],
    c: [u32; 4],
}

impl Comp2 {
    fn new() -> Self {
        let mut rng = rand::thread_rng();

        let a: Vec<Comp1> = (0..rng.gen_range(0..100)).map(|_| Comp1::new()).collect();
        let b: Vec<usize> = (0..123).map(|_| rng.gen()).collect();

        Comp2 {
            a,
            b: b.try_into().unwrap(),
            c: [rng.gen(), rng.gen(), rng.gen(), rng.gen()],
        }
    }
}

#[test]
fn it_works() {
    let mut layout = EntityStorageLayout::new();
    let a1 = add_archetype!(layout, Comp1, Comp2);
    let a2 = add_archetype!(layout, Comp1);
    let a3 = add_archetype!(layout, Comp2);

    let mut storage = EntityStorage::new(&layout);

    let e00v = Comp1::new();
    let e01v = Comp2::new();
    let e1v = Comp1::new();
    let e2v = Comp2::new();

    let _e0 = storage.add_entity(
        a1,
        entity_state!(Comp1 = e00v.clone(), Comp2 = e01v.clone()),
    );
    let e0 = storage.add_entity(
        a1,
        entity_state!(Comp1 = e00v.clone(), Comp2 = e01v.clone()),
    );
    let _e1 = storage.add_entity(a2, entity_state!(Comp1 = e1v.clone()));
    let e1 = storage.add_entity(a2, entity_state!(Comp1 = e1v.clone()));
    let _e2 = storage.add_entity(a3, entity_state!(Comp2 = e2v.clone()));
    let e2 = storage.add_entity(a3, entity_state!(Comp2 = e2v.clone()));

    assert_eq!(storage.len(), 6);

    let v00 = storage.get::<Comp1>(&e0).unwrap();
    let v01 = storage.get::<Comp2>(&e0).unwrap();
    let v1 = storage.get::<Comp1>(&e1).unwrap();
    let v2 = storage.get::<Comp2>(&e2).unwrap();

    assert_eq!(&e00v, v00);
    assert_eq!(&e01v, v01);
    assert_eq!(&e1v, v1);
    assert_eq!(&e2v, v2);

    storage.remove(&_e0);
    storage.remove(&_e1);
    storage.remove(&_e2);
    storage.remove(&e0);
    storage.remove(&e1);
    storage.remove(&e2);

    assert_eq!(storage.len(), 0);
}
