//! Contains `FluoriteFluidArea3D`, a type that can locally override the global fluid.
use godot::{classes::Area3D, prelude::*};

use super::fluorite_fluid_config::FluoriteFluidConfig;

#[derive(GodotClass)]
#[class(init, base=Area3D)]
/// A special Area3D that can also override the local fluid.
pub struct FluoriteFluidArea3D {
    base: Base<Area3D>,
    #[export]
    #[init(val = 0)]
    /// The priority of the fluid.
    pub fluid_override_priority: i64,
    #[export]
    /// The config of the fluid.
    /// 
    /// **This field must always be `Some`** (non-`null`).
    /// If this invariant is broken, all casts that overlap with this area will panic.
    pub fluid_override_config: Option<Gd<FluoriteFluidConfig>>,
}