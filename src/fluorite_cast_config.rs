use godot::{classes::{Curve, Shape3D}, prelude::*};

use super::fluorite_cast::{FluoriteCast, FluoriteSpaceCastResult};

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum EvaluateMode {
    #[default]
    /// Automatically evaluate the cast every `physics_process`.
    PhysicsProcess,
    /// Automatically evaluate the cast every `process`.
    Process,
    /// Do not automatically evaluate the cast. Calling `evaluate` manually is needed.
    Manual,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum ProjectileLookBehavior {
    #[default]
    /// The cast will face towards the velocity vector every time it is evaluated.
    FollowVelocity,
    /// The cast will have to be reoriented manually.
    Manual,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum SuperSamplingMode {
    /// Never supersample.
    Never,
    #[default]
    /// Supersample only if `delta` is too long, when `evaluate` is called.
    IfAboveTargetDelta,
    /// Supersample only if the calculated length on a `evaluate` call is too long.
    IfTooLong,
    /// Supersample if `delta` or the calculated length is too long when `evaluate` is caled.
    IfAboveTargetDeltaOrTooLong,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum GravityBehavior {
    /// Ignore all gravity.
    Ignore,
    #[default]
    /// Use global gravity as a constant.
    UseGlobalGravityCached,
    /// Query global gravity every operation.
    UseGlobalGravityRealTime,
    /// Query local gravity every operation.
    UseCurrentGravityRealTime,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum FluidDynamicsBehavior {
    /// Ignore all fluids.
    Ignore,
    /// Use global fluid as a constant.
    #[default]
    UseGlobalFluidCached,
    /// Query global fluid every operation.
    UseGlobalFluidRealTime,
    /// Query current fluid every operation.
    UseCurrentFluidRealTime,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum FluidDynamicsFidelity {
    /// Do not simulate fluid dynamics at all.
    Ignore,
    /// Simulate acceleration from ambient airspeed, but do not simulate drag.
    /// 
    /// This works well enough for most arcade shooters.
    #[default]
    OnlyAmbientAirspeed,
    /// Simulate ambient airspeed and simulate drag with the drag coefficient constant,
    /// reference area constant, airspeed, and fluid density.
    /// 
    /// This is good enough for most semi-realistic shooters.
    DragCoefficient,
    /// Simulate ambient airspeed, and simulate drag with everything from `DragCoefficient`,
    /// and a multiplier based on the mach speed of the projectile.
    /// 
    /// This can approximate proper fluid dynamics more closely with a fine tuned mach-based curve.
    DragCoefficientAndMach,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum CollisionDetectionMode {
    /// Do not check for collisions at all.
    /// 
    /// This will cause the projectile to clip into colliders, and never fire hit signals or run hit callbacks.
    Ignore,
    #[default]
    /// Collision check is done by raycasting.
    ByRaycast,
    /// Collision check is done by shapecasting.
    ByShapecast,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum AlwaysExecuteCodeVia {
    /// Use the associated FnMut assigned to the struct, panicking if not present.
    /// 
    /// Suitable for direct Rust usage.
    #[default]
    ViaRustFnMut,
    /// Use a method named in snake_case implemented to a Godot Resource assigned to the struct,
    /// panicking if not present.
    /// 
    /// Suitable for GDScript usage.
    ViaMethodOnResourceSnakeCase,
    /// Use a method named in PascalCase implemented to a Godot Resource assigned to the struct,
    /// panicking if not present.
    /// 
    /// Suitable for C# usage.
    ViaMethodOnResourcePascalCase,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum MaybeExecuteCodeVia {
    /// Do not execute anything and always assume the default value in its place.
    #[default]
    NeverExecute,
    /// Use the associated FnMut assigned to the struct, using the default value if not present.
    /// 
    /// Suitable for direct Rust usage.
    ViaRustFnMut,
    /// Use a method named in snake_case implemented to an associated Godot Resource assigned to the struct,
    /// using the default value if not present.
    /// 
    /// Suitable for GDScript usage.
    ViaMethodOnResourceSnakeCase,
    /// Use a method named in PascalCase implemented to a Godot Resource assigned to the struct,
    /// using the default value if not present.
    /// 
    /// Suitable for C# usage.
    ViaMethodOnResourcePascalCase,
}

////////////////

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgFluidDynamics {
    base: Base<Resource>,
    #[export]
    /// How the cast will sample the local fluid.
    pub fluid_dynamics_behavior: FluidDynamicsBehavior,
    #[export]
    /// The fidelity of the fluid dynamics simulation for the cast.
    /// 
    /// Only relevant if `fluid_dynamics_behavior` is `DragCoefficient` or higher.
    pub fluid_dynamics_fidelity: FluidDynamicsFidelity,
    #[export]
    #[init(val = 25.0)]
    /// The reference area of the cast, in mm^2. Used for computing drag.
    /// 
    /// Only relevant if `fluid_dynamics_behavior` is `DragCoefficient` or higher.
    pub projectile_reference_area_mm2: f64, // We assume the reference area is a constant so we don't have to do needlessly expensive realtime computation
    #[export]
    #[init(val = 0.5)]
    /// The drag coefficient of the cast, used for computing drag.
    /// 
    /// Only relevant if `fluid_dynamics_behavior` is `DragCoefficient` or higher.
    pub drag_coefficient: f64, // We assume the drag coefficient is *mostly* a constant for end user sanity
    #[export]
    /// The mach-based drag multiplier for the cast.
    /// 
    /// Only relevant if `fluid_dynamics_behavior` is `DragCoefficientAndMach`.
    /// It can be None (null) if otherwise.
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
    /// Always use this instead of `Self.new()`.
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
    /// How the cast will supersample.
    pub super_sampling_mode: SuperSamplingMode,
    #[export]
    #[init(val = Self::delta_to_hz(120.0 * 0.95))]
    /// The target `delta` for every `evaluate` call made.
    pub target_delta: f64,
    #[export]
    #[init(val = 25.0)]
    /// The target calculated length for every `evaluate` call made
    pub target_length: f64,
    #[export]
    #[init(val = 4)]
    /// The maximum limit of how many segments the cast can supersample to.
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
    /// Always use this instead of `Self.new()`.
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
    /// How the cast respects gravity.
    pub gravity_behavior: GravityBehavior,
    #[export]
    #[init(val = 1.0)]
    /// The multiplier to the gravity that the cast experiences.
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
    /// Always use this instead of `Self.new()`.
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
    /// How the cast will detect collisions.
    pub collision_detection_mode: CollisionDetectionMode,
    #[export(flags_3d_physics)]
    #[init(val = u32::MAX)]
    /// The collision mask for detecting collisions.
    pub hit_collision_mask: u32,
    #[export]
    /// What nodes to ignore when checking collisions.
    pub exclude_list_paths_shallow: Array<NodePath>,
    #[export]
    /// What nodes to ignore when checking collisions.
    /// 
    /// Will resolve recursively at runtime.
    pub exclude_list_paths_recursive: Array<NodePath>,
    #[export]
    #[init(val = false)]
    /// Suppress warning about invalid node paths passed to `exclude_list_paths_*`.
    pub suppress_invalid_path_warnings: bool,
    #[export]
    #[init(val = None)]
    /// The collision shape of the shapecast. This must be Some (non-null)
    /// if `collision_detection_mode` is `ByShapecast`.
    /// 
    /// Otherwise, this property is irrelevant.
    pub hit_shape: Option<Gd<Shape3D>>,
    #[export]
    /// The shape margin of the shapecast.
    /// 
    /// This property is only relevant if `collision_detection_mode` is `ByShapecast`.
    pub shape_margin: f64,
    #[export]
    #[init(val = Basis::IDENTITY)]
    /// The basis of the shapecast.
    /// 
    /// This property is only relevant if `collision_detection_mode` is `ByShapecast`.
    pub shape_basis: Basis,
    #[export]
    #[init(val = true)]
    /// If collision checks should respect back faces.
    pub should_hit_back_faces: bool,
    #[export]
    #[init(val = false)]
    /// If collision checks should hit from inside.
    pub should_hit_from_inside: bool,
    #[export]
    #[init(val = false)]
    /// If collision checks should respect `Area3D`s.
    pub should_collide_with_areas: bool,
    #[export]
    #[init(val = true)]
    /// If collision checks should respect `PhysicsBody3D`s.
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
    /// Always use this instead of `Self.new()`.
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
pub struct FluoriteCastCfgBuiltinFlags {
    base: Base<Resource>,
    #[export]
    #[init(val = true)]
    /// If the cast should use the builtin penetration system.
    pub builtin_penetration: bool,
}
#[godot_api]
impl FluoriteCastCfgBuiltinFlags {
    #[func]
    pub fn new_config(
        builtin_penetration: bool,
    ) -> Gd<FluoriteCastCfgBuiltinFlags> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                builtin_penetration,
            }
        })
    }
    #[func]
    /// Always use this instead of `Self.new()`.
    pub fn new_default() -> Gd<FluoriteCastCfgBuiltinFlags> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                builtin_penetration: true,
            }
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgMethods {
    base: Base<Resource>,
    #[export]
    /// Depending on `*_via` settings, the instance provided needs to implement:
    /// 
    /// `try_penetrate` or `TryPenetrate` => signature `(FluoriteCast, FluoriteSpaceCastResult) -> bool`
    /// 
    /// `cast_raw_evaluated` or `CastRawEvaluated` => signature `(FluoriteCast, Vector3, float, bool) -> void`
    /// 
    /// `on_new_cast` or `OnNewCast` => signature `(FluoriteCast) -> void`
    /// 
    /// This field must always be `Some` (non-`null`) **if any `*_via` properties are NOT Ignore nor ViaRustFnMut**.
    /// If this invariant is broken, the cast will panic.
    pub methods_holder: Option<Gd<Resource>>,

    #[export]
    /// If this is None (null in Godot), then all builtin custom behavior will be disabled.
    /// 
    /// If this is Some, then builtin custom behavior will be selectively enabled.
    /// Make sure to pass in `FluoriteBuiltinConfig` to `FluoriteCastConfig.custom_config` with the key as `"__builtin"`.
    pub builtin_flags: Option<Gd<FluoriteCastCfgBuiltinFlags>>,

    #[export]
    /// Where to find and use the callback `try_penetrate`.
    pub try_penetrate_via: MaybeExecuteCodeVia,
    /// The callback for `try_penetrate`, if using the `ViaRustFnMut` mode.
    pub try_penetrate_rs: Option<Box<dyn FnMut(&mut FluoriteCast, Gd<FluoriteSpaceCastResult>) -> bool>>,

    #[export]
    /// Where to find and use the callback `cast_raw_evaluated`.
    pub cast_raw_evaluated_via: MaybeExecuteCodeVia,
    /// The callback for `cast_raw_evaluated`, if using the `ViaRustFnMut` mode.
    pub cast_raw_evaluated_rs: Option<Box<dyn FnMut(&mut FluoriteCast, Vector3, f64, bool) -> ()>>,

    #[export]
    /// Where to find and use the callback `on_new_cast`.
    pub on_new_cast_via: MaybeExecuteCodeVia,
    /// The callback for `on_new_cast`, if using the `ViaRustFnMut` mode.
    /// 
    /// If `self.config.cast_methods_cfg.builtin_flags` is None, then `self.custom_data_rs` will be None.
    /// If it's Some, then `self.custom_data_rs` will be populated as Some.
    /// 
    /// If you want to make use of the `custom_data_rs` property, make sure to initialize it in this FnMut if it's not already.
    pub on_new_cast_rs: Option<Box<dyn FnMut(&mut FluoriteCast) -> ()>>,

    #[export]
    #[init(val = true)]
    /// If this is false, you need to call `cleanup` on the instance manually when `expired` or `terminated` fires.
    /// 
    /// Do not use `queue_free` directly, unless you need to bypass firing the `freeing` signal for some reason.
    pub auto_queue_free_on_terminate: bool,
}
#[godot_api]
impl FluoriteCastCfgMethods {
    #[func]
    pub fn new_config(
        methods_holder: Option<Gd<Resource>>,
        auto_queue_free_on_terminate: bool,
        builtin_flags: Option<Gd<FluoriteCastCfgBuiltinFlags>>,
        try_penetrate_via: MaybeExecuteCodeVia,
        cast_raw_evaluated_via: MaybeExecuteCodeVia,
        on_new_cast_via: MaybeExecuteCodeVia,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                methods_holder,
                builtin_flags,
                try_penetrate_via,
                try_penetrate_rs: None,
                cast_raw_evaluated_via,
                cast_raw_evaluated_rs: None,
                on_new_cast_via,
                on_new_cast_rs: None,
                auto_queue_free_on_terminate,
            }
        })
    }
    #[func]
    /// Always use this instead of `Self.new()`.
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                methods_holder: None,
                builtin_flags: None,
                try_penetrate_via: MaybeExecuteCodeVia::default(),
                try_penetrate_rs: None,
                cast_raw_evaluated_via: MaybeExecuteCodeVia::default(),
                cast_raw_evaluated_rs: None,
                on_new_cast_via: MaybeExecuteCodeVia::default(),
                on_new_cast_rs: None,
                auto_queue_free_on_terminate: true,
            }
        })
    }

    /// Injection of `*_rs` properties is possible with this constructor.
    /// Available on Rust-side only.
    pub fn new_config_rs(
        methods_holder: Option<Gd<Resource>>,
        auto_queue_free_on_terminate: bool,
        builtin_flags: Option<Gd<FluoriteCastCfgBuiltinFlags>>,
        try_penetrate_via: MaybeExecuteCodeVia,
        try_penetrate_rs: Option<Box<dyn FnMut(&mut FluoriteCast, Gd<FluoriteSpaceCastResult>) -> bool>>,
        cast_raw_evaluated_via: MaybeExecuteCodeVia,
        cast_raw_evaluated_rs: Option<Box<dyn FnMut(&mut FluoriteCast, Vector3, f64, bool) -> ()>>,
        on_new_cast_via: MaybeExecuteCodeVia,
        on_new_cast_rs: Option<Box<dyn FnMut(&mut FluoriteCast) -> ()>>,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                methods_holder,
                builtin_flags,
                try_penetrate_via,
                try_penetrate_rs,
                cast_raw_evaluated_via,
                cast_raw_evaluated_rs,
                on_new_cast_via,
                on_new_cast_rs,
                auto_queue_free_on_terminate,
            }
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastCfgGeneral {
    base: Base<Resource>,
    #[export]
    /// This field is nullable. If it's None (null),
    /// then a pointcast will be done instead of a shapecast
    /// to test for Area3Ds.
    pub area_collision_shape: Option<Gd<Shape3D>>,
    #[export]
    /// The basis for area collision checks.
    /// Only relevant when area_collision_shape is Some.
    pub area_collision_basis: Basis,
    #[export(flags_3d_physics)]
    #[init(val = u32::MAX)]
    /// The collision mask for detecting areas.
    pub area_collision_mask: u32,
    #[export]
    #[init(val = 15.0)]
    /// How long the cast can live for before expiring.
    pub max_alive_time: f64,
    #[export]
    #[init(val = 2000.0)]
    /// How far the cast can travel before expiring.
    pub max_total_length: f64,
    #[export]
    #[init(val = ProjectileLookBehavior::default())]
    /// How the projectile will change its orientation every `evaluate` call.
    pub projectile_look_behavior: ProjectileLookBehavior,
}
#[godot_api]
impl FluoriteCastCfgGeneral {
    #[func]
    pub fn new_config(
        projectile_look_behavior: ProjectileLookBehavior,
        area_collision_shape: Option<Gd<Shape3D>>,
        area_collision_basis: Basis,
        area_collision_mask: u32,
        max_alive_time: f64,
        max_total_length: f64
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                area_collision_shape,
                area_collision_mask,
                area_collision_basis,
                max_alive_time,
                max_total_length,
                projectile_look_behavior,
            }
        })
    }
    #[func]
    /// Always use this instead of `Self.new()`.
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                area_collision_shape: None,
                area_collision_mask: u32::MAX,
                area_collision_basis: Basis::IDENTITY,
                max_alive_time: 15.0,
                max_total_length: 2000.0,
                projectile_look_behavior: ProjectileLookBehavior::default(),
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
    /// When the cast will evaluate.
    pub evaluate_mode: EvaluateMode,
    #[export]
    pub cast_general_cfg: Option<Gd<FluoriteCastCfgGeneral>>,
    #[export]
    pub cast_gravity_cfg: Option<Gd<FluoriteCastCfgGravity>>,
    #[export]
    pub cast_fidelity_cfg: Option<Gd<FluoriteCastCfgFidelity>>,
    #[export]
    pub cast_fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
    #[export]
    pub cast_hit_detection_cfg: Option<Gd<FluoriteCastCfgHitDetection>>,
    #[export]
    /// **This field must always be `Some`** (non-`null`).
    /// If this invariant is broken, the cast will panic.
    pub cast_methods_cfg: Option<Gd<FluoriteCastCfgMethods>>,
    #[export]
    /// If you need to add FluoriteBuiltinConfig, add it with the key as `"__builtin"`.
    pub custom_config: Dictionary<GString, Option<Gd<Resource>>>,
}

#[godot_api]
impl FluoriteCastConfig {
    #[func]
    pub fn new_config(
        evaluate_mode: EvaluateMode,
        &custom_config: Dictionary<GString, Option<Gd<Resource>>>, // probably don't want to move this, so get a reference and clone it like a Rc
        cast_general_cfg: Option<Gd<FluoriteCastCfgGeneral>>,
        cast_gravity_cfg: Option<Gd<FluoriteCastCfgGravity>>,
        cast_fidelity_cfg: Option<Gd<FluoriteCastCfgFidelity>>,
        cast_fluid_dynamics_cfg: Option<Gd<FluoriteCastCfgFluidDynamics>>,
        cast_hit_detection_cfg: Option<Gd<FluoriteCastCfgHitDetection>>,
        cast_methods_cfg: Option<Gd<FluoriteCastCfgMethods>>,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                evaluate_mode,
                cast_general_cfg: Some(cast_general_cfg.unwrap_or_else(|| FluoriteCastCfgGeneral::new_default())),
                cast_gravity_cfg: Some(cast_gravity_cfg.unwrap_or_else(|| FluoriteCastCfgGravity::new_default())),
                cast_fidelity_cfg: Some(cast_fidelity_cfg.unwrap_or_else(|| FluoriteCastCfgFidelity::new_default())),
                cast_fluid_dynamics_cfg: Some(cast_fluid_dynamics_cfg.unwrap_or_else(|| FluoriteCastCfgFluidDynamics::new_default())),
                cast_hit_detection_cfg: Some(cast_hit_detection_cfg.unwrap_or_else(|| FluoriteCastCfgHitDetection::new_default())),
                cast_methods_cfg: Some(cast_methods_cfg.unwrap_or_else(|| FluoriteCastCfgMethods::new_default())),
                custom_config: custom_config.clone(),
            }
        })
    }
    #[func]
    /// Always use this instead of `Self.new()`.
    pub fn new_default() -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                evaluate_mode: EvaluateMode::default(),
                cast_general_cfg: Some(FluoriteCastCfgGeneral::new_default()),
                cast_gravity_cfg: Some(FluoriteCastCfgGravity::new_default()),
                cast_fidelity_cfg: Some(FluoriteCastCfgFidelity::new_default()),
                cast_fluid_dynamics_cfg: Some(FluoriteCastCfgFluidDynamics::new_default()),
                cast_hit_detection_cfg: Some(FluoriteCastCfgHitDetection::new_default()),
                cast_methods_cfg: Some(FluoriteCastCfgMethods::new_default()),
                custom_config: Dictionary::new(),
            }
        })
    }
}
