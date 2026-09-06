use godot::{classes::{Curve, Shape3D}, prelude::*};

use super::fluorite_cast::{FluoriteCast, FluoriteSpaceCastResult};

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
    /// This works well enough for most arcade shooters
    #[default]
    OnlyAmbientAirspeed,
    /// Simulate ambient airspeed and simulate drag with the drag coefficient constant, reference area constant, airspeed, and fluid density
    /// This is good enough for most semi-realistic shooters
    DragCoefficient,
    /// Simulate ambient airspeed, and simulate drag with everything from DragCoefficient, and a multiplier based on the mach speed of the projectile
    /// This can approximate proper fluid dynamics more closely with a fine tuned mach-based curve
    DragCoefficientAndMach,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum CollisionDetectionMode {
    /// Do not check for collisions at all
    /// This will cause the projectile to clip into colliders and never fire hit signals or run hit callbacks
    Ignore,
    #[default]
    /// Collision check is done by raycasting
    ByRaycast,
    /// Collision check is done by shapecasting
    ByShapecast,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum AlwaysExecuteCodeVia {
    /// Use the associated FnMut assigned to the struct, panicking if not present
    /// Suitable for direct Rust usage
    #[default]
    ViaRustFnMut,
    /// Use a method named in snake_case implemented to a Godot Resource assigned to the struct,
    /// panicking if not present
    /// Suitable for GDScript usage
    ViaMethodOnResourceSnakeCase,
    /// Use a method named in PascalCase implemented to a Godot Resource assigned to the struct,
    /// panicking if not present
    /// Suitable for C# usage
    ViaMethodOnResourcePascalCase,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum MaybeExecuteCodeVia {
    /// Do not execute anything and always assume the default value in its place
    #[default]
    NeverExecute,
    /// Use the associated FnMut assigned to the struct, using the default value if not present
    /// Suitable for direct Rust usage
    ViaRustFnMut,
    /// Use a method named in snake_case implemented to an associated Godot Resource assigned to the struct,
    /// using the default value if not present
    /// Suitable for GDScript usage
    ViaMethodOnResourceSnakeCase,
    /// Use a method named in PascalCase implemented to a Godot Resource assigned to the struct,
    /// using the default value if not present
    /// Suitable for C# usage
    ViaMethodOnResourcePascalCase,
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
    #[init(val = 25.0)]
    pub projectile_reference_area_mm2: f64, // We assume the reference area is a constant so we don't have to do needlessly expensive realtime computation
    #[export]
    #[init(val = 0.5)]
    pub drag_coefficient: f64, // We assume the drag coefficient is *mostly* a constant for end user sanity
    #[export]
    pub mach_based_drag_multiplier: Option<Gd<Curve>>,
}
#[godot_api]
impl FluoriteCastCfgFluidDynamics {
    #[func]
    pub fn new_config(fluid_dynamics_behavior: FluidDynamicsBehavior, fluid_dynamics_fidelity: FluidDynamicsFidelity,
            projectile_reference_area_mm2: f64, drag_coefficient: f64, mach_based_drag_multiplier: Option<Gd<Curve>>)
        -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                fluid_dynamics_behavior,
                fluid_dynamics_fidelity,
                projectile_reference_area_mm2,
                drag_coefficient,
                mach_based_drag_multiplier,
            }
        })
    }
    #[func]
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                // Generic 5.56x45mm NATO bullet
                base,
                fluid_dynamics_behavior: FluidDynamicsBehavior::default(),
                fluid_dynamics_fidelity: FluidDynamicsFidelity::default(),
                projectile_reference_area_mm2: 25.0,
                drag_coefficient: 0.5,
                mach_based_drag_multiplier: None,
            }
        })
    }
    pub fn m2_to_mm2(m2: f64) -> f64 {
        m2 * 1000.0 * 1000.0
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
    #[func]
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
    #[func]
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
#[godot_api]
impl FluoriteCastCfgGravity {
    #[func]
    pub fn new_config(gravity_behavior: GravityBehavior, gravity_multiplier: f64) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                gravity_behavior,
                gravity_multiplier,
            }
        })
    }
    #[func]
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

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgHitDetection {
    base: Base<Resource>,
    #[export]
    pub collision_detection_mode: CollisionDetectionMode,
    #[export(flags_3d_physics)]
    #[init(val = u32::MAX)]
    pub hit_collision_mask: u32,
    #[export]
    pub exclude_list_paths_shallow: Array<NodePath>,
    #[export]
    pub exclude_list_paths_recursive: Array<NodePath>,
    #[export]
    #[init(val = false)]
    pub suppress_invalid_path_warnings: bool,
    #[export]
    #[init(val = None)]
    pub hit_shape: Option<Gd<Shape3D>>,
    #[export]
    pub shape_margin: f64,
    #[export]
    #[init(val = Basis::IDENTITY)]
    pub shape_basis: Basis,
    #[export]
    #[init(val = true)]
    pub should_hit_back_faces: bool,
    #[export]
    #[init(val = false)]
    pub should_hit_from_inside: bool,
    #[export]
    #[init(val = false)]
    pub should_collide_with_areas: bool,
    #[export]
    #[init(val = true)]
    pub should_collide_with_bodies: bool,
}
#[godot_api]
impl FluoriteCastCfgHitDetection {
    #[func]
    pub fn new_config(
        collision_detection_mode: CollisionDetectionMode,
        hit_collision_mask: u32,
        exclude_list_paths_shallow: Array<NodePath>,
        exclude_list_paths_recursive: Array<NodePath>,
        suppress_invalid_path_warnings: bool,
        hit_shape: Option<Gd<Shape3D>>,
        shape_margin: f64,
        shape_basis: Basis,
        should_hit_back_faces: bool,
        should_hit_from_inside: bool,
        should_collide_with_areas: bool,
        should_collide_with_bodies: bool,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode,
                hit_collision_mask,
                exclude_list_paths_shallow,
                exclude_list_paths_recursive,
                suppress_invalid_path_warnings,
                hit_shape,
                shape_margin,
                shape_basis,
                should_hit_back_faces,
                should_hit_from_inside,
                should_collide_with_areas,
                should_collide_with_bodies,
            }
        })
    }
    #[func]
    pub fn new_ignore() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::Ignore,
                hit_collision_mask: u32::MAX,
                exclude_list_paths_shallow: array![],
                exclude_list_paths_recursive: array![],
                suppress_invalid_path_warnings: false,
                hit_shape: None,
                shape_margin: 0.0,
                shape_basis: Basis::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
    #[func]
    pub fn new_by_ray(hit_collision_mask: u32, exclude_list_paths_shallow: Array<NodePath>, exclude_list_paths_recursive: Array<NodePath>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::ByRaycast,
                hit_collision_mask,
                exclude_list_paths_shallow,
                exclude_list_paths_recursive,
                suppress_invalid_path_warnings: false,
                hit_shape: None,
                shape_margin: 0.0,
                shape_basis: Basis::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
    #[func]
    pub fn new_by_shape(hit_collision_mask: u32, hit_shape: Gd<Shape3D>, exclude_list_paths_shallow: Array<NodePath>, exclude_list_paths_recursive: Array<NodePath>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::ByShapecast,
                hit_collision_mask,
                exclude_list_paths_shallow,
                exclude_list_paths_recursive,
                suppress_invalid_path_warnings: false,
                hit_shape: Some(hit_shape),
                shape_margin: 0.0,
                shape_basis: Basis::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
    #[func]
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::default(),
                hit_collision_mask: u32::MAX,
                exclude_list_paths_shallow: array![],
                exclude_list_paths_recursive: array![],
                suppress_invalid_path_warnings: false,
                hit_shape: None,
                shape_margin: 0.0,
                shape_basis: Basis::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgOnHit {
    base: Base<Resource>,
    #[export]
    /// Needs to implement:
    /// `try_penetrate` or `TryPenetrate` => signature `(FluoriteCast, FluoriteSpaceCastResult) -> bool`
    pub on_hit_methods_holder: Option<Gd<Resource>>,

    #[export]
    pub try_penetrate_via: MaybeExecuteCodeVia,
    pub try_penetrate_rs: Option<Box<dyn FnMut(Gd<FluoriteCast>, Gd<FluoriteSpaceCastResult>) -> bool>>,

    #[export]
    #[init(val = true)]
    pub auto_queue_free_on_terminate: bool,
}
#[godot_api]
impl FluoriteCastCfgOnHit {
    #[func]
    fn new_config(
        on_hit_methods_holder: Option<Gd<Resource>>,
        try_penetrate_via: MaybeExecuteCodeVia,
        auto_queue_free_on_terminate: bool,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                on_hit_methods_holder,
                try_penetrate_via,
                try_penetrate_rs: None,
                auto_queue_free_on_terminate,
            }
        })
    }
    #[func]
    fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                on_hit_methods_holder: None,
                try_penetrate_via: MaybeExecuteCodeVia::default(),
                try_penetrate_rs: None,
                auto_queue_free_on_terminate: true,
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
    #[init(val = u32::MAX)]
    pub area_collision_mask: u32,
    #[export]
    #[init(val = 15.0)]
    pub max_alive_time: f64,
    #[export]
    #[init(val = 2000.0)]
    pub max_total_length: f64,
    #[export]
    pub evaluate_mode: EvaluateMode,
    #[export]
    pub cast_gravity_cfg: Option<Gd<FluoriteCastCfgGravity>>,
    #[export]
    pub cast_fidelity_cfg: Option<Gd<FluoriteCastCfgFidelity>>,
    #[export]
    pub cast_fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
    #[export]
    pub cast_hit_detection_cfg: Option<Gd<FluoriteCastCfgHitDetection>>,
    #[export]
    pub cast_on_hit_cfg: Option<Gd<FluoriteCastCfgOnHit>>,
    #[export]
    pub custom_data: VarDictionary,
}

#[godot_api]
impl FluoriteCastConfig {
    #[func]
    pub fn new_config(
        shape: Gd<Shape3D>,
        area_collision_mask: u32,
        max_alive_time: f64,
        max_total_length: f64,
        evaluate_mode: EvaluateMode,
        &custom_data: VarDictionary, // probably don't want to move this, so get a reference and clone it like a Rc
        cast_gravity_cfg: Option<Gd<FluoriteCastCfgGravity>>,
        cast_fidelity_cfg: Option<Gd<FluoriteCastCfgFidelity>>,
        cast_fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
        cast_hit_detection_cfg: Option<Gd<FluoriteCastCfgHitDetection>>,
        cast_on_hit_cfg: Option<Gd<FluoriteCastCfgOnHit>>,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                shape: Some(shape.clone()),
                area_collision_mask,
                max_alive_time,
                max_total_length,
                evaluate_mode,
                cast_gravity_cfg: Some(cast_gravity_cfg.unwrap_or_else(|| FluoriteCastCfgGravity::new_default())),
                cast_fidelity_cfg: Some(cast_fidelity_cfg.unwrap_or_else(|| FluoriteCastCfgFidelity::new_default())),
                cast_fluid_dynamics_cfg: Some(cast_fluid_dynamics_cfg.unwrap_or_else(|| FluoriteCastCfgFluidDynamics::new_default())),
                cast_hit_detection_cfg: Some(cast_hit_detection_cfg.unwrap_or_else(|| FluoriteCastCfgHitDetection::new_default())),
                cast_on_hit_cfg: Some(cast_on_hit_cfg.unwrap_or_else(|| FluoriteCastCfgOnHit::new_default())),
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
                area_collision_mask: u32::MAX,
                max_alive_time: 15.0,
                max_total_length: 2000.0,
                evaluate_mode: EvaluateMode::default(),
                cast_gravity_cfg: Some(FluoriteCastCfgGravity::new_default()),
                cast_fidelity_cfg: Some(FluoriteCastCfgFidelity::new_default()),
                cast_fluid_dynamics_cfg: Some(FluoriteCastCfgFluidDynamics::new_default()),
                cast_hit_detection_cfg: Some(FluoriteCastCfgHitDetection::new_default()),
                cast_on_hit_cfg: Some(FluoriteCastCfgOnHit::new_default()),
                custom_data: VarDictionary::new(),
            }
        })
    }
}
