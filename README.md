# entity_data

[![Build Status][build_img]][build_lnk] [![Crates.io][crates_img]][crates_lnk] [![Docs.rs][doc_img]][doc_lnk]

[build_img]: https://github.com/a7292969/entity_data/actions/workflows/build.yml/badge.svg

[build_lnk]: https://github.com/a7292969/entity_data/actions

[crates_img]: https://img.shields.io/crates/v/entity_data.svg

[crates_lnk]: https://crates.io/crates/entity_data

[doc_img]: https://docs.rs/entity_data/badge.svg

[doc_lnk]: https://docs.rs/entity_data

A container for entity component data.

An entity is an opaque identifier for an object.
Each entity can have multiple components associated with it.
Storage is based on [ECS](https://en.wikipedia.org/wiki/Entity_component_system) technique,
but the main purpose of this crate is to efficiently store and access
individual components without `Box`ing them.

The approach used in this crate is superior to Rust's dynamic dispatch because
components can have their separate fields and components of the
same type are stored in a contiguous vector.

## Example

```rust
use entity_data::{EntityStorage, Archetype};

#[derive(Clone)]
struct Barks {
    bark_sound: String,
}

impl Barks {
    fn bark(&self) {
        println!("{}", self.bark_sound);
    }
}

#[derive(Clone)]
struct Eats {
    favorite_food: String,
    eaten_food: Vec<String>,
}

impl Eats {
    fn eat(&mut self, food: String) {
        self.eaten_food.push(food);
    }
}

#[derive(Clone)]
struct Animal {
    weight: f32,
    habitat: String,
}

#[derive(Clone, Archetype)]
struct Dog {
    animal: Animal,
    barks: Barks,
    eats: Eats,
}

#[derive(Clone, Archetype)]
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