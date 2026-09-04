use godot::prelude::*;

// We assume that the temperature and pressure are constants
// This works well with almost all gameplay scenarios without causing huge headaches
#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteFluidConfig {
    base: Base<Resource>,
    #[export]
    pub fluid_density_kgm3: f64, // kg/m3 scalar
    #[export]
    pub dynamic_viscosity_upas: f64, // uPa*s scalar (NOT Pa*s)
    #[export]
    pub speed_of_sound: f64, // m/s scalar
    #[export]
    pub ambient_airspeed: Vector3, // m/s vector
}

#[godot_api]
impl FluoriteFluidConfig {
    #[func]
    fn new_config(fluid_density_kgm3: f64, dynamic_viscosity_upas: f64, speed_of_sound: f64, ambient_airspeed: Vector3) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                fluid_density_kgm3,
                dynamic_viscosity_upas,
                speed_of_sound,
                ambient_airspeed,
            }
        })
    }
    #[func]
    pub fn new_air(ambient_airspeed: Vector3) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            // 1atm, 300K
            Self {
                base,
                fluid_density_kgm3: 1.164,
                dynamic_viscosity_upas: 30.6,
                speed_of_sound: 332.0,
                ambient_airspeed,
            }
        })
    }
    #[func]
    pub fn new_still_air() -> Gd<Self> {
        Self::new_air(Vector3::ZERO)
    }
    #[func]
    pub fn new_water(ambient_airspeed: Vector3) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            // 300K
            Self {
                base,
                fluid_density_kgm3: Self::gcm3_to_kgm3(0.9965),
                dynamic_viscosity_upas: Self::mpas_to_upas(0.854),
                speed_of_sound: 1550.0,
                ambient_airspeed,
            }
        })
    }
    #[func]
    pub fn new_still_water() -> Gd<Self> {
        Self::new_water(Vector3::ZERO)
    }
    #[func]
    pub fn new_vacuum() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                fluid_density_kgm3: 0.0001,
                dynamic_viscosity_upas: 0.0001,
                speed_of_sound: 299792458.0, // 1c as placeholder
                ambient_airspeed: Vector3::ZERO,
            }
        })
    }
    #[func]
    pub fn gcm3_to_kgm3(gcm3: f64) -> f64 {
        gcm3 * 1000.0
    }
    #[func]
    pub fn mpas_to_upas(base: f64) -> f64 {
        base * 1000.0
    }
}