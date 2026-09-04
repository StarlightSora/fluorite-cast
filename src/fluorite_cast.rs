use godot::{
    classes::{
        CollisionShape3D,
        IStaticBody3D,
        ProjectSettings,
        StaticBody3D,
    },
    global::ceilf,
    prelude::*,
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
};

#[derive(GodotClass)]
#[class(init, base=StaticBody3D)]
pub struct FluoriteCast {
    base: Base<StaticBody3D>,
    payload_node: Option<Gd<Node3D>>,
    gravity_cache: Option<Vector3>,
    ambient_airspeed_cache: Option<Vector3>,
    speed_of_sound_cache: Option<f64>,
    fluid_drag_const_cache: Option<f64>,
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
            }
        });
        // The base needs to be `StaticBody3D`, and a `CollisionShape3D` is needed so we can use `get_gravity` for `UseCurrentGravityRealTime` mode
        let mut gd_colshape3d = None;

        let mut new_node_bind = new_node.bind_mut();
        let collision_mask_data = new_node_bind.config.bind().collision_mask;
        let gravity_behavior = new_node_bind.config.bind().cast_gravity_cfg.as_ref().expect("Should always exist").bind().gravity_behavior;
        let fluid_dynamics_behavior = new_node_bind.config.bind().fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().fluid_dynamics_behavior;
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
    pub fn evaluate(&mut self, delta: f64) -> () {
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
            self.evaluate_raw(sliced_delta);
        }
        self.maybe_free();
    }
    #[func]
    pub fn evaluate_raw(&mut self, delta: f64) -> () {
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
        let fluid_dynamics_cfg_bind = self_config_binding.fluid_dynamics_cfg.as_ref().expect("Should always exist").bind();
        let fluid_dynamics_fidelity = fluid_dynamics_cfg_bind.fluid_dynamics_fidelity;
        let fluid_dynamics_behavior = fluid_dynamics_cfg_bind.fluid_dynamics_behavior;
        drop(fluid_dynamics_cfg_bind);
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
            FluidDynamicsFidelity::ReynoldsNumber => {
                self.current_velocity += ambient_airspeed*(delta as f32);
                let external_airspeed = self.current_velocity - ambient_airspeed;
                self.current_velocity += self.compute_drag_reynolds(external_airspeed.length() as f64, external_airspeed.normalized())*(delta as f32);
            },
            FluidDynamicsFidelity::Full => {
                self.current_velocity += ambient_airspeed*(delta as f32);
                let external_airspeed = self.current_velocity - ambient_airspeed;
                self.current_velocity += self.compute_drag_full(external_airspeed.length() as f64, external_airspeed.normalized())*(delta as f32);
            },
        }
        let vel = self.current_velocity;
        let mut base_mut = self.base_mut();
        let starting_pos = base_mut.get_position();
        let dist = vel*(delta as f32);
        base_mut.set_global_position(starting_pos + dist);
        drop(base_mut);
        self.alive_for += delta;
        self.distance_covered += dist.length();
    }
    #[func]
    pub fn maybe_free(&mut self) -> () {
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
            self.base_mut().queue_free();
        }
    }
    #[func]
    pub fn get_config(&self) -> Gd<FluoriteCastConfig> {
        self.config.clone()
    }
    #[func]
    pub fn compute_drag_full(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        // The general idea is as follows:
        // drag = -0.5 * gas_density * airspeed^2 * ref_area * drag_coefficient * airspeed_unit_vector
        // where drag_coefficient = airspeed / (dynamic_viscosity / gas_density) * some_curve.map_to(airspeed / speed_of_sound)
        // => therefore drag = -0.5 * ref_area * airspeed^3 * gas_density^2 / dynamic_viscosity * airspeed_unit_vector * some_curve.map_to(airspeed / speed_of_sound)
        // where gas_density + ref_area + dynamic_viscosity + speed_of_sound is const
        // where airspeed + airspeed_unit_vector is mut
        // where some_curve is Curve
        self.compute_drag_reynolds(airspeed, airspeed_unit_vector) * (self.compute_drag_dyn_component_mach(airspeed) as f32)
    } 
    #[func]
    pub fn compute_drag_reynolds(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        self.get_drag_const_component() as f32 * self.compute_drag_dyn_component_reynolds(airspeed, airspeed_unit_vector)
    } 
    #[func]
    pub fn compute_drag_const_component(&self, with_fluid_cfg: Gd<FluoriteFluidConfig>) -> f64 {
        let binding = self.config.bind();
        let current_fluid_cfg = with_fluid_cfg.bind();
        let fluid_dynamics_cfg = binding.fluid_dynamics_cfg.as_ref().expect("Should always exist").bind();

        -0.5
        * fluid_dynamics_cfg.projectile_reference_area
        * current_fluid_cfg.fluid_density_kgm3 * current_fluid_cfg.fluid_density_kgm3
        / (current_fluid_cfg.dynamic_viscosity_upas * 1000.0 * 1000.0) // uPa*s -> Pa*s
    }
    #[func]
    pub fn compute_drag_dyn_component_reynolds(&self, airspeed: f64, airspeed_unit_vector: Vector3) -> Vector3 {
        (airspeed * airspeed * airspeed) as f32 * airspeed_unit_vector
    }
    #[func]
    pub fn compute_drag_dyn_component_mach(&self, airspeed: f64) -> f64 {
        if let Some(curve) = self.config.bind().fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().mach_based_drag_multiplier.as_ref() {
            curve.sample(self.get_mach_number(airspeed) as f32) as f64
        } else {
            1.0
        }
    }
    #[func]
    pub fn get_mach_number(&self, airspeed: f64) -> f64 {
        airspeed / self.speed_of_sound_cache.unwrap_or_else(|| {
            let fluid_dynamics_behavior = self.config.bind().fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().fluid_dynamics_behavior;
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
            let fluid_dynamics_behavior = self.config.bind().fluid_dynamics_cfg.as_ref().expect("Should always exist").bind().fluid_dynamics_behavior;
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
            self.evaluate(delta);
        }
    }
    fn physics_process(&mut self, delta: f64) {
        let mut can_do = false;
        if let EvaluateMode::PhysicsProcess = self.config.bind().evaluate_mode {
            can_do = true;
        }
        if can_do {
            self.evaluate(delta);
        }
    }
}