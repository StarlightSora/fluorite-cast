//! This is the root module of this library.
//!
//! To properly register the library in an existing project using godot-rust,
//! remember to add this to your root crate's `lib.rs`:
//! ```
//! extern crate fluorite_cast;
//! ```
//!
//! The `gdextension` `entry_symbol` of this library is `fluorite_cast`.
#[cfg(feature = "standalone_cdylib")]
use godot::prelude::*;

#[cfg(feature = "standalone_cdylib")]
pub struct FluoriteCastExtension;
#[cfg(feature = "standalone_cdylib")]
#[gdextension(entry_symbol = fluorite_cast)]
#[cfg(feature = "standalone_cdylib")]
unsafe impl ExtensionLibrary for FluoriteCastExtension {}

pub mod fluorite_cast;
pub mod fluorite_cast_config;
pub mod fluorite_cast_factory;
pub mod fluorite_fluid_area3d;
pub mod fluorite_fluid_config;
pub mod prelude;
