use godot::{classes::{Curve, Shape3D}, prelude::*};

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum EvaluateMode {
    #[default]
    PhysicsProcess,
    Process,
    Manual,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum LookBehavior {
    #[default]
    FollowVelocity, // TODO: Not in use yet
    Manual,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum SuperSamplingMode {
    Never,
    #[default]
    IfAboveTargetDelta,
    IfTooLong,
    IfAboveTargetDeltaOrTooLong,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum GravityBehavior {
    Ignore,
    #[default]
    UseGlobalGravityCached,
    UseGlobalGravityRealTime,
    UseCurrentGravityRealTime,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum FluidDynamicsBehavior {
    /// Ignore all fluids
    Ignore,
    /// Use global fluid as a constant
    #[default]
    UseGlobalFluidCached,
    /// Query global fluid every operation
    UseGlobalFluidRealTime,
    /// Query current fluid every operation (WIP, functions as UseGlobalFluidRealTime at the moment)
    UseCurrentFluidRealTime, // TODO: Make custom Area3Ds that can override the global fluid for this to work
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum FluidDynamicsFidelity {
    /// Do not simulate fluid dynamics at all
    Ignore,
    /// Simulate acceleration from ambient airspeed, but do not simulate drag
    #[default]
    OnlyAmbientAirspeed,
    /// Simulate ambient airspeed and calculate drag coefficient via Reynold's number only
    ReynoldsNumber,
    /// Simulate ambient airspeed, and calculate drag coefficient via Reynold's number + mach number
    Full,
}

////////////////

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgFluidDynamics {
    base: Base<Resource>,
    #[export]
    pub fluid_dynamics_behavior: FluidDynamicsBehavior,
    #[export]
    pub fluid_dynamics_fidelity: FluidDynamicsFidelity,
    #[export]
    #[init(val = 1.0)]
    pub projectile_reference_area: f64, // We assume the reference area is a constant so we don't have to do needlessly expensive realtime computation
    #[export]
    pub mach_based_drag_multiplier: Option<Gd<Curve>>,
}
#[godot_api]
impl FluoriteCastCfgFluidDynamics {
    pub fn new_config(fluid_dynamics_behavior: FluidDynamicsBehavior, fluid_dynamics_fidelity: FluidDynamicsFidelity, projectile_reference_area: f64, mach_based_drag_multiplier: Option<Gd<Curve>>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                fluid_dynamics_behavior,
                fluid_dynamics_fidelity,
                projectile_reference_area,
                mach_based_drag_multiplier,
            }
        })
    }
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                fluid_dynamics_behavior: FluidDynamicsBehavior::default(),
                fluid_dynamics_fidelity: FluidDynamicsFidelity::default(),
                projectile_reference_area: 1.0,
                mach_based_drag_multiplier: None,
            }
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgFidelity {
    base: Base<Resource>,
    #[export]
    pub super_sampling_mode: SuperSamplingMode,
    #[export]
    #[init(val = Self::delta_to_hz(120.0 * 0.95))]
    pub target_delta: f64,
    #[export]
    #[init(val = 25.0)]
    pub target_length: f64,
    #[export]
    #[init(val = 4)]
    pub max_supersampling: i64,
}
#[godot_api]
impl FluoriteCastCfgFidelity {
    pub fn new_config(super_sampling_mode: SuperSamplingMode, target_delta: f64, target_length: f64, max_supersampling: i64) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                super_sampling_mode,
                target_delta,
                target_length,
                max_supersampling,
            }
        })
    }
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                super_sampling_mode: SuperSamplingMode::default(),
                target_delta: Self::hz_to_delta(120.0 * 0.95),
                target_length: 25.0,
                max_supersampling: 4,
            }
        })
    }
    #[func]
    pub fn hz_to_delta(hz: f64) -> f64 {
        1.0 / hz
    }
    #[func]
    pub fn delta_to_hz(delta: f64) -> f64 {
        1.0 / delta
    }
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgGravity {
    base: Base<Resource>,
    #[export]
    pub gravity_behavior: GravityBehavior,
    #[export]
    #[init(val = 1.0)]
    pub gravity_multiplier: f64,
}
impl FluoriteCastCfgGravity {
    pub fn new_config(gravity_behavior: GravityBehavior, gravity_multiplier: f64) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                gravity_behavior,
                gravity_multiplier,
            }
        })
    }
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                gravity_behavior: GravityBehavior::default(),
                gravity_multiplier: 1.0,
            }
        })
    }
}

////////////////

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastConfig {
    base: Base<Resource>,
    #[export]
    pub shape: Option<Gd<Shape3D>>,
    #[export(flags_3d_physics)]
    pub collision_mask: u32,
    #[export]
    pub max_alive_time: f64,
    #[export]
    pub max_total_length: f64,
    #[export]
    pub evaluate_mode: EvaluateMode,
    #[export]
    pub cast_gravity_cfg: Option<Gd<FluoriteCastCfgGravity>>,
    #[export]
    pub cast_fidelity_cfg: Option<Gd<FluoriteCastCfgFidelity>>,
    #[export]
    pub fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
    #[export]
    pub custom_data: VarDictionary,
    // TODO: start worrying about acutal ray/shapecasting soon
}

#[godot_api]
impl FluoriteCastConfig {
    #[func]
    pub fn new_config(
        shape: Gd<Shape3D>,
        collision_mask: u32,
        max_alive_time: f64,
        max_total_length: f64,
        evaluate_mode: EvaluateMode,
        &custom_data: VarDictionary, // probably don't want to move this, so get a reference and clone it like a Rc
        cast_gravity_cfg: Option<Gd<FluoriteCastCfgGravity>>,
        cast_fidelity_cfg: Option<Gd<FluoriteCastCfgFidelity>>,
        fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                shape: Some(shape.clone()),
                collision_mask,
                max_alive_time,
                max_total_length,
                evaluate_mode,
                cast_gravity_cfg: Some(cast_gravity_cfg.unwrap_or_else(|| FluoriteCastCfgGravity::new_default())),
                cast_fidelity_cfg: Some(cast_fidelity_cfg.unwrap_or_else(|| FluoriteCastCfgFidelity::new_default())),
                fluid_dynamics_cfg: Some(fluid_dynamics_cfg.unwrap_or_else(|| FluoriteCastCfgFluidDynamics::new_default())),
                custom_data: custom_data.clone(),
            }
        })
    }
    #[func]
    pub fn new_default_config(shape: Gd<Shape3D>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                shape: Some(shape.clone()),
                collision_mask: 0b1,
                max_alive_time: 15.0,
                max_total_length: 2000.0,
                evaluate_mode: EvaluateMode::default(),
                cast_gravity_cfg: Some(FluoriteCastCfgGravity::new_default()),
                cast_fidelity_cfg: Some(FluoriteCastCfgFidelity::new_default()),
                fluid_dynamics_cfg: Some(FluoriteCastCfgFluidDynamics::new_default()),
                custom_data: VarDictionary::new(),
            }
        })
    }
}
