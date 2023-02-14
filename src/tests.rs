use crate::StaticArchetype;
use crate::{Archetype, EntityStorage};
use rand::Rng;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct Comp3;

#[derive(Clone, Archetype)]
struct Archetype12 {
    comp1: Comp1,
    comp2: Comp2,
}

#[derive(Clone, Archetype)]
struct Archetype1 {
    comp1: Comp1,
}

#[derive(Clone, Archetype)]
struct Archetype2(Comp2);

#[derive(Clone, Archetype)]
struct Archetype3(Comp3);

#[test]
fn general() {
    let mut storage = EntityStorage::new();

    let e00v = Comp1::new();
    let e01v = Comp2::new();
    let e1v = Comp1::new();
    let e2v = Comp2::new();

    let _e0 = storage.add(Archetype12 {
        comp1: e00v.clone(),
        comp2: e01v.clone(),
    });

    let e0 = storage.add(
        Archetype12 {
            comp1: e00v.clone(),
            comp2: e01v.clone(),
        }
        .into_any(),
    );

    let temp = storage.add(Archetype3(Comp3).into_any());
    storage.remove(&temp);

    let _e1 = storage.add(Archetype1 { comp1: e1v.clone() });
    let e1 = storage.add(Archetype1 { comp1: e1v.clone() });
    let _e2 = storage.add(Archetype2(e2v.clone()));
    let e2 = storage.add(Archetype2(e2v.clone()).into_any());

    assert_eq!(storage.count_entities(), 6);

    let v00 = storage.get::<Comp1>(&e0).unwrap();
    let v01 = storage.get::<Comp2>(&e0).unwrap();
    let v1 = storage.get::<Comp1>(&e1).unwrap();
    let v2 = storage.get::<Comp2>(&e2).unwrap();

    assert_eq!(&e00v, v00);
    assert_eq!(&e01v, v01);
    assert_eq!(&e1v, v1);
    assert_eq!(&e2v, v2);

    assert_eq!(storage.entry(&e1).unwrap().get::<Comp1>(), Some(&e1v));

    storage.remove(&_e0);
    storage.remove(&_e1);
    storage.remove(&_e2);
    storage.remove(&e0);
    storage.remove(&e1);
    storage.remove(&e2);

    let v00 = storage.get::<Comp1>(&e0);
    let v01 = storage.get::<Comp2>(&e0);
    let v1 = storage.get::<Comp1>(&e1);
    let v2 = storage.get::<Comp2>(&e2);

    assert_eq!(v00, None);
    assert_eq!(v01, None);
    assert_eq!(v1, None);
    assert_eq!(v2, None);

    assert_eq!(storage.count_entities(), 0);
}

#[test]
fn add_modify_remove_add() {
    let mut storage = EntityStorage::new();

    let e = storage.add(Archetype1 {
        comp1: Comp1 {
            a: 123,
            b: Default::default(),
        },
    });

    storage.get_mut::<Comp1>(&e).unwrap().a = 230;
    storage.remove(&e);

    let e2 = storage.add(Archetype1 {
        comp1: Comp1 {
            a: 123,
            b: Default::default(),
        },
    });

    assert_eq!(storage.get::<Comp1>(&e2).unwrap().a, 123);
}
