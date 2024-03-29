# entity_data

[![Build Status][build_img]][build_lnk] [![Crates.io][crates_img]][crates_lnk] [![Docs.rs][doc_img]][doc_lnk]

[build_img]: https://github.com/volodya7292/entity_data/actions/workflows/build.yml/badge.svg

[build_lnk]: https://github.com/volodya7292/entity_data/actions

[crates_img]: https://img.shields.io/crates/v/entity_data.svg

[crates_lnk]: https://crates.io/crates/entity_data

[doc_img]: https://docs.rs/entity_data/badge.svg

[doc_lnk]: https://docs.rs/entity_data

A container for entity component data.

## Use Case

Suppose you need to store large number of objects, each of which can have multiple different fields.
But you don't want to use Rust's dynamic-dispatch feature for the following reasons:

1. Virtual dispatch induces indirection.
2. You will have to store every object somewhere on the heap.
   This leads to cache-misses and hence slower iteration over the objects.

Data-oriented approach helps to overcome these issues.

## The Architecture

An *entity* is an identifier for an object.
Each entity can have multiple components ("fields") associated with it.
The storage itself is based on [ECS](https://en.wikipedia.org/wiki/Entity_component_system) technique.

A unique set of components is called an `Archetype`.
An archetype maintains a contiguous vector for each of its component types.

## Examples

Simple usage:

```rust
use entity_data::{EntityStorage, Archetype};

struct Barks {
    bark_sound: String,
}

impl Barks {
    fn bark(&self) {
        println!("{}", self.bark_sound);
    }
}

struct Eats {
    favorite_food: String,
    eaten_food: Vec<String>,
}

impl Eats {
    fn eat(&mut self, food: String) {
        self.eaten_food.push(food);
    }
}

struct Animal {
    weight: f32,
    habitat: String,
}

#[derive(Archetype)]
struct Dog {
    animal: Animal,
    barks: Barks,
    eats: Eats,
}

#[derive(Archetype)]
struct Bird(Animal, Eats);

fn main() {
    let mut storage = EntityStorage::new();

    let super_dog = storage.add_entity(Dog {
        animal: Animal { weight: 30.0, habitat: "forest".to_string(), },
        barks: Barks { bark_sound: "bark.ogg".to_string(), },
        eats: Eats { favorite_food: "meat".to_string(), eaten_food: vec![] },
    });

    let hummingbird = storage.add_entity(Bird(
        Animal { weight: 5.0, habitat: "gardens".to_string() },
        Eats { favorite_food: "apples".to_string(), eaten_food: vec![] }
    ));

    let super_dog_barks = storage.get::<Barks>(&super_dog).unwrap();
    super_dog_barks.bark();

    let super_dog_eats = storage.get_mut::<Eats>(&super_dog).unwrap();
    super_dog_eats.favorite_food = "beans".to_string();

    let hummingbird_eats = storage.get_mut::<Eats>(&hummingbird).unwrap();
    hummingbird_eats.eat("seeds".to_string());
}
```

Processing multiple components simultaneously:

```rust
use entity_data::{EntityId, EntityStorage, System, SystemHandler};
use entity_data::system::SystemAccess;
use macros::Archetype;

#[derive(Default, Debug)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug)]
struct Name(String);

#[derive(Archetype)]
struct Dog {
    pos: Position,
    name: Name,
}

struct PositionsPrintSystem {}

#[derive(Default)]
struct ConcatAllNamesSystem {
    result: String
}

impl SystemHandler for PositionsPrintSystem {
    fn run(&mut self, data: SystemAccess) {
        let positions = data.component::<Position>();
        let names = data.component::<Name>();
        for (pos, name) in positions.iter().zip(names) {
            println!("{:?} - {:?}", pos, name);
        }
    }
}

impl SystemHandler for ConcatAllNamesSystem {
    fn run(&mut self, data: SystemAccess) {
        let names = data.component::<Name>();
        for name in names {
            self.result += &name.0;
        }
    }
}

fn main() {
    let mut storage = EntityStorage::new();

    let dog0 = storage.add(Dog {
        pos: Default::default(),
        name: Name("Bobby".to_owned())
    });
    let dog1 = storage.add(Dog {
        pos: Position { x: 3.0, y: 5.0 },
        name: Name("Jet".to_owned())
    });

    let mut positions_print_system = PositionsPrintSystem {};
    let mut concat_names_system = ConcatAllNamesSystem::default();

    let mut sys0 = System::new(&mut positions_print_system)
        .with::<Position>().with::<Name>();
    let mut sys1 = System::new(&mut concat_names_system)
        .with::<Name>();

    // or storage.dispatch_par() to run systems in parallel (requires `rayon` feature to be enabled).
    storage.dispatch(&mut [sys0, sys1]);

    println!("{}", concat_names_system.result);
}
```
