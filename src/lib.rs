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
//! struct Animal {
//!     weight: f32,
//!     habitat: String,
//! }
//!
//! #[derive(Archetype)]
//! struct Dog {
//!     animal: Animal,
//!     barks: Barks,
//!     eats: Eats,
//! }
//!
//! #[derive(Archetype)]
//! struct Bird(Animal, Eats);
//!
//! fn main() {
//!     let mut storage = EntityStorage::new();
//!
//!     let super_dog_entity = storage.add(Dog {
//!         animal: Animal { weight: 30.0, habitat: "forest".to_string(), },
//!         barks: Barks { bark_sound: "bark.ogg".to_string(), },
//!         eats: Eats { favorite_food: "meat".to_string(), eaten_food: vec![] },
//!     });
//!
//!     let hummingbird_entity = storage.add(Bird(
//!         Animal { weight: 5.0, habitat: "gardens".to_string()},
//!         Eats { favorite_food: "apples".to_string(), eaten_food: vec![] }
//!     ));
//!
//!     let mut  super_dog = storage.entry_mut(&super_dog_entity).unwrap();
//!     let super_dog_barks = super_dog.get::<Barks>().unwrap();
//!     super_dog_barks.bark();
//!
//!     let super_dog_eats = super_dog.get_mut::<Eats>().unwrap();
//!     super_dog_eats.favorite_food = "beans".to_string();
//!
//!     let hummingbird_eats = storage.get_mut::<Eats>(&hummingbird_entity).unwrap();
//!     hummingbird_eats.eat("seeds".to_string());
//! }

#[cfg(test)]
mod tests;

pub mod archetype;
pub mod entity;
pub mod entity_storage;
pub mod entry;
pub mod private;
pub mod state;
pub mod system;

pub use archetype::component::Component;
pub use archetype::entities::ArchetypeEntities;
pub use archetype::ArchetypeStorage;
pub use entity::EntityId;
pub use entity_storage::EntityStorage;
pub use entry::{Entry, EntryMut};
pub use macros::Archetype;
pub use state::{AnyState, ArchetypeState, StaticArchetype};
pub use system::component::{GenericComponentGlobalAccess, GlobalComponentAccess};
pub use system::{System, SystemAccess, SystemHandler};

pub(crate) type HashMap<K, V> = ahash::AHashMap<K, V>;

extern crate self as entity_data;
