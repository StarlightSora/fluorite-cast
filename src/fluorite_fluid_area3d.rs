use godot::{classes::Area3D, prelude::*};

use super::fluorite_fluid_config::FluoriteFluidConfig;

#[derive(GodotClass)]
#[class(init, base=Area3D)]
pub struct FluoriteFluidArea3D {
    base: Base<Area3D>,
    #[export]
    #[init(val = 0)]
    pub fluid_override_priority: i64,
    #[export]
    pub fluid_override_config: Option<Gd<FluoriteFluidConfig>>,
}