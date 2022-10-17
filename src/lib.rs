//! A container for entity component data.
//!
//! # Use Case
//! Suppose you need to store large amounts of objects, each of which can have different fields.
//! But you don't want to use Rust's dynamic-dispatch feature for the following reasons:
//! 1. Virtual dispatch induces indirection.
//! 2. You will have to store every object somewhere on heap.
//! That leads to cache-misses and hence slower iteration over the objects.
//!
//! Data-oriented programming helps to overcome these issues.
//!
//! # The Architecture
//!
//! An *entity* is an identifier for an object.
//! Each entity can have multiple components ("fields") associated with it.
//! The storage of all components itself is based on [ECS](https://en.wikipedia.org/wiki/Entity_component_system) technique.
//!
//! A unique set of components is called an `Archetype`.
//! An archetype maintains a contiguous vector for each of its component types.
//!
//! # Examples
//!
//! Simple usage:
//! ```
//! use entity_data::{EntityStorage, Archetype};
//!
//! #[derive(Clone)]
//! struct Barks {
//!     bark_sound: String,
//! }
//!
//! impl Barks {
//!     fn bark(&self) {
//!         println!("{}", self.bark_sound);
//!     }
//! }
//!
//! #[derive(Clone)]
//! struct Eats {
//!     favorite_food: String,
//!     eaten_food: Vec<String>,
//! }
//!
//! impl Eats {
//!     fn eat(&mut self, food: String) {
//!         self.eaten_food.push(food);
//!     }
//! }
//!
//! #[derive(Clone)]
//! struct Animal {
//!     weight: f32,
//!     habitat: String,
//! }
//!
//! #[derive(Clone, Archetype)]
//! struct Dog {
//!     animal: Animal,
//!     barks: Barks,
//!     eats: Eats,
//! }
//!
//! #[derive(Clone, Archetype)]
//! struct Bird(Animal, Eats);
//!
//! fn main() {
//!     let mut storage = EntityStorage::new();
//!
//!     let super_dog = storage.add_entity(Dog {
//!         animal: Animal { weight: 30.0, habitat: "forest".to_string(), },
//!         barks: Barks { bark_sound: "bark.ogg".to_string(), },
//!         eats: Eats { favorite_food: "meat".to_string(), eaten_food: vec![] },
//!     });
//!
//!     let hummingbird = storage.add_entity(Bird(
//!         Animal { weight: 5.0, habitat: "gardens".to_string()},
//!         Eats { favorite_food: "apples".to_string(), eaten_food: vec![] }
//!     ));
//!
//!     let super_dog_barks = storage.get::<Barks>(&super_dog).unwrap();
//!     super_dog_barks.bark();
//!
//!     let super_dog_eats = storage.get_mut::<Eats>(&super_dog).unwrap();
//!     super_dog_eats.favorite_food = "beans".to_string();
//!
//!     let hummingbird_eats = storage.get_mut::<Eats>(&hummingbird).unwrap();
//!     hummingbird_eats.eat("seeds".to_string());
//! }
//! ```
//!
//! Processing multiple components simultaneously:
//!
//! ```
//! use entity_data::{EntityId, EntityStorage, System, SystemHandler};
//! use entity_data::system::SystemData;
//! use macros::Archetype;
//!
//! #[derive(Default, Debug, Clone)]
//! struct Position {
//!     x: f32,
//!     y: f32,
//! }
//!
//! #[derive(Debug, Clone)]
//! struct Name(String);
//!
//! #[derive(Clone, Archetype)]
//! struct Dog {
//!     pos: Position,
//!     name: Name,
//! }
//!
//! struct PositionsPrintSystem {}
//!
//! #[derive(Default)]
//! struct ConcatAllNamesSystem {
//!     result: String
//! }
//!
//! fn main() {
//!     let mut storage = EntityStorage::new();
//!
//!     let dog0 = storage.add_entity(Dog {
//!         pos: Default::default(),
//!         name: Name("Bobby".to_owned())
//!     });
//!     let dog1 = storage.add_entity(Dog {
//!         pos: Position { x: 3.0, y: 5.0 },
//!         name: Name("Jet".to_owned())
//!     });
//!
//!
//!     impl SystemHandler for PositionsPrintSystem {
//!         fn run(&mut self, data: SystemData) {
//!             let positions = data.component::<Position>();
//!             let names = data.component::<Name>();
//!             for (pos, name) in positions.iter().zip(names) {
//!                 println!("{:?} - {:?}", pos, name);
//!             }
//!         }
//!     }
//!
//!     impl SystemHandler for ConcatAllNamesSystem {
//!         fn run(&mut self, data: SystemData) {
//!             let names = data.component::<Name>();
//!             for name in names {
//!                 self.result += &name.0;
//!             }
//!         }
//!     }
//!
//!     let mut positions_print_system = PositionsPrintSystem {};
//!     let mut concat_names_system = ConcatAllNamesSystem::default();
//!
//!     let mut sys0 = System::new(&mut positions_print_system)
//!         .with::<Position>().with::<Name>();
//!     let mut sys1 = System::new(&mut concat_names_system)
//!         .with::<Name>();
//!
//!     // or storage.dispatch_par() to run systems in parallel (requires `rayon` feature to be enabled).
//!     storage.dispatch(&mut [sys0, sys1]);
//!
//!     println!("{}", concat_names_system.result);
//! }
//! ```

#[cfg(test)]
mod tests;

pub mod archetype;
pub mod entity;
pub mod entity_storage;
pub mod private;
pub mod state;
pub mod system;

pub use archetype::component::Component;
pub use archetype::entities::ArchetypeEntities;
pub use archetype::ArchetypeStorage;
pub use entity::EntityId;
pub use entity_storage::component::ComponentGlobalAccess;
pub use entity_storage::component::ComponentGlobalIter;
pub use entity_storage::component::ComponentGlobalIterMut;
pub use entity_storage::EntityStorage;
pub use macros::Archetype;
pub use state::AnyState;
pub use state::ArchetypeState;
pub use state::StaticArchetype;
pub use system::System;
pub use system::SystemHandler;

pub(crate) type HashMap<K, V> = ahash::AHashMap<K, V>;

extern crate self as entity_data;
