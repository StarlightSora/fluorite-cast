//! To properly register the library in an existing project using godot-rust,
//! remember to add this to your root crate's `lib.rs`:
//! ```
//! extern crate fluorite_cast;
//! ```
//! 
//! The `gdextension` `entry_symbol` of this library is `fluorite_cast`.
use godot::prelude::*;

pub struct FluoriteCastExtension;
#[gdextension(entry_symbol = fluorite_cast)]
unsafe impl ExtensionLibrary for FluoriteCastExtension {}

pub mod prelude;
pub mod fluorite_fluid_config;
//pub mod fluorite_cast_factory;
pub mod fluorite_cast_config;
pub mod fluorite_cast;