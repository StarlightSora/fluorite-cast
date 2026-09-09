pub mod builtins;

// If you're unfamiliar with godot-rust:
//
// Yes, this whole thing is a bit of a mess, and definitely far from idiomatic Rust.
// However this is *necessary* Rust because of how dynamic Godot is.
// In particular: every property exposed to Godot that holds an Object needs to be nullable.
// Every Variant needs to be downcasted to some concrete type.
// Though, we know that we can cast away all these safely as long as all the invariants are enforced by us and the caller.
// Should an invariant be broken, a runtime panic will occur, and be printed out to Godot's output log.

const MAX_CAST_RESULTS: i32 = 8;

use godot::{
    classes::{
        Area3D,
        PhysicsPointQueryParameters3D,
        PhysicsRayQueryParameters3D,
        PhysicsShapeQueryParameters3D,
        ProjectSettings,
        area_3d::SpaceOverride,
    }, global::{
        ceilf,
        push_warning
    }, meta::conv::ObjectToOwned,
    prelude::*,
};
use hashbrown::HashMap;
use core::cmp::max;
use core::any::Any;

use super::fluorite_fluid_area3d::FluoriteFluidArea3D;
use super::fluorite_fluid_config::FluoriteFluidConfig;
use super::fluorite_cast_config::{
    FluoriteCastConfig,
    EvaluateMode,
    SuperSamplingMode,
    GravityBehavior,
    FluidDynamicsBehavior,
    FluidDynamicsFidelity,
    CollisionDetectionMode,
    MaybeExecuteCodeVia,
    FluoriteCastCfgHitDetection,
    ProjectileLookBehavior,
};

// Intermediate, type-safe representation for raycast and shapecast results
enum SpaceCastResult {
    HitNothing,
    HitByRaycast(VarDictionary, Vector3),
    HitByShapecast(VarDictionary, Vector3),
}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
/// Type-safe representation of point/ray/shapecast results.
pub struct FluoriteSpaceCastResult {
    base: Base<RefCounted>,
    #[var]
    /// Global position of where the cast hit.
    pub position: Vector3,
    #[var]
    /// The object's surface normal at the intersection point,
    /// or Vector3(0, 0, 0) if the ray starts inside the shape and `PhysicsRayQueryParameters3D.hit_from_inside` is `true`.
    pub normal: Vector3,
    #[var]
    /// The intersecting object's RID.
    pub rid: i64,
    #[var]
    /// The colliding object.
    pub collider: Option<Gd<Node3D>>,
    #[var]
    /// The ID of the colliding object.
    pub collider_id: i64,
    #[var]
    /// The shape index of the colliding shape.
    pub shape: i64,
    #[var]
    /// How far the cast travelled from the origin for it to hit something.
    pub march_by: Vector3,
}

#[godot_api]
impl FluoriteSpaceCastResult {
    #[func]
    /// Construct a new `FluoriteSpaceCastResult`.
    /// Always use this instead of `FluoriteSpaceCastResult.new()`.
    pub fn new_result(
        position: Vector3,
        normal: Vector3,
        rid: i64,
        collider: Option<Gd<Node3D>>,
        collider_id: i64,
        shape: i64,
        march_by: Vector3,
    ) -> Gd<FluoriteSpaceCastResult> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                position,
                normal,
                rid,
                collider,
                collider_id,
                shape,
                march_by,
            }
        })
    }
}

#[derive(GodotClass)]
#[class(init, base=Node3D)]
/// The core type of this library.
/// 
/// It works by manually simulating physics, bypassing inconsistency of the physics engine entirely,
/// while making the projectile itself not affect physics objects in the scene directly (i.e. by causing a physics collision, moving a `RigidBody3D`).
pub struct FluoriteCast {
    base: Base<Node3D>,
    gravity_cache: Option<Vector3>,
    ambient_airspeed_cache: Option<Vector3>,
    speed_of_sound_cache: Option<f64>,
    fluid_drag_const_cache: Option<f64>,
    disabled: bool,
    is_cleaning_up: bool,
    config: Gd<FluoriteCastConfig>,
    payload_node: Option<Gd<Node3D>>,
    #[var]
    /// The current velocity of the cast.
    /// This can be arbitrarily written to if modification of the velocity is desired.
    pub current_velocity: Vector3,
    #[var]
    /// The current acceleration of the cast.
    /// 
    /// This applies **on top of implicit factors** such as gravity, wind and drag.
    /// Implicit factors are not reflected in this property.
    /// 
    /// This can be arbitrarily written to if modification of the acceleration is desired.
    pub current_acceleration: Vector3,
    #[var]
    /// How far the projectile traveled.
    /// 
    /// While safe to write be written to arbitrarily, there is usually no reason to do so.
    pub distance_covered: f32,
    #[var]
    /// How long the projectile existed.
    /// 
    /// While safe to write be written to arbitrarily, there is usually no reason to do so.
    pub alive_for: f64,
    #[var]
    /// What the projectile considers as the global fluid.
    /// 
    /// **This field must always be `Some`** (non-`null`).
    /// If this invariant is broken, the cast will panic.
    pub global_fluid: Option<Gd<FluoriteFluidConfig>>,
    #[var]
    /// Arbitrary data assigned to the cast.
    /// 
    /// This can be arbitrarily read from and written to as it fits the caller's needs.
    pub custom_data: VarDictionary,
    #[var]
    /// The `PhysicsRayQueryParameters3D` cache for raycasting, if relevant.
    /// 
    /// There is usually no reason to write on this as the caller. Removing the cache arbitrarily may result in a panic.
    pub query_params_cache_ray: Option<Gd<PhysicsRayQueryParameters3D>>,
    #[var]
    /// The `PhysicsShapeQueryParameters3D` cache for shapecasting, if relevant.
    /// 
    /// There is usually no reason to write on this as the caller. Removing the cache arbitrarily may result in a panic.
    pub query_params_cache_shape: Option<Gd<PhysicsShapeQueryParameters3D>>,
    #[var]
    /// The `PhysicsPointQueryParameters3D` cache for testing for `Area3D`s, if relevant.
    /// 
    /// There is usually no reason to write on this as the caller. Removing the cache arbitrarily may result in a panic.
    pub area_test_cache_point: Option<Gd<PhysicsPointQueryParameters3D>>,
    #[var]
    /// The `PhysicsShapeQueryParameters3D` cache for testing for `Area3D`s, if relevant.
    /// 
    /// There is usually no reason to write on this as the caller. Removing the cache arbitrarily may result in a panic.
    pub area_test_cache_shape: Option<Gd<PhysicsShapeQueryParameters3D>>,
    /// Meant as an alternative to `custom_data` if you use Rust to use this library, as it is generally faster and more type safe.
    /// 
    /// Since we cannot use generic types on structs registered to Godot, we upcast to Any in the struct definition
    /// and downcast whenever it has to be read and written.
    /// 
    /// The builtin data type, if enabled, will be registered as the key `String::new("__builtin")` and the value resolves to the type `FluoriteBuiltinState`.
    /// This type contains all the information that builtin callbacks act on.
    /// 
    /// This property is lazily populated; in particular, if `self.config.cast_methods_cfg.builtin_flags` is None, then this will be None.
    /// If it's Some, then this will be populated as Some, with the above mentioned key occupied.
    /// 
    /// See the source code of `super::builtins` to see how to upcast and downcast on this property if you're unsure.
    pub custom_data_rs: Option<HashMap<String, Box<dyn Any>>>,
}

#[godot_api]
impl FluoriteCast {
    #[signal]
    /// Fired when the projectile penetrates - that is, the projectile hit a collider, but it has decided to tunnel through it.
    pub fn penetrated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);
    #[signal]
    /// Fired when the projectile terminates - that is, the projectile hit a collider, and it has decided to stop existing.
    pub fn terminated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);
    #[signal]
    /// Fired when the projectile expires - that is, the projectile traveled too long or lived too long.
    pub fn expired(this: Gd<FluoriteCast>);
    #[signal]
    /// Fired when the projectile is about to be freed.
    pub fn freeing(this: Gd<FluoriteCast>);

    #[func]
    /// Constructs a new `FluoriteCast`.
    /// Always use this instead of `FluoriteCast.new()`.
    /// 
    /// Unless you're explicitly making an ad-hoc cast, prefer casting on behalf of `FluoriteCastFactory` instead.
    pub fn new_cast(
        &mut parent_to: Gd<Node3D>, payload: Option<Gd<Node3D>>, config: Gd<FluoriteCastConfig>, global_fluid: Gd<FluoriteFluidConfig>, custom_data: VarDictionary) -> Gd<Self> {
        let mut new_node = Gd::from_init_fn(|base| {
            Self {
                base,
                payload_node: None,
                gravity_cache: None,
                ambient_airspeed_cache: None,
                speed_of_sound_cache: None,
                fluid_drag_const_cache: None,
                config: config.clone(), // We clone because Gd<T> is basically a Rc<RefCell<T>>
                current_velocity: Vector3::ZERO,
                current_acceleration: Vector3::ZERO,
                distance_covered: 0.0f32,
                alive_for: 0.0f64,
                global_fluid: Some(global_fluid),
                custom_data,
                disabled: false,
                is_cleaning_up: false,
                query_params_cache_ray: None,
                query_params_cache_shape: None,
                area_test_cache_point: None,
                area_test_cache_shape: None,
                custom_data_rs: None,
            }
        });
        // The base needs to be `StaticBody3D`, and a `CollisionShape3D` is needed so we can use `get_gravity` for `UseCurrentGravityRealTime` mode
        //let mut gd_colshape3d = None;

        let mut new_node_bind = new_node.bind_mut();
        let cfg_binding = new_node_bind.config.bind();
        let mut area3d_test_needed = false;
        let cast_general_cfg_binding = cfg_binding.cast_general_cfg.as_ref().expect("cast_general_cfg should always exist").bind();
        let collision_shape_data = cast_general_cfg_binding.area_collision_shape.clone();
        let collision_mask_data = cast_general_cfg_binding.area_collision_mask;
        let cast_gravity_cfg_binding = cfg_binding.cast_gravity_cfg.as_ref().expect("cast_gravity_cfg should always exist").bind();
        let gravity_behavior = cast_gravity_cfg_binding.gravity_behavior;
        let gravity_multiplier = cast_gravity_cfg_binding.gravity_multiplier;
        let fluid_dynamics_behavior = cfg_binding.cast_fluid_dynamics_cfg.as_ref().expect("cast_fluid_dynamics_cfg should always exist").bind().fluid_dynamics_behavior;
        let hit_detection_cfg_bind = cfg_binding.cast_hit_detection_cfg.as_ref().expect("cast_hit_detection_cfg should always exist").bind();
        let collision_detection_mode = hit_detection_cfg_bind.collision_detection_mode;
        let suppress_invalid_path_warnings = hit_detection_cfg_bind.suppress_invalid_path_warnings;
        drop(cast_gravity_cfg_binding);
        drop(cast_general_cfg_binding);
        drop(hit_detection_cfg_bind);
        drop(cfg_binding);
        match gravity_behavior {
            GravityBehavior::Ignore => {
                new_node_bind.gravity_cache.replace(Vector3::ZERO);
            },
            GravityBehavior::UseGlobalGravityCached => {
                let res = new_node_bind.get_global_gravity() * (gravity_multiplier as f32);
                new_node_bind.gravity_cache.replace(res);
            },
            GravityBehavior::UseGlobalGravityRealTime => {}, // No-op
            GravityBehavior::UseCurrentGravityRealTime => { area3d_test_needed = true },
        }
        match fluid_dynamics_behavior {
            FluidDynamicsBehavior::Ignore => {
                new_node_bind.ambient_airspeed_cache.replace(Vector3::ZERO);
                new_node_bind.fluid_drag_const_cache.replace(0.0);
                new_node_bind.speed_of_sound_cache.replace(299792458.0); // 1c, placeholder value
            },
            FluidDynamicsBehavior::UseGlobalFluidCached => {
                let global_fluid = new_node_bind.get_global_fluid_config();
                new_node_bind.speed_of_sound_cache.replace(global_fluid.bind().speed_of_sound);
                new_node_bind.ambient_airspeed_cache.replace(global_fluid.bind().ambient_airspeed);
                let computed_const_component = new_node_bind.compute_drag_const_component(global_fluid);
                new_node_bind.fluid_drag_const_cache.replace(computed_const_component);
            },
            FluidDynamicsBehavior::UseGlobalFluidRealTime => {} // No-op 
            FluidDynamicsBehavior::UseCurrentFluidRealTime => { area3d_test_needed = true },
        }
        if area3d_test_needed {
            // A map_or_else is more idiomatic, but that creates a double mutable borrow to new_node_bind, which I do not want to deal with
            if collision_shape_data.is_some() {
                let mut some_new = PhysicsShapeQueryParameters3D::new_gd();
                some_new.set_collide_with_areas(true);
                some_new.set_collide_with_bodies(false);
                some_new.set_collision_mask(collision_mask_data);
                some_new.set_shape(&*collision_shape_data.as_ref().expect("collision_shape_data is checked above to be Some"));
                new_node_bind.area_test_cache_shape.replace(some_new);
                // let mut new_cs3d = CollisionShape3D::new_alloc().to_godot_owned();
                // new_cs3d.set_shape(&collision_shape_data.expect("collision_shape_data is checked above to be Some"));
                // gd_colshape3d.replace(new_cs3d);
            } else {
                let mut some_new = PhysicsPointQueryParameters3D::new_gd();
                some_new.set_collide_with_areas(true);
                some_new.set_collide_with_bodies(false);
                some_new.set_collision_mask(collision_mask_data);
                new_node_bind.area_test_cache_point.replace(some_new);
                // let mut new_cs3d = CollisionShape3D::new_alloc().to_godot_owned();
                // let mut ad_hoc = SphereShape3D::new_gd(); // this stupidly makes a new unique instance every time, is there a better way??
                // ad_hoc.set_radius(0.0001);
                // new_cs3d.set_shape(&ad_hoc);
                // gd_colshape3d.replace(new_cs3d);
            }
        }
        let make_exclude_list = |hit_detection_cfg: GdRef<'_, FluoriteCastCfgHitDetection>| -> Array<Rid> {
            let mut arr = array![];//[new_node_bind.base().get_rid()];
            hit_detection_cfg.exclude_list_paths_shallow
                .iter_shared()
                .for_each(|pth| {
                    let maybe_node = parent_to.get_node_or_null(&pth);
                    if let Some(mut some_node) = maybe_node {
                        if some_node.has_method("get_rid") {
                            arr.push(some_node.call("get_rid", &[]).try_to::<Rid>().expect("get_rid should return Rid"));
                        } else {
                            if !suppress_invalid_path_warnings {
                                push_warning(&[pth.to_string().to_variant(), " does not implement `get_rid`, so it will be ignored".to_variant()]);
                            }
                        }
                    } else {
                        if !suppress_invalid_path_warnings {
                            push_warning(&[pth.to_string().to_variant(), " pointed to null, so it will be ignored".to_variant()]);
                        }
                    }
                });
            hit_detection_cfg.exclude_list_paths_recursive
                .iter_shared()
                .for_each(|pth| {
                    let maybe_node = parent_to.get_node_or_null(&pth);
                    if let Some(some_node) = maybe_node {
                        // Recursive FnMut closures are nearly impossible to do safely, so we declare a local function instead
                        fn recursive_search(mut from: Gd<Node>, mut arr: &mut Array<Rid>, depth: u8) -> () {
                            if depth == u8::MAX {
                                // who knows what abomination of a scene tree you have if you hit this limit
                                godot_warn!("Recursion depth of 255 reached in recursive_search while parsing exclude_list_paths_recursive! Will not recurse deeper!");
                                return
                            }
                            if from.has_method("get_rid") {
                                arr.push(from.call("get_rid", &[]).try_to::<Rid>().expect("get_rid should return Rid"));
                            }
                            from.get_children()
                                .iter_shared()
                                .for_each(|child| recursive_search(child, &mut arr, depth + 1));
                        }
                        recursive_search(some_node, &mut arr, 0);
                    } else {
                        if !suppress_invalid_path_warnings {
                            push_warning(&[pth.to_string().to_variant(), " pointed to null, so it will be ignored".to_variant()]);
                        }
                    }
                });
            arr
        };
        match collision_detection_mode {
            CollisionDetectionMode::Ignore => {}, // No-op
            CollisionDetectionMode::ByRaycast => {
                let binding = new_node_bind.config.bind();
                let hit_detection_cfg = binding.cast_hit_detection_cfg.as_ref().expect("cast_hit_detection_cfg should always exist").bind();
                let mut query_params = PhysicsRayQueryParameters3D::new_gd();
                query_params.set_collision_mask(hit_detection_cfg.hit_collision_mask);
                query_params.set_collide_with_areas(hit_detection_cfg.should_collide_with_areas);
                query_params.set_collide_with_bodies(hit_detection_cfg.should_collide_with_bodies);
                query_params.set_hit_back_faces(hit_detection_cfg.should_hit_back_faces);
                query_params.set_hit_from_inside(hit_detection_cfg.should_hit_from_inside);
                query_params.set_exclude(&make_exclude_list(hit_detection_cfg));
                drop(binding);
                let _ = new_node_bind.query_params_cache_ray.insert(query_params);
            },
            CollisionDetectionMode::ByShapecast => {
                let binding = new_node_bind.config.bind();
                let hit_detection_cfg = binding.cast_hit_detection_cfg.as_ref().expect("cast_hit_detection_cfg should always exist").bind();
                let mut query_params = PhysicsShapeQueryParameters3D::new_gd();
                query_params.set_collision_mask(hit_detection_cfg.hit_collision_mask);
                query_params.set_collide_with_areas(hit_detection_cfg.should_collide_with_areas);
                query_params.set_collide_with_bodies(hit_detection_cfg.should_collide_with_bodies);
                query_params.set_shape(hit_detection_cfg.hit_shape.as_ref().expect("hit_shape should always exist"));
                query_params.set_margin(hit_detection_cfg.shape_margin as f32);
                query_params.set_exclude(&make_exclude_list(hit_detection_cfg));
                drop(binding);
                let _ = new_node_bind.query_params_cache_shape.insert(query_params);
            },
        }
        new_node_bind.assign_payload(payload);
        drop(new_node_bind);

        // This causes a regression where Area3Ds cannot apply gravity on it, so it was reworked
        // If we set set_collision_layer to collision_mask_data, it will interact with physics objects in the world, which is undersirable
        // new_node.set_collision_mask(collision_mask_data);
        // new_node.set_collision_layer(0); // we only need to get affected by `Area3D`s for real-time local gravity polling, so disable collision layer entirely
        
        // if let Some(some_gd_cs3) = gd_colshape3d {
        //     new_node.add_child(&some_gd_cs3);
        // }

        parent_to.add_child(&new_node);

        if new_node.bind().config.bind().cast_methods_cfg.as_ref().expect("cast_methods_cfg should always exist").bind().builtin_flags.is_some() {
            Self::parse_builtin_config(&mut new_node.bind_mut());
        }

        let new_node_clo = new_node.clone();
        let mut new_node_bind = new_node.bind_mut();
        let mut config_bind = new_node_bind.config.bind_mut();
        let mut cast_methods_cfg_bind = config_bind.cast_methods_cfg.as_mut().expect("cast_methods_cfg should always exist").bind_mut();
        let on_new_cast_via = cast_methods_cfg_bind.on_new_cast_via;
        match on_new_cast_via {
            MaybeExecuteCodeVia::ViaRustFnMut => {
                let maybe_closure = cast_methods_cfg_bind.on_new_cast_rs.take();
                drop(cast_methods_cfg_bind);
                drop(config_bind);
                drop(new_node_bind);
                if let Some(mut associated_closure) = maybe_closure {
                    associated_closure(&mut new_node.bind_mut());
                };
                new_node
            },
            MaybeExecuteCodeVia::ViaMethodOnResourceSnakeCase
                if let Some(method_holder) = cast_methods_cfg_bind.methods_holder.as_mut()
                && method_holder.has_method("on_new_cast") => {
                    method_holder.call(
                        "on_new_cast",
                        &[
                            new_node_clo.to_variant(),
                        ]
                    );
                    drop(cast_methods_cfg_bind);
                    drop(config_bind);
                    drop(new_node_bind);
                    new_node
                },
            MaybeExecuteCodeVia::ViaMethodOnResourcePascalCase
                if let Some(method_holder) = cast_methods_cfg_bind.methods_holder.as_mut()
                && method_holder.has_method("OnNewCast") => {
                    method_holder.call(
                        "OnNewCast",
                        &[
                            new_node_clo.to_variant(),
                        ]
                    );
                    drop(cast_methods_cfg_bind);
                    drop(config_bind);
                    drop(new_node_bind);
                    new_node
                },
            _ => {
                drop(cast_methods_cfg_bind);
                drop(config_bind);
                drop(new_node_bind);
                new_node
            },
        }
    }
    
    /// Adds the node `from_node`'s RID to the given `ignore_list`, recursively so if desired.
    /// 
    /// The given `ignore_list` will be consumed, mutated, then returned back as the output.
    #[func]
    pub fn add_ignore_rid(from_node: Gd<Node>, mut ignore_list: Array<Rid>, is_recursive: bool) -> Array<Rid> {
        fn recursive_search(mut from: Gd<Node>, mut arr: &mut Array<Rid>, depth: u8, should_recurse: bool) -> () {
            if depth == u8::MAX {
                // who knows what abomination of a scene tree you have if you hit this limit
                godot_warn!("Recursion depth of 255 reached in recursive_search while parsing exclude_list_paths_recursive! Will not recurse deeper!");
                return
            }
            if from.has_method("get_rid") {
                arr.push(from.call("get_rid", &[]).try_to::<Rid>().expect("get_rid should return Rid"));
            }
            if !should_recurse { return }
            from.get_children()
                .iter_shared()
                .for_each(|child| recursive_search(child, &mut arr, depth + 1, should_recurse));
        }
        recursive_search(from_node, &mut ignore_list, 0, is_recursive);
        ignore_list
    }

    #[func]
    /// Assign a payload to the cast, overwriting any pre-existing payload.
    pub fn assign_payload(&mut self, payload: Option<Gd<Node3D>>) -> () {
        if let Some(mut node) = self.payload_node.take() {
            node.queue_free();
        }
        self.payload_node = payload;
        if let Some(payload_rc) = self.payload_node.clone() {
            self.base_mut().add_child(&payload_rc);
        }
    }
    #[func]
    /// Gets a handle to the payload this cast has, if it exists.
    pub fn get_payload(&self) -> Option<Gd<Node3D>> {
        self.payload_node.clone()
    }
    #[func]
    /// Check if this cast is disabled - that is, if it is ignoring any `evaluate` calls.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
    #[func]
    /// Check if this cast is being cleaned up and will be freed.
    pub fn is_scheduled_free(&self) -> bool {
        self.is_cleaning_up
    }
    #[func]
    /// Fire the cast from the given `global_origin`, with the velocity as `direction`.
    /// 
    /// Unless you're explicitly making an ad-hoc cast, there is not much reason to call this.
    /// Prefer casting on behalf of `FluoriteCastFactory` instead.
    pub fn fire(&mut self, global_origin: Transform3D, direction: Vector3) -> () {
        self.base_mut().set_global_transform(global_origin);
        self.add_velocity(direction);
    }
    #[func]
    /// Increments the `current_velocity` of this cast.
    pub fn add_velocity(&mut self, by: Vector3) -> () {
        self.current_velocity += by;
    }
    #[func]
    /// Evaluates the cast, advancing it forward by `delta` seconds.
    /// The cast may be evaluated several times (by calling `evaluate_raw`), depending on supersampling settings.
    /// 
    /// You typically do not need to call this yourself unless `self.config.EvaluateMode` is `Manual`.
    pub fn evaluate(&mut self, delta: f64, forced: bool) -> () {
        if self.disabled && !forced {
            return
        };
        let vel = self.current_velocity;
        let estimated_dist = (vel*(delta as f32)).length() as f64;
        let slice_count: i64;
        let cfg_bind = self.config.bind();
        let cast_fidelity_cfg_bind = cfg_bind.cast_fidelity_cfg.as_ref().expect("cast_fidelity_cfg should always exist").bind();
        match cast_fidelity_cfg_bind.super_sampling_mode {
            SuperSamplingMode::Never => {
                slice_count = 1;
            },
            SuperSamplingMode::IfAboveTargetDelta => {
                let ratio_t = delta / cast_fidelity_cfg_bind.target_delta;
                if ratio_t > 1.0 {
                    slice_count = max(ceilf(ratio_t) as i64, cast_fidelity_cfg_bind.max_supersampling);
                } else {
                    slice_count = 1;
                }
            },
            SuperSamplingMode::IfTooLong => {
                let ratio_l = estimated_dist / cast_fidelity_cfg_bind.target_length;
                if ratio_l > 1.0 {
                    slice_count = max(ceilf(ratio_l) as i64, cast_fidelity_cfg_bind.max_supersampling);
                } else {
                    slice_count = 1;
                }
            },
            SuperSamplingMode::IfAboveTargetDeltaOrTooLong => {
                let mut tmp: i64 = 1;
                let ratio_t = delta / cast_fidelity_cfg_bind.target_delta;
                if ratio_t > 1.0 {
                    tmp = max(tmp, ceilf(ratio_t) as i64);
                }
                let ratio_l = estimated_dist / cast_fidelity_cfg_bind.target_length;
                if ratio_l > 1.0 {
                    tmp = max(tmp, ceilf(ratio_l) as i64);
                }
                slice_count = max(tmp, cast_fidelity_cfg_bind.max_supersampling);
            },
        }
        drop(cast_fidelity_cfg_bind);
        drop(cfg_bind);
        let sliced_delta = delta / (slice_count as f64);
        for _ in 0..slice_count {
            self.evaluate_raw(sliced_delta, forced, false, Vector3::ZERO, 1);
        }
        let look_behavior = self.config
            .bind()
            .cast_general_cfg
            .as_ref()
            .expect("cast_general_cfg should always exist")
            .bind()
            .projectile_look_behavior;
        if let ProjectileLookBehavior::FollowVelocity = look_behavior {
            let current_vel = self.current_velocity;
            self.base_mut().set_basis(Basis::looking_at(current_vel));
        }
        self.try_expire();
    }
    #[func]
    /// Evaluate the cast by `delta` seconds.
    /// 
    /// There is not much reason to call this manually, prefer calling `evaluate` instead,
    /// unless you have a good reason to bypass the abstraction and pre/post-checks that `evaluate` does.
    pub fn evaluate_raw(&mut self, delta: f64, forced: bool, override_dist: bool, overridden_dist_v3: Vector3, recursion_depth: i64) -> () {
        if self.disabled && !forced {
            return
        };
        if recursion_depth > 16 {
            godot_warn!("Recursion depth of 16 exceeded in evaluate_raw!");
            return
        }
        if !override_dist {
            let mut area_cache: Option<Array<VarDictionary>> = None;
            match self.gravity_cache {
                None => {
                    let binding = self.config.bind();
                    let grav_cfg = binding.cast_gravity_cfg.as_ref().expect("cast_gravity_cfg should always exist").bind();
                    let gravity_behavior = grav_cfg.gravity_behavior;
                    let gravity_multiplier = grav_cfg.gravity_multiplier;
                    drop(grav_cfg);
                    drop(binding);
                    match gravity_behavior {
                        GravityBehavior::UseGlobalGravityRealTime => {
                            let res = self.get_global_gravity() * (gravity_multiplier as f32)
                            ;
                            self.current_velocity += (res + self.current_acceleration)*(delta as f32);
                        },
                        GravityBehavior::UseCurrentGravityRealTime => {
                            if area_cache.is_none() {
                                area_cache.replace(self.scan_overlapping_area3ds(MAX_CAST_RESULTS));
                            }
                            let grav = self.get_current_gravity(area_cache.clone().expect("cache should be filled right above or upstream"));
                            // let grav = self.base().get_gravity(); // This doesn't work! We need a custom gravity calculator!
                            self.current_velocity += ((grav*(gravity_multiplier as f32)) + self.current_acceleration)*(delta as f32);
                        },
                        _ => { panic!("gravity_cache should exist for cached modes") }
                    }
                },
                Some(g) => {
                    let gravity_multiplier = self.config.bind().cast_gravity_cfg.as_ref().expect("cast_gravity_cfg should always exist").bind().gravity_multiplier;
                    self.current_velocity += (g*(gravity_multiplier as f32) + self.current_acceleration)*(delta as f32);
                },
            }
            let self_config_binding = self.config.bind();
            let cast_fluid_dynamics_cfg_bind = self_config_binding.cast_fluid_dynamics_cfg.as_ref().expect("cast_fluid_dynamics_cfg should always exist").bind();
            let fluid_dynamics_fidelity = cast_fluid_dynamics_cfg_bind.fluid_dynamics_fidelity;
            let fluid_dynamics_behavior = cast_fluid_dynamics_cfg_bind.fluid_dynamics_behavior;
            drop(cast_fluid_dynamics_cfg_bind);
            drop(self_config_binding);
            let maybe_fluid_config = {
                match fluid_dynamics_behavior {
                    FluidDynamicsBehavior::UseGlobalFluidRealTime => {
                        Some(self.get_global_fluid_config())
                    },
                    FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                        if area_cache.is_none() {
                            area_cache.replace(self.scan_overlapping_area3ds(MAX_CAST_RESULTS));
                        }
                        Some(self.get_current_fluid_config(area_cache.clone().expect("cache should be filled right above or upstream")))
                    },
                    _ => {
                        None
                    },
                }
            };
            let ambient_airspeed = self.ambient_airspeed_cache.unwrap_or_else(|| {
                match fluid_dynamics_behavior {
                    FluidDynamicsBehavior::UseGlobalFluidRealTime | FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                        maybe_fluid_config.clone().expect("maybe_fluid_config should be Some").bind().ambient_airspeed
                    },
                    _ => {
                        panic!("fluid_dynamics_behavior was not *RealTime while ambient_airspeed_cache was None!")
                    },
                }
            });
            match fluid_dynamics_fidelity {
                FluidDynamicsFidelity::Ignore => {}, // No-op
                FluidDynamicsFidelity::OnlyAmbientAirspeed => {
                    self.current_velocity += ambient_airspeed*(delta as f32);
                },
                FluidDynamicsFidelity::DragCoefficient => {
                    self.current_velocity += ambient_airspeed*(delta as f32);
                    let external_airspeed = self.current_velocity - ambient_airspeed;
                    let drag = self.compute_drag_ideal(
                        external_airspeed.length() as f64,
                        external_airspeed.normalized(),
                        maybe_fluid_config.clone()
                    )*(delta as f32);
                    self.current_velocity += drag;
                },
                FluidDynamicsFidelity::DragCoefficientAndMach => {
                    self.current_velocity += ambient_airspeed*(delta as f32);
                    let external_airspeed = self.current_velocity - ambient_airspeed;
                    let drag = self.compute_drag_full_approx(
                        external_airspeed.length() as f64,
                        external_airspeed.normalized(),
                        maybe_fluid_config // last time we need area_cache, so don't clone
                    )*(delta as f32);
                    self.current_velocity += drag
                },
            }
        }
        let vel = self.current_velocity;
        let base = self.base();
        let starting_pos = base.get_global_position();
        drop(base);
        let dist = match override_dist {
            true => { overridden_dist_v3 },
            false => { vel*(delta as f32) },
        };

        let exec_callback = |this: &mut Self| {
            let gd_this = this.object_to_owned();
            let mut config_binding = this.config.bind_mut();
            let mut cast_methods_cfg_bind = config_binding.cast_methods_cfg.as_mut().expect("cast_methods_cfg should always exist").bind_mut();
            let cast_raw_evaluated_via = cast_methods_cfg_bind.cast_raw_evaluated_via;
            match cast_raw_evaluated_via {
                MaybeExecuteCodeVia::ViaRustFnMut => {
                    let maybe_closure = cast_methods_cfg_bind.cast_raw_evaluated_rs.take();
                    if let Some(mut associated_closure) = maybe_closure {
                        drop(cast_methods_cfg_bind);
                        drop(config_binding);
                        associated_closure(this, dist, delta, override_dist);
                        let _ = this.config.bind_mut()
                            .cast_methods_cfg
                            .as_mut()
                            .expect("cast_methods_cfg should always exist")
                            .bind_mut()
                            .cast_raw_evaluated_rs
                            .insert(associated_closure);
                    }
                },
                MaybeExecuteCodeVia::ViaMethodOnResourceSnakeCase
                    if let Some(method_holder) = cast_methods_cfg_bind.methods_holder.as_mut()
                    && method_holder.has_method("cast_raw_evaluated") => {
                        method_holder.call(
                            "cast_raw_evaluated",
                            &[
                                gd_this.to_variant(),
                                dist.to_variant(),
                                delta.to_variant(),
                                override_dist.to_variant(),
                            ]
                        );
                    },
                MaybeExecuteCodeVia::ViaMethodOnResourcePascalCase
                    if let Some(method_holder) = cast_methods_cfg_bind.methods_holder.as_mut()
                    && method_holder.has_method("CastRawEvaluated") => {
                        method_holder.call(
                            "CastRawEvaluated",
                            &[
                                gd_this.to_variant(),
                                dist.to_variant(),
                                delta.to_variant(),
                                override_dist.to_variant(),
                            ]
                        );
                    },
                _ => {}, // No-op
            }
        };
        let res = self.try_intersect(starting_pos, starting_pos + dist);
        if let Some(cast_result) = res {
            let has_penetrated = self.try_penetrate(cast_result.clone());
            let self_clo = self.object_to_owned().clone();
            if !has_penetrated {
                let mut base_mut = self.base_mut();
                base_mut.set_global_position(cast_result.bind().position);
                drop(base_mut);
                self.signals().terminated().emit_tuple((self_clo, cast_result));

                let self_config_binding = self.config.bind();
                let cast_methods_cfg_bind = self_config_binding.cast_methods_cfg.as_ref().expect("cast_methods_cfg should always exist").bind();
                let should_cleanup = cast_methods_cfg_bind.auto_queue_free_on_terminate;
                drop(cast_methods_cfg_bind);
                drop(self_config_binding);
                self.disabled = true;
                if should_cleanup {
                    self.cleanup();
                }
                exec_callback(self);
            } else {
                let mut base_mut = self.base_mut();
                let starting_pos = base_mut.get_global_position();
                let march_by = cast_result.bind().march_by;
                base_mut.set_global_position(starting_pos + march_by);//(cast_result.bind().position);
                drop(base_mut);
                self.signals().penetrated().emit_tuple((self_clo, cast_result));
                if !override_dist {
                    self.alive_for += delta;
                    self.distance_covered += dist.length();
                }
                exec_callback(self);
                // We recurse with a smaller slice to keep casting in this frame
                self.evaluate_raw(0.0, forced, true, dist - march_by, recursion_depth + 1);
            }
        } else {
            let mut base_mut = self.base_mut();
            base_mut.set_global_position(starting_pos + dist);
            drop(base_mut);
            if !override_dist {
                self.alive_for += delta;
                self.distance_covered += dist.length();
            }
            exec_callback(self);
        }
    }
    #[func]
    /// Check if the cast should be expired, and if so, make it expired.
    pub fn try_expire(&mut self) -> () {
        let alive_for = self.alive_for;
        let distance_covered = self.distance_covered;
        let cfg_bind = self.config.bind();
        let general_bind = cfg_bind.cast_general_cfg.as_ref().expect("cast_general_cfg should always exist").bind();
        let should_free: bool;
        if alive_for > general_bind.max_alive_time {
            should_free = true
        } else if distance_covered > (general_bind.max_total_length as f32) {
            should_free = true
        } else {
            should_free = false
        }
        drop(general_bind);
        drop(cfg_bind);
        if should_free {
            let self_clo = self.object_to_owned();
            self.signals().expired().emit(&self_clo); // I have no idea `emit` wants Gd<_> passed by reference, but `emit_tuple` by value??? But OK.
            let should_cleanup = self.config.bind().cast_methods_cfg.as_ref().expect("cast_methods_cfg should always exist").bind().auto_queue_free_on_terminate;
            if should_cleanup {
                self.cleanup();
            }
        }
    }
    #[func]
    /// Cleans up the cast. Always call this instead of `free` or `queue_free`,
    /// unless you have a good reason to bypass emitting the `freeing` signal.
    pub fn cleanup(&mut self) -> () {
        if self.is_cleaning_up { return }
        self.is_cleaning_up = true;
        let self_clo = self.object_to_owned();
        self.signals().freeing().emit(&self_clo);
        self.base_mut().queue_free();
    }
    #[func]
    /// Get a handle to the `FluoriteCastConfig` inside the cast.
    pub fn get_config(&self) -> Gd<FluoriteCastConfig> {
        self.config.clone()
    }
    #[func]
    /// Mutate the `FluoriteCastConfig` inside the cast on behalf of the callable `with`.
    /// 
    /// The callable signature of `with` should be: `(FluoriteCast) -> void`
    pub fn mut_config(&self, &with: Callable) -> () {
        with.call(&[
            self.config.to_variant()
        ]);
    }
    #[func]
    /// Compute the drag force the cast experiences right now.
    pub fn compute_drag_full_approx(&mut self, airspeed: f64, airspeed_unit_vector: Vector3, maybe_fluid_cfg: Option<Gd<FluoriteFluidConfig>>) -> Vector3 {
        // The general idea is as follows:
        // drag = -0.5 * gas_density * ref_area * airspeed^2 * drag_coefficient * airspeed_unit_vector
        // where drag_coefficient = too_complicated_to_compute_for_this_library_so_const * some_curve.map_to(airspeed / speed_of_sound)
        // => therefore drag = (-0.5 * gas_density * ref_area * too_complicated_to_compute_for_this_library_so_const) * (some_curve.map_to(airspeed / speed_of_sound) * airspeed^2 * airspeed_unit_vector)
        // where the first (expr) is const, the second (expr) is dyn
        // where gas_density + ref_area + speed_of_sound + too_complicated_to_compute_for_this_library_so_const is const
        // where airspeed + airspeed_unit_vector is dyn
        // where some_curve is Curve
        self.compute_drag_ideal(airspeed, airspeed_unit_vector, maybe_fluid_cfg.clone()) * (self.compute_drag_dyn_component_mach(airspeed, maybe_fluid_cfg) as f32)
    } 
    #[func]
    /// Compute the drag force the cast experiences right now, ignoring the mach-based multiplier.
    pub fn compute_drag_ideal(&mut self, airspeed: f64, airspeed_unit_vector: Vector3, maybe_fluid_cfg: Option<Gd<FluoriteFluidConfig>>) -> Vector3 {
        self.get_drag_const_component(maybe_fluid_cfg) as f32 * self.compute_drag_dyn_component_airspeed(airspeed, airspeed_unit_vector)
    } 
    #[func]
    /// Compute the constant component of the drag force equation.
    pub fn compute_drag_const_component(&self, with_fluid_cfg: Gd<FluoriteFluidConfig>) -> f64 {
        let binding = self.config.bind();
        let current_fluid_cfg = with_fluid_cfg.bind();
        let cast_fluid_dynamics_cfg = binding.cast_fluid_dynamics_cfg.as_ref().expect("cast_fluid_dynamics_cfg should always exist").bind();

        // mm2 -> m2 requires dividing by 1000 two times
        -0.5 * current_fluid_cfg.fluid_density_kgm3 * (cast_fluid_dynamics_cfg.projectile_reference_area_mm2 / 1000.0 / 1000.0) * cast_fluid_dynamics_cfg.drag_coefficient
    }
    #[func]
    /// Compute the airspeed component of the drag force equation.
    pub fn compute_drag_dyn_component_airspeed(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        (airspeed * airspeed) as f32 * airspeed_unit_vector
    }
    #[func]
    /// Compute the mach-based multiplier of the drag force equation.
    pub fn compute_drag_dyn_component_mach(&mut self, airspeed: f64, maybe_fluid_cfg: Option<Gd<FluoriteFluidConfig>>) -> f64 {
        let maybe_curve = self.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("cast_fluid_dynamics_cfg should always exist").bind().mach_based_drag_multiplier.clone();
        if let Some(curve) = maybe_curve {
            let mach_number = self.get_mach_number(airspeed, maybe_fluid_cfg) as f32;
            curve.sample(mach_number) as f64
        } else {
            1.0
        }
    }
    #[func]
    /// Gets the mach number of the cast.
    pub fn get_mach_number(&mut self, airspeed: f64, maybe_fluid_cfg: Option<Gd<FluoriteFluidConfig>>) -> f64 {
        airspeed / self.speed_of_sound_cache.unwrap_or_else(|| {
            let fluid_dynamics_behavior = self.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("cast_fluid_dynamics_cfg should always exist").bind().fluid_dynamics_behavior;
            match fluid_dynamics_behavior {
                FluidDynamicsBehavior::UseGlobalFluidRealTime => {
                    maybe_fluid_cfg.expect("maybe_fluid_cfg should be Some in UseGlobalFluidRealTime").bind().speed_of_sound
                },
                FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                    maybe_fluid_cfg.expect("maybe_fluid_cfg should be Some in UseCurrentFluidRealTime").bind().speed_of_sound
                },
                _ => {
                    panic!("fluid_dynamics_behavior was not *RealTime while speed_of_sound_cache was None!")
                },
            }
        })
    }
    #[func]
    /// Gets the constant component of the drag force equation.
    /// 
    /// It will just return the cached value if the constant component is cached.
    /// Otherwise it will call `compute_drag_const_component` and forward the result.
    pub fn get_drag_const_component(&mut self, maybe_fluid_cfg: Option<Gd<FluoriteFluidConfig>>) -> f64 {
        self.fluid_drag_const_cache.unwrap_or_else(|| {
            let fluid_dynamics_behavior = self.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("cast_fluid_dynamics_cfg should always exist").bind().fluid_dynamics_behavior;
            match fluid_dynamics_behavior {
                FluidDynamicsBehavior::UseGlobalFluidRealTime => {
                    let fluid = maybe_fluid_cfg.expect("maybe_fluid_cfg should be Some in UseGlobalFluidRealTime");
                    self.compute_drag_const_component(fluid)
                },
                FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                    let fluid = maybe_fluid_cfg.expect("maybe_fluid_cfg should be Some in UseCurrentFluidRealTime");
                    self.compute_drag_const_component(fluid)
                },
                _ => {
                    panic!("fluid_dynamics_behavior was not *RealTime while fluid_drag_const_cache was None!")
                },
            }
        })
    }
    #[func]
    /// Test if the cast should penetrate, according to the `cast_result`.
    pub fn try_penetrate(&mut self, cast_result: Gd<FluoriteSpaceCastResult>) -> bool {
        // this sucks, so it's abstracted away to this function
        // NOTE: it is FnMut because it needs to mutate state of self in a pragmatic implementation.
        // Typically in the `custom_data` field to accumulate penetration data, or to modify `current_velocity`
        let self_clo = self.object_to_owned().clone();
        let mut self_config_binding = self.config.bind_mut();
        let mut cast_methods_cfg_bind = self_config_binding.cast_methods_cfg.as_mut().expect("cast_methods_cfg should always exist").bind_mut();
        let try_penetrate_via = cast_methods_cfg_bind.try_penetrate_via;
        match try_penetrate_via {
            MaybeExecuteCodeVia::ViaRustFnMut => {
                let maybe_closure = cast_methods_cfg_bind.try_penetrate_rs.take();
                if let Some(mut associated_closure) = maybe_closure {
                    // we have to drop all the borrow guards, or else we will double borrow in the associated closure!
                    drop(cast_methods_cfg_bind);
                    drop(self_config_binding);

                    let temp = associated_closure(self, cast_result);

                    let _ = self.config
                        .bind_mut()
                        .cast_methods_cfg
                        .as_mut()
                        .expect("cast_methods_cfg should always exist")
                        .bind_mut()
                        .try_penetrate_rs.insert(associated_closure);
                    temp
                } else {
                    false
                }
            },
            MaybeExecuteCodeVia::ViaMethodOnResourceSnakeCase
                if let Some(method_holder) = cast_methods_cfg_bind.methods_holder.as_mut()
                && method_holder.has_method("try_penetrate") => {
                    // for some reason, calling Godot methods while a guard is active doesn't crash?
                    method_holder.call(
                        "try_penetrate",
                        &[
                            self_clo.to_variant(),
                            cast_result.to_variant(),
                        ]
                    ).try_to().expect("try_penetrate should return bool")
                },
            MaybeExecuteCodeVia::ViaMethodOnResourcePascalCase
                if let Some(method_holder) = cast_methods_cfg_bind.methods_holder.as_mut()
                && method_holder.has_method("TryPenetrate") => {
                    method_holder.call(
                        "TryPenetrate",
                        &[
                            self_clo.to_variant(),
                            cast_result.to_variant(),
                        ]
                    ).try_to().expect("TryPenetrate should return bool")
                },
            _ => { 
                let builtin_flags = cast_methods_cfg_bind.builtin_flags.clone();
                drop(cast_methods_cfg_bind);
                drop(self_config_binding);

                if let Some(flags) = builtin_flags && flags.bind().builtin_penetration {
                    Self::_builtin_try_penetrate(self, cast_result)
                } else {
                    false
                }
            },
        }
    }
    #[func]
    /// Test if the projectile would collide with anything, in the given path `from` to `to`.
    pub fn try_intersect(&mut self, from: Vector3, to: Vector3) -> Option<Gd<FluoriteSpaceCastResult>> {
        let binding = self.config.bind();
        let hit_detection_cfg = binding.cast_hit_detection_cfg.as_ref().expect("hit_detection_cfg should always exist").bind();
        let space_cast_result = match hit_detection_cfg.collision_detection_mode {
            CollisionDetectionMode::Ignore => { SpaceCastResult::HitNothing },
            CollisionDetectionMode::ByRaycast => {
                let mut direct_space = self.base().get_world_3d().expect("world_3d should exist").get_direct_space_state().expect("direct_space_state should exist");
                let query_params = self.query_params_cache_ray.as_mut().expect("query_params_cache_ray should exist in ByRaycast mode");
                query_params.set_from(from);
                query_params.set_to(to);
                let res = direct_space.intersect_ray(&*query_params);
                if res.contains_key("normal") {
                    let res_pos = res.get("position").expect("position should always exist").try_to::<Vector3>().expect("position should be Vector3");
                    SpaceCastResult::HitByRaycast(res, res_pos - from)
                } else {
                    SpaceCastResult::HitNothing
                }
            },
            CollisionDetectionMode::ByShapecast => {
                let mut direct_space = self.base().get_world_3d().expect("world_3d should exist").get_direct_space_state().expect("direct_space_state should exist");
                let query_params = self.query_params_cache_shape.as_mut().expect("query_params_cache_shape should exist in ByShapecast mode");
                let diff_v3 = to - from;    
                query_params.set_motion(diff_v3);
                query_params.set_transform(Transform3D::new(
                    hit_detection_cfg.shape_basis
                    * Basis::looking_at(self.current_velocity),
                    from
                )); // TODO: we might want to make a manual version of this, just like how we do it for the payload?
                let proportions = direct_space.cast_motion(&*query_params);
                let safe_proportion = proportions.get(0).expect("get(0) should be Some, is hit_shape null?");
                if safe_proportion >= 1.0 {
                    SpaceCastResult::HitNothing
                } else {
                    let unsafe_proportion = proportions.get(1).expect("get(1) should be Some");
                    let unsafe_march = diff_v3*(unsafe_proportion as f32);
                    //let safe_march = diff_v3*(safe_proportion as f32);
                    query_params.set_transform(Transform3D::new(
                        hit_detection_cfg.shape_basis
                        * Basis::looking_at(self.current_velocity),
                        from + unsafe_march
                    ));
                    let mut res = direct_space.get_rest_info(&*query_params);

                    if res.contains_key("normal") {
                        let res_cid: i64 = res.get("collider_id").expect("collider_id should exist").try_to().expect("Should be an i64");
                        let mut res_creal = None;
                        let res2 = direct_space.intersect_shape_ex(&*query_params)
                            .max_results(MAX_CAST_RESULTS)
                            .done();
                        for entry in res2.iter_shared() {
                            if entry.get("collider_id").as_ref().is_some_and(|x| x.try_to::<i64>().expect("Should be an i64") == res_cid) {
                                res_creal.replace(entry.get("collider").expect("Should exist"));
                                break;
                            }
                        }
                        if res_creal.is_some() {
                            // probably can be written in a more idiomatic way, but I forgot how
                            let _ = res.insert("collider", &res_creal.expect("Infallible, checked above"));
                        } else {
                            godot_warn!("Could not infer collider, so `collider` will be null");
                        }
                        SpaceCastResult::HitByShapecast(res, unsafe_march)
                    } else {
                        SpaceCastResult::HitNothing
                    }
                }
            },
        };

        match space_cast_result {
            SpaceCastResult::HitNothing => { None },
            SpaceCastResult::HitByRaycast(res, marched_by) => {
                // collider, collider_id, normal, position, face_index, rid, shape
                Some(FluoriteSpaceCastResult::new_result(
                    res.get("position").expect("position should exist").try_to().expect("position should be Vector3"),
                    res.get("normal").expect("normal should exist").try_to().expect("normal should be Vector3"),
                    res.get("rid").expect("rid should exist").try_to::<Rid>().expect("rid should be Rid").to_u64() as i64,
                    res.get("collider").map(|some| some.try_to().expect("collider should be Gd<Node3D>")),
                    res.get("collider_id").expect("collider_id should exist").try_to().expect("collider_id should be i64"),
                    res.get("shape").expect("shape should exist").try_to().expect("shape should be i64"),
                    marched_by,
                ))
            },
            SpaceCastResult::HitByShapecast(res, marched_by) => {
                // collider (injected manually), collider_id, linear_velocity, normal, point, rid, shape
                Some(FluoriteSpaceCastResult::new_result(
                    res.get("point").expect("point should exist").try_to().expect("point should be Vector3"),
                    res.get("normal").expect("normal should exist").try_to().expect("normal should be Vector3"),
                    res.get("rid").expect("rid should exist").try_to::<Rid>().expect("rid should be Rid").to_u64() as i64,
                    res.get("collider").map(|some| some.try_to().expect("collider should be Gd<Node3D>")),
                    res.get("collider_id").expect("collider_id should exist").try_to().expect("collider_id should be i64"),
                    res.get("shape").expect("shape should exist").try_to().expect("shape should be i64"),
                    marched_by,
                ))
            },
        }
    }
    #[func]
    /// Check what `Area3D`s the cast is overlapping with right now.
    pub fn scan_overlapping_area3ds(&mut self, max_results: i32) -> Array<VarDictionary> {
        let mut direct_space = self.base().get_world_3d().expect("world_3d should exist").get_direct_space_state().expect("direct_space_state should exist");
        let result;
        if self.area_test_cache_shape.is_some() {
            let area_collision_basis = self.config.bind().cast_general_cfg.as_ref().expect("cast_general_cfg should always exist").bind().area_collision_basis;
            let looking_at = Basis::looking_at(self.current_velocity);
            let base_pos = self.base().get_global_position();
            let cache_shape_binding = self.area_test_cache_shape.as_mut().expect("area_test_cache_shape is checked above");
            cache_shape_binding.set_transform(Transform3D::new(
                area_collision_basis
                * looking_at, // TODO: we might want to make a manual version of this, just like how we do it for the payload?
                base_pos
            ));
            result = direct_space.intersect_shape_ex(&*cache_shape_binding).max_results(max_results).done();
        } else {
            let base_pos = self.base().get_global_position();
            let cache_point_binding = self.area_test_cache_point.as_mut().expect("area_test_cache_point should exist if area_test_cache_shape does not");
            cache_point_binding.set_position(base_pos);
            result = direct_space.intersect_point_ex(&*cache_point_binding).max_results(max_results).done();
        }
        result
    }
    #[func]
    /// Get the config of the current fluid the cast is currently in.
    pub fn get_current_fluid_config(&self, overlap_cache: Array<VarDictionary>) -> Gd<FluoriteFluidConfig> {
        let result = overlap_cache; //self.scan_overlapping_area3ds(MAX_CAST_RESULTS);
        let mut fluid_area3ds = Vec::new();
        for entry in result.iter_shared() {
            if
                let Some(collider_variant) = entry.get("collider")
                && let Ok(collider) = collider_variant.try_to::<Gd<FluoriteFluidArea3D>>()
            {
                fluid_area3ds.push(collider);
            }
        }
        fluid_area3ds.sort_unstable_by(|a, b| {
            a.bind().fluid_override_priority.cmp(&b.bind().fluid_override_priority).reverse()
        });
        fluid_area3ds.iter().next().map_or_else(|| {
            self.get_global_fluid_config()
        }, |area| {
            area.bind().fluid_override_config.clone().expect("fluid_override_config should always exist on a FluoriteFluidArea3D")
        })
    }
    #[func]
    /// Get the config of the global fluid.
    pub fn get_global_fluid_config(&self) -> Gd<FluoriteFluidConfig> {
        self.global_fluid.clone().expect("global_fluid should always exist")
    }
    #[func]
    /// Get the gravity force the cast is currently experiencing.
    pub fn get_current_gravity(&self, overlap_cache: Array<VarDictionary>) -> Vector3 {
        let result = overlap_cache; //self.scan_overlapping_area3ds(MAX_CAST_RESULTS);
        let mut gravity_area3ds = Vec::new();
        for entry in result.iter_shared() {
            if
                let Some(collider_variant) = entry.get("collider")
                && let Ok(collider) = collider_variant.try_to::<Gd<Area3D>>()
                && collider.get_gravity_space_override_mode() != SpaceOverride::DISABLED
            {
                gravity_area3ds.push(collider);
            }
        }
        gravity_area3ds.sort_unstable_by(|a, b| {
            a.get_priority().cmp(&b.get_priority()).reverse()
        });
        let mut gravity = self.get_global_gravity();
        for area in gravity_area3ds.iter() {
            let this_gravity;
            if area.is_gravity_a_point() {
                let this_pos = self.base().get_global_position();
                let grav_center = (area.get_transform() * Transform3D::new(Basis::IDENTITY, area.get_gravity_point_center())).origin;
                let diff = this_pos - grav_center;
                let diff_len = diff.length();
                let diff_norm = diff.normalized_or_zero();
                let mut directional_grav = -diff_norm;
                let point_unit_dist = area.get_gravity_point_unit_distance();
                if point_unit_dist > 0.0 {
                    let lin_ratio = diff_len / point_unit_dist;
                    let inv_sq_ratio = 1.0 / (lin_ratio*lin_ratio);
                    directional_grav *= inv_sq_ratio;
                }
                this_gravity = area.get_gravity() * directional_grav;
            } else {
                this_gravity = area.get_gravity() * area.get_gravity_direction();
            }

            match area.get_gravity_space_override_mode() {
                SpaceOverride::DISABLED => unreachable!("SpaceOverride::DISABLED Area3Ds should get discarded above!"),
                SpaceOverride::COMBINE => {
                    gravity += this_gravity;
                },
                SpaceOverride::COMBINE_REPLACE => {
                    gravity += this_gravity;
                    break;
                },
                SpaceOverride::REPLACE => {
                    gravity = this_gravity;
                    break;
                },
                SpaceOverride::REPLACE_COMBINE => {
                    gravity = this_gravity;
                },
                _ => unreachable!("Invalid SpaceOverride flag in Area3D!"),
            }
        }
        gravity
    }
    #[func]
    /// Get the global gravity.
    pub fn get_global_gravity(&self) -> Vector3 {
        let project_settings = ProjectSettings::singleton();
        project_settings.get_setting("physics/3d/default_gravity_vector").try_to::<Vector3>().expect("default_gravity_vector should be Vector3")
            * (project_settings.get_setting("physics/3d/default_gravity").try_to::<f64>().expect("default_gravity should be f64") as f32)
        
    }
}

#[godot_api]
impl INode3D for FluoriteCast {
    fn process(&mut self, delta: f64) {
        let mut can_do = false; // The stupid crap borrowck forces me to do
        if let EvaluateMode::Process = self.config.bind().evaluate_mode {
            can_do = true;
        }
        if can_do {
            self.evaluate(delta, false);
        }
    }
    fn physics_process(&mut self, delta: f64) {
        let mut can_do = false;
        if let EvaluateMode::PhysicsProcess = self.config.bind().evaluate_mode {
            can_do = true;
        }
        if can_do {
            self.evaluate(delta, false);
        }
    }
}