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
//! use entity_data::entity_state;
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
//! #[derive(Clone)]
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
//!     let eats = Eats { eaten_food: vec![] };
//!
//!     let super_dog = storage.add_entity(type_dog, entity_state!(
//!         Dog = Dog { favorite_food: "meat".to_string(), },
//!         Eats = eats.clone(),
//!         Barks = Barks { bark_sound: "bark.ogg".to_string() },
//!     ));
//!
//!     let hummingbird = storage.add_entity(type_bird, entity_state!(
//!         Bird = Bird { weight: 0.07, habitat: "gardens".to_string() },
//!         Eats = eats
//!     ));
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
pub use entity_storage::EntityId;
pub use entity_storage::EntityState;
pub use entity_storage::EntityStorage;
pub use entity_storage::EntityStorageLayout;
pub use entity_storage::ComponentInfo;

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

#[macro_export]
macro_rules! __replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

#[macro_export]
macro_rules! entity_state {
    ($($comp_type:ident = $component: expr ),* $(,)?) => {{
        const TOTAL_SIZE: usize = 0 $(+ std::mem::size_of::<$comp_type>())*;
        const N: usize =  0 $(+ $crate::__replace_expr!($component 1))*;

        let mut data = std::mem::MaybeUninit::<[u8; TOTAL_SIZE]>::uninit();
        let mut offsets = std::mem::MaybeUninit::<[$crate::ComponentInfo; N]>::uninit();
        let mut data_ptr = data.as_mut_ptr() as *mut u8;
        let mut data_offset = 0;
        let mut offset_ptr = offsets.as_mut_ptr() as *mut $crate::ComponentInfo;

        #[allow(unused_assignments)]
        unsafe {
            $(
                let comp_size = std::mem::size_of::<$comp_type>();

                std::ptr::write(data_ptr as *mut $comp_type, $component);

                std::ptr::write(offset_ptr, $crate::ComponentInfo {
                    type_id: std::any::TypeId::of::<$comp_type>(),
                    range: data_offset..(data_offset + comp_size),
                });

                data_ptr = data_ptr.add(comp_size);
                data_offset += comp_size;
                offset_ptr = offset_ptr.add(1);
            )*

            $crate::EntityState::from_raw(data.assume_init(), offsets.assume_init())
        }
    }};
}
