use godot::{
    classes::{
        CollisionShape3D, IStaticBody3D, PhysicsRayQueryParameters3D, PhysicsShapeQueryParameters3D, ProjectSettings, StaticBody3D,
    }, global::{ceilf, push_warning}, meta::conv::ObjectToOwned, prelude::*,
};
use core::cmp::max;

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
};

enum SpaceCastResult {
    HitNothing,
    HitByRaycast(VarDictionary, Vector3),
    HitByShapecast(VarDictionary, Vector3),
}

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct FluoriteSpaceCastResult {
    base: Base<RefCounted>,
    #[var]
    position: Vector3,
    #[var]
    normal: Vector3,
    #[var]
    rid: i64,
    #[var]
    collider: Option<Gd<Node3D>>,
    #[var]
    collider_id: i64,
    #[var]
    shape: i64,
    #[var]
    march_by: Vector3,
}

#[godot_api]
impl FluoriteSpaceCastResult {
    #[func]
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
#[class(init, base=StaticBody3D)]
pub struct FluoriteCast {
    base: Base<StaticBody3D>,
    payload_node: Option<Gd<Node3D>>,
    gravity_cache: Option<Vector3>,
    ambient_airspeed_cache: Option<Vector3>,
    speed_of_sound_cache: Option<f64>,
    fluid_drag_const_cache: Option<f64>,
    disabled: bool,
    is_cleaning_up: bool,
    config: Gd<FluoriteCastConfig>,
    #[var]
    current_velocity: Vector3,
    #[var]
    current_acceleration: Vector3,
    #[var]
    distance_covered: f32,
    #[var]
    alive_for: f64,
    #[var]
    global_fluid: Option<Gd<FluoriteFluidConfig>>,
    #[var]
    custom_data: VarDictionary,
}

#[godot_api]
impl FluoriteCast {
    #[signal]
    fn penetrated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);

    #[signal]
    fn terminated(this: Gd<FluoriteCast>, cast_result: Gd<FluoriteSpaceCastResult>);

    #[signal]
    fn expired(this: Gd<FluoriteCast>);

    #[func]
    pub fn new_cast(&mut parent_to: Gd<Node3D>, payload: Option<Gd<Node3D>>, config: Gd<FluoriteCastConfig>, global_fluid: Gd<FluoriteFluidConfig>, custom_data: VarDictionary) -> Gd<Self> {
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
            }
        });
        // The base needs to be `StaticBody3D`, and a `CollisionShape3D` is needed so we can use `get_gravity` for `UseCurrentGravityRealTime` mode
        let mut gd_colshape3d = None;

        let mut new_node_bind = new_node.bind_mut();
        let collision_mask_data = new_node_bind.config.bind().area_collision_mask;
        let gravity_behavior = new_node_bind.config.bind().cast_gravity_cfg.as_ref().expect("Should always exist").bind().gravity_behavior;
        let fluid_dynamics_behavior = new_node_bind.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().fluid_dynamics_behavior;
        match gravity_behavior {
            GravityBehavior::Ignore => {
                new_node_bind.gravity_cache.replace(Vector3::ZERO);
            },
            GravityBehavior::UseGlobalGravityCached => {
                let res = ProjectSettings::singleton().get_setting("physics/3d/default_gravity_vector").to::<Vector3>()
                    * (ProjectSettings::singleton().get_setting("physics/3d/default_gravity").to::<f64>() as f32);
                new_node_bind.gravity_cache.replace(res);
            },
            GravityBehavior::UseGlobalGravityRealTime => {}, // No-op
            GravityBehavior::UseCurrentGravityRealTime => {
                // `CollisionShape3D` is only needed for real-time local gravity polling
                gd_colshape3d.replace(CollisionShape3D::new_alloc());
                let some_cs3d = gd_colshape3d.as_mut().expect("Added right above");
                some_cs3d.set_name("_FluoriteCastGravityPollingCS3D");
                some_cs3d.set_shape(&new_node_bind.config.bind().shape.clone().expect("Shape should always be assigned"));
            },
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
            FluidDynamicsBehavior::UseGlobalFluidRealTime | FluidDynamicsBehavior::UseCurrentFluidRealTime
                => {}, // No-op
        }
        new_node_bind.assign_payload(payload);
        drop(new_node_bind);

        new_node.set_collision_mask(collision_mask_data);
        new_node.set_collision_layer(0b0); // we only need to get affected by `Area3D`s for real-time local gravity polling, so disable collision layer entirely
        if let Some(some_gd_cs3) = gd_colshape3d {
            new_node.add_child(&some_gd_cs3);
        }

        parent_to.add_child(&new_node);

        new_node
    }
    #[func]
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
    pub fn fire(&mut self, global_origin: Transform3D, direction: Vector3) -> () {
        self.base_mut().set_global_transform(global_origin);
        self.add_velocity(direction);
    }
    #[func]
    pub fn add_velocity(&mut self, by: Vector3) -> () {
        self.current_velocity += by;
    }
    #[func]
    pub fn evaluate(&mut self, delta: f64, forced: bool) -> () {
        if self.disabled && !forced {
            return
        };
        let vel = self.current_velocity;
        let estimated_dist = (vel*(delta as f32)).length() as f64;
        let slice_count: i64;
        let cfg_bind = self.config.bind();
        let cast_fidelity_cfg_bind = cfg_bind.cast_fidelity_cfg.as_ref().expect("Should always exist").bind();
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
        self.try_expire();
    }
    #[func]
    pub fn evaluate_raw(&mut self, delta: f64, forced: bool, override_dist: bool, overridden_dist_v3: Vector3, recursion_depth: i64) -> () {
        if self.disabled && !forced {
            return
        };
        if recursion_depth > 16 {
            godot_warn!("Recursion depth of 16 exceeded in evaluate_raw!");
            return
        }
        match self.gravity_cache {
            None => {
                let gravity_behavior = self.config.bind().cast_gravity_cfg.as_ref().expect("Should always exist").bind().gravity_behavior;
                let gravity_multiplier = self.config.bind().cast_gravity_cfg.as_ref().expect("Should always exist").bind().gravity_multiplier;
                match gravity_behavior {
                    GravityBehavior::UseGlobalGravityRealTime | GravityBehavior::UseCurrentGravityRealTime => {
                        // `get_gravity` returns global gravity if `CollisionShape3D` is missing
                        self.current_velocity += ((self.base().get_gravity()*(gravity_multiplier as f32)) + self.current_acceleration)*(delta as f32);
                    },
                    _ => { panic!("gravity_cache should exist for cached modes") }
                }
            },
            Some(g) => {
                let gravity_multiplier = self.config.bind().cast_gravity_cfg.as_ref().expect("Should always exist").bind().gravity_multiplier;
                self.current_velocity += (g*(gravity_multiplier as f32) + self.current_acceleration)*(delta as f32);
            },
        }
        let self_config_binding = self.config.bind();
        let cast_fluid_dynamics_cfg_bind = self_config_binding.cast_fluid_dynamics_cfg.as_ref().expect("Should always exist").bind();
        let fluid_dynamics_fidelity = cast_fluid_dynamics_cfg_bind.fluid_dynamics_fidelity;
        let fluid_dynamics_behavior = cast_fluid_dynamics_cfg_bind.fluid_dynamics_behavior;
        drop(cast_fluid_dynamics_cfg_bind);
        drop(self_config_binding);
        let ambient_airspeed = self.ambient_airspeed_cache.unwrap_or_else(|| {
            match fluid_dynamics_behavior {
                FluidDynamicsBehavior::UseGlobalFluidRealTime => {
                    self.get_global_fluid_config().bind().ambient_airspeed
                },
                FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                    self.get_current_fluid_config().bind().ambient_airspeed
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
                self.current_velocity += self.compute_drag_ideal(external_airspeed.length() as f64, external_airspeed.normalized())*(delta as f32);
            },
            FluidDynamicsFidelity::DragCoefficientAndMach => {
                self.current_velocity += ambient_airspeed*(delta as f32);
                let external_airspeed = self.current_velocity - ambient_airspeed;
                self.current_velocity += self.compute_drag_full_approx(external_airspeed.length() as f64, external_airspeed.normalized())*(delta as f32);
            },
        }
        let vel = self.current_velocity;
        let base = self.base();
        let starting_pos = base.get_position();
        drop(base);
        let dist = match override_dist {
            true => { overridden_dist_v3 },
            false => { vel*(delta as f32) },
        };

        let res = self.try_intersect(starting_pos, starting_pos + dist); // TODO: Finish integrating this
        if let Some(cast_result) = res {
            godot_print!("Got an intersection");
            let has_penetrated = self.try_penetrate(cast_result.clone());
            // enable these again if we find the need to do so

            let self_clo = self.object_to_owned().clone();
            if !has_penetrated {
                let mut base_mut = self.base_mut();
                base_mut.set_global_position(cast_result.bind().position);
                drop(base_mut);
                self.signals().terminated().emit_tuple((self_clo, cast_result)); // tail of block, so no need to clone cast_result

                let self_config_binding = self.config.bind();
                let cast_on_hit_cfg_bind = self_config_binding.cast_on_hit_cfg.as_ref().expect("Should always exist").bind();
                let should_cleanup = cast_on_hit_cfg_bind.auto_queue_free_on_terminate;
                drop(cast_on_hit_cfg_bind);
                drop(self_config_binding);
                self.disabled = true;
                if should_cleanup {
                    self.cleanup();
                }
                // TODO: do some stuff I guess:
                // set position to hit position (done)
                // downstream code should not execute (done), and future evaluate calls should be silently dropped (done)
                // queue_free if programmed to do so, otherwise the caller needs to free it explicitly (done)
            } else {
                let mut base_mut = self.base_mut();
                base_mut.set_global_position(cast_result.bind().position);
                drop(base_mut);
                self.signals().penetrated().emit_tuple((self_clo, cast_result.clone()));
                // TODO: do some other stuff I guess:
                // set position to hit position (done)
                //~~ downstream code should not execute (so we don't tunnel through another collider behind the one we just hit)~~ (dropped)
                // maybe they should keep executing anyway but by accomodating for that collision so we don't make the projectile pay every frame per collider penetrated (done)
                // future evaluations keep working of course (done)
                if !override_dist {
                    self.alive_for += delta;
                    self.distance_covered += dist.length();
                }
                // We recurse with a smaller slice to keep casting in this frame
                self.evaluate_raw(0.0, forced, true, cast_result.bind().march_by, recursion_depth + 1);
            }
        } else {
            let mut base_mut = self.base_mut();
            base_mut.set_global_position(starting_pos + dist);
            drop(base_mut);
            if !override_dist {
                self.alive_for += delta;
                self.distance_covered += dist.length();
            }
        }
    }
    #[func]
    pub fn try_expire(&mut self) -> () {
        let alive_for = self.alive_for;
        let distance_covered = self.distance_covered;
        let cfg_bind = self.config.bind();
        let should_free: bool;
        if alive_for > cfg_bind.max_alive_time {
            should_free = true
        } else if distance_covered > (cfg_bind.max_total_length as f32) {
            should_free = true
        } else { should_free = false }
        drop(cfg_bind);
        if should_free {
            let self_clo = self.object_to_owned();
            self.signals().expired().emit(&self_clo); // I have no idea `emit` wants Gd<_> passed by reference, but `emit_tuple` by value??? But OK.
            self.cleanup();
        }
    }
    #[func]
    pub fn cleanup(&mut self) -> () {
        self.is_cleaning_up = true;
        self.base_mut().queue_free();
    }
    #[func]
    pub fn get_config(&self) -> Gd<FluoriteCastConfig> {
        self.config.clone()
    }
    #[func]
    pub fn compute_drag_full_approx(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        // The general idea is as follows:
        // drag = -0.5 * gas_density * ref_area * airspeed^2 * drag_coefficient * airspeed_unit_vector
        // where drag_coefficient = too_complicated_to_compute_for_this_library_so_const * some_curve.map_to(airspeed / speed_of_sound)
        // => therefore drag = (-0.5 * gas_density * ref_area * too_complicated_to_compute_for_this_library_so_const) * (some_curve.map_to(airspeed / speed_of_sound) * airspeed^2 * airspeed_unit_vector)
        // where the first (expr) is const, the second (expr) is dyn
        // where gas_density + ref_area + speed_of_sound + too_complicated_to_compute_for_this_library_so_const is const
        // where airspeed + airspeed_unit_vector is dyn
        // where some_curve is Curve
        self.compute_drag_ideal(airspeed, airspeed_unit_vector) * (self.compute_drag_dyn_component_mach(airspeed) as f32)
    } 
    #[func]
    pub fn compute_drag_ideal(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        self.get_drag_const_component() as f32 * self.compute_drag_dyn_component_airspeed(airspeed, airspeed_unit_vector)
    } 
    #[func]
    pub fn compute_drag_const_component(&self, with_fluid_cfg: Gd<FluoriteFluidConfig>) -> f64 {
        let binding = self.config.bind();
        let current_fluid_cfg = with_fluid_cfg.bind();
        let cast_fluid_dynamics_cfg = binding.cast_fluid_dynamics_cfg.as_ref().expect("Should always exist").bind();

        // mm2 -> m2 requires dividing by 1000 two times
        -0.5 * current_fluid_cfg.fluid_density_kgm3 * (cast_fluid_dynamics_cfg.projectile_reference_area_mm2 / 1000.0 / 1000.0) * cast_fluid_dynamics_cfg.drag_coefficient
    }
    #[func]
    pub fn compute_drag_dyn_component_airspeed(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        (airspeed * airspeed) as f32 * airspeed_unit_vector
    }
    #[func]
    pub fn compute_drag_dyn_component_mach(&self, airspeed: f64) -> f64 {
        if let Some(curve) = self.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().mach_based_drag_multiplier.as_ref() {
            curve.sample(self.get_mach_number(airspeed) as f32) as f64
        } else {
            1.0
        }
    }
    #[func]
    pub fn get_mach_number(&self, airspeed: f64) -> f64 {
        airspeed / self.speed_of_sound_cache.unwrap_or_else(|| {
            let fluid_dynamics_behavior = self.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().fluid_dynamics_behavior;
            match fluid_dynamics_behavior {
                FluidDynamicsBehavior::UseGlobalFluidRealTime => {
                    self.get_global_fluid_config().bind().speed_of_sound
                },
                FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                    self.get_current_fluid_config().bind().speed_of_sound
                },
                _ => {
                    panic!("fluid_dynamics_behavior was not *RealTime while speed_of_sound_cache was None!")
                },
            }
        })
    }
    #[func]
    pub fn get_drag_const_component(&self) -> f64 {
        self.fluid_drag_const_cache.unwrap_or_else(|| {
            let fluid_dynamics_behavior = self.config.bind().cast_fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().fluid_dynamics_behavior;
            match fluid_dynamics_behavior {
                FluidDynamicsBehavior::UseGlobalFluidRealTime => {
                    self.compute_drag_const_component(self.get_global_fluid_config())
                },
                FluidDynamicsBehavior::UseCurrentFluidRealTime => {
                    self.compute_drag_const_component(self.get_current_fluid_config())
                },
                _ => {
                    panic!("fluid_dynamics_behavior was not *RealTime while fluid_drag_const_cache was None!")
                },
            }
        })
    }
    #[func]
    pub fn try_penetrate(&mut self, cast_result: Gd<FluoriteSpaceCastResult>) -> bool {
        // this sucks, so it's abstracted away to this function
        // NOTE: it is FnMut because it needs to mutate state of self in a pragmatic implementation.
        // Typically in the `custom_data` field to accumulate penetration data, or to modify `current_velocity`
        // TODO: Maybe provide a builtin try_penetrate_rs closure later?
        let self_clo = self.object_to_owned().clone(); // this is a new handle towards itself, so we can pass it into closures without making borrowck pissed
        let mut self_config_binding = self.config.bind_mut();
        let mut cast_on_hit_cfg_bind = self_config_binding.cast_on_hit_cfg.as_mut().expect("Should always exist").bind_mut();
        let try_penetrate_via = cast_on_hit_cfg_bind.try_penetrate_via;
        match try_penetrate_via {
            MaybeExecuteCodeVia::ViaRustFnMut if let Some(associated_closure) = cast_on_hit_cfg_bind.try_penetrate_rs.as_mut() => {
                associated_closure(self_clo, cast_result)
            },
            MaybeExecuteCodeVia::ViaMethodOnResourceSnakeCase
                if let Some(method_holder) = cast_on_hit_cfg_bind.on_hit_methods_holder.as_mut()
                && method_holder.has_method("try_penetrate") => {
                    method_holder.call(
                        "try_penetrate",
                        &[
                            self_clo.to_variant(),
                            cast_result.to_variant(), // cast_result should probably be cloned in the call site if the caller still needs it downstream
                        ]
                    ).try_to().expect("try_penetrate should return bool")
                },
            MaybeExecuteCodeVia::ViaMethodOnResourcePascalCase
                if let Some(method_holder) = cast_on_hit_cfg_bind.on_hit_methods_holder.as_mut()
                && method_holder.has_method("TryPenetrate") => {
                    method_holder.call(
                        "TryPenetrate",
                        &[
                            self_clo.to_variant(),
                            cast_result.to_variant(),
                        ]
                    ).try_to().expect("TryPenetrate should return bool")
                },
            _ => { false },
        }
    }
    #[func]
    pub fn try_intersect(&self, from: Vector3, to: Vector3) -> Option<Gd<FluoriteSpaceCastResult>> {
        let binding = self.config.bind();
        let hit_detection_cfg = binding.cast_hit_detection_cfg.as_ref().expect("Should always exist").bind();
        let space_cast_result = match hit_detection_cfg.collision_detection_mode {
            CollisionDetectionMode::Ignore => { SpaceCastResult::HitNothing },
            CollisionDetectionMode::ByRaycast => {
                let mut direct_space = self.base().get_world_3d().expect("Should exist").get_direct_space_state().expect("Should exist");
                let mut query_params = PhysicsRayQueryParameters3D::new_gd();
                query_params.set_from(from);
                query_params.set_to(to);
                // TODO: Maybe cache query_params into the struct itself upon construction and wrap it in a `Cow` or something like that?? 
                // It's kind of stupid to reconstruct this entire thing every single time
                query_params.set_collision_mask(hit_detection_cfg.hit_collision_mask);
                query_params.set_collide_with_areas(hit_detection_cfg.should_collide_with_areas);
                query_params.set_collide_with_bodies(hit_detection_cfg.should_collide_with_bodies);
                query_params.set_hit_back_faces(hit_detection_cfg.should_hit_back_faces);
                query_params.set_hit_from_inside(hit_detection_cfg.should_hit_from_inside);
                query_params.set_exclude(&array![self.base().get_rid()]); // TODO: Apply exclude_list_paths in cfg (maybe cache this first)
                let res = direct_space.intersect_ray(&query_params);
                if res.contains_key("normal") {
                    let res_pos = res.get("position").expect("position should always exist").to::<Vector3>();
                    SpaceCastResult::HitByRaycast(res, res_pos - from)
                } else {
                    SpaceCastResult::HitNothing
                }
            },
            CollisionDetectionMode::ByShapecast => {
                let mut direct_space = self.base().get_world_3d().expect("Should exist").get_direct_space_state().expect("Should exist");
                let mut query_params = PhysicsShapeQueryParameters3D::new_gd();
                let diff_v3 = to - from;
                query_params.set_motion(diff_v3);
                query_params.set_transform(Transform3D::new(
                    hit_detection_cfg.shape_basis
                    * Basis::looking_at(self.current_velocity),
                    from
                )); // TODO: Basis::looking_at is a possible placeholder, recheck later
                query_params.set_collision_mask(hit_detection_cfg.hit_collision_mask);
                query_params.set_collide_with_areas(hit_detection_cfg.should_collide_with_areas);
                query_params.set_collide_with_bodies(hit_detection_cfg.should_collide_with_bodies);
                query_params.set_shape(hit_detection_cfg.hit_shape.as_ref().expect("hit_shape should always exist"));
                query_params.set_margin(hit_detection_cfg.shape_margin as f32);
                query_params.set_exclude(&array![self.base().get_rid()]); // TODO: Apply exclude_list_paths in cfg (maybe cache this first)
                let proportions = direct_space.cast_motion(&query_params);
                let safe_proportion = proportions.get(0).expect("get(0) should be Some, is hit_shape null?");
                if safe_proportion >= 1.0 {
                    SpaceCastResult::HitNothing
                } else {
                    let unsafe_proportion = proportions.get(1).expect("get(1) should be Some");
                    let unsafe_march = diff_v3*(unsafe_proportion as f32);
                    query_params.set_transform(Transform3D::new(
                        hit_detection_cfg.shape_basis
                        * Basis::looking_at(self.current_velocity),
                        from + unsafe_march
                    ));
                    let mut res = direct_space.get_rest_info(&query_params);

                    if res.contains_key("normal") {
                        let res_cid: i64 = res.get("collider_id").expect("Should exist").try_to().expect("Should be an i64");
                        let mut res_creal = None;
                        let res2 = direct_space.intersect_shape_ex(&query_params)
                            .max_results(8)
                            .done();
                        for entry in res2.iter_shared() {
                            if entry.get("collider_id").as_ref().is_some_and(|x| x.try_to::<i64>().expect("Should be an i64") == res_cid) {
                                res_creal.replace(entry.get("collider").expect("Should exist"));
                                break;
                            }
                        }
                        if res_creal.is_some() {
                            let _ = res.insert("collider", &res_creal.expect("Checked above"));
                        } else {
                            push_warning(&["Could not infer collider, so `collider` will be null".to_variant()]);
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
                    res.get("position").unwrap().to(),
                    res.get("normal").unwrap().to(),
                    res.get("rid").unwrap().to::<Rid>().to_u64() as i64,
                    res.get("collider").map(|some| some.to()),
                    res.get("collider_id").unwrap().to(),
                    res.get("shape").unwrap().to(),
                    marched_by,
                ))
            },
            SpaceCastResult::HitByShapecast(res, marched_by) => {
                // collider (injected manually), collider_id, linear_velocity, normal, point, rid, shape
                Some(FluoriteSpaceCastResult::new_result(
                    res.get("point").unwrap().to(),
                    res.get("normal").unwrap().to(),
                    res.get("rid").unwrap().to::<Rid>().to_u64() as i64,
                    res.get("collider").map(|some| some.to()),
                    res.get("collider_id").unwrap().to(),
                    res.get("shape").unwrap().to(),
                    marched_by,
                ))
            },
        }
    }
    #[func]
    pub fn get_current_fluid_config(&self) -> Gd<FluoriteFluidConfig> {
        self.get_global_fluid_config()
    }
    #[func]
    pub fn get_global_fluid_config(&self) -> Gd<FluoriteFluidConfig> {
        self.global_fluid.clone().expect("Should always exist")
    }
}

#[godot_api]
impl IStaticBody3D for FluoriteCast {
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