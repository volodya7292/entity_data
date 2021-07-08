//! A container for entity component data.
//!
//! An entity is an opaque identifier for an object.
//! Each entity can have multiple components associated with it.
//! Storage is based on [ECS](https://en.wikipedia.org/wiki/Entity_component_system) technique,
//! but the main purpose is to efficiently store and access
//! individual components without `Box`ing them.
//!
//! The approach used in this library is superior to Rust's dynamic dispatch because
//! components can have their separate fields and components of the
//! same type are stored in a contiguous vector.
//!
//! # Examples
//!
//! ```
//! use entity_data::EntityStorageLayout;
//! use entity_data::EntityStorage;
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
//!     eaten_food: Vec<String>,
//! }
//!
//! impl Eats {
//!     fn eat(&mut self, food: String) {
//!         self.eaten_food.push(food);
//!     }
//! }
//!
//! struct Dog {
//!     favorite_food: String,
//! }
//!
//! struct Bird {
//!     weight: f32,
//!     habitat: String,
//! }
//!
//! fn main() {
//!     let mut layout = EntityStorageLayout::new();
//!     let type_dog = layout.add_archetype().with::<Dog>().with::<Barks>().with::<Eats>().build();
//!     let type_bird = layout.add_archetype().with::<Bird>().with::<Eats>().build();
//!
//!     let mut storage = EntityStorage::new(&layout);
//!
//!     let super_dog = storage.add_entity(type_dog)
//!         .with(Dog { favorite_food: "meat".to_string(), })
//!         .with(Eats { eaten_food: vec![] })
//!         .with(Barks { bark_sound: "bark.ogg".to_string() })
//!         .build();
//!
//!     let hummingbird = storage.add_entity(type_bird)
//!         .with(Bird { weight: 0.07, habitat: "gardens".to_string() })
//!         .with(Eats { eaten_food: vec![] })
//!         .build();
//!
//!
//!     let super_dog_barks = storage.get::<Barks>(&super_dog).unwrap();
//!     super_dog_barks.bark();
//!
//!     let super_dog_props = storage.get_mut::<Dog>(&super_dog).unwrap();
//!     super_dog_props.favorite_food = "beans".to_string();
//!
//!     let hummingbird_eats = storage.get_mut::<Eats>(&hummingbird).unwrap();
//!     hummingbird_eats.eat("seeds".to_string());
//! }
//! ```

#[cfg(test)]
mod tests;

mod archetype;
mod entity_storage;
mod utils;

pub use entity_storage::ArchetypeBuilder;
pub use entity_storage::EntityBuilder;
pub use entity_storage::EntityId;
pub use entity_storage::EntityStorage;
pub use entity_storage::EntityStorageLayout;

/// A simple method of adding a archetype to `EntityStorageLayout`.
///
/// # Examples
/// ```
/// use entity_data::EntityStorageLayout;
/// use entity_data::add_archetype;
///
/// struct Comp1 { }
/// struct Comp2 { }
///
/// let mut layout = EntityStorageLayout::new();
/// let id = add_archetype!(layout, Comp1, Comp2);
/// ```
#[macro_export]
macro_rules! add_archetype {
    ($storage_layout: expr, $($component_ty: ty),+) => {
        $storage_layout.add_archetype()
        $(.with::<$component_ty>())*
        .build()
    };
}

/// A simple method of adding an entity to `EntityStorage`.
///
/// # Examples
/// ```
/// use entity_data::{EntityStorageLayout, EntityStorage, add_archetype, add_entity};
///
/// struct Comp1 { }
/// struct Comp2 { }
///
/// let mut layout = EntityStorageLayout::new();
/// let arch_id = add_archetype!(layout, Comp1, Comp2);
///
/// let mut storage = EntityStorage::new(&layout);
/// let entity_id = add_entity!(storage, arch_id, Comp1 {}, Comp2 {});
/// ```
#[macro_export]
macro_rules! add_entity {
    ($storage: expr, $archetype_id: expr, $($component: expr),+) => {
        $storage.add_entity($archetype_id)
        $(.with($component))*
        .build()
    };
}
