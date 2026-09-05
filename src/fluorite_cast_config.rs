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
    pub exclude_list_paths: Array<NodePath>,
    #[export]
    #[init(val = None)]
    pub hit_shape: Option<Gd<Shape3D>>,
    #[export]
    pub shape_margin: f64,
    #[export]
    #[init(val = Transform3D::IDENTITY)]
    pub shape_transform: Transform3D,
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
impl FluoriteCastCfgHitDetection {
    pub fn new_config(
        collision_detection_mode: CollisionDetectionMode,
        hit_collision_mask: u32,
        exclude_list_paths: Array<NodePath>,
        hit_shape: Option<Gd<Shape3D>>,
        shape_margin: f64,
        shape_transform: Transform3D,
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
                exclude_list_paths,
                hit_shape,
                shape_margin,
                shape_transform,
                should_hit_back_faces,
                should_hit_from_inside,
                should_collide_with_areas,
                should_collide_with_bodies,
            }
        })
    }
    pub fn new_ignore() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::Ignore,
                hit_collision_mask: u32::MAX,
                exclude_list_paths: array![],
                hit_shape: None,
                shape_margin: 0.0,
                shape_transform: Transform3D::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
    pub fn new_by_ray(hit_collision_mask: u32, exclude_list_paths: Array<NodePath>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::ByRaycast,
                hit_collision_mask,
                exclude_list_paths,
                hit_shape: None,
                shape_margin: 0.0,
                shape_transform: Transform3D::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
    pub fn new_by_shape(hit_collision_mask: u32, exclude_list_paths: Array<NodePath>, hit_shape: Gd<Shape3D>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::ByShapecast,
                hit_collision_mask,
                exclude_list_paths,
                hit_shape: Some(hit_shape),
                shape_margin: 0.0,
                shape_transform: Transform3D::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
            }
        })
    }
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                collision_detection_mode: CollisionDetectionMode::default(),
                hit_collision_mask: u32::MAX,
                exclude_list_paths: array![],
                hit_shape: None,
                shape_margin: 0.0,
                shape_transform: Transform3D::IDENTITY,
                should_hit_back_faces: true,
                should_hit_from_inside: false,
                should_collide_with_areas: false,
                should_collide_with_bodies: true,
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
    pub cast_fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
    #[export]
    pub cast_hit_detection_cfg: Option<Gd<FluoriteCastCfgHitDetection>>,
    #[export]
    pub custom_data: VarDictionary,
    // TODO: start worrying about acutal ray/shapecasting soon
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
                custom_data: VarDictionary::new(),
            }
        })
    }
}
